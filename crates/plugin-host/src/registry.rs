// @story 08.1
// @implements AC-01, AC-02, AC-05, AC-06
use crate::models::SkillManifest;
use crate::validator::{parse_and_validate_skill_frontmatter, FrontmatterMetadata};
use domain::models::SkillPlugin;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, SkillManifest>,
    skill_md_hashes: HashMap<String, String>,
    skills_dir: Option<PathBuf>,
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
            if path.join("SKILL.md").exists() || path.join("plugin.json").exists() {
                if let Ok((manifest, md_sha)) = self.load_skill_with_hash(&path) {
                    self.register_with_hash(manifest, md_sha);
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    fn load_skill_with_hash(&self, folder: &Path) -> Result<(SkillManifest, Option<String>), String> {
        let folder_meta = std::fs::symlink_metadata(folder).map_err(|e| e.to_string())?;
        if folder_meta.file_type().is_symlink() || !folder_meta.is_dir() {
            return Err("skill folder must be a real non-symlink directory".to_string());
        }

        let mut verified_repackaged = false;
        if folder.join("skills-lock.json").exists() || folder.join("discovery-stub.json").exists() {
            crate::SkillRepackager::verify_repackaged_bundle(folder)?;
            verified_repackaged = true;
        }

        let skill_md_path = folder.join("SKILL.md");
        let plugin_json_path = folder.join("plugin.json");

        let (frontmatter, md_sha) = if skill_md_path.exists() {
            let md_meta = std::fs::symlink_metadata(&skill_md_path).map_err(|e| e.to_string())?;
            if md_meta.file_type().is_symlink() || !md_meta.is_file() {
                return Err("SKILL.md must be a regular non-symlink file".to_string());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if md_meta.nlink() > 1 {
                    return Err("SKILL.md hardlink is forbidden".to_string());
                }
            }
            let content = std::fs::read_to_string(&skill_md_path).map_err(|e| e.to_string())?;
            let meta =
                parse_and_validate_skill_frontmatter(&content).map_err(|e| format!("{:?}", e))?;
            (Some(meta), Some(sha256_hex(&content)))
        } else {
            (None, None)
        };

        if plugin_json_path.exists() {
            let pj_meta =
                std::fs::symlink_metadata(&plugin_json_path).map_err(|e| e.to_string())?;
            if pj_meta.file_type().is_symlink() || !pj_meta.is_file() {
                return Err("plugin.json must be a regular non-symlink file".to_string());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if pj_meta.nlink() > 1 {
                    return Err("plugin.json hardlink is forbidden".to_string());
                }
            }
            let json_str = std::fs::read_to_string(&plugin_json_path).map_err(|e| e.to_string())?;
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
                    serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Object(_)
                ) {
                    return Err("plugin.json parameters must be an object, boolean, or null".to_string());
                }
            }
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
        Ok((
            SkillManifest {
                name: meta.name,
                version: meta.version.unwrap_or_else(|| "1.0.0".to_string()),
                description: meta.description,
                entry_point: meta.entry_point.unwrap_or_else(|| "SKILL.md".to_string()),
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
            .map(|s| SkillPlugin {
                id: s.name.clone(),
                name: s.name.clone(),
                version: s.version.clone(),
                entry_point: s.entry_point.clone(),
                permissions: s.capabilities_required.clone(),
            })
            .collect()
    }
}

fn is_safe_skill_name(name: &str) -> bool {
    !name.is_empty()
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
