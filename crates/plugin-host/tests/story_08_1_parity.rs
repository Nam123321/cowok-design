// @story 08.1
// @cover AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
use plugin_host::registry::SkillRegistry;
use plugin_host::staging::PerRunStager;
use plugin_host::triangle_linter::TriangleLinter;
use plugin_host::validator::{parse_and_validate_skill_frontmatter, validate_skill_frontmatter};
use plugin_host::SkillManifest;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn golden_fixture() -> Value {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path =
        manifest_dir.join("../../packages/builtin-skills/tests/fixtures/story-08.1-golden.json");
    let raw = fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("Failed reading {:?}: {}", fixture_path, e));
    serde_json::from_str(&raw).expect("Golden fixture must be valid JSON")
}

fn builtin_skills_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/builtin-skills")
}

#[test]
fn ut_081_yaml_and_bpe_ssot_gates() {
    let fixture = golden_fixture();
    let shared = &fixture["shared_vectors"];
    let rust_ssot = &fixture["rust_ssot_bpe_vectors"];

    // 1. Valid YAML containing `---` inside quoted scalar must NOT truncate early
    let valid_dash = shared["yaml_valid_with_triple_dash_scalar"]
        .as_str()
        .unwrap();
    let meta = parse_and_validate_skill_frontmatter(valid_dash)
        .expect("Quoted scalar containing '---' must parse cleanly");
    assert_eq!(meta.name, "dash-in-scalar");
    assert_eq!(meta.version.as_deref(), Some("1.0.0"));
    assert_eq!(
        meta.description,
        "Handle --- delimiter inside quoted scalar safely"
    );
    assert_eq!(meta.risk_tier, Some(1));

    // 2. Missing opening delimiter must FAIL (no raw-markdown fallback)
    let no_open = shared["yaml_invalid_no_opening_delimiter"]
        .as_str()
        .unwrap();
    assert!(validate_skill_frontmatter(no_open).is_err());

    // 3. > 1,024 UTF-8 bytes frontmatter must FAIL
    let over_bytes = shared["yaml_invalid_over_1024_bytes"].as_str().unwrap();
    assert!(validate_skill_frontmatter(over_bytes).is_err());

    // 4. Anchors/aliases, custom tags, duplicate keys, and multilingual >100 BPE must FAIL
    for key in [
        "yaml_invalid_anchor_alias",
        "yaml_invalid_custom_tag",
        "yaml_invalid_duplicate_key",
        "yaml_vietnamese_multilingual_over_100_bpe",
    ] {
        let raw = rust_ssot[key].as_str().unwrap();
        assert!(
            validate_skill_frontmatter(raw).is_err(),
            "Expected validation failure for vector '{key}'"
        );
    }
}

#[test]
fn ut_081_kv_hash_golden_parity_and_mutations() {
    let fixture = golden_fixture();
    let kv = &fixture["shared_vectors"]["kv_hash_parity"];
    let expected = kv["expected_stable_prompt_hash"].as_str().unwrap();

    let parse_entries = |arr: &Vec<Value>| -> Vec<(SkillManifest, Option<String>)> {
        arr.iter()
            .map(|item| {
                let manifest: SkillManifest =
                    serde_json::from_value(item["manifest"].clone()).unwrap();
                let sha = item["skill_md_sha256"].as_str().map(|s| s.to_string());
                (manifest, sha)
            })
            .collect()
    };

    let entries_a = parse_entries(kv["entries_variant_a"].as_array().unwrap());
    let entries_b = parse_entries(kv["entries_variant_b"].as_array().unwrap());

    let refs_a: Vec<(&SkillManifest, Option<&str>)> =
        entries_a.iter().map(|(m, s)| (m, s.as_deref())).collect();
    let refs_b: Vec<(&SkillManifest, Option<&str>)> =
        entries_b.iter().map(|(m, s)| (m, s.as_deref())).collect();

    let hash_a = PerRunStager::compute_stable_prompt_hash_with_sources(&refs_a);
    let hash_b = PerRunStager::compute_stable_prompt_hash_with_sources(&refs_b);
    assert_eq!(hash_a, expected);
    assert_eq!(hash_b, expected);

    // Every single prompt-relevant mutation must bust cache hash
    let (base_m, base_sha) = &entries_a[0];
    let other = &entries_a[1];

    let mut mut_overview = base_m.clone();
    mut_overview.prompt_overview = Some("Different prompt".to_string());
    assert_ne!(
        PerRunStager::compute_stable_prompt_hash_with_sources(&[
            (&mut_overview, base_sha.as_deref()),
            (&other.0, other.1.as_deref())
        ]),
        expected
    );

    let mut mut_params = base_m.clone();
    mut_params.parameters = Some(serde_json::json!({"z_opt": false}));
    assert_ne!(
        PerRunStager::compute_stable_prompt_hash_with_sources(&[
            (&mut_params, base_sha.as_deref()),
            (&other.0, other.1.as_deref())
        ]),
        expected
    );

    let mut mut_tier = base_m.clone();
    mut_tier.risk_tier = 3;
    assert_ne!(
        PerRunStager::compute_stable_prompt_hash_with_sources(&[
            (&mut_tier, base_sha.as_deref()),
            (&other.0, other.1.as_deref())
        ]),
        expected
    );

    let mut mut_tag = base_m.clone();
    mut_tag.tags.push("extra".to_string());
    assert_ne!(
        PerRunStager::compute_stable_prompt_hash_with_sources(&[
            (&mut_tag, base_sha.as_deref()),
            (&other.0, other.1.as_deref())
        ]),
        expected
    );

    let changed_sha = "c".repeat(64);
    assert_ne!(
        PerRunStager::compute_stable_prompt_hash_with_sources(&[
            (base_m, Some(changed_sha.as_str())),
            (&other.0, other.1.as_deref())
        ]),
        expected
    );
}

#[test]
fn ut_081_atomic_readonly_staging_and_prompt_sanitization() {
    let temp = tempfile::tempdir().unwrap();
    let non_existent_base = temp.path().join("nested/cowok-skills");
    let stager = PerRunStager::new(&non_existent_base);

    let skill = SkillManifest {
        name: "safe-skill".to_string(),
        version: "1.0.0".to_string(),
        description: "Safe staging skill".to_string(),
        entry_point: "SKILL.md".to_string(),
        capabilities_required: vec!["sdi".to_string()],
        risk_tier: 1,
        parameters: Some(serde_json::json!({"b": 2, "a": 1})),
        tags: vec!["core".to_string()],
        prompt_overview: Some("Clean deterministic prompt.".to_string()),
    };
    let skill_sha = "d".repeat(64);

    let receipt = stager
        .stage_run_with_sources("run-readonly", &[(&skill, Some(skill_sha.as_str()))])
        .expect("Atomic staging should succeed");
    assert_eq!(receipt.run_id, "run-readonly");

    // Reusing existing run directory must fail closed
    assert!(stager
        .stage_run_with_sources("run-readonly", &[(&skill, Some(skill_sha.as_str()))])
        .is_err());

    // Pre-existing symlink run directory must fail closed
    #[cfg(unix)]
    {
        let target = temp.path().join("symlink-target");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, non_existent_base.join("run-symlink")).unwrap();
        assert!(stager
            .stage_run_with_sources("run-symlink", &[(&skill, None)])
            .is_err());
    }

    // Staged files and directories must be read-only
    let staged_prompt = non_existent_base.join("run-readonly/safe-skill/PROMPT.md");
    assert!(fs::metadata(&staged_prompt)
        .unwrap()
        .permissions()
        .readonly());
    assert!(fs::write(&staged_prompt, "tamper").is_err());

    // Prompt injection / hidden Unicode / beacons must be rejected and leave no partial run dir
    let fixture = golden_fixture();
    for (idx, item) in fixture["shared_vectors"]["prompt_injection_rejections"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let mut bad = skill.clone();
        bad.prompt_overview = Some(item["content"].as_str().unwrap().to_string());
        let bad_run = format!("run-bad-{idx}");
        assert!(
            stager.stage_run(&bad_run, &[bad]).is_err(),
            "Expected prompt sanitization rejection for {}",
            item["id"]
        );
        assert!(
            !non_existent_base.join(&bad_run).exists(),
            "Failed staging must clean up partial run dir"
        );
    }

    // cleanup_run cleanly removes read-only 0o555 dirs without EACCES
    stager
        .cleanup_run("run-readonly")
        .expect("cleanup_run must restore permissions and remove run dir");
    assert!(!non_existent_base.join("run-readonly").exists());
}

#[test]
fn ut_081_registry_parity_and_builtin_skills_audit() {
    let skills_dir = builtin_skills_root().join("skills");
    let reg = SkillRegistry::with_directory(&skills_dir);
    let list = reg.list();
    assert!(
        list.len() >= 9,
        "Expected >= 9 built-in skills, found {}",
        list.len()
    );

    // Every built-in skill must have a recorded SKILL.md SHA-256 and match to_domain_plugins ordering
    let domain_plugins = reg.to_domain_plugins();
    let names_from_list: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
    let names_from_domain: Vec<&str> = domain_plugins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names_from_list, names_from_domain);
    for name in &names_from_list {
        assert!(reg.skill_md_sha256(name).is_some());
    }

    // Mismatch between SKILL.md and plugin.json must fail closed
    let temp = tempfile::tempdir().unwrap();
    let mismatch_dir = temp.path().join("mismatch-skill");
    fs::create_dir(&mismatch_dir).unwrap();
    fs::write(
        mismatch_dir.join("SKILL.md"),
        "---\nname: mismatch-skill\nversion: 1.0.0\ndescription: Matches?\nrisk_tier: 1\n---\n# Body\n",
    )
    .unwrap();
    fs::write(
        mismatch_dir.join("plugin.json"),
        r#"{"name":"mismatch-skill","version":"2.0.0","description":"Matches?","risk_tier":1}"#,
    )
    .unwrap();
    assert!(reg.load_skill_from_folder(&mismatch_dir).is_err());

    // SKILL.md-only fallback preserves frontmatter fields instead of dummy defaults
    let md_only_dir = temp.path().join("md-only");
    fs::create_dir(&md_only_dir).unwrap();
    fs::write(
        md_only_dir.join("SKILL.md"),
        "---\nname: md-only\nversion: 3.1.4\ndescription: Frontmatter description\nrisk_tier: 2\n---\n",
    )
    .unwrap();
    let loaded = reg.load_skill_from_folder(&md_only_dir).unwrap();
    assert_eq!(loaded.version, "3.1.4");
    assert_eq!(loaded.description, "Frontmatter description");
    assert_eq!(loaded.risk_tier, 2);
}

#[test]
fn perf_081_linter_security_hygiene_and_budget() {
    let fixture = golden_fixture();
    for item in fixture["shared_vectors"]["linter_security_hygiene"]
        .as_array()
        .unwrap()
    {
        let content = item["content"].as_str().unwrap();
        let expected_rule = item["expected_rule"].as_str().unwrap();
        let report = TriangleLinter::lint(content);
        assert!(!report.passed);
        assert!(
            report.issues.iter().any(|i| i.rule_id == expected_rule),
            "Missing rule {expected_rule} for {}",
            item["id"]
        );
    }

    // F-03 regression: multi-line <img and multi-line Markdown external beacons
    let multiline_img = "<img\n  src=\"https://tracker.invalid/p.gif\"\n  alt=\"beacon\"\n/>";
    let ml_img_report = TriangleLinter::lint(multiline_img);
    assert!(ml_img_report
        .issues
        .iter()
        .any(|i| i.rule_id == "ANTI_SLOP_07_EXTERNAL_BEACON"));

    let multiline_md = "![tracker](\nhttps://tracker.invalid/p.gif\n)";
    let ml_md_report = TriangleLinter::lint(multiline_md);
    assert!(ml_md_report
        .issues
        .iter()
        .any(|i| i.rule_id == "ANTI_SLOP_07_EXTERNAL_BEACON"));

    // F-03/F-081-W2-01 regression: Unicode case expansion before beacon must not panic
    for prefix in ["\u{1E9E}", "\u{0130}"] {
        let img_payload = format!("{prefix}<img src=\"https://tracker.invalid/p.gif\">");
        let rep = TriangleLinter::lint(&img_payload);
        assert!(rep
            .issues
            .iter()
            .any(|i| i.rule_id == "ANTI_SLOP_07_EXTERNAL_BEACON"));

        let md_payload = format!("{prefix}![tracker](https://tracker.invalid/p.gif)");
        let rep_md = TriangleLinter::lint(&md_payload);
        assert!(rep_md
            .issues
            .iter()
            .any(|i| i.rule_id == "ANTI_SLOP_07_EXTERNAL_BEACON"));
    }

    // Worst-case 100-iteration performance budget < 25ms
    let corpus = "<div style=\"background:#6366f1\"><button class=\"btn\">Deploy</button><script>x()</script></div>\n".repeat(40);
    // Warm
    let _ = TriangleLinter::lint(&corpus);
    let mut durations = Vec::with_capacity(100);
    for _ in 0..100 {
        let start = Instant::now();
        let report = TriangleLinter::lint(&corpus);
        assert!(!report.passed);
        durations.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = durations[94];
    let max = *durations.last().unwrap();
    assert!(
        p95 < 25.0 && max < 25.0,
        "Linter exceeded 25ms: p95={p95:.2}ms max={max:.2}ms"
    );
}
