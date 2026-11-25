// @story 04.4
// @implements AC-02 (ActiveWorkerRegistry, -PGID Reaper, Shadow Buffer & Startup/Lease Sweepers)
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use domain::HarnessError;
use tokio::sync::watch;

pub use super::execution_sweeper::{
    cleanup_orphan_shadow_dirs, reap_orphan_jobs_on_startup, reap_orphan_jobs_with_workers,
    spawn_managed_worker_heartbeat, sweep_expired_leases, sweep_expired_leases_with_workers,
};

#[derive(Debug, Clone)]
pub struct ActiveWorkerEntry {
    pub job_id: String,
    pub project_id: String,
    pub fencing_token: i64,
    pub pid: u32,
    pub pgid: i32,
    pub start_time: String,
    pub shadow_dir: PathBuf,
}

type HeartbeatMap = Arc<Mutex<HashMap<(String, i64), watch::Sender<bool>>>>;

#[derive(Clone, Default)]
pub struct ActiveWorkerRegistry {
    inner: Arc<Mutex<HashMap<(String, i64), ActiveWorkerEntry>>>,
    heartbeats: HeartbeatMap,
}

impl ActiveWorkerRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&self, entry: ActiveWorkerEntry) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert((entry.job_id.clone(), entry.fencing_token), entry);
        }
    }
    pub fn register_heartbeat(&self, job_id: &str, fencing_token: i64, tx: watch::Sender<bool>) {
        if let Ok(mut guard) = self.heartbeats.lock() {
            if let Some(prev) = guard.insert((job_id.to_string(), fencing_token), tx) {
                let _ = prev.send(true);
            }
        }
    }
    pub fn cancel_heartbeat(&self, job_id: &str, fencing_token: i64) {
        if let Ok(mut guard) = self.heartbeats.lock() {
            if let Some(tx) = guard.remove(&(job_id.to_string(), fencing_token)) {
                let _ = tx.send(true);
            }
        }
    }
    pub fn cancel_all_heartbeats_for_job(&self, job_id: &str) {
        if let Ok(mut guard) = self.heartbeats.lock() {
            let keys: Vec<_> = guard
                .keys()
                .filter(|(jid, _)| jid == job_id)
                .cloned()
                .collect();
            for k in keys {
                if let Some(tx) = guard.remove(&k) {
                    let _ = tx.send(true);
                }
            }
        }
    }
    pub fn take_all_for_job(&self, job_id: &str) -> Vec<ActiveWorkerEntry> {
        self.cancel_all_heartbeats_for_job(job_id);
        let Ok(mut guard) = self.inner.lock() else {
            return Vec::new();
        };
        let keys: Vec<_> = guard
            .keys()
            .filter(|(jid, _)| jid == job_id)
            .cloned()
            .collect();
        keys.into_iter().filter_map(|k| guard.remove(&k)).collect()
    }
    pub fn take_by_project_and_pgid(&self, project_id: &str, pgid: i32) -> Vec<ActiveWorkerEntry> {
        let Ok(mut guard) = self.inner.lock() else {
            return Vec::new();
        };
        let keys: Vec<_> = guard
            .iter()
            .filter(|(_, entry)| entry.project_id == project_id && entry.pgid == pgid)
            .map(|(k, _)| k.clone())
            .collect();
        let taken: Vec<ActiveWorkerEntry> =
            keys.into_iter().filter_map(|k| guard.remove(&k)).collect();
        drop(guard);
        for entry in &taken {
            self.cancel_heartbeat(&entry.job_id, entry.fencing_token);
        }
        taken
    }
    pub fn take_all(&self) -> Vec<ActiveWorkerEntry> {
        if let Ok(mut hb_guard) = self.heartbeats.lock() {
            for (_, tx) in hb_guard.drain() {
                let _ = tx.send(true);
            }
        }
        let Ok(mut guard) = self.inner.lock() else {
            return Vec::new();
        };
        guard.drain().map(|(_, entry)| entry).collect()
    }
}

fn validate_safe_segment(raw: &str, label: &str) -> Result<String, HarnessError> {
    let s = raw.trim();
    if s.is_empty() || s.contains("..") || s.contains('/') || s.contains('\\') || s.contains('\0') {
        return Err(HarnessError::BoundaryViolation {
            reason: format!("SHADOW_JAIL_INVALID_SEGMENT: {label}"),
        });
    }
    Ok(s.to_string())
}

pub fn shadow_dir_for(
    workspace_root: &Path,
    project_id: &str,
    job_id: &str,
) -> Result<PathBuf, HarnessError> {
    let pid = validate_safe_segment(project_id, "project_id")?;
    let jid = validate_safe_segment(job_id, "job_id")?;
    Ok(workspace_root
        .join(".cowok-design")
        .join("shadow")
        .join(pid)
        .join(jid))
}

pub fn discard_shadow_dir(dir: &Path) -> std::io::Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn read_process_start_time(pid: u32) -> Option<String> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rparen = raw.rfind(')')?;
    let rest: Vec<&str> = raw[rparen + 1..].split_whitespace().collect();
    if *rest.first()? == "Z" {
        return None;
    }
    rest.get(19).map(|s| (*s).to_string())
}

#[cfg(not(target_os = "linux"))]
pub fn read_process_start_time(pid: u32) -> Option<String> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "state=,lstart="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() || text.starts_with('Z') {
        return None;
    }
    let mut parts = text.splitn(2, char::is_whitespace);
    let _ = parts.next()?;
    let lstart = parts.next()?.trim();
    (!lstart.is_empty()).then(|| lstart.to_string())
}

pub fn spawn_pgid_child(mut cmd: Command) -> std::io::Result<(Child, u32, i32, String)> {
    // SAFETY: pre_exec only invokes async-signal-safe libc::setpgid(0, 0) prior to exec.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let child = cmd.spawn()?;
    let pid = child.id();
    let start_time = read_process_start_time(pid).unwrap_or_default();
    Ok((child, pid, pid as i32, start_time))
}

fn pgid_or_child_alive(pid: u32, pgid: i32) -> bool {
    let mut status: libc::c_int = 0;
    // SAFETY: waitpid(WNOHANG) reaps zombie children; kill(-pgid, 0) probes process group liveness.
    unsafe {
        let _ = libc::waitpid(pid as libc::pid_t, &mut status, libc::WNOHANG);
        libc::kill(-pgid, 0) == 0
    }
}

pub fn terminate_pgid_checked(
    pid: u32,
    pgid: i32,
    expected_start_time: &str,
) -> Result<bool, String> {
    if pid == 0 || pgid <= 0 {
        return Err("INVALID_PID_OR_PGID".to_string());
    }
    let expected = expected_start_time.trim();
    if expected.is_empty() {
        return Err("MISSING_EXPECTED_START_TIME".to_string());
    }
    let Some(live_start) = read_process_start_time(pid) else {
        return Ok(false);
    };
    if live_start != expected {
        return Ok(false);
    }
    // SAFETY: verified live start time matches expected_start_time; signal only target -PGID.
    if unsafe { libc::kill(-pgid, libc::SIGTERM) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let deadline = Instant::now() + Duration::from_millis(250);
    while Instant::now() < deadline {
        if !pgid_or_child_alive(pid, pgid) {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // SAFETY: grace period elapsed; force-kill remaining -PGID processes and reap.
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
        let mut status: libc::c_int = 0;
        libc::waitpid(pid as libc::pid_t, &mut status, 0);
    }
    Ok(true)
}

fn discard_shadow_checked(dir: &Path) -> Result<(), HarnessError> {
    discard_shadow_dir(dir).map_err(|e| HarnessError::BoundaryViolation {
        reason: format!("SHADOW_DISCARD_FAILED: {e}"),
    })
}

pub fn reap_job_worker_and_shadow(
    registry: &ActiveWorkerRegistry,
    workspace_root: &Path,
    project_id: &str,
    job_id: &str,
) -> Result<bool, HarnessError> {
    let mut reaped = false;
    for entry in registry.take_all_for_job(job_id) {
        let _ = terminate_pgid_checked(entry.pid, entry.pgid, &entry.start_time);
        discard_shadow_checked(&entry.shadow_dir)?;
        reaped = true;
    }
    let canonical_shadow = shadow_dir_for(workspace_root, project_id, job_id)?;
    discard_shadow_checked(&canonical_shadow)?;
    Ok(reaped)
}
