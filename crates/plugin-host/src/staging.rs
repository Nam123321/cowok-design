use crate::models::{SkillManifest, StagingRunReceipt};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Manages per-run ephemeral skill staging (.cowok-skills/<run_id>/)
/// and calculates stable prompt hashes for KV Prompt Cache preservation.
pub struct PerRunStager {
    base_staging_dir: PathBuf,
}

impl PerRunStager {
    pub fn new<P: AsRef<Path>>(base_staging_dir: P) -> Self {
        Self {
            base_staging_dir: base_staging_dir.as_ref().to_path_buf(),
        }
    }

    /// Computes deterministic stable_prompt_hash across sorted skill manifests
    pub fn compute_stable_prompt_hash(skills: &[SkillManifest]) -> String {
        let mut sorted_skills = skills.to_vec();
        sorted_skills.sort_by(|a, b| a.name.cmp(&b.name));

        let mut hasher = Sha256::new();
        for skill in &sorted_skills {
            hasher.update(skill.name.as_bytes());
            hasher.update(b"::");
            hasher.update(skill.version.as_bytes());
            hasher.update(b"::");
            hasher.update(skill.description.as_bytes());
            hasher.update(b"::");
            for cap in &skill.capabilities_required {
                hasher.update(cap.as_bytes());
                hasher.update(b",");
            }
            hasher.update(b";");
        }
        format!("{:x}", hasher.finalize())
    }

    /// Stages skills into `.cowok-skills/<run_id>/` and writes a staging receipt
    pub fn stage_run(
        &self,
        run_id: &str,
        skills: &[SkillManifest],
    ) -> std::io::Result<StagingRunReceipt> {
        let run_dir = self.base_staging_dir.join(run_id);
        std::fs::create_dir_all(&run_dir)?;

        let mut staged_names = Vec::new();
        for skill in skills {
            let skill_dir = run_dir.join(&skill.name);
            std::fs::create_dir_all(&skill_dir)?;

            let manifest_path = skill_dir.join("plugin.json");
            let json = serde_json::to_string_pretty(skill).map_err(std::io::Error::other)?;
            std::fs::write(manifest_path, json)?;

            if let Some(overview) = &skill.prompt_overview {
                let overview_path = skill_dir.join("PROMPT.md");
                std::fs::write(overview_path, overview)?;
            }

            staged_names.push(skill.name.clone());
        }

        let stable_prompt_hash = Self::compute_stable_prompt_hash(skills);
        let receipt = StagingRunReceipt {
            run_id: run_id.to_string(),
            staged_skills: staged_names,
            stable_prompt_hash: stable_prompt_hash.clone(),
            staged_dir: run_dir.to_string_lossy().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        let receipt_path = run_dir.join("run-receipt.json");
        let receipt_json = serde_json::to_string_pretty(&receipt).map_err(std::io::Error::other)?;
        std::fs::write(receipt_path, receipt_json)?;

        Ok(receipt)
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

        // Must be order-independent to preserve KV cache
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
    }
}
