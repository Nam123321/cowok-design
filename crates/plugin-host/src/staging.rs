// @story 08.1
// @implements AC-01, AC-07
use crate::models::{SkillManifest, StagingRunReceipt};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions, Permissions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

const KV_HASH_PROTOCOL: &[u8] = b"cowok-kv-hash-v1";

/// Manages per-run ephemeral skill staging (.cowok-skills/<run_id>/)
/// and calculates stable prompt hashes for KV Prompt Cache preservation.
pub struct PerRunStager {
    base_staging_dir: PathBuf,
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn canonicalize_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                return i.to_string();
            }
            if let Some(u) = n.as_u64() {
                return u.to_string();
            }
            if let Some(f) = n.as_f64() {
                if f.is_finite()
                    && f.fract() == 0.0
                    && (-9_007_199_254_740_991.0_f64..=9_007_199_254_740_991.0_f64).contains(&f)
                {
                    if f == 0.0 {
                        return "0".to_string();
                    }
                    return (f as i64).to_string();
                }
            }
            n.to_string()
        }
        serde_json::Value::String(s) => serde_json::to_string(s).unwrap_or_default(),
        serde_json::Value::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(canonicalize_json).collect();
            format!("[{}]", parts.join(","))
        }
        serde_json::Value::Object(obj) => {
            let sorted: BTreeMap<&String, &serde_json::Value> = obj.iter().collect();
            let parts: Vec<String> = sorted
                .into_iter()
                .map(|(k, v)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_default(),
                        canonicalize_json(v)
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

fn hash_optional_bytes(hasher: &mut Sha256, opt: Option<&[u8]>) {
    match opt {
        Some(bytes) => {
            hasher.update([1u8]);
            update_len_prefixed(hasher, bytes);
        }
        None => hasher.update([0u8]),
    }
}

fn hash_sorted_unique(hasher: &mut Sha256, items: &[String]) {
    let unique: BTreeSet<&str> = items.iter().map(String::as_str).collect();
    hasher.update((unique.len() as u64).to_be_bytes());
    for item in unique {
        update_len_prefixed(hasher, item.as_bytes());
    }
}

fn has_hidden_or_bidi_unicode(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c as u32,
            0x00AD
                | 0x034F
                | 0x061C
                | 0x180E
                | 0x200B..=0x200F
                | 0x2028..=0x202E
                | 0x2060..=0x2069
                | 0xFE00..=0xFE0F
                | 0xFEFF
                | 0xE0001..=0xE007F
        )
    })
}

/// Pure-Rust read-only homoglyph/NFKC shadow fold covering fullwidth, math-alnum, ligatures, angle/dash variants, and combining marks.
pub(crate) fn shadow_nfkc_fold(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        let code = c as u32;
        if (0x0300..=0x036F).contains(&code) {
            continue;
        }
        match code {
            0xFB00 => {
                out.push_str("ff");
                continue;
            }
            0xFB01 => {
                out.push_str("fi");
                continue;
            }
            0xFB02 => {
                out.push_str("fl");
                continue;
            }
            0xFB03 => {
                out.push_str("ffi");
                continue;
            }
            0xFB04 => {
                out.push_str("ffl");
                continue;
            }
            0xFB05 | 0xFB06 => {
                out.push_str("st");
                continue;
            }
            _ => {}
        }
        let mapped = match code {
            0xFF01..=0xFF5E => char::from_u32(code - 0xFEE0).unwrap_or(c),
            0x3000 | 0x00A0 => ' ',
            0x2010..=0x2015 | 0x2212 | 0xFE58 | 0xFE63 => '-',
            0xFE64 | 0x27E8 | 0x3008 => '<',
            0xFE65 | 0x27E9 | 0x3009 => '>',
            // Mathematical Alphanumeric Symbols (U+1D400..=U+1D6A5: 13 styles x 52 letters each)
            0x1D400..=0x1D6A3 => {
                let offset = (code - 0x1D400) % 52;
                let base = if offset < 26 {
                    b'A' + offset as u8
                } else {
                    b'a' + (offset - 26) as u8
                };
                char::from(base)
            }
            // Mathematical digits (U+1D7CE..=U+1D7FF: 5 styles x 10 digits)
            0x1D7CE..=0x1D7FF => {
                let d = (code - 0x1D7CE) % 10;
                char::from(b'0' + d as u8)
            }
            _ => c,
        };
        out.push(mapped);
    }
    out
}

fn contains_markdown_image_beacon(lower: &str) -> bool {
    let mut search_from = 0usize;
    while let Some(pos) = lower[search_from..].find("![") {
        let rest = &lower[search_from + pos + 2..];
        if let Some(close_bracket) = rest.find("](") {
            let after = &rest[close_bracket + 2..];
            if after.contains(')') {
                return true;
            }
        }
        search_from += pos + 2;
    }
    false
}

/// Core low-level prompt directive/Unicode baseline check used by both `validate_staged_prompt_content`
/// and `security_scanner::scan_skill_markdown_safety` without creating recursion.
pub(crate) fn validate_staged_prompt_base(overview: &str) -> std::io::Result<()> {
    if has_hidden_or_bidi_unicode(overview) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Disallowed zero-width or bidirectional control Unicode character in PROMPT.md",
        ));
    }
    let shadow = shadow_nfkc_fold(overview);
    for candidate in [overview.to_ascii_lowercase(), shadow.to_ascii_lowercase()] {
        for forbidden in [
            "<important",
            "</important",
            "<system",
            "</system",
            "<!--",
            "-->",
            "<img",
        ] {
            if candidate.contains(forbidden) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Disallowed prompt directive or HTML tag '{forbidden}' in PROMPT.md"),
                ));
            }
        }
        if contains_markdown_image_beacon(&candidate) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Disallowed Markdown image beacon in PROMPT.md",
            ));
        }
    }
    Ok(())
}

/// Validates staged prompt_overview content via unified `scan_skill_markdown_safety` (IPI, XSS, HTML/Markdown beacons, Unicode spoofing).
pub fn validate_staged_prompt_content(overview: &str) -> std::io::Result<()> {
    crate::repackager::scan_skill_markdown_safety("PROMPT.md", overview)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))
}

/// Safely creates/validates a directory path component-by-component without traversing symlinks.
pub fn ensure_safe_dir_no_symlink(base: &Path) -> std::io::Result<PathBuf> {
    ensure_safe_base_dir(base)
}

fn ensure_safe_base_dir(base: &Path) -> std::io::Result<PathBuf> {
    use std::path::Component;
    let abs_base = if base.is_relative() {
        std::env::current_dir()?.join(base)
    } else {
        base.to_path_buf()
    };

    let mut comps: Vec<std::ffi::OsString> = Vec::new();
    for comp in abs_base.components() {
        match comp {
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Parent traversal '..' disallowed in staging path: {}",
                        base.display()
                    ),
                ));
            }
            Component::Normal(seg) => comps.push(seg.to_os_string()),
            Component::Prefix(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Unsupported path prefix in staging path",
                ));
            }
        }
    }

    let (mut resolved, start_idx) = {
        #[cfg(target_os = "macos")]
        {
            let mut mac_resolved = PathBuf::from("/");
            let mut mac_start = 0usize;
            if let Some(first) = comps.first().and_then(|s| s.to_str()) {
                if matches!(first, "tmp" | "var" | "etc")
                    && fs::symlink_metadata(format!("/{first}"))
                        .is_ok_and(|m| m.file_type().is_symlink())
                {
                    mac_resolved = PathBuf::from(match first {
                        "tmp" => "/private/tmp",
                        "var" => "/private/var",
                        "etc" => "/private/etc",
                        _ => "/",
                    });
                    mac_start = 1;
                }
            }
            (mac_resolved, mac_start)
        }
        #[cfg(not(target_os = "macos"))]
        {
            (PathBuf::from("/"), 0usize)
        }
    };

    for seg in comps.into_iter().skip(start_idx) {
        resolved.push(&seg);
        match fs::symlink_metadata(&resolved) {
            Ok(meta) => {
                if meta.file_type().is_symlink() || !meta.is_dir() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "Symlink or non-directory component disallowed in staging path: {}",
                            resolved.display()
                        ),
                    ));
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&resolved) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e),
                }
                let meta = fs::symlink_metadata(&resolved)?;
                if meta.file_type().is_symlink() || !meta.is_dir() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "Symlink or non-directory component disallowed in staging path: {}",
                            resolved.display()
                        ),
                    ));
                }
            }
            Err(err) => return Err(err),
        }
    }
    Ok(resolved)
}

fn write_file_nofollow_excl(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    opts.custom_flags(libc::O_NOFOLLOW);
    let mut f = opts.open(path)?;
    #[cfg(unix)]
    {
        let meta = f.metadata()?;
        if !meta.is_file() || meta.nlink() != 1 {
            let _ = fs::remove_file(path);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Security violation: hardlinked/non-regular staged file at {}",
                    path.display()
                ),
            ));
        }
    }
    f.write_all(bytes)?;
    f.sync_all()
}

fn set_path_readonly(path: &Path, is_dir: bool) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mode = if is_dir { 0o555 } else { 0o444 };
        let mut opts = OpenOptions::new();
        opts.read(true);
        let flags = if is_dir {
            libc::O_NOFOLLOW | libc::O_DIRECTORY
        } else {
            libc::O_NOFOLLOW
        };
        opts.custom_flags(flags);
        let f = opts.open(path)?;
        f.set_permissions(Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = is_dir;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_readonly(true);
        fs::set_permissions(path, perms)
    }
}

fn restore_writable_recursive(path: &Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Refusing cleanup across symlink: {}", path.display()),
        ));
    }
    #[cfg(unix)]
    {
        let mode = if meta.is_dir() { 0o755 } else { 0o644 };
        fs::set_permissions(path, Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    {
        let mut perms = meta.permissions();
        perms.set_readonly(false);
        fs::set_permissions(path, perms)?;
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            restore_writable_recursive(&entry?.path())?;
        }
    }
    Ok(())
}

impl PerRunStager {
    pub fn new<P: AsRef<Path>>(base_staging_dir: P) -> Self {
        Self {
            base_staging_dir: base_staging_dir.as_ref().to_path_buf(),
        }
    }

    /// Computes canonical KV-cache hash with explicit optional SKILL.md SHA-256 digests.
    pub fn compute_stable_prompt_hash_with_sources(
        skills: &[(&SkillManifest, Option<&str>)],
    ) -> String {
        let mut sorted: Vec<(&SkillManifest, Option<&str>)> = skills.to_vec();
        sorted.sort_by(|a, b| a.0.name.as_bytes().cmp(b.0.name.as_bytes()));

        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, KV_HASH_PROTOCOL);
        hasher.update((sorted.len() as u64).to_be_bytes());

        for (skill, md_sha) in sorted {
            update_len_prefixed(&mut hasher, skill.name.as_bytes());
            update_len_prefixed(&mut hasher, skill.version.as_bytes());
            update_len_prefixed(&mut hasher, skill.description.as_bytes());
            update_len_prefixed(&mut hasher, skill.entry_point.as_bytes());
            hash_sorted_unique(&mut hasher, &skill.capabilities_required);
            hasher.update([skill.risk_tier]);
            let params_str = skill.parameters.as_ref().map(canonicalize_json);
            hash_optional_bytes(&mut hasher, params_str.as_deref().map(str::as_bytes));
            hash_sorted_unique(&mut hasher, &skill.tags);
            hash_optional_bytes(
                &mut hasher,
                skill.prompt_overview.as_deref().map(str::as_bytes),
            );
            hash_optional_bytes(&mut hasher, md_sha.map(str::as_bytes));
        }

        format!("{:x}", hasher.finalize())
    }

    /// Computes deterministic stable_prompt_hash across sorted skill manifests.
    pub fn compute_stable_prompt_hash(skills: &[SkillManifest]) -> String {
        let refs: Vec<(&SkillManifest, Option<&str>)> = skills.iter().map(|s| (s, None)).collect();
        Self::compute_stable_prompt_hash_with_sources(&refs)
    }

    fn validate_safe_segment(kind: &str, segment: &str) -> std::io::Result<()> {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.contains('/')
            || segment.contains('\\')
            || segment.contains('\0')
            || segment.chars().any(|c| c.is_control())
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid {kind} segment '{segment}': path separators and traversal segments are forbidden"
                ),
            ));
        }
        Ok(())
    }

    /// Atomically stages skills and SKILL.md SHA-256 bindings into `.cowok-skills/<run_id>/`,
    /// enforces prompt sanitization and read-only permissions, and rolls back owned artifacts on error.
    pub fn stage_run_with_sources(
        &self,
        run_id: &str,
        skills: &[(&SkillManifest, Option<&str>)],
    ) -> std::io::Result<StagingRunReceipt> {
        Self::validate_safe_segment("run_id", run_id)?;
        let safe_base = ensure_safe_base_dir(&self.base_staging_dir)?;

        let run_dir = safe_base.join(run_id);
        if fs::symlink_metadata(&run_dir).is_ok() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("Staging run directory already exists or is a symlink: {run_id}"),
            ));
        }

        fs::create_dir(&run_dir)?;
        match self.populate_and_lock_run(&run_dir, run_id, skills) {
            Ok(receipt) => Ok(receipt),
            Err(err) => {
                let _ = restore_writable_recursive(&run_dir);
                let _ = fs::remove_dir_all(&run_dir);
                Err(err)
            }
        }
    }

    fn populate_and_lock_run(
        &self,
        run_dir: &Path,
        run_id: &str,
        skills: &[(&SkillManifest, Option<&str>)],
    ) -> std::io::Result<StagingRunReceipt> {
        let mut sorted: Vec<(&SkillManifest, Option<&str>)> = skills.to_vec();
        sorted.sort_by(|a, b| a.0.name.as_bytes().cmp(b.0.name.as_bytes()));

        let mut staged_names = Vec::with_capacity(sorted.len());
        let mut seen_names = BTreeSet::new();
        let mut skill_dirs = Vec::with_capacity(sorted.len());

        for &(skill, _) in &sorted {
            Self::validate_safe_segment("skill.name", &skill.name)?;
            if !seen_names.insert(skill.name.as_str()) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Duplicate skill name '{}' in staging run", skill.name),
                ));
            }
            if let Some(overview) = &skill.prompt_overview {
                validate_staged_prompt_content(overview)?;
            }

            let skill_dir = run_dir.join(&skill.name);
            fs::create_dir(&skill_dir)?;

            let manifest_path = skill_dir.join("plugin.json");
            let json = serde_json::to_string_pretty(skill).map_err(std::io::Error::other)?;
            write_file_nofollow_excl(&manifest_path, json.as_bytes())?;
            set_path_readonly(&manifest_path, false)?;

            if let Some(overview) = &skill.prompt_overview {
                let overview_path = skill_dir.join("PROMPT.md");
                write_file_nofollow_excl(&overview_path, overview.as_bytes())?;
                set_path_readonly(&overview_path, false)?;
            }

            skill_dirs.push(skill_dir);
            staged_names.push(skill.name.clone());
        }

        let stable_prompt_hash = Self::compute_stable_prompt_hash_with_sources(&sorted);
        let receipt = StagingRunReceipt {
            run_id: run_id.to_string(),
            staged_skills: staged_names,
            stable_prompt_hash,
            staged_dir: run_dir.to_string_lossy().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        let receipt_path = run_dir.join("run-receipt.json");
        let receipt_json = serde_json::to_string_pretty(&receipt).map_err(std::io::Error::other)?;
        write_file_nofollow_excl(&receipt_path, receipt_json.as_bytes())?;
        set_path_readonly(&receipt_path, false)?;

        for skill_dir in skill_dirs {
            set_path_readonly(&skill_dir, true)?;
        }
        set_path_readonly(run_dir, true)?;

        Ok(receipt)
    }

    /// Stages skills into `.cowok-skills/<run_id>/` and writes a staging receipt.
    pub fn stage_run(
        &self,
        run_id: &str,
        skills: &[SkillManifest],
    ) -> std::io::Result<StagingRunReceipt> {
        let refs: Vec<(&SkillManifest, Option<&str>)> = skills.iter().map(|s| (s, None)).collect();
        self.stage_run_with_sources(run_id, &refs)
    }

    /// Restores permissions safely on owned run directory and removes it without affecting siblings.
    pub fn cleanup_run(&self, run_id: &str) -> std::io::Result<()> {
        Self::validate_safe_segment("run_id", run_id)?;
        let safe_base = ensure_safe_base_dir(&self.base_staging_dir)?;
        let run_dir = safe_base.join(run_id);
        let Ok(meta) = fs::symlink_metadata(&run_dir) else {
            return Ok(());
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Refusing cleanup on symlink or non-directory: {run_id}"),
            ));
        }
        restore_writable_recursive(&run_dir)?;
        fs::remove_dir_all(&run_dir)
    }

    /// Prunes expired run staging directories older than `max_age_secs` safely (skipping symlinks/non-dirs).
    pub fn prune_expired_runs(&self, max_age_secs: u64) -> std::io::Result<usize> {
        let safe_base = ensure_safe_base_dir(&self.base_staging_dir)?;
        let now = std::time::SystemTime::now();
        let mut removed = 0usize;
        for entry in fs::read_dir(&safe_base)? {
            let entry = entry?;
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() || !meta.is_dir() {
                continue;
            }
            let Ok(modified) = meta.modified() else {
                continue;
            };
            let Ok(age) = now.duration_since(modified) else {
                continue;
            };
            if age.as_secs() >= max_age_secs {
                if let Some(name) = entry.file_name().to_str() {
                    if Self::validate_safe_segment("run_id", name).is_ok()
                        && self.cleanup_run(name).is_ok()
                    {
                        removed += 1;
                    }
                }
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_prompt_hash_deterministic() {
        let s1 = SkillManifest {
            name: "beta".to_string(),
            version: "1.0.0".to_string(),
            description: "Beta skill".to_string(),
            entry_point: "index.js".to_string(),
            capabilities_required: vec!["sdi".to_string()],
            risk_tier: 1,
            parameters: None,
            tags: vec![],
            prompt_overview: None,
        };
        let s2 = SkillManifest {
            name: "alpha".to_string(),
            version: "1.0.0".to_string(),
            description: "Alpha skill".to_string(),
            entry_point: "index.js".to_string(),
            capabilities_required: vec![],
            risk_tier: 1,
            parameters: None,
            tags: vec![],
            prompt_overview: None,
        };

        let hash_forward = PerRunStager::compute_stable_prompt_hash(&[s1.clone(), s2.clone()]);
        let hash_reverse = PerRunStager::compute_stable_prompt_hash(&[s2, s1]);

        assert_eq!(hash_forward, hash_reverse);
        assert_eq!(hash_forward.len(), 64);
    }

    #[test]
    fn test_stage_run_creates_files() {
        let temp = tempfile::tempdir().unwrap();
        let stager = PerRunStager::new(temp.path());

        let skill = SkillManifest {
            name: "test-skill".to_string(),
            version: "0.1.0".to_string(),
            description: "Test description".to_string(),
            entry_point: "main.js".to_string(),
            capabilities_required: vec!["testing".to_string()],
            risk_tier: 1,
            parameters: None,
            tags: vec!["test".to_string()],
            prompt_overview: Some("System prompt overview".to_string()),
        };

        let receipt = stager
            .stage_run("run-42", &[skill])
            .expect("Staging should succeed");
        assert_eq!(receipt.run_id, "run-42");
        assert_eq!(receipt.staged_skills, vec!["test-skill"]);

        let staged_file = temp.path().join("run-42/test-skill/plugin.json");
        assert!(staged_file.exists());
        stager.cleanup_run("run-42").expect("Cleanup succeeds");
    }

    #[test]
    fn test_stage_run_blocks_path_traversal() {
        let temp = tempfile::tempdir().unwrap();
        let stager = PerRunStager::new(temp.path());
        let bad_skill = SkillManifest {
            name: "../escaped".to_string(),
            version: "0.1.0".to_string(),
            description: "Bad skill".to_string(),
            entry_point: "main.js".to_string(),
            capabilities_required: vec![],
            risk_tier: 1,
            parameters: None,
            tags: vec![],
            prompt_overview: None,
        };
        assert!(stager
            .stage_run("../bad-run", std::slice::from_ref(&bad_skill))
            .is_err());
        assert!(stager.stage_run("ok-run", &[bad_skill]).is_err());
    }
}
