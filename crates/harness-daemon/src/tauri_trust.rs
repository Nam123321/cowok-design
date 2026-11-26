// @story 12.2
// @implements AC-01, AC-03, AC-04 (Tauri v2 HMAC Capability Token Trust Boundary, Ed25519 + SHA-256 Sidecar/Updater Integrity Harness, O_NOFOLLOW + nlink==1 File Verification & Jailed Extraction Guard)
use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use ed25519_dalek::SigningKey;
pub use ed25519_dalek::VerifyingKey;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::routes::distribution_ops_crypto::{
    sign_manifest, verify_manifest_fail_closed, verifying_key_base64, ManifestVerifyError,
    SignedManifestEnvelope,
};
use crate::routes::{constant_time_eq, decode_hex, hmac_sha256};

pub const CAPABILITY_TOKEN_VERSION: u8 = 1;
pub const DEFAULT_CAPABILITY_TTL_SECS: i64 = 900;
pub const MAX_CAPABILITY_TTL_SECS: i64 = 3_600;
pub const MAX_CLOCK_SKEW_SECS: i64 = 30;
pub const CAPABILITY_NONCE_LRU_CAPACITY: usize = 8_192;
pub const MAX_REPLAY_ENTRIES: usize = CAPABILITY_NONCE_LRU_CAPACITY;
pub const MAX_LIVE_HANDLES: usize = 8_192;
pub const SIDECAR_INTEGRITY_MANIFEST_KIND: &str = "desktop_sidecar_integrity";
pub const MAX_UPDATER_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_EXTRACTED_ENTRY_BYTES: u64 = MAX_UPDATER_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityScope {
    WorkspaceRead,
    WorkspaceWrite,
    AssetImport,
    ExportArtifact,
    ApplyUpdate,
    SupportDiagnostics,
}

impl CapabilityScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorkspaceRead => "workspace_read",
            Self::WorkspaceWrite => "workspace_write",
            Self::AssetImport => "asset_import",
            Self::ExportArtifact => "export_artifact",
            Self::ApplyUpdate => "apply_update",
            Self::SupportDiagnostics => "support_diagnostics",
        }
    }

    /// Returns true if this granted scope authorizes `required`.
    /// Only exact scope matches or explicit `WorkspaceWrite -> WorkspaceRead` downgrade are permitted.
    pub fn authorizes(self, required: Self) -> bool {
        self == required
            || matches!(
                (self, required),
                (Self::WorkspaceWrite, Self::WorkspaceRead)
            )
    }

    /// Sensitive mutating scopes enforce single-use replay defense automatically.
    pub fn is_single_use_by_default(self) -> bool {
        matches!(self, Self::ExportArtifact | Self::ApplyUpdate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HmacCapabilityToken {
    pub token_version: u8,
    /// Opaque handle (`cap_<32-hex>`) mapped to canonical PathBuf solely inside Rust main.
    pub handle_id: String,
    /// Salted domain-separated SHA-256 digest of the canonical workspace root (non-reversible).
    pub workspace_digest_sha256: String,
    /// Safe display basename (sanitized: no `/`, `\`, `:`, `..`, NUL; max 64 chars).
    pub display_label: String,
    pub scope: CapabilityScope,
    /// 32-hex CSPRNG nonce tracked for replay defense on single-use/mutating calls.
    pub nonce: String,
    pub issued_at: i64,
    pub expires_at: i64,
    /// 64-hex RFC-2104 HMAC-SHA256 tag computed with Rust-main ephemeral 32-byte key.
    pub hmac_tag_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityVerifyOptions {
    pub required_scope: CapabilityScope,
    pub single_use: bool,
    pub now_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustBoundaryError {
    InvalidTokenFormat(String),
    InvalidTtl(String),
    ClockSkewRejected {
        issued_at: i64,
        now_unix: i64,
    },
    CapabilityExpired {
        expires_at: i64,
        now_unix: i64,
    },
    HmacSignatureMismatch,
    ScopeMismatch {
        granted: CapabilityScope,
        required: CapabilityScope,
    },
    UnknownHandle,
    CapabilityRevoked,
    CapabilityReplayDetected,
    ReplayCacheSaturated,
    HandleRegistrySaturated,
    RawPathLeakPrevented,
    PathTraversalDenied,
    SymlinkRejected,
    HardlinkRejected {
        nlink: u64,
    },
    InvalidRegularFile(String),
    FileTooLarge {
        actual: u64,
        max: u64,
    },
    EmptyArtifactFile,
    Sha256Mismatch {
        artifact_id: String,
    },
    ManifestVerification(ManifestVerifyError),
    StateLockPoisoned,
    Io(String),
}

pub type CapabilityError = TrustBoundaryError;

impl TrustBoundaryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidTokenFormat(_) => "INVALID_TOKEN_FORMAT",
            Self::InvalidTtl(_) => "INVALID_CAPABILITY_TTL",
            Self::ClockSkewRejected { .. } => "CLOCK_SKEW_REJECTED",
            Self::CapabilityExpired { .. } => "CAPABILITY_EXPIRED",
            Self::HmacSignatureMismatch => "HMAC_SIGNATURE_MISMATCH",
            Self::ScopeMismatch { .. } => "SCOPE_MISMATCH",
            Self::UnknownHandle => "UNKNOWN_CAPABILITY_HANDLE",
            Self::CapabilityRevoked => "CAPABILITY_REVOKED",
            Self::CapabilityReplayDetected => "CAPABILITY_REPLAY_DETECTED",
            Self::ReplayCacheSaturated => "REPLAY_CACHE_SATURATED",
            Self::HandleRegistrySaturated => "HANDLE_REGISTRY_SATURATED",
            Self::RawPathLeakPrevented => "RAW_PATH_LEAK_PREVENTED",
            Self::PathTraversalDenied => "PATH_TRAVERSAL_DENIED",
            Self::SymlinkRejected => "SYMLINK_REJECTED",
            Self::HardlinkRejected { .. } => "HARDLINK_REJECTED",
            Self::InvalidRegularFile(_) => "INVALID_REGULAR_FILE",
            Self::FileTooLarge { .. } => "FILE_TOO_LARGE",
            Self::EmptyArtifactFile => "EMPTY_ARTIFACT_FILE",
            Self::Sha256Mismatch { .. } => "SHA256_MISMATCH",
            Self::ManifestVerification(inner) => inner.code(),
            Self::StateLockPoisoned => "STATE_LOCK_POISONED",
            Self::Io(_) => "TRUST_BOUNDARY_IO_ERROR",
        }
    }

    /// Returns a strictly sanitized error message safe to cross into untrusted WebView IPC.
    /// Guaranteed to NEVER echo raw host filesystem paths.
    pub fn webview_safe_message(&self) -> String {
        match self {
            Self::InvalidTokenFormat(reason) => {
                format!("INVALID_TOKEN_FORMAT: {reason}")
            }
            Self::InvalidTtl(reason) => format!("INVALID_CAPABILITY_TTL: {reason}"),
            Self::ClockSkewRejected { .. } => {
                "CLOCK_SKEW_REJECTED: capability issued_at exceeds allowed clock skew".to_string()
            }
            Self::CapabilityExpired { .. } => {
                "CAPABILITY_EXPIRED: workspace capability token has expired".to_string()
            }
            Self::HmacSignatureMismatch => {
                "HMAC_SIGNATURE_MISMATCH: capability token signature failed verification"
                    .to_string()
            }
            Self::ScopeMismatch { granted, required } => format!(
                "SCOPE_MISMATCH: granted '{}' does not authorize '{}'",
                granted.as_str(),
                required.as_str()
            ),
            Self::UnknownHandle => {
                "UNKNOWN_CAPABILITY_HANDLE: capability handle is not registered in Rust main"
                    .to_string()
            }
            Self::CapabilityRevoked => {
                "CAPABILITY_REVOKED: capability handle has been revoked".to_string()
            }
            Self::CapabilityReplayDetected => {
                "CAPABILITY_REPLAY_DETECTED: single-use capability nonce was already consumed"
                    .to_string()
            }
            Self::ReplayCacheSaturated => {
                "REPLAY_CACHE_SATURATED: single-use capability replay cache is saturated with unexpired nonces"
                    .to_string()
            }
            Self::HandleRegistrySaturated => {
                "HANDLE_REGISTRY_SATURATED: active capability handle registry is at capacity"
                    .to_string()
            }
            Self::RawPathLeakPrevented => {
                "RAW_PATH_LEAK_PREVENTED: blocked token serialization containing host path segment"
                    .to_string()
            }
            Self::PathTraversalDenied => {
                "PATH_TRAVERSAL_DENIED: path escapes sandbox boundary".to_string()
            }
            Self::SymlinkRejected => {
                "SYMLINK_REJECTED: symbolic link rejected under O_NOFOLLOW policy".to_string()
            }
            Self::HardlinkRejected { nlink } => {
                format!("HARDLINK_REJECTED: hardlinked file rejected (nlink={nlink})")
            }
            Self::InvalidRegularFile(_) => {
                "INVALID_REGULAR_FILE: target is not a verified regular file".to_string()
            }
            Self::FileTooLarge { actual, max } => {
                format!("FILE_TOO_LARGE: file size {actual} exceeds limit {max}")
            }
            Self::EmptyArtifactFile => {
                "EMPTY_ARTIFACT_FILE: verified artifact must be non-empty".to_string()
            }
            Self::Sha256Mismatch { artifact_id } => {
                format!("SHA256_MISMATCH: digest verification failed for artifact '{artifact_id}'")
            }
            Self::ManifestVerification(inner) => format!("{}: {}", inner.code(), inner.message()),
            Self::StateLockPoisoned => {
                "STATE_LOCK_POISONED: internal trust registry lock unavailable".to_string()
            }
            Self::Io(_) => "TRUST_BOUNDARY_IO_ERROR: verified file operation failed".to_string(),
        }
    }
}

impl fmt::Display for TrustBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.webview_safe_message())
    }
}

impl std::error::Error for TrustBoundaryError {}

struct CanonicalHandleRecord {
    canonical_path: PathBuf,
    workspace_digest_sha256: String,
    expires_at_unix: i64,
    revoked: bool,
}

struct BoundedNonceReplayCache {
    max_entries: usize,
    entries: HashMap<String, i64>,
}

impl BoundedNonceReplayCache {
    fn with_capacity(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.max(1),
            entries: HashMap::new(),
        }
    }

    /// Prunes ONLY expired nonces (`expires_at_unix <= now_unix`) and fails closed
    /// with `ReplayCacheSaturated` if the cache remains at capacity with unexpired entries.
    fn check_and_record(
        &mut self,
        nonce: &str,
        expires_at_unix: i64,
        now_unix: i64,
    ) -> Result<(), TrustBoundaryError> {
        self.entries.retain(|_, exp| *exp > now_unix);
        if self.entries.contains_key(nonce) {
            return Err(TrustBoundaryError::CapabilityReplayDetected);
        }
        if self.entries.len() >= self.max_entries {
            return Err(TrustBoundaryError::ReplayCacheSaturated);
        }
        self.entries.insert(nonce.to_string(), expires_at_unix);
        Ok(())
    }
}

/// Rust-main-only capability registry holding an ephemeral CSPRNG HMAC-SHA256 secret
/// inside `Zeroizing<[u8; 32]>` and mapping opaque `cap_<hex>` handles to canonical paths.
pub struct WorkspaceCapabilityRegistry {
    secret_key: Zeroizing<[u8; 32]>,
    digest_salt: Zeroizing<[u8; 32]>,
    max_live_handles: usize,
    handles: Mutex<HashMap<String, CanonicalHandleRecord>>,
    replay_cache: Mutex<BoundedNonceReplayCache>,
}

impl Default for WorkspaceCapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceCapabilityRegistry {
    pub fn new() -> Self {
        Self::with_limits(MAX_REPLAY_ENTRIES, MAX_LIVE_HANDLES)
    }

    pub fn with_limits(max_replay_entries: usize, max_live_handles: usize) -> Self {
        let mut key = Zeroizing::new([0u8; 32]);
        let mut salt = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(&mut *key);
        OsRng.fill_bytes(&mut *salt);
        Self {
            secret_key: key,
            digest_salt: salt,
            max_live_handles: max_live_handles.max(1),
            handles: Mutex::new(HashMap::new()),
            replay_cache: Mutex::new(BoundedNonceReplayCache::with_capacity(max_replay_entries)),
        }
    }

    pub fn active_handle_count(&self) -> usize {
        self.handles.lock().map(|h| h.len()).unwrap_or(0)
    }

    pub fn active_replay_count(&self) -> usize {
        self.replay_cache
            .lock()
            .map(|c| c.entries.len())
            .unwrap_or(0)
    }

    /// Convenience wrapper using wall-clock UTC timestamp and explicit TTL in seconds.
    pub fn issue_token(
        &self,
        workspace_root: &Path,
        scope: CapabilityScope,
        ttl_secs: u64,
    ) -> Result<HmacCapabilityToken, TrustBoundaryError> {
        let ttl_i64 = i64::try_from(ttl_secs)
            .map_err(|_| TrustBoundaryError::InvalidTtl("ttl_secs overflow".to_string()))?;
        self.issue_capability(workspace_root, scope, Some(ttl_i64), Utc::now().timestamp())
    }

    /// Issues an opaque HMAC-SHA256 capability token for `workspace_root` after enforcing
    /// traversal checks, `O_NOFOLLOW` non-symlink checks, and raw-path redaction invariants.
    pub fn issue_capability(
        &self,
        workspace_root: &Path,
        scope: CapabilityScope,
        ttl_secs: Option<i64>,
        now_unix: i64,
    ) -> Result<HmacCapabilityToken, TrustBoundaryError> {
        let ttl = ttl_secs.unwrap_or(DEFAULT_CAPABILITY_TTL_SECS);
        if ttl <= 0 || ttl > MAX_CAPABILITY_TTL_SECS {
            return Err(TrustBoundaryError::InvalidTtl(format!(
                "ttl must be within 1..={MAX_CAPABILITY_TTL_SECS} seconds"
            )));
        }

        let canonical_root = validate_capability_target_path(workspace_root, scope)?;
        let workspace_digest_sha256 =
            compute_salted_workspace_digest(&self.digest_salt[..], &canonical_root);
        let display_label = sanitize_display_label(&canonical_root);
        let handle_id = format!("cap_{}", csprng_hex_16());
        let nonce = csprng_hex_16();
        let issued_at = now_unix;
        let expires_at = issued_at
            .checked_add(ttl)
            .ok_or_else(|| TrustBoundaryError::InvalidTtl("expires_at overflow".to_string()))?;

        let signing_input = build_capability_mac_input(
            CAPABILITY_TOKEN_VERSION,
            &handle_id,
            &workspace_digest_sha256,
            &display_label,
            scope,
            &nonce,
            issued_at,
            expires_at,
        );
        let tag_bytes = hmac_sha256(&self.secret_key[..], signing_input.as_bytes());
        let hmac_tag_hex = hex_encode(&tag_bytes);

        let token = HmacCapabilityToken {
            token_version: CAPABILITY_TOKEN_VERSION,
            handle_id: handle_id.clone(),
            workspace_digest_sha256: workspace_digest_sha256.clone(),
            display_label,
            scope,
            nonce,
            issued_at,
            expires_at,
            hmac_tag_hex,
        };

        assert_token_leaks_no_raw_path(&token, &canonical_root)?;

        let mut handles = self
            .handles
            .lock()
            .map_err(|_| TrustBoundaryError::StateLockPoisoned)?;
        handles.retain(|_, rec| rec.expires_at_unix > now_unix);
        if handles.len() >= self.max_live_handles {
            return Err(TrustBoundaryError::HandleRegistrySaturated);
        }
        handles.insert(
            handle_id,
            CanonicalHandleRecord {
                canonical_path: canonical_root,
                workspace_digest_sha256,
                expires_at_unix: expires_at,
                revoked: false,
            },
        );
        Ok(token)
    }

    /// Verifies the HMAC tag in constant time, enforces TTL/expiry/clock-skew/scope/replay/revocation,
    /// re-verifies live `O_NOFOLLOW` target integrity, and resolves the canonical `PathBuf` strictly
    /// inside Rust main.
    pub fn verify_and_resolve(
        &self,
        token: &HmacCapabilityToken,
        opts: &CapabilityVerifyOptions,
    ) -> Result<PathBuf, TrustBoundaryError> {
        if token.token_version != CAPABILITY_TOKEN_VERSION {
            return Err(TrustBoundaryError::InvalidTokenFormat(format!(
                "unsupported tokenVersion {}",
                token.token_version
            )));
        }
        if !is_valid_handle_id(&token.handle_id) {
            return Err(TrustBoundaryError::InvalidTokenFormat(
                "malformed handleId".to_string(),
            ));
        }
        if !is_hex_len(&token.workspace_digest_sha256, 64) {
            return Err(TrustBoundaryError::InvalidTokenFormat(
                "malformed workspaceDigestSha256".to_string(),
            ));
        }
        if !is_hex_len(&token.nonce, 32) {
            return Err(TrustBoundaryError::InvalidTokenFormat(
                "malformed nonce".to_string(),
            ));
        }
        if token.display_label.is_empty()
            || token.display_label.len() > 64
            || token.display_label.contains('/')
            || token.display_label.contains('\\')
            || token.display_label.contains("..")
            || token.display_label.contains('\0')
            || token.display_label.chars().any(char::is_control)
            || !token
                .display_label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' '))
        {
            return Err(TrustBoundaryError::InvalidTokenFormat(
                "malformed displayLabel".to_string(),
            ));
        }
        if token.issued_at > token.expires_at {
            return Err(TrustBoundaryError::InvalidTokenFormat(
                "issuedAt exceeds expiresAt".to_string(),
            ));
        }
        let span = token.expires_at.saturating_sub(token.issued_at);
        if span <= 0 || span > MAX_CAPABILITY_TTL_SECS {
            return Err(TrustBoundaryError::InvalidTtl(
                "token lifetime exceeds policy ceiling".to_string(),
            ));
        }
        if token.issued_at > opts.now_unix.saturating_add(MAX_CLOCK_SKEW_SECS) {
            return Err(TrustBoundaryError::ClockSkewRejected {
                issued_at: token.issued_at,
                now_unix: opts.now_unix,
            });
        }
        if token.expires_at <= opts.now_unix {
            return Err(TrustBoundaryError::CapabilityExpired {
                expires_at: token.expires_at,
                now_unix: opts.now_unix,
            });
        }

        let provided_tag = decode_hex(&token.hmac_tag_hex)
            .filter(|b| b.len() == 32)
            .ok_or(TrustBoundaryError::HmacSignatureMismatch)?;
        let signing_input = build_capability_mac_input(
            token.token_version,
            &token.handle_id,
            &token.workspace_digest_sha256,
            &token.display_label,
            token.scope,
            &token.nonce,
            token.issued_at,
            token.expires_at,
        );
        let expected_tag = hmac_sha256(&self.secret_key[..], signing_input.as_bytes());
        if !constant_time_eq(&provided_tag, &expected_tag) {
            return Err(TrustBoundaryError::HmacSignatureMismatch);
        }

        if !token.scope.authorizes(opts.required_scope) {
            return Err(TrustBoundaryError::ScopeMismatch {
                granted: token.scope,
                required: opts.required_scope,
            });
        }

        let resolved_path = {
            let mut handles = self
                .handles
                .lock()
                .map_err(|_| TrustBoundaryError::StateLockPoisoned)?;
            handles.retain(|_, rec| rec.expires_at_unix > opts.now_unix);
            let record = handles
                .get(&token.handle_id)
                .ok_or(TrustBoundaryError::UnknownHandle)?;
            if record.revoked {
                return Err(TrustBoundaryError::CapabilityRevoked);
            }
            if record.expires_at_unix <= opts.now_unix {
                return Err(TrustBoundaryError::CapabilityExpired {
                    expires_at: record.expires_at_unix,
                    now_unix: opts.now_unix,
                });
            }
            if !constant_time_eq(
                record.workspace_digest_sha256.as_bytes(),
                token.workspace_digest_sha256.as_bytes(),
            ) {
                return Err(TrustBoundaryError::HmacSignatureMismatch);
            }
            record.canonical_path.clone()
        };

        if opts.single_use || token.scope.is_single_use_by_default() {
            let mut cache = self
                .replay_cache
                .lock()
                .map_err(|_| TrustBoundaryError::StateLockPoisoned)?;
            cache.check_and_record(&token.nonce, token.expires_at, opts.now_unix)?;
        }

        let live_canonical = validate_capability_target_path(&resolved_path, token.scope)?;
        if live_canonical != resolved_path {
            return Err(TrustBoundaryError::SymlinkRejected);
        }
        Ok(live_canonical)
    }

    /// Immediately revokes an issued handle so subsequent calls fail closed.
    pub fn revoke_handle(&self, handle_id: &str) -> bool {
        let Ok(mut handles) = self.handles.lock() else {
            return false;
        };
        if let Some(rec) = handles.get_mut(handle_id) {
            rec.revoked = true;
            true
        } else {
            false
        }
    }
}

/// Ensures serialized `HmacCapabilityToken` JSON never exposes raw host filesystem paths
/// or OS home/system path prefixes.
pub fn assert_token_leaks_no_raw_path(
    token: &HmacCapabilityToken,
    canonical_root: &Path,
) -> Result<(), TrustBoundaryError> {
    let serialized =
        serde_json::to_string(token).map_err(|_| TrustBoundaryError::RawPathLeakPrevented)?;
    let lower = serialized.to_ascii_lowercase();
    let full_raw = canonical_root.to_string_lossy().to_ascii_lowercase();
    if !full_raw.is_empty() && lower.contains(&full_raw) {
        return Err(TrustBoundaryError::RawPathLeakPrevented);
    }
    if let Some(parent) = canonical_root.parent() {
        let parent_raw = parent.to_string_lossy().to_ascii_lowercase();
        if parent_raw.len() > 1 && lower.contains(&parent_raw) {
            return Err(TrustBoundaryError::RawPathLeakPrevented);
        }
    }
    for forbidden_prefix in [
        "/users/",
        "/home/",
        "/private/",
        "/var/",
        "/tmp/",
        "c:\\",
        "c:/",
        "\\\\",
    ] {
        if lower.contains(forbidden_prefix) {
            return Err(TrustBoundaryError::RawPathLeakPrevented);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarArtifactKind {
    SidecarBinary,
    DesktopUpdater,
    SkillAdvisoryPack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SidecarArtifactEntry {
    pub artifact_id: String,
    pub target_triple: String,
    pub relative_path: String,
    pub byte_size: u64,
    pub sha256: String,
    pub kind: SidecarArtifactKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SidecarIntegrityManifest {
    pub channel: String,
    pub app_version: String,
    pub min_compatible_version: String,
    pub release_seq: i64,
    pub artifacts: Vec<SidecarArtifactEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterSignatureReceipt {
    pub verified: bool,
    pub manifest_seq: i64,
    pub key_id: String,
    pub public_key_base64: String,
    pub artifact_id: String,
    pub target_triple: String,
    pub computed_sha256: String,
    pub expected_sha256: String,
    pub byte_size: u64,
    pub no_follow_checked: bool,
    pub nlink_verified: u64,
    pub verified_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedArchiveEntry {
    pub relative_path: String,
    pub bytes: Vec<u8>,
    pub expected_sha256: Option<String>,
    pub unix_mode: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedEntryReceipt {
    pub relative_path: String,
    pub byte_size: usize,
    pub sha256: String,
    pub nlink_verified: u64,
}

/// Signs a `SidecarIntegrityManifest` using Story 14.3's canonical `SignedManifestEnvelope`.
pub fn sign_sidecar_integrity_manifest(
    signing_key: &SigningKey,
    key_id: &str,
    issued_at: i64,
    expires_at: i64,
    manifest: SidecarIntegrityManifest,
) -> Result<SignedManifestEnvelope<SidecarIntegrityManifest>, TrustBoundaryError> {
    if manifest.channel != "desktop" {
        return Err(TrustBoundaryError::ManifestVerification(
            ManifestVerifyError::InvalidEnvelope(format!(
                "sidecar integrity channel must be 'desktop', got '{}'",
                manifest.channel
            )),
        ));
    }
    if manifest.release_seq < 1 || manifest.artifacts.is_empty() {
        return Err(TrustBoundaryError::ManifestVerification(
            ManifestVerifyError::InvalidEnvelope(
                "releaseSeq must be >= 1 and artifacts must be non-empty".to_string(),
            ),
        ));
    }
    let seq = manifest.release_seq;
    sign_manifest(
        signing_key,
        key_id,
        SIDECAR_INTEGRITY_MANIFEST_KIND,
        seq,
        issued_at,
        expires_at,
        manifest,
    )
    .map_err(|e| {
        TrustBoundaryError::ManifestVerification(ManifestVerifyError::InvalidEnvelope(e.message()))
    })
}

/// Fail-closed Ed25519 + SHA-256 + `O_NOFOLLOW` + `nlink == 1` verification harness for
/// native sidecar binaries and desktop auto-updater bundles.
pub fn verify_sidecar_and_updater_integrity(
    envelope: &SignedManifestEnvelope<SidecarIntegrityManifest>,
    trusted_public_key: &VerifyingKey,
    artifact_root: &Path,
    last_applied_seq: i64,
    now_unix: i64,
) -> Result<Vec<UpdaterSignatureReceipt>, TrustBoundaryError> {
    if last_applied_seq > 0 && envelope.manifest_seq <= last_applied_seq {
        return Err(TrustBoundaryError::ManifestVerification(
            ManifestVerifyError::RollbackAttackDetected {
                manifest_seq: envelope.manifest_seq,
                last_seen_seq: last_applied_seq,
            },
        ));
    }

    verify_manifest_fail_closed(
        envelope,
        SIDECAR_INTEGRITY_MANIFEST_KIND,
        trusted_public_key,
        last_applied_seq.max(0),
        now_unix,
    )
    .map_err(TrustBoundaryError::ManifestVerification)?;

    let payload = &envelope.payload;
    if payload.channel != "desktop" {
        return Err(TrustBoundaryError::ManifestVerification(
            ManifestVerifyError::InvalidEnvelope(format!(
                "invalid channel '{}', expected 'desktop'",
                payload.channel
            )),
        ));
    }
    if payload.release_seq != envelope.manifest_seq {
        return Err(TrustBoundaryError::ManifestVerification(
            ManifestVerifyError::InvalidEnvelope(
                "payload releaseSeq does not match signed envelope manifestSeq".to_string(),
            ),
        ));
    }
    if payload.artifacts.is_empty() {
        return Err(TrustBoundaryError::ManifestVerification(
            ManifestVerifyError::InvalidEnvelope(
                "sidecar integrity manifest contains no artifacts".to_string(),
            ),
        ));
    }

    reject_symlink_ancestors(artifact_root)?;
    let root_meta = fs::symlink_metadata(artifact_root)
        .map_err(|e| TrustBoundaryError::Io(format!("stat artifact root failed: {e}")))?;
    if root_meta.file_type().is_symlink() || !root_meta.file_type().is_dir() {
        return Err(TrustBoundaryError::SymlinkRejected);
    }
    let canon_root = artifact_root
        .canonicalize()
        .map_err(|e| TrustBoundaryError::Io(format!("canonicalize artifact root failed: {e}")))?;

    let pubkey_b64 = verifying_key_base64(trusted_public_key);
    let mut receipts = Vec::with_capacity(payload.artifacts.len());

    for entry in &payload.artifacts {
        if !is_hex_len(&entry.sha256, 64) {
            return Err(TrustBoundaryError::ManifestVerification(
                ManifestVerifyError::InvalidEnvelope(format!(
                    "artifact '{}' has malformed sha256 digest",
                    entry.artifact_id
                )),
            ));
        }
        let rel = validate_relative_sandbox_path(&entry.relative_path)?;
        let full_target = canon_root.join(&rel);
        reject_symlink_ancestors(&full_target)?;

        let (mut file, nlink, meta_len) = open_verified_regular_file_nofollow_nlink1(&full_target)?;
        if meta_len == 0 {
            return Err(TrustBoundaryError::EmptyArtifactFile);
        }
        if meta_len > MAX_UPDATER_BYTES {
            return Err(TrustBoundaryError::FileTooLarge {
                actual: meta_len,
                max: MAX_UPDATER_BYTES,
            });
        }

        let (computed_sha256, streamed_len) = stream_sha256_bounded(&mut file, MAX_UPDATER_BYTES)?;
        if streamed_len == 0 {
            return Err(TrustBoundaryError::EmptyArtifactFile);
        }
        if streamed_len != entry.byte_size {
            return Err(TrustBoundaryError::Sha256Mismatch {
                artifact_id: entry.artifact_id.clone(),
            });
        }
        let expected_lower = entry.sha256.to_ascii_lowercase();
        if !constant_time_eq(computed_sha256.as_bytes(), expected_lower.as_bytes()) {
            return Err(TrustBoundaryError::Sha256Mismatch {
                artifact_id: entry.artifact_id.clone(),
            });
        }

        receipts.push(UpdaterSignatureReceipt {
            verified: true,
            manifest_seq: envelope.manifest_seq,
            key_id: envelope.key_id.clone(),
            public_key_base64: pubkey_b64.clone(),
            artifact_id: entry.artifact_id.clone(),
            target_triple: entry.target_triple.clone(),
            computed_sha256,
            expected_sha256: expected_lower,
            byte_size: streamed_len,
            no_follow_checked: true,
            nlink_verified: nlink,
            verified_at: now_unix,
        });
    }

    Ok(receipts)
}

/// Safely extracts verified bundle/archive entries strictly inside `sandbox_root`,
/// rejecting `..`, absolute paths, Windows prefixes, symlinks (`O_NOFOLLOW`), and
/// hardlinks (`nlink > 1`) before writing atomically.
pub fn extract_verified_entries_jailed(
    sandbox_root: &Path,
    entries: &[VerifiedArchiveEntry],
) -> Result<Vec<ExtractedEntryReceipt>, TrustBoundaryError> {
    reject_symlink_ancestors(sandbox_root)?;
    let root_meta = fs::symlink_metadata(sandbox_root)
        .map_err(|e| TrustBoundaryError::Io(format!("stat sandbox root failed: {e}")))?;
    if root_meta.file_type().is_symlink() || !root_meta.file_type().is_dir() {
        return Err(TrustBoundaryError::SymlinkRejected);
    }
    let canon_root = sandbox_root
        .canonicalize()
        .map_err(|e| TrustBoundaryError::Io(format!("canonicalize sandbox root failed: {e}")))?;

    let mut receipts = Vec::with_capacity(entries.len());
    for entry in entries {
        let rel = validate_relative_sandbox_path(&entry.relative_path)?;
        let byte_len = entry.bytes.len() as u64;
        if byte_len > MAX_EXTRACTED_ENTRY_BYTES {
            return Err(TrustBoundaryError::FileTooLarge {
                actual: byte_len,
                max: MAX_EXTRACTED_ENTRY_BYTES,
            });
        }
        let actual_sha = sha256_hex(&entry.bytes);
        if let Some(ref expected) = entry.expected_sha256 {
            let expected_lower = expected.to_ascii_lowercase();
            if !is_hex_len(&expected_lower, 64)
                || !constant_time_eq(actual_sha.as_bytes(), expected_lower.as_bytes())
            {
                return Err(TrustBoundaryError::Sha256Mismatch {
                    artifact_id: entry.relative_path.clone(),
                });
            }
        }

        let target_path = canon_root.join(&rel);
        let nlink =
            write_atomic_nofollow_nlink1(&canon_root, &target_path, &entry.bytes, entry.unix_mode)?;
        receipts.push(ExtractedEntryReceipt {
            relative_path: entry.relative_path.clone(),
            byte_size: entry.bytes.len(),
            sha256: actual_sha,
            nlink_verified: nlink,
        });
    }
    Ok(receipts)
}

/// Alias matching Story 12.2 jailed bundle extraction naming.
pub fn extract_verified_bundle_jailed(
    sandbox_root: &Path,
    entries: &[VerifiedArchiveEntry],
) -> Result<Vec<ExtractedEntryReceipt>, TrustBoundaryError> {
    extract_verified_entries_jailed(sandbox_root, entries)
}

/// Opens a regular file using live-FD `O_NOFOLLOW | O_CLOEXEC`, verifies `!is_symlink`,
/// `is_file()`, and `nlink == 1` on Unix directly on the opened FD (zero TOCTOU window).
pub fn open_verified_regular_file_nofollow_nlink1(
    path: &Path,
) -> Result<(File, u64, u64), TrustBoundaryError> {
    reject_symlink_ancestors(path)?;
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = opts.open(path).map_err(|err| {
        #[cfg(unix)]
        if err.raw_os_error() == Some(libc::ELOOP) {
            return TrustBoundaryError::SymlinkRejected;
        }
        TrustBoundaryError::Io(format!("open verified file failed: {err}"))
    })?;
    let meta = file
        .metadata()
        .map_err(|e| TrustBoundaryError::Io(format!("fstat failed: {e}")))?;
    if !meta.is_file() {
        return Err(TrustBoundaryError::InvalidRegularFile(
            "opened descriptor is not a regular file".to_string(),
        ));
    }
    #[cfg(unix)]
    let nlink = {
        use std::os::unix::fs::MetadataExt;
        let links = meta.nlink();
        if links != 1 {
            return Err(TrustBoundaryError::HardlinkRejected { nlink: links });
        }
        links
    };
    #[cfg(not(unix))]
    let nlink = 1u64;

    Ok((file, nlink, meta.len()))
}

/// Atomically writes `bytes` into `target_path` inside `canon_sandbox_root`, enforcing
/// symlink-free ancestor directories, `O_NOFOLLOW | O_CLOEXEC` temp file creation,
/// pre-rename symlink/hardlink checks (`nlink == 1`), and post-write containment verification.
pub fn write_atomic_nofollow_nlink1(
    canon_sandbox_root: &Path,
    target_path: &Path,
    bytes: &[u8],
    unix_mode: u32,
) -> Result<u64, TrustBoundaryError> {
    reject_symlink_ancestors(target_path)?;
    let parent = target_path
        .parent()
        .ok_or(TrustBoundaryError::PathTraversalDenied)?;

    // Walk and create intermediate directories safely under canon_sandbox_root
    let rel_parent = parent
        .strip_prefix(canon_sandbox_root)
        .map_err(|_| TrustBoundaryError::PathTraversalDenied)?;
    let mut cursor = canon_sandbox_root.to_path_buf();
    for comp in rel_parent.components() {
        let Component::Normal(seg) = comp else {
            return Err(TrustBoundaryError::PathTraversalDenied);
        };
        cursor.push(seg);
        match fs::symlink_metadata(&cursor) {
            Ok(meta) => {
                if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
                    return Err(TrustBoundaryError::SymlinkRejected);
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&cursor)
                    .map_err(|e| TrustBoundaryError::Io(format!("create directory failed: {e}")))?;
                let check = fs::symlink_metadata(&cursor).map_err(|e| {
                    TrustBoundaryError::Io(format!("stat created directory failed: {e}"))
                })?;
                if check.file_type().is_symlink() || !check.file_type().is_dir() {
                    return Err(TrustBoundaryError::SymlinkRejected);
                }
            }
            Err(err) => {
                return Err(TrustBoundaryError::Io(format!(
                    "inspect directory failed: {err}"
                )));
            }
        }
    }

    // Reject pre-existing symlink or hardlinked target file
    reject_existing_symlink_or_hardlink(target_path)?;

    let tmp_name = format!(".cowok-atomic-tmp-{}", Uuid::new_v4().simple());
    let tmp_path = parent.join(tmp_name);
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(unix_mode & 0o777);
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(not(unix))]
    let _ = unix_mode;

    let write_res = opts
        .open(&tmp_path)
        .and_then(|mut f| f.write_all(bytes).and_then(|_| f.sync_all()));
    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(TrustBoundaryError::Io(format!(
            "atomic temp write failed: {err}"
        )));
    }

    if let Err(err) = reject_existing_symlink_or_hardlink(target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    if let Err(err) = fs::rename(&tmp_path, target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(TrustBoundaryError::Io(format!(
            "atomic rename failed: {err}"
        )));
    }

    let post_meta = fs::symlink_metadata(target_path)
        .map_err(|e| TrustBoundaryError::Io(format!("post-write stat failed: {e}")))?;
    if post_meta.file_type().is_symlink() || !post_meta.file_type().is_file() {
        return Err(TrustBoundaryError::SymlinkRejected);
    }
    #[cfg(unix)]
    let nlink = {
        use std::os::unix::fs::MetadataExt;
        let links = post_meta.nlink();
        if links != 1 {
            return Err(TrustBoundaryError::HardlinkRejected { nlink: links });
        }
        links
    };
    #[cfg(not(unix))]
    let nlink = 1u64;

    let canon_target = target_path
        .canonicalize()
        .map_err(|e| TrustBoundaryError::Io(format!("post-write canonicalize failed: {e}")))?;
    if !canon_target.starts_with(canon_sandbox_root) {
        return Err(TrustBoundaryError::PathTraversalDenied);
    }
    Ok(nlink)
}

fn reject_existing_symlink_or_hardlink(path: &Path) -> Result<(), TrustBoundaryError> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(TrustBoundaryError::SymlinkRejected);
            }
            if !meta.file_type().is_file() {
                return Err(TrustBoundaryError::InvalidRegularFile(
                    "target path exists as non-file".to_string(),
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if meta.nlink() != 1 {
                    return Err(TrustBoundaryError::HardlinkRejected {
                        nlink: meta.nlink(),
                    });
                }
            }
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(TrustBoundaryError::Io(format!(
            "inspect target file failed: {err}"
        ))),
    }
}

pub fn validate_relative_sandbox_path(raw: &str) -> Result<PathBuf, TrustBoundaryError> {
    if raw.is_empty() || raw.contains('\0') || raw.contains('\\') {
        return Err(TrustBoundaryError::PathTraversalDenied);
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(TrustBoundaryError::PathTraversalDenied);
    }
    let mut out = PathBuf::new();
    let mut count = 0usize;
    for comp in path.components() {
        match comp {
            Component::Normal(seg) => {
                if seg == "." || seg == ".." {
                    return Err(TrustBoundaryError::PathTraversalDenied);
                }
                out.push(seg);
                count += 1;
            }
            _ => return Err(TrustBoundaryError::PathTraversalDenied),
        }
    }
    if count == 0 {
        return Err(TrustBoundaryError::PathTraversalDenied);
    }
    Ok(out)
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

/// Walks every path segment from root down to `path` and rejects `..`, NUL bytes,
/// and any non-OS-root symlink component.
pub fn reject_symlink_ancestors(path: &Path) -> Result<(), TrustBoundaryError> {
    if path.to_string_lossy().contains('\0') {
        return Err(TrustBoundaryError::PathTraversalDenied);
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(TrustBoundaryError::PathTraversalDenied);
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
            Component::ParentDir => return Err(TrustBoundaryError::PathTraversalDenied),
            Component::Normal(seg) => {
                current.push(seg);
                if is_os_root_symlink(&current) {
                    continue;
                }
                match fs::symlink_metadata(&current) {
                    Ok(meta) => {
                        if meta.file_type().is_symlink() {
                            return Err(TrustBoundaryError::SymlinkRejected);
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
                    Err(err) => {
                        return Err(TrustBoundaryError::Io(format!(
                            "inspect ancestor failed: {err}"
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_capability_target_path(
    target: &Path,
    scope: CapabilityScope,
) -> Result<PathBuf, TrustBoundaryError> {
    reject_symlink_ancestors(target)?;
    let meta = fs::symlink_metadata(target)
        .map_err(|e| TrustBoundaryError::Io(format!("stat target failed: {e}")))?;
    if meta.file_type().is_symlink() {
        return Err(TrustBoundaryError::SymlinkRejected);
    }
    if scope == CapabilityScope::AssetImport && meta.file_type().is_file() {
        let (_file, _nlink, _len) = open_verified_regular_file_nofollow_nlink1(target)?;
    } else if !meta.file_type().is_dir() {
        return Err(TrustBoundaryError::InvalidRegularFile(
            "workspace capability target must be an existing directory".to_string(),
        ));
    }
    target
        .canonicalize()
        .map_err(|e| TrustBoundaryError::Io(format!("canonicalize target failed: {e}")))
}

fn compute_salted_workspace_digest(salt: &[u8], canonical_root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"cowok-ws-digest-v1:");
    hasher.update(salt);
    hasher.update(b":");
    hasher.update(canonical_root.to_string_lossy().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn sanitize_display_label(canonical_root: &Path) -> String {
    let raw = canonical_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("workspace");
    let filtered: String = raw
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' '))
        .take(64)
        .collect();
    let trimmed = filtered.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || trimmed.contains("..") {
        "workspace".to_string()
    } else {
        trimmed.to_string()
    }
}

fn push_len_prefixed_field(out: &mut String, field: &str) {
    use std::fmt::Write as _;
    let _ = write!(out, "{}:{field}|", field.len());
}

#[allow(clippy::too_many_arguments)]
fn build_capability_mac_input(
    token_version: u8,
    handle_id: &str,
    workspace_digest_sha256: &str,
    display_label: &str,
    scope: CapabilityScope,
    nonce: &str,
    issued_at: i64,
    expires_at: i64,
) -> String {
    let version_str = token_version.to_string();
    let issued_str = issued_at.to_string();
    let expires_str = expires_at.to_string();
    let mut out = String::with_capacity(256);
    push_len_prefixed_field(&mut out, "cowok-tauri-cap-v1");
    push_len_prefixed_field(&mut out, &version_str);
    push_len_prefixed_field(&mut out, handle_id);
    push_len_prefixed_field(&mut out, workspace_digest_sha256);
    push_len_prefixed_field(&mut out, display_label);
    push_len_prefixed_field(&mut out, scope.as_str());
    push_len_prefixed_field(&mut out, nonce);
    push_len_prefixed_field(&mut out, &issued_str);
    push_len_prefixed_field(&mut out, &expires_str);
    out
}

fn stream_sha256_bounded(
    file: &mut File,
    max_bytes: u64,
) -> Result<(String, u64), TrustBoundaryError> {
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 16 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| TrustBoundaryError::Io(format!("read verified file failed: {e}")))?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        if total > max_bytes {
            return Err(TrustBoundaryError::FileTooLarge {
                actual: total,
                max: max_bytes,
            });
        }
        hasher.update(&buf[..n]);
    }
    Ok((format!("{:x}", hasher.finalize()), total))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn csprng_hex_16() -> String {
    let mut buf = [0u8; 16];
    OsRng.fill_bytes(&mut buf);
    hex_encode(&buf)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn is_hex_len(raw: &str, expected_len: usize) -> bool {
    raw.len() == expected_len && raw.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_valid_handle_id(raw: &str) -> bool {
    raw.strip_prefix("cap_")
        .is_some_and(|hex| is_hex_len(hex, 32))
}
