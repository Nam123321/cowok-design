use crate::models::SkillManifest;
use crate::validator::validate_skill_frontmatter;
use domain::models::SkillPlugin;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, SkillManifest>,
    skills_dir: Option<PathBuf>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_directory<P: AsRef<Path>>(dir: P) -> Self {
        let mut registry = Self {
            skills: HashMap::new(),
            skills_dir: Some(dir.as_ref().to_path_buf()),
        };
        let _ = registry.discover();
        registry
    }

    /// Registers a manifest directly
    pub fn register(&mut self, manifest: SkillManifest) {
        self.skills.insert(manifest.name.clone(), manifest);
    }

    /// Scans the configured directory for agentskills.io skills
    pub fn discover(&mut self) -> Result<usize, String> {
        let dir = match &self.skills_dir {
            Some(d) => d.clone(),
            None => return Ok(0),
        };

        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                let plugin_json = path.join("plugin.json");

                if skill_md.exists() || plugin_json.exists() {
                    if let Ok(manifest) = self.load_skill_from_folder(&path) {
                        self.register(manifest);
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    /// Loads a single skill from its folder, validating SKILL.md frontmatter if present
    pub fn load_skill_from_folder(&self, folder: &Path) -> Result<SkillManifest, String> {
        let skill_md_path = folder.join("SKILL.md");
        let plugin_json_path = folder.join("plugin.json");

        if skill_md_path.exists() {
            let content = std::fs::read_to_string(&skill_md_path).map_err(|e| e.to_string())?;
            validate_skill_frontmatter(&content).map_err(|e| format!("{:?}", e))?;
        }

        if plugin_json_path.exists() {
            let json_str = std::fs::read_to_string(&plugin_json_path).map_err(|e| e.to_string())?;
            let manifest: SkillManifest =
                serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
            return Ok(manifest);
        }

        // Fallback: construct from SKILL.md frontmatter
        let folder_name = folder
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        Ok(SkillManifest {
            name: folder_name.to_string(),
            version: "0.1.0".to_string(),
            description: format!("Discovered skill: {}", folder_name),
            entry_point: "SKILL.md".to_string(),
            capabilities_required: Vec::new(),
            risk_tier: 1,
            parameters: None,
            tags: Vec::new(),
            prompt_overview: None,
        })
    }

    /// Retrieves a skill manifest by name
    pub fn get(&self, name: &str) -> Option<&SkillManifest> {
        self.skills.get(name)
    }

    /// Lists all registered skills
    pub fn list(&self) -> Vec<&SkillManifest> {
        let mut list: Vec<&SkillManifest> = self.skills.values().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    /// Finds skills requiring a specific capability
    pub fn find_by_capability(&self, capability: &str) -> Vec<&SkillManifest> {
        self.skills
            .values()
            .filter(|s| s.capabilities_required.iter().any(|c| c == capability))
            .collect()
    }

    /// Converts all registered skills to domain::models::SkillPlugin format
    pub fn to_domain_plugins(&self) -> Vec<SkillPlugin> {
        self.skills
            .values()
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
