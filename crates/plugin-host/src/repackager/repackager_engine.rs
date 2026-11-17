// @story 08.2
// @implements AC-01, AC-02, AC-05, AC-06, AC-07
use crate::models::SkillManifest;
use crate::repackager::security_scanner::{
    classify_and_validate_source_asset, is_reserved_root_filename, scan_skill_markdown_safety,
    validate_skill_metadata_text, RepackageAssetBucket,
};
use crate::validator::parse_and_validate_skill_frontmatter;
use domain::models::SkillPlugin;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::fd::FromRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub const PKG_HASH_DOMAIN: &[u8] = b"cowok-pkg-hash-v1";
pub const MAX_DISCOVERY_STUB_BYTES: usize = 1_536;
pub const MAX_SKILL_NAME_BYTES: usize = 64;
pub const MAX_VERSION_BYTES: usize = 32;
pub const MAX_DESCRIPTION_BYTES: usize = 320;
pub const MAX_UPSTREAM_URL_BYTES: usize = 256;
pub const MAX_CAPABILITIES_COUNT: usize = 16;
pub const MAX_CAPABILITY_ITEM_BYTES: usize = 48;
pub const MAX_TRAVERSAL_DEPTH: usize = 8;
pub const MAX_PACKAGE_FILES: usize = 256;
pub const MAX_SINGLE_FILE_BYTES: usize = 1_048_576; // 1 MiB
pub const MAX_TOTAL_PACKAGE_BYTES: usize = 8_388_608; // 8 MiB

static STAGING_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryStub {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_url: Option<String>,
    pub capabilities: Vec<String>,
    pub risk_tier: u8,
    pub package_sha256: String,
    pub stub_size_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedSkillEntry {
    pub path: String,
    pub packaged_path: String,
    pub sha256: String,
    pub bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsLockFile {
    pub schema_version: u32,
    pub skill_name: String,
    pub version: String,
    pub package_sha256: String,
    pub source_files: Vec<LockedSkillEntry>,
    pub generated_files: Vec<LockedSkillEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedBundleReceipt {
    pub tmp_dir: String,
    pub skill_name: String,
    pub package_sha256: String,
    pub staging_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StagingCapabilityRecord {
    tmp_name: String,
    skill_name: String,
    tmp_dev: u64,
    tmp_ino: u64,
    package_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepackageReceipt {
    pub skill_name: String,
    pub version: String,
    pub target_dir: String,
    pub files_repackaged: usize,
    pub discovery_stub: DiscoveryStub,
    pub skill_plugin: SkillPlugin,
    pub created_at: i64,
}

/// Maps a repackaged SkillManifest to canonical `domain::models::SkillPlugin` (AC-02).
pub fn manifest_to_skill_plugin(manifest: &SkillManifest) -> SkillPlugin {
    SkillPlugin::from_core(
        manifest.name.clone(),
        manifest.name.clone(),
        manifest.version.clone(),
        manifest.entry_point.clone(),
        manifest.capabilities_required.clone(),
    )
}

#[derive(Serialize)]
struct RepackagedFrontmatter<'a> {
    name: &'a str,
    version: &'a str,
    description: &'a str,
    capabilities_required: &'a [String],
    risk_tier: u8,
}

struct DiscoveredSource {
    rel_path: String,
    bucket: RepackageAssetBucket,
    raw_bytes: Vec<u8>,
    file_sha256: String,
}

pub fn validate_safe_segment(label: &str, segment: &str) -> Result<(), String> {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.len() > MAX_SKILL_NAME_BYTES
        || !segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(format!(
            "Invalid {label} '{segment}': unsafe characters, traversal segment, or length > {MAX_SKILL_NAME_BYTES}B"
        ));
    }
    Ok(())
}

fn validate_locked_rel_path(label: &str, rel_path: &str) -> Result<(), String> {
    if rel_path.is_empty()
        || rel_path.starts_with('/')
        || rel_path.ends_with('/')
        || rel_path.contains('\\')
        || rel_path.contains('\0')
        || rel_path.chars().any(|c| c.is_control())
    {
        return Err(format!(
            "Unsafe path in skills-lock ({label}): '{rel_path}'"
        ));
    }
    for seg in rel_path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(format!(
                "Path traversal segment rejected in skills-lock ({label}): '{rel_path}'"
            ));
        }
    }
    Ok(())
}

fn validate_upstream_url(raw: &str) -> Result<(), String> {
    if raw.len() > MAX_UPSTREAM_URL_BYTES {
        return Err(format!(
            "DiscoveryStub overflow: upstream_url exceeds {MAX_UPSTREAM_URL_BYTES} bytes ({})",
            raw.len()
        ));
    }
    let after_scheme = if raw
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("https://"))
    {
        &raw[8..]
    } else if raw
        .get(..7)
        .is_some_and(|p| p.eq_ignore_ascii_case("http://"))
    {
        &raw[7..]
    } else {
        return Err("Invalid upstream_url: must start with https:// or http://".to_string());
    };
    let authority = after_scheme.split('/').next().unwrap_or("");
    if authority.is_empty()
        || raw.contains('@')
        || raw.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(
            "Invalid upstream_url: malformed authority or disallowed characters".to_string(),
        );
    }
    Ok(())
}

fn validate_stub_field_budgets(
    skill_name: &str,
    description: &str,
    capabilities: &[String],
    upstream_url: Option<&str>,
) -> Result<(), String> {
    if description.len() > MAX_DESCRIPTION_BYTES {
        return Err(format!(
            "DiscoveryStub overflow: description {} bytes > {MAX_DESCRIPTION_BYTES}B",
            description.len()
        ));
    }
    if capabilities.len() > MAX_CAPABILITIES_COUNT {
        return Err(format!(
            "DiscoveryStub overflow: capabilities count {} > {MAX_CAPABILITIES_COUNT}",
            capabilities.len()
        ));
    }
    for cap in capabilities {
        if cap.len() > MAX_CAPABILITY_ITEM_BYTES {
            return Err(format!(
                "DiscoveryStub overflow: capability '{cap}' {} bytes > {MAX_CAPABILITY_ITEM_BYTES}B",
                cap.len()
            ));
        }
        validate_skill_metadata_text("capability", cap).map_err(|e| e.to_string())?;
    }
    if let Some(url) = upstream_url {
        validate_upstream_url(url)?;
    }
    if "1.0.0".len() > MAX_VERSION_BYTES || skill_name.len() > MAX_SKILL_NAME_BYTES {
        return Err("DiscoveryStub overflow: version/name exceeds ceiling".to_string());
    }
    Ok(())
}

fn build_discovery_stub_with_meta(
    skill_name: &str,
    version: &str,
    risk_tier: u8,
    description: &str,
    capabilities: Vec<String>,
    upstream_url: Option<&str>,
    package_sha256: String,
) -> Result<(DiscoveryStub, String), String> {
    let mut stub = DiscoveryStub {
        name: skill_name.to_string(),
        version: version.to_string(),
        description: description.to_string(),
        upstream_url: upstream_url.map(|s| s.to_string()),
        capabilities,
        risk_tier,
        package_sha256,
        stub_size_bytes: 0,
    };
    for _ in 0..8 {
        let serialized = serde_json::to_string_pretty(&stub).map_err(|e| e.to_string())?;
        if serialized.len() == stub.stub_size_bytes {
            if serialized.len() > MAX_DISCOVERY_STUB_BYTES {
                return Err(format!(
                    "DiscoveryStubFailClosed: serialized size {} bytes exceeds hard limit {MAX_DISCOVERY_STUB_BYTES}B",
                    serialized.len()
                ));
            }
            return Ok((stub, serialized));
        }
        stub.stub_size_bytes = serialized.len();
    }
    Err(
        "DiscoveryStubFailClosed: stub_size_bytes failed to converge to serialized byte length"
            .to_string(),
    )
}

#[cfg(test)]
fn build_discovery_stub(
    skill_name: &str,
    description: &str,
    capabilities: Vec<String>,
    upstream_url: Option<&str>,
    package_sha256: String,
) -> Result<(DiscoveryStub, String), String> {
    build_discovery_stub_with_meta(
        skill_name,
        "1.0.0",
        2,
        description,
        capabilities,
        upstream_url,
        package_sha256,
    )
}

fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn compute_package_sha256<'a, I>(entries: I) -> String
where
    I: Iterator<Item = (&'a str, &'a [u8])>,
{
    let mut hasher = Sha256::new();
    hasher.update(PKG_HASH_DOMAIN);
    for (rel_path, raw) in entries {
        let p_bytes = rel_path.as_bytes();
        hasher.update((p_bytes.len() as u64).to_be_bytes());
        hasher.update(p_bytes);
        hasher.update((raw.len() as u64).to_be_bytes());
        hasher.update(raw);
    }
    format!("{:x}", hasher.finalize())
}

fn build_validated_skill_md_with_meta(
    skill_name: &str,
    version: &str,
    risk_tier: u8,
    description: &str,
    capabilities: &[String],
) -> Result<String, String> {
    let fm = RepackagedFrontmatter {
        name: skill_name,
        version,
        description,
        capabilities_required: capabilities,
        risk_tier,
    };
    let mut yaml_block = serde_yaml::to_string(&fm).map_err(|e| e.to_string())?;
    if let Some(stripped) = yaml_block.strip_prefix("---\n") {
        yaml_block = stripped.to_string();
    }
    let yaml_trimmed = yaml_block.trim_end();
    let skill_md = format!(
        "---\n{yaml_trimmed}\n---\n\n# {skill_name} Operating Skill\n\n## 1. Overview\n{description}\n\n## 2. Repackaged Structure\nThis skill was repackaged into standard agentskills.io layout from external sources.\n"
    );
    parse_and_validate_skill_frontmatter(&skill_md)
        .map_err(|e| format!("Generated SKILL.md violated frontmatter rule: {e}"))?;
    scan_skill_markdown_safety("SKILL.md", &skill_md).map_err(|e| e.to_string())?;
    Ok(skill_md)
}

#[cfg(unix)]
mod unix_fd {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::fs::MetadataExt;

    pub struct SafeDirFd(OwnedFd);

    fn reset_errno() {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
        unsafe {
            *libc::__error() = 0;
        }
        #[cfg(target_os = "linux")]
        unsafe {
            *libc::__errno_location() = 0;
        }
    }

    impl SafeDirFd {
        pub fn as_raw(&self) -> libc::c_int {
            self.0.as_raw_fd()
        }

        pub fn open_root_nofollow(path: &Path) -> Result<Self, String> {
            use std::path::Component;
            let abs = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map_err(|e| format!("Failed resolving cwd: {e}"))?
                    .join(path)
            };
            // On macOS, /tmp, /var, and /etc are root-owned system symlinks to /private/*; resolve only those fixed OS prefixes before walking user segments with O_NOFOLLOW.
            let mut comps: Vec<String> = Vec::new();
            let mut anchor = "/";
            for comp in abs.components() {
                match comp {
                    Component::RootDir => {}
                    Component::CurDir => {}
                    Component::ParentDir => {
                        return Err(format!(
                            "Pre-flight rejected parent traversal '..' in '{}'",
                            path.display()
                        ));
                    }
                    Component::Normal(s) => {
                        let seg = s
                            .to_str()
                            .ok_or_else(|| "Non-UTF8 path component".to_string())?;
                        comps.push(seg.to_string());
                    }
                    Component::Prefix(_) => return Err("Unsupported path prefix".to_string()),
                }
            }
            #[cfg(target_os = "macos")]
            if let Some(first) = comps.first().map(|s| s.as_str()) {
                if matches!(first, "tmp" | "var" | "etc")
                    && std::fs::symlink_metadata(format!("/{first}"))
                        .is_ok_and(|m| m.file_type().is_symlink())
                {
                    anchor = match first {
                        "tmp" => "/private/tmp",
                        "var" => "/private/var",
                        "etc" => "/private/etc",
                        _ => "/",
                    };
                    comps.remove(0);
                }
            }
            let c_anchor = CString::new(anchor).map_err(|e| e.to_string())?;
            let fd = unsafe {
                libc::open(
                    c_anchor.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!(
                    "Pre-flight failed opening trusted anchor '{anchor}': {err}"
                ));
            }
            let mut cur = Self(unsafe { OwnedFd::from_raw_fd(fd) });
            for seg in comps {
                cur = cur.open_subdir_nofollow(&seg).map_err(|e| {
                    format!(
                        "Pre-flight failed opening non-symlink directory '{}' at segment '{seg}': {e}",
                        path.display()
                    )
                })?;
            }
            Ok(cur)
        }

        pub fn open_subdir_nofollow(&self, seg: &str) -> Result<Self, std::io::Error> {
            let c_seg = CString::new(seg).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL segment")
            })?;
            let fd = unsafe {
                libc::openat(
                    self.as_raw(),
                    c_seg.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self(unsafe { OwnedFd::from_raw_fd(fd) }))
        }

        pub fn mkdirat(&self, seg: &str, mode: libc::mode_t) -> Result<(), std::io::Error> {
            let c_seg = CString::new(seg).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL segment")
            })?;
            let rc = unsafe { libc::mkdirat(self.as_raw(), c_seg.as_ptr(), mode) };
            if rc < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }

        pub fn open_file_read_nofollow(&self, seg: &str) -> Result<File, String> {
            let c_seg = CString::new(seg).map_err(|_| "NUL filename".to_string())?;
            let fd = unsafe {
                libc::openat(
                    self.as_raw(),
                    c_seg.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!("Security/openat failed on '{seg}': {err}"));
            }
            let file = unsafe { File::from_raw_fd(fd) };
            let meta = file.metadata().map_err(|e| e.to_string())?;
            if !meta.is_file() {
                return Err(format!(
                    "Security scan failed: '{seg}' is not a regular file"
                ));
            }
            if meta.nlink() > 1 {
                return Err(format!(
                    "HardlinkRejected: '{seg}' has link count {} > 1",
                    meta.nlink()
                ));
            }
            Ok(file)
        }

        pub fn create_file_new_nofollow(&self, seg: &str, bytes: &[u8]) -> Result<(), String> {
            let c_seg = CString::new(seg).map_err(|_| "NUL filename".to_string())?;
            let fd = unsafe {
                libc::openat(
                    self.as_raw(),
                    c_seg.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o644 as libc::c_uint,
                )
            };
            if fd < 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!("Failed creating '{seg}' inside staging: {err}"));
            }
            let mut file = unsafe { File::from_raw_fd(fd) };
            file.write_all(bytes).map_err(|e| e.to_string())?;
            file.flush().map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())
        }

        pub fn dir_dev_ino(&self) -> Result<(u64, u64), String> {
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(self.as_raw(), &mut st) } < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok((st.st_dev as u64, st.st_ino as u64))
        }

        pub fn same_dir_identity(&self, other: &SafeDirFd) -> Result<bool, String> {
            Ok(self.dir_dev_ino()? == other.dir_dev_ino()?)
        }

        pub fn ensure_private_dir_mode(&self, label: &str) -> Result<(), String> {
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(self.as_raw(), &mut st) } < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let uid = unsafe { libc::geteuid() };
            if st.st_uid != uid {
                return Err(format!(
                    "UntrustedDirectoryOwnership: '{label}' uid {} != effective uid {uid}",
                    st.st_uid
                ));
            }
            if (st.st_mode & 0o022) != 0 {
                return Err(format!(
                    "UntrustedDirectoryPermissions: '{label}' is group/world-writable"
                ));
            }
            Ok(())
        }

        pub fn rename_entry_at(
            &self,
            from_seg: &str,
            to_dir: &SafeDirFd,
            to_seg: &str,
        ) -> Result<(), std::io::Error> {
            let c_from = CString::new(from_seg)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL"))?;
            let c_to = CString::new(to_seg)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL"))?;
            let rc = unsafe {
                libc::renameat(
                    self.as_raw(),
                    c_from.as_ptr(),
                    to_dir.as_raw(),
                    c_to.as_ptr(),
                )
            };
            if rc < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }

        pub fn unlink_at(&self, seg: &str, is_dir: bool) -> Result<(), std::io::Error> {
            let c_seg = CString::new(seg)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL"))?;
            let flag = if is_dir { libc::AT_REMOVEDIR } else { 0 };
            let rc = unsafe { libc::unlinkat(self.as_raw(), c_seg.as_ptr(), flag) };
            if rc < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }

        pub fn read_entry_names(&self) -> Result<Vec<(String, bool, bool)>, String> {
            // Use openat(".") rather than F_DUPFD so directory offset resets independently per scan.
            let c_dot = CString::new(".").map_err(|e| e.to_string())?;
            let dup_fd = unsafe {
                libc::openat(
                    self.as_raw(),
                    c_dot.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                )
            };
            if dup_fd < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let dirp = unsafe { libc::fdopendir(dup_fd) };
            if dirp.is_null() {
                let err = std::io::Error::last_os_error();
                unsafe { libc::close(dup_fd) };
                return Err(err.to_string());
            }
            let mut out = Vec::new();
            loop {
                reset_errno();
                let ent = unsafe { libc::readdir(dirp) };
                if ent.is_null() {
                    let err = std::io::Error::last_os_error();
                    if err.raw_os_error().unwrap_or(0) != 0 {
                        unsafe { libc::closedir(dirp) };
                        return Err(format!("readdir failed: {err}"));
                    }
                    break;
                }
                let raw_name = unsafe { CStr::from_ptr((*ent).d_name.as_ptr()) };
                let name = raw_name.to_string_lossy().into_owned();
                if name == "." || name == ".." {
                    continue;
                }
                let dt = unsafe { (*ent).d_type };
                let (is_dir, is_symlink) = if dt == libc::DT_DIR {
                    (true, false)
                } else if dt == libc::DT_LNK {
                    (false, true)
                } else if dt == libc::DT_REG {
                    (false, false)
                } else {
                    let c_seg = CString::new(name.as_str())
                        .map_err(|_| format!("NUL byte in filename '{name}'"))?;
                    let mut st: libc::stat = unsafe { std::mem::zeroed() };
                    let rc = unsafe {
                        libc::fstatat(
                            self.as_raw(),
                            c_seg.as_ptr(),
                            &mut st,
                            libc::AT_SYMLINK_NOFOLLOW,
                        )
                    };
                    if rc < 0 {
                        let err = std::io::Error::last_os_error();
                        unsafe { libc::closedir(dirp) };
                        return Err(format!("fstatat failed on '{name}': {err}"));
                    }
                    let fmt = st.st_mode & libc::S_IFMT;
                    (fmt == libc::S_IFDIR, fmt == libc::S_IFLNK)
                };
                out.push((name, is_dir, is_symlink));
            }
            unsafe { libc::closedir(dirp) };
            out.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(out)
        }
    }

    pub fn cleanup_dir_fd_recursive(parent_fd: &SafeDirFd, dir_name: &str) -> Result<(), String> {
        let dir_fd = parent_fd
            .open_subdir_nofollow(dir_name)
            .map_err(|e| format!("open_subdir '{dir_name}': {e}"))?;
        let entries = dir_fd.read_entry_names()?;
        for (name, is_dir, is_symlink) in entries {
            if is_symlink {
                dir_fd
                    .unlink_at(&name, false)
                    .map_err(|e| format!("unlink symlink '{name}': {e}"))?;
            } else if is_dir {
                cleanup_dir_fd_recursive(&dir_fd, &name)?;
            } else {
                dir_fd
                    .unlink_at(&name, false)
                    .map_err(|e| format!("unlink '{name}': {e}"))?;
            }
        }
        parent_fd
            .unlink_at(dir_name, true)
            .map_err(|e| format!("rmdir '{dir_name}': {e}"))
    }
}

#[cfg(unix)]
use unix_fd::{cleanup_dir_fd_recursive, SafeDirFd};

#[cfg(unix)]
fn read_bounded_file(dir_fd: &SafeDirFd, seg: &str) -> Result<Vec<u8>, String> {
    let file = dir_fd.open_file_read_nofollow(seg)?;
    let mut buf = Vec::new();
    file.take((MAX_SINGLE_FILE_BYTES + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|e| format!("Bounded read failed on '{seg}': {e}"))?;
    if buf.len() > MAX_SINGLE_FILE_BYTES {
        return Err(format!(
            "PackageQuotaExceeded: file '{seg}' exceeds single-file cap {MAX_SINGLE_FILE_BYTES}B"
        ));
    }
    Ok(buf)
}

#[cfg(unix)]
fn collect_sources_fd(
    dir_fd: &SafeDirFd,
    prefix: &str,
    depth: usize,
    total_bytes: &mut usize,
    out: &mut Vec<DiscoveredSource>,
) -> Result<(), String> {
    if depth > MAX_TRAVERSAL_DEPTH {
        return Err(format!(
            "PackageQuotaExceeded: directory depth {depth} > {MAX_TRAVERSAL_DEPTH}"
        ));
    }
    for (name, is_dir, is_symlink) in dir_fd.read_entry_names()? {
        if is_symlink {
            return Err(format!(
                "Security scan failed: symlink rejected at '{prefix}{name}'"
            ));
        }
        if name.contains('\\') || name.contains('\0') || name.chars().any(|c| c.is_control()) {
            return Err(format!(
                "Security scan failed: unsafe filename segment '{prefix}{name}'"
            ));
        }
        let rel_path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if is_dir {
            let child_fd = dir_fd
                .open_subdir_nofollow(&name)
                .map_err(|e| format!("Security scan failed: openat subdir '{rel_path}': {e}"))?;
            collect_sources_fd(&child_fd, &rel_path, depth + 1, total_bytes, out)?;
        } else {
            if is_reserved_root_filename(&rel_path) {
                return Err(format!(
                    "ReservedPackageFilenameCollision: source_dir root cannot contain '{rel_path}'"
                ));
            }
            if out.len() >= MAX_PACKAGE_FILES {
                return Err(format!(
                    "PackageQuotaExceeded: file count exceeds {MAX_PACKAGE_FILES}"
                ));
            }
            let raw_bytes = read_bounded_file(dir_fd, &name)?;
            *total_bytes += raw_bytes.len();
            if *total_bytes > MAX_TOTAL_PACKAGE_BYTES {
                return Err(format!(
                    "PackageQuotaExceeded: total package size exceeds {MAX_TOTAL_PACKAGE_BYTES}B"
                ));
            }
            let bucket = classify_and_validate_source_asset(&rel_path, &raw_bytes)
                .map_err(|e| format!("Security scan failed on '{rel_path}': {e}"))?;
            let file_sha256 = compute_sha256_hex(&raw_bytes);
            out.push(DiscoveredSource {
                rel_path,
                bucket,
                raw_bytes,
                file_sha256,
            });
        }
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_subdir_fd(root_fd: &SafeDirFd, rel_dir: &str) -> Result<SafeDirFd, String> {
    if rel_dir.is_empty() {
        return root_fd.open_subdir_nofollow(".").map_err(|e| e.to_string());
    }
    let mut cur = root_fd
        .open_subdir_nofollow(".")
        .map_err(|e| e.to_string())?;
    for seg in rel_dir.split('/') {
        if let Err(e) = cur.mkdirat(seg, 0o755) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(format!("mkdirat '{seg}' failed: {e}"));
            }
        }
        cur = cur
            .open_subdir_nofollow(seg)
            .map_err(|e| format!("open_subdir '{seg}' failed: {e}"))?;
    }
    Ok(cur)
}

#[cfg(unix)]
fn write_staged_payload(
    tmp_fd: &SafeDirFd,
    sources: &[DiscoveredSource],
    skill_md: &str,
    manifest_json: &str,
    stub_json: &str,
    lock_json: &str,
) -> Result<(), String> {
    for sub in ["scripts", "references", "assets"] {
        tmp_fd
            .mkdirat(sub, 0o755)
            .map_err(|e| format!("mkdirat '{sub}' failed: {e}"))?;
    }
    for src in sources {
        let bucket_dir = src.bucket.target_subdir();
        let packaged = format!("{bucket_dir}/{}", src.rel_path);
        let (parent_rel, file_name) = packaged.rsplit_once('/').unwrap_or(("", &packaged));
        let target_dir = ensure_subdir_fd(tmp_fd, parent_rel)?;
        target_dir.create_file_new_nofollow(file_name, &src.raw_bytes)?;
    }
    tmp_fd.create_file_new_nofollow("SKILL.md", skill_md.as_bytes())?;
    tmp_fd.create_file_new_nofollow("plugin.json", manifest_json.as_bytes())?;
    tmp_fd.create_file_new_nofollow("discovery-stub.json", stub_json.as_bytes())?;
    tmp_fd.create_file_new_nofollow("skills-lock.json", lock_json.as_bytes())?;
    Ok(())
}

const INFLIGHT_COMMIT_MARKER: &str = ".cowok-inflight";

#[cfg(unix)]
fn recover_verified_inflight_orphan_fd(
    dest_fd: &SafeDirFd,
    skill_name: &str,
) -> Result<bool, String> {
    let dst_fd = match dest_fd.open_subdir_nofollow(skill_name) {
        Ok(fd) => fd,
        Err(_) => return Ok(false),
    };
    if dst_fd.ensure_private_dir_mode(skill_name).is_err() {
        return Ok(false);
    }
    let entries = dst_fd.read_entry_names().unwrap_or_default();
    let has_lock = entries.iter().any(|(n, _, _)| n == "skills-lock.json");
    let has_marker = entries
        .iter()
        .any(|(n, is_dir, is_sym)| n == INFLIGHT_COMMIT_MARKER && !*is_dir && !*is_sym);
    if !has_lock && has_marker {
        // Verify marker itself is a regular nlink==1 file owned by current UID before cleanup.
        let _ = read_bounded_file(&dst_fd, INFLIGHT_COMMIT_MARKER)?;
        cleanup_dir_fd_recursive(dest_fd, skill_name)?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(unix)]
fn commit_repackaged_dir_fd(
    dest_fd: &SafeDirFd,
    tmp_name: &str,
    tmp_fd: &SafeDirFd,
    skill_name: &str,
    owned_dst: &mut bool,
) -> Result<(), String> {
    dest_fd.mkdirat(skill_name, 0o755).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("DestinationAlreadyExists: target '{skill_name}' already exists (EEXIST)")
        } else {
            format!("mkdirat '{skill_name}' failed: {e}")
        }
    })?;
    *owned_dst = true;

    let dst_fd = dest_fd
        .open_subdir_nofollow(skill_name)
        .map_err(|e| format!("open_subdir dst '{skill_name}' failed: {e}"))?;
    dst_fd.create_file_new_nofollow(INFLIGHT_COMMIT_MARKER, b"cowok-inflight\n")?;

    let entries = tmp_fd.read_entry_names()?;
    let present_dirs: BTreeSet<String> = entries
        .into_iter()
        .filter(|(_, is_dir, _)| *is_dir)
        .map(|(n, _, _)| n)
        .collect();

    for dir_name in ["scripts", "references", "assets"] {
        if present_dirs.contains(dir_name) {
            tmp_fd
                .rename_entry_at(dir_name, &dst_fd, dir_name)
                .map_err(|e| format!("renameat '{dir_name}' into dst failed: {e}"))?;
        }
    }
    for entry in ["SKILL.md", "plugin.json", "discovery-stub.json"] {
        tmp_fd
            .rename_entry_at(entry, &dst_fd, entry)
            .map_err(|e| format!("renameat '{entry}' into dst failed: {e}"))?;
    }
    tmp_fd
        .rename_entry_at("skills-lock.json", &dst_fd, "skills-lock.json")
        .map_err(|e| format!("renameat 'skills-lock.json' failed: {e}"))?;
    dst_fd
        .unlink_at(INFLIGHT_COMMIT_MARKER, false)
        .map_err(|e| format!("unlink inflight marker failed: {e}"))?;
    if unsafe { libc::fsync(dst_fd.as_raw()) } < 0 {
        return Err(format!(
            "fsync '{skill_name}' directory failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    dest_fd
        .unlink_at(tmp_name, true)
        .map_err(|e| format!("unlink temp dir '{tmp_name}' failed: {e}"))?;
    if unsafe { libc::fsync(dest_fd.as_raw()) } < 0 {
        return Err(format!(
            "fsync parent dest_dir failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn cleanup_with_error(
    dest_fd: &SafeDirFd,
    tmp_name: &str,
    owned_dst: Option<&str>,
    primary: String,
) -> String {
    let mut cleanup_errs = Vec::new();
    if let Some(dst_seg) = owned_dst {
        if let Err(e) = cleanup_dir_fd_recursive(dest_fd, dst_seg) {
            cleanup_errs.push(e);
        }
    }
    if let Err(e) = cleanup_dir_fd_recursive(dest_fd, tmp_name) {
        cleanup_errs.push(e);
    }
    if cleanup_errs.is_empty() {
        primary
    } else {
        format!(
            "{primary} | rollback_cleanup_error: {}",
            cleanup_errs.join("; ")
        )
    }
}

#[cfg(unix)]
fn open_nested_file_read(root_fd: &SafeDirFd, rel_path: &str) -> Result<Vec<u8>, String> {
    validate_locked_rel_path("entry", rel_path)?;
    let parts: Vec<&str> = rel_path.split('/').collect();
    let Some((file_seg, dirs)) = parts.split_last() else {
        return Err("Empty path in skills-lock verification".to_string());
    };
    let mut cur = root_fd
        .open_subdir_nofollow(".")
        .map_err(|e| e.to_string())?;
    for seg in dirs {
        cur = cur
            .open_subdir_nofollow(seg)
            .map_err(|e| format!("openat dir '{seg}' in '{rel_path}': {e}"))?;
    }
    read_bounded_file(&cur, file_seg)
}

#[cfg(unix)]
fn generate_csprng_hex_token() -> Result<String, String> {
    let mut buf = [0u8; 32];
    let c_urand = std::ffi::CString::new("/dev/urandom").map_err(|e| e.to_string())?;
    let fd = unsafe { libc::open(c_urand.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(format!(
            "Failed opening /dev/urandom: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    file.read_exact(&mut buf)
        .map_err(|e| format!("Failed reading CSPRNG capability bytes: {e}"))?;
    let mut hex = String::with_capacity(64);
    for b in buf {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{b:02x}");
    }
    Ok(hex)
}

#[cfg(unix)]
fn validate_staging_token(token: &str) -> Result<(), String> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(
            "MissingOrInvalidStagingCapability: malformed 64-hex capability token".to_string(),
        );
    }
    Ok(())
}

#[cfg(unix)]
struct PreparedStage {
    dest_fd: SafeDirFd,
    tmp_name: String,
    tmp_fd: SafeDirFd,
    manifest: SkillManifest,
    stub: DiscoveryStub,
    package_sha256: String,
    sources_count: usize,
}

#[cfg(unix)]
fn prepare_staged_payload(
    source_dir: &Path,
    dest_dir: &Path,
    skill_name: &str,
    description: &str,
    capabilities: Vec<String>,
    upstream_url: Option<&str>,
) -> Result<PreparedStage, String> {
    prepare_staged_payload_inner(
        source_dir,
        dest_dir,
        skill_name,
        description,
        capabilities,
        upstream_url,
        None,
        None,
    )
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
fn prepare_staged_payload_inner(
    source_dir: &Path,
    dest_dir: &Path,
    skill_name: &str,
    description: &str,
    capabilities: Vec<String>,
    upstream_url: Option<&str>,
    manifest_override: Option<SkillManifest>,
    skill_md_override: Option<&str>,
) -> Result<PreparedStage, String> {
    validate_safe_segment("skill_name", skill_name)?;
    validate_skill_metadata_text("description", description).map_err(|e| e.to_string())?;
    validate_stub_field_budgets(skill_name, description, &capabilities, upstream_url)?;

    let src_fd = SafeDirFd::open_root_nofollow(source_dir)?;
    let dest_fd = SafeDirFd::open_root_nofollow(dest_dir)?;
    let _ = recover_verified_inflight_orphan_fd(&dest_fd, skill_name)?;
    if let Ok(entries) = dest_fd.read_entry_names() {
        if entries.iter().any(|(n, _, _)| n == skill_name) {
            return Err(format!(
                "DestinationAlreadyExists: target '{skill_name}' already exists or is a symlink"
            ));
        }
    }

    let mut sources = Vec::new();
    let mut total_bytes = 0usize;
    collect_sources_fd(&src_fd, "", 0, &mut total_bytes, &mut sources)?;
    sources.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let package_sha256 = compute_package_sha256(
        sources
            .iter()
            .map(|s| (s.rel_path.as_str(), s.raw_bytes.as_slice())),
    );
    let (manifest_version, manifest_risk_tier) = match &manifest_override {
        Some(m) => (m.version.clone(), m.risk_tier),
        None => ("1.0.0".to_string(), 2u8),
    };
    let skill_md = match skill_md_override {
        Some(custom) => custom.to_string(),
        None => build_validated_skill_md_with_meta(
            skill_name,
            &manifest_version,
            manifest_risk_tier,
            description,
            &capabilities,
        )?,
    };
    let manifest = manifest_override.unwrap_or_else(|| SkillManifest {
        name: skill_name.to_string(),
        version: "1.0.0".to_string(),
        description: description.to_string(),
        entry_point: "SKILL.md".to_string(),
        capabilities_required: capabilities.clone(),
        risk_tier: 2,
        parameters: None,
        tags: vec!["repackaged".to_string()],
        prompt_overview: Some(description.to_string()),
    });
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    let (stub, stub_json) = build_discovery_stub_with_meta(
        skill_name,
        &manifest.version,
        manifest.risk_tier,
        &manifest.description,
        manifest.capabilities_required.clone(),
        upstream_url,
        package_sha256.clone(),
    )?;

    let source_entries: Vec<LockedSkillEntry> = sources
        .iter()
        .map(|s| LockedSkillEntry {
            path: s.rel_path.clone(),
            packaged_path: format!("{}/{}", s.bucket.target_subdir(), s.rel_path),
            sha256: s.file_sha256.clone(),
            bytes: s.raw_bytes.len(),
        })
        .collect();
    let generated_entries = vec![
        LockedSkillEntry {
            path: "SKILL.md".to_string(),
            packaged_path: "SKILL.md".to_string(),
            sha256: compute_sha256_hex(skill_md.as_bytes()),
            bytes: skill_md.len(),
        },
        LockedSkillEntry {
            path: "discovery-stub.json".to_string(),
            packaged_path: "discovery-stub.json".to_string(),
            sha256: compute_sha256_hex(stub_json.as_bytes()),
            bytes: stub_json.len(),
        },
        LockedSkillEntry {
            path: "plugin.json".to_string(),
            packaged_path: "plugin.json".to_string(),
            sha256: compute_sha256_hex(manifest_json.as_bytes()),
            bytes: manifest_json.len(),
        },
    ];
    let lock_file = SkillsLockFile {
        schema_version: 1,
        skill_name: skill_name.to_string(),
        version: manifest.version.clone(),
        package_sha256: package_sha256.clone(),
        source_files: source_entries,
        generated_files: generated_entries,
    };
    let lock_json = serde_json::to_string_pretty(&lock_file).map_err(|e| e.to_string())?;

    let seq = STAGING_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(".tmp-repackage-{}-{seq}", std::process::id());
    dest_fd
        .mkdirat(&tmp_name, 0o700)
        .map_err(|e| format!("Failed creating staging dir '{tmp_name}': {e}"))?;
    let stage_res = (|| -> Result<SafeDirFd, String> {
        let tmp_fd = dest_fd
            .open_subdir_nofollow(&tmp_name)
            .map_err(|e| format!("open_subdir '{tmp_name}': {e}"))?;
        write_staged_payload(
            &tmp_fd,
            &sources,
            &skill_md,
            &manifest_json,
            &stub_json,
            &lock_json,
        )?;
        Ok(tmp_fd)
    })();
    match stage_res {
        Ok(tmp_fd) => Ok(PreparedStage {
            dest_fd,
            tmp_name,
            tmp_fd,
            manifest,
            stub,
            package_sha256,
            sources_count: sources.len(),
        }),
        Err(primary) => Err(cleanup_with_error(&dest_fd, &tmp_name, None, primary)),
    }
}

pub struct SkillRepackager;

impl SkillRepackager {
    /// Stages a repackaged bundle under `dest_dir/.tmp-repackage-*` and issues a single-use CSPRNG capability token.
    pub fn stage_directory<P: AsRef<Path>, Q: AsRef<Path>>(
        source_dir: P,
        dest_dir: Q,
        skill_name: &str,
        description: &str,
        capabilities: Vec<String>,
        upstream_url: Option<&str>,
    ) -> Result<StagedBundleReceipt, String> {
        #[cfg(not(unix))]
        {
            let _ = (
                source_dir,
                dest_dir,
                skill_name,
                description,
                capabilities,
                upstream_url,
            );
            return Err(
                "UnsupportedPlatformSecurityCheck: FD-relative O_NOFOLLOW and hardlink nlink checks require Unix"
                    .to_string(),
            );
        }
        #[cfg(unix)]
        {
            let prep = prepare_staged_payload(
                source_dir.as_ref(),
                dest_dir.as_ref(),
                skill_name,
                description,
                capabilities,
                upstream_url,
            )?;
            let issue_res = (|| -> Result<StagedBundleReceipt, String> {
                verify_repackaged_dir_fd(&prep.tmp_fd)?;
                let (tmp_dev, tmp_ino) = prep.tmp_fd.dir_dev_ino()?;
                let token = generate_csprng_hex_token()?;
                let caps_dir = ".cowok-stage-caps";
                if let Err(e) = prep.dest_fd.mkdirat(caps_dir, 0o700) {
                    if e.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(format!("Failed creating capability dir '{caps_dir}': {e}"));
                    }
                }
                let caps_fd = prep
                    .dest_fd
                    .open_subdir_nofollow(caps_dir)
                    .map_err(|e| format!("open_subdir '{caps_dir}': {e}"))?;
                caps_fd.ensure_private_dir_mode(caps_dir)?;
                let cap_record = StagingCapabilityRecord {
                    tmp_name: prep.tmp_name.clone(),
                    skill_name: skill_name.to_string(),
                    tmp_dev,
                    tmp_ino,
                    package_sha256: prep.package_sha256.clone(),
                };
                let cap_json = serde_json::to_string(&cap_record).map_err(|e| e.to_string())?;
                caps_fd.create_file_new_nofollow(&format!("{token}.json"), cap_json.as_bytes())?;
                Ok(StagedBundleReceipt {
                    tmp_dir: dest_dir
                        .as_ref()
                        .join(&prep.tmp_name)
                        .to_string_lossy()
                        .to_string(),
                    skill_name: skill_name.to_string(),
                    package_sha256: prep.package_sha256.clone(),
                    staging_token: token,
                })
            })();
            issue_res
                .map_err(|primary| cleanup_with_error(&prep.dest_fd, &prep.tmp_name, None, primary))
        }
    }

    /// Repackages raw input files into a certified agentskills.io package via FD-relative primitives.
    pub fn repackage_directory<P: AsRef<Path>, Q: AsRef<Path>>(
        source_dir: P,
        dest_dir: Q,
        skill_name: &str,
        description: &str,
        capabilities: Vec<String>,
        upstream_url: Option<&str>,
    ) -> Result<RepackageReceipt, String> {
        #[cfg(not(unix))]
        {
            let _ = (
                source_dir,
                dest_dir,
                skill_name,
                description,
                capabilities,
                upstream_url,
            );
            return Err(
                "UnsupportedPlatformSecurityCheck: FD-relative O_NOFOLLOW and hardlink nlink checks require Unix"
                    .to_string(),
            );
        }

        #[cfg(unix)]
        {
            let prep = prepare_staged_payload(
                source_dir.as_ref(),
                dest_dir.as_ref(),
                skill_name,
                description,
                capabilities,
                upstream_url,
            )?;
            let mut owned_dst = false;
            let result = (|| -> Result<RepackageReceipt, String> {
                commit_repackaged_dir_fd(
                    &prep.dest_fd,
                    &prep.tmp_name,
                    &prep.tmp_fd,
                    skill_name,
                    &mut owned_dst,
                )?;
                let version_out = prep.manifest.version.clone();
                let skill_plugin = manifest_to_skill_plugin(&prep.manifest);
                Ok(RepackageReceipt {
                    skill_name: skill_name.to_string(),
                    version: version_out,
                    target_dir: dest_dir
                        .as_ref()
                        .join(skill_name)
                        .to_string_lossy()
                        .to_string(),
                    files_repackaged: prep.sources_count + 4,
                    discovery_stub: prep.stub,
                    skill_plugin,
                    created_at: chrono::Utc::now().timestamp(),
                })
            })();
            result.map_err(|primary| {
                cleanup_with_error(
                    &prep.dest_fd,
                    &prep.tmp_name,
                    if owned_dst { Some(skill_name) } else { None },
                    primary,
                )
            })
        }
    }

    /// Repackages staged updates while preserving exact manifest version/name/capabilities and `SKILL.md` contents.
    pub fn repackage_directory_with_manifest<P: AsRef<Path>, Q: AsRef<Path>>(
        source_dir: P,
        dest_dir: Q,
        target_folder_name: &str,
        manifest: SkillManifest,
        skill_md_override: Option<&str>,
        upstream_url: Option<&str>,
    ) -> Result<RepackageReceipt, String> {
        #[cfg(not(unix))]
        {
            let _ = (
                source_dir,
                dest_dir,
                target_folder_name,
                manifest,
                skill_md_override,
                upstream_url,
            );
            return Err("UnsupportedPlatformSecurityCheck: requires Unix O_NOFOLLOW".to_string());
        }
        #[cfg(unix)]
        {
            let prep = prepare_staged_payload_inner(
                source_dir.as_ref(),
                dest_dir.as_ref(),
                target_folder_name,
                &manifest.description.clone(),
                manifest.capabilities_required.clone(),
                upstream_url,
                Some(manifest),
                skill_md_override,
            )?;
            let mut owned_dst = false;
            let result = (|| -> Result<RepackageReceipt, String> {
                commit_repackaged_dir_fd(
                    &prep.dest_fd,
                    &prep.tmp_name,
                    &prep.tmp_fd,
                    target_folder_name,
                    &mut owned_dst,
                )?;
                let version_out = prep.manifest.version.clone();
                let skill_plugin = manifest_to_skill_plugin(&prep.manifest);
                Ok(RepackageReceipt {
                    skill_name: prep.manifest.name.clone(),
                    version: version_out,
                    target_dir: dest_dir
                        .as_ref()
                        .join(target_folder_name)
                        .to_string_lossy()
                        .to_string(),
                    files_repackaged: prep.sources_count + 4,
                    discovery_stub: prep.stub,
                    skill_plugin,
                    created_at: chrono::Utc::now().timestamp(),
                })
            })();
            result.map_err(|primary| {
                cleanup_with_error(
                    &prep.dest_fd,
                    &prep.tmp_name,
                    if owned_dst {
                        Some(target_folder_name)
                    } else {
                        None
                    },
                    primary,
                )
            })
        }
    }

    /// Verifies a published bundle exclusively through FD-relative `O_NOFOLLOW` handles and `skills-lock.json`.
    pub fn verify_repackaged_bundle<P: AsRef<Path>>(
        bundle_dir: P,
    ) -> Result<SkillsLockFile, String> {
        #[cfg(not(unix))]
        {
            let _ = bundle_dir;
            return Err("UnsupportedPlatformSecurityCheck: requires Unix O_NOFOLLOW".to_string());
        }
        #[cfg(unix)]
        {
            let root_fd = SafeDirFd::open_root_nofollow(bundle_dir.as_ref())?;
            verify_repackaged_dir_fd(&root_fd)
        }
    }

    /// Atomically publishes an engine-staged directory to `dst_dir` by consuming a single-use CSPRNG `staging_token`, verifying `(st_dev, st_ino)` + full FD integrity, and executing `mkdirat` (`EEXIST`) + `renameat`.
    pub fn commit_staged_bundle_atomic<P: AsRef<Path>, Q: AsRef<Path>>(
        tmp_dir: P,
        dst_dir: Q,
        staging_token: &str,
    ) -> Result<(), String> {
        #[cfg(not(unix))]
        {
            let _ = (tmp_dir, dst_dir, staging_token);
            return Err("UnsupportedPlatformSecurityCheck: requires Unix O_NOFOLLOW".to_string());
        }
        #[cfg(unix)]
        {
            validate_staging_token(staging_token)?;
            let tmp_ref = tmp_dir.as_ref();
            let dst_ref = dst_dir.as_ref();
            let tmp_parent = tmp_ref
                .parent()
                .ok_or_else(|| "Invalid tmp_dir: missing parent".to_string())?;
            let dst_parent = dst_ref
                .parent()
                .ok_or_else(|| "Invalid dst_dir: missing parent".to_string())?;
            let tmp_name = tmp_ref
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| "Invalid tmp_dir name".to_string())?;
            let skill_name = dst_ref
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| "Invalid dst_dir name".to_string())?;
            validate_safe_segment("tmp_name", tmp_name)?;
            validate_safe_segment("skill_name", skill_name)?;
            if !tmp_name.starts_with(".tmp-") {
                return Err(format!(
                    "InvalidStagingPrefix: tmp_dir '{tmp_name}' must be a '.tmp-*' staging directory"
                ));
            }
            let tmp_parent_fd = SafeDirFd::open_root_nofollow(tmp_parent)?;
            let dest_fd = SafeDirFd::open_root_nofollow(dst_parent)?;
            if !tmp_parent_fd.same_dir_identity(&dest_fd)? {
                return Err(
                    "CrossDirectoryCommitRejected: tmp_dir and dst_dir parents differ".to_string(),
                );
            }

            let caps_fd = dest_fd
                .open_subdir_nofollow(".cowok-stage-caps")
                .map_err(|_| {
                    "MissingOrInvalidStagingCapability: staging capability registry absent"
                        .to_string()
                })?;
            caps_fd.ensure_private_dir_mode(".cowok-stage-caps")?;
            let cap_filename = format!("{staging_token}.json");
            let cap_bytes = read_bounded_file(&caps_fd, &cap_filename).map_err(|_| {
                "MissingOrInvalidStagingCapability: unknown or already-consumed staging_token"
                    .to_string()
            })?;
            // Single-use atomic capability consumption BEFORE commit attempt.
            caps_fd.unlink_at(&cap_filename, false).map_err(|e| {
                format!("CapabilityConsumeFailed: unable to consume staging_token ({e})")
            })?;
            let _ = dest_fd.unlink_at(".cowok-stage-caps", true);

            let record: StagingCapabilityRecord = serde_json::from_slice(&cap_bytes)
                .map_err(|e| format!("CorruptStagingCapability: {e}"))?;
            if record.tmp_name != tmp_name || record.skill_name != skill_name {
                return Err("StagingProvenanceMismatch: staging_token bound to a different staging dir or target skill".to_string());
            }

            let tmp_fd = dest_fd
                .open_subdir_nofollow(tmp_name)
                .map_err(|e| format!("open_subdir '{tmp_name}': {e}"))?;
            tmp_fd.ensure_private_dir_mode(tmp_name)?;
            if tmp_fd.dir_dev_ino()? != (record.tmp_dev, record.tmp_ino) {
                return Err("StagingProvenanceMismatch: tmp_dir inode changed since staging capability issuance".to_string());
            }
            // Re-verify full staging integrity, quotas, zero symlinks/hardlinks, and cross-artifact parity via pinned FD before mkdirat(dst).
            let verified_lock = verify_repackaged_dir_fd(&tmp_fd)?;
            if verified_lock.package_sha256 != record.package_sha256 {
                return Err(
                    "StagingProvenanceMismatch: package_sha256 diverged from issued capability"
                        .to_string(),
                );
            }

            let mut owned_dst = false;
            if let Err(primary) =
                commit_repackaged_dir_fd(&dest_fd, tmp_name, &tmp_fd, skill_name, &mut owned_dst)
            {
                let mut errs = Vec::new();
                if owned_dst {
                    if let Err(cleanup_err) = cleanup_dir_fd_recursive(&dest_fd, skill_name) {
                        errs.push(cleanup_err);
                    }
                }
                if let Err(tmp_cleanup_err) = cleanup_dir_fd_recursive(&dest_fd, tmp_name) {
                    errs.push(tmp_cleanup_err);
                }
                if !errs.is_empty() {
                    return Err(format!(
                        "{primary} | rollback_cleanup_error: {}",
                        errs.join("; ")
                    ));
                }
                return Err(primary);
            }
            Ok(())
        }
    }
}

#[cfg(unix)]
fn collect_bundle_subtree_fd(
    dir_fd: &SafeDirFd,
    prefix: &str,
    depth: usize,
    total_bytes: &mut usize,
    disk_files: &mut BTreeSet<String>,
) -> Result<(), String> {
    if depth > MAX_TRAVERSAL_DEPTH {
        return Err(format!(
            "PackageQuotaExceeded: bundle directory depth {depth} > {MAX_TRAVERSAL_DEPTH}"
        ));
    }
    for (name, is_dir, is_symlink) in dir_fd.read_entry_names()? {
        if is_symlink {
            return Err(format!(
                "Security scan failed: symlink rejected at '{prefix}/{name}'"
            ));
        }
        if name.contains('\\') || name.contains('\0') || name.chars().any(|c| c.is_control()) {
            return Err(format!(
                "BundleIntegrityMismatch: unsafe filename segment '{prefix}/{name}'"
            ));
        }
        let rel = format!("{prefix}/{name}");
        if is_dir {
            let child_fd = dir_fd
                .open_subdir_nofollow(&name)
                .map_err(|e| format!("Security scan failed: openat subdir '{rel}': {e}"))?;
            collect_bundle_subtree_fd(&child_fd, &rel, depth + 1, total_bytes, disk_files)?;
        } else {
            if disk_files.len() >= MAX_PACKAGE_FILES {
                return Err(format!(
                    "PackageQuotaExceeded: bundle file count exceeds {MAX_PACKAGE_FILES}"
                ));
            }
            let bytes = read_bounded_file(dir_fd, &name)?;
            *total_bytes += bytes.len();
            if *total_bytes > MAX_TOTAL_PACKAGE_BYTES {
                return Err(format!(
                    "PackageQuotaExceeded: bundle size exceeds {MAX_TOTAL_PACKAGE_BYTES}B"
                ));
            }
            disk_files.insert(rel);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn collect_all_bundle_disk_files_fd(root_fd: &SafeDirFd) -> Result<BTreeSet<String>, String> {
    let allowed_root_files: BTreeSet<&str> = [
        "SKILL.md",
        "discovery-stub.json",
        "plugin.json",
        "skills-lock.json",
    ]
    .into_iter()
    .collect();
    let allowed_root_dirs: BTreeSet<&str> =
        ["scripts", "references", "assets"].into_iter().collect();

    let mut total_bytes = 0usize;
    let mut disk_files = BTreeSet::new();
    for (name, is_dir, is_symlink) in root_fd.read_entry_names()? {
        if is_symlink {
            return Err(format!(
                "Security scan failed: symlink rejected at '{name}'"
            ));
        }
        if is_dir {
            if !allowed_root_dirs.contains(name.as_str()) {
                return Err(format!(
                    "BundleIntegrityMismatch: untracked directory '{name}' in bundle root"
                ));
            }
            let sub_fd = root_fd
                .open_subdir_nofollow(&name)
                .map_err(|e| format!("Security scan failed: openat subdir '{name}': {e}"))?;
            collect_bundle_subtree_fd(&sub_fd, &name, 1, &mut total_bytes, &mut disk_files)?;
        } else {
            if !allowed_root_files.contains(name.as_str()) {
                return Err(format!(
                    "BundleIntegrityMismatch: untracked root file '{name}' in bundle"
                ));
            }
            let bytes = read_bounded_file(root_fd, &name)?;
            total_bytes += bytes.len();
            if total_bytes > MAX_TOTAL_PACKAGE_BYTES {
                return Err(format!(
                    "PackageQuotaExceeded: bundle size exceeds {MAX_TOTAL_PACKAGE_BYTES}B"
                ));
            }
            disk_files.insert(name);
        }
    }
    Ok(disk_files)
}

#[cfg(unix)]
fn verify_repackaged_dir_fd(root_fd: &SafeDirFd) -> Result<SkillsLockFile, String> {
    let lock_bytes = read_bounded_file(root_fd, "skills-lock.json").map_err(|e| {
        format!("IncompleteOrTamperedBundle: missing/invalid skills-lock.json ({e})")
    })?;
    let lock: SkillsLockFile = serde_json::from_slice(&lock_bytes)
        .map_err(|e| format!("Corrupt skills-lock.json: {e}"))?;
    if lock.schema_version != 1 {
        return Err("Unsupported skills-lock schema_version".to_string());
    }
    validate_safe_segment("skill_name", &lock.skill_name)?;

    let disk_files = collect_all_bundle_disk_files_fd(root_fd)?;

    let mut seen_src = BTreeSet::new();
    let mut seen_pkg = BTreeSet::new();
    let mut expected_files: BTreeSet<String> =
        ["skills-lock.json".to_string()].into_iter().collect();
    let mut recomputed_sources: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for entry in &lock.source_files {
        validate_locked_rel_path("source path", &entry.path)?;
        validate_locked_rel_path("source packaged_path", &entry.packaged_path)?;
        let has_bucket = ["scripts/", "references/", "assets/"]
            .iter()
            .any(|p| entry.packaged_path.starts_with(p));
        if !has_bucket {
            return Err(format!(
                "BundleIntegrityMismatch: invalid bucket path '{}'",
                entry.packaged_path
            ));
        }
        if !seen_src.insert(entry.path.clone()) || !seen_pkg.insert(entry.packaged_path.clone()) {
            return Err(format!(
                "BundleIntegrityMismatch: duplicate entry '{}'",
                entry.path
            ));
        }
        let bytes = open_nested_file_read(root_fd, &entry.packaged_path)?;
        let digest = compute_sha256_hex(&bytes);
        if bytes.len() != entry.bytes || digest != entry.sha256 {
            return Err(format!(
                "BundleIntegrityMismatch: '{}' digest/size mismatch",
                entry.packaged_path
            ));
        }
        let expected_bucket = classify_and_validate_source_asset(&entry.path, &bytes)
            .map_err(|e| format!("Security scan failed on '{}': {e}", entry.packaged_path))?;
        let expected_prefix = format!("{}/", expected_bucket.target_subdir());
        if !entry.packaged_path.starts_with(&expected_prefix) {
            return Err(format!(
                "BundleIntegrityMismatch: '{}' placed outside expected bucket '{expected_prefix}'",
                entry.packaged_path
            ));
        }
        expected_files.insert(entry.packaged_path.clone());
        recomputed_sources.insert(entry.path.clone(), bytes);
    }

    let required_gen: BTreeSet<&str> = ["SKILL.md", "discovery-stub.json", "plugin.json"]
        .into_iter()
        .collect();
    let mut seen_gen = BTreeSet::new();
    let mut fm_meta = None;
    let mut parsed_manifest: Option<SkillManifest> = None;
    let mut parsed_stub: Option<(DiscoveryStub, usize)> = None;

    for entry in &lock.generated_files {
        validate_locked_rel_path("generated path", &entry.path)?;
        validate_locked_rel_path("generated packaged_path", &entry.packaged_path)?;
        if entry.path != entry.packaged_path
            || !required_gen.contains(entry.path.as_str())
            || !seen_gen.insert(entry.path.as_str())
        {
            return Err(format!(
                "BundleIntegrityMismatch: invalid or duplicate generated entry '{}'",
                entry.path
            ));
        }
        let bytes = open_nested_file_read(root_fd, &entry.packaged_path)?;
        let digest = compute_sha256_hex(&bytes);
        if bytes.len() != entry.bytes || digest != entry.sha256 {
            return Err(format!(
                "BundleIntegrityMismatch: '{}' digest/size mismatch",
                entry.packaged_path
            ));
        }
        expected_files.insert(entry.packaged_path.clone());
        if entry.path == "SKILL.md" {
            let text =
                std::str::from_utf8(&bytes).map_err(|_| "SKILL.md is invalid UTF-8".to_string())?;
            let meta = parse_and_validate_skill_frontmatter(text)
                .map_err(|e| format!("SKILL.md frontmatter invalid: {e}"))?;
            scan_skill_markdown_safety("SKILL.md", text).map_err(|e| e.to_string())?;
            if meta.name != lock.skill_name
                || meta.version.as_deref() != Some(lock.version.as_str())
            {
                return Err(
                    "BundleIntegrityMismatch: SKILL.md identity does not match skills-lock.json"
                        .to_string(),
                );
            }
            fm_meta = Some(meta);
        } else if entry.path == "plugin.json" {
            let manifest: SkillManifest = serde_json::from_slice(&bytes)
                .map_err(|e| format!("plugin.json invalid SkillManifest JSON: {e}"))?;
            let plugin = manifest_to_skill_plugin(&manifest);
            if plugin.id.is_empty()
                || plugin.version.is_empty()
                || plugin.entry_point != "SKILL.md"
                || manifest.name != lock.skill_name
                || manifest.version != lock.version
            {
                return Err(
                    "BundleIntegrityMismatch: plugin.json identity does not match skills-lock.json"
                        .to_string(),
                );
            }
            parsed_manifest = Some(manifest);
        } else if entry.path == "discovery-stub.json" {
            if bytes.len() > MAX_DISCOVERY_STUB_BYTES {
                return Err(format!(
                    "BundleIntegrityMismatch: discovery-stub.json {} bytes > {MAX_DISCOVERY_STUB_BYTES}B",
                    bytes.len()
                ));
            }
            let stub: DiscoveryStub = serde_json::from_slice(&bytes)
                .map_err(|e| format!("discovery-stub.json invalid JSON: {e}"))?;
            validate_stub_field_budgets(
                &stub.name,
                &stub.description,
                &stub.capabilities,
                stub.upstream_url.as_deref(),
            )?;
            validate_skill_metadata_text("description", &stub.description)
                .map_err(|e| e.to_string())?;
            for cap in &stub.capabilities {
                validate_skill_metadata_text("capability", cap).map_err(|e| e.to_string())?;
            }
            if stub.stub_size_bytes != bytes.len() {
                return Err(format!(
                    "BundleIntegrityMismatch: discovery-stub.json stub_size_bytes {} != disk bytes {}",
                    stub.stub_size_bytes,
                    bytes.len()
                ));
            }
            parsed_stub = Some((stub, bytes.len()));
        }
    }
    if seen_gen != required_gen {
        return Err(
            "BundleIntegrityMismatch: generated_files missing required generated artifact(s)"
                .to_string(),
        );
    }
    if disk_files != expected_files {
        let extra: Vec<String> = disk_files.difference(&expected_files).cloned().collect();
        return Err(format!(
            "BundleIntegrityMismatch: untracked or missing files relative to skills-lock.json ({})",
            extra.join(", ")
        ));
    }

    let recomputed_pkg = compute_package_sha256(
        recomputed_sources
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice())),
    );
    if recomputed_pkg != lock.package_sha256 {
        return Err(format!(
            "BundleIntegrityMismatch: package_sha256 expected {}, computed {recomputed_pkg}",
            lock.package_sha256
        ));
    }
    if let (Some(fm), Some(manifest), Some((stub, _))) = (fm_meta, parsed_manifest, parsed_stub) {
        if fm.risk_tier != Some(manifest.risk_tier)
            || fm.description != manifest.description
            || fm.capabilities_required != manifest.capabilities_required
            || stub.name != lock.skill_name
            || stub.version != lock.version
            || stub.risk_tier != manifest.risk_tier
            || stub.description != manifest.description
            || stub.capabilities != manifest.capabilities_required
            || stub.package_sha256 != lock.package_sha256
        {
            return Err("BundleIntegrityMismatch: cross-artifact metadata divergence across SKILL.md, plugin.json, and discovery-stub.json".to_string());
        }
    }
    Ok(lock)
}

/// Loads a `StagedPluginBundle` from disk strictly through Story 08.2 `SafeDirFd` (`O_NOFOLLOW`, `nlink == 1`)
/// and `skills-lock.json` integrity verification (`SEC-083-05`).
pub fn load_verified_staged_bundle_from_dir(
    dir: &Path,
    expected_package_sha256: Option<&str>,
) -> Result<crate::lifecycle::StagedPluginBundle, String> {
    #[cfg(unix)]
    {
        let root_fd = SafeDirFd::open_root_nofollow(dir)?;
        let lock = verify_repackaged_dir_fd(&root_fd)?;
        if let Some(exp_sha) = expected_package_sha256 {
            let clean = exp_sha.trim();
            if clean.is_empty() || lock.package_sha256 != clean {
                return Err(format!(
                    "BundleIntegrityMismatch: expected package_sha256 '{clean}', got '{}'",
                    lock.package_sha256
                ));
            }
        }
        let manifest_bytes = open_nested_file_read(&root_fd, "plugin.json")?;
        let manifest: SkillManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| format!("Invalid plugin.json: {e}"))?;
        let skill_md_bytes = open_nested_file_read(&root_fd, "SKILL.md")?;
        let skill_md = String::from_utf8(skill_md_bytes)
            .map_err(|_| "SKILL.md must be valid UTF-8".to_string())?;
        let mut scripts = Vec::new();
        for entry in &lock.source_files {
            if entry.packaged_path.starts_with("scripts/") {
                let raw = open_nested_file_read(&root_fd, &entry.packaged_path)?;
                let content = String::from_utf8(raw).map_err(|_| {
                    format!(
                        "Staged script '{}' must be valid UTF-8",
                        entry.packaged_path
                    )
                })?;
                scripts.push(crate::lifecycle::StagedScriptFile {
                    rel_path: entry.packaged_path.clone(),
                    content,
                });
            }
        }
        Ok(crate::lifecycle::StagedPluginBundle {
            manifest,
            skill_md,
            scripts,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = (dir, expected_package_sha256);
        Err("Secure FD staging verification requires Unix O_NOFOLLOW support".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_repackages_clean_skill_directory() {
        let src_tmp = tempfile::tempdir().unwrap();
        let dst_tmp = tempfile::tempdir().unwrap();

        std::fs::write(
            src_tmp.path().join("helper.py"),
            "def greet(): return 'hello'",
        )
        .unwrap();
        std::fs::write(
            src_tmp.path().join("guide.md"),
            "# Guide\nInstructions here",
        )
        .unwrap();

        let receipt = SkillRepackager::repackage_directory(
            src_tmp.path(),
            dst_tmp.path(),
            "external-helper",
            "A clean helper skill repackaged from upstream",
            vec!["helpers".to_string()],
            Some("https://github.com/example/helper"),
        )
        .expect("Repackaging should succeed");

        assert_eq!(receipt.skill_name, "external-helper");
        assert!(receipt.files_repackaged >= 4);

        let target_dir = PathBuf::from(&receipt.target_dir);
        assert!(target_dir.join("SKILL.md").exists());
        assert!(target_dir.join("plugin.json").exists());
        assert!(target_dir.join("discovery-stub.json").exists());
        assert!(target_dir.join("skills-lock.json").exists());
        assert!(target_dir.join("scripts/helper.py").exists());
        assert!(target_dir.join("references/guide.md").exists());
        SkillRepackager::verify_repackaged_bundle(&target_dir).expect("Bundle lock verified");
    }

    #[test]
    fn test_rejects_malicious_script_during_repackage() {
        let src_tmp = tempfile::tempdir().unwrap();
        let dst_tmp = tempfile::tempdir().unwrap();

        std::fs::write(src_tmp.path().join("evil.sh"), "curl | bash").unwrap();

        let err = SkillRepackager::repackage_directory(
            src_tmp.path(),
            dst_tmp.path(),
            "evil-skill",
            "Malicious skill",
            vec![],
            None,
        )
        .unwrap_err();

        assert!(err.contains("Security scan failed"));
    }

    #[cfg(unix)]
    #[test]
    fn test_commit_empty_dest_collision_unit() {
        let dst_dir = tempfile::tempdir().unwrap();
        let empty_dst = dst_dir.path().join("race-empty");
        std::fs::create_dir(&empty_dst).unwrap();

        let dest_fd = SafeDirFd::open_root_nofollow(dst_dir.path()).unwrap();
        let tmp_name = ".tmp-race-probe";
        dest_fd.mkdirat(tmp_name, 0o700).unwrap();
        let tmp_fd = dest_fd.open_subdir_nofollow(tmp_name).unwrap();
        let mut owned_dst = false;
        let err =
            commit_repackaged_dir_fd(&dest_fd, tmp_name, &tmp_fd, "race-empty", &mut owned_dst)
                .unwrap_err();
        let merged = cleanup_with_error(&dest_fd, tmp_name, None, err);
        assert!(merged.contains("DestinationAlreadyExists"));
        assert!(empty_dst.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn test_cleanup_with_error_hook_unit() {
        use std::os::unix::fs::PermissionsExt;
        let locked_root = tempfile::tempdir().unwrap();
        let stuck_tmp = locked_root.path().join(".tmp-stuck");
        std::fs::create_dir(&stuck_tmp).unwrap();
        std::fs::write(stuck_tmp.join("f.txt"), "x").unwrap();
        std::fs::set_permissions(&stuck_tmp, std::fs::Permissions::from_mode(0o555)).unwrap();
        let dest_fd = SafeDirFd::open_root_nofollow(locked_root.path()).unwrap();
        let err = cleanup_with_error(&dest_fd, ".tmp-stuck", None, "PrimaryFail".to_string());
        std::fs::set_permissions(&stuck_tmp, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(err.contains("PrimaryFail") && err.contains("rollback_cleanup_error"));
    }

    #[cfg(unix)]
    #[test]
    fn test_commit_staged_bundle_atomic_rolls_back_partial_dst() {
        let root = tempfile::tempdir().unwrap();
        let staged = root.path().join(".tmp-incomplete");
        let dst = root.path().join("partial-skill");
        std::fs::create_dir_all(staged.join("scripts")).unwrap();
        std::fs::create_dir_all(staged.join("references")).unwrap();
        std::fs::create_dir_all(staged.join("assets")).unwrap();
        std::fs::write(staged.join("SKILL.md"), "# Incomplete").unwrap();

        let forged_tok = "a".repeat(64);
        let err =
            SkillRepackager::commit_staged_bundle_atomic(&staged, &dst, &forged_tok).unwrap_err();
        assert!(err.contains("MissingOrInvalidStagingCapability"));
        assert!(
            !dst.exists(),
            "destination must not be created with forged capability"
        );

        let dest_fd = SafeDirFd::open_root_nofollow(root.path()).unwrap();
        let tmp_fd = dest_fd.open_subdir_nofollow(".tmp-incomplete").unwrap();
        let mut owned_dst = false;
        let commit_err = commit_repackaged_dir_fd(
            &dest_fd,
            ".tmp-incomplete",
            &tmp_fd,
            "partial-skill",
            &mut owned_dst,
        )
        .unwrap_err();
        assert!(commit_err.contains("plugin.json") && owned_dst);
        cleanup_dir_fd_recursive(&dest_fd, "partial-skill").unwrap();
        assert!(
            !dst.exists(),
            "partial destination must be rolled back on mid-commit failure"
        );
    }

    #[test]
    fn test_discovery_stub_combined_overflow_unit() {
        let err = build_discovery_stub(
            &"s".repeat(64),
            &"d".repeat(320),
            vec!["c".repeat(48); 16],
            Some(&format!("https://example.com/{}", "u".repeat(236))),
            "a".repeat(64),
        )
        .unwrap_err();
        assert!(err.contains("DiscoveryStubFailClosed"));
    }
}
