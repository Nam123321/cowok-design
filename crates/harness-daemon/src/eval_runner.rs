// @story 13.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Out-of-Band Pinned SHA-256 + chmod 0444 Sealed Eval Runner, Single-Flight Lock, V33 MPSC Persistence & Atomic JSONL Ledger)
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use spec_registry::pool::V33_MIGRATION_SQL;
use spec_registry::SqlitePool;
use uuid::Uuid;

use crate::eval_runner_suites::{
    canonical_sealed_fixture_files, compute_eval_verdict, evaluate_decision_calibration,
    evaluate_fault_injections, evaluate_mandatory_security_sim_cases, evaluate_paired_ablation,
    evaluate_sim_user_25_cases, sha256_hex, summarize_milestones, DecisionCalibrationSummary,
    EvalBenchmarkRunDto, EvalTier, EvalVerdict, EvalVerdictInput, FaultInjectionSummary,
    ManifestPinSource, MilestoneSummary, PairedAblationSummary, SealedBriefsFixture,
    SealedCalibrationFixture, SealedFaultInjectionsFixture, SealedSimCasesFixture, SimCaseResult,
    SimEvalContext, MANDATORY_SECURITY_GATE_CASE_IDS,
};
use crate::routes::constant_time_eq;
use crate::routes::orchestrator::shared_write_actor_for_pool;

/// Out-of-band compile-time pinned SHA-256 of canonical `tests/eval/sealed/manifest.json`.
/// Overridable strictly via CI protected env `COWOK_SEALED_MANIFEST_SHA256` (exact 64 lowercase hex).
pub const PINNED_SEALED_MANIFEST_SHA256: &str =
    "0f61c8fb50d0b8f3ceab15dc9b7237ad02225af5320578a2f37b5f94ce00c7a3";

pub const SEALED_MANIFEST_ENV_VAR: &str = "COWOK_SEALED_MANIFEST_SHA256";
pub const SEALED_EVAL_REL_DIR: &str = "tests/eval/sealed";
pub const GATE_HISTORY_REL_PATH: &str = ".cowok-design/evals/gate-history.jsonl";
pub const EVAL_RUN_LOCK_FILENAME: &str = "eval-run.lock";
pub const MAX_SEALED_FILE_BYTES: u64 = 4 * 1024 * 1024;

static EVAL_RUN_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// RAII single-flight guard enforcing `409 EVAL_ALREADY_RUNNING` across both
/// in-process threads (`EVAL_RUN_IN_FLIGHT`) and concurrent OS processes (`flock(LOCK_EX | LOCK_NB)`).
pub struct EvalSingleFlightGuard {
    #[cfg(unix)]
    lock_file: Option<fs::File>,
    #[cfg(not(unix))]
    _private: (),
}

impl Drop for EvalSingleFlightGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(ref f) = self.lock_file {
            use std::os::unix::io::AsRawFd;
            unsafe {
                libc::flock(f.as_raw_fd(), libc::LOCK_UN);
            }
        }
        EVAL_RUN_IN_FLIGHT.store(false, Ordering::Release);
    }
}

pub fn acquire_eval_single_flight() -> Result<EvalSingleFlightGuard, EvalRunnerError> {
    if EVAL_RUN_IN_FLIGHT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(EvalRunnerError::AlreadyRunning);
    }
    Ok(EvalSingleFlightGuard {
        #[cfg(unix)]
        lock_file: None,
        #[cfg(not(unix))]
        _private: (),
    })
}

/// Acquires both the in-process atomic latch AND a cross-process non-blocking
/// `flock(LOCK_EX | LOCK_NB)` on `<evals_dir>/eval-run.lock` (`O_NOFOLLOW | O_CLOEXEC`, `0o600`).
pub fn acquire_eval_single_flight_with_lock_dir(
    evals_dir: &Path,
) -> Result<EvalSingleFlightGuard, EvalRunnerError> {
    let mut guard = acquire_eval_single_flight()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        use std::os::unix::io::AsRawFd;

        ensure_secure_evals_dir(evals_dir)?;
        let lock_path = evals_dir.join(EVAL_RUN_LOCK_FILENAME);
        if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(EvalRunnerError::Storage(
                "Symlink rejected on eval-run.lock".to_string(),
            ));
        }

        let mut opts = OpenOptions::new();
        opts.read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
        let file = opts
            .open(&lock_path)
            .map_err(|e| EvalRunnerError::Storage(format!("eval-run.lock open failed: {e}")))?;
        let fmeta = file
            .metadata()
            .map_err(|e| EvalRunnerError::Storage(format!("eval-run.lock fstat failed: {e}")))?;
        if !fmeta.file_type().is_file() {
            return Err(EvalRunnerError::Storage(
                "eval-run.lock must be a regular file".to_string(),
            ));
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| EvalRunnerError::Storage(format!("eval-run.lock fchmod failed: {e}")))?;

        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == ErrorKind::WouldBlock
                || err.raw_os_error() == Some(libc::EWOULDBLOCK)
                || err.raw_os_error() == Some(libc::EAGAIN)
            {
                return Err(EvalRunnerError::AlreadyRunning);
            }
            return Err(EvalRunnerError::Storage(format!(
                "eval-run.lock flock failed: {err}"
            )));
        }
        guard.lock_file = Some(file);
    }
    #[cfg(not(unix))]
    {
        let _ = evals_dir;
    }

    Ok(guard)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SealedManifestFile {
    pub schema_version: String,
    pub suite: String,
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedIntegritySnapshot {
    pub sealed_integrity_ok: bool,
    pub sealed_manifest_sha256: String,
    pub manifest_pin_source: ManifestPinSource,
    pub tamper_reason: Option<String>,
    pub sealed_files_count: usize,
}

#[derive(Debug, Clone)]
pub struct VerifiedSealedEvalBundle {
    pub snapshot: SealedIntegritySnapshot,
    pub expected_filenames: Vec<String>,
    pub briefs: SealedBriefsFixture,
    pub calibration: SealedCalibrationFixture,
    pub faults: SealedFaultInjectionsFixture,
    pub sim_cases: SealedSimCasesFixture,
}

#[derive(Debug, Clone)]
pub struct SealedIntegrityError {
    pub reason: String,
    pub detail: String,
    pub observed_sha256: String,
    pub pin_source: ManifestPinSource,
}

#[derive(Debug)]
pub enum EvalRunnerError {
    AlreadyRunning,
    InvalidInput {
        field: &'static str,
        message: String,
    },
    TamperDetected {
        run: Box<EvalBenchmarkRunDto>,
        reason: String,
        detail: String,
    },
    Storage(String),
}

#[derive(Debug, Clone)]
pub struct EvalRunOptions {
    pub tier: EvalTier,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub design_craft_candidate_dir: Option<PathBuf>,
    pub run_simulated_users: bool,
    pub triggered_by: String,
    pub seal_if_clean: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalHistoryData {
    pub sealed_integrity_ok: bool,
    pub sealed_manifest_sha256: String,
    pub manifest_pin_source: ManifestPinSource,
    pub tamper_reason: Option<String>,
    pub latest_run: Option<EvalBenchmarkRunDto>,
    pub runs: Vec<EvalBenchmarkRunDto>,
    pub total_runs: usize,
}

pub fn is_valid_sha256_hex(raw: &str) -> bool {
    raw.len() == 64 && raw.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn resolve_pinned_manifest_hash() -> Result<(String, ManifestPinSource), SealedIntegrityError> {
    match std::env::var(SEALED_MANIFEST_ENV_VAR) {
        Ok(env_val) => {
            let trimmed = env_val.trim();
            if !is_valid_sha256_hex(trimmed) {
                return Err(SealedIntegrityError {
                    reason: "MALFORMED_PINNED_MANIFEST_SHA256".to_string(),
                    detail: format!(
                        "{SEALED_MANIFEST_ENV_VAR} must be an exact 64-char lowercase hex SHA-256 digest"
                    ),
                    observed_sha256: String::new(),
                    pin_source: ManifestPinSource::CiEnvOverride,
                });
            }
            Ok((trimmed.to_string(), ManifestPinSource::CiEnvOverride))
        }
        Err(_) => Ok((
            PINNED_SEALED_MANIFEST_SHA256.to_string(),
            ManifestPinSource::CompiledBinary,
        )),
    }
}

fn resolve_eval_workspace_root(workspace_root: &Path) -> PathBuf {
    let direct = workspace_root.join(SEALED_EVAL_REL_DIR);
    if direct.exists() {
        return workspace_root.to_path_buf();
    }
    if workspace_root
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name == ".cowok-design")
    {
        if let Some(parent) = workspace_root.parent() {
            return parent.to_path_buf();
        }
    }
    workspace_root.to_path_buf()
}

fn reject_symlink_in_sealed_ancestry(
    base: &Path,
    rel_segments: &[&str],
    pin_source: ManifestPinSource,
) -> Result<PathBuf, SealedIntegrityError> {
    let mut cur = base.to_path_buf();
    for seg in rel_segments {
        cur.push(seg);
        match fs::symlink_metadata(&cur) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(SealedIntegrityError {
                        reason: "SYMLINK_DETECTED".to_string(),
                        detail: format!("Symlink rejected in sealed eval path '{}'", cur.display()),
                        observed_sha256: String::new(),
                        pin_source,
                    });
                }
                if !meta.file_type().is_dir() {
                    return Err(SealedIntegrityError {
                        reason: "SEALED_DIR_NOT_DIRECTORY".to_string(),
                        detail: format!("Expected directory at '{}'", cur.display()),
                        observed_sha256: String::new(),
                        pin_source,
                    });
                }
            }
            Err(_) => {
                return Err(SealedIntegrityError {
                    reason: "SEALED_DIR_MISSING".to_string(),
                    detail: format!("Missing sealed eval directory '{}'", cur.display()),
                    observed_sha256: String::new(),
                    pin_source,
                });
            }
        }
    }
    Ok(cur)
}

fn read_sealed_file_nofollow(
    path: &Path,
    require_readonly: bool,
    pin_source: ManifestPinSource,
) -> Result<Vec<u8>, SealedIntegrityError> {
    let meta = fs::symlink_metadata(path).map_err(|e| SealedIntegrityError {
        reason: "MISSING_SEALED_FIXTURE".to_string(),
        detail: format!("Unable to stat sealed file '{}': {e}", path.display()),
        observed_sha256: String::new(),
        pin_source,
    })?;
    if meta.file_type().is_symlink() {
        return Err(SealedIntegrityError {
            reason: "SYMLINK_DETECTED".to_string(),
            detail: format!("Symlink rejected at '{}'", path.display()),
            observed_sha256: String::new(),
            pin_source,
        });
    }
    if !meta.file_type().is_file() {
        return Err(SealedIntegrityError {
            reason: "UNEXPECTED_ENTRY_IN_SEALED_DIR".to_string(),
            detail: format!("Non-regular file entry '{}' in sealed dir", path.display()),
            observed_sha256: String::new(),
            pin_source,
        });
    }
    if meta.len() > MAX_SEALED_FILE_BYTES {
        return Err(SealedIntegrityError {
            reason: "SEALED_FILE_TOO_LARGE".to_string(),
            detail: format!(
                "Sealed file '{}' size {} exceeds {} byte cap",
                path.display(),
                meta.len(),
                MAX_SEALED_FILE_BYTES
            ),
            observed_sha256: String::new(),
            pin_source,
        });
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
        if meta.nlink() != 1 {
            return Err(SealedIntegrityError {
                reason: "HARDLINK_DETECTED".to_string(),
                detail: format!(
                    "Hardlinked sealed file rejected at '{}' (nlink={})",
                    path.display(),
                    meta.nlink()
                ),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        if require_readonly && (meta.permissions().mode() & 0o222) != 0 {
            return Err(SealedIntegrityError {
                reason: "WRITABLE_SEALED_FILE".to_string(),
                detail: format!(
                    "Sealed file '{}' has writable permissions {:o} (expected 0444)",
                    path.display(),
                    meta.permissions().mode() & 0o777
                ),
                observed_sha256: String::new(),
                pin_source,
            });
        }

        let mut opts = OpenOptions::new();
        opts.read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
        let file = opts.open(path).map_err(|e| {
            let reason = if e.raw_os_error() == Some(libc::ELOOP) {
                "SYMLINK_DETECTED"
            } else {
                "SEALED_FILE_READ_FAILED"
            };
            SealedIntegrityError {
                reason: reason.to_string(),
                detail: format!("Failed opening sealed file '{}': {e}", path.display()),
                observed_sha256: String::new(),
                pin_source,
            }
        })?;
        let fmeta = file.metadata().map_err(|e| SealedIntegrityError {
            reason: "SEALED_FILE_READ_FAILED".to_string(),
            detail: format!("Failed fstat on '{}': {e}", path.display()),
            observed_sha256: String::new(),
            pin_source,
        })?;
        if !fmeta.file_type().is_file() {
            return Err(SealedIntegrityError {
                reason: "UNEXPECTED_ENTRY_IN_SEALED_DIR".to_string(),
                detail: format!(
                    "Non-regular file detected post-open at '{}'",
                    path.display()
                ),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        if fmeta.len() > MAX_SEALED_FILE_BYTES {
            return Err(SealedIntegrityError {
                reason: "SEALED_FILE_TOO_LARGE".to_string(),
                detail: format!(
                    "Sealed file '{}' exceeds {} byte cap post-open",
                    path.display(),
                    MAX_SEALED_FILE_BYTES
                ),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        if fmeta.nlink() != 1 {
            return Err(SealedIntegrityError {
                reason: "HARDLINK_DETECTED".to_string(),
                detail: format!("Hardlinked sealed file rejected at '{}'", path.display()),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        if require_readonly && (fmeta.permissions().mode() & 0o222) != 0 {
            return Err(SealedIntegrityError {
                reason: "WRITABLE_SEALED_FILE".to_string(),
                detail: format!("Sealed file '{}' became writable", path.display()),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        let mut buf = Vec::new();
        file.take(MAX_SEALED_FILE_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|e| SealedIntegrityError {
                reason: "SEALED_FILE_READ_FAILED".to_string(),
                detail: format!("Failed reading sealed file '{}': {e}", path.display()),
                observed_sha256: String::new(),
                pin_source,
            })?;
        if buf.len() as u64 > MAX_SEALED_FILE_BYTES {
            return Err(SealedIntegrityError {
                reason: "SEALED_FILE_TOO_LARGE".to_string(),
                detail: format!(
                    "Sealed file '{}' exceeded {} byte read cap",
                    path.display(),
                    MAX_SEALED_FILE_BYTES
                ),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        Ok(buf)
    }
    #[cfg(not(unix))]
    {
        let _ = require_readonly;
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|e| SealedIntegrityError {
                reason: "SEALED_FILE_READ_FAILED".to_string(),
                detail: format!("Failed opening sealed file '{}': {e}", path.display()),
                observed_sha256: String::new(),
                pin_source,
            })?;
        let fmeta = file.metadata().map_err(|e| SealedIntegrityError {
            reason: "SEALED_FILE_READ_FAILED".to_string(),
            detail: format!("Failed stat on '{}': {e}", path.display()),
            observed_sha256: String::new(),
            pin_source,
        })?;
        if !fmeta.file_type().is_file() {
            return Err(SealedIntegrityError {
                reason: "UNEXPECTED_ENTRY_IN_SEALED_DIR".to_string(),
                detail: format!("Non-regular file entry '{}' in sealed dir", path.display()),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        if fmeta.len() > MAX_SEALED_FILE_BYTES {
            return Err(SealedIntegrityError {
                reason: "SEALED_FILE_TOO_LARGE".to_string(),
                detail: format!(
                    "Sealed file '{}' exceeds {} byte cap",
                    path.display(),
                    MAX_SEALED_FILE_BYTES
                ),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        let mut buf = Vec::new();
        file.take(MAX_SEALED_FILE_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|e| SealedIntegrityError {
                reason: "SEALED_FILE_READ_FAILED".to_string(),
                detail: format!("Failed reading sealed file '{}': {e}", path.display()),
                observed_sha256: String::new(),
                pin_source,
            })?;
        if buf.len() as u64 > MAX_SEALED_FILE_BYTES {
            return Err(SealedIntegrityError {
                reason: "SEALED_FILE_TOO_LARGE".to_string(),
                detail: format!(
                    "Sealed file '{}' exceeded {} byte read cap",
                    path.display(),
                    MAX_SEALED_FILE_BYTES
                ),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        Ok(buf)
    }
}

fn verify_sealed_eval_inner(
    workspace_root: &Path,
    require_readonly: bool,
) -> Result<VerifiedSealedEvalBundle, SealedIntegrityError> {
    let (pinned_sha, pin_source) = resolve_pinned_manifest_hash()?;
    let root = resolve_eval_workspace_root(workspace_root);
    let sealed_dir =
        reject_symlink_in_sealed_ancestry(&root, &["tests", "eval", "sealed"], pin_source)?;

    let mut present_entries = BTreeSet::new();
    let dir_iter = fs::read_dir(&sealed_dir).map_err(|e| SealedIntegrityError {
        reason: "SEALED_DIR_READ_FAILED".to_string(),
        detail: format!("Failed reading '{}': {e}", sealed_dir.display()),
        observed_sha256: String::new(),
        pin_source,
    })?;
    for entry_res in dir_iter {
        let entry = entry_res.map_err(|e| SealedIntegrityError {
            reason: "SEALED_DIR_READ_FAILED".to_string(),
            detail: format!("Directory entry error: {e}"),
            observed_sha256: String::new(),
            pin_source,
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let entry_path = entry.path();
        let meta = fs::symlink_metadata(&entry_path).map_err(|e| SealedIntegrityError {
            reason: "SEALED_DIR_READ_FAILED".to_string(),
            detail: format!("Failed stat '{}': {e}", entry_path.display()),
            observed_sha256: String::new(),
            pin_source,
        })?;
        if meta.file_type().is_symlink() {
            return Err(SealedIntegrityError {
                reason: "SYMLINK_DETECTED".to_string(),
                detail: format!("Symlink rejected in sealed dir: '{}'", entry_path.display()),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        if !meta.file_type().is_file() {
            return Err(SealedIntegrityError {
                reason: "UNEXPECTED_ENTRY_IN_SEALED_DIR".to_string(),
                detail: format!("Unexpected non-file '{}' in sealed dir", name),
                observed_sha256: String::new(),
                pin_source,
            });
        }
        present_entries.insert(name);
    }

    let manifest_path = sealed_dir.join("manifest.json");
    let manifest_bytes = read_sealed_file_nofollow(&manifest_path, require_readonly, pin_source)?;
    let observed_manifest_sha = sha256_hex(&manifest_bytes);
    if !constant_time_eq(observed_manifest_sha.as_bytes(), pinned_sha.as_bytes()) {
        return Err(SealedIntegrityError {
            reason: "ROOT_MANIFEST_SHA256_MISMATCH".to_string(),
            detail: format!(
                "manifest.json SHA-256 mismatch (observed={observed_manifest_sha}, pinned={pinned_sha})"
            ),
            observed_sha256: observed_manifest_sha,
            pin_source,
        });
    }

    let manifest: SealedManifestFile =
        serde_json::from_slice(&manifest_bytes).map_err(|e| SealedIntegrityError {
            reason: "MANIFEST_PARSE_ERROR".to_string(),
            detail: format!("Invalid manifest.json: {e}"),
            observed_sha256: observed_manifest_sha.clone(),
            pin_source,
        })?;
    if manifest.schema_version.trim().is_empty()
        || manifest.suite.trim().is_empty()
        || manifest.files.is_empty()
    {
        return Err(SealedIntegrityError {
            reason: "MANIFEST_PARSE_ERROR".to_string(),
            detail: "manifest.json missing required fields".to_string(),
            observed_sha256: observed_manifest_sha,
            pin_source,
        });
    }

    let mut expected_names = BTreeSet::new();
    expected_names.insert("manifest.json".to_string());
    for file_name in manifest.files.keys() {
        if file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") {
            return Err(SealedIntegrityError {
                reason: "MANIFEST_PATH_TRAVERSAL".to_string(),
                detail: format!("Invalid file name '{file_name}' in manifest.json"),
                observed_sha256: observed_manifest_sha,
                pin_source,
            });
        }
        expected_names.insert(file_name.clone());
    }

    for actual in &present_entries {
        if !expected_names.contains(actual) {
            return Err(SealedIntegrityError {
                reason: "UNEXPECTED_FILE_IN_SEALED_DIR".to_string(),
                detail: format!("Unlisted file '{actual}' detected in tests/eval/sealed/"),
                observed_sha256: observed_manifest_sha,
                pin_source,
            });
        }
    }

    let mut raw_files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (file_name, expected_sha) in &manifest.files {
        let fpath = sealed_dir.join(file_name);
        let bytes = read_sealed_file_nofollow(&fpath, require_readonly, pin_source)?;
        let actual_sha = sha256_hex(&bytes);
        if !constant_time_eq(actual_sha.as_bytes(), expected_sha.as_bytes()) {
            return Err(SealedIntegrityError {
                reason: "FIXTURE_SHA256_MISMATCH".to_string(),
                detail: format!(
                    "Sealed fixture '{file_name}' SHA-256 mismatch (observed={actual_sha}, expected={expected_sha})"
                ),
                observed_sha256: observed_manifest_sha,
                pin_source,
            });
        }
        raw_files.insert(file_name.clone(), bytes);
    }

    let briefs: SealedBriefsFixture = parse_fixture(
        &raw_files,
        "briefs.json",
        &observed_manifest_sha,
        pin_source,
    )?;
    let calibration: SealedCalibrationFixture = parse_fixture(
        &raw_files,
        "decision-calibration.json",
        &observed_manifest_sha,
        pin_source,
    )?;
    let faults: SealedFaultInjectionsFixture = parse_fixture(
        &raw_files,
        "fault-injections.json",
        &observed_manifest_sha,
        pin_source,
    )?;
    let sim_cases: SealedSimCasesFixture = parse_fixture(
        &raw_files,
        "sim-user-25-cases.json",
        &observed_manifest_sha,
        pin_source,
    )?;

    Ok(VerifiedSealedEvalBundle {
        snapshot: SealedIntegritySnapshot {
            sealed_integrity_ok: true,
            sealed_manifest_sha256: observed_manifest_sha,
            manifest_pin_source: pin_source,
            tamper_reason: None,
            sealed_files_count: expected_names.len(),
        },
        expected_filenames: expected_names.into_iter().collect(),
        briefs,
        calibration,
        faults,
        sim_cases,
    })
}

fn parse_fixture<T: for<'de> Deserialize<'de>>(
    raw_files: &BTreeMap<String, Vec<u8>>,
    name: &str,
    manifest_sha: &str,
    pin_source: ManifestPinSource,
) -> Result<T, SealedIntegrityError> {
    let bytes = raw_files.get(name).ok_or_else(|| SealedIntegrityError {
        reason: "MISSING_SEALED_FIXTURE".to_string(),
        detail: format!("manifest.json missing required fixture '{name}'"),
        observed_sha256: manifest_sha.to_string(),
        pin_source,
    })?;
    serde_json::from_slice(bytes).map_err(|e| SealedIntegrityError {
        reason: "FIXTURE_PARSE_ERROR".to_string(),
        detail: format!("Failed parsing sealed fixture '{name}': {e}"),
        observed_sha256: manifest_sha.to_string(),
        pin_source,
    })
}

/// Strict fail-closed sealed eval integrity verifier (`AC-01`, `AC-05`).
/// Always requires `0444` read-only permissions, zero symlinks, `nlink == 1`,
/// out-of-band pinned root manifest SHA-256, and exhaustive fixture hashes.
pub fn verify_sealed_eval_integrity(
    workspace_root: &Path,
) -> Result<VerifiedSealedEvalBundle, SealedIntegrityError> {
    verify_sealed_eval_inner(workspace_root, true)
}

/// Non-mutating integrity snapshot for `GET /api/v1/eval/history`.
pub fn inspect_sealed_integrity(workspace_root: &Path) -> SealedIntegritySnapshot {
    match verify_sealed_eval_integrity(workspace_root) {
        Ok(bundle) => bundle.snapshot,
        Err(err) => SealedIntegritySnapshot {
            sealed_integrity_ok: false,
            sealed_manifest_sha256: err.observed_sha256,
            manifest_pin_source: err.pin_source,
            tamper_reason: Some(err.reason),
            sealed_files_count: 0,
        },
    }
}

/// Clean checkout sealer (`cowok-design eval seal` / `--seal-if-clean`).
/// Verifies out-of-band root SHA-256 + every fixture hash + zero symlinks BEFORE
/// applying FD-based `fchmod 0444` to each verified file in `tests/eval/sealed/`.
pub fn seal_sealed_eval_dir(
    workspace_root: &Path,
) -> Result<SealedIntegritySnapshot, SealedIntegrityError> {
    let verified = verify_sealed_eval_inner(workspace_root, false)?;
    let root = resolve_eval_workspace_root(workspace_root);
    let pin_source = verified.snapshot.manifest_pin_source;
    let manifest_sha = verified.snapshot.sealed_manifest_sha256.clone();
    let sealed_dir =
        reject_symlink_in_sealed_ancestry(&root, &["tests", "eval", "sealed"], pin_source)?;

    let expected_set: BTreeSet<&str> = verified
        .expected_filenames
        .iter()
        .map(String::as_str)
        .collect();

    let dir_iter = fs::read_dir(&sealed_dir).map_err(|e| SealedIntegrityError {
        reason: "SEAL_CHMOD_FAILED".to_string(),
        detail: format!("Failed reading sealed dir for chmod: {e}"),
        observed_sha256: manifest_sha.clone(),
        pin_source,
    })?;
    for entry_res in dir_iter {
        let entry = entry_res.map_err(|e| SealedIntegrityError {
            reason: "SEAL_CHMOD_FAILED".to_string(),
            detail: format!("Entry error during seal: {e}"),
            observed_sha256: manifest_sha.clone(),
            pin_source,
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !expected_set.contains(name.as_str()) {
            return Err(SealedIntegrityError {
                reason: "UNEXPECTED_FILE_IN_SEALED_DIR".to_string(),
                detail: format!("Unexpected entry '{name}' appeared during seal"),
                observed_sha256: manifest_sha.clone(),
                pin_source,
            });
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
        for file_name in &verified.expected_filenames {
            let path = sealed_dir.join(file_name);
            let mut opts = OpenOptions::new();
            opts.read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
            let file = opts.open(&path).map_err(|e| {
                let reason = if e.raw_os_error() == Some(libc::ELOOP) {
                    "SYMLINK_DETECTED"
                } else {
                    "SEAL_CHMOD_FAILED"
                };
                SealedIntegrityError {
                    reason: reason.to_string(),
                    detail: format!("Failed opening '{}' for fchmod: {e}", path.display()),
                    observed_sha256: manifest_sha.clone(),
                    pin_source,
                }
            })?;
            let fmeta = file.metadata().map_err(|e| SealedIntegrityError {
                reason: "SEAL_CHMOD_FAILED".to_string(),
                detail: format!("fstat error during seal '{}': {e}", path.display()),
                observed_sha256: manifest_sha.clone(),
                pin_source,
            })?;
            if !fmeta.file_type().is_file() {
                return Err(SealedIntegrityError {
                    reason: "UNEXPECTED_ENTRY_IN_SEALED_DIR".to_string(),
                    detail: format!("Non-regular file rejected during seal '{}'", path.display()),
                    observed_sha256: manifest_sha.clone(),
                    pin_source,
                });
            }
            if fmeta.nlink() != 1 {
                return Err(SealedIntegrityError {
                    reason: "HARDLINK_DETECTED".to_string(),
                    detail: format!("Hardlinked file rejected during seal '{}'", path.display()),
                    observed_sha256: manifest_sha.clone(),
                    pin_source,
                });
            }
            file.set_permissions(fs::Permissions::from_mode(0o444))
                .map_err(|e| SealedIntegrityError {
                    reason: "SEAL_CHMOD_FAILED".to_string(),
                    detail: format!("Failed fchmod 0444 '{}': {e}", path.display()),
                    observed_sha256: manifest_sha.clone(),
                    pin_source,
                })?;
        }
    }

    let post_verified = verify_sealed_eval_integrity(workspace_root)?;
    Ok(post_verified.snapshot)
}

/// Safe operator helper (`cowok-design eval init-sealed`) that materializes the 5 canonical
/// sealed fixture files into an empty `<workspace>/tests/eval/sealed/` directory, verifies
/// against `PINNED_SEALED_MANIFEST_SHA256`, and seals every file to `chmod 0444`.
/// Refuses to overwrite existing files in `tests/eval/sealed/`.
pub fn init_sealed_eval_dir(
    workspace_root: &Path,
) -> Result<SealedIntegritySnapshot, SealedIntegrityError> {
    let (pinned_sha, pin_source) = resolve_pinned_manifest_hash()?;
    let canonical_files = canonical_sealed_fixture_files();
    let canonical_manifest_sha = canonical_files
        .get("manifest.json")
        .map(|s| sha256_hex(s.as_bytes()))
        .unwrap_or_default();
    if !constant_time_eq(canonical_manifest_sha.as_bytes(), pinned_sha.as_bytes()) {
        return Err(SealedIntegrityError {
            reason: "ROOT_MANIFEST_SHA256_MISMATCH".to_string(),
            detail: format!(
                "Canonical manifest SHA-256 ({canonical_manifest_sha}) does not match pinned digest ({pinned_sha})"
            ),
            observed_sha256: canonical_manifest_sha,
            pin_source,
        });
    }

    let root = resolve_eval_workspace_root(workspace_root);
    let mut cur = root.clone();
    for seg in &["tests", "eval", "sealed"] {
        cur.push(seg);
        if let Ok(meta) = fs::symlink_metadata(&cur) {
            if meta.file_type().is_symlink() {
                return Err(SealedIntegrityError {
                    reason: "SYMLINK_DETECTED".to_string(),
                    detail: format!("Symlink rejected at '{}'", cur.display()),
                    observed_sha256: String::new(),
                    pin_source,
                });
            }
            if !meta.file_type().is_dir() {
                return Err(SealedIntegrityError {
                    reason: "SEALED_DIR_NOT_DIRECTORY".to_string(),
                    detail: format!("Expected directory at '{}'", cur.display()),
                    observed_sha256: String::new(),
                    pin_source,
                });
            }
        }
    }

    fs::create_dir_all(&cur).map_err(|e| SealedIntegrityError {
        reason: "SEALED_DIR_CREATE_FAILED".to_string(),
        detail: format!("Failed creating '{}': {e}", cur.display()),
        observed_sha256: String::new(),
        pin_source,
    })?;
    let sealed_dir =
        reject_symlink_in_sealed_ancestry(&root, &["tests", "eval", "sealed"], pin_source)?;

    let mut existing_iter = fs::read_dir(&sealed_dir).map_err(|e| SealedIntegrityError {
        reason: "SEALED_DIR_READ_FAILED".to_string(),
        detail: format!("Failed inspecting '{}': {e}", sealed_dir.display()),
        observed_sha256: String::new(),
        pin_source,
    })?;
    if existing_iter.next().is_some() {
        return Err(SealedIntegrityError {
            reason: "SEALED_DIR_NOT_EMPTY".to_string(),
            detail: format!(
                "'{}' already contains files; refusing to overwrite existing sealed fixtures",
                sealed_dir.display()
            ),
            observed_sha256: String::new(),
            pin_source,
        });
    }

    for (name, content) in &canonical_files {
        let target = sealed_dir.join(name);
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o644)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut f = opts.open(&target).map_err(|e| SealedIntegrityError {
            reason: "SEALED_INIT_WRITE_FAILED".to_string(),
            detail: format!("Failed creating '{}': {e}", target.display()),
            observed_sha256: String::new(),
            pin_source,
        })?;
        f.write_all(content.as_bytes())
            .and_then(|_| f.sync_data())
            .map_err(|e| SealedIntegrityError {
                reason: "SEALED_INIT_WRITE_FAILED".to_string(),
                detail: format!("Failed writing '{}': {e}", target.display()),
                observed_sha256: String::new(),
                pin_source,
            })?;
    }

    seal_sealed_eval_dir(&root)
}

/// Validates optional `designCraftCandidateDir` strictly inside `workspace_root` (rejecting `..` & symlinks).
pub fn validate_candidate_dir_in_workspace(
    workspace_root: &Path,
    raw_dir: &str,
) -> Result<PathBuf, EvalRunnerError> {
    let trimmed = raw_dir.trim();
    if trimmed.is_empty() || trimmed.len() > 512 || trimmed.contains('\0') {
        return Err(EvalRunnerError::InvalidInput {
            field: "designCraftCandidateDir",
            message: "designCraftCandidateDir must be a non-empty path (<=512 chars)".to_string(),
        });
    }
    let path = Path::new(trimmed);
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(EvalRunnerError::InvalidInput {
            field: "designCraftCandidateDir",
            message: "designCraftCandidateDir cannot contain '..' traversal segments".to_string(),
        });
    }

    let resolved_root = resolve_eval_workspace_root(workspace_root);
    let canon_root = resolved_root
        .canonicalize()
        .unwrap_or_else(|_| resolved_root.clone());
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        canon_root.join(path)
    };

    if fs::symlink_metadata(&candidate).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(EvalRunnerError::InvalidInput {
            field: "designCraftCandidateDir",
            message: "designCraftCandidateDir cannot be a symlink".to_string(),
        });
    }
    let canon_candidate = candidate
        .canonicalize()
        .map_err(|_| EvalRunnerError::InvalidInput {
            field: "designCraftCandidateDir",
            message: format!(
                "designCraftCandidateDir '{}' does not exist inside workspace",
                trimmed
            ),
        })?;
    if !canon_candidate.starts_with(&canon_root) {
        return Err(EvalRunnerError::InvalidInput {
            field: "designCraftCandidateDir",
            message: "designCraftCandidateDir must stay strictly inside workspace root".to_string(),
        });
    }
    if !canon_candidate.is_dir() {
        return Err(EvalRunnerError::InvalidInput {
            field: "designCraftCandidateDir",
            message: "designCraftCandidateDir must be a directory inside workspace".to_string(),
        });
    }
    Ok(canon_candidate)
}

fn resolve_writable_evals_dir(pool: &SqlitePool, eval_root: &Path) -> PathBuf {
    if let Some(parent) = pool
        .db_path()
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        if parent.file_name() == Some(OsStr::new(".cowok-design")) {
            return parent.join("evals");
        }
        return parent.join(".cowok-design").join("evals");
    }
    eval_root.join(".cowok-design").join("evals")
}

/// Executes a full single-flight sealed evaluation run, persisting atomically to V33 SQLite (via MPSC actor)
/// and `.cowok-design/evals/gate-history.jsonl`.
pub fn execute_eval_run(
    pool: &SqlitePool,
    workspace_root: &Path,
    opts: EvalRunOptions,
) -> Result<EvalBenchmarkRunDto, EvalRunnerError> {
    if !opts.run_simulated_users && opts.tier != EvalTier::Pr {
        return Err(EvalRunnerError::InvalidInput {
            field: "runSimulatedUsers",
            message: "runSimulatedUsers=false is only permitted on tier 'pr'; stage and release require full 25-case simulation".to_string(),
        });
    }

    let eval_root = resolve_eval_workspace_root(workspace_root);
    let evals_dir = resolve_writable_evals_dir(pool, &eval_root);
    let _single_flight = acquire_eval_single_flight_with_lock_dir(&evals_dir)?;
    let started = Instant::now();
    let run_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().timestamp_millis();
    let offline_enforced = opts.tier == EvalTier::Pr;
    let ledger_path_buf = evals_dir.join("gate-history.jsonl");
    let ledger_path = GATE_HISTORY_REL_PATH.to_string();

    let seal_on_boot =
        opts.seal_if_clean || std::env::var("COWOK_EVAL_SEAL_ON_BOOT").as_deref() == Ok("1");
    if seal_on_boot {
        if let Err(err) = seal_sealed_eval_dir(&eval_root) {
            let tamper_run = build_tamper_run_dto(
                run_id,
                &opts,
                offline_enforced,
                &err,
                started.elapsed().as_millis() as u64,
                ledger_path,
                created_at,
            );
            let _ = persist_eval_run_dual(pool, &opts.tenant_id, &ledger_path_buf, &tamper_run);
            return Err(EvalRunnerError::TamperDetected {
                run: Box::new(tamper_run),
                reason: err.reason,
                detail: err.detail,
            });
        }
    }

    let verified = match verify_sealed_eval_integrity(&eval_root) {
        Ok(b) => b,
        Err(err) => {
            let tamper_run = build_tamper_run_dto(
                run_id,
                &opts,
                offline_enforced,
                &err,
                started.elapsed().as_millis() as u64,
                ledger_path,
                created_at,
            );
            let _ = persist_eval_run_dual(pool, &opts.tenant_id, &ledger_path_buf, &tamper_run);
            return Err(EvalRunnerError::TamperDetected {
                run: Box::new(tamper_run),
                reason: err.reason,
                detail: err.detail,
            });
        }
    };

    let ablation = evaluate_paired_ablation(
        &verified.briefs,
        opts.design_craft_candidate_dir.as_deref(),
        &eval_root,
    );
    let calibration = evaluate_decision_calibration(&verified.calibration);
    let fault_injection = evaluate_fault_injections(&verified.faults);
    let estimated_cost_usd = 0.0f64;

    let write_actor_alive = shared_write_actor_for_pool(pool)
        .map(|a| a.is_alive())
        .unwrap_or(false);
    let sim_ctx = SimEvalContext {
        workspace_root: &eval_root,
        tier: opts.tier,
        offline_enforced,
        sealed_integrity_ok: true,
        write_actor_alive,
        ablation: &ablation,
        calibration: &calibration,
        fault_injection: &fault_injection,
        started,
        estimated_cost_usd,
    };

    let sim_cases: Vec<SimCaseResult> = if opts.run_simulated_users {
        evaluate_sim_user_25_cases(&verified.sim_cases, &sim_ctx)
    } else {
        evaluate_mandatory_security_sim_cases(&verified.sim_cases, &sim_ctx)
    };
    let milestones: Vec<MilestoneSummary> = summarize_milestones(&sim_cases);
    let sim_cases_passed = sim_cases
        .iter()
        .filter(|c| c.status == crate::eval_runner_suites::SimCaseStatus::Pass)
        .count();
    let sim_cases_blocked = sim_cases
        .iter()
        .filter(|c| c.status == crate::eval_runner_suites::SimCaseStatus::Blocked)
        .count();
    let sim_cases_gap = sim_cases
        .iter()
        .filter(|c| c.status == crate::eval_runner_suites::SimCaseStatus::Gap)
        .count();
    let sim_cases_total = if opts.run_simulated_users {
        verified.sim_cases.cases.len().max(25)
    } else {
        sim_cases.len().max(MANDATORY_SECURITY_GATE_CASE_IDS.len())
    };
    let duration_ms = started.elapsed().as_millis() as u64;

    let (verdict, security_gates_passed, security_blockers) =
        compute_eval_verdict(&EvalVerdictInput {
            tier: opts.tier,
            offline_enforced,
            duration_ms,
            estimated_cost_usd,
            ablation: &ablation,
            calibration: &calibration,
            fault_injection: &fault_injection,
            sim_cases: &sim_cases,
            run_simulated_users: opts.run_simulated_users,
        });

    let dto = EvalBenchmarkRunDto {
        run_id,
        project_id: opts.project_id,
        tier: opts.tier,
        offline_enforced,
        sealed_manifest_sha256: verified.snapshot.sealed_manifest_sha256,
        manifest_pin_source: verified.snapshot.manifest_pin_source,
        tamper_detected: false,
        tamper_reason: None,
        verdict,
        duration_ms,
        estimated_cost_usd,
        sim_cases_passed,
        sim_cases_blocked,
        sim_cases_gap,
        sim_cases_total,
        security_gates_passed,
        security_blockers,
        milestones,
        sim_cases,
        ablation,
        calibration,
        fault_injection,
        ledger_path,
        triggered_by: opts.triggered_by,
        created_at,
    };

    persist_eval_run_dual(pool, &opts.tenant_id, &ledger_path_buf, &dto)?;
    Ok(dto)
}

fn build_tamper_run_dto(
    run_id: String,
    opts: &EvalRunOptions,
    offline_enforced: bool,
    err: &SealedIntegrityError,
    duration_ms: u64,
    ledger_path: String,
    created_at: i64,
) -> EvalBenchmarkRunDto {
    EvalBenchmarkRunDto {
        run_id,
        project_id: opts.project_id.clone(),
        tier: opts.tier,
        offline_enforced,
        sealed_manifest_sha256: err.observed_sha256.clone(),
        manifest_pin_source: err.pin_source,
        tamper_detected: true,
        tamper_reason: Some(err.reason.clone()),
        verdict: EvalVerdict::SealedEvalTamperError,
        duration_ms,
        estimated_cost_usd: 0.0,
        sim_cases_passed: 0,
        sim_cases_blocked: 0,
        sim_cases_gap: 0,
        sim_cases_total: 25,
        security_gates_passed: false,
        security_blockers: vec![format!("UC-25:M5_SEALED_TAMPER_GUARD:{}", err.reason)],
        milestones: Vec::new(),
        sim_cases: Vec::new(),
        ablation: PairedAblationSummary::default(),
        calibration: DecisionCalibrationSummary::default(),
        fault_injection: FaultInjectionSummary::default(),
        ledger_path,
        triggered_by: opts.triggered_by.clone(),
        created_at,
    }
}

fn persist_eval_run_dual(
    pool: &SqlitePool,
    tenant_id: &str,
    ledger_path: &Path,
    dto: &EvalBenchmarkRunDto,
) -> Result<(), EvalRunnerError> {
    persist_eval_run_via_mpsc_actor(pool, tenant_id, dto)?;
    append_gate_history_jsonl_atomic(ledger_path, dto)?;
    Ok(())
}

fn persist_eval_run_via_mpsc_actor(
    pool: &SqlitePool,
    tenant_id: &str,
    dto: &EvalBenchmarkRunDto,
) -> Result<(), EvalRunnerError> {
    let actor =
        shared_write_actor_for_pool(pool).map_err(|e| EvalRunnerError::Storage(e.to_string()))?;
    let tenant = tenant_id.to_string();
    let row = dto.clone();
    let milestones_json = serde_json::to_string(&row.milestones).unwrap_or_else(|_| "[]".into());
    let sim_cases_json = serde_json::to_string(&row.sim_cases).unwrap_or_else(|_| "[]".into());
    let ablation_json = serde_json::to_string(&row.ablation).unwrap_or_else(|_| "{}".into());
    let calibration_json = serde_json::to_string(&row.calibration).unwrap_or_else(|_| "{}".into());
    let fault_json = serde_json::to_string(&row.fault_injection).unwrap_or_else(|_| "{}".into());
    let blockers_json =
        serde_json::to_string(&row.security_blockers).unwrap_or_else(|_| "[]".into());

    actor
        .execute_immediate(move |tx| {
            tx.execute_batch(V33_MIGRATION_SQL)?;
            tx.execute(
                "INSERT INTO eval_benchmark_runs (
                    id, tenant_id, project_id, tier, offline_enforced,
                    sealed_manifest_sha256, manifest_pin_source, tamper_detected, tamper_reason,
                    ablation_briefs_count, ablation_lift_percent, ablation_regressions,
                    design_craft_semver_passed, decision_providers_calibrated, decision_float_leaks,
                    fault_injected_count, fault_caught_count, fault_catch_rate,
                    sim_cases_passed, sim_cases_blocked, sim_cases_gap, sim_cases_total,
                    security_gates_passed, verdict,
                    milestones_json, sim_cases_json, ablation_json, calibration_json,
                    fault_injection_json, security_blockers_json,
                    duration_ms, estimated_cost_usd, triggered_by, created_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5,
                    ?6, ?7, ?8, ?9,
                    ?10, ?11, ?12,
                    ?13, ?14, ?15,
                    ?16, ?17, ?18,
                    ?19, ?20, ?21, ?22,
                    ?23, ?24,
                    ?25, ?26, ?27, ?28,
                    ?29, ?30,
                    ?31, ?32, ?33, ?34
                 )",
                params![
                    row.run_id,
                    tenant,
                    row.project_id,
                    row.tier.as_str(),
                    if row.offline_enforced { 1i64 } else { 0i64 },
                    row.sealed_manifest_sha256,
                    row.manifest_pin_source.as_str(),
                    if row.tamper_detected { 1i64 } else { 0i64 },
                    row.tamper_reason,
                    row.ablation.briefs_evaluated as i64,
                    row.ablation.lift_percent,
                    row.ablation.regressions_count as i64,
                    if row.ablation.design_craft_semver_passed {
                        1i64
                    } else {
                        0i64
                    },
                    row.calibration.providers.len() as i64,
                    row.calibration.total_float_leaks as i64,
                    row.fault_injection.injected_count as i64,
                    row.fault_injection.caught_count as i64,
                    row.fault_injection.catch_rate_percent,
                    row.sim_cases_passed as i64,
                    row.sim_cases_blocked as i64,
                    row.sim_cases_gap as i64,
                    row.sim_cases_total as i64,
                    if row.security_gates_passed {
                        1i64
                    } else {
                        0i64
                    },
                    row.verdict.as_str(),
                    milestones_json,
                    sim_cases_json,
                    ablation_json,
                    calibration_json,
                    fault_json,
                    blockers_json,
                    row.duration_ms as i64,
                    row.estimated_cost_usd,
                    row.triggered_by,
                    row.created_at,
                ],
            )?;
            Ok(())
        })
        .map_err(|e| EvalRunnerError::Storage(format!("V33_WRITE_ACTOR_FAILED: {e}")))
}

fn ensure_secure_evals_dir(evals_dir: &Path) -> Result<(), EvalRunnerError> {
    if let Some(cowok_dir) = evals_dir.parent() {
        if let Ok(meta) = fs::symlink_metadata(cowok_dir) {
            if meta.file_type().is_symlink() {
                return Err(EvalRunnerError::Storage(
                    "Symlink rejected on .cowok-design directory".to_string(),
                ));
            }
            if !meta.file_type().is_dir() {
                return Err(EvalRunnerError::Storage(
                    ".cowok-design path is not a directory".to_string(),
                ));
            }
        }
    }
    if let Ok(meta) = fs::symlink_metadata(evals_dir) {
        if meta.file_type().is_symlink() {
            return Err(EvalRunnerError::Storage(
                "Symlink rejected on .cowok-design/evals directory".to_string(),
            ));
        }
        if !meta.file_type().is_dir() {
            return Err(EvalRunnerError::Storage(
                ".cowok-design/evals path is not a directory".to_string(),
            ));
        }
    }

    fs::create_dir_all(evals_dir)
        .map_err(|e| EvalRunnerError::Storage(format!("mkdir evals: {e}")))?;

    // Re-verify non-symlink ancestry post-create and enforce FD-based 0o700 (no path-based chmod).
    let dirs_to_harden: Vec<&Path> = match evals_dir.parent() {
        Some(cowok_dir) => vec![cowok_dir, evals_dir],
        None => vec![evals_dir],
    };
    for dir in dirs_to_harden {
        let meta = fs::symlink_metadata(dir)
            .map_err(|e| EvalRunnerError::Storage(format!("stat '{}': {e}", dir.display())))?;
        if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
            return Err(EvalRunnerError::Storage(format!(
                "Symlink or non-directory rejected at '{}'",
                dir.display()
            )));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let mut dopts = OpenOptions::new();
            dopts
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
            let dir_fd = dopts.open(dir).map_err(|e| {
                EvalRunnerError::Storage(format!("open dir '{}' nofollow: {e}", dir.display()))
            })?;
            let fmeta = dir_fd.metadata().map_err(|e| {
                EvalRunnerError::Storage(format!("fstat dir '{}': {e}", dir.display()))
            })?;
            if !fmeta.file_type().is_dir() {
                return Err(EvalRunnerError::Storage(format!(
                    "Non-directory rejected post-open at '{}'",
                    dir.display()
                )));
            }
            dir_fd
                .set_permissions(fs::Permissions::from_mode(0o700))
                .map_err(|e| {
                    EvalRunnerError::Storage(format!("fchmod 0700 '{}': {e}", dir.display()))
                })?;
        }
    }
    Ok(())
}

fn append_gate_history_jsonl_atomic(
    ledger_path: &Path,
    dto: &EvalBenchmarkRunDto,
) -> Result<(), EvalRunnerError> {
    let parent = ledger_path
        .parent()
        .ok_or_else(|| EvalRunnerError::Storage("Invalid gate-history.jsonl parent".to_string()))?;

    ensure_secure_evals_dir(parent)?;

    if fs::symlink_metadata(ledger_path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(EvalRunnerError::Storage(
            "Symlink rejected on gate-history.jsonl".to_string(),
        ));
    }

    let record = serde_json::json!({
        "runId": dto.run_id,
        "tier": dto.tier,
        "verdict": dto.verdict,
        "sealedManifestSha256": dto.sealed_manifest_sha256,
        "manifestPinSource": dto.manifest_pin_source,
        "tamperDetected": dto.tamper_detected,
        "ablationLiftPercent": dto.ablation.lift_percent,
        "ablationRegressions": dto.ablation.regressions_count,
        "designCraftSemverPassed": dto.ablation.design_craft_semver_passed,
        "faultCatchRate": dto.fault_injection.catch_rate_percent,
        "simCasesPassed": dto.sim_cases_passed,
        "simCasesBlocked": dto.sim_cases_blocked,
        "simCasesGap": dto.sim_cases_gap,
        "simCasesTotal": dto.sim_cases_total,
        "securityGatesPassed": dto.security_gates_passed,
        "securityBlockers": dto.security_blockers,
        "durationMs": dto.duration_ms,
        "estimatedCostUsd": dto.estimated_cost_usd,
        "createdAt": dto.created_at,
    });
    let mut line =
        serde_json::to_string(&record).map_err(|e| EvalRunnerError::Storage(e.to_string()))?;
    line.push('\n');

    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }

    let mut file = opts
        .open(ledger_path)
        .map_err(|e| EvalRunnerError::Storage(format!("gate-history.jsonl open: {e}")))?;
    let fmeta = file
        .metadata()
        .map_err(|e| EvalRunnerError::Storage(format!("gate-history.jsonl fstat: {e}")))?;
    if !fmeta.file_type().is_file() {
        return Err(EvalRunnerError::Storage(
            "gate-history.jsonl must be a regular file".to_string(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| EvalRunnerError::Storage(format!("gate-history.jsonl fchmod: {e}")))?;
    }
    file.write_all(line.as_bytes())
        .and_then(|_| file.sync_data())
        .map_err(|e| EvalRunnerError::Storage(format!("gate-history.jsonl write: {e}")))?;
    Ok(())
}

pub fn list_eval_history(
    pool: &SqlitePool,
    workspace_root: &Path,
    tenant_id: &str,
    limit: usize,
    tier_filter: Option<EvalTier>,
    project_id_filter: Option<&str>,
) -> Result<EvalHistoryData, EvalRunnerError> {
    let eval_root = resolve_eval_workspace_root(workspace_root);
    let integrity = inspect_sealed_integrity(&eval_root);
    let conn = pool
        .connection()
        .map_err(|e| EvalRunnerError::Storage(e.to_string()))?;

    let table_exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'eval_benchmark_runs'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| EvalRunnerError::Storage(e.to_string()))?;
    if table_exists.is_none() {
        return Ok(EvalHistoryData {
            sealed_integrity_ok: integrity.sealed_integrity_ok,
            sealed_manifest_sha256: integrity.sealed_manifest_sha256,
            manifest_pin_source: integrity.manifest_pin_source,
            tamper_reason: integrity.tamper_reason,
            latest_run: None,
            runs: Vec::new(),
            total_runs: 0,
        });
    }

    let clamped_limit = limit.clamp(1, 100) as i64;
    let tier_str = tier_filter.map(|t| t.as_str().to_string());
    let proj_str = project_id_filter
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let total_runs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM eval_benchmark_runs
             WHERE tenant_id = ?1
               AND (?2 IS NULL OR tier = ?2)
               AND (?3 IS NULL OR project_id = ?3)",
            params![tenant_id, tier_str, proj_str],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| EvalRunnerError::Storage(e.to_string()))?
        .unwrap_or(0);

    let mut stmt = conn
        .prepare(
            "SELECT
                id, project_id, tier, offline_enforced,
                sealed_manifest_sha256, manifest_pin_source, tamper_detected, tamper_reason,
                sim_cases_passed, sim_cases_blocked, sim_cases_gap, sim_cases_total,
                security_gates_passed, verdict,
                milestones_json, sim_cases_json, ablation_json, calibration_json,
                fault_injection_json, security_blockers_json,
                duration_ms, estimated_cost_usd, triggered_by, created_at
             FROM eval_benchmark_runs
             WHERE tenant_id = ?1
               AND (?2 IS NULL OR tier = ?2)
               AND (?3 IS NULL OR project_id = ?3)
             ORDER BY created_at DESC, rowid DESC
             LIMIT ?4",
        )
        .map_err(|e| EvalRunnerError::Storage(e.to_string()))?;

    let rows = stmt
        .query_map(params![tenant_id, tier_str, proj_str, clamped_limit], |r| {
            let tier_raw: String = r.get(2)?;
            let pin_raw: String = r.get(5)?;
            let verdict_raw: String = r.get(13)?;
            let milestones_raw: String = r.get(14)?;
            let sim_cases_raw: String = r.get(15)?;
            let ablation_raw: String = r.get(16)?;
            let calibration_raw: String = r.get(17)?;
            let fault_raw: String = r.get(18)?;
            let blockers_raw: String = r.get(19)?;

            Ok(EvalBenchmarkRunDto {
                run_id: r.get(0)?,
                project_id: r.get(1)?,
                tier: EvalTier::parse(&tier_raw).unwrap_or(EvalTier::Pr),
                offline_enforced: r.get::<_, i64>(3)? != 0,
                sealed_manifest_sha256: r.get(4)?,
                manifest_pin_source: ManifestPinSource::parse(&pin_raw)
                    .unwrap_or(ManifestPinSource::CompiledBinary),
                tamper_detected: r.get::<_, i64>(6)? != 0,
                tamper_reason: r.get(7)?,
                sim_cases_passed: r.get::<_, i64>(8)?.max(0) as usize,
                sim_cases_blocked: r.get::<_, i64>(9)?.max(0) as usize,
                sim_cases_gap: r.get::<_, i64>(10)?.max(0) as usize,
                sim_cases_total: r.get::<_, i64>(11)?.max(1) as usize,
                security_gates_passed: r.get::<_, i64>(12)? != 0,
                verdict: EvalVerdict::parse(&verdict_raw).unwrap_or(EvalVerdict::Fail),
                milestones: serde_json::from_str(&milestones_raw).unwrap_or_default(),
                sim_cases: serde_json::from_str(&sim_cases_raw).unwrap_or_default(),
                ablation: serde_json::from_str(&ablation_raw).unwrap_or_default(),
                calibration: serde_json::from_str(&calibration_raw).unwrap_or_default(),
                fault_injection: serde_json::from_str(&fault_raw).unwrap_or_default(),
                security_blockers: serde_json::from_str(&blockers_raw).unwrap_or_default(),
                duration_ms: r.get::<_, i64>(20)?.max(0) as u64,
                estimated_cost_usd: r.get(21)?,
                ledger_path: GATE_HISTORY_REL_PATH.to_string(),
                triggered_by: r.get(22)?,
                created_at: r.get(23)?,
            })
        })
        .map_err(|e| EvalRunnerError::Storage(e.to_string()))?;

    let mut runs = Vec::new();
    for item in rows {
        runs.push(item.map_err(|e| EvalRunnerError::Storage(e.to_string()))?);
    }

    let latest_run = runs.first().cloned();
    Ok(EvalHistoryData {
        sealed_integrity_ok: integrity.sealed_integrity_ok,
        sealed_manifest_sha256: integrity.sealed_manifest_sha256,
        manifest_pin_source: integrity.manifest_pin_source,
        tamper_reason: integrity.tamper_reason,
        latest_run,
        runs,
        total_runs: total_runs.max(0) as usize,
    })
}
