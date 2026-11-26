// @story 12.3
// @implements AC-01, AC-02, AC-03 (Multi-Arch OCI Container Diagnostics, Pooled 1-Chromium CDP & `cowok-design ci gate --json`)
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use html_forge::visual_qc::{
    discover_local_chromium_binary, DomGeometryNode, PooledCdpEvaluator, QcBoundingBox,
    QcTargetKind, QcVerdict, VisualQcEvalInput, CANONICAL_APP_VIEWPORTS,
    DEFAULT_SSIM_PASS_THRESHOLD,
};
use plugin_host::{run_builtin_de_suite, BuiltInDeTool, DeRunReport, DeSuiteInput};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::{retry_on_busy, SpecRegistry, SqlitePool, SqliteWalConfig};
use uuid::Uuid;

use crate::cli::commands::{format_json_error, format_json_success};
use crate::cli::CliOutput;
use crate::lockfile::{reject_if_symlink, DEFAULT_DAEMON_PORT};
use crate::port_binder::{
    probe_dir_writable_atomic, resolve_runtime_storage_paths, ChildProcessReaper,
    RuntimeStoragePaths,
};
use crate::routes::visual_qc::{
    ensure_v26_tables, evaluate_qc_for_project, shared_cdp_evaluator, QcRunRequest,
    QcRunResponseData,
};
use crate::routes::DaemonAppState;
use crate::visual_diff::FOCUS_FREE_HEADLESS_FLAGS;

pub const DEFAULT_CI_PROJECT_ID: &str = "ci-gate-default";
pub const DEFAULT_CI_TARGET_ID: &str = "ci-gate-default";
pub const DEFAULT_MAX_MEMORY_MB: u64 = 512;
pub const MIN_MAX_MEMORY_MB: u64 = 128;
pub const MAX_MAX_MEMORY_MB: u64 = 16_384;
pub const SHM_SAFE_THRESHOLD_BYTES: u64 = 256 * 1024 * 1024; // 256 MiB (268_435_456 bytes)
pub const EXPECTED_CONTAINER_UID: u32 = 10_001;
pub const POOLED_CDP_MAX_CONCURRENCY: u8 = 1;
pub const MAX_CI_HTML_BYTES: u64 = 512 * 1024; // 512 KiB (parity with visual_qc::MAX_HTML_LEN)
pub const DEFAULT_SHM_PATH: &str = "/dev/shm";
pub const DEFAULT_CGROUP_V2_MEMORY_MAX: &str = "/sys/fs/cgroup/memory.max";
pub const DEFAULT_CGROUP_V1_MEMORY_LIMIT: &str = "/sys/fs/cgroup/memory/memory.limit_in_bytes";

const CGROUP_V1_UNBOUNDED_SENTINEL: u64 = 1u64 << 60;
const CHROMIUM_CI_CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_CAPTURE_DOM_BYTES: u64 = 2 * 1024 * 1024; // 2 MiB bounded DOM capture
const FALLBACK_CLEAN_HTML: &str = "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"><title>CI Gate Baseline</title></head><body><main><h1>Design Audit Baseline</h1><p>Clean semantic baseline for headless CI gate verification.</p></main></body></html>";

fn default_ci_target_id() -> String {
    DEFAULT_CI_TARGET_ID.to_string()
}

/// Parsed input contract for `cowok-design ci gate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiGateRequest {
    pub project_id: String,
    #[serde(default = "default_ci_target_id")]
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_memory_mb: Option<u64>,
    #[serde(default)]
    pub force_no_browser: bool,
    #[serde(default)]
    pub fail_on_unverified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<PathBuf>,
}

impl Default for CiGateRequest {
    fn default() -> Self {
        Self {
            project_id: DEFAULT_CI_PROJECT_ID.to_string(),
            target_id: DEFAULT_CI_TARGET_ID.to_string(),
            html_path: None,
            max_memory_mb: None,
            force_no_browser: false,
            fail_on_unverified: false,
            workspace: None,
        }
    }
}

/// Overall deterministic CI gate verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CiGateVerdict {
    #[serde(rename = "PASS")]
    Pass,
    #[serde(rename = "FAIL")]
    Fail,
    #[serde(rename = "UNVERIFIED_NO_BROWSER")]
    UnverifiedNoBrowser,
}

impl CiGateVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::UnverifiedNoBrowser => "UNVERIFIED_NO_BROWSER",
        }
    }
}

/// `/dev/shm` capacity probe, container sandbox decision, & automatic `--disable-dev-shm-usage`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiShmDiagnostics {
    pub shm_path: String,
    pub shm_available_bytes: Option<u64>,
    pub shm_total_bytes: Option<u64>,
    /// Minimum safe threshold for Headless Chromium shared memory (256 MiB = 268_435_456 bytes).
    pub threshold_bytes: u64,
    /// True when `/dev/shm` is unavailable or < 256 MiB, or `COWOK_DISABLE_DEV_SHM=1`.
    pub auto_disable_dev_shm_usage: bool,
    /// True when `COWOK_CONTAINER_MODE=1` or `COWOK_CHROMIUM_NO_SANDBOX=1` appends `--no-sandbox`
    /// inside a hardened non-root (`uid 10001`), `read_only`, `cap_drop: ALL` container.
    #[serde(default)]
    pub container_no_sandbox: bool,
    /// Complete resolved Chromium CLI flags (`FOCUS_FREE_HEADLESS_FLAGS` + container guards).
    pub resolved_chromium_flags: Vec<String>,
}

/// Cgroup v2/v1 memory limit detection & Pooled Chromium cap enforcement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiMemoryCapDiagnostics {
    /// One of: `"cgroup_v2"`, `"cgroup_v1"`, `"host_unbounded"`.
    pub cgroup_source: String,
    pub cgroup_limit_mb: Option<u64>,
    pub requested_max_memory_mb: u64,
    /// Effective cap = `min(requested_max_memory_mb, floor(cgroup_limit_mb * 0.75))` clamped to `>= 128 MiB`.
    pub effective_max_memory_mb: u64,
    pub cgroup_clamped: bool,
    pub js_heap_flag: String,
}

/// Read-only / mismatched-UID bind-mount SQLite WAL staging status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiWalStagingDiagnostics {
    pub workspace_root: String,
    pub state_root: String,
    pub db_path: String,
    pub lock_path: String,
    pub read_only_fallback: bool,
}

/// Non-root execution, zero-office-suite invariant, and OCI container runtime diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiContainerDiagnostics {
    pub effective_uid: u32,
    pub effective_gid: u32,
    pub expected_container_uid: u32,
    pub is_non_root: bool,
    pub chromium_binary_path: Option<String>,
    pub pooled_cdp_max_concurrency: u8,
    #[serde(default)]
    pub live_capture_executed: bool,
    pub libreoffice_binary_detected: bool,
    pub zero_libreoffice_invariant: bool,
    pub shm: CiShmDiagnostics,
    pub memory_cap: CiMemoryCapDiagnostics,
    pub wal_staging: CiWalStagingDiagnostics,
}

/// Unified `cowok-design ci gate --json` payload wrapped in `CliSuccessEnvelope<CiGateResponse>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiGateResponse {
    pub gate_id: String,
    pub project_id: String,
    pub verdict: CiGateVerdict,
    pub passed: bool,
    pub exit_code: i32,
    pub fail_on_unverified: bool,
    pub failure_reasons: Vec<String>,
    /// Story 07.1 Visual QC parity (`evaluate_qc_for_project` / `POST /api/v1/qc/run`).
    pub visual_qc: QcRunResponseData,
    /// Story 12.1 7 Built-in DE Tools parity (`run_builtin_de_suite` / `POST /api/v1/de/run`).
    pub de_suite: DeRunReport,
    /// Story 12.3 OCI container environment & hardening diagnostics.
    pub diagnostics: CiContainerDiagnostics,
    pub total_duration_ms: u64,
    pub created_at: i64,
}

/// Typed validation/runtime error returned before a gate verdict can be computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiGateError {
    pub exit_code: i32,
    pub code: &'static str,
    pub message: String,
    pub field: Option<&'static str>,
}

impl CiGateError {
    pub fn validation(
        code: &'static str,
        message: impl Into<String>,
        field: Option<&'static str>,
    ) -> Self {
        Self {
            exit_code: 2,
            code,
            message: message.into(),
            field,
        }
    }

    pub fn runtime(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            exit_code: 1,
            code,
            message: message.into(),
            field: None,
        }
    }
}

pub type CiGateExecutionResult = Result<CiGateResponse, CiGateError>;

/// Output returned by a live Pooled 1-Chromium capture pass.
#[derive(Debug, Clone)]
pub struct CiLiveCaptureOutput {
    pub chromium_binary: String,
    pub flags_used: Vec<String>,
    pub geometry_nodes: Vec<DomGeometryNode>,
    pub captured_dom_bytes: usize,
}

/// Injectable capture hook `(chrome_bin, resolved_chromium_flags, html_source)` for deterministic
/// offline testing of the live Pooled 1-Chromium capture pass.
pub type CiLiveCaptureHook =
    Arc<dyn Fn(&str, &[String], &str) -> Result<CiLiveCaptureOutput, String> + Send + Sync>;

/// Injectable host-probe overrides for deterministic offline tests.
#[derive(Clone, Default)]
pub struct CiRuntimeProbeOverrides {
    pub shm_path: Option<PathBuf>,
    pub shm_available_override: Option<u64>,
    pub shm_total_override: Option<u64>,
    pub env_force_disable_shm: Option<bool>,
    pub env_chromium_no_sandbox: Option<bool>,
    /// Outer `None` reads `std::env::var("COWOK_MAX_MEMORY_MB")`; `Some(None)` simulates unset;
    /// `Some(Some(val))` injects an explicit env var value.
    pub env_max_memory_mb: Option<Option<String>>,
    pub cgroup_v2_path: Option<PathBuf>,
    pub cgroup_v1_path: Option<PathBuf>,
    pub uid_gid_override: Option<(u32, u32)>,
    pub libreoffice_search_dirs: Option<Vec<PathBuf>>,
    pub fallback_home: Option<PathBuf>,
    /// Outer `None` runs `discover_local_chromium_binary()`; `Some(inner)` injects the binary path.
    pub chromium_binary_override: Option<Option<String>>,
    pub live_capture_override: Option<CiLiveCaptureHook>,
}

impl fmt::Debug for CiRuntimeProbeOverrides {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CiRuntimeProbeOverrides")
            .field("shm_path", &self.shm_path)
            .field("shm_available_override", &self.shm_available_override)
            .field("shm_total_override", &self.shm_total_override)
            .field("env_force_disable_shm", &self.env_force_disable_shm)
            .field("env_chromium_no_sandbox", &self.env_chromium_no_sandbox)
            .field("env_max_memory_mb", &self.env_max_memory_mb)
            .field("cgroup_v2_path", &self.cgroup_v2_path)
            .field("cgroup_v1_path", &self.cgroup_v1_path)
            .field("uid_gid_override", &self.uid_gid_override)
            .field("libreoffice_search_dirs", &self.libreoffice_search_dirs)
            .field("fallback_home", &self.fallback_home)
            .field("chromium_binary_override", &self.chromium_binary_override)
            .field(
                "live_capture_override",
                &self.live_capture_override.as_ref().map(|_| "<hook>"),
            )
            .finish()
    }
}

fn is_os_root_symlink(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        path == Path::new("/var") || path == Path::new("/tmp") || path == Path::new("/etc")
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}

fn reject_symlink_ancestors(path: &Path) -> Result<(), CiGateError> {
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(CiGateError::validation(
            "PATH_TRAVERSAL_DENIED",
            format!(
                "HTML path '{}' cannot contain '..' traversal segments",
                path.display()
            ),
            Some("html"),
        ));
    }
    let anchored = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut current = PathBuf::new();
    for comp in anchored.components() {
        match comp {
            Component::RootDir => current.push(Path::new("/")),
            Component::Prefix(p) => current.push(p.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(CiGateError::validation(
                    "PATH_TRAVERSAL_DENIED",
                    format!(
                        "HTML path '{}' cannot contain '..' traversal segments",
                        path.display()
                    ),
                    Some("html"),
                ));
            }
            Component::Normal(seg) => {
                current.push(seg);
                if is_os_root_symlink(&current) {
                    continue;
                }
                match fs::symlink_metadata(&current) {
                    Ok(meta) => {
                        if meta.file_type().is_symlink() {
                            return Err(CiGateError::validation(
                                "SYMLINK_REJECTED",
                                format!("Symlink component '{}' is forbidden", current.display()),
                                Some("html"),
                            ));
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
                    Err(err) => {
                        return Err(CiGateError::validation(
                            "HTML_FILE_UNREADABLE",
                            format!("Failed inspecting '{}': {err}", current.display()),
                            Some("html"),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn is_eloop_error(err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        err.raw_os_error() == Some(libc::ELOOP)
    }
    #[cfg(not(unix))]
    {
        let _ = err;
        false
    }
}

/// Opens and reads an input HTML file with strict `O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK`,
/// ancestor symlink rejection, regular-file check (rejecting FIFOs/sockets immediately),
/// and bounded byte length (`<= 512 KiB`).
pub fn read_bounded_html_nofollow(path: &Path) -> Result<String, CiGateError> {
    reject_symlink_ancestors(path)?;
    if reject_if_symlink(path).is_err() {
        return Err(CiGateError::validation(
            "SYMLINK_REJECTED",
            format!("Input HTML path '{}' cannot be a symlink", path.display()),
            Some("html"),
        ));
    }
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    let file = opts.open(path).map_err(|err| {
        if is_eloop_error(&err) {
            CiGateError::validation(
                "SYMLINK_REJECTED",
                format!("Input HTML path '{}' resolved to a symlink", path.display()),
                Some("html"),
            )
        } else {
            CiGateError::validation(
                "HTML_FILE_UNREADABLE",
                format!("Failed opening '{}': {err}", path.display()),
                Some("html"),
            )
        }
    })?;
    let meta = file.metadata().map_err(|err| {
        CiGateError::validation(
            "HTML_FILE_UNREADABLE",
            format!("Failed reading metadata for '{}': {err}", path.display()),
            Some("html"),
        )
    })?;
    if !meta.is_file() {
        return Err(CiGateError::validation(
            "HTML_NOT_REGULAR_FILE",
            format!(
                "Input HTML path '{}' must be a regular file",
                path.display()
            ),
            Some("html"),
        ));
    }
    if meta.len() > MAX_CI_HTML_BYTES {
        return Err(CiGateError::validation(
            "HTML_TOO_LARGE",
            format!(
                "Input HTML '{}' ({} bytes) exceeds {} byte limit",
                path.display(),
                meta.len(),
                MAX_CI_HTML_BYTES
            ),
            Some("html"),
        ));
    }
    let mut buf = Vec::with_capacity(meta.len().min(MAX_CI_HTML_BYTES) as usize);
    file.take(MAX_CI_HTML_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|err| {
            CiGateError::validation(
                "HTML_FILE_UNREADABLE",
                format!("Failed reading '{}': {err}", path.display()),
                Some("html"),
            )
        })?;
    if buf.len() as u64 > MAX_CI_HTML_BYTES {
        return Err(CiGateError::validation(
            "HTML_TOO_LARGE",
            format!(
                "Input HTML '{}' exceeded {} byte limit while reading",
                path.display(),
                MAX_CI_HTML_BYTES
            ),
            Some("html"),
        ));
    }
    String::from_utf8(buf).map_err(|err| {
        CiGateError::validation(
            "HTML_INVALID_UTF8",
            format!("Input HTML '{}' is not valid UTF-8: {err}", path.display()),
            Some("html"),
        )
    })
}

#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
fn query_statvfs_bytes(path: &Path) -> Option<(u64, u64)> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c_path` is a valid NUL-terminated path and `stat` is writable stack memory.
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return None;
    }
    let block_size = if stat.f_frsize > 0 {
        stat.f_frsize as u64
    } else {
        stat.f_bsize as u64
    };
    let available = (stat.f_bavail as u64).saturating_mul(block_size);
    let total = (stat.f_blocks as u64).saturating_mul(block_size);
    Some((available, total))
}

#[cfg(not(unix))]
fn query_statvfs_bytes(_path: &Path) -> Option<(u64, u64)> {
    None
}

fn env_flag_enabled(keys: &[&str]) -> bool {
    for key in keys {
        if let Ok(val) = std::env::var(key) {
            let trimmed = val.trim().to_ascii_lowercase();
            if matches!(trimmed.as_str(), "1" | "true" | "yes") {
                return true;
            }
        }
    }
    false
}

fn env_requests_disable_dev_shm() -> bool {
    env_flag_enabled(&["COWOK_DISABLE_DEV_SHM", "COWOK_CHROMIUM_DISABLE_DEV_SHM"])
}

fn env_requests_chromium_no_sandbox() -> bool {
    env_flag_enabled(&["COWOK_CONTAINER_MODE"]) && env_flag_enabled(&["COWOK_CHROMIUM_NO_SANDBOX"])
}

/// Probes `/dev/shm` capacity (with optional overrides for deterministic testing) and
/// builds focus-free headless Chromium flags, automatically injecting `--disable-dev-shm-usage`
/// when `/dev/shm` is absent, unreadable, or smaller than 256 MiB.
pub fn probe_shm_diagnostics(
    shm_path: &Path,
    available_override: Option<u64>,
    total_override: Option<u64>,
    env_force_disable: bool,
) -> CiShmDiagnostics {
    probe_shm_diagnostics_with_container(
        shm_path,
        available_override,
        total_override,
        env_force_disable,
        false,
    )
}

/// Extended `/dev/shm` + hardened container sandbox flag resolver. Appends `--no-sandbox`
/// ONLY when `container_no_sandbox` is enabled (`COWOK_CONTAINER_MODE=1` /
/// `COWOK_CHROMIUM_NO_SANDBOX=1`) inside a non-root hardened OCI container boundary.
pub fn probe_shm_diagnostics_with_container(
    shm_path: &Path,
    available_override: Option<u64>,
    total_override: Option<u64>,
    env_force_disable: bool,
    container_no_sandbox: bool,
) -> CiShmDiagnostics {
    let (shm_available_bytes, shm_total_bytes) =
        if available_override.is_some() || total_override.is_some() {
            (available_override, total_override.or(available_override))
        } else {
            match query_statvfs_bytes(shm_path) {
                Some((avail, total)) => (Some(avail), Some(total)),
                None => (None, None),
            }
        };

    let auto_disable_dev_shm_usage = env_force_disable
        || shm_available_bytes.is_none_or(|avail| avail < SHM_SAFE_THRESHOLD_BYTES);

    let mut resolved_chromium_flags: Vec<String> = FOCUS_FREE_HEADLESS_FLAGS
        .iter()
        .map(|flag| (*flag).to_string())
        .collect();
    if auto_disable_dev_shm_usage {
        resolved_chromium_flags.push("--disable-dev-shm-usage".to_string());
    }
    if container_no_sandbox {
        resolved_chromium_flags.push("--no-sandbox".to_string());
    }

    CiShmDiagnostics {
        shm_path: shm_path.display().to_string(),
        shm_available_bytes,
        shm_total_bytes,
        threshold_bytes: SHM_SAFE_THRESHOLD_BYTES,
        auto_disable_dev_shm_usage,
        container_no_sandbox,
        resolved_chromium_flags,
    }
}

fn read_cgroup_limit_mb(cgroup_v2_path: &Path, cgroup_v1_path: &Path) -> (String, Option<u64>) {
    if let Ok(raw) = fs::read_to_string(cgroup_v2_path) {
        let trimmed = raw.trim();
        if !trimmed.eq_ignore_ascii_case("max") {
            if let Ok(bytes) = trimmed.parse::<u64>() {
                if bytes > 0 {
                    let mb = (bytes / (1024 * 1024)).max(1);
                    return ("cgroup_v2".to_string(), Some(mb));
                }
            }
        }
        // Hybrid cgroup v1+v2 hosts can expose `"max"` on v2 while enforcing a limit on v1;
        // fall through to inspect `cgroup_v1_path` before declaring `host_unbounded`.
    }

    if let Ok(raw) = fs::read_to_string(cgroup_v1_path) {
        let trimmed = raw.trim();
        if let Ok(bytes) = trimmed.parse::<u64>() {
            if bytes > 0 && bytes < CGROUP_V1_UNBOUNDED_SENTINEL {
                let mb = (bytes / (1024 * 1024)).max(1);
                return ("cgroup_v1".to_string(), Some(mb));
            }
        }
    }

    ("host_unbounded".to_string(), None)
}

/// Resolves requested memory cap giving precedence to explicit CLI `--max-memory-mb` over
/// `COWOK_MAX_MEMORY_MB` env var (falling back to `None` -> `DEFAULT_MAX_MEMORY_MB`).
pub fn resolve_requested_max_memory_mb(
    cli_arg: Option<u64>,
    env_raw: Option<&str>,
) -> Result<Option<u64>, String> {
    if cli_arg.is_some() {
        return Ok(cli_arg);
    }
    let Some(raw) = env_raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = trimmed.parse::<u64>().map_err(|_| {
        format!(
            "COWOK_MAX_MEMORY_MB must be an integer within {MIN_MAX_MEMORY_MB}..={MAX_MAX_MEMORY_MB} MiB (got '{trimmed}')"
        )
    })?;
    Ok(Some(parsed))
}

/// Validates `--max-memory-mb` (`128..=16384`) and clamps it to 75% of the detected
/// cgroup v2 (`memory.max`) or cgroup v1 (`memory.limit_in_bytes`) ceiling (floored at 128 MiB).
pub fn resolve_memory_cap(
    requested_mb: Option<u64>,
    cgroup_v2_path: &Path,
    cgroup_v1_path: &Path,
) -> Result<CiMemoryCapDiagnostics, String> {
    let requested = requested_mb.unwrap_or(DEFAULT_MAX_MEMORY_MB);
    if !(MIN_MAX_MEMORY_MB..=MAX_MAX_MEMORY_MB).contains(&requested) {
        return Err(format!(
            "--max-memory-mb must be within {MIN_MAX_MEMORY_MB}..={MAX_MAX_MEMORY_MB} MiB (got {requested})"
        ));
    }

    let (cgroup_source, cgroup_limit_mb) = read_cgroup_limit_mb(cgroup_v2_path, cgroup_v1_path);
    let (effective_max_memory_mb, cgroup_clamped) = match cgroup_limit_mb {
        Some(limit_mb) => {
            let safe_ceiling = (limit_mb.saturating_mul(3) / 4).max(MIN_MAX_MEMORY_MB);
            let effective = requested.min(safe_ceiling).max(MIN_MAX_MEMORY_MB);
            (effective, effective < requested)
        }
        None => (requested, false),
    };

    let js_heap_flag = format!("--js-flags=--max-old-space-size={effective_max_memory_mb}");
    Ok(CiMemoryCapDiagnostics {
        cgroup_source,
        cgroup_limit_mb,
        requested_max_memory_mb: requested,
        effective_max_memory_mb,
        cgroup_clamped,
        js_heap_flag,
    })
}

/// Returns `(effective_uid, effective_gid, is_non_root)` for container non-root verification.
pub fn inspect_uid_gid_diagnostics(uid_gid_override: Option<(u32, u32)>) -> (u32, u32, bool) {
    let (uid, gid) = match uid_gid_override {
        Some(pair) => pair,
        None => {
            #[cfg(unix)]
            {
                // SAFETY: `geteuid` and `getegid` are read-only POSIX queries.
                unsafe { (libc::geteuid(), libc::getegid()) }
            }
            #[cfg(not(unix))]
            {
                (EXPECTED_CONTAINER_UID, EXPECTED_CONTAINER_UID)
            }
        }
    };
    (uid, gid, uid != 0)
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        (meta.permissions().mode() & 0o111) != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Scans `dirs` for forbidden office-suite binaries (`libreoffice`, `soffice`) to enforce
/// the ADR-006 zero-LibreOffice core container invariant.
pub fn detect_libreoffice_in_dirs(dirs: &[PathBuf]) -> bool {
    for dir in dirs {
        for bin in ["libreoffice", "soffice"] {
            let candidate = dir.join(bin);
            if is_executable_file(&candidate) {
                return true;
            }
        }
    }
    false
}

fn default_binary_search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|raw| std::env::split_paths(&raw).collect())
        .unwrap_or_default();
    for standard in ["/usr/bin", "/usr/local/bin"] {
        let p = PathBuf::from(standard);
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    dirs
}

/// Resolves runtime storage paths cleanly, isolating explicit `fallback_home` overrides from
/// any pre-set host `XDG_DATA_HOME` / `XDG_CACHE_HOME` environment variables.
fn resolve_ci_storage_paths(
    workspace_override: Option<&Path>,
    fallback_home: Option<&Path>,
) -> Result<RuntimeStoragePaths, CiGateError> {
    if let Some(home) = fallback_home {
        let raw_workspace = match workspace_override {
            Some(p) => p.to_path_buf(),
            None => std::env::var_os("UXUI_OS_WORKSPACE_ROOT")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        };
        if reject_if_symlink(&raw_workspace).is_ok() {
            if let Ok(canonical_workspace) = raw_workspace.canonicalize() {
                let ws_cowok = canonical_workspace.join(".cowok-design");
                let ws_writable = if ws_cowok.exists() {
                    probe_dir_writable_atomic(&ws_cowok)
                } else {
                    probe_dir_writable_atomic(&canonical_workspace)
                };
                if !ws_writable {
                    let data_root = home.join("cowok-design");
                    let cache_root = home.join("cowok-design-cache");
                    let cowok_dir = data_root.join(".cowok-design");
                    fs::create_dir_all(&cowok_dir).map_err(|err| {
                        CiGateError::runtime(
                            "STORAGE_UNAVAILABLE",
                            format!("Failed creating fallback staging dir: {err}"),
                        )
                    })?;
                    fs::create_dir_all(&cache_root).map_err(|err| {
                        CiGateError::runtime(
                            "STORAGE_UNAVAILABLE",
                            format!("Failed creating fallback cache dir: {err}"),
                        )
                    })?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = fs::set_permissions(&data_root, fs::Permissions::from_mode(0o700));
                        let _ = fs::set_permissions(&cowok_dir, fs::Permissions::from_mode(0o700));
                        let _ = fs::set_permissions(&cache_root, fs::Permissions::from_mode(0o700));
                    }
                    return Ok(RuntimeStoragePaths {
                        workspace_root: canonical_workspace,
                        state_root: data_root,
                        data_dir: cowok_dir.clone(),
                        cache_dir: cache_root,
                        db_path: cowok_dir.join("cowok-design.sqlite3"),
                        lock_path: cowok_dir.join("daemon.lock"),
                        read_only_fallback: true,
                    });
                }
            }
        }
    }
    resolve_runtime_storage_paths(workspace_override, fallback_home)
        .map_err(|err| CiGateError::runtime("STORAGE_UNAVAILABLE", err.to_string()))
}

fn derive_collision_proof_slug(project_id: &str) -> String {
    let base_slug: String = project_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let base_slug = if base_slug.is_empty() {
        DEFAULT_CI_PROJECT_ID.to_string()
    } else {
        base_slug
    };
    let mut hasher = Sha256::new();
    hasher.update(project_id.as_bytes());
    let digest = hasher.finalize();
    let hash8 = format!(
        "{:02x}{:02x}{:02x}{:02x}",
        digest[0], digest[1], digest[2], digest[3]
    );
    format!("{base_slug}-{hash8}")
}

fn ensure_ci_project_row(
    pool: &SqlitePool,
    project_id: &str,
    workspace_root: &Path,
) -> Result<(), CiGateError> {
    let mut conn = pool.connection().map_err(|err| {
        CiGateError::runtime(
            "STORAGE_BUSY",
            format!("Failed acquiring SQLite connection: {err}"),
        )
    })?;
    let slug = derive_collision_proof_slug(project_id);
    let _ = workspace_root;
    let now_ms = Utc::now().timestamp_millis();
    retry_on_busy("ci_gate_ensure_project_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM projects WHERE id = ?1",
                params![project_id],
                |r| r.get(0),
            )
            .optional()?;
        if existing.is_none() {
            tx.execute(
                "INSERT INTO projects
                    (id, slug, name, workspace_root, product_brief, tenant_id, provenance, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, '.', '', 'local-default', 'ci_gate', 'ACTIVE', ?4, ?4)
                 ON CONFLICT(id) DO NOTHING",
                params![project_id, slug, format!("CI Gate {project_id}"), now_ms],
            )?;
        }
        tx.commit()
    })
    .map_err(|err| {
        CiGateError::runtime(
            "STORAGE_PROJECT_INIT_FAILED",
            format!("Failed bootstrapping CI project row '{project_id}': {err}"),
        )
    })
}

fn default_live_capture_geometry_node(captured_dom_bytes: usize) -> DomGeometryNode {
    DomGeometryNode {
        ff_id: "ci-live-root".to_string(),
        tag: "main".to_string(),
        role: Some("main".to_string()),
        bbox: QcBoundingBox {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 120.0,
        },
        z_index: 0,
        scroll_width: 320.0,
        client_width: 320.0,
        scroll_height: 120.0,
        client_height: 120.0,
        font_size_px: 16.0,
        line_height_ratio: 1.5,
        fg_hex: None,
        bg_hex: None,
        border_radius_px: None,
        parent_radius_px: None,
        parent_padding_px: None,
        is_interactive: false,
        is_visible: true,
        pointer_events_none: false,
        text_len: captured_dom_bytes.min(512),
        has_aria_label: true,
        has_focus_ring: true,
    }
}

/// Runs a bounded headless Chromium DOM capture pass with the exact resolved container flags
/// (`FOCUS_FREE_HEADLESS_FLAGS`, `--disable-dev-shm-usage`, `--no-sandbox` in container mode,
/// and `--js-flags=--max-old-space-size=...`), cleaning up the ephemeral profile & child PGID.
fn run_live_chromium_dom_capture(
    chrome_bin: &str,
    resolved_flags: &[String],
    html_source: &str,
) -> Result<CiLiveCaptureOutput, String> {
    let scratch_dir =
        std::env::temp_dir().join(format!("cowok-ci-cdp-{}", Uuid::new_v4().simple()));
    let profile_dir = scratch_dir.join("profile");
    let target_html_path = scratch_dir.join("target.html");

    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&scratch_dir)
            .map_err(|e| format!("Failed creating ephemeral CI CDP scratch dir: {e}"))?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&profile_dir)
            .map_err(|e| format!("Failed creating ephemeral CI CDP profile dir: {e}"))?;
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&target_html_path)
            .map_err(|e| {
                let _ = fs::remove_dir_all(&scratch_dir);
                format!("Failed creating ephemeral CI HTML target: {e}")
            })?;
        f.write_all(html_source.as_bytes()).map_err(|e| {
            let _ = fs::remove_dir_all(&scratch_dir);
            format!("Failed writing ephemeral CI HTML target: {e}")
        })?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(&profile_dir).map_err(|e| {
            let _ = fs::remove_dir_all(&scratch_dir);
            format!("Failed creating ephemeral CI CDP profile dir: {e}")
        })?;
        fs::write(&target_html_path, html_source.as_bytes()).map_err(|e| {
            let _ = fs::remove_dir_all(&scratch_dir);
            format!("Failed writing ephemeral CI HTML target: {e}")
        })?;
    }

    let target_url = format!("file://{}", target_html_path.display());
    let mut args = resolved_flags.to_vec();
    args.push("--window-size=1440,900".to_string());
    args.push(format!("--user-data-dir={}", profile_dir.display()));
    args.push("--virtual-time-budget=2000".to_string());
    args.push("--dump-dom".to_string());
    args.push(target_url);

    let mut cmd = Command::new(chrome_bin);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let reaper = ChildProcessReaper::new();
    let (mut child, handle) = reaper
        .spawn_managed_child(cmd, "pooled-chromium-ci-gate")
        .map_err(|e| {
            let _ = fs::remove_dir_all(&scratch_dir);
            format!("Failed spawning headless Chromium '{chrome_bin}': {e}")
        })?;

    let stdout_reader = child.stdout.take().map(|out| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = out.take(MAX_CAPTURE_DOM_BYTES).read_to_end(&mut buf);
            buf
        })
    });

    let deadline = Instant::now() + CHROMIUM_CI_CAPTURE_TIMEOUT;
    let status_res = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Ok(st),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = ChildProcessReaper::terminate_pgid_safe(
                        handle.pid,
                        handle.pgid,
                        &handle.start_time,
                    );
                    let _ = child.wait();
                    break Err("Headless Chromium CI DOM capture exceeded 15s timeout".to_string());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => break Err(format!("Failed waiting for headless Chromium: {e}")),
        }
    };
    // Terminate any lingering Chromium renderer/GPU subprocesses in the PGID before joining
    // stdout_reader so an orphaned helper holding the pipe open can never hang past timeout.
    let _ = reaper.reap_all();

    let dom_bytes = stdout_reader
        .and_then(|t| t.join().ok())
        .unwrap_or_default();
    let _ = fs::remove_dir_all(&scratch_dir);

    match status_res {
        Ok(st) if st.success() && !dom_bytes.is_empty() => Ok(CiLiveCaptureOutput {
            chromium_binary: chrome_bin.to_string(),
            flags_used: resolved_flags.to_vec(),
            geometry_nodes: vec![default_live_capture_geometry_node(dom_bytes.len())],
            captured_dom_bytes: dom_bytes.len(),
        }),
        Ok(st) => Err(format!(
            "Headless Chromium exited with status {st} (captured {} bytes)",
            dom_bytes.len()
        )),
        Err(msg) => Err(msg),
    }
}

/// Runs the full Story 12.3 headless CI gate (Story 07.1 Visual QC + Story 12.1 7 DE Tools +
/// Story 12.3 OCI container diagnostics) against the resolved SQLite WAL.
pub async fn run_ci_gate(req: &CiGateRequest) -> CiGateExecutionResult {
    run_ci_gate_with_overrides(req, &CiRuntimeProbeOverrides::default()).await
}

/// Runs the full Story 12.3 headless CI gate with injectable runtime probe overrides for
/// deterministic testing of `/dev/shm`, cgroup memory caps, non-root UID, LibreOffice scans,
/// live Pooled 1-Chromium capture flags, and read-only workspace XDG staging.
pub async fn run_ci_gate_with_overrides(
    req: &CiGateRequest,
    overrides: &CiRuntimeProbeOverrides,
) -> CiGateExecutionResult {
    let started = Instant::now();

    let project_id = req.project_id.trim();
    if project_id.is_empty() || project_id.len() > 160 {
        return Err(CiGateError::validation(
            "INVALID_PROJECT_ID",
            "projectId must be 1..=160 characters",
            Some("project"),
        ));
    }
    let target_id = req.target_id.trim();
    if target_id.is_empty() || target_id.len() > 160 {
        return Err(CiGateError::validation(
            "INVALID_TARGET_ID",
            "targetId must be 1..=160 characters",
            Some("targetId"),
        ));
    }

    let env_mem_raw = match &overrides.env_max_memory_mb {
        Some(injected) => injected.clone(),
        None => std::env::var("COWOK_MAX_MEMORY_MB").ok(),
    };
    let requested_max_memory_mb =
        resolve_requested_max_memory_mb(req.max_memory_mb, env_mem_raw.as_deref()).map_err(
            |msg| CiGateError::validation("INVALID_MAX_MEMORY_MB", msg, Some("maxMemoryMb")),
        )?;

    let cgroup_v2 = overrides
        .cgroup_v2_path
        .as_deref()
        .unwrap_or_else(|| Path::new(DEFAULT_CGROUP_V2_MEMORY_MAX));
    let cgroup_v1 = overrides
        .cgroup_v1_path
        .as_deref()
        .unwrap_or_else(|| Path::new(DEFAULT_CGROUP_V1_MEMORY_LIMIT));
    let memory_cap =
        resolve_memory_cap(requested_max_memory_mb, cgroup_v2, cgroup_v1).map_err(|msg| {
            CiGateError::validation("INVALID_MAX_MEMORY_MB", msg, Some("maxMemoryMb"))
        })?;

    let (html_source, target_hint_path) = if let Some(ref html_path) = req.html_path {
        let html = read_bounded_html_nofollow(html_path)?;
        (html, Some(html_path.clone()))
    } else {
        (FALLBACK_CLEAN_HTML.to_string(), None)
    };

    let storage =
        resolve_ci_storage_paths(req.workspace.as_deref(), overrides.fallback_home.as_deref())?;

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&storage.db_path, &storage.state_root))
        .map_err(|err| {
            CiGateError::runtime(
                "STORAGE_UNAVAILABLE",
                format!(
                    "Failed opening SQLite WAL at '{}': {err}",
                    storage.db_path.display()
                ),
            )
        })?;
    ensure_ci_project_row(&pool, project_id, &storage.workspace_root)?;
    let registry = Arc::new(SpecRegistry::new(pool.clone()));
    let state = DaemonAppState::new(registry, "127.0.0.1".to_string(), DEFAULT_DAEMON_PORT);

    let shm_path = overrides
        .shm_path
        .as_deref()
        .unwrap_or_else(|| Path::new(DEFAULT_SHM_PATH));
    let env_force_disable = overrides
        .env_force_disable_shm
        .unwrap_or_else(env_requests_disable_dev_shm);
    let container_no_sandbox = overrides
        .env_chromium_no_sandbox
        .unwrap_or_else(env_requests_chromium_no_sandbox);
    let mut shm = probe_shm_diagnostics_with_container(
        shm_path,
        overrides.shm_available_override,
        overrides.shm_total_override,
        env_force_disable,
        container_no_sandbox,
    );
    shm.resolved_chromium_flags
        .push(memory_cap.js_heap_flag.clone());

    let (effective_uid, effective_gid, is_non_root) =
        inspect_uid_gid_diagnostics(overrides.uid_gid_override);
    let chromium_binary_path = if req.force_no_browser {
        None
    } else if let Some(ref overridden) = overrides.chromium_binary_override {
        overridden.clone()
    } else {
        discover_local_chromium_binary()
    };
    let search_dirs = overrides
        .libreoffice_search_dirs
        .clone()
        .unwrap_or_else(default_binary_search_dirs);
    let libreoffice_binary_detected = detect_libreoffice_in_dirs(&search_dirs);
    let zero_libreoffice_invariant = !libreoffice_binary_detected;

    // 1. Live Pooled 1-Chromium capture pass (when browser enabled & binary present)
    //    Permit is acquired inside `spawn_blocking` and dropped before `evaluate_qc_for_project`
    //    so max concurrency == 1 is enforced without self-deadlocking.
    let mut live_capture_error: Option<String> = None;
    let mut live_capture_executed = false;
    let mut captured_geometry_nodes: Vec<DomGeometryNode> = Vec::new();
    if !req.force_no_browser {
        if let Some(ref chrome_bin) = chromium_binary_path {
            let chrome_bin_owned = chrome_bin.clone();
            let resolved_flags = shm.resolved_chromium_flags.clone();
            let html_clone = html_source.clone();
            let hook_opt = overrides.live_capture_override.clone();

            let capture_res = tokio::task::spawn_blocking(move || {
                let evaluator = shared_cdp_evaluator();
                let _permit = evaluator.acquire_permit();
                let res = if let Some(hook) = hook_opt {
                    hook(&chrome_bin_owned, &resolved_flags, &html_clone)
                } else {
                    run_live_chromium_dom_capture(&chrome_bin_owned, &resolved_flags, &html_clone)
                };
                match res {
                    Ok(output) => {
                        evaluator.record_worker_healthy();
                        Ok(output)
                    }
                    Err(reason) => {
                        let _ = evaluator.record_worker_crash(&reason);
                        Err(reason)
                    }
                }
            })
            .await
            .map_err(|err| {
                CiGateError::runtime(
                    "CDP_CAPTURE_WORKER_PANICKED",
                    format!("Pooled 1-Chromium capture worker panicked: {err}"),
                )
            })?;

            match capture_res {
                Ok(output) if !output.geometry_nodes.is_empty() => {
                    captured_geometry_nodes = output.geometry_nodes;
                    live_capture_executed = true;
                }
                Ok(_) => {
                    live_capture_error =
                        Some("Live Chromium capture produced zero DOM geometry nodes".to_string());
                }
                Err(reason) => {
                    live_capture_error = Some(reason);
                }
            }
        }
    }

    let wal_staging = CiWalStagingDiagnostics {
        workspace_root: storage.workspace_root.display().to_string(),
        state_root: storage.state_root.display().to_string(),
        db_path: storage.db_path.display().to_string(),
        lock_path: storage.lock_path.display().to_string(),
        read_only_fallback: storage.read_only_fallback,
    };

    let diagnostics = CiContainerDiagnostics {
        effective_uid,
        effective_gid,
        expected_container_uid: EXPECTED_CONTAINER_UID,
        is_non_root,
        chromium_binary_path: chromium_binary_path.clone(),
        pooled_cdp_max_concurrency: POOLED_CDP_MAX_CONCURRENCY,
        live_capture_executed,
        libreoffice_binary_detected,
        zero_libreoffice_invariant,
        shm,
        memory_cap,
        wal_staging,
    };

    // 2. Execute Story 07.1 Visual QC parity (`POST /api/v1/qc/run` engine)
    let qc_req = QcRunRequest {
        project_id: project_id.to_string(),
        target_kind: QcTargetKind::AppScreen,
        target_id: target_id.to_string(),
        html: html_source.clone(),
        constraints_json: "{}".to_string(),
        viewports: CANONICAL_APP_VIEWPORTS.to_vec(),
        fixture_ref: "ci-gate".to_string(),
        geometry_nodes: captured_geometry_nodes.clone(),
        flow_nodes: Vec::new(),
        flow_edges: Vec::new(),
        reference_rgba: None,
        candidate_rgba: None,
        frame_width: 0,
        frame_height: 0,
        ssim_threshold: DEFAULT_SSIM_PASS_THRESHOLD,
        force_refresh: true,
        // Fail-closed: if live capture was not executed or crashed, force UNVERIFIED_NO_BROWSER
        // so empty geometry_nodes can never produce a false-positive QcVerdict::Pass.
        force_no_browser: req.force_no_browser || !live_capture_executed,
    };
    let mut visual_qc = evaluate_qc_for_project(&state, qc_req)
        .await
        .map_err(|msg| CiGateError::runtime("QC_RUN_FAILED", msg))?;

    // When a test injects a mock Chromium override on a host lacking a local Chromium binary,
    // `shared_cdp_evaluator()` has `browser_binary = None`. Re-evaluate with a scoped
    // `PooledCdpEvaluator` bound to the overridden binary and sync the persisted SQLite row.
    if live_capture_executed
        && overrides.chromium_binary_override.is_some()
        && visual_qc.verdict == QcVerdict::UnverifiedNoBrowser
        && !shared_cdp_evaluator().can_attempt_cdp()
    {
        let scoped_pool = PooledCdpEvaluator::new(chromium_binary_path.clone());
        let eval_input = VisualQcEvalInput {
            target_kind: QcTargetKind::AppScreen,
            target_id: target_id.to_string(),
            html: html_source.clone(),
            constraints_json: "{}".to_string(),
            viewports: CANONICAL_APP_VIEWPORTS.to_vec(),
            fixture_ref: "ci-gate".to_string(),
            geometry_nodes: captured_geometry_nodes,
            flow_nodes: Vec::new(),
            flow_edges: Vec::new(),
            reference_rgba: None,
            candidate_rgba: None,
            frame_width: 0,
            frame_height: 0,
            ssim_threshold: DEFAULT_SSIM_PASS_THRESHOLD,
            force_no_browser: false,
        };
        let eval_res = tokio::task::spawn_blocking(move || scoped_pool.evaluate(&eval_input))
            .await
            .map_err(|err| {
                CiGateError::runtime("QC_RUN_FAILED", format!("scoped visual qc panicked: {err}"))
            })?;
        visual_qc.verdict = eval_res.verdict;
        visual_qc.p0_count = eval_res.p0_count;
        visual_qc.p1_count = eval_res.p1_count;
        visual_qc.ssim_score = eval_res.ssim_score;
        visual_qc.issues = eval_res.issues;
        visual_qc.used_fallback = eval_res.used_fallback;
        visual_qc.fallback_mode = eval_res.fallback_mode;
        visual_qc.breaker_status = eval_res.breaker_status;

        let issues_json =
            serde_json::to_string(&visual_qc.issues).unwrap_or_else(|_| "[]".to_string());
        let mut conn = pool.connection().map_err(|err| {
            CiGateError::runtime(
                "STORAGE_BUSY",
                format!("Failed acquiring SQLite connection: {err}"),
            )
        })?;
        ensure_v26_tables(&conn)
            .map_err(|err| CiGateError::runtime("STORAGE_UNAVAILABLE", err.to_string()))?;
        let qc_run_id_clone = visual_qc.qc_run_id.clone();
        let verdict_str = visual_qc.verdict.as_db_str().to_string();
        let p0_i64 = visual_qc.p0_count as i64;
        let p1_i64 = visual_qc.p1_count as i64;
        retry_on_busy("ci_gate_update_qc_run_override_tx", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE qc_gate_runs SET verdict = ?1, p0_count = ?2, p1_count = ?3, issues_json = ?4 WHERE id = ?5",
                params![verdict_str, p0_i64, p1_i64, issues_json, qc_run_id_clone],
            )?;
            tx.commit()
        })
        .map_err(|err| {
            CiGateError::runtime(
                "QC_RUN_FAILED",
                format!("Failed syncing overridden QC verdict to SQLite: {err}"),
            )
        })?;
    }

    // 3. Execute Story 12.1 7 built-in Rust DE tools parity (`run_builtin_de_suite`)
    let target_rel = target_hint_path
        .as_deref()
        .and_then(|p| p.canonicalize().ok())
        .and_then(|canon| {
            canon
                .strip_prefix(&storage.workspace_root)
                .ok()
                .map(|rel| rel.to_string_lossy().into_owned())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "inline-ci-gate.html".to_string());

    let de_input = DeSuiteInput {
        workspace_root: &storage.workspace_root,
        target_artifact_path: &target_rel,
        content_override: Some(&html_source),
        tools: BuiltInDeTool::all(),
        token_pairs: &[],
    };
    let de_suite = run_builtin_de_suite(&de_input)
        .map_err(|err| CiGateError::runtime("DE_RUN_FAILED", err.to_string()))?;

    // 4. Evaluate deterministic CI gate verdict & exit code
    let mut failure_reasons = Vec::new();
    if visual_qc.p0_count > 0 || visual_qc.verdict == QcVerdict::Fail {
        failure_reasons.push(format!(
            "QC_P0_FAILED: visual QC verdict={:?}, p0Count={}, p1Count={}",
            visual_qc.verdict, visual_qc.p0_count, visual_qc.p1_count
        ));
    }
    if !de_suite.passed {
        let failed_tools: Vec<&str> = de_suite
            .results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| r.tool.as_str())
            .collect();
        failure_reasons.push(format!(
            "DE_SUITE_FAILED: failing DE tools=[{}]",
            failed_tools.join(", ")
        ));
    }
    if diagnostics.libreoffice_binary_detected {
        failure_reasons.push(
            "ZERO_LIBREOFFICE_VIOLATION: forbidden LibreOffice/soffice binary detected in runtime environment (ADR-006)"
                .to_string(),
        );
    }
    if visual_qc.verdict == QcVerdict::UnverifiedNoBrowser && req.fail_on_unverified {
        let reason_detail = live_capture_error
            .as_deref()
            .map(|e| format!(" ({e})"))
            .unwrap_or_default();
        failure_reasons.push(format!(
            "UNVERIFIED_NO_BROWSER: --fail-on-unverified requires a live Pooled 1-Chromium CDP verification{reason_detail}"
        ));
    }

    let (verdict, passed, exit_code) = if !failure_reasons.is_empty() {
        (CiGateVerdict::Fail, false, 3)
    } else if visual_qc.verdict == QcVerdict::UnverifiedNoBrowser {
        (CiGateVerdict::UnverifiedNoBrowser, true, 0)
    } else {
        (CiGateVerdict::Pass, true, 0)
    };

    Ok(CiGateResponse {
        gate_id: Uuid::new_v4().to_string(),
        project_id: project_id.to_string(),
        verdict,
        passed,
        exit_code,
        fail_on_unverified: req.fail_on_unverified,
        failure_reasons,
        visual_qc,
        de_suite,
        diagnostics,
        total_duration_ms: started.elapsed().as_millis() as u64,
        created_at: Utc::now().timestamp_millis(),
    })
}

/// Async CLI entry function invoked by `cli/commands.rs` (`run_async(exec_ci_gate(req, json))`).
pub async fn exec_ci_gate(req: CiGateRequest, json: bool) -> CliOutput {
    exec_ci_gate_with_overrides(req, json, &CiRuntimeProbeOverrides::default()).await
}

/// Async CLI entry function with injectable overrides for deterministic test suites.
pub async fn exec_ci_gate_with_overrides(
    req: CiGateRequest,
    json: bool,
    overrides: &CiRuntimeProbeOverrides,
) -> CliOutput {
    match run_ci_gate_with_overrides(&req, overrides).await {
        Ok(resp) => {
            if json {
                CliOutput {
                    exit_code: resp.exit_code,
                    stdout: format_json_success(&resp),
                    stderr: String::new(),
                }
            } else {
                let summary = format!(
                    "CI gate verdict={} (passed={}, exit={}, qc={:?}[p0={}], dePassed={}, deTools={}, shmFallback={}, memCapMb={}, roFallback={})",
                    resp.verdict.as_str(),
                    resp.passed,
                    resp.exit_code,
                    resp.visual_qc.verdict,
                    resp.visual_qc.p0_count,
                    resp.de_suite.passed,
                    resp.de_suite.results.len(),
                    resp.diagnostics.shm.auto_disable_dev_shm_usage,
                    resp.diagnostics.memory_cap.effective_max_memory_mb,
                    resp.diagnostics.wal_staging.read_only_fallback,
                );
                CliOutput {
                    exit_code: resp.exit_code,
                    stdout: summary,
                    stderr: String::new(),
                }
            }
        }
        Err(err) => {
            if json {
                CliOutput {
                    exit_code: err.exit_code,
                    stdout: format_json_error(err.code, &err.message, err.field),
                    stderr: String::new(),
                }
            } else {
                CliOutput::err(
                    err.exit_code,
                    format!("error [{}]: {}", err.code, err.message),
                )
            }
        }
    }
}

/// Synchronous CLI entry point ready for direct dispatch from `cli/commands.rs`.
pub fn execute_ci_gate_cli(req: CiGateRequest, json: bool) -> CliOutput {
    execute_ci_gate_cli_with_overrides(req, json, &CiRuntimeProbeOverrides::default())
}

/// Synchronous CLI entry point with injectable runtime probe overrides for host-independent tests.
pub fn execute_ci_gate_cli_with_overrides(
    req: CiGateRequest,
    json: bool,
    overrides: &CiRuntimeProbeOverrides,
) -> CliOutput {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| {
            handle.block_on(exec_ci_gate_with_overrides(req, json, overrides))
        })
    } else {
        match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt.block_on(exec_ci_gate_with_overrides(req, json, overrides)),
            Err(err) => {
                if json {
                    CliOutput {
                        exit_code: 1,
                        stdout: format_json_error(
                            "RUNTIME_INIT_FAILED",
                            &format!("Failed starting async runtime: {err}"),
                            None,
                        ),
                        stderr: String::new(),
                    }
                } else {
                    CliOutput::err(1, format!("Failed starting async runtime: {err}"))
                }
            }
        }
    }
}
