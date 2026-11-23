// @story 09.1
// @implements AC-01, AC-03 (Two-Store SQLite FTS5 Cache at .cowok-design/cache/rea-reference.sqlite3 & Tier-1A <=250 tok cap)
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::firewall::FirewallVerdict;

pub const TIER1A_MAX_TOKENS: usize = 250;
pub const TIER1A_CHARS_PER_TOKEN: usize = 4;
pub const TIER1A_MAX_CHARS: usize = TIER1A_MAX_TOKENS * TIER1A_CHARS_PER_TOKEN;
pub const CACHE_RELATIVE_DIR: &str = ".cowok-design/cache";
pub const CACHE_DB_FILENAME: &str = "rea-reference.sqlite3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReaSearchTier {
    #[serde(rename = "1a")]
    Tier1a,
    #[serde(rename = "1b")]
    Tier1b,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofStatus {
    #[serde(rename = "awaiting-verification")]
    AwaitingVerification,
    #[serde(rename = "proof-verified")]
    ProofVerified,
}

impl ProofStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingVerification => "awaiting-verification",
            Self::ProofVerified => "proof-verified",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw {
            "awaiting-verification" => Some(Self::AwaitingVerification),
            "proof-verified" => Some(Self::ProofVerified),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaReferenceRecord {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub source_url: String,
    pub source_format: String,
    pub license_spdx: String,
    pub firewall_verdict: FirewallVerdict,
    pub firewall_reason: String,
    pub tier1a_summary: String,
    pub tier1a_tokens: usize,
    pub design_dna_json: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_fields_json: Option<String>,
    pub proof_status: ProofStatus,
    pub sanitized_secrets_count: usize,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaSearchHit {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub source_url: String,
    pub source_format: String,
    pub license_spdx: String,
    pub firewall_verdict: FirewallVerdict,
    pub firewall_reason: String,
    pub tier1a_summary: String,
    pub tier1a_tokens: usize,
    pub proof_status: ProofStatus,
    pub sanitized_secrets_count: usize,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_dna: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_fields: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bm25_rank: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct UpsertReaReference {
    pub tenant_id: String,
    pub slug: String,
    pub source_url: String,
    pub source_format: String,
    pub license_spdx: String,
    pub firewall_verdict: FirewallVerdict,
    pub firewall_reason: String,
    pub tier1a_summary: String,
    pub design_dna_json: String,
    pub raw_fields_json: Option<String>,
    pub proof_status: ProofStatus,
    pub sanitized_secrets_count: usize,
}

#[derive(Debug, Error)]
pub enum ReaStoreError {
    #[error("Refusing to follow symlink at '{0}'")]
    SymlinkRejected(String),
    #[error("Rejected verdict cannot be persisted to Two-Store cache")]
    RejectedVerdictForbidden,
    #[error("Reference '{0}' not found for tenant")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Deterministic token counter enforcing BOTH whitespace token count AND `ceil(char_count / 4)`
/// so whitespace-free strings cannot bypass the `Tier-1A <= 250` token invariant (hard max 1000 chars).
pub fn count_deterministic_tokens(text: &str) -> usize {
    let whitespace_tokens = text.split_whitespace().count();
    if whitespace_tokens == 0 {
        return 0;
    }
    let char_count = text.chars().count();
    let char_tokens = char_count.div_ceil(TIER1A_CHARS_PER_TOKEN);
    whitespace_tokens.max(char_tokens)
}

/// Deterministically truncates `text` to satisfy BOTH `whitespace_tokens <= max_tokens`
/// AND `ceil(char_count / 4) <= max_tokens` (hard max `max_tokens * 4` chars).
pub fn truncate_to_token_cap(text: &str, max_tokens: usize) -> (String, usize) {
    if max_tokens == 0 {
        return (String::new(), 0);
    }
    let max_chars = max_tokens.saturating_mul(TIER1A_CHARS_PER_TOKEN);
    let mut out = String::new();
    let mut char_len = 0usize;

    for word in text.split_whitespace().take(max_tokens) {
        let sep_chars = usize::from(!out.is_empty());
        if char_len + sep_chars >= max_chars {
            break;
        }
        let available = max_chars - char_len - sep_chars;
        if sep_chars == 1 {
            out.push(' ');
            char_len += 1;
        }
        let word_chars = word.chars().count();
        if word_chars <= available {
            out.push_str(word);
            char_len += word_chars;
        } else {
            for ch in word.chars().take(available) {
                out.push(ch);
            }
            char_len += available;
        }
        if char_len >= max_chars {
            break;
        }
    }

    let final_tokens = count_deterministic_tokens(&out);
    (out, final_tokens)
}

/// Sanitizes arbitrary user search input into a safe FTS5 MATCH expression.
/// Strips FTS5 syntax operators (`"`, `*`, `^`, `:`, `()`, `NEAR`, `AND`, `OR`, `NOT`) so malformed
/// queries never cause SQLite FTS5 syntax errors.
pub fn sanitize_fts5_query(raw: &str) -> String {
    let mut terms: Vec<String> = Vec::new();
    for segment in raw.split_whitespace() {
        let upper_seg = segment.to_ascii_uppercase();
        if upper_seg.starts_with("NEAR/") {
            continue;
        }
        let mut buf = String::new();
        for ch in segment.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                buf.push(ch);
            } else if !buf.is_empty() {
                push_safe_term(&mut terms, &mut buf);
            }
        }
        if !buf.is_empty() {
            push_safe_term(&mut terms, &mut buf);
        }
    }
    terms
        .into_iter()
        .take(16)
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_safe_term(terms: &mut Vec<String>, buf: &mut String) {
    let token = std::mem::take(buf);
    let upper = token.to_ascii_uppercase();
    if matches!(upper.as_str(), "AND" | "OR" | "NOT" | "NEAR") {
        return;
    }
    if !terms.iter().any(|t| t.eq_ignore_ascii_case(&token)) {
        terms.push(token);
    }
}

fn sqlite_sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut os = db_path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

fn reject_symlink_path(path: &Path) -> Result<(), ReaStoreError> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(ReaStoreError::SymlinkRejected(path.display().to_string()));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn verify_and_chmod_dir(path: &Path, mode: u32) -> Result<(), ReaStoreError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    reject_symlink_path(path)?;
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| ReaStoreError::SymlinkRejected(path.display().to_string()))?;
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if matches!(err.raw_os_error(), Some(libc::ELOOP) | Some(libc::ENOTDIR)) {
            return Err(ReaStoreError::SymlinkRejected(path.display().to_string()));
        }
        return Err(ReaStoreError::Io(err));
    }
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let fstat_rc = unsafe { libc::fstat(fd, &mut st) };
    if fstat_rc != 0 {
        let err = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(ReaStoreError::Io(err));
    }
    if (st.st_mode & libc::S_IFMT) != libc::S_IFDIR {
        unsafe { libc::close(fd) };
        return Err(ReaStoreError::SymlinkRejected(path.display().to_string()));
    }
    let chmod_rc = unsafe { libc::fchmod(fd, mode as libc::mode_t) };
    if chmod_rc != 0 {
        let err = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(ReaStoreError::Io(err));
    }
    unsafe {
        libc::close(fd);
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_and_chmod_dir(path: &Path, _mode: u32) -> Result<(), ReaStoreError> {
    reject_symlink_path(path)?;
    if !path.is_dir() {
        return Err(ReaStoreError::SymlinkRejected(path.display().to_string()));
    }
    Ok(())
}

#[cfg(unix)]
fn verify_and_chmod_regular_file(
    path: &Path,
    create: bool,
    mode: u32,
) -> Result<(), ReaStoreError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    reject_symlink_path(path)?;
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| ReaStoreError::SymlinkRejected(path.display().to_string()))?;
    let mut flags = libc::O_RDWR | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    if create {
        flags |= libc::O_CREAT;
    }
    let fd = unsafe { libc::open(c_path.as_ptr(), flags, mode as libc::c_uint) };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if !create && err.raw_os_error() == Some(libc::ENOENT) {
            return Ok(());
        }
        if err.raw_os_error() == Some(libc::ELOOP) {
            return Err(ReaStoreError::SymlinkRejected(path.display().to_string()));
        }
        return Err(ReaStoreError::Io(err));
    }
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let fstat_rc = unsafe { libc::fstat(fd, &mut st) };
    if fstat_rc != 0 {
        let err = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(ReaStoreError::Io(err));
    }
    if (st.st_mode & libc::S_IFMT) != libc::S_IFREG {
        unsafe { libc::close(fd) };
        return Err(ReaStoreError::SymlinkRejected(path.display().to_string()));
    }
    let chmod_rc = unsafe { libc::fchmod(fd, mode as libc::mode_t) };
    if chmod_rc != 0 {
        let err = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(ReaStoreError::Io(err));
    }
    unsafe {
        libc::close(fd);
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_and_chmod_regular_file(
    path: &Path,
    create: bool,
    _mode: u32,
) -> Result<(), ReaStoreError> {
    reject_symlink_path(path)?;
    if !path.exists() {
        if create {
            fs::OpenOptions::new().create(true).write(true).open(path)?;
        }
        return Ok(());
    }
    if !path.is_file() {
        return Err(ReaStoreError::SymlinkRejected(path.display().to_string()));
    }
    Ok(())
}

fn ensure_dir_no_symlink(dir: &Path) -> Result<(), ReaStoreError> {
    reject_symlink_path(dir)?;
    match fs::create_dir(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(ReaStoreError::Io(e)),
    }
}

fn validate_store_hierarchy(
    base_dir: &Path,
    db_path: &Path,
    create_if_missing: bool,
) -> Result<(), ReaStoreError> {
    let dot_dir = base_dir.join(".cowok-design");
    let cache_dir = dot_dir.join("cache");
    reject_symlink_path(base_dir)?;

    if create_if_missing {
        ensure_dir_no_symlink(&dot_dir)?;
        verify_and_chmod_dir(&dot_dir, 0o700)?;
        ensure_dir_no_symlink(&cache_dir)?;
        verify_and_chmod_dir(&cache_dir, 0o700)?;
    } else {
        verify_and_chmod_dir(&dot_dir, 0o700)?;
        verify_and_chmod_dir(&cache_dir, 0o700)?;
    }

    let canon_base = fs::canonicalize(base_dir)?;
    let canon_cache = fs::canonicalize(&cache_dir)?;
    if !canon_cache.starts_with(&canon_base) {
        return Err(ReaStoreError::SymlinkRejected(
            cache_dir.display().to_string(),
        ));
    }

    verify_and_chmod_regular_file(db_path, create_if_missing, 0o600)?;
    let wal_path = sqlite_sidecar_path(db_path, "-wal");
    let shm_path = sqlite_sidecar_path(db_path, "-shm");
    verify_and_chmod_regular_file(&wal_path, false, 0o600)?;
    verify_and_chmod_regular_file(&shm_path, false, 0o600)?;
    Ok(())
}

/// Dedicated Two-Store SQLite FTS5 reference cache (`<base_dir>/.cowok-design/cache/rea-reference.sqlite3`).
#[derive(Debug, Clone)]
pub struct ReaTwoStore {
    base_dir: PathBuf,
    db_path: PathBuf,
}

impl ReaTwoStore {
    pub fn resolve_db_path(base_dir: &Path) -> PathBuf {
        base_dir.join(CACHE_RELATIVE_DIR).join(CACHE_DB_FILENAME)
    }

    pub fn open(base_dir: &Path) -> Result<Self, ReaStoreError> {
        let db_path = Self::resolve_db_path(base_dir);
        validate_store_hierarchy(base_dir, &db_path, true)?;

        let store = Self {
            base_dir: base_dir.to_path_buf(),
            db_path,
        };
        let conn = store.connect()?;
        Self::init_schema(&conn)?;
        store.harden_sqlite_files()?;
        Ok(store)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn harden_sqlite_files(&self) -> Result<(), ReaStoreError> {
        verify_and_chmod_regular_file(&self.db_path, false, 0o600)?;
        let wal_path = sqlite_sidecar_path(&self.db_path, "-wal");
        let shm_path = sqlite_sidecar_path(&self.db_path, "-shm");
        verify_and_chmod_regular_file(&wal_path, false, 0o600)?;
        verify_and_chmod_regular_file(&shm_path, false, 0o600)?;
        Ok(())
    }

    fn connect(&self) -> Result<Connection, ReaStoreError> {
        validate_store_hierarchy(&self.base_dir, &self.db_path, false)?;
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;",
        )?;
        self.harden_sqlite_files()?;
        Ok(conn)
    }

    fn init_schema(conn: &Connection) -> Result<(), ReaStoreError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS rea_references (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                slug TEXT NOT NULL,
                source_url TEXT NOT NULL,
                source_format TEXT NOT NULL CHECK (source_format IN ('url','github','fig','clipboard','zip','pen','wire')),
                license_spdx TEXT NOT NULL,
                firewall_verdict TEXT NOT NULL CHECK (firewall_verdict IN ('ADOPT_PRIOR_ART','REFERENCE_ONLY','REJECT')),
                firewall_reason TEXT NOT NULL,
                tier1a_summary TEXT NOT NULL,
                tier1a_tokens INTEGER NOT NULL CHECK (tier1a_tokens <= 250),
                design_dna_json TEXT NOT NULL,
                raw_fields_json TEXT,
                proof_status TEXT NOT NULL DEFAULT 'awaiting-verification' CHECK (proof_status IN ('awaiting-verification','proof-verified')),
                sanitized_secrets_count INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                UNIQUE(tenant_id, slug)
            );

            CREATE INDEX IF NOT EXISTS idx_rea_references_tenant_created
                ON rea_references(tenant_id, created_at DESC);

            CREATE VIRTUAL TABLE IF NOT EXISTS rea_references_fts USING fts5(
                slug,
                tier1a_summary,
                design_dna_json,
                content='rea_references',
                content_rowid='rowid'
            );

            CREATE TRIGGER IF NOT EXISTS rea_references_ai AFTER INSERT ON rea_references BEGIN
                INSERT INTO rea_references_fts(rowid, slug, tier1a_summary, design_dna_json)
                VALUES (new.rowid, new.slug, new.tier1a_summary, new.design_dna_json);
            END;

            CREATE TRIGGER IF NOT EXISTS rea_references_ad AFTER DELETE ON rea_references BEGIN
                INSERT INTO rea_references_fts(rea_references_fts, rowid, slug, tier1a_summary, design_dna_json)
                VALUES ('delete', old.rowid, old.slug, old.tier1a_summary, old.design_dna_json);
            END;

            CREATE TRIGGER IF NOT EXISTS rea_references_au AFTER UPDATE ON rea_references BEGIN
                INSERT INTO rea_references_fts(rea_references_fts, rowid, slug, tier1a_summary, design_dna_json)
                VALUES ('delete', old.rowid, old.slug, old.tier1a_summary, old.design_dna_json);
                INSERT INTO rea_references_fts(rowid, slug, tier1a_summary, design_dna_json)
                VALUES (new.rowid, new.slug, new.tier1a_summary, new.design_dna_json);
            END;",
        )?;
        Ok(())
    }

    pub fn upsert_reference(
        &self,
        input: UpsertReaReference,
    ) -> Result<ReaReferenceRecord, ReaStoreError> {
        if input.firewall_verdict == FirewallVerdict::Reject {
            return Err(ReaStoreError::RejectedVerdictForbidden);
        }
        let (capped_summary, token_count) =
            truncate_to_token_cap(&input.tier1a_summary, TIER1A_MAX_TOKENS);
        let now = Utc::now().timestamp_millis();

        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let existing_id: Option<String> = tx
            .query_row(
                "SELECT id FROM rea_references WHERE tenant_id = ?1 AND slug = ?2",
                params![&input.tenant_id, &input.slug],
                |r| r.get(0),
            )
            .optional()?;

        let id = existing_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        tx.execute(
            "INSERT INTO rea_references (
                id, tenant_id, slug, source_url, source_format, license_spdx,
                firewall_verdict, firewall_reason, tier1a_summary, tier1a_tokens,
                design_dna_json, raw_fields_json, proof_status, sanitized_secrets_count, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(tenant_id, slug) DO UPDATE SET
                source_url = excluded.source_url,
                source_format = excluded.source_format,
                license_spdx = excluded.license_spdx,
                firewall_verdict = excluded.firewall_verdict,
                firewall_reason = excluded.firewall_reason,
                tier1a_summary = excluded.tier1a_summary,
                tier1a_tokens = excluded.tier1a_tokens,
                design_dna_json = excluded.design_dna_json,
                raw_fields_json = excluded.raw_fields_json,
                proof_status = excluded.proof_status,
                sanitized_secrets_count = excluded.sanitized_secrets_count,
                created_at = excluded.created_at",
            params![
                &id,
                &input.tenant_id,
                &input.slug,
                &input.source_url,
                &input.source_format,
                &input.license_spdx,
                input.firewall_verdict.as_str(),
                &input.firewall_reason,
                &capped_summary,
                token_count as i64,
                &input.design_dna_json,
                &input.raw_fields_json,
                input.proof_status.as_str(),
                input.sanitized_secrets_count as i64,
                now,
            ],
        )?;
        tx.commit()?;
        self.harden_sqlite_files()?;

        Ok(ReaReferenceRecord {
            id,
            tenant_id: input.tenant_id,
            slug: input.slug,
            source_url: input.source_url,
            source_format: input.source_format,
            license_spdx: input.license_spdx,
            firewall_verdict: input.firewall_verdict,
            firewall_reason: input.firewall_reason,
            tier1a_summary: capped_summary,
            tier1a_tokens: token_count,
            design_dna_json: input.design_dna_json,
            raw_fields_json: input.raw_fields_json,
            proof_status: input.proof_status,
            sanitized_secrets_count: input.sanitized_secrets_count,
            created_at: now,
        })
    }

    pub fn search(
        &self,
        tenant_id: &str,
        query: &str,
        tier: ReaSearchTier,
        limit: usize,
    ) -> Result<Vec<ReaSearchHit>, ReaStoreError> {
        let clamped_limit = limit.clamp(1, 100) as i64;
        let conn = self.connect()?;
        let sanitized_q = sanitize_fts5_query(query);
        let include_tier1b = tier == ReaSearchTier::Tier1b;

        if sanitized_q.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT id, tenant_id, slug, source_url, source_format, license_spdx,
                        firewall_verdict, firewall_reason, tier1a_summary, tier1a_tokens,
                        design_dna_json, raw_fields_json, proof_status, sanitized_secrets_count,
                        created_at
                 FROM rea_references
                 WHERE tenant_id = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![tenant_id, clamped_limit], |row| {
                row_to_search_hit(row, include_tier1b, None)
            })?;
            let mut hits = Vec::new();
            for item in rows {
                hits.push(item?);
            }
            return Ok(hits);
        }

        let mut stmt = conn.prepare(
            "SELECT r.id, r.tenant_id, r.slug, r.source_url, r.source_format, r.license_spdx,
                    r.firewall_verdict, r.firewall_reason, r.tier1a_summary, r.tier1a_tokens,
                    r.design_dna_json, r.raw_fields_json, r.proof_status, r.sanitized_secrets_count,
                    r.created_at, bm25(rea_references_fts) AS rank_score
             FROM rea_references_fts f
             JOIN rea_references r ON r.rowid = f.rowid
             WHERE r.tenant_id = ?1 AND rea_references_fts MATCH ?2
             ORDER BY rank_score ASC, r.created_at DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![tenant_id, sanitized_q, clamped_limit], |row| {
            let rank: f64 = row.get(15)?;
            row_to_search_hit(row, include_tier1b, Some(rank))
        })?;
        let mut hits = Vec::new();
        for item in rows {
            hits.push(item?);
        }
        Ok(hits)
    }

    pub fn get_by_id(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> Result<Option<ReaReferenceRecord>, ReaStoreError> {
        let conn = self.connect()?;
        let row = conn
            .query_row(
                "SELECT id, tenant_id, slug, source_url, source_format, license_spdx,
                        firewall_verdict, firewall_reason, tier1a_summary, tier1a_tokens,
                        design_dna_json, raw_fields_json, proof_status, sanitized_secrets_count,
                        created_at
                 FROM rea_references
                 WHERE tenant_id = ?1 AND id = ?2",
                params![tenant_id, id],
                row_to_record,
            )
            .optional()?;
        Ok(row)
    }

    pub fn search_awesome_re_catalog(
        &self,
        raw_query: &str,
        limit: usize,
    ) -> Result<crate::catalog::AwesomeReCatalogResponse, ReaStoreError> {
        let mut conn = self.connect()?;
        let res = crate::catalog::query_awesome_re_catalog(&mut conn, raw_query, limit)?;
        self.harden_sqlite_files()?;
        Ok(res)
    }

    pub fn update_proof_status(
        &self,
        tenant_id: &str,
        id: &str,
        proof_status: ProofStatus,
    ) -> Result<ReaReferenceRecord, ReaStoreError> {
        let mut conn = self.connect()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = tx.execute(
            "UPDATE rea_references SET proof_status = ?1 WHERE tenant_id = ?2 AND id = ?3",
            params![proof_status.as_str(), tenant_id, id],
        )?;
        if updated == 0 {
            return Err(ReaStoreError::NotFound(id.to_string()));
        }
        let rec = tx.query_row(
            "SELECT id, tenant_id, slug, source_url, source_format, license_spdx,
                    firewall_verdict, firewall_reason, tier1a_summary, tier1a_tokens,
                    design_dna_json, raw_fields_json, proof_status, sanitized_secrets_count,
                    created_at
             FROM rea_references
             WHERE tenant_id = ?1 AND id = ?2",
            params![tenant_id, id],
            row_to_record,
        )?;
        tx.commit()?;
        self.harden_sqlite_files()?;
        Ok(rec)
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReaReferenceRecord> {
    let verdict_str: String = row.get(6)?;
    let proof_str: String = row.get(12)?;
    let tokens_i64: i64 = row.get(9)?;
    let secrets_i64: i64 = row.get(13)?;
    Ok(ReaReferenceRecord {
        id: row.get(0)?,
        tenant_id: row.get(1)?,
        slug: row.get(2)?,
        source_url: row.get(3)?,
        source_format: row.get(4)?,
        license_spdx: row.get(5)?,
        firewall_verdict: FirewallVerdict::from_db_str(&verdict_str)
            .unwrap_or(FirewallVerdict::ReferenceOnly),
        firewall_reason: row.get(7)?,
        tier1a_summary: row.get(8)?,
        tier1a_tokens: tokens_i64.max(0) as usize,
        design_dna_json: row.get(10)?,
        raw_fields_json: row.get(11)?,
        proof_status: ProofStatus::from_db_str(&proof_str)
            .unwrap_or(ProofStatus::AwaitingVerification),
        sanitized_secrets_count: secrets_i64.max(0) as usize,
        created_at: row.get(14)?,
    })
}

fn row_to_search_hit(
    row: &rusqlite::Row<'_>,
    include_tier1b: bool,
    bm25_rank: Option<f64>,
) -> rusqlite::Result<ReaSearchHit> {
    let rec = row_to_record(row)?;
    let design_dna = if include_tier1b {
        serde_json::from_str(&rec.design_dna_json).ok()
    } else {
        None
    };
    let raw_fields = if include_tier1b {
        rec.raw_fields_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
    } else {
        None
    };
    Ok(ReaSearchHit {
        id: rec.id,
        tenant_id: rec.tenant_id,
        slug: rec.slug,
        source_url: rec.source_url,
        source_format: rec.source_format,
        license_spdx: rec.license_spdx,
        firewall_verdict: rec.firewall_verdict,
        firewall_reason: rec.firewall_reason,
        tier1a_summary: rec.tier1a_summary,
        tier1a_tokens: rec.tier1a_tokens,
        proof_status: rec.proof_status,
        sanitized_secrets_count: rec.sanitized_secrets_count,
        created_at: rec.created_at,
        design_dna,
        raw_fields,
        bm25_rank,
    })
}
