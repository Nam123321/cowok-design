// @story 04.3
// @implements AC-03 (OS Keychain + Argon2id + AES-256-GCM 0o600 Vault Custody & Anti-Downgrade)
use std::collections::{BTreeMap, HashMap};
use std::ffi::CString;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::io::{FromRawFd, OwnedFd};

fn learn_ring_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn machine_secret_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncryptedVaultEnvelope {
    version: u8,
    kdf: String,
    cipher: String,
    salt_b64: String,
    nonce_b64: String,
    ciphertext_b64: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultPayload {
    pub secrets: BTreeMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedLearnKeyEntry {
    kid: String,
    key_b64: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedLearnKeyRing {
    active_kid: String,
    active_key_b64: String,
    #[serde(default)]
    retired: Vec<PersistedLearnKeyEntry>,
}

#[derive(Clone)]
pub struct LearnSigningKeyRing {
    pub active_kid: String,
    pub active_key: [u8; 32],
    pub retired: Vec<(String, [u8; 32])>,
}

impl std::fmt::Debug for LearnSigningKeyRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LearnSigningKeyRing")
            .field("active_kid", &self.active_kid)
            .field("retired_count", &self.retired.len())
            .finish()
    }
}

impl Drop for LearnSigningKeyRing {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.active_key);
        for (_, k) in &mut self.retired {
            zeroize::Zeroize::zeroize(k);
        }
    }
}

impl PersistedLearnKeyRing {
    fn into_runtime(self) -> Result<LearnSigningKeyRing, String> {
        let decode_32 = |raw: &str| -> Result<[u8; 32], String> {
            let bytes = B64
                .decode(raw.trim())
                .map_err(|e| format!("Invalid LEARN key base64: {e}"))?;
            if bytes.len() != 32 {
                return Err("LEARN key must be 32 bytes".to_string());
            }
            let mut out = [0u8; 32];
            out.copy_from_slice(&bytes);
            Ok(out)
        };
        let active_key = decode_32(&self.active_key_b64)?;
        let mut retired = Vec::with_capacity(self.retired.len());
        for entry in self.retired {
            retired.push((entry.kid, decode_32(&entry.key_b64)?));
        }
        Ok(LearnSigningKeyRing {
            active_kid: self.active_kid,
            active_key,
            retired,
        })
    }
}

type RamVaultMap = HashMap<(String, String), Zeroizing<String>>;

#[derive(Debug, Clone, Default)]
pub struct RamVaultStore {
    inner: Arc<RwLock<RamVaultMap>>,
}

impl RamVaultStore {
    pub fn upsert(&self, tenant_id: &str, provider_id: &str, secret: &str) {
        if let Ok(mut guard) = self.inner.write() {
            guard.insert(
                (tenant_id.to_string(), provider_id.to_ascii_lowercase()),
                Zeroizing::new(secret.to_string()),
            );
        }
    }

    pub fn remove(&self, tenant_id: &str, provider_id: &str) -> bool {
        if let Ok(mut guard) = self.inner.write() {
            return guard
                .remove(&(tenant_id.to_string(), provider_id.to_ascii_lowercase()))
                .is_some();
        }
        false
    }

    pub fn contains(&self, tenant_id: &str, provider_id: &str) -> bool {
        self.inner
            .read()
            .map(|guard| {
                guard.contains_key(&(tenant_id.to_string(), provider_id.to_ascii_lowercase()))
            })
            .unwrap_or(false)
    }

    pub fn clear_tenant(&self, tenant_id: &str) {
        if let Ok(mut guard) = self.inner.write() {
            guard.retain(|(tid, _), _| tid != tenant_id);
        }
    }

    pub fn list_tenant_secrets(&self, tenant_id: &str) -> Vec<(String, Zeroizing<String>)> {
        self.inner
            .read()
            .map(|guard| {
                guard
                    .iter()
                    .filter(|((tid, _), _)| tid == tenant_id)
                    .map(|((_, provider), secret)| (provider.clone(), secret.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub struct VaultStore {
    cowok_dir: PathBuf,
    vault_path: PathBuf,
    master_key_path: PathBuf,
}

impl VaultStore {
    pub fn from_workspace_root(workspace_root: &Path) -> Self {
        let cowok_dir = workspace_root.join(".cowok-design");
        let ws_canon =
            fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
        let mut hasher = Sha256::new();
        hasher.update(ws_canon.to_string_lossy().as_bytes());
        let digest = hasher.finalize();
        let ws_hash: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();

        // Keep key custody OUTSIDE workspace `.cowok-design/` so repository/workspace backups never co-bundle key + ciphertext
        let key_dir = std::env::var_os("COWOK_VAULT_KEY_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|v| !v.is_empty())
                    .map(|h| PathBuf::from(h).join(".config/cowok-design/keys"))
            })
            .unwrap_or_else(|| std::env::temp_dir().join("cowok-design-keys"));

        Self {
            vault_path: cowok_dir.join("vault.enc"),
            master_key_path: key_dir.join(format!("{ws_hash}.key")),
            cowok_dir,
        }
    }

    pub fn vault_path(&self) -> &Path {
        &self.vault_path
    }

    pub fn master_key_path(&self) -> &Path {
        &self.master_key_path
    }

    /// Pure read-only check: returns true if `.cowok-design/vault.enc` (or existing parent dir) is writable
    /// without mutating filesystem state (`create_dir_all` forbidden in check).
    pub fn is_vault_writable(&self) -> bool {
        if let Ok(meta) = fs::symlink_metadata(&self.vault_path) {
            if meta.file_type().is_symlink() {
                return false;
            }
            return !meta.permissions().readonly();
        }
        let check_dir = if self.cowok_dir.exists() {
            if fs::symlink_metadata(&self.cowok_dir)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(true)
            {
                return false;
            }
            &self.cowok_dir
        } else if let Some(parent) = self.cowok_dir.parent() {
            if !parent.exists()
                || fs::symlink_metadata(parent)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(true)
            {
                return false;
            }
            parent
        } else {
            return false;
        };

        #[cfg(unix)]
        {
            if let Ok(meta) = fs::symlink_metadata(check_dir) {
                let mode = meta.permissions().mode();
                return (mode & 0o200) != 0 && !meta.permissions().readonly();
            }
            false
        }
        #[cfg(not(unix))]
        {
            fs::metadata(check_dir)
                .map(|m| !m.permissions().readonly())
                .unwrap_or(false)
        }
    }

    fn ensure_cowok_dir(&self) -> Result<(), String> {
        if let Ok(meta) = fs::symlink_metadata(&self.cowok_dir) {
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return Err(
                    "Security violation: .cowok-design is a symlink or non-directory".into(),
                );
            }
        }
        fs::create_dir_all(&self.cowok_dir)
            .map_err(|e| format!("Failed creating .cowok-design dir: {e}"))?;
        let meta = fs::symlink_metadata(&self.cowok_dir)
            .map_err(|e| format!("Failed inspecting .cowok-design dir: {e}"))?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err("Security violation: .cowok-design became a symlink".into());
        }
        #[cfg(unix)]
        {
            fs::set_permissions(&self.cowok_dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("Failed enforcing 0700 on .cowok-design: {e}"))?;
        }
        Ok(())
    }

    #[cfg(unix)]
    fn open_parent_dir_nofollow(parent: &Path) -> Result<OwnedFd, String> {
        if let Ok(meta) = fs::symlink_metadata(parent) {
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return Err(format!(
                    "Security violation: vault parent '{}' is a symlink or non-directory",
                    parent.display()
                ));
            }
        }
        let c_parent = CString::new(parent.as_os_str().as_bytes())
            .map_err(|_| "Invalid NUL byte in vault parent path".to_string())?;
        let fd = unsafe {
            libc::open(
                c_parent.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(format!(
                "Failed opening vault parent dir '{}' with O_NOFOLLOW: {}",
                parent.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    fn write_atomic_0600(target: &Path, bytes: &[u8]) -> Result<(), String> {
        let parent = target
            .parent()
            .ok_or_else(|| "Vault target path missing parent dir".to_string())?;
        fs::create_dir_all(parent).map_err(|e| format!("Failed creating parent vault dir: {e}"))?;
        #[cfg(unix)]
        {
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }

        let target_name = target
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .ok_or_else(|| "Vault target path missing filename".to_string())?;
        let tmp_name = format!(".{target_name}.tmp.{}", Uuid::new_v4());
        let tmp_path = parent.join(&tmp_name);

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            if let Ok(meta) = fs::symlink_metadata(target) {
                if meta.file_type().is_symlink() {
                    return Err(format!(
                        "Security violation: target vault file '{}' is a symlink",
                        target.display()
                    ));
                }
            }
            let dir_fd = Self::open_parent_dir_nofollow(parent)?;
            let c_tmp = CString::new(tmp_name.as_bytes())
                .map_err(|_| "Invalid NUL byte in temp filename".to_string())?;
            let c_target = CString::new(target_name.as_bytes())
                .map_err(|_| "Invalid NUL byte in target filename".to_string())?;
            let fd = unsafe {
                libc::openat(
                    dir_fd.as_raw_fd(),
                    c_tmp.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600 as libc::c_uint,
                )
            };
            if fd < 0 {
                return Err(format!(
                    "Failed creating 0o600 vault temp file via O_NOFOLLOW: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let mut file = unsafe { File::from_raw_fd(fd) };
            if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
                drop(file);
                unsafe {
                    libc::unlinkat(dir_fd.as_raw_fd(), c_tmp.as_ptr(), 0);
                }
                return Err(format!("Failed writing/syncing temp vault file: {e}"));
            }
            drop(file);
            if unsafe {
                libc::renameat(
                    dir_fd.as_raw_fd(),
                    c_tmp.as_ptr(),
                    dir_fd.as_raw_fd(),
                    c_target.as_ptr(),
                )
            } != 0
            {
                let err = std::io::Error::last_os_error();
                unsafe {
                    libc::unlinkat(dir_fd.as_raw_fd(), c_tmp.as_ptr(), 0);
                }
                return Err(format!("Failed atomic renameat of vault file: {err}"));
            }
            fs::set_permissions(target, fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("Failed enforcing 0o600 on vault file: {e}"))?;
        }
        #[cfg(not(unix))]
        {
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp_path)
                .map_err(|e| format!("Failed opening temp vault file: {e}"))?;
            if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
                let _ = fs::remove_file(&tmp_path);
                return Err(format!("Failed writing/syncing temp vault file: {e}"));
            }
            drop(f);
            fs::rename(&tmp_path, target).map_err(|e| {
                let _ = fs::remove_file(&tmp_path);
                format!("Failed atomic rename of vault file: {e}")
            })?;
        }
        let _ = tmp_path;
        Ok(())
    }

    pub(crate) fn scoped_key(tenant_id: &str, provider_id: &str) -> String {
        let tid = tenant_id.trim();
        let pid = provider_id.trim().to_ascii_lowercase();
        format!("tenant::{tid}::{pid}")
    }

    fn workbench_secret_path(&self) -> PathBuf {
        let state_root = std::env::var_os("COWOK_WORKBENCH_SECRET_DIR")
            .or_else(|| std::env::var_os("XDG_STATE_HOME"))
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|v| !v.is_empty())
                    .map(|h| PathBuf::from(h).join(".local").join("state"))
            })
            .unwrap_or_else(|| self.cowok_dir.join("state-fallback"));
        let mut hasher = Sha256::new();
        hasher.update(self.cowok_dir.to_string_lossy().as_bytes());
        let ws_hash = format!("{:x}", hasher.finalize());
        state_root
            .join("cowok-design")
            .join("workbench-secrets")
            .join(format!("{ws_hash}.secret"))
    }

    fn load_or_init_machine_secret(&self) -> Result<Zeroizing<Vec<u8>>, String> {
        let _guard = machine_secret_mutex()
            .lock()
            .map_err(|_| "Vault machine secret mutex poisoned".to_string())?;
        #[cfg(test)]
        if let Ok(test_key) = std::env::var("COWOK_VAULT_TEST_KEY") {
            let trimmed = test_key.trim();
            if !trimmed.is_empty() {
                return Ok(Zeroizing::new(trimmed.as_bytes().to_vec()));
            }
        }

        if self.master_key_path.exists() {
            let meta = fs::symlink_metadata(&self.master_key_path)
                .map_err(|e| format!("Failed checking vault key metadata: {e}"))?;
            if meta.file_type().is_symlink() || !meta.is_file() {
                return Err(
                    "Security violation: vault master key path is a symlink or non-file"
                        .to_string(),
                );
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if meta.nlink() != 1 {
                    return Err(
                        "Security violation: vault master key path is a hardlink".to_string()
                    );
                }
            }
            let raw = Zeroizing::new(
                fs::read_to_string(&self.master_key_path)
                    .map_err(|e| format!("Failed reading vault master key: {e}"))?,
            );
            let decoded = Zeroizing::new(
                B64.decode(raw.trim())
                    .map_err(|e| format!("Corrupted vault master key base64: {e}"))?,
            );
            if decoded.len() < 32 {
                return Err("Corrupted vault master key length < 32 bytes".to_string());
            }
            return Ok(decoded);
        }

        let mut secret = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(&mut *secret);
        let encoded = Zeroizing::new(B64.encode(*secret));
        Self::write_atomic_0600(&self.master_key_path, encoded.as_bytes())?;
        Ok(Zeroizing::new(secret.to_vec()))
    }

    fn derive_aes_key(ikm: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
        let mut derived = Zeroizing::new([0u8; 32]);
        // Argon2id with explicit moderate cost parameters suitable for fast daemon calls
        let params = argon2::Params::new(16 * 1024, 2, 1, Some(32))
            .map_err(|e| format!("Argon2id params error: {e}"))?;
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        argon2
            .hash_password_into(ikm, salt, &mut *derived)
            .map_err(|e| format!("Argon2id derivation error: {e}"))?;
        Ok(derived)
    }

    pub fn seal_secrets(
        &self,
        secrets: &BTreeMap<String, String>,
        master_passphrase: Option<&str>,
    ) -> Result<(), String> {
        self.ensure_cowok_dir()?;
        let ikm: Zeroizing<Vec<u8>> =
            match master_passphrase.map(str::trim).filter(|s| !s.is_empty()) {
                Some(pass) => Zeroizing::new(pass.as_bytes().to_vec()),
                None => self.load_or_init_machine_secret()?,
            };

        let mut salt = [0u8; 16];
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce_bytes);

        let key_bytes = Self::derive_aes_key(&ikm, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&*key_bytes)
            .map_err(|e| format!("AES-256-GCM init error: {e}"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let payload = VaultPayload {
            secrets: secrets.clone(),
        };
        let plaintext = Zeroizing::new(
            serde_json::to_vec(&payload)
                .map_err(|e| format!("Failed serializing vault payload: {e}"))?,
        );
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_slice())
            .map_err(|e| format!("AES-256-GCM encryption failed: {e}"))?;

        let env = EncryptedVaultEnvelope {
            version: 1,
            kdf: "argon2id".to_string(),
            cipher: "aes-256-gcm".to_string(),
            salt_b64: B64.encode(salt),
            nonce_b64: B64.encode(nonce_bytes),
            ciphertext_b64: B64.encode(ciphertext),
        };
        let encoded_env = serde_json::to_vec_pretty(&env)
            .map_err(|e| format!("Failed serializing encrypted vault envelope: {e}"))?;

        Self::write_atomic_0600(&self.vault_path, &encoded_env)
    }

    pub fn unseal_secrets(
        &self,
        master_passphrase: Option<&str>,
    ) -> Result<BTreeMap<String, String>, String> {
        if !self.vault_path.exists() {
            return Ok(BTreeMap::new());
        }
        let meta = fs::symlink_metadata(&self.vault_path)
            .map_err(|e| format!("Failed inspecting vault.enc metadata: {e}"))?;
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err(
                "Security violation: .cowok-design/vault.enc is a symlink or non-file".to_string(),
            );
        }

        let raw =
            fs::read(&self.vault_path).map_err(|e| format!("Failed reading vault.enc: {e}"))?;
        let env: EncryptedVaultEnvelope = serde_json::from_slice(&raw)
            .map_err(|e| format!("Invalid vault.enc JSON envelope: {e}"))?;
        if env.version != 1 || env.kdf != "argon2id" || env.cipher != "aes-256-gcm" {
            return Err("Unsupported vault.enc envelope format".to_string());
        }

        let salt = B64
            .decode(env.salt_b64.trim())
            .map_err(|e| format!("Invalid vault.enc salt: {e}"))?;
        let nonce_bytes = B64
            .decode(env.nonce_b64.trim())
            .map_err(|e| format!("Invalid vault.enc nonce: {e}"))?;
        if nonce_bytes.len() != 12 {
            return Err("Invalid vault.enc nonce length".to_string());
        }
        let ciphertext = B64
            .decode(env.ciphertext_b64.trim())
            .map_err(|e| format!("Invalid vault.enc ciphertext: {e}"))?;

        let ikm: Zeroizing<Vec<u8>> =
            match master_passphrase.map(str::trim).filter(|s| !s.is_empty()) {
                Some(pass) => Zeroizing::new(pass.as_bytes().to_vec()),
                None => self.load_or_init_machine_secret()?,
            };
        let key_bytes = Self::derive_aes_key(&ikm, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&*key_bytes)
            .map_err(|e| format!("AES-256-GCM init error: {e}"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext =
            Zeroizing::new(cipher.decrypt(nonce, ciphertext.as_slice()).map_err(|_| {
                "Vault decryption failed (invalid master key or tampered ciphertext)".to_string()
            })?);
        let payload: VaultPayload = serde_json::from_slice(&plaintext)
            .map_err(|e| format!("Corrupted decrypted vault payload: {e}"))?;
        Ok(payload.secrets)
    }

    pub fn upsert_secret(
        &self,
        provider_id: &str,
        secret: &str,
        master_passphrase: Option<&str>,
    ) -> Result<String, String> {
        self.upsert_secret_scoped("local-default", provider_id, secret, master_passphrase)
    }

    pub fn upsert_secret_scoped(
        &self,
        tenant_id: &str,
        provider_id: &str,
        secret: &str,
        master_passphrase: Option<&str>,
    ) -> Result<String, String> {
        let _guard = learn_ring_mutex()
            .lock()
            .map_err(|_| "Vault upsert mutex poisoned".to_string())?;
        let mut secrets = if self.vault_path.exists() {
            self.unseal_secrets(master_passphrase)?
        } else {
            BTreeMap::new()
        };
        let pid = provider_id.trim().to_ascii_lowercase();
        let scoped = Self::scoped_key(tenant_id, &pid);
        secrets.insert(scoped.clone(), secret.trim().to_string());
        self.seal_secrets(&secrets, master_passphrase)?;

        if std::env::var("COWOK_OS_KEYCHAIN").as_deref() == Ok("1") {
            let _ = self.try_store_os_keychain(&scoped, secret.trim());
        }
        if tenant_id.trim() == "local-default" {
            Ok(format!("vault://vault_0600/{pid}"))
        } else {
            Ok(format!("vault://vault_0600/{tenant_id}/{pid}"))
        }
    }

    pub fn contains_secret_scoped(
        &self,
        tenant_id: &str,
        provider_id: &str,
        master_passphrase: Option<&str>,
    ) -> Result<bool, String> {
        let _guard = learn_ring_mutex()
            .lock()
            .map_err(|_| "Vault read mutex poisoned".to_string())?;
        if !self.vault_path.exists() {
            return Ok(false);
        }
        let pid = provider_id.trim().to_ascii_lowercase();
        let scoped = Self::scoped_key(tenant_id, &pid);
        let secrets = self.unseal_secrets(master_passphrase)?;
        Ok(secrets.contains_key(&scoped))
    }

    pub fn delete_secret(
        &self,
        provider_id: &str,
        master_passphrase: Option<&str>,
    ) -> Result<(), String> {
        self.delete_secret_scoped("local-default", provider_id, master_passphrase)
    }

    pub fn delete_secret_scoped(
        &self,
        tenant_id: &str,
        provider_id: &str,
        master_passphrase: Option<&str>,
    ) -> Result<(), String> {
        let _guard = learn_ring_mutex()
            .lock()
            .map_err(|_| "Vault delete mutex poisoned".to_string())?;
        let pid = provider_id.trim().to_ascii_lowercase();
        let scoped = Self::scoped_key(tenant_id, &pid);
        if self.vault_path.exists() {
            let mut secrets = self.unseal_secrets(master_passphrase)?;
            let mut removed = secrets.remove(&scoped).is_some();
            if tenant_id == "local-default" {
                removed |= secrets.remove(&pid).is_some();
            }
            if removed {
                self.seal_secrets(&secrets, master_passphrase)?;
            }
        }
        if std::env::var("COWOK_OS_KEYCHAIN").as_deref() == Ok("1") {
            let _ = self.try_delete_os_keychain(&scoped);
            if tenant_id == "local-default" {
                let _ = self.try_delete_os_keychain(&pid);
            }
        }
        Ok(())
    }

    /// Loads or initializes the Vault `0o600` LEARN HMAC signing key ring (`active_kid` + retired `kid` entries)
    /// so `HumanSessionAuthority::from_key_with_kid` & `LearnPipeline::register_authority` survive restarts and rotations.
    pub fn ensure_or_load_learn_signing_ring(&self) -> Result<LearnSigningKeyRing, String> {
        const RING_KEY: &str = "cowok_learn_hmac_ring_v1";
        let _guard = learn_ring_mutex()
            .lock()
            .map_err(|_| "LEARN signing ring mutex poisoned".to_string())?;
        let secrets = if self.vault_path.exists() {
            self.unseal_secrets(None)?
        } else {
            BTreeMap::new()
        };
        if let Some(raw_json) = secrets.get(RING_KEY) {
            if let Ok(parsed) = serde_json::from_str::<PersistedLearnKeyRing>(raw_json) {
                return parsed.into_runtime();
            }
        }
        let mut active_key = [0u8; 32];
        OsRng.fill_bytes(&mut active_key);
        let active_kid = format!("kid-{}", Uuid::new_v4().simple());
        let persisted = PersistedLearnKeyRing {
            active_kid: active_kid.clone(),
            active_key_b64: B64.encode(active_key),
            retired: Vec::new(),
        };
        let mut next = secrets;
        next.insert(
            RING_KEY.to_string(),
            serde_json::to_string(&persisted).map_err(|e| e.to_string())?,
        );
        self.seal_secrets(&next, None)?;
        Ok(LearnSigningKeyRing {
            active_kid,
            active_key,
            retired: Vec::new(),
        })
    }

    /// Reads an already-provisioned interactive workbench secret from outside `.cowok-design/` (`0o600`, non-symlink).
    pub fn read_existing_workbench_human_secret(
        &self,
    ) -> Result<Option<Zeroizing<String>>, String> {
        let _guard = learn_ring_mutex()
            .lock()
            .map_err(|_| "Workbench secret mutex poisoned".to_string())?;
        let secret_path = self.workbench_secret_path();
        if !secret_path.exists() {
            return Ok(None);
        }
        let meta = fs::symlink_metadata(&secret_path)
            .map_err(|e| format!("Failed checking workbench secret metadata: {e}"))?;
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err(
                "Security violation: workbench secret is a symlink or non-file".to_string(),
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() != 1 {
                return Err("Security violation: workbench secret is a hardlink".to_string());
            }
        }
        let raw = Zeroizing::new(
            fs::read_to_string(&secret_path)
                .map_err(|e| format!("Failed reading workbench secret: {e}"))?,
        );
        let trimmed = raw.trim();
        if trimmed.len() >= 32 {
            return Ok(Some(Zeroizing::new(trimmed.to_string())));
        }
        Ok(None)
    }

    /// Loads or initializes the out-of-workspace `0o600` interactive workbench secret
    /// under XDG/HOME user-private state (outside `.cowok-design/` next to `daemon.lock`).
    pub fn load_or_init_workbench_human_secret(&self) -> Result<Zeroizing<String>, String> {
        if let Some(existing) = self.read_existing_workbench_human_secret()? {
            return Ok(existing);
        }
        let _guard = learn_ring_mutex()
            .lock()
            .map_err(|_| "Workbench secret mutex poisoned".to_string())?;
        let secret_path = self.workbench_secret_path();
        if let Some(parent) = secret_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed creating workbench secret dir: {e}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
                    .map_err(|e| format!("Failed enforcing 0o700 on workbench secret dir: {e}"))?;
            }
        }
        let mut bytes = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(&mut *bytes);
        let hex_secret: Zeroizing<String> =
            Zeroizing::new(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
        Self::write_atomic_0600(&secret_path, hex_secret.as_bytes())?;
        Ok(hex_secret)
    }

    /// Rotates the active LEARN HMAC signing key while preserving retired keys so prior card signatures remain valid.
    pub fn rotate_learn_signing_key(&self) -> Result<LearnSigningKeyRing, String> {
        const RING_KEY: &str = "cowok_learn_hmac_ring_v1";
        let _guard = learn_ring_mutex()
            .lock()
            .map_err(|_| "LEARN signing ring mutex poisoned".to_string())?;
        let current = {
            let secrets = if self.vault_path.exists() {
                self.unseal_secrets(None)?
            } else {
                BTreeMap::new()
            };
            if let Some(raw_json) = secrets.get(RING_KEY) {
                serde_json::from_str::<PersistedLearnKeyRing>(raw_json)
                    .map_err(|e| e.to_string())?
                    .into_runtime()?
            } else {
                drop(_guard);
                return self.ensure_or_load_learn_signing_ring();
            }
        };
        let mut retired_persisted: Vec<PersistedLearnKeyEntry> = current
            .retired
            .iter()
            .map(|(kid, key)| PersistedLearnKeyEntry {
                kid: kid.clone(),
                key_b64: B64.encode(key),
            })
            .collect();
        retired_persisted.push(PersistedLearnKeyEntry {
            kid: current.active_kid.clone(),
            key_b64: B64.encode(current.active_key),
        });
        if retired_persisted.len() > 8 {
            let trim = retired_persisted.len() - 8;
            retired_persisted.drain(0..trim);
        }
        let mut next_key = [0u8; 32];
        OsRng.fill_bytes(&mut next_key);
        let next_kid = format!("kid-{}", Uuid::new_v4().simple());
        let persisted = PersistedLearnKeyRing {
            active_kid: next_kid,
            active_key_b64: B64.encode(next_key),
            retired: retired_persisted,
        };
        let mut secrets = if self.vault_path.exists() {
            self.unseal_secrets(None)?
        } else {
            BTreeMap::new()
        };
        secrets.insert(
            RING_KEY.to_string(),
            serde_json::to_string(&persisted).map_err(|e| e.to_string())?,
        );
        self.seal_secrets(&secrets, None)?;
        persisted.into_runtime()
    }

    /// Loads optional GitHub PAT scoped to `tenant_id` (`"github_plugins"` or `"github"`) inside Vault `0o600`.
    pub fn get_github_sync_token(&self) -> Result<Option<Zeroizing<String>>, String> {
        self.get_github_sync_token_scoped("local-default")
    }

    pub fn get_github_sync_token_scoped(
        &self,
        tenant_id: &str,
    ) -> Result<Option<Zeroizing<String>>, String> {
        if !self.vault_path.exists() {
            return Ok(None);
        }
        let secrets = self.unseal_secrets(None)?;
        for provider in ["github_plugins", "github"] {
            let scoped = Self::scoped_key(tenant_id, provider);
            if let Some(val) = secrets.get(&scoped) {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    return Ok(Some(Zeroizing::new(trimmed.to_string())));
                }
            }
            if tenant_id == "local-default" {
                if let Some(val) = secrets.get(provider) {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        return Ok(Some(Zeroizing::new(trimmed.to_string())));
                    }
                }
            }
        }
        Ok(None)
    }

    #[cfg(target_os = "macos")]
    fn try_store_os_keychain(&self, provider_id: &str, secret: &str) -> Result<(), String> {
        if provider_id
            .chars()
            .any(|c| c.is_control() || c == '"' || c == '\\')
            || secret.chars().any(|c| c.is_control())
        {
            return Err("Keychain item contains unsupported control characters".to_string());
        }
        // Pass secret via piped stdin (`security -i`) so secret NEVER appears in process CLI argv (`ps aux`)
        let escaped_secret = secret.replace('\\', "\\\\").replace('"', "\\\"");
        let cmd_line = Zeroizing::new(format!(
            "add-generic-password -U -s cowok-design-byok -a \"{provider_id}\" -w \"{escaped_secret}\"\n"
        ));
        let mut child = std::process::Command::new("/usr/bin/security")
            .arg("-i")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(cmd_line.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("security -i add-generic-password failed".to_string())
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn try_store_os_keychain(&self, _provider_id: &str, _secret: &str) -> Result<(), String> {
        Err("non-macos".to_string())
    }

    #[cfg(target_os = "macos")]
    fn try_delete_os_keychain(&self, provider_id: &str) -> Result<(), String> {
        let _ = std::process::Command::new("/usr/bin/security")
            .args([
                "delete-generic-password",
                "-s",
                "cowok-design-byok",
                "-a",
                provider_id,
            ])
            .stdin(std::process::Stdio::null())
            .output();
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn try_delete_os_keychain(&self, _provider_id: &str) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn test_vault_seal_unseal_and_0600_permissions() {
        let tmp = TempDir::new().unwrap();
        let store = VaultStore::from_workspace_root(tmp.path());
        assert!(store.is_vault_writable());

        let secret_val = "sk-ant-api03-supersecret-1234567890abcdef";
        let vault_ref = store
            .upsert_secret("anthropic", secret_val, None)
            .expect("upsert should succeed");
        assert_eq!(vault_ref, "vault://vault_0600/anthropic");

        // Ciphertext on disk must NEVER contain raw secret
        let raw_on_disk = fs::read_to_string(store.vault_path()).unwrap();
        assert!(!raw_on_disk.contains(secret_val));
        assert!(raw_on_disk.contains("\"kdf\": \"argon2id\""));
        assert!(raw_on_disk.contains("\"cipher\": \"aes-256-gcm\""));

        #[cfg(unix)]
        {
            let mode = fs::metadata(store.vault_path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "vault.enc must be strictly 0600");
        }

        let unsealed = store.unseal_secrets(None).expect("unseal should succeed");
        assert_eq!(
            unsealed
                .get("tenant::local-default::anthropic")
                .map(String::as_str),
            Some(secret_val)
        );
        // Cross-tenant isolation: tenant-b upsert must not overwrite local-default
        store
            .upsert_secret_scoped("tenant-b", "anthropic", "sk-tenant-b-only", None)
            .expect("tenant-b upsert");
        let after_b = store.unseal_secrets(None).unwrap();
        assert_eq!(
            after_b
                .get("tenant::local-default::anthropic")
                .map(String::as_str),
            Some(secret_val)
        );
        assert_eq!(
            after_b
                .get("tenant::tenant-b::anthropic")
                .map(String::as_str),
            Some("sk-tenant-b-only")
        );

        // Delete secret removes only the targeted tenant scope
        store.delete_secret("anthropic", None).unwrap();
        let after_del = store.unseal_secrets(None).unwrap();
        assert!(!after_del.contains_key("tenant::local-default::anthropic"));
        assert!(after_del.contains_key("tenant::tenant-b::anthropic"));
    }

    #[test]
    fn test_vault_passphrase_isolation() {
        let tmp = TempDir::new().unwrap();
        let store = VaultStore::from_workspace_root(tmp.path());
        let mut map = BTreeMap::new();
        map.insert(
            "openai".to_string(),
            "sk-test-secret-key-987654".to_string(),
        );
        store
            .seal_secrets(&map, Some("correct-horse-battery-staple"))
            .unwrap();

        assert!(store.unseal_secrets(Some("wrong-passphrase")).is_err());
        // Security invariant: wrong passphrase must NOT overwrite or wipe vault.enc
        assert!(store
            .upsert_secret("extra", "sk-nope", Some("wrong-passphrase"))
            .is_err());
        let ok = store
            .unseal_secrets(Some("correct-horse-battery-staple"))
            .unwrap();
        assert_eq!(
            ok.get("openai").map(String::as_str),
            Some("sk-test-secret-key-987654")
        );
        // Master key path must stay outside .cowok-design/ workspace config directory
        assert!(!store
            .master_key_path
            .starts_with(tmp.path().join(".cowok-design")));
    }

    #[test]
    fn test_ram_vault_store_clears_tenant_and_is_isolated() {
        let ram = RamVaultStore::default();
        ram.upsert("tenant-a", "Anthropic", "sk-secret-a");
        ram.upsert("tenant-b", "openai", "sk-secret-b");
        assert!(ram.contains("tenant-a", "anthropic"));
        assert!(ram.contains("tenant-b", "openai"));
        ram.clear_tenant("tenant-a");
        assert!(!ram.contains("tenant-a", "anthropic"));
        assert!(ram.contains("tenant-b", "openai"));
        assert!(ram.remove("tenant-b", "openai"));
        assert!(!ram.contains("tenant-b", "openai"));
    }
}
