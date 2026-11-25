// @story 11.2
// @implements AC-03, AC-04 (1-Click Safe External Host MCP Config Installer with O_NOFOLLOW, File Lock, Sibling-Preserving JSON/TOML Merge, SHA-256 Backup & 1-Click Rollback)
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

pub const COWOK_SERVER_KEY: &str = "cowok-design";
pub const COWOK_TOML_KEY: &str = "cowok_design";

#[derive(Debug, Error)]
pub enum HostInstallError {
    #[error("Unsupported external host client '{host_id}'.")]
    UnsupportedHost { host_id: String },
    #[error("Refused symlink path component at '{path}' (O_NOFOLLOW security policy).")]
    SymlinkRejected { path: String },
    #[error("Invalid or malformed host configuration file at '{path}': {reason}")]
    MalformedConfig { path: String, reason: String },
    #[error("Backup checksum verification failed for '{backup_path}': expected {expected}, got {actual}")]
    ChecksumMismatch {
        backup_path: String,
        expected: String,
        actual: String,
    },
    #[error("Backup file not found at '{path}'.")]
    BackupNotFound { path: String },
    #[error("I/O error during host config operation: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostClientId {
    CowokAi,
    Antigravity,
    ClaudeCode,
    Codex,
    Cursor,
    Opencode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostConfigFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostTargetInfo {
    pub host_id: String,
    pub display_name: String,
    pub config_format: HostConfigFormat,
    pub default_relative_path: String,
    pub detected: bool,
}

impl HostClientId {
    pub const ALL: &'static [HostClientId] = &[
        Self::CowokAi,
        Self::Antigravity,
        Self::ClaudeCode,
        Self::Codex,
        Self::Cursor,
        Self::Opencode,
    ];

    pub fn parse(raw: &str) -> Result<Self, HostInstallError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cowok_ai" | "cowok-ai" | "cowok" => Ok(Self::CowokAi),
            "antigravity" => Ok(Self::Antigravity),
            "claude_code" | "claude-code" | "claude" => Ok(Self::ClaudeCode),
            "codex" => Ok(Self::Codex),
            "cursor" => Ok(Self::Cursor),
            "opencode" | "open_code" | "open-code" => Ok(Self::Opencode),
            _ => Err(HostInstallError::UnsupportedHost {
                host_id: raw.to_string(),
            }),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CowokAi => "cowok_ai",
            Self::Antigravity => "antigravity",
            Self::ClaudeCode => "claude_code",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
            Self::Opencode => "opencode",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::CowokAi => "Cowok.ai",
            Self::Antigravity => "Antigravity",
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
            Self::Opencode => "OpenCode",
        }
    }

    pub fn config_format(&self) -> HostConfigFormat {
        match self {
            Self::Codex => HostConfigFormat::Toml,
            _ => HostConfigFormat::Json,
        }
    }

    pub fn default_relative_path(&self) -> &'static str {
        match self {
            Self::CowokAi => ".cowok-ai/mcp.json",
            Self::Antigravity => ".gemini/antigravity/mcp_config.json",
            Self::ClaudeCode => ".claude.json",
            Self::Codex => ".codex/config.toml",
            Self::Cursor => ".cursor/mcp.json",
            Self::Opencode => ".config/opencode/opencode.json",
        }
    }

    pub fn json_servers_container_key(&self) -> &'static str {
        match self {
            Self::Opencode => "mcp",
            _ => "mcpServers",
        }
    }
}

pub fn list_supported_hosts(home_dir: Option<&Path>) -> Vec<HostTargetInfo> {
    HostClientId::ALL
        .iter()
        .map(|host| {
            let detected = home_dir
                .map(|home| {
                    let candidate = home.join(host.default_relative_path());
                    candidate
                        .parent()
                        .map(|p| {
                            fs::symlink_metadata(p)
                                .map(|m| !m.file_type().is_symlink())
                                .unwrap_or(false)
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            HostTargetInfo {
                host_id: host.as_str().to_string(),
                display_name: host.display_name().to_string(),
                config_format: host.config_format(),
                default_relative_path: format!("~/{}", host.default_relative_path()),
                detected,
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct HostInstallOptions {
    pub host: HostClientId,
    pub config_path: PathBuf,
    pub backup_dir: PathBuf,
    pub server_command: String,
    pub server_args: Vec<String>,
    pub server_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInstallReceipt {
    pub backup_id: String,
    pub host_id: String,
    pub display_name: String,
    pub config_path: String,
    pub backup_path: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub created_new: bool,
    pub installed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRollbackReceipt {
    pub host_id: String,
    pub config_path: String,
    pub backup_path: String,
    pub restored_sha256: String,
    pub rolled_back_at: i64,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
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

/// Rejects any symlink at `target` or in its non-OS ancestor chain (O_NOFOLLOW defense).
pub fn reject_symlinks_in_path(target: &Path) -> Result<(), HostInstallError> {
    let mut current = PathBuf::new();
    for comp in target.components() {
        match comp {
            Component::RootDir => current.push(Path::new("/")),
            Component::Prefix(p) => current.push(p.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(HostInstallError::SymlinkRejected {
                    path: target.display().to_string(),
                });
            }
            Component::Normal(seg) => {
                current.push(seg);
                if is_os_root_symlink(&current) {
                    continue;
                }
                if let Ok(meta) = fs::symlink_metadata(&current) {
                    if meta.file_type().is_symlink() {
                        return Err(HostInstallError::SymlinkRejected {
                            path: current.display().to_string(),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn open_read_nofollow(path: &Path) -> Result<Option<File>, HostInstallError> {
    reject_symlinks_in_path(path)?;
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    match opts.open(path) {
        Ok(file) => Ok(Some(file)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => {
            #[cfg(unix)]
            if err.raw_os_error() == Some(libc::ELOOP) {
                return Err(HostInstallError::SymlinkRejected {
                    path: path.display().to_string(),
                });
            }
            Err(HostInstallError::Io(err))
        }
    }
}

fn write_atomic_nofollow(target: &Path, content: &[u8]) -> Result<(), HostInstallError> {
    reject_symlinks_in_path(target)?;
    let parent = target.parent().ok_or_else(|| {
        HostInstallError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Target config path has no parent directory",
        ))
    })?;
    fs::create_dir_all(parent)?;
    reject_symlinks_in_path(parent)?;

    let tmp_name = format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("mcp-config"),
        Uuid::new_v4()
    );
    let tmp_path = parent.join(tmp_name);
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut tmp_file = opts.open(&tmp_path)?;
    tmp_file.write_all(content)?;
    tmp_file.sync_all()?;
    drop(tmp_file);

    // Re-verify target right before atomic rename
    if let Ok(meta) = fs::symlink_metadata(target) {
        if meta.file_type().is_symlink() {
            let _ = fs::remove_file(&tmp_path);
            return Err(HostInstallError::SymlinkRejected {
                path: target.display().to_string(),
            });
        }
    }
    fs::rename(&tmp_path, target)?;
    Ok(())
}

struct AdvisoryFileLock {
    _file: File,
}

impl AdvisoryFileLock {
    fn acquire(lock_path: &Path) -> Result<Self, HostInstallError> {
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
            reject_symlinks_in_path(parent)?;
        }
        reject_symlinks_in_path(lock_path)?;
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = opts.open(lock_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            // SAFETY: file descriptor is valid and owned by `file` for the lifetime of this guard.
            let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
            if rc != 0 {
                return Err(HostInstallError::Io(std::io::Error::last_os_error()));
            }
        }
        Ok(Self { _file: file })
    }
}

/// Merges the `cowok-design` stanza into a JSON host config while preserving every sibling key and server.
pub fn merge_json_host_config(
    existing_raw: &str,
    host: HostClientId,
    server_command: &str,
    server_args: &[String],
    server_url: &str,
    path_for_err: &str,
) -> Result<String, HostInstallError> {
    let mut root: serde_json::Value = if existing_raw.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(existing_raw).map_err(|e| HostInstallError::MalformedConfig {
            path: path_for_err.to_string(),
            reason: e.to_string(),
        })?
    };

    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| HostInstallError::MalformedConfig {
            path: path_for_err.to_string(),
            reason: "Root JSON configuration must be an object".to_string(),
        })?;

    let container_key = host.json_servers_container_key();
    if !root_obj.contains_key(container_key) {
        root_obj.insert(container_key.to_string(), serde_json::json!({}));
    }
    let servers_obj = root_obj
        .get_mut(container_key)
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| HostInstallError::MalformedConfig {
            path: path_for_err.to_string(),
            reason: format!("'{container_key}' must be a JSON object"),
        })?;

    let entry = serde_json::json!({
        "command": server_command,
        "args": server_args,
        "url": server_url,
        "transport": "stdio",
        "managedBy": "cowok-design-mcp-gateway"
    });
    servers_obj.insert(COWOK_SERVER_KEY.to_string(), entry);

    serde_json::to_string_pretty(&root)
        .map(|s| format!("{s}\n"))
        .map_err(|e| HostInstallError::MalformedConfig {
            path: path_for_err.to_string(),
            reason: e.to_string(),
        })
}

fn escape_toml_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 8);
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) == 0x7F => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn is_cowok_toml_section(segments: &[String]) -> bool {
    segments.len() >= 2 && segments[0] == "mcp_servers" && segments[1] == COWOK_TOML_KEY
}

struct TomlStructuralParser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> TomlStructuralParser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn starts_with(&self, needle: &str) -> bool {
        self.bytes[self.pos..].starts_with(needle.as_bytes())
    }

    fn skip_inline_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.pos += 1;
        }
    }

    fn skip_comment_to_eol(&mut self) {
        if self.peek() == Some(b'#') {
            while let Some(b) = self.peek() {
                if b == b'\n' || b == b'\r' {
                    break;
                }
                self.pos += 1;
            }
        }
    }

    fn skip_ws_comments_newlines(&mut self) {
        loop {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
                self.pos += 1;
            }
            if self.peek() == Some(b'#') {
                self.skip_comment_to_eol();
            } else {
                break;
            }
        }
    }

    fn expect_line_end(&mut self) -> Result<(), String> {
        self.skip_inline_ws();
        self.skip_comment_to_eol();
        match self.peek() {
            None => Ok(()),
            Some(b'\n') => {
                self.pos += 1;
                Ok(())
            }
            Some(b'\r') => {
                self.pos += 1;
                if self.peek() == Some(b'\n') {
                    self.pos += 1;
                }
                Ok(())
            }
            Some(other) => Err(format!(
                "Unexpected trailing character '{}' at byte {}",
                other as char, self.pos
            )),
        }
    }

    fn parse_basic_string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(format!("Expected '\"' at byte {}", self.pos));
        }
        self.pos += 1;
        let mut out = String::new();
        while self.pos < self.bytes.len() {
            let ch = self.src[self.pos..]
                .chars()
                .next()
                .ok_or_else(|| "Unexpected EOF in basic string".to_string())?;
            self.pos += ch.len_utf8();
            match ch {
                '"' => return Ok(out),
                '\n' | '\r' => {
                    return Err("Unterminated single-line basic string before newline".to_string());
                }
                c if (c as u32) < 0x20 && c != '\t' => {
                    return Err(format!(
                        "Unescaped control character U+{:04X} in basic string",
                        c as u32
                    ));
                }
                '\\' => {
                    let esc = self.parse_escape_sequence()?;
                    out.push(esc);
                }
                c => out.push(c),
            }
        }
        Err("Unterminated basic string".to_string())
    }

    fn parse_multiline_basic_string(&mut self) -> Result<(), String> {
        if !self.starts_with("\"\"\"") {
            return Err(format!("Expected '\"\"\"' at byte {}", self.pos));
        }
        self.pos += 3;
        if self.peek() == Some(b'\r') {
            self.pos += 1;
            if self.peek() == Some(b'\n') {
                self.pos += 1;
            }
        } else if self.peek() == Some(b'\n') {
            self.pos += 1;
        }

        while self.pos < self.bytes.len() {
            if self.starts_with("\"\"\"") {
                self.pos += 3;
                // TOML allows up to 2 extra quotes at the end of a multiline basic string
                let mut extra = 0;
                while self.peek() == Some(b'"') && extra < 2 {
                    self.pos += 1;
                    extra += 1;
                }
                return Ok(());
            }
            let ch = self.src[self.pos..]
                .chars()
                .next()
                .ok_or_else(|| "Unexpected EOF in multiline basic string".to_string())?;
            self.pos += ch.len_utf8();
            if ch == '\\' {
                // Line-ending backslash trims whitespace and newlines
                let saved = self.pos;
                self.skip_inline_ws();
                if matches!(self.peek(), Some(b'\n' | b'\r')) {
                    while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                        self.pos += 1;
                    }
                } else {
                    self.pos = saved;
                    let _ = self.parse_escape_sequence()?;
                }
            }
        }
        Err("Unterminated multiline basic string".to_string())
    }

    fn parse_escape_sequence(&mut self) -> Result<char, String> {
        let Some(esc) = self.peek() else {
            return Err("Unterminated escape sequence".to_string());
        };
        self.pos += 1;
        match esc {
            b'b' => Ok('\u{0008}'),
            b't' => Ok('\t'),
            b'n' => Ok('\n'),
            b'f' => Ok('\u{000C}'),
            b'r' => Ok('\r'),
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'u' => self.parse_unicode_escape(4),
            b'U' => self.parse_unicode_escape(8),
            other => Err(format!(
                "Invalid TOML escape sequence '\\{}'",
                other as char
            )),
        }
    }

    fn parse_unicode_escape(&mut self, digits: usize) -> Result<char, String> {
        let end = self.pos + digits;
        let Some(hex) = self.src.get(self.pos..end) else {
            return Err("Incomplete unicode escape in TOML string".to_string());
        };
        self.pos = end;
        let code = u32::from_str_radix(hex, 16)
            .map_err(|_| format!("Invalid hex unicode escape '{hex}'"))?;
        char::from_u32(code).ok_or_else(|| format!("Invalid unicode scalar value U+{code:X}"))
    }

    fn parse_literal_string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'\'') {
            return Err(format!("Expected '\\'' at byte {}", self.pos));
        }
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'\'' {
                let s = self.src[start..self.pos].to_string();
                self.pos += 1;
                return Ok(s);
            }
            if b == b'\n' || b == b'\r' {
                return Err("Unterminated literal string before newline".to_string());
            }
            self.pos += 1;
        }
        Err("Unterminated literal string".to_string())
    }

    fn parse_multiline_literal_string(&mut self) -> Result<(), String> {
        if !self.starts_with("'''") {
            return Err(format!("Expected \"'''\" at byte {}", self.pos));
        }
        self.pos += 3;
        while self.pos < self.bytes.len() {
            if self.starts_with("'''") {
                self.pos += 3;
                let mut extra = 0;
                while self.peek() == Some(b'\'') && extra < 2 {
                    self.pos += 1;
                    extra += 1;
                }
                return Ok(());
            }
            self.pos += 1;
        }
        Err("Unterminated multiline literal string".to_string())
    }

    fn parse_key_segment(&mut self) -> Result<String, String> {
        self.skip_inline_ws();
        match self.peek() {
            Some(b'"') => self.parse_basic_string(),
            Some(b'\'') => self.parse_literal_string(),
            Some(_) => {
                let start = self.pos;
                while let Some(b) = self.peek() {
                    if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                if self.pos == start {
                    return Err(format!("Expected TOML key at byte {}", self.pos));
                }
                Ok(self.src[start..self.pos].to_string())
            }
            None => Err("Unexpected EOF while parsing TOML key".to_string()),
        }
    }

    fn parse_key_path(&mut self) -> Result<Vec<String>, String> {
        let mut segments = vec![self.parse_key_segment()?];
        loop {
            self.skip_inline_ws();
            if self.peek() == Some(b'.') {
                self.pos += 1;
                segments.push(self.parse_key_segment()?);
            } else {
                break;
            }
        }
        Ok(segments)
    }

    fn parse_array(&mut self) -> Result<(), String> {
        if self.peek() != Some(b'[') {
            return Err(format!("Expected '[' at byte {}", self.pos));
        }
        self.pos += 1;
        loop {
            self.skip_ws_comments_newlines();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(());
            }
            self.parse_value()?;
            self.skip_ws_comments_newlines();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(());
                }
                _ => {
                    return Err(format!(
                        "Expected ',' or ']' in TOML array at byte {}",
                        self.pos
                    ))
                }
            }
        }
    }

    fn parse_inline_table(&mut self) -> Result<(), String> {
        if self.peek() != Some(b'{') {
            return Err(format!("Expected '{{' at byte {}", self.pos));
        }
        self.pos += 1;
        self.skip_inline_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(());
        }
        loop {
            self.skip_inline_ws();
            let _ = self.parse_key_path()?;
            self.skip_inline_ws();
            if self.peek() != Some(b'=') {
                return Err(format!("Expected '=' in inline table at byte {}", self.pos));
            }
            self.pos += 1;
            self.skip_inline_ws();
            self.parse_value()?;
            self.skip_inline_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(());
                }
                _ => {
                    return Err(format!(
                        "Expected ',' or '}}' in inline table at byte {}",
                        self.pos
                    ));
                }
            }
        }
    }

    fn parse_bare_scalar(&mut self) -> Result<(), String> {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'#' | b',' | b']' | b'}') {
                break;
            }
            self.pos += 1;
        }
        if self.pos == start {
            return Err(format!("Missing TOML value at byte {}", self.pos));
        }
        let mut token = &self.src[start..self.pos];
        // Support RFC 3339 datetime with a single space separator between date and time
        if is_iso_date_prefix(token) && self.peek() == Some(b' ') {
            let saved = self.pos;
            self.pos += 1;
            let time_start = self.pos;
            while let Some(b) = self.peek() {
                if matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'#' | b',' | b']' | b'}') {
                    break;
                }
                self.pos += 1;
            }
            let time_part = &self.src[time_start..self.pos];
            if is_iso_time_token(time_part) {
                token = &self.src[start..self.pos];
            } else {
                self.pos = saved;
            }
        }

        if is_valid_toml_bare_scalar(token) {
            Ok(())
        } else {
            Err(format!("Invalid TOML value '{token}'"))
        }
    }

    fn parse_value(&mut self) -> Result<(), String> {
        self.skip_inline_ws();
        if self.starts_with("\"\"\"") {
            self.parse_multiline_basic_string()
        } else if self.starts_with("'''") {
            self.parse_multiline_literal_string()
        } else {
            match self.peek() {
                Some(b'"') => self.parse_basic_string().map(|_| ()),
                Some(b'\'') => self.parse_literal_string().map(|_| ()),
                Some(b'[') => self.parse_array(),
                Some(b'{') => self.parse_inline_table(),
                Some(_) => self.parse_bare_scalar(),
                None => Err("Unexpected EOF where TOML value was expected".to_string()),
            }
        }
    }

    fn parse_document_and_find_cowok_ranges(&mut self) -> Result<Vec<Range<usize>>, String> {
        let mut explicit_tables: HashSet<Vec<String>> = HashSet::new();
        let mut removed_ranges: Vec<Range<usize>> = Vec::new();
        let mut active_cowok_range: Option<Range<usize>> = None;

        loop {
            self.skip_ws_comments_newlines();
            if self.pos >= self.bytes.len() {
                break;
            }
            let stmt_start = self.pos;
            if self.starts_with("[[") {
                if let Some(range) = active_cowok_range.take() {
                    removed_ranges.push(range);
                }
                self.pos += 2;
                let segments = self.parse_key_path()?;
                self.skip_inline_ws();
                if !self.starts_with("]]") {
                    return Err(format!(
                        "Unclosed array-of-tables header at byte {}",
                        stmt_start
                    ));
                }
                self.pos += 2;
                self.expect_line_end()?;
                if is_cowok_toml_section(&segments) {
                    active_cowok_range = Some(stmt_start..self.pos);
                }
            } else if self.peek() == Some(b'[') {
                if let Some(range) = active_cowok_range.take() {
                    removed_ranges.push(range);
                }
                self.pos += 1;
                let segments = self.parse_key_path()?;
                self.skip_inline_ws();
                if self.peek() != Some(b']') {
                    return Err(format!("Unclosed table header at byte {}", stmt_start));
                }
                self.pos += 1;
                self.expect_line_end()?;
                if !explicit_tables.insert(segments.clone()) {
                    return Err(format!(
                        "Duplicate TOML table header '[{}]'",
                        segments.join(".")
                    ));
                }
                if is_cowok_toml_section(&segments) {
                    active_cowok_range = Some(stmt_start..self.pos);
                }
            } else {
                let _key = self.parse_key_path()?;
                self.skip_inline_ws();
                if self.peek() != Some(b'=') {
                    return Err(format!(
                        "Expected '=' in key-value pair at byte {}",
                        self.pos
                    ));
                }
                self.pos += 1;
                self.skip_inline_ws();
                self.parse_value()?;
                self.expect_line_end()?;
                if let Some(ref mut range) = active_cowok_range {
                    range.end = self.pos;
                }
            }
        }

        if let Some(range) = active_cowok_range {
            removed_ranges.push(range);
        }

        Ok(removed_ranges)
    }
}

fn is_iso_date_prefix(tok: &str) -> bool {
    let b = tok.as_bytes();
    b.len() == 10
        && b[0..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
}

fn is_iso_time_token(tok: &str) -> bool {
    let b = tok.as_bytes();
    b.len() >= 8
        && b[0..2].iter().all(u8::is_ascii_digit)
        && b[2] == b':'
        && b[3..5].iter().all(u8::is_ascii_digit)
        && b[5] == b':'
        && b[6..8].iter().all(u8::is_ascii_digit)
}

fn is_valid_toml_bare_scalar(token: &str) -> bool {
    if matches!(
        token,
        "true" | "false" | "inf" | "+inf" | "-inf" | "nan" | "+nan" | "-nan"
    ) {
        return true;
    }
    if is_iso_date_prefix(token) || is_iso_time_token(token) {
        return true;
    }
    if token.is_ascii()
        && token.len() >= 19
        && is_iso_date_prefix(&token[..10])
        && matches!(token.as_bytes()[10], b'T' | b't' | b' ')
        && is_iso_time_token(&token[11..])
    {
        return true;
    }
    let unsigned = token.strip_prefix(['+', '-']).unwrap_or(token);
    if let Some(hex) = unsigned.strip_prefix("0x") {
        return !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit() || b == b'_');
    }
    if let Some(oct) = unsigned.strip_prefix("0o") {
        return !oct.is_empty() && oct.bytes().all(|b| matches!(b, b'0'..=b'7' | b'_'));
    }
    if let Some(bin) = unsigned.strip_prefix("0b") {
        return !bin.is_empty() && bin.bytes().all(|b| matches!(b, b'0' | b'1' | b'_'));
    }
    if unsigned.is_empty() || !unsigned.as_bytes()[0].is_ascii_digit() {
        return false;
    }
    let cleaned = unsigned.replace('_', "");
    cleaned.parse::<f64>().is_ok()
}

fn validate_toml_document(
    src: &str,
    path_for_err: &str,
) -> Result<Vec<Range<usize>>, HostInstallError> {
    let mut parser = TomlStructuralParser::new(src);
    parser
        .parse_document_and_find_cowok_ranges()
        .map_err(|reason| HostInstallError::MalformedConfig {
            path: path_for_err.to_string(),
            reason,
        })
}

fn merge_codex_toml_config_with_path(
    existing_raw: &str,
    server_command: &str,
    server_args: &[String],
    server_url: &str,
    path_for_err: &str,
) -> Result<String, HostInstallError> {
    let removed_ranges = validate_toml_document(existing_raw, path_for_err)?;

    let args_formatted = server_args
        .iter()
        .map(|a| format!("\"{}\"", escape_toml_string(a)))
        .collect::<Vec<_>>()
        .join(", ");

    let new_block = format!(
        "[mcp_servers.{COWOK_TOML_KEY}]\ncommand = \"{}\"\nargs = [{args_formatted}]\nurl = \"{}\"\ntransport = \"stdio\"\n",
        escape_toml_string(server_command),
        escape_toml_string(server_url),
    );

    let mut kept = String::with_capacity(existing_raw.len());
    let mut cursor = 0usize;
    for range in removed_ranges {
        if range.start > cursor {
            kept.push_str(&existing_raw[cursor..range.start]);
        }
        cursor = range.end;
    }
    if cursor < existing_raw.len() {
        kept.push_str(&existing_raw[cursor..]);
    }

    let kept_trimmed = kept.trim_end();
    let merged = if kept_trimmed.is_empty() {
        new_block
    } else {
        format!("{kept_trimmed}\n\n{new_block}")
    };

    // Verify merged output parses cleanly as valid TOML before returning
    let _ = validate_toml_document(&merged, path_for_err)?;
    Ok(merged)
}

/// Merges `[mcp_servers.cowok_design]` into Codex TOML without clobbering sibling tables or comments.
pub fn merge_codex_toml_config(
    existing_raw: &str,
    server_command: &str,
    server_args: &[String],
    server_url: &str,
) -> Result<String, HostInstallError> {
    merge_codex_toml_config_with_path(
        existing_raw,
        server_command,
        server_args,
        server_url,
        "config.toml",
    )
}

/// Installs Cowok-Design MCP server config into the target host with `O_NOFOLLOW`, advisory lock,
/// SHA-256 pre-write backup, sibling-preserving JSON/TOML merge, and atomic rename.
pub fn install_host_config(
    opts: &HostInstallOptions,
) -> Result<HostInstallReceipt, HostInstallError> {
    reject_symlinks_in_path(&opts.config_path)?;
    reject_symlinks_in_path(&opts.backup_dir)?;
    fs::create_dir_all(&opts.backup_dir)?;

    let lock_path = opts
        .backup_dir
        .join(format!(".{}.install.lock", opts.host.as_str()));
    let _lock = AdvisoryFileLock::acquire(&lock_path)?;

    let (existing_bytes, created_new) = match open_read_nofollow(&opts.config_path)? {
        Some(mut file) => {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            (buf, false)
        }
        None => (Vec::new(), true),
    };

    let before_sha256 = sha256_hex(&existing_bytes);
    let existing_str = String::from_utf8(existing_bytes.clone()).map_err(|e| {
        HostInstallError::MalformedConfig {
            path: opts.config_path.display().to_string(),
            reason: format!("Host config is not valid UTF-8: {e}"),
        }
    })?;

    let merged = match opts.host.config_format() {
        HostConfigFormat::Json => merge_json_host_config(
            &existing_str,
            opts.host,
            &opts.server_command,
            &opts.server_args,
            &opts.server_url,
            &opts.config_path.display().to_string(),
        )?,
        HostConfigFormat::Toml => merge_codex_toml_config_with_path(
            &existing_str,
            &opts.server_command,
            &opts.server_args,
            &opts.server_url,
            &opts.config_path.display().to_string(),
        )?,
    };

    let backup_id = Uuid::new_v4().to_string();
    let now_ms = Utc::now().timestamp_millis();
    let backup_file_name = format!("{}-{}-{}.bak", opts.host.as_str(), now_ms, backup_id);
    let backup_path = opts.backup_dir.join(backup_file_name);
    write_atomic_nofollow(&backup_path, &existing_bytes)?;

    let merged_bytes = merged.as_bytes();
    let after_sha256 = sha256_hex(merged_bytes);
    write_atomic_nofollow(&opts.config_path, merged_bytes)?;

    Ok(HostInstallReceipt {
        backup_id,
        host_id: opts.host.as_str().to_string(),
        display_name: opts.host.display_name().to_string(),
        config_path: opts.config_path.display().to_string(),
        backup_path: backup_path.display().to_string(),
        before_sha256,
        after_sha256,
        created_new,
        installed_at: now_ms,
    })
}

/// Restores a host configuration file atomically from a verified SHA-256 backup.
pub fn rollback_host_install(
    host: HostClientId,
    config_path: &Path,
    backup_path: &Path,
    expected_before_sha256: &str,
) -> Result<HostRollbackReceipt, HostInstallError> {
    reject_symlinks_in_path(config_path)?;
    reject_symlinks_in_path(backup_path)?;

    let backup_dir = backup_path.parent().ok_or_else(|| {
        HostInstallError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Backup path has no parent directory for advisory lock",
        ))
    })?;
    let lock_path = backup_dir.join(format!(".{}.install.lock", host.as_str()));
    let _lock = AdvisoryFileLock::acquire(&lock_path)?;

    let Some(mut backup_file) = open_read_nofollow(backup_path)? else {
        return Err(HostInstallError::BackupNotFound {
            path: backup_path.display().to_string(),
        });
    };
    let mut backup_bytes = Vec::new();
    backup_file.read_to_end(&mut backup_bytes)?;

    let actual_sha = sha256_hex(&backup_bytes);
    if actual_sha != expected_before_sha256 {
        return Err(HostInstallError::ChecksumMismatch {
            backup_path: backup_path.display().to_string(),
            expected: expected_before_sha256.to_string(),
            actual: actual_sha,
        });
    }

    if backup_bytes.is_empty() {
        // Config file did not exist prior to install (empty backup): verify non-symlink and unlink
        // rather than writing a 0-byte corrupt JSON/TOML file.
        match fs::symlink_metadata(config_path) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(HostInstallError::SymlinkRejected {
                        path: config_path.display().to_string(),
                    });
                }
                fs::remove_file(config_path)?;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(HostInstallError::Io(err)),
        }
    } else {
        write_atomic_nofollow(config_path, &backup_bytes)?;
    }

    Ok(HostRollbackReceipt {
        host_id: host.as_str().to_string(),
        config_path: config_path.display().to_string(),
        backup_path: backup_path.display().to_string(),
        restored_sha256: actual_sha,
        rolled_back_at: Utc::now().timestamp_millis(),
    })
}
