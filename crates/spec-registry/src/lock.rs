// @story 01.2
// @implements AC-06
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use domain::{CanonicalSpecKey, HarnessError};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

/// RAII inter-process advisory lock guard for a single `(project_id, spec_key)`.
/// Uses OS kernel advisory lock (`flock(LOCK_EX | LOCK_NB)` on Unix) on
/// `.cowok-locks/<sha256(project_id || "\0" || spec_key)>.lock` so crashed processes
/// automatically release locks when the kernel closes their file descriptors.
/// Because `flock` is bound to the open file description (`File::open`), every call to
/// `SpecLockGuard::acquire` opens an independent file description, providing both
/// intra-process (multi-thread) and inter-process mutual exclusion with zero orphan-file risk.
#[derive(Debug)]
pub struct SpecLockGuard {
    lock_path: PathBuf,
    flock_file: File,
}

impl SpecLockGuard {
    /// Acquires an exclusive inter-process and intra-process lock for `(project_id, spec_key)` within `timeout`.
    pub fn acquire(
        workspace_root: &Path,
        project_id: Uuid,
        spec_key: CanonicalSpecKey,
        timeout: Duration,
    ) -> Result<Self, HarnessError> {
        let locks_dir = workspace_root.join(".cowok-locks");
        fs::create_dir_all(&locks_dir).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("LOCK_UNAVAILABLE: failed to create lock directory: {e}"),
        })?;

        let mut hasher = Sha256::new();
        hasher.update(project_id.to_string().as_bytes());
        hasher.update(b"\0");
        hasher.update(spec_key.as_str().as_bytes());
        let digest = format!("{:x}", hasher.finalize());

        let lock_path = locks_dir.join(format!("{digest}.lock"));
        let start = Instant::now();

        let mut flock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("LOCK_UNAVAILABLE: failed to open lock file: {e}"),
            })?;

        loop {
            match try_acquire_os_flock(&flock_file) {
                Ok(true) => {
                    let _ = flock_file.set_len(0);
                    let _ = writeln!(flock_file, "pid={}", std::process::id());
                    let _ = flock_file.sync_all();
                    return Ok(Self {
                        lock_path,
                        flock_file,
                    });
                }
                Ok(false) => {
                    if start.elapsed() >= timeout {
                        return Err(HarnessError::SqliteBusy {
                            details: format!(
                                "LOCK_TIMEOUT: could not acquire per-spec lock for ({project_id}, {}) within {}ms",
                                spec_key.as_str(),
                                timeout.as_millis()
                            ),
                        });
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(reason) => {
                    return Err(HarnessError::ValidationFailed { reason });
                }
            }
        }
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[cfg(windows)]
#[repr(C)]
struct OVERLAPPED {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    h_event: *mut std::ffi::c_void,
}

#[cfg(windows)]
extern "system" {
    fn LockFileEx(
        hFile: *mut std::ffi::c_void,
        dwFlags: u32,
        dwReserved: u32,
        nNumberOfBytesToLockLow: u32,
        nNumberOfBytesToLockHigh: u32,
        lpOverlapped: *mut OVERLAPPED,
    ) -> i32;

    fn UnlockFileEx(
        hFile: *mut std::ffi::c_void,
        dwReserved: u32,
        nNumberOfBytesToUnlockLow: u32,
        nNumberOfBytesToUnlockHigh: u32,
        lpOverlapped: *mut OVERLAPPED,
    ) -> i32;
}

#[cfg(windows)]
const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x00000001;
#[cfg(windows)]
const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x00000002;

impl Drop for SpecLockGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::flock(self.flock_file.as_raw_fd(), libc::LOCK_UN);
        }
        #[cfg(windows)]
        unsafe {
            let mut overlapped = OVERLAPPED {
                internal: 0,
                internal_high: 0,
                offset: 0,
                offset_high: 0,
                h_event: std::ptr::null_mut(),
            };
            let handle = self.flock_file.as_raw_handle();
            let _ = UnlockFileEx(handle as *mut std::ffi::c_void, 0, 1, 0, &mut overlapped);
        }
    }
}

#[cfg(unix)]
fn try_acquire_os_flock(file: &File) -> Result<bool, String> {
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(true);
    }
    let err = std::io::Error::last_os_error();
    if err.kind() == std::io::ErrorKind::WouldBlock
        || err.raw_os_error() == Some(libc::EWOULDBLOCK)
        || err.raw_os_error() == Some(libc::EAGAIN)
    {
        return Ok(false);
    }
    Err(format!("LOCK_UNAVAILABLE: OS flock failed: {err}"))
}

#[cfg(windows)]
fn try_acquire_os_flock(file: &File) -> Result<bool, String> {
    let mut overlapped = OVERLAPPED {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        h_event: std::ptr::null_mut(),
    };
    let handle = file.as_raw_handle();
    let res = unsafe {
        LockFileEx(
            handle as *mut std::ffi::c_void,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        )
    };
    if res != 0 {
        return Ok(true);
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(33) {
        // ERROR_LOCK_VIOLATION
        return Ok(false);
    }
    Err(format!(
        "LOCK_UNAVAILABLE: Windows LockFileEx failed: {err}"
    ))
}

#[cfg(not(any(unix, windows)))]
fn try_acquire_os_flock(_file: &File) -> Result<bool, String> {
    Err("LOCK_UNAVAILABLE: advisory file locking is unsupported on non-Unix/Windows targets (fail-closed)".to_string())
}
