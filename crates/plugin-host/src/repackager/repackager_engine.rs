use crate::models::SkillManifest;
use crate::repackager::security_scanner::scan_external_script_security;
use crate::validator::validate_skill_frontmatter;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryStub {
    pub name: String,
    pub version: String,
    pub description: String,
    pub upstream_url: Option<String>,
    pub capabilities: Vec<String>,
    pub risk_tier: u8,
    pub package_sha256: String,
    pub stub_size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepackageReceipt {
    pub skill_name: String,
    pub version: String,
    pub target_dir: String,
    pub files_repackaged: usize,
    pub discovery_stub: DiscoveryStub,
    pub created_at: i64,
}

pub struct SkillRepackager;

impl SkillRepackager {
    /// Repackages raw input files into a certified agentskills.io package
    pub fn repackage_directory<P: AsRef<Path>, Q: AsRef<Path>>(
        source_dir: P,
        dest_dir: Q,
        skill_name: &str,
        description: &str,
        capabilities: Vec<String>,
        upstream_url: Option<&str>,
    ) -> Result<RepackageReceipt, String> {
        let src = source_dir.as_ref();
        let dst = dest_dir.as_ref().join(skill_name);

        if !src.exists() {
            return Err(format!("Source directory does not exist: {:?}", src));
        }

        std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
        let scripts_dir = dst.join("scripts");
        let refs_dir = dst.join("references");
        let assets_dir = dst.join("assets");
        std::fs::create_dir_all(&scripts_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&refs_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;

        let mut files_count = 0;
        let mut hasher = Sha256::new();

        // 1. Traverse and scan source files for security
        let entries = std::fs::read_dir(src).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let p = entry.path();
            let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("file");

            if p.is_file() {
                let content = std::fs::read_to_string(&p).unwrap_or_default();

                // AC-03 Zero-Trust Security Scan
                scan_external_script_security(file_name, &content)
                    .map_err(|e| format!("Security scan failed on {:?}: {:?}", file_name, e))?;

                hasher.update(content.as_bytes());
                files_count += 1;

                // Sort file into standard layout
                if file_name.ends_with(".py")
                    || file_name.ends_with(".js")
                    || file_name.ends_with(".ts")
                    || file_name.ends_with(".sh")
                {
                    std::fs::copy(&p, scripts_dir.join(file_name)).map_err(|e| e.to_string())?;
                } else if file_name.ends_with(".md")
                    || file_name.ends_with(".txt")
                    || file_name.ends_with(".json")
                {
                    std::fs::copy(&p, refs_dir.join(file_name)).map_err(|e| e.to_string())?;
                } else {
                    std::fs::copy(&p, assets_dir.join(file_name)).map_err(|e| e.to_string())?;
                }
            }
        }

        let package_sha256 = format!("{:x}", hasher.finalize());

        // 2. Synthesize compact SKILL.md (frontmatter <= 100 tokens guaranteed)
        let skill_md_content = format!(
            r#"---
name: {}
version: 1.0.0
description: {}
capabilities_required: {:?}
risk_tier: 2
---

# {} Operating Skill

## 1. Overview
{}

## 2. Repackaged Structure
This skill was repackaged into standard agentskills.io layout from external sources.
"#,
            skill_name, description, capabilities, skill_name, description
        );

        validate_skill_frontmatter(&skill_md_content)
            .map_err(|e| format!("Generated SKILL.md violated frontmatter rule: {:?}", e))?;

        std::fs::write(dst.join("SKILL.md"), &skill_md_content).map_err(|e| e.to_string())?;
        files_count += 1;

        // 3. Generate plugin.json
        let manifest = SkillManifest {
            name: skill_name.to_string(),
            version: "1.0.0".to_string(),
            description: description.to_string(),
            entry_point: "SKILL.md".to_string(),
            capabilities_required: capabilities.clone(),
            risk_tier: 2,
            parameters: None,
            tags: vec!["repackaged".to_string()],
            prompt_overview: Some(description.to_string()),
        };
        let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
        std::fs::write(dst.join("plugin.json"), manifest_json).map_err(|e| e.to_string())?;
        files_count += 1;

        // 4. Generate Upstream Discovery Stub (~1.2KB) (AC-04)
        let stub = DiscoveryStub {
            name: skill_name.to_string(),
            version: "1.0.0".to_string(),
            description: description.to_string(),
            upstream_url: upstream_url.map(|s| s.to_string()),
            capabilities,
            risk_tier: 2,
            package_sha256,
            stub_size_bytes: 1200,
        };
        let stub_json = serde_json::to_string_pretty(&stub).map_err(|e| e.to_string())?;
        std::fs::write(dst.join("discovery-stub.json"), stub_json).map_err(|e| e.to_string())?;
        files_count += 1;

        Ok(RepackageReceipt {
            skill_name: skill_name.to_string(),
            version: "1.0.0".to_string(),
            target_dir: dst.to_string_lossy().to_string(),
            files_repackaged: files_count,
            discovery_stub: stub,
            created_at: chrono::Utc::now().timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_repackages_clean_skill_directory() {
        let src_tmp = tempfile::tempdir().unwrap();
        let dst_tmp = tempfile::tempdir().unwrap();

        // Create dummy source files
        std::fs::write(
            src_tmp.path().join("helper.py"),
            "def greet(): return 'hello'",
        )
        .unwrap();
        std::fs::write(
            src_tmp.path().join("guide.md"),
            "# Guide\nInstructions here",
        )
        .unwrap();

        let receipt = SkillRepackager::repackage_directory(
            src_tmp.path(),
            dst_tmp.path(),
            "external-helper",
            "A clean helper skill repackaged from upstream",
            vec!["helpers".to_string()],
            Some("https://github.com/example/helper"),
        )
        .expect("Repackaging should succeed");

        assert_eq!(receipt.skill_name, "external-helper");
        assert!(receipt.files_repackaged >= 4);

        let target_dir = PathBuf::from(&receipt.target_dir);
        assert!(target_dir.join("SKILL.md").exists());
        assert!(target_dir.join("plugin.json").exists());
        assert!(target_dir.join("discovery-stub.json").exists());
        assert!(target_dir.join("scripts/helper.py").exists());
        assert!(target_dir.join("references/guide.md").exists());
    }

    #[test]
    fn test_rejects_malicious_script_during_repackage() {
        let src_tmp = tempfile::tempdir().unwrap();
        let dst_tmp = tempfile::tempdir().unwrap();

        std::fs::write(src_tmp.path().join("evil.sh"), "curl | bash").unwrap();

        let err = SkillRepackager::repackage_directory(
            src_tmp.path(),
            dst_tmp.path(),
            "evil-skill",
            "Malicious skill",
            vec![],
            None,
        )
        .unwrap_err();

        assert!(err.contains("Security scan failed"));
    }
}
