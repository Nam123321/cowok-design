// @story 14.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Ed25519 Supply-Chain Cryptographic Kernel, Fail-Closed Manifest Verifier, Deterministic Zero-PII A/B Bucketing, Security Denylist, Local Override Resolver & Offline Licenses)
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::{STANDARD as B64_STD, URL_SAFE_NO_PAD as B64_URL};
use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

pub const SIGNED_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_ED25519_KEY_ID: &str = "cowok-ed25519-v1";
pub const OFFLINE_LICENSE_TOKEN_PREFIX: &str = "cwk_lic_v1";
pub const DEFAULT_MANIFEST_TTL_SECS: i64 = 7 * 24 * 60 * 60;

pub const PROTECTED_SECURITY_INVARIANTS: [&str; 6] = [
    "HUMAN_ONLY",
    "ZERO_PII",
    "O_NOFOLLOW",
    "RBAC",
    "SECURITY_BYPASS",
    "PRIVACY_SCRUBBER",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestVerifyError {
    InvalidEnvelope(String),
    TamperedSignature(String),
    ExpiredManifest {
        expires_at: i64,
        now_unix: i64,
    },
    RollbackAttackDetected {
        manifest_seq: i64,
        last_seen_seq: i64,
    },
}

impl ManifestVerifyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidEnvelope(_) => "INVALID_MANIFEST_ENVELOPE",
            Self::TamperedSignature(_) => "TAMPERED_SIGNATURE",
            Self::ExpiredManifest { .. } => "EXPIRED_MANIFEST",
            Self::RollbackAttackDetected { .. } => "ROLLBACK_ATTACK_DETECTED",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidEnvelope(msg) => format!("Invalid signed manifest envelope: {msg}"),
            Self::TamperedSignature(msg) => {
                format!("Ed25519 signature verification failed (fail-closed): {msg}")
            }
            Self::ExpiredManifest {
                expires_at,
                now_unix,
            } => format!("Signed manifest has expired (expiresAt={expires_at}, now={now_unix})"),
            Self::RollbackAttackDetected {
                manifest_seq,
                last_seen_seq,
            } => format!(
                "Rollback attack detected: manifestSeq={manifest_seq} < lastSeenSeq={last_seen_seq}"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DistributionCryptoError {
    KeyStorage(String),
    Serialization(String),
    SecurityInvariantProtected(String),
}

impl DistributionCryptoError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::KeyStorage(_) => "KEY_STORAGE_ERROR",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
            Self::SecurityInvariantProtected(_) => "SECURITY_INVARIANT_PROTECTED",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::KeyStorage(msg) => msg.clone(),
            Self::Serialization(msg) => msg.clone(),
            Self::SecurityInvariantProtected(invariant) => format!(
                "A/B experiment is forbidden from targeting or relaxing security invariant '{invariant}'"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedManifestEnvelope<T> {
    pub schema_version: u32,
    pub manifest_kind: String,
    pub manifest_seq: i64,
    pub issued_at: i64,
    pub expires_at: i64,
    pub key_id: String,
    pub public_key_base64: String,
    pub payload_canonical_json: String,
    pub signature_base64: String,
    pub payload: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbVariantWeight {
    pub key: String,
    pub weight: u8,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPluginRecord {
    pub plugin_id: String,
    pub source_kind: String,
    pub lifecycle_status: String,
    pub repo_url: Option<String>,
    pub pinned_commit_sha: Option<String>,
    pub min_app_version: String,
    pub default_enabled: bool,
    pub default_config: Value,
    pub title_i18n: Value,
    pub summary_i18n: Value,
    pub manifest_seq: i64,
    pub updated_by: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAdvisoryRecord {
    pub id: String,
    pub skill_or_plugin_id: String,
    pub severity: String,
    pub action: String,
    pub reason: String,
    pub affected_sha256: Option<String>,
    pub advisory_seq: i64,
    pub issued_by: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPluginOverride {
    pub plugin_id: String,
    pub enabled: bool,
    #[serde(default)]
    pub pinned_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectivePluginState {
    pub plugin_id: String,
    pub source_kind: String,
    pub lifecycle_status: String,
    pub effective_enabled: bool,
    pub quarantine_locked: bool,
    pub resolution_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisory_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisory_severity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineLicensePayload {
    pub license_id: String,
    pub tier: String,
    pub subject_org: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cowok_org_id: Option<String>,
    pub max_seats: i64,
    pub features: Vec<String>,
    pub issued_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedOfflineLicense {
    pub payload: OfflineLicensePayload,
    pub key_id: String,
    pub public_key_base64: String,
    pub offline_verified: bool,
}

/// Recursively sorts all JSON object keys to produce a deterministic canonical representation.
pub fn canonicalize_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), canonicalize_json_value(v));
            }
            let mut out = serde_json::Map::with_capacity(sorted.len());
            for (k, v) in sorted {
                out.insert(k, v);
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json_value).collect()),
        other => other.clone(),
    }
}

pub fn to_canonical_json<T: Serialize>(payload: &T) -> Result<String, DistributionCryptoError> {
    let val = serde_json::to_value(payload)
        .map_err(|e| DistributionCryptoError::Serialization(format!("to_value failed: {e}")))?;
    let canonical = canonicalize_json_value(&val);
    serde_json::to_string(&canonical)
        .map_err(|e| DistributionCryptoError::Serialization(format!("canonical json failed: {e}")))
}

pub fn build_signing_input(
    schema_version: u32,
    key_id: &str,
    manifest_kind: &str,
    manifest_seq: i64,
    issued_at: i64,
    expires_at: i64,
    payload_canonical_json: &str,
) -> String {
    format!(
        "v{schema_version}:{key_id}:{manifest_kind}:{manifest_seq}:{issued_at}:{expires_at}:{payload_canonical_json}"
    )
}

fn ensure_no_symlink_key_dirs(workspace_root: &Path) -> Result<PathBuf, DistributionCryptoError> {
    let mut cur = workspace_root.to_path_buf();
    for segment in [".cowok-design", "keys"] {
        cur.push(segment);
        match fs::symlink_metadata(&cur) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(DistributionCryptoError::KeyStorage(format!(
                        "refusing symlink in Ed25519 key directory path: {}",
                        cur.display()
                    )));
                }
                if !meta.file_type().is_dir() {
                    return Err(DistributionCryptoError::KeyStorage(format!(
                        "Ed25519 key directory path is not a directory: {}",
                        cur.display()
                    )));
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if let Err(mk_err) = fs::create_dir(&cur) {
                    if mk_err.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(DistributionCryptoError::KeyStorage(format!(
                            "create keys directory failed: {mk_err}"
                        )));
                    }
                    let meta = fs::symlink_metadata(&cur).map_err(|e| {
                        DistributionCryptoError::KeyStorage(format!(
                            "stat keys directory failed: {e}"
                        ))
                    })?;
                    if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
                        return Err(DistributionCryptoError::KeyStorage(format!(
                            "unsafe symlink or non-directory at keys path: {}",
                            cur.display()
                        )));
                    }
                }
            }
            Err(err) => {
                return Err(DistributionCryptoError::KeyStorage(format!(
                    "inspect keys directory failed: {err}"
                )));
            }
        }
        #[cfg(unix)]
        {
            if segment == "keys" {
                let _ = fs::set_permissions(&cur, fs::Permissions::from_mode(0o700));
            }
        }
    }
    Ok(cur.join("distribution-ed25519.key"))
}

fn read_signing_key_file_nofollow(
    path: &Path,
) -> Result<Option<SigningKey>, DistributionCryptoError> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(DistributionCryptoError::KeyStorage(
                "refusing symlink at Ed25519 keyfile path".to_string(),
            ));
        }
    }
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = match opts.open(path) {
        Ok(f) => f,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(DistributionCryptoError::KeyStorage(format!(
                "open Ed25519 keyfile (O_NOFOLLOW) failed: {err}"
            )));
        }
    };
    let mut content = String::new();
    file.read_to_string(&mut content).map_err(|e| {
        DistributionCryptoError::KeyStorage(format!("read Ed25519 keyfile failed: {e}"))
    })?;
    let mut decoded = B64_STD.decode(content.trim()).map_err(|e| {
        DistributionCryptoError::KeyStorage(format!("decode Ed25519 keyfile failed: {e}"))
    })?;
    content.zeroize();
    if decoded.len() != 32 {
        decoded.zeroize();
        return Err(DistributionCryptoError::KeyStorage(
            "Persisted Ed25519 seed file has invalid length".to_string(),
        ));
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&decoded);
    decoded.zeroize();
    let key = SigningKey::from_bytes(&seed);
    seed.zeroize();
    Ok(Some(key))
}

/// Loads or initializes the workspace-scoped Ed25519 signing keypair.
/// Respects `COWOK_DISTRIBUTION_ED25519_SEED_B64` if configured; otherwise persists a random
/// 32-byte seed under `<workspace_root>/.cowok-design/keys/distribution-ed25519.key` with
/// `0600` unix permissions, parent-dir symlink rejection, and `O_NOFOLLOW` protection.
pub fn load_or_create_workspace_signing_key(
    workspace_root: &Path,
) -> Result<(SigningKey, String), DistributionCryptoError> {
    if let Ok(env_seed) = std::env::var("COWOK_DISTRIBUTION_ED25519_SEED_B64") {
        let trimmed = env_seed.trim();
        if !trimmed.is_empty() {
            let mut raw = B64_STD.decode(trimmed).map_err(|e| {
                DistributionCryptoError::KeyStorage(format!(
                    "invalid COWOK_DISTRIBUTION_ED25519_SEED_B64: {e}"
                ))
            })?;
            if raw.len() != 32 {
                raw.zeroize();
                return Err(DistributionCryptoError::KeyStorage(
                    "COWOK_DISTRIBUTION_ED25519_SEED_B64 must decode to 32 bytes".to_string(),
                ));
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&raw);
            raw.zeroize();
            let key = SigningKey::from_bytes(&seed);
            seed.zeroize();
            let key_id = std::env::var("COWOK_DISTRIBUTION_ED25519_KEY_ID")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_ED25519_KEY_ID.to_string());
            return Ok((key, key_id));
        }
    }

    let path = ensure_no_symlink_key_dirs(workspace_root)?;
    if let Some(key) = read_signing_key_file_nofollow(&path)? {
        return Ok((key, DEFAULT_ED25519_KEY_ID.to_string()));
    }

    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let mut encoded = B64_STD.encode(seed);

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }

    match opts.open(&path) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(encoded.as_bytes()) {
                encoded.zeroize();
                seed.zeroize();
                return Err(DistributionCryptoError::KeyStorage(format!(
                    "write Ed25519 keyfile failed: {e}"
                )));
            }
            let _ = file.sync_all();
            #[cfg(unix)]
            {
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
            }
            encoded.zeroize();
            let key = SigningKey::from_bytes(&seed);
            seed.zeroize();
            Ok((key, DEFAULT_ED25519_KEY_ID.to_string()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            encoded.zeroize();
            seed.zeroize();
            let key = read_signing_key_file_nofollow(&path)?.ok_or_else(|| {
                DistributionCryptoError::KeyStorage(
                    "Ed25519 keyfile disappeared after concurrent creation".to_string(),
                )
            })?;
            Ok((key, DEFAULT_ED25519_KEY_ID.to_string()))
        }
        Err(err) => {
            encoded.zeroize();
            seed.zeroize();
            Err(DistributionCryptoError::KeyStorage(format!(
                "create Ed25519 keyfile failed: {err}"
            )))
        }
    }
}

pub fn verifying_key_base64(key: &VerifyingKey) -> String {
    B64_STD.encode(key.to_bytes())
}

pub fn decode_verifying_key_base64(encoded: &str) -> Result<VerifyingKey, ManifestVerifyError> {
    let raw = B64_STD.decode(encoded.trim()).map_err(|e| {
        ManifestVerifyError::InvalidEnvelope(format!("invalid public key b64: {e}"))
    })?;
    let bytes: [u8; 32] = raw.try_into().map_err(|_| {
        ManifestVerifyError::InvalidEnvelope("public key must be 32 bytes".to_string())
    })?;
    VerifyingKey::from_bytes(&bytes).map_err(|e| {
        ManifestVerifyError::InvalidEnvelope(format!("invalid Ed25519 public key: {e}"))
    })
}

/// Signs any serializable payload into a canonical `SignedManifestEnvelope<T>`.
pub fn sign_manifest<T: Serialize>(
    signing_key: &SigningKey,
    key_id: &str,
    manifest_kind: &str,
    manifest_seq: i64,
    issued_at: i64,
    expires_at: i64,
    payload: T,
) -> Result<SignedManifestEnvelope<T>, DistributionCryptoError> {
    let payload_canonical_json = to_canonical_json(&payload)?;
    let signing_input = build_signing_input(
        SIGNED_MANIFEST_SCHEMA_VERSION,
        key_id,
        manifest_kind,
        manifest_seq,
        issued_at,
        expires_at,
        &payload_canonical_json,
    );
    let signature: Signature = signing_key.sign(signing_input.as_bytes());
    let verifying_key = signing_key.verifying_key();

    Ok(SignedManifestEnvelope {
        schema_version: SIGNED_MANIFEST_SCHEMA_VERSION,
        manifest_kind: manifest_kind.to_string(),
        manifest_seq,
        issued_at,
        expires_at,
        key_id: key_id.to_string(),
        public_key_base64: verifying_key_base64(&verifying_key),
        payload_canonical_json,
        signature_base64: B64_STD.encode(signature.to_bytes()),
        payload,
    })
}

/// Strictly fail-closed verifier for `SignedManifestEnvelope<T>`:
/// 1. Validates schema version, non-empty `key_id`, expected manifest kind, `manifest_seq >= 1`,
///    and `issued_at <= expires_at`.
/// 2. Rejects expired envelopes with strict boundary (`expires_at <= now_unix`).
/// 3. Rejects downgrade/rollback attacks (`manifest_seq < last_seen_seq`).
/// 4. Recomputes canonical payload JSON and verifies exact match with `payload_canonical_json`.
/// 5. Enforces mandatory trusted `VerifyingKey` pinning (self-signed untrusted envelopes are never accepted).
/// 6. Verifies Ed25519 signature over canonical signed bytes including `schema_version` & `key_id`.
pub fn verify_manifest_fail_closed<T: Serialize>(
    envelope: &SignedManifestEnvelope<T>,
    expected_kind: &str,
    trusted_public_key: &VerifyingKey,
    last_seen_seq: i64,
    now_unix: i64,
) -> Result<(), ManifestVerifyError> {
    if envelope.schema_version != SIGNED_MANIFEST_SCHEMA_VERSION {
        return Err(ManifestVerifyError::InvalidEnvelope(format!(
            "unsupported schemaVersion {}",
            envelope.schema_version
        )));
    }
    if envelope.key_id.trim().is_empty() {
        return Err(ManifestVerifyError::InvalidEnvelope(
            "keyId must not be empty".to_string(),
        ));
    }
    if envelope.manifest_kind != expected_kind {
        return Err(ManifestVerifyError::InvalidEnvelope(format!(
            "expected manifestKind '{}', got '{}'",
            expected_kind, envelope.manifest_kind
        )));
    }
    if envelope.manifest_seq < 1 {
        return Err(ManifestVerifyError::InvalidEnvelope(format!(
            "manifestSeq must be >= 1, got {}",
            envelope.manifest_seq
        )));
    }
    if envelope.issued_at > envelope.expires_at {
        return Err(ManifestVerifyError::InvalidEnvelope(format!(
            "issuedAt ({}) must be <= expiresAt ({})",
            envelope.issued_at, envelope.expires_at
        )));
    }
    if envelope.expires_at <= now_unix {
        return Err(ManifestVerifyError::ExpiredManifest {
            expires_at: envelope.expires_at,
            now_unix,
        });
    }
    if envelope.manifest_seq < last_seen_seq {
        return Err(ManifestVerifyError::RollbackAttackDetected {
            manifest_seq: envelope.manifest_seq,
            last_seen_seq,
        });
    }

    let recomputed_canonical = to_canonical_json(&envelope.payload).map_err(|e| {
        ManifestVerifyError::InvalidEnvelope(format!(
            "re-canonicalize payload failed: {}",
            e.message()
        ))
    })?;
    if recomputed_canonical != envelope.payload_canonical_json {
        return Err(ManifestVerifyError::TamperedSignature(
            "payload does not match signed payloadCanonicalJson".to_string(),
        ));
    }

    let envelope_pk = decode_verifying_key_base64(&envelope.public_key_base64)?;
    if trusted_public_key.to_bytes() != envelope_pk.to_bytes() {
        return Err(ManifestVerifyError::TamperedSignature(
            "manifest public key does not match trusted verifying key".to_string(),
        ));
    }

    let sig_raw = B64_STD
        .decode(envelope.signature_base64.trim())
        .map_err(|_| {
            ManifestVerifyError::TamperedSignature("invalid base64 signature encoding".to_string())
        })?;
    let sig_bytes: [u8; 64] = sig_raw.try_into().map_err(|_| {
        ManifestVerifyError::TamperedSignature("Ed25519 signature must be 64 bytes".to_string())
    })?;
    let signature = Signature::from_bytes(&sig_bytes);

    let signing_input = build_signing_input(
        envelope.schema_version,
        &envelope.key_id,
        &envelope.manifest_kind,
        envelope.manifest_seq,
        envelope.issued_at,
        envelope.expires_at,
        &envelope.payload_canonical_json,
    );

    trusted_public_key
        .verify(signing_input.as_bytes(), &signature)
        .map_err(|_| {
            ManifestVerifyError::TamperedSignature(
                "Ed25519 signature mismatch against canonical manifest".to_string(),
            )
        })
}

/// Deterministic zero-PII A/B bucketing: `SHA-256("{install_id}:{experiment_key}") % 100`.
pub fn assign_ab_bucket(install_id: &str, experiment_key: &str) -> u8 {
    let mut hasher = Sha256::new();
    hasher.update(install_id.trim().as_bytes());
    hasher.update(b":");
    hasher.update(experiment_key.trim().as_bytes());
    let digest = hasher.finalize();
    let mut head = [0u8; 8];
    head.copy_from_slice(&digest[..8]);
    (u64::from_be_bytes(head) % 100) as u8
}

/// Selects the deterministic A/B variant for `(install_id, experiment_key)` given cumulative weights.
pub fn select_ab_variant<'a>(
    install_id: &str,
    experiment_key: &str,
    variants: &'a [AbVariantWeight],
) -> Option<(&'a AbVariantWeight, u8)> {
    if variants.is_empty() {
        return None;
    }
    let bucket = assign_ab_bucket(install_id, experiment_key);
    let mut cumulative: u16 = 0;
    for variant in variants {
        cumulative = cumulative.saturating_add(u16::from(variant.weight));
        if u16::from(bucket) < cumulative {
            return Some((variant, bucket));
        }
    }
    variants.last().map(|v| (v, bucket))
}

pub fn is_valid_ascii_identifier(raw: &str) -> bool {
    (2..=80).contains(&raw.len())
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn normalize_lookalike_char(ch: char) -> Option<char> {
    if ch.is_control()
        || matches!(
            ch,
            '\u{00AD}'
                | '\u{034F}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{0300}'..='\u{036F}'
        )
    {
        return None;
    }
    let code = ch as u32;
    if (0xFF01..=0xFF5E).contains(&code) {
        if let Some(ascii) = char::from_u32(code - 0xFEE0) {
            return Some(ascii);
        }
    }
    let mapped = match ch {
        'А' | 'Α' | 'À'..='Å' => 'A',
        'а' | 'α' | 'à'..='å' => 'a',
        'В' | 'Β' => 'B',
        'β' => 'b',
        'С' | 'Ç' => 'C',
        'с' | 'ç' => 'c',
        'Е' | 'Ε' | 'È'..='Ë' => 'E',
        'е' | 'ε' | 'è'..='ë' => 'e',
        'Н' | 'Η' => 'H',
        'І' | 'Ї' | 'Ι' | 'Ì'..='Ï' => 'I',
        'і' | 'ї' | 'ι' | 'ì'..='ï' => 'i',
        'Ј' => 'J',
        'ј' => 'j',
        'К' | 'Κ' => 'K',
        'к' | 'κ' => 'k',
        'М' | 'Μ' => 'M',
        'Ν' | 'Ñ' => 'N',
        'ñ' => 'n',
        'О' | 'Ο' | 'Ò'..='Ö' => 'O',
        'о' | 'ο' | 'ò'..='ö' => 'o',
        'Р' | 'Ρ' => 'P',
        'р' | 'ρ' => 'p',
        'Ѕ' => 'S',
        'ѕ' => 's',
        'Т' | 'Τ' => 'T',
        'т' | 'τ' => 't',
        'Ù'..='Ü' => 'U',
        'ù'..='ü' => 'u',
        'Х' | 'Χ' => 'X',
        'х' | 'χ' => 'x',
        'У' | 'Υ' | 'Ý' => 'Y',
        'у' | 'υ' | 'ý' | 'ÿ' => 'y',
        'Ζ' => 'Z',
        other => other,
    };
    Some(mapped)
}

const PROTECTED_SNAKE_TOKENS: [(&str, &str); 11] = [
    ("HUMAN_ONLY", "HUMAN_ONLY"),
    ("ZERO_PII", "ZERO_PII"),
    ("O_NOFOLLOW", "O_NOFOLLOW"),
    ("NOFOLLOW", "O_NOFOLLOW"),
    ("RBAC", "RBAC"),
    ("SECURITY_BYPASS", "SECURITY_BYPASS"),
    ("PRIVACY_SCRUBBER", "PRIVACY_SCRUBBER"),
    ("PII_SCRUBBER", "PRIVACY_SCRUBBER"),
    ("DISABLE_MFA", "DISABLE_MFA"),
    ("BYPASS_AUTH", "BYPASS_AUTH"),
    ("MFA", "MFA"),
];

const PROTECTED_COMPACT_TOKENS: [(&str, &str); 11] = [
    ("HUMANONLY", "HUMAN_ONLY"),
    ("ZEROPII", "ZERO_PII"),
    ("ONOFOLLOW", "O_NOFOLLOW"),
    ("NOFOLLOW", "O_NOFOLLOW"),
    ("RBAC", "RBAC"),
    ("SECURITYBYPASS", "SECURITY_BYPASS"),
    ("PRIVACYSCRUBBER", "PRIVACY_SCRUBBER"),
    ("PIISCRUBBER", "PRIVACY_SCRUBBER"),
    ("DISABLEMFA", "DISABLE_MFA"),
    ("BYPASSAUTH", "BYPASS_AUTH"),
    ("MFA", "MFA"),
];

fn contains_protected_invariant(raw: &str) -> Option<&'static str> {
    let cleaned: Vec<char> = raw.chars().filter_map(normalize_lookalike_char).collect();

    let mut snake_upper = String::with_capacity(cleaned.len() + 8);
    let mut compact_upper = String::with_capacity(cleaned.len());

    for (idx, &ch) in cleaned.iter().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if idx > 0 && ch.is_ascii_uppercase() {
                let prev = cleaned[idx - 1];
                let next_is_lower = cleaned.get(idx + 1).is_some_and(|n| n.is_ascii_lowercase());
                if (prev.is_ascii_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_ascii_uppercase() && next_is_lower))
                    && !snake_upper.is_empty()
                    && !snake_upper.ends_with('_')
                {
                    snake_upper.push('_');
                }
            }
            let up = ch.to_ascii_uppercase();
            snake_upper.push(up);
            compact_upper.push(up);
        } else if !snake_upper.is_empty() && !snake_upper.ends_with('_') {
            snake_upper.push('_');
        }
    }
    let snake_trimmed = snake_upper.trim_end_matches('_');

    for (pattern, canonical) in PROTECTED_SNAKE_TOKENS {
        if snake_trimmed.contains(pattern) {
            return Some(canonical);
        }
    }
    for (pattern, canonical) in PROTECTED_COMPACT_TOKENS {
        if compact_upper.contains(pattern) {
            return Some(canonical);
        }
    }
    None
}

fn scan_value_for_protected_invariants(value: &Value) -> Option<&'static str> {
    match value {
        Value::String(s) => contains_protected_invariant(s),
        Value::Array(items) => items.iter().find_map(scan_value_for_protected_invariants),
        Value::Object(map) => {
            for (k, v) in map {
                if let Some(hit) = contains_protected_invariant(k) {
                    return Some(hit);
                }
                if let Some(hit) = scan_value_for_protected_invariants(v) {
                    return Some(hit);
                }
            }
            None
        }
        _ => None,
    }
}

/// Hard-blocks any A/B experiment targeting or relaxing protected security invariants.
pub fn validate_experiment_security_guard(
    experiment_key: &str,
    title: &str,
    variants_value: &Value,
) -> Result<(), DistributionCryptoError> {
    if let Some(hit) = contains_protected_invariant(experiment_key) {
        return Err(DistributionCryptoError::SecurityInvariantProtected(
            hit.to_string(),
        ));
    }
    if let Some(hit) = contains_protected_invariant(title) {
        return Err(DistributionCryptoError::SecurityInvariantProtected(
            hit.to_string(),
        ));
    }
    if let Some(hit) = scan_value_for_protected_invariants(variants_value) {
        return Err(DistributionCryptoError::SecurityInvariantProtected(
            hit.to_string(),
        ));
    }
    Ok(())
}

pub fn is_valid_lifecycle_status(status: &str) -> bool {
    matches!(
        status,
        "ACTIVE" | "INACTIVE" | "DEPRECATED" | "QUARANTINED" | "PROCESSING" | "PENDING_APPROVAL"
    )
}

/// Resolves effective plugin states strictly enforcing `Local Override > Cloud Default`,
/// EXCEPT when an active global `QUARANTINE` advisory or `QUARANTINED` lifecycle status applies
/// (in which case `effective_enabled = false` and `quarantine_locked = true`), or when a plugin
/// is still in async security evaluation (`PROCESSING` or `PENDING_APPROVAL`, in which case
/// `effective_enabled = false` with reason `"Awaiting security review / approval"`).
/// Recommended GitHub plugins are never auto-enabled without an explicit local user override.
pub fn resolve_effective_plugin_states(
    cloud_catalog: &[CatalogPluginRecord],
    cloud_advisories: &[SkillAdvisoryRecord],
    local_overrides: &[LocalPluginOverride],
) -> Vec<EffectivePluginState> {
    // Latest advisory by strictly greater (advisory_seq, created_at) per plugin_id determines active state.
    let mut latest_advisory_by_id: HashMap<&str, &SkillAdvisoryRecord> = HashMap::new();
    for adv in cloud_advisories {
        latest_advisory_by_id
            .entry(adv.skill_or_plugin_id.as_str())
            .and_modify(|existing| {
                if (adv.advisory_seq, adv.created_at) > (existing.advisory_seq, existing.created_at)
                {
                    *existing = adv;
                }
            })
            .or_insert(adv);
    }

    let mut overrides_by_id: HashMap<&str, &LocalPluginOverride> = HashMap::new();
    for ov in local_overrides {
        overrides_by_id.insert(ov.plugin_id.as_str(), ov);
    }

    let mut seen = HashMap::new();
    let mut resolved = Vec::with_capacity(cloud_catalog.len() + local_overrides.len());

    for entry in cloud_catalog {
        seen.insert(entry.plugin_id.as_str(), true);
        let active_adv = latest_advisory_by_id.get(entry.plugin_id.as_str()).copied();
        let quarantine_adv = active_adv.filter(|a| a.action == "QUARANTINE");
        let quarantined_by_advisory = quarantine_adv.is_some();
        let quarantined_by_status = entry.lifecycle_status == "QUARANTINED";

        if quarantined_by_advisory || quarantined_by_status {
            resolved.push(EffectivePluginState {
                plugin_id: entry.plugin_id.clone(),
                source_kind: entry.source_kind.clone(),
                lifecycle_status: "QUARANTINED".to_string(),
                effective_enabled: false,
                quarantine_locked: true,
                resolution_source: "GLOBAL_QUARANTINE".to_string(),
                advisory_reason: quarantine_adv
                    .map(|a| a.reason.clone())
                    .or_else(|| Some("Global security quarantine active".to_string())),
                advisory_severity: quarantine_adv
                    .map(|a| a.severity.clone())
                    .or_else(|| Some("CRITICAL".to_string())),
            });
            continue;
        }

        if matches!(
            entry.lifecycle_status.as_str(),
            "PROCESSING" | "PENDING_APPROVAL"
        ) {
            resolved.push(EffectivePluginState {
                plugin_id: entry.plugin_id.clone(),
                source_kind: entry.source_kind.clone(),
                lifecycle_status: entry.lifecycle_status.clone(),
                effective_enabled: false,
                quarantine_locked: false,
                resolution_source: "SECURITY_REVIEW_PENDING".to_string(),
                advisory_reason: Some("Awaiting security review / approval".to_string()),
                advisory_severity: None,
            });
            continue;
        }

        let warn_reason = active_adv
            .filter(|a| a.action == "WARN")
            .map(|a| a.reason.clone());
        let warn_severity = active_adv
            .filter(|a| a.action == "WARN")
            .map(|a| a.severity.clone());

        if let Some(local) = overrides_by_id.get(entry.plugin_id.as_str()) {
            resolved.push(EffectivePluginState {
                plugin_id: entry.plugin_id.clone(),
                source_kind: entry.source_kind.clone(),
                lifecycle_status: entry.lifecycle_status.clone(),
                effective_enabled: local.enabled,
                quarantine_locked: false,
                resolution_source: "LOCAL_OVERRIDE".to_string(),
                advisory_reason: warn_reason,
                advisory_severity: warn_severity,
            });
            continue;
        }

        // Never silently install/enable RECOMMENDED_GITHUB plugins without explicit local override.
        let cloud_default_enabled = if entry.source_kind == "RECOMMENDED_GITHUB" {
            false
        } else {
            entry.default_enabled && entry.lifecycle_status == "ACTIVE"
        };

        resolved.push(EffectivePluginState {
            plugin_id: entry.plugin_id.clone(),
            source_kind: entry.source_kind.clone(),
            lifecycle_status: entry.lifecycle_status.clone(),
            effective_enabled: cloud_default_enabled,
            quarantine_locked: false,
            resolution_source: "CLOUD_DEFAULT".to_string(),
            advisory_reason: warn_reason,
            advisory_severity: warn_severity,
        });
    }

    // Preserve local-only plugins while still enforcing global quarantine advisories against them.
    for local in local_overrides {
        if seen.contains_key(local.plugin_id.as_str()) {
            continue;
        }
        let active_adv = latest_advisory_by_id.get(local.plugin_id.as_str()).copied();
        if let Some(adv) = active_adv.filter(|a| a.action == "QUARANTINE") {
            resolved.push(EffectivePluginState {
                plugin_id: local.plugin_id.clone(),
                source_kind: "LOCAL_CUSTOM".to_string(),
                lifecycle_status: "QUARANTINED".to_string(),
                effective_enabled: false,
                quarantine_locked: true,
                resolution_source: "GLOBAL_QUARANTINE".to_string(),
                advisory_reason: Some(adv.reason.clone()),
                advisory_severity: Some(adv.severity.clone()),
            });
        } else {
            resolved.push(EffectivePluginState {
                plugin_id: local.plugin_id.clone(),
                source_kind: "LOCAL_CUSTOM".to_string(),
                lifecycle_status: "ACTIVE".to_string(),
                effective_enabled: local.enabled,
                quarantine_locked: false,
                resolution_source: "LOCAL_OVERRIDE".to_string(),
                advisory_reason: active_adv
                    .filter(|a| a.action == "WARN")
                    .map(|a| a.reason.clone()),
                advisory_severity: active_adv
                    .filter(|a| a.action == "WARN")
                    .map(|a| a.severity.clone()),
            });
        }
    }

    resolved
}

pub fn default_features_for_tier(tier: &str) -> Vec<String> {
    match tier {
        "ENTERPRISE" => vec![
            "desktop-pro".to_string(),
            "docker-enterprise".to_string(),
            "cowok-org-federation".to_string(),
            "offline-airgap".to_string(),
            "custom-sso".to_string(),
        ],
        "TEAM" => vec![
            "desktop-pro".to_string(),
            "team-workspaces".to_string(),
            "cowok-org-federation".to_string(),
            "offline-airgap".to_string(),
        ],
        "PRO" => vec![
            "desktop-pro".to_string(),
            "offline-airgap".to_string(),
            "priority-templates".to_string(),
        ],
        _ => vec!["community-core".to_string()],
    }
}

/// Signs an `OfflineLicensePayload` into a compact, 100% offline-verifiable Ed25519 token.
pub fn sign_offline_license(
    signing_key: &SigningKey,
    key_id: &str,
    payload: &OfflineLicensePayload,
) -> Result<String, DistributionCryptoError> {
    let envelope = sign_manifest(
        signing_key,
        key_id,
        "license",
        1,
        payload.issued_at,
        payload.expires_at,
        payload.clone(),
    )?;
    let bytes = serde_json::to_vec(&envelope).map_err(|e| {
        DistributionCryptoError::Serialization(format!("serialize license envelope failed: {e}"))
    })?;
    Ok(format!(
        "{OFFLINE_LICENSE_TOKEN_PREFIX}.{}",
        B64_URL.encode(&bytes)
    ))
}

/// Verifies an offline Ed25519 license token 100% locally without network I/O (fail-closed).
pub fn verify_offline_license(
    token: &str,
    trusted_public_key: &VerifyingKey,
    now_unix: i64,
) -> Result<VerifiedOfflineLicense, ManifestVerifyError> {
    let trimmed = token.trim();
    let rest = trimmed
        .strip_prefix(&format!("{OFFLINE_LICENSE_TOKEN_PREFIX}."))
        .ok_or_else(|| {
            ManifestVerifyError::InvalidEnvelope("missing cwk_lic_v1 token prefix".to_string())
        })?;
    let bytes = B64_URL.decode(rest).map_err(|_| {
        ManifestVerifyError::TamperedSignature("invalid base64url license encoding".to_string())
    })?;
    let envelope: SignedManifestEnvelope<OfflineLicensePayload> = serde_json::from_slice(&bytes)
        .map_err(|e| {
            ManifestVerifyError::InvalidEnvelope(format!("invalid license JSON envelope: {e}"))
        })?;

    verify_manifest_fail_closed(&envelope, "license", trusted_public_key, 1, now_unix)?;
    if envelope.payload.issued_at > envelope.payload.expires_at {
        return Err(ManifestVerifyError::InvalidEnvelope(format!(
            "license payload issuedAt ({}) must be <= expiresAt ({})",
            envelope.payload.issued_at, envelope.payload.expires_at
        )));
    }
    if envelope.payload.expires_at <= now_unix {
        return Err(ManifestVerifyError::ExpiredManifest {
            expires_at: envelope.payload.expires_at,
            now_unix,
        });
    }

    Ok(VerifiedOfflineLicense {
        payload: envelope.payload,
        key_id: envelope.key_id,
        public_key_base64: envelope.public_key_base64,
        offline_verified: true,
    })
}

/// Helper for parsing a raw JSON envelope into `SignedManifestEnvelope<T>` and verifying fail-closed.
pub fn parse_and_verify_manifest_json<T: Serialize + DeserializeOwned>(
    raw_json: &str,
    expected_kind: &str,
    trusted_public_key: &VerifyingKey,
    last_seen_seq: i64,
    now_unix: i64,
) -> Result<SignedManifestEnvelope<T>, ManifestVerifyError> {
    let envelope: SignedManifestEnvelope<T> = serde_json::from_str(raw_json).map_err(|e| {
        ManifestVerifyError::InvalidEnvelope(format!("malformed manifest JSON: {e}"))
    })?;
    verify_manifest_fail_closed(
        &envelope,
        expected_kind,
        trusted_public_key,
        last_seen_seq,
        now_unix,
    )?;
    Ok(envelope)
}
