// @story 04.3
// @implements AC-03 (OS Keychain + Argon2id + AES-256-GCM 0o600 Vault Custody & Anti-Downgrade)
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

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

#[derive(Debug, Clone)]
pub struct VaultStore {
    cowok_dir: PathBuf,
    vault_path: PathBuf,
    master_key_path: PathBuf,
}

impl VaultStore {
    pub fn from_workspace_root(workspace_root: &Path) -> Self {
        let cowok_dir = workspace_root.join(".cowok-design");
        Self {
            vault_path: cowok_dir.join("vault.enc"),
            master_key_path: cowok_dir.join("vault.key"),
            cowok_dir,
        }
    }

    pub fn vault_path(&self) -> &Path {
        &self.vault_path
    }

    /// Returns true if `.cowok-design/vault.enc` (or `.cowok-design/` directory) is writable.
    /// Used by anti-downgrade guard (409 VAULT_DOWNGRADE_FORBIDDEN on `ephemeral`).
    pub fn is_vault_writable(&self) -> bool {
        if let Ok(meta) = fs::symlink_metadata(&self.vault_path) {
            if meta.file_type().is_symlink() {
                return false;
            }
            return !meta.permissions().readonly();
        }
        if fs::create_dir_all(&self.cowok_dir).is_err() {
            return false;
        }
        #[cfg(unix)]
        {
            if let Ok(meta) = fs::metadata(&self.cowok_dir) {
                let mode = meta.permissions().mode();
                // Owner write bit (0o200)
                return (mode & 0o200) != 0 && !meta.permissions().readonly();
            }
        }
        true
    }

    fn ensure_cowok_dir(&self) -> Result<(), String> {
        fs::create_dir_all(&self.cowok_dir)
            .map_err(|e| format!("Failed creating .cowok-design dir: {e}"))?;
        #[cfg(unix)]
        {
            let _ = fs::set_permissions(&self.cowok_dir, fs::Permissions::from_mode(0o700));
        }
        Ok(())
    }

    fn write_atomic_0600(target: &Path, bytes: &[u8]) -> Result<(), String> {
        let parent = target
            .parent()
            .ok_or_else(|| "Vault target path missing parent dir".to_string())?;
        fs::create_dir_all(parent).map_err(|e| format!("Failed creating parent vault dir: {e}"))?;

        let tmp_name = format!(
            ".{}.tmp.{}",
            target
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("vault"),
            Uuid::new_v4()
        );
        let tmp_path = parent.join(tmp_name);

        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            opts.mode(0o600);
        }

        let mut f = opts
            .open(&tmp_path)
            .map_err(|e| format!("Failed opening temp 0o600 vault file: {e}"))?;
        if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!("Failed writing/syncing temp vault file: {e}"));
        }
        drop(f);

        #[cfg(unix)]
        {
            fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("Failed enforcing 0o600 on temp vault file: {e}"))?;
        }

        fs::rename(&tmp_path, target).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            format!("Failed atomic rename of vault file: {e}")
        })?;

        #[cfg(unix)]
        {
            fs::set_permissions(target, fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("Failed enforcing 0o600 on vault file: {e}"))?;
        }
        Ok(())
    }

    fn load_or_init_machine_secret(&self) -> Result<Vec<u8>, String> {
        self.ensure_cowok_dir()?;
        if self.master_key_path.exists() {
            let meta = fs::symlink_metadata(&self.master_key_path)
                .map_err(|e| format!("Failed checking vault.key metadata: {e}"))?;
            if meta.file_type().is_symlink() || !meta.is_file() {
                return Err(
                    "Security violation: .cowok-design/vault.key is a symlink or non-file"
                        .to_string(),
                );
            }
            let raw = fs::read_to_string(&self.master_key_path)
                .map_err(|e| format!("Failed reading .cowok-design/vault.key: {e}"))?;
            let decoded = B64
                .decode(raw.trim())
                .map_err(|e| format!("Corrupted .cowok-design/vault.key base64: {e}"))?;
            if decoded.len() < 32 {
                return Err("Corrupted .cowok-design/vault.key length < 32 bytes".to_string());
            }
            return Ok(decoded);
        }

        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        let encoded = B64.encode(secret);
        Self::write_atomic_0600(&self.master_key_path, encoded.as_bytes())?;
        Ok(secret.to_vec())
    }

    fn derive_aes_key(ikm: &[u8], salt: &[u8]) -> Result<[u8; 32], String> {
        let mut derived = [0u8; 32];
        // Argon2id with explicit moderate cost parameters suitable for fast daemon calls
        let params = argon2::Params::new(16 * 1024, 2, 1, Some(32))
            .map_err(|e| format!("Argon2id params error: {e}"))?;
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        argon2
            .hash_password_into(ikm, salt, &mut derived)
            .map_err(|e| format!("Argon2id derivation error: {e}"))?;
        Ok(derived)
    }

    pub fn seal_secrets(
        &self,
        secrets: &BTreeMap<String, String>,
        master_passphrase: Option<&str>,
    ) -> Result<(), String> {
        let ikm = match master_passphrase.map(str::trim).filter(|s| !s.is_empty()) {
            Some(pass) => pass.as_bytes().to_vec(),
            None => self.load_or_init_machine_secret()?,
        };

        let mut salt = [0u8; 16];
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce_bytes);

        let key_bytes = Self::derive_aes_key(&ikm, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| format!("AES-256-GCM init error: {e}"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let payload = VaultPayload {
            secrets: secrets.clone(),
        };
        let plaintext = serde_json::to_vec(&payload)
            .map_err(|e| format!("Failed serializing vault payload: {e}"))?;
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

        let ikm = match master_passphrase.map(str::trim).filter(|s| !s.is_empty()) {
            Some(pass) => pass.as_bytes().to_vec(),
            None => self.load_or_init_machine_secret()?,
        };
        let key_bytes = Self::derive_aes_key(&ikm, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| format!("AES-256-GCM init error: {e}"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ciphertext.as_slice()).map_err(|_| {
            "Vault decryption failed (invalid master key or tampered ciphertext)".to_string()
        })?;
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
        let mut secrets = self.unseal_secrets(master_passphrase).unwrap_or_default();
        let pid = provider_id.trim().to_ascii_lowercase();
        secrets.insert(pid.clone(), secret.trim().to_string());
        self.seal_secrets(&secrets, master_passphrase)?;

        // Best-effort non-interactive OS Keychain sync when COWOK_OS_KEYCHAIN=1
        if std::env::var("COWOK_OS_KEYCHAIN").as_deref() == Ok("1") {
            let _ = self.try_store_os_keychain(&pid, secret.trim());
        }
        Ok(format!("vault://vault_0600/{pid}"))
    }

    pub fn delete_secret(
        &self,
        provider_id: &str,
        master_passphrase: Option<&str>,
    ) -> Result<(), String> {
        let pid = provider_id.trim().to_ascii_lowercase();
        if self.vault_path.exists() {
            if let Ok(mut secrets) = self.unseal_secrets(master_passphrase) {
                if secrets.remove(&pid).is_some() {
                    self.seal_secrets(&secrets, master_passphrase)?;
                }
            }
        }
        if std::env::var("COWOK_OS_KEYCHAIN").as_deref() == Ok("1") {
            let _ = self.try_delete_os_keychain(&pid);
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn try_store_os_keychain(&self, provider_id: &str, secret: &str) -> Result<(), String> {
        let out = std::process::Command::new("/usr/bin/security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                "cowok-design-byok",
                "-a",
                provider_id,
                "-w",
                secret,
            ])
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err("security add-generic-password failed".to_string())
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
            unsealed.get("anthropic").map(String::as_str),
            Some(secret_val)
        );

        // Delete secret removes it cleanly
        store.delete_secret("anthropic", None).unwrap();
        let after_del = store.unseal_secrets(None).unwrap();
        assert!(!after_del.contains_key("anthropic"));
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
        let ok = store
            .unseal_secrets(Some("correct-horse-battery-staple"))
            .unwrap();
        assert_eq!(
            ok.get("openai").map(String::as_str),
            Some("sk-test-secret-key-987654")
        );
    }
}
