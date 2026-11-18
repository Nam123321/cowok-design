// @story 08.1
// @implements AC-01, AC-02, AC-05, AC-06
use crate::models::SkillManifest;
use crate::repackager::repackager_engine::MAX_SKILL_NAME_BYTES;
use crate::repackager::security_scanner::{
    scan_skill_markdown_safety, validate_skill_metadata_text,
};
use crate::validator::{parse_and_validate_skill_frontmatter, FrontmatterMetadata};
use domain::models::SkillPlugin;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, SkillManifest>,
    skill_md_hashes: HashMap<String, String>,
    skills_dir: Option<PathBuf>,
    rejected_skills: Vec<(String, String)>,
}

fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn verify_frontmatter_parity(
    meta: &FrontmatterMetadata,
    manifest: &SkillManifest,
) -> Result<(), String> {
    if meta.name != manifest.name {
        return Err(format!(
            "SKILL.md name '{}' does not match plugin.json name '{}'",
            meta.name, manifest.name
        ));
    }
    if meta.version.as_deref() != Some(manifest.version.as_str()) {
        return Err(format!(
            "SKILL.md version '{:?}' does not match plugin.json version '{}'",
            meta.version, manifest.version
        ));
    }
    if meta.description != manifest.description {
        return Err(format!(
            "SKILL.md description '{}' does not match plugin.json description '{}'",
            meta.description, manifest.description
        ));
    }
    if meta.risk_tier != Some(manifest.risk_tier) {
        return Err(format!(
            "SKILL.md risk_tier {:?} does not match plugin.json risk_tier {}",
            meta.risk_tier, manifest.risk_tier
        ));
    }
    if meta.entry_point.as_deref().unwrap_or("SKILL.md") != manifest.entry_point {
        return Err(format!(
            "SKILL.md entry_point '{:?}' does not match plugin.json entry_point '{}'",
            meta.entry_point, manifest.entry_point
        ));
    }
    let fm_caps: BTreeSet<&str> = meta
        .capabilities_required
        .iter()
        .map(String::as_str)
        .collect();
    let man_caps: BTreeSet<&str> = manifest
        .capabilities_required
        .iter()
        .map(String::as_str)
        .collect();
    if fm_caps != man_caps {
        return Err(
            "SKILL.md capabilities_required does not match plugin.json capabilities_required"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_skill_entry_point(folder: &Path, entry_point: &str) -> Result<(), String> {
    let ep = entry_point.trim();
    if ep.is_empty()
        || ep.starts_with('/')
        || ep.ends_with('/')
        || ep.contains('\\')
        || ep.contains('\0')
        || ep.chars().any(|c| c.is_control())
        || ep
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return Err(format!("unsafe skill entry_point '{entry_point}'"));
    }
    if ep == "SKILL.md" {
        return Ok(());
    }
    let mut cur = folder.to_path_buf();
    let segments: Vec<&str> = ep.split('/').collect();
    for (idx, seg) in segments.iter().enumerate() {
        cur.push(seg);
        let meta = std::fs::symlink_metadata(&cur)
            .map_err(|e| format!("entry_point '{ep}' missing/unreadable: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!("entry_point '{ep}' symlink component rejected"));
        }
        let is_last = idx + 1 == segments.len();
        if is_last {
            if !meta.is_file() {
                return Err(format!("entry_point '{ep}' must be a regular file"));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if meta.nlink() != 1 {
                    return Err(format!("entry_point '{ep}' hardlink rejected (nlink != 1)"));
                }
            }
        } else if !meta.is_dir() {
            return Err(format!("entry_point '{ep}' parent is not a directory"));
        }
    }
    Ok(())
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_directory<P: AsRef<Path>>(dir: P) -> Self {
        let mut registry = Self {
            skills: HashMap::new(),
            skill_md_hashes: HashMap::new(),
            skills_dir: Some(dir.as_ref().to_path_buf()),
            rejected_skills: Vec::new(),
        };
        let _ = registry.discover();
        registry
    }

    /// Registers a manifest directly without a backing SKILL.md digest.
    pub fn register(&mut self, manifest: SkillManifest) {
        self.skills.insert(manifest.name.clone(), manifest);
    }

    /// Registers a manifest alongside its canonical SKILL.md SHA-256 digest.
    pub fn register_with_hash(&mut self, manifest: SkillManifest, skill_md_sha256: Option<String>) {
        if let Some(digest) = skill_md_sha256 {
            self.skill_md_hashes.insert(manifest.name.clone(), digest);
        }
        self.skills.insert(manifest.name.clone(), manifest);
    }

    /// Returns the canonical SKILL.md SHA-256 hex digest recorded for `name`, if any.
    pub fn skill_md_sha256(&self, name: &str) -> Option<&str> {
        self.skill_md_hashes.get(name).map(String::as_str)
    }

    /// Returns lexicographically sorted skill manifests paired with optional SKILL.md SHA-256 digests.
    pub fn staged_sources(&self) -> Vec<(&SkillManifest, Option<&str>)> {
        self.list()
            .into_iter()
            .map(|m| (m, self.skill_md_sha256(&m.name)))
            .collect()
    }

    /// Scans the configured directory in deterministic lexicographical order for agentskills.io skills.
    pub fn discover(&mut self) -> Result<usize, String> {
        self.rejected_skills.clear();
        let Some(dir) = self.skills_dir.clone() else {
            return Ok(0);
        };
        if !dir.exists() {
            return Ok(0);
        }

        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .collect();
        paths.sort();

        let mut count = 0;
        for path in paths {
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() || !meta.is_dir() {
                continue;
            }
            if std::fs::symlink_metadata(path.join("SKILL.md")).is_ok()
                || std::fs::symlink_metadata(path.join("plugin.json")).is_ok()
            {
                match self.load_skill_with_hash(&path) {
                    Ok((manifest, md_sha)) => {
                        self.register_with_hash(manifest, md_sha);
                        count += 1;
                    }
                    Err(err) => {
                        self.rejected_skills
                            .push((path.display().to_string(), err.clone()));
                        eprintln!(
                            "SkillRegistry::discover rejected '{}': {err}",
                            path.display()
                        );
                    }
                }
            }
        }
        Ok(count)
    }

    /// Returns `(folder_path, error)` pairs for skills rejected during the latest `discover()` pass.
    pub fn last_rejections(&self) -> &[(String, String)] {
        &self.rejected_skills
    }

    fn read_regular_utf8_nofollow(file_path: &Path, label: &str) -> Result<String, String> {
        use std::io::Read;
        let mut opts = std::fs::OpenOptions::new();
        opts.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = opts
            .open(file_path)
            .map_err(|e| format!("{label} open failed (symlink/missing): {e}"))?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err(format!("{label} must be a regular non-symlink file"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() != 1 {
                return Err(format!("{label} hardlink is forbidden (nlink != 1)"));
            }
        }
        let mut out = String::new();
        file.read_to_string(&mut out).map_err(|e| e.to_string())?;
        Ok(out)
    }

    fn load_skill_with_hash(
        &self,
        folder: &Path,
    ) -> Result<(SkillManifest, Option<String>), String> {
        let folder_meta = std::fs::symlink_metadata(folder).map_err(|e| e.to_string())?;
        if folder_meta.file_type().is_symlink() || !folder_meta.is_dir() {
            return Err("skill folder must be a real non-symlink directory".to_string());
        }

        let mut verified_repackaged = false;
        if std::fs::symlink_metadata(folder.join("skills-lock.json")).is_ok()
            || std::fs::symlink_metadata(folder.join("discovery-stub.json")).is_ok()
        {
            crate::SkillRepackager::verify_repackaged_bundle(folder)?;
            verified_repackaged = true;
        }

        let skill_md_path = folder.join("SKILL.md");
        let plugin_json_path = folder.join("plugin.json");

        let (frontmatter, md_sha) = if std::fs::symlink_metadata(&skill_md_path).is_ok() {
            let content = Self::read_regular_utf8_nofollow(&skill_md_path, "SKILL.md")?;
            let meta =
                parse_and_validate_skill_frontmatter(&content).map_err(|e| format!("{:?}", e))?;
            scan_skill_markdown_safety("SKILL.md", &content).map_err(|e| e.to_string())?;
            validate_skill_metadata_text("SKILL.md name", &meta.name).map_err(|e| e.to_string())?;
            validate_skill_metadata_text("SKILL.md description", &meta.description)
                .map_err(|e| e.to_string())?;
            (Some(meta), Some(sha256_hex(&content)))
        } else {
            (None, None)
        };

        if std::fs::symlink_metadata(&plugin_json_path).is_ok() {
            let json_str = Self::read_regular_utf8_nofollow(&plugin_json_path, "plugin.json")?;
            let manifest: SkillManifest =
                serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
            if !(1..=3).contains(&manifest.risk_tier) {
                return Err(format!(
                    "plugin.json risk_tier {} must be within 1..=3",
                    manifest.risk_tier
                ));
            }
            if let Some(params) = &manifest.parameters {
                if !matches!(
                    params,
                    serde_json::Value::Null
                        | serde_json::Value::Bool(_)
                        | serde_json::Value::Object(_)
                ) {
                    return Err(
                        "plugin.json parameters must be an object, boolean, or null".to_string()
                    );
                }
            }
            validate_skill_metadata_text("plugin.json name", &manifest.name)
                .map_err(|e| e.to_string())?;
            validate_skill_metadata_text("plugin.json description", &manifest.description)
                .map_err(|e| e.to_string())?;
            if let Some(ref pov) = manifest.prompt_overview {
                scan_skill_markdown_safety("plugin.json prompt_overview", pov)
                    .map_err(|e| e.to_string())?;
            }
            validate_skill_entry_point(folder, &manifest.entry_point)?;
            if manifest.tags.iter().any(|t| t == "repackaged") && !verified_repackaged {
                crate::SkillRepackager::verify_repackaged_bundle(folder)?;
            }
            if !is_safe_skill_name(&manifest.name) {
                return Err(format!("unsafe skill manifest name: {}", manifest.name));
            }
            if let Some(meta) = &frontmatter {
                verify_frontmatter_parity(meta, &manifest)?;
            }
            return Ok((manifest, md_sha));
        }

        let meta = frontmatter.ok_or_else(|| "missing SKILL.md and plugin.json".to_string())?;
        if !is_safe_skill_name(&meta.name) {
            return Err(format!("unsafe skill manifest name: {}", meta.name));
        }
        let ep = meta.entry_point.unwrap_or_else(|| "SKILL.md".to_string());
        validate_skill_entry_point(folder, &ep)?;
        Ok((
            SkillManifest {
                name: meta.name,
                version: meta.version.unwrap_or_else(|| "1.0.0".to_string()),
                description: meta.description,
                entry_point: ep,
                capabilities_required: meta.capabilities_required,
                risk_tier: meta.risk_tier.unwrap_or(1),
                parameters: None,
                tags: meta.tags,
                prompt_overview: None,
            },
            md_sha,
        ))
    }

    /// Loads a single skill from its folder, enforcing SKILL.md frontmatter and plugin.json parity.
    pub fn load_skill_from_folder(&self, folder: &Path) -> Result<SkillManifest, String> {
        self.load_skill_with_hash(folder).map(|(m, _)| m)
    }

    /// Retrieves a skill manifest by name.
    pub fn get(&self, name: &str) -> Option<&SkillManifest> {
        self.skills.get(name)
    }

    /// Lists all registered skills sorted lexicographically by name.
    pub fn list(&self) -> Vec<&SkillManifest> {
        let mut list: Vec<&SkillManifest> = self.skills.values().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    /// Finds skills requiring a specific capability in deterministic sorted order.
    pub fn find_by_capability(&self, capability: &str) -> Vec<&SkillManifest> {
        self.list()
            .into_iter()
            .filter(|s| s.capabilities_required.iter().any(|c| c == capability))
            .collect()
    }

    /// Converts all registered skills to domain::models::SkillPlugin format in deterministic order.
    pub fn to_domain_plugins(&self) -> Vec<SkillPlugin> {
        self.list()
            .into_iter()
            .map(|s| {
                SkillPlugin::from_core(
                    s.name.clone(),
                    s.name.clone(),
                    s.version.clone(),
                    s.entry_point.clone(),
                    s.capabilities_required.clone(),
                )
            })
            .collect()
    }
}

fn is_safe_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SKILL_NAME_BYTES
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_registration_and_domain_mapping() {
        let mut reg = SkillRegistry::new();
        reg.register(SkillManifest {
            name: "design-guard".to_string(),
            version: "1.0.0".to_string(),
            description: "UX design guard".to_string(),
            entry_point: "index.js".to_string(),
            capabilities_required: vec!["ux-audit".to_string()],
            risk_tier: 1,
            parameters: None,
            tags: vec!["ux".to_string()],
            prompt_overview: None,
        });

        assert_eq!(reg.list().len(), 1);
        let domain_plugins = reg.to_domain_plugins();
        assert_eq!(domain_plugins.len(), 1);
        assert_eq!(domain_plugins[0].id, "design-guard");
        assert_eq!(domain_plugins[0].permissions, vec!["ux-audit"]);
    }
}
