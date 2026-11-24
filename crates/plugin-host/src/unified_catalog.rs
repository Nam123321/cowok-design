//! Zero-Hardcode `UnifiedSkillCatalog` discovering and isolating skills across 4 sources:
//! 1. `packages/host-skills/` (`host_skills`)
//! 2. `packages/design-craft/` / `mount_default_design_craft_plugin()` (`design_craft`)
//! 3. `packages/builtin-skills/upstream-skills/` (`upstream_verbatim` — read-only MIT/Apache)
//! 4. SQLite `skill_plugins (lifecycle_state = 'ACTIVE')` + approved `knowledge_cards` (`sqlite_plugin`, `learn_card`)
//!
//! Enforces `O_NOFOLLOW`, canonical path-jail containment, per-plugin fault isolation,
//! deterministic `catalog_sha256` / `tier1_index_sha256`, Tier-3 lazy `references/*.md` loading,
//! and multilingual VI/EN BM25-style routing.

// @story 08.7
// @implements AC-01, AC-02, AC-05

use crate::default_mount::{mount_default_design_craft_plugin, mount_design_craft_bundle_at};
use crate::repackager::security_scanner::scan_skill_markdown_safety;
use crate::validator::{estimate_token_count, parse_and_validate_skill_frontmatter};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};

pub const HOST_SKILLS_RELATIVE_DIR: &str = "packages/host-skills";
pub const CANONICAL_DESIGN_CRAFT_RELATIVE_DIR: &str = "packages/design-craft";
pub const UPSTREAM_SKILLS_RELATIVE_DIR: &str = "packages/builtin-skills/upstream-skills";
pub const MAX_TIER1_SKILL_TOKENS: usize = 100;
pub const MAX_SKILL_BYTES: usize = 256 * 1024;
const MAX_SKILL_ID_LEN: usize = 128;
const MAX_SKILL_NAME_LEN: usize = 128;
const MAX_SKILL_VERSION_LEN: usize = 32;
const MAX_SKILL_DESC_LEN: usize = 600;
const MAX_SKILL_TAGS: usize = 24;
const MAX_REFERENCE_PATHS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSourceBucket {
    DesignCraft,
    HostSkills,
    UpstreamVerbatim,
    SqlitePlugin,
    LearnCard,
}

impl SkillSourceBucket {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DesignCraft => "design_craft",
            Self::HostSkills => "host_skills",
            Self::UpstreamVerbatim => "upstream_verbatim",
            Self::SqlitePlugin => "sqlite_plugin",
            Self::LearnCard => "learn_card",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedSkillEntry {
    pub skill_id: String,
    pub name: String,
    pub version: String,
    pub bucket: SkillSourceBucket,
    pub description: String,
    pub tags: Vec<String>,
    pub reference_paths: Vec<String>,
    pub tier1_token_estimate: usize,
    pub content_sha256: String,
    #[serde(skip)]
    pub source_root: Option<PathBuf>,
    #[serde(skip)]
    pub skill_md_rel_path: Option<String>,
    #[serde(skip)]
    pub inline_body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedSkillCatalogSnapshot {
    pub catalog_sha256: String,
    pub tier1_index_sha256: String,
    pub total_skills: usize,
    pub bucket_counts: BTreeMap<SkillSourceBucket, usize>,
    pub skills: Vec<UnifiedSkillEntry>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMatchResult {
    pub skill_id: String,
    pub bucket: SkillSourceBucket,
    pub score: f64,
    pub matched_terms: Vec<String>,
}

#[derive(Debug, Error)]
pub enum UnifiedCatalogError {
    #[error("skill not found in catalog: {0}")]
    SkillNotFound(String),
    #[error("symlink rejected under Zero-Trust path jail: {0}")]
    SymlinkRejected(PathBuf),
    #[error("hardlink rejected under Zero-Trust path jail (nlink != 1): {0}")]
    HardlinkRejected(PathBuf),
    #[error("skill file exceeds {max_bytes} byte ceiling: {path}")]
    FileTooLarge { path: PathBuf, max_bytes: usize },
    #[error("path traversal rejected: {0}")]
    PathTraversal(String),
    #[error("reference file not found: {0}")]
    ReferenceNotFound(String),
    #[error("skill integrity SHA-256 mismatch on reload: {skill_id}")]
    IntegrityMismatch { skill_id: String },
    #[error("skill safety check rejected '{skill_id}': {reason}")]
    SafetyViolation { skill_id: String, reason: String },
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct UnifiedSkillCatalog {
    entries: Vec<UnifiedSkillEntry>,
    warnings: Vec<String>,
    catalog_sha256: String,
    zone1_tier1_index: String,
    tier1_index_sha256: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    s.trim().chars().take(max_chars).collect()
}

fn extract_safe_relative_segments(rel: &str) -> Result<Vec<String>, UnifiedCatalogError> {
    if rel.is_empty() || rel.contains('\\') || rel.starts_with('/') || rel.contains('\0') {
        return Err(UnifiedCatalogError::PathTraversal(rel.to_string()));
    }
    let mut segments = Vec::new();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(seg) => {
                let s = seg
                    .to_str()
                    .ok_or_else(|| UnifiedCatalogError::PathTraversal(rel.to_string()))?;
                if s.is_empty() || s == "." || s == ".." || s.contains('\0') {
                    return Err(UnifiedCatalogError::PathTraversal(rel.to_string()));
                }
                segments.push(s.to_string());
            }
            _ => return Err(UnifiedCatalogError::PathTraversal(rel.to_string())),
        }
    }
    if segments.is_empty() {
        return Err(UnifiedCatalogError::PathTraversal(rel.to_string()));
    }
    Ok(segments)
}

#[cfg(unix)]
fn open_dir_nofollow_root(dir: &Path) -> Result<OwnedFd, UnifiedCatalogError> {
    let meta = std::fs::symlink_metadata(dir).map_err(|source| UnifiedCatalogError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    if meta.file_type().is_symlink() {
        return Err(UnifiedCatalogError::SymlinkRejected(dir.to_path_buf()));
    }
    let c_dir = CString::new(dir.as_os_str().as_bytes())
        .map_err(|_| UnifiedCatalogError::PathTraversal(dir.display().to_string()))?;
    // SAFETY: c_dir is a valid NUL-terminated CString; O_NOFOLLOW | O_DIRECTORY | O_CLOEXEC rejects symlinks atomically.
    let fd = unsafe {
        libc::open(
            c_dir.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        return match err.raw_os_error() {
            Some(libc::ELOOP) => Err(UnifiedCatalogError::SymlinkRejected(dir.to_path_buf())),
            Some(libc::ENOENT) => Err(UnifiedCatalogError::ReferenceNotFound(
                dir.display().to_string(),
            )),
            _ => Err(UnifiedCatalogError::Io {
                path: dir.to_path_buf(),
                source: err,
            }),
        };
    }
    // SAFETY: fd is a freshly opened valid descriptor owned by OwnedFd.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[cfg(unix)]
fn openat_dir_nofollow(
    parent_fd: &OwnedFd,
    segment: &str,
    display_path: &Path,
) -> Result<OwnedFd, UnifiedCatalogError> {
    let c_seg = CString::new(segment.as_bytes())
        .map_err(|_| UnifiedCatalogError::PathTraversal(segment.to_string()))?;
    // SAFETY: parent_fd is a valid directory FD and c_seg is NUL-terminated; O_NOFOLLOW | O_DIRECTORY prevents TOCTOU symlink traversal.
    let fd = unsafe {
        libc::openat(
            parent_fd.as_raw_fd(),
            c_seg.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        return match err.raw_os_error() {
            Some(libc::ELOOP) => Err(UnifiedCatalogError::SymlinkRejected(
                display_path.to_path_buf(),
            )),
            Some(libc::ENOENT) => Err(UnifiedCatalogError::ReferenceNotFound(
                display_path.display().to_string(),
            )),
            Some(libc::ENOTDIR) => Err(UnifiedCatalogError::ReferenceNotFound(
                display_path.display().to_string(),
            )),
            _ => Err(UnifiedCatalogError::Io {
                path: display_path.to_path_buf(),
                source: err,
            }),
        };
    }
    // SAFETY: fd is a freshly opened valid descriptor owned by OwnedFd.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[cfg(unix)]
fn open_confined_file_stepwise_nofollow(
    root_dir: &Path,
    rel: &str,
) -> Result<File, UnifiedCatalogError> {
    let segments = extract_safe_relative_segments(rel)?;
    let (file_name, parent_dirs) = segments
        .split_last()
        .ok_or_else(|| UnifiedCatalogError::PathTraversal(rel.to_string()))?;

    let mut dir_fd = open_dir_nofollow_root(root_dir)?;
    let mut cursor = root_dir.to_path_buf();
    for seg in parent_dirs {
        cursor.push(seg);
        dir_fd = openat_dir_nofollow(&dir_fd, seg, &cursor)?;
    }
    cursor.push(file_name);

    let c_file = CString::new(file_name.as_bytes())
        .map_err(|_| UnifiedCatalogError::PathTraversal(rel.to_string()))?;
    // SAFETY: dir_fd is an O_NOFOLLOW directory FD and c_file is NUL-terminated; O_NOFOLLOW rejects symlink target atomically.
    let fd = unsafe {
        libc::openat(
            dir_fd.as_raw_fd(),
            c_file.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        return match err.raw_os_error() {
            Some(libc::ELOOP) => Err(UnifiedCatalogError::SymlinkRejected(cursor)),
            Some(libc::ENOENT) => Err(UnifiedCatalogError::ReferenceNotFound(rel.to_string())),
            _ => Err(UnifiedCatalogError::Io {
                path: cursor,
                source: err,
            }),
        };
    }
    // SAFETY: fd is a freshly opened valid descriptor owned by File.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(not(unix))]
fn open_confined_file_stepwise_nofollow(
    root_dir: &Path,
    rel: &str,
) -> Result<File, UnifiedCatalogError> {
    let segments = extract_safe_relative_segments(rel)?;
    let canonical_root =
        std::fs::canonicalize(root_dir).map_err(|source| UnifiedCatalogError::Io {
            path: root_dir.to_path_buf(),
            source,
        })?;
    let mut cursor = root_dir.to_path_buf();
    for seg in &segments {
        cursor.push(seg);
        let meta = match std::fs::symlink_metadata(&cursor) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(UnifiedCatalogError::ReferenceNotFound(rel.to_string()));
            }
            Err(source) => {
                return Err(UnifiedCatalogError::Io {
                    path: cursor,
                    source,
                });
            }
        };
        if meta.file_type().is_symlink() {
            return Err(UnifiedCatalogError::SymlinkRejected(cursor));
        }
    }
    let canonical = std::fs::canonicalize(&cursor).map_err(|source| UnifiedCatalogError::Io {
        path: cursor.clone(),
        source,
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(UnifiedCatalogError::SymlinkRejected(cursor));
    }
    File::open(&cursor).map_err(|source| UnifiedCatalogError::Io {
        path: cursor,
        source,
    })
}

/// Reads a confined file beneath `root_dir` via stepwise POSIX `openat(O_NOFOLLOW)`:
/// - Zero path-TOCTOU symlink escape across intermediate directories or leaf file
/// - Rejects non-regular files and hardlinks (`nlink() != 1` on Unix)
/// - Enforces strict `MAX_SKILL_BYTES` (`256 KiB`) ceiling via open-FD `fstat` + bounded `.take(MAX_SKILL_BYTES + 1)`
fn read_confined_file_nofollow_bounded(
    root_dir: &Path,
    rel: &str,
) -> Result<String, UnifiedCatalogError> {
    let display_path = root_dir.join(rel);
    let file = open_confined_file_stepwise_nofollow(root_dir, rel)?;
    let meta = file.metadata().map_err(|source| UnifiedCatalogError::Io {
        path: display_path.clone(),
        source,
    })?;
    if !meta.file_type().is_file() {
        return Err(UnifiedCatalogError::ReferenceNotFound(rel.to_string()));
    }
    #[cfg(unix)]
    if meta.nlink() != 1 {
        return Err(UnifiedCatalogError::HardlinkRejected(display_path));
    }
    if meta.len() > MAX_SKILL_BYTES as u64 {
        return Err(UnifiedCatalogError::FileTooLarge {
            path: display_path,
            max_bytes: MAX_SKILL_BYTES,
        });
    }
    let mut bytes = Vec::with_capacity((meta.len() as usize).min(MAX_SKILL_BYTES));
    file.take((MAX_SKILL_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|source| UnifiedCatalogError::Io {
            path: display_path.clone(),
            source,
        })?;
    if bytes.len() > MAX_SKILL_BYTES {
        return Err(UnifiedCatalogError::FileTooLarge {
            path: display_path,
            max_bytes: MAX_SKILL_BYTES,
        });
    }
    String::from_utf8(bytes).map_err(|e| UnifiedCatalogError::Io {
        path: display_path,
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })
}

fn collect_skill_md_paths_isolated(
    dir: &Path,
    canonical_root: &Path,
    acc: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) {
    let dir_meta = match std::fs::symlink_metadata(dir) {
        Ok(m) => m,
        Err(e) => {
            warnings.push(format!("Skipped unreadable dir '{}': {e}", dir.display()));
            return;
        }
    };
    if dir_meta.file_type().is_symlink() {
        warnings.push(format!(
            "Symlink directory rejected by path jail: {}",
            dir.display()
        ));
        return;
    }
    let Ok(canonical_dir) = std::fs::canonicalize(dir) else {
        warnings.push(format!("Failed to canonicalize dir: {}", dir.display()));
        return;
    };
    if !canonical_dir.starts_with(canonical_root) {
        warnings.push(format!(
            "Directory escaped canonical root jail: {}",
            dir.display()
        ));
        return;
    }
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        warnings.push(format!("Cannot enumerate directory: {}", dir.display()));
        return;
    };
    let mut entries: Vec<PathBuf> = read_dir.flatten().map(|e| e.path()).collect();
    entries.sort();
    for entry in entries {
        let Ok(meta) = std::fs::symlink_metadata(&entry) else {
            warnings.push(format!("Cannot stat path: {}", entry.display()));
            continue;
        };
        if meta.file_type().is_symlink() {
            warnings.push(format!(
                "Symlink entry rejected by path jail: {}",
                entry.display()
            ));
            continue;
        }
        if meta.is_dir() {
            if entry
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "node_modules" || n == ".git" || n == "references")
            {
                continue;
            }
            collect_skill_md_paths_isolated(&entry, canonical_root, acc, warnings);
        } else if meta.is_file() && entry.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
            match std::fs::canonicalize(&entry) {
                Ok(c) if c.starts_with(canonical_root) => acc.push(entry),
                _ => warnings.push(format!(
                    "SKILL.md escaped canonical root jail: {}",
                    entry.display()
                )),
            }
        }
    }
}

fn discover_reference_paths(
    skill_dir: &Path,
    canonical_root: &Path,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let refs_dir = skill_dir.join("references");
    let Ok(meta) = std::fs::symlink_metadata(&refs_dir) else {
        return Vec::new();
    };
    if meta.file_type().is_symlink() {
        warnings.push(format!(
            "Rejected symlink references/ directory: {}",
            refs_dir.display()
        ));
        return Vec::new();
    }
    if !meta.is_dir() {
        return Vec::new();
    }
    let Ok(canonical_refs) = std::fs::canonicalize(&refs_dir) else {
        return Vec::new();
    };
    if !canonical_refs.starts_with(canonical_root) {
        warnings.push(format!(
            "references/ escaped canonical root: {}",
            refs_dir.display()
        ));
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&refs_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        let Ok(m) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if m.file_type().is_symlink() {
            warnings.push(format!("Rejected symlink reference file: {}", p.display()));
            continue;
        }
        #[cfg(unix)]
        if m.nlink() != 1 {
            warnings.push(format!(
                "Rejected hardlinked reference file: {}",
                p.display()
            ));
            continue;
        }
        if m.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Some(fname) = p.file_name().and_then(|n| n.to_str()) {
                let rel_ref = format!("references/{fname}");
                if rel_ref.len() <= 256 {
                    out.push(rel_ref);
                }
            }
        }
    }
    out.sort();
    out.truncate(MAX_REFERENCE_PATHS);
    out
}

fn format_tier1_line(
    skill_id: &str,
    bucket: SkillSourceBucket,
    version: &str,
    desc: &str,
) -> String {
    format!(
        "- {} [{}@v{}]: {}",
        skill_id,
        bucket.as_str(),
        version,
        desc
    )
}

/// Enforces strict Zod-aligned Rust length bounds and actual rendered Tier-1 token ceiling (`<= 100` tokens)
/// without silent clamping: if a skill's description pushes its rendered Tier-1 line over 100 tokens,
/// truncates cleanly word-by-word and records a diagnostic warning.
fn fit_tier1_description(
    skill_id: &str,
    bucket: SkillSourceBucket,
    version: &str,
    raw_desc: &str,
    warnings: &mut Vec<String>,
) -> (String, usize) {
    let compact: String = raw_desc
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_SKILL_DESC_LEN)
        .collect();
    let compact = if compact.is_empty() {
        format!("Skill {skill_id}")
    } else {
        compact
    };

    let full_line = format_tier1_line(skill_id, bucket, version, &compact);
    let raw_tokens = estimate_token_count(&full_line);
    if raw_tokens <= MAX_TIER1_SKILL_TOKENS {
        return (compact, raw_tokens.max(1));
    }

    warnings.push(format!(
        "Tier-1 metadata for '{}' exceeded {} tokens ({} tok); truncated cleanly to fit budget",
        skill_id, MAX_TIER1_SKILL_TOKENS, raw_tokens
    ));

    let words: Vec<&str> = compact.split_whitespace().collect();
    let mut kept = String::new();
    for word in words {
        let candidate = if kept.is_empty() {
            word.to_string()
        } else {
            format!("{kept} {word}")
        };
        let line = format_tier1_line(skill_id, bucket, version, &candidate);
        if estimate_token_count(&line) <= MAX_TIER1_SKILL_TOKENS {
            kept = candidate;
        } else {
            break;
        }
    }
    if kept.is_empty() {
        // Fallback character truncation if single long token exceeded budget
        let prefix_len = format_tier1_line(skill_id, bucket, version, "").len();
        let avail = (MAX_TIER1_SKILL_TOKENS * 4)
            .saturating_sub(prefix_len + 4)
            .max(16);
        kept = compact.chars().take(avail).collect();
    }
    let final_line = format_tier1_line(skill_id, bucket, version, &kept);
    let final_tokens = estimate_token_count(&final_line).max(1);
    (kept, final_tokens)
}

fn parse_lenient_upstream_frontmatter(
    raw: &str,
    fallback_name: &str,
) -> (String, String, String, Vec<String>) {
    let stripped = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
    let mut name = truncate_chars(fallback_name, MAX_SKILL_NAME_LEN);
    let mut version = "1.0.0".to_string();
    let mut description = format!("Upstream skill {name}");
    let mut tags: Vec<String> = Vec::new();

    if let Some(rest) = stripped
        .strip_prefix("---\r\n")
        .or_else(|| stripped.strip_prefix("---\n"))
    {
        if let Some(end_idx) = rest.find("\n---") {
            let yaml_block = &rest[..end_idx];
            if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(yaml_block) {
                if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
                    let t = truncate_chars(n, MAX_SKILL_NAME_LEN);
                    if !t.is_empty() {
                        name = t;
                    }
                }
                if let Some(v) = val.get("version").and_then(|v| v.as_str()) {
                    let t = truncate_chars(v, MAX_SKILL_VERSION_LEN);
                    if !t.is_empty() {
                        version = t;
                    }
                }
                if let Some(d) = val.get("description").and_then(|v| v.as_str()) {
                    let compact: String = d.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !compact.is_empty() {
                        description = compact.chars().take(MAX_SKILL_DESC_LEN).collect();
                    }
                }
                if let Some(seq) = val.get("tags").and_then(|v| v.as_sequence()) {
                    for item in seq {
                        if let Some(s) = item.as_str() {
                            let t = s.trim();
                            if !t.is_empty() && t.len() <= 48 && tags.len() < MAX_SKILL_TAGS {
                                tags.push(t.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    (name, version, description, tags)
}

fn infer_default_tags(skill_id: &str, description: &str, bucket: SkillSourceBucket) -> Vec<String> {
    let mut tags = BTreeSet::new();
    tags.insert(bucket.as_str().to_string());
    for part in skill_id.split(['-', '_']) {
        let p = part.trim().to_ascii_lowercase();
        if p.len() >= 2 && p.len() <= 48 {
            tags.insert(p);
        }
    }
    let lower = description.to_ascii_lowercase();
    for kw in [
        "chart",
        "dashboard",
        "slide",
        "oklch",
        "typography",
        "diagram",
        "a11y",
        "wcag",
        "i18n",
        "wireframe",
        "storytelling",
        "kpi",
        "polish",
        "audit",
        "state",
    ] {
        if lower.contains(kw) {
            tags.insert(kw.to_string());
        }
    }
    tags.into_iter().take(24).collect()
}

fn scan_directory_bucket(
    bucket_root: &Path,
    bucket: SkillSourceBucket,
    strict_frontmatter: bool,
    entries_by_id: &mut BTreeMap<String, UnifiedSkillEntry>,
    warnings: &mut Vec<String>,
) {
    let Ok(meta) = std::fs::symlink_metadata(bucket_root) else {
        return;
    };
    if meta.file_type().is_symlink() {
        warnings.push(format!(
            "Bucket root symlink rejected: {}",
            bucket_root.display()
        ));
        return;
    }
    if !meta.is_dir() {
        return;
    }
    let Ok(canonical_root) = std::fs::canonicalize(bucket_root) else {
        warnings.push(format!(
            "Cannot canonicalize bucket root: {}",
            bucket_root.display()
        ));
        return;
    };

    let mut skill_files = Vec::new();
    collect_skill_md_paths_isolated(bucket_root, &canonical_root, &mut skill_files, warnings);

    for skill_md_path in skill_files {
        let rel = skill_md_path
            .strip_prefix(bucket_root)
            .unwrap_or(&skill_md_path)
            .to_string_lossy()
            .replace('\\', "/");
        let raw = match read_confined_file_nofollow_bounded(bucket_root, &rel) {
            Ok(text) => text,
            Err(e) => {
                warnings.push(format!("Isolated failed skill '{rel}': {e}"));
                continue;
            }
        };
        if let Err(e) = scan_skill_markdown_safety(&rel, &raw) {
            warnings.push(format!("Isolated unsafe skill '{rel}' ({bucket:?}): {e}"));
            continue;
        }
        let fallback_name = skill_md_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed-skill")
            .to_string();

        let (name, version, raw_description, mut tags) = if strict_frontmatter {
            match parse_and_validate_skill_frontmatter(&raw) {
                Ok(fm) => (
                    truncate_chars(&fm.name, MAX_SKILL_NAME_LEN),
                    truncate_chars(
                        fm.version.as_deref().unwrap_or("1.0.0"),
                        MAX_SKILL_VERSION_LEN,
                    ),
                    fm.description,
                    fm.tags
                        .into_iter()
                        .filter(|t| !t.trim().is_empty() && t.len() <= 48)
                        .take(MAX_SKILL_TAGS)
                        .collect(),
                ),
                Err(e) => {
                    warnings.push(format!("Isolated invalid frontmatter in '{rel}': {e}"));
                    continue;
                }
            }
        } else {
            parse_lenient_upstream_frontmatter(&raw, &fallback_name)
        };

        let skill_id = truncate_chars(&name, MAX_SKILL_ID_LEN);
        let version = if version.is_empty() {
            "1.0.0".to_string()
        } else {
            version
        };
        let (description, tier1_tokens) =
            fit_tier1_description(&skill_id, bucket, &version, &raw_description, warnings);

        if tags.is_empty() {
            tags = infer_default_tags(&name, &description, bucket);
        }
        let skill_dir = skill_md_path.parent().unwrap_or(bucket_root);
        let reference_paths = discover_reference_paths(skill_dir, &canonical_root, warnings);
        let content_sha256 = sha256_hex(raw.as_bytes());

        entries_by_id
            .entry(skill_id.clone())
            .or_insert_with(|| UnifiedSkillEntry {
                skill_id,
                name,
                version,
                bucket,
                description,
                tags,
                reference_paths,
                tier1_token_estimate: tier1_tokens,
                content_sha256,
                source_root: Some(skill_dir.to_path_buf()),
                skill_md_rel_path: Some("SKILL.md".to_string()),
                inline_body: None,
            });
    }
}

fn load_sqlite_active_plugins_and_learn_cards(
    conn: &Connection,
    tenant_id: &str,
    project_id: Option<&str>,
    entries_by_id: &mut BTreeMap<String, UnifiedSkillEntry>,
    warnings: &mut Vec<String>,
) {
    // 1. Active SQLite skill_plugins (excluding quarantined)
    let plugin_sql = if project_id.is_some() {
        "SELECT plugin_slug, version, manifest_json, COALESCE(staging_package_sha256, '') \
         FROM skill_plugins \
         WHERE tenant_id = ?1 AND project_id = ?2 AND lifecycle_state = 'ACTIVE' AND quarantined = 0 \
         ORDER BY plugin_slug ASC"
    } else {
        "SELECT plugin_slug, version, manifest_json, COALESCE(staging_package_sha256, '') \
         FROM skill_plugins \
         WHERE tenant_id = ?1 AND lifecycle_state = 'ACTIVE' AND quarantined = 0 \
         ORDER BY plugin_slug ASC"
    };

    match conn.prepare(plugin_sql) {
        Ok(mut stmt) => {
            let map_row =
                |r: &rusqlite::Row<'_>| -> rusqlite::Result<(String, String, String, String)> {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                };
            let rows_result: rusqlite::Result<Vec<(String, String, String, String)>> =
                if let Some(pid) = project_id {
                    stmt.query_map(params![tenant_id, pid], map_row)
                        .map(|iter| iter.flatten().collect())
                } else {
                    stmt.query_map(params![tenant_id], map_row)
                        .map(|iter| iter.flatten().collect())
                };
            match rows_result {
                Ok(rows) => {
                    for (slug, version, manifest_json, pkg_sha) in rows {
                        let parsed = serde_json::from_str::<serde_json::Value>(&manifest_json)
                            .unwrap_or_else(|_| serde_json::json!({}));
                        let description = parsed
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(|s| s.trim().chars().take(600).collect::<String>())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| format!("Installed active plugin '{slug}'"));
                        let tags: Vec<String> = parsed
                            .get("tags")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty() && s.len() <= 48)
                                    .take(24)
                                    .collect()
                            })
                            .filter(|v: &Vec<String>| !v.is_empty())
                            .unwrap_or_else(|| {
                                infer_default_tags(
                                    &slug,
                                    &description,
                                    SkillSourceBucket::SqlitePlugin,
                                )
                            });
                        let content_sha256 = if pkg_sha.len() == 64
                            && pkg_sha.bytes().all(|b| b.is_ascii_hexdigit())
                        {
                            pkg_sha.to_ascii_lowercase()
                        } else {
                            sha256_hex(format!("{slug}:{version}:{manifest_json}").as_bytes())
                        };
                        let safe_name = truncate_chars(&slug, MAX_SKILL_NAME_LEN);
                        let safe_version = {
                            let v = truncate_chars(&version, MAX_SKILL_VERSION_LEN);
                            if v.is_empty() {
                                "1.0.0".to_string()
                            } else {
                                v
                            }
                        };
                        let skill_id =
                            truncate_chars(&format!("plugin:{safe_name}"), MAX_SKILL_ID_LEN);
                        let (fitted_desc, tier1_tokens) = fit_tier1_description(
                            &skill_id,
                            SkillSourceBucket::SqlitePlugin,
                            &safe_version,
                            &description,
                            warnings,
                        );
                        entries_by_id.insert(
                            skill_id.clone(),
                            UnifiedSkillEntry {
                                skill_id,
                                name: safe_name,
                                version: safe_version,
                                bucket: SkillSourceBucket::SqlitePlugin,
                                description: fitted_desc,
                                tags,
                                reference_paths: Vec::new(),
                                tier1_token_estimate: tier1_tokens,
                                content_sha256,
                                source_root: None,
                                skill_md_rel_path: None,
                                inline_body: Some(manifest_json),
                            },
                        );
                    }
                }
                Err(e) => warnings.push(format!("SQLite skill_plugins query warning: {e}")),
            }
        }
        Err(e) => warnings.push(format!("SQLite skill_plugins unavailable: {e}")),
    }

    // 2. Approved (ACCEPTED) HUMAN_ONLY LEARN knowledge_cards
    let card_sql = if project_id.is_some() {
        "SELECT id, category, title, content, content_sha256 \
         FROM knowledge_cards \
         WHERE tenant_id = ?1 AND project_id = ?2 AND status = 'ACCEPTED' \
         ORDER BY id ASC"
    } else {
        "SELECT id, category, title, content, content_sha256 \
         FROM knowledge_cards \
         WHERE tenant_id = ?1 AND status = 'ACCEPTED' \
         ORDER BY id ASC"
    };

    match conn.prepare(card_sql) {
        Ok(mut stmt) => {
            type LearnCardRow = (String, String, String, String, String);
            let map_card = |r: &rusqlite::Row<'_>| -> rusqlite::Result<LearnCardRow> {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            };
            let rows_result: rusqlite::Result<Vec<LearnCardRow>> = if let Some(pid) = project_id {
                stmt.query_map(params![tenant_id, pid], map_card)
                    .map(|iter| iter.flatten().collect())
            } else {
                stmt.query_map(params![tenant_id], map_card)
                    .map(|iter| iter.flatten().collect())
            };
            match rows_result {
                Ok(rows) => {
                    for (card_id, category, title, content, content_sha) in rows {
                        let digest = if content_sha.len() == 64
                            && content_sha.bytes().all(|b| b.is_ascii_hexdigit())
                        {
                            content_sha.to_ascii_lowercase()
                        } else {
                            sha256_hex(content.as_bytes())
                        };
                        let raw_desc = format!("[{category}] {title}");
                        let safe_name = truncate_chars(&title, MAX_SKILL_NAME_LEN);
                        let skill_id =
                            truncate_chars(&format!("learn:{card_id}"), MAX_SKILL_ID_LEN);
                        let (desc, tier1_tokens) = fit_tier1_description(
                            &skill_id,
                            SkillSourceBucket::LearnCard,
                            "1.0.0",
                            &raw_desc,
                            warnings,
                        );
                        entries_by_id.insert(
                            skill_id.clone(),
                            UnifiedSkillEntry {
                                skill_id,
                                name: safe_name,
                                version: "1.0.0".to_string(),
                                bucket: SkillSourceBucket::LearnCard,
                                description: desc,
                                tags: vec!["learn".to_string(), category.to_ascii_lowercase()],
                                reference_paths: Vec::new(),
                                tier1_token_estimate: tier1_tokens,
                                content_sha256: digest,
                                source_root: None,
                                skill_md_rel_path: None,
                                inline_body: Some(content),
                            },
                        );
                    }
                }
                Err(e) => warnings.push(format!("SQLite knowledge_cards query warning: {e}")),
            }
        }
        Err(e) => warnings.push(format!("SQLite knowledge_cards unavailable: {e}")),
    }
}

fn render_deterministic_tier1_index(entries: &[UnifiedSkillEntry]) -> String {
    let mut lines = Vec::with_capacity(entries.len() + 1);
    lines.push("# Unified Tier-1 Skill Index (Deterministic Sorted <= 100 tok/skill)".to_string());
    for e in entries {
        lines.push(format_tier1_line(
            &e.skill_id,
            e.bucket,
            &e.version,
            &e.description,
        ));
    }
    lines.join("\n")
}

fn compute_catalog_sha256(entries: &[UnifiedSkillEntry]) -> String {
    let mut hasher = Sha256::new();
    for e in entries {
        hasher.update(e.skill_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(e.version.as_bytes());
        hasher.update(b"\0");
        hasher.update(e.bucket.as_str().as_bytes());
        hasher.update(b"\0");
        hasher.update(e.content_sha256.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// Strip common Vietnamese diacritics so both accented and unaccented queries match cleanly.
fn fold_vietnamese_ascii(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let mapped = match ch {
            'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ' | 'ầ'
            | 'ẩ' | 'ẫ' | 'ậ' | 'Á' | 'À' | 'Ả' | 'Ã' | 'Ạ' | 'Ă' | 'Ắ' | 'Ằ' | 'Ẳ' | 'Ẵ' | 'Ặ'
            | 'Â' | 'Ấ' | 'Ầ' | 'Ẩ' | 'Ẫ' | 'Ậ' => 'a',
            'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' | 'É' | 'È' | 'Ẻ'
            | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ế' | 'Ề' | 'Ể' | 'Ễ' | 'Ệ' => 'e',
            'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' | 'Í' | 'Ì' | 'Ỉ' | 'Ĩ' | 'Ị' => 'i',
            'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ' | 'ờ'
            | 'ở' | 'ỡ' | 'ợ' | 'Ó' | 'Ò' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ố' | 'Ồ' | 'Ổ' | 'Ỗ' | 'Ộ'
            | 'Ơ' | 'Ớ' | 'Ờ' | 'Ở' | 'Ỡ' | 'Ợ' => 'o',
            'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' | 'Ú' | 'Ù' | 'Ủ'
            | 'Ũ' | 'Ụ' | 'Ư' | 'Ứ' | 'Ừ' | 'Ử' | 'Ữ' | 'Ự' => 'u',
            'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' | 'Ý' | 'Ỳ' | 'Ỷ' | 'Ỹ' | 'Ỵ' => 'y',
            'đ' | 'Đ' => 'd',
            other => other.to_ascii_lowercase(),
        };
        out.push(mapped);
    }
    out
}

fn tokenize_folded(text: &str) -> Vec<String> {
    fold_vietnamese_ascii(text)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| s.len() >= 2)
        .map(str::to_string)
        .collect()
}

/// Expands Vietnamese and English design/data storytelling phrases into canonical routing terms.
fn expand_multilingual_query_terms(query: &str) -> Vec<String> {
    let folded = fold_vietnamese_ascii(query);
    let mut terms: Vec<String> = tokenize_folded(query);

    const PHRASE_MAP: &[(&str, &[&str])] = &[
        (
            "bieu do",
            &["chart", "data", "viz", "storyteller", "dashboard", "kpi"],
        ),
        (
            "ke chuyen du lieu",
            &[
                "data",
                "viz",
                "storyteller",
                "storytelling",
                "narrative",
                "chart",
            ],
        ),
        (
            "bang dieu khien",
            &["dashboard", "design", "ux", "kpi", "table", "drawer"],
        ),
        (
            "thuyet trinh",
            &["slide", "stageshow", "presentation", "deck", "storyteller"],
        ),
        (
            "trinh chieu",
            &["slide", "stageshow", "presentation", "deck"],
        ),
        ("slide", &["stageshow", "slide", "presentation", "deck"]),
        (
            "phac thao",
            &["sketch", "monochrome", "wireframe", "structural"],
        ),
        ("mau sac", &["oklch", "palette", "color", "theme", "token"]),
        ("bang mau", &["oklch", "palette", "color", "theme"]),
        ("kieu chu", &["typography", "font", "scale", "numerals"]),
        ("chu viet", &["typography", "microcopy", "i18n"]),
        ("so do", &["archify", "diagram", "svg", "flow", "topology"]),
        (
            "kien truc",
            &["archify", "diagram", "architecture", "topology"],
        ),
        (
            "trang thai",
            &["flow", "state", "completer", "loading", "empty", "error"],
        ),
        (
            "kiem thu",
            &["eval", "gate", "audit", "verify", "simulation"],
        ),
        ("nguoi dung", &["user", "simulation", "ux", "persona"]),
        ("truy cap", &["a11y", "wcag", "responsive", "contrast"]),
        ("viet hoa", &["i18n", "microcopy", "ux"]),
        ("cham chut", &["impeccable", "polish", "craft", "slop"]),
        (
            "doanh thu",
            &["chart", "data", "viz", "storyteller", "kpi", "dashboard"],
        ),
        ("chi so", &["kpi", "dashboard", "chart", "data", "viz"]),
        (
            "ceo",
            &["executive", "storyteller", "slide", "dashboard", "kpi"],
        ),
    ];

    for (phrase, expansions) in PHRASE_MAP {
        if folded.contains(phrase) {
            for exp in *expansions {
                terms.push((*exp).to_string());
            }
        }
    }
    terms
}

impl UnifiedSkillCatalog {
    /// Dynamically discovers all skills across the 4 canonical buckets with `O_NOFOLLOW`,
    /// canonical path-jail, and per-plugin fault isolation.
    pub fn discover(
        repo_root: &Path,
        conn: Option<&Connection>,
        tenant_id: &str,
        project_id: Option<&str>,
    ) -> Self {
        let mut entries_by_id: BTreeMap<String, UnifiedSkillEntry> = BTreeMap::new();
        let mut warnings: Vec<String> = Vec::new();

        // Bucket 1: Canonical Host-Runtime Skills (`packages/host-skills`)
        let host_root = repo_root.join(HOST_SKILLS_RELATIVE_DIR);
        scan_directory_bucket(
            &host_root,
            SkillSourceBucket::HostSkills,
            true,
            &mut entries_by_id,
            &mut warnings,
        );

        // Bucket 2: Standalone `design-craft` — ALWAYS run `mount_design_craft_bundle_at` /
        // `mount_default_design_craft_plugin` validation so `plugin.json`, `design-auditor` read-only,
        // `optionalPeers`, and `phaseSkillMap` contracts are unconditionally enforced.
        let canonical_dc_root = repo_root.join(CANONICAL_DESIGN_CRAFT_RELATIVE_DIR);
        let default_dc_mount = if canonical_dc_root.is_dir() {
            mount_design_craft_bundle_at(repo_root, CANONICAL_DESIGN_CRAFT_RELATIVE_DIR)
        } else {
            mount_default_design_craft_plugin(repo_root)
        };
        match default_dc_mount {
            Ok(mounted) => {
                let skills_dir = mounted.bundle_dir.join("skills");
                scan_directory_bucket(
                    &skills_dir,
                    SkillSourceBucket::DesignCraft,
                    true,
                    &mut entries_by_id,
                    &mut warnings,
                );
            }
            Err(e) => warnings.push(format!(
                "Default design-craft contract validation warning (isolated): {e}"
            )),
        }

        // Bucket 3: Verbatim Upstream Community Skills (`packages/builtin-skills/upstream-skills`)
        let upstream_root = repo_root.join(UPSTREAM_SKILLS_RELATIVE_DIR);
        scan_directory_bucket(
            &upstream_root,
            SkillSourceBucket::UpstreamVerbatim,
            false,
            &mut entries_by_id,
            &mut warnings,
        );

        // Bucket 4: SQLite ACTIVE `skill_plugins` + ACCEPTED `knowledge_cards` (`HUMAN_ONLY`)
        if let Some(db) = conn {
            load_sqlite_active_plugins_and_learn_cards(
                db,
                tenant_id,
                project_id,
                &mut entries_by_id,
                &mut warnings,
            );
        }

        let entries: Vec<UnifiedSkillEntry> = entries_by_id.into_values().collect();
        let catalog_sha256 = compute_catalog_sha256(&entries);
        let zone1_tier1_index = render_deterministic_tier1_index(&entries);
        let tier1_index_sha256 = sha256_hex(zone1_tier1_index.as_bytes());

        Self {
            entries,
            warnings,
            catalog_sha256,
            zone1_tier1_index,
            tier1_index_sha256,
        }
    }

    pub fn entries(&self) -> &[UnifiedSkillEntry] {
        &self.entries
    }

    pub fn get(&self, skill_id: &str) -> Option<&UnifiedSkillEntry> {
        self.entries.iter().find(|e| e.skill_id == skill_id)
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn catalog_sha256(&self) -> &str {
        &self.catalog_sha256
    }

    pub fn tier1_index_sha256(&self) -> &str {
        &self.tier1_index_sha256
    }

    pub fn render_zone1_tier1_index(&self) -> &str {
        &self.zone1_tier1_index
    }

    pub fn snapshot(&self) -> UnifiedSkillCatalogSnapshot {
        let mut bucket_counts = BTreeMap::new();
        for b in [
            SkillSourceBucket::DesignCraft,
            SkillSourceBucket::HostSkills,
            SkillSourceBucket::UpstreamVerbatim,
            SkillSourceBucket::SqlitePlugin,
            SkillSourceBucket::LearnCard,
        ] {
            bucket_counts.insert(b, 0usize);
        }
        for entry in &self.entries {
            *bucket_counts.entry(entry.bucket).or_insert(0) += 1;
        }
        UnifiedSkillCatalogSnapshot {
            catalog_sha256: self.catalog_sha256.clone(),
            tier1_index_sha256: self.tier1_index_sha256.clone(),
            total_skills: self.entries.len(),
            bucket_counts,
            skills: self.entries.clone(),
            warnings: self.warnings.clone(),
        }
    }

    /// Filters catalog entries by multilingual VI/EN query while preserving deterministic catalog/Tier-1 index hashes.
    pub fn filter_snapshot(&self, query: &str) -> UnifiedSkillCatalogSnapshot {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.snapshot();
        }
        let matches = self.route_skills_multilingual(trimmed, self.entries.len().max(1));
        let mut skills = Vec::with_capacity(matches.len());
        let mut bucket_counts = BTreeMap::new();
        for b in [
            SkillSourceBucket::DesignCraft,
            SkillSourceBucket::HostSkills,
            SkillSourceBucket::UpstreamVerbatim,
            SkillSourceBucket::SqlitePlugin,
            SkillSourceBucket::LearnCard,
        ] {
            bucket_counts.insert(b, 0usize);
        }
        for m in matches {
            if let Some(entry) = self.get(&m.skill_id) {
                *bucket_counts.entry(entry.bucket).or_insert(0) += 1;
                skills.push(entry.clone());
            }
        }
        UnifiedSkillCatalogSnapshot {
            catalog_sha256: self.catalog_sha256.clone(),
            tier1_index_sha256: self.tier1_index_sha256.clone(),
            total_skills: skills.len(),
            bucket_counts,
            skills,
            warnings: self.warnings.clone(),
        }
    }

    /// Lazy Tier-2 `SKILL.md` loader enforcing stepwise `openat(O_NOFOLLOW)`, hardlink rejection,
    /// bounded `MAX_SKILL_BYTES` read, SHA-256 integrity verification against `entry.content_sha256`,
    /// and runtime markdown safety re-scan.
    pub fn load_tier2_skill_body(&self, skill_id: &str) -> Result<String, UnifiedCatalogError> {
        let entry = self
            .get(skill_id)
            .ok_or_else(|| UnifiedCatalogError::SkillNotFound(skill_id.to_string()))?;
        if let Some(ref inline) = entry.inline_body {
            scan_skill_markdown_safety(skill_id, inline).map_err(|e| {
                UnifiedCatalogError::SafetyViolation {
                    skill_id: skill_id.to_string(),
                    reason: e.to_string(),
                }
            })?;
            return Ok(inline.clone());
        }
        let root = entry
            .source_root
            .as_deref()
            .ok_or_else(|| UnifiedCatalogError::SkillNotFound(skill_id.to_string()))?;
        let rel = entry.skill_md_rel_path.as_deref().unwrap_or("SKILL.md");
        let body = read_confined_file_nofollow_bounded(root, rel)?;
        let actual_sha = sha256_hex(body.as_bytes());
        if actual_sha != entry.content_sha256 {
            return Err(UnifiedCatalogError::IntegrityMismatch {
                skill_id: skill_id.to_string(),
            });
        }
        scan_skill_markdown_safety(rel, &body).map_err(|e| {
            UnifiedCatalogError::SafetyViolation {
                skill_id: skill_id.to_string(),
                reason: e.to_string(),
            }
        })?;
        Ok(body)
    }

    /// Lazy Tier-3 `references/*.md` loader enforcing stepwise `openat(O_NOFOLLOW)`, hardlink rejection,
    /// bounded `MAX_SKILL_BYTES` read, and markdown safety re-scan.
    pub fn load_tier3_reference(
        &self,
        skill_id: &str,
        rel_ref_path: &str,
    ) -> Result<String, UnifiedCatalogError> {
        let entry = self
            .get(skill_id)
            .ok_or_else(|| UnifiedCatalogError::SkillNotFound(skill_id.to_string()))?;
        let root = entry
            .source_root
            .as_deref()
            .ok_or_else(|| UnifiedCatalogError::ReferenceNotFound(rel_ref_path.to_string()))?;
        if !rel_ref_path.starts_with("references/") || !rel_ref_path.ends_with(".md") {
            return Err(UnifiedCatalogError::PathTraversal(rel_ref_path.to_string()));
        }
        let content = read_confined_file_nofollow_bounded(root, rel_ref_path)?;
        scan_skill_markdown_safety(rel_ref_path, &content).map_err(|e| {
            UnifiedCatalogError::SafetyViolation {
                skill_id: format!("{skill_id}:{rel_ref_path}"),
                reason: e.to_string(),
            }
        })?;
        Ok(content)
    }

    /// Multilingual VI/EN BM25-style routing over all dynamically discovered skills.
    pub fn route_skills_multilingual(
        &self,
        query: &str,
        max_results: usize,
    ) -> Vec<SkillMatchResult> {
        let query_terms = expand_multilingual_query_terms(query);
        if query_terms.is_empty() || self.entries.is_empty() || max_results == 0 {
            return Vec::new();
        }

        // Precompute document token bags & document frequencies for BM25
        let n_docs = self.entries.len() as f64;
        let mut doc_tokens: Vec<(Vec<String>, BTreeSet<String>)> =
            Vec::with_capacity(self.entries.len());
        let mut df: HashMap<String, usize> = HashMap::new();
        let mut total_len = 0usize;

        for entry in &self.entries {
            let mut bag = Vec::new();
            // Weight skill_id and tags strongly by repeating them in the lexical bag
            let id_toks = tokenize_folded(&entry.skill_id);
            bag.extend(id_toks.iter().cloned());
            bag.extend(id_toks);
            for tag in &entry.tags {
                let tag_toks = tokenize_folded(tag);
                bag.extend(tag_toks.iter().cloned());
                bag.extend(tag_toks);
            }
            bag.extend(tokenize_folded(&entry.description));
            total_len += bag.len().max(1);
            let unique: BTreeSet<String> = bag.iter().cloned().collect();
            for t in &unique {
                *df.entry(t.clone()).or_insert(0) += 1;
            }
            doc_tokens.push((bag, unique));
        }
        let avgdl = (total_len as f64 / n_docs).max(1.0);
        let k1 = 1.5_f64;
        let b = 0.75_f64;

        let unique_query: BTreeSet<String> = query_terms.into_iter().collect();
        let mut scored: Vec<SkillMatchResult> = Vec::new();

        for (idx, entry) in self.entries.iter().enumerate() {
            let (ref bag, _) = doc_tokens[idx];
            let dl = bag.len().max(1) as f64;
            let mut tf_map: HashMap<&str, usize> = HashMap::new();
            for tok in bag {
                *tf_map.entry(tok.as_str()).or_insert(0) += 1;
            }
            let mut score = 0.0_f64;
            let mut matched = Vec::new();
            for qt in &unique_query {
                let tf = *tf_map.get(qt.as_str()).unwrap_or(&0) as f64;
                if tf > 0.0 {
                    let doc_freq = *df.get(qt).unwrap_or(&1) as f64;
                    let idf = ((n_docs - doc_freq + 0.5) / (doc_freq + 0.5) + 1.0).ln();
                    let denom = tf + k1 * (1.0 - b + b * (dl / avgdl));
                    score += idf * ((tf * (k1 + 1.0)) / denom);
                    matched.push(qt.clone());
                }
            }
            if score > 0.0 {
                scored.push(SkillMatchResult {
                    skill_id: entry.skill_id.clone(),
                    bucket: entry.bucket,
                    score,
                    matched_terms: matched,
                });
            }
        }

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.skill_id.cmp(&b.skill_id))
        });
        scored.truncate(max_results);
        scored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("plugin-host lives in crates/plugin-host")
            .to_path_buf()
    }

    #[test]
    fn discovers_all_three_disk_buckets_and_routes_vietnamese_intents() {
        let catalog = UnifiedSkillCatalog::discover(&workspace_root(), None, "local-default", None);
        let snap = catalog.snapshot();
        assert_eq!(snap.catalog_sha256.len(), 64);
        assert_eq!(snap.tier1_index_sha256.len(), 64);
        assert!(
            *snap
                .bucket_counts
                .get(&SkillSourceBucket::HostSkills)
                .unwrap_or(&0)
                >= 3
        );
        assert!(
            *snap
                .bucket_counts
                .get(&SkillSourceBucket::DesignCraft)
                .unwrap_or(&0)
                >= 20
        );
        assert!(
            *snap
                .bucket_counts
                .get(&SkillSourceBucket::UpstreamVerbatim)
                .unwrap_or(&0)
                >= 1
        );

        let vi_chart =
            catalog.route_skills_multilingual("Tạo biểu đồ doanh thu kể chuyện dữ liệu cho CEO", 5);
        let ids: Vec<&str> = vi_chart.iter().map(|m| m.skill_id.as_str()).collect();
        assert!(
            ids.contains(&"data-viz-storyteller"),
            "Vietnamese chart intent must match data-viz-storyteller, got {ids:?}"
        );

        let vi_dash = catalog.route_skills_multilingual("Thiết kế bảng điều khiển KPI", 5);
        let dash_ids: Vec<&str> = vi_dash.iter().map(|m| m.skill_id.as_str()).collect();
        assert!(
            dash_ids.contains(&"dashboard-design") || dash_ids.contains(&"dashboard-ux"),
            "Vietnamese dashboard intent must match dashboard-design or dashboard-ux, got {dash_ids:?}"
        );

        // Tier-3 lazy reference loading & path traversal rejection
        let arc = catalog
            .load_tier3_reference("data-viz-storyteller", "references/storytelling-arc.md")
            .expect("load Tier-3 storytelling-arc.md");
        assert!(arc.contains("AI Lens Strip"));
        assert!(catalog
            .load_tier3_reference("data-viz-storyteller", "../SKILL.md")
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn isolates_corrupt_skill_and_blocks_symlink_reference_escape() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().expect("tempdir");
        let host_dir = tmp.path().join("packages/host-skills");
        let good_dir = host_dir.join("good-skill");
        let bad_dir = host_dir.join("corrupt-skill");
        std::fs::create_dir_all(good_dir.join("references")).unwrap();
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(
            good_dir.join("SKILL.md"),
            "---\nname: good-skill\nversion: 1.0.0\ndescription: Clean valid skill.\nrisk_tier: 1\n---\n# Good\n",
        )
        .unwrap();
        std::fs::write(bad_dir.join("SKILL.md"), "NOT VALID YAML FRONTMATTER").unwrap();
        let outside = tmp.path().join("outside-secret.md");
        std::fs::write(&outside, "SECRET").unwrap();
        symlink(&outside, good_dir.join("references/leak.md")).unwrap();

        let catalog = UnifiedSkillCatalog::discover(tmp.path(), None, "local-default", None);
        assert!(catalog.get("good-skill").is_some());
        assert!(catalog.get("corrupt-skill").is_none());
        assert!(!catalog.warnings().is_empty());
        assert!(matches!(
            catalog.load_tier3_reference("good-skill", "references/leak.md"),
            Err(UnifiedCatalogError::SymlinkRejected(_))
        ));

        // Hardlink rejection (nlink != 1)
        let hardlink_target = tmp.path().join("hardlink-source.md");
        std::fs::write(&hardlink_target, "# Hardlink source\n").unwrap();
        std::fs::hard_link(&hardlink_target, good_dir.join("references/hardlink.md")).unwrap();
        assert!(matches!(
            catalog.load_tier3_reference("good-skill", "references/hardlink.md"),
            Err(UnifiedCatalogError::HardlinkRejected(_))
        ));

        // Oversized file rejection (> MAX_SKILL_BYTES)
        let huge = "# Huge\n".to_string() + &"a".repeat(MAX_SKILL_BYTES + 128);
        std::fs::write(good_dir.join("references/huge.md"), huge).unwrap();
        assert!(matches!(
            catalog.load_tier3_reference("good-skill", "references/huge.md"),
            Err(UnifiedCatalogError::FileTooLarge { .. })
        ));

        // Tier-2 SHA-256 tamper detection
        std::fs::write(
            good_dir.join("SKILL.md"),
            "---\nname: good-skill\nversion: 1.0.0\ndescription: Tampered body.\nrisk_tier: 1\n---\n# Tampered\n",
        )
        .unwrap();
        assert!(matches!(
            catalog.load_tier2_skill_body("good-skill"),
            Err(UnifiedCatalogError::IntegrityMismatch { .. })
        ));
    }

    #[test]
    fn enforces_actual_tier1_token_budget_with_warning_and_no_refs_bloat() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let up_dir = tmp
            .path()
            .join("packages/builtin-skills/upstream-skills/verbose-skill");
        std::fs::create_dir_all(up_dir.join("references")).unwrap();
        let long_desc = "operational telemetry insight storytelling metric ".repeat(30);
        std::fs::write(
            up_dir.join("SKILL.md"),
            format!(
                "---\nname: verbose-skill\nversion: 1.0.0\ndescription: {long_desc}\n---\n# Body\n"
            ),
        )
        .unwrap();
        std::fs::write(up_dir.join("references/extra.md"), "# Extra\n").unwrap();

        let catalog = UnifiedSkillCatalog::discover(tmp.path(), None, "local-default", None);
        let entry = catalog
            .get("verbose-skill")
            .expect("discovered verbose-skill");
        assert!(entry.tier1_token_estimate <= MAX_TIER1_SKILL_TOKENS);
        assert!(catalog
            .warnings()
            .iter()
            .any(|w| w.contains("verbose-skill") && w.contains("truncated cleanly")));
        let rendered = catalog.render_zone1_tier1_index();
        assert!(
            !rendered.contains("refs=["),
            "Tier-1 index must not bloat with refs=[...]"
        );
        for line in rendered.lines().skip(1) {
            assert!(
                estimate_token_count(line) <= MAX_TIER1_SKILL_TOKENS,
                "Rendered Tier-1 line exceeded 100 tokens: {line}"
            );
        }
    }
}
