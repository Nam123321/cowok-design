// @story 03.2
// @cover AC-01, AC-02, AC-03
use constraint_engine::{
    ActiveConstraint, AstLinter, ComponentRegistryContract, ComponentRegistryEntry, IconManifest,
    IconManifestEntry, IconValidator, TypographyContract,
};

#[test]
fn test_flagship_icon_manifest_loads_and_verifies_143_icons() {
    let manifest = IconValidator::get_flagship_manifest();
    assert_eq!(manifest.name, "cowok.ai Iconography v2");
    assert_eq!(manifest.count, 143);
    assert_eq!(manifest.icons.len(), 143);

    // Verify key flagship icons exist
    let icon_names: Vec<&str> = manifest.icons.iter().map(|i| i.name.as_str()).collect();
    assert!(icon_names.contains(&"activity"));
    assert!(icon_names.contains(&"ai-agent"));
    assert!(icon_names.contains(&"settings"));
    assert!(icon_names.contains(&"bell"));
    assert!(icon_names.contains(&"search"));
}

#[test]
fn test_load_manifest_from_file() {
    let crate_asset = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/flagship-icon-manifest.json");
    assert!(
        crate_asset.is_file(),
        "Crate-local flagship asset must exist at {}",
        crate_asset.display()
    );
    let crate_loaded = IconValidator::load_from_file(&crate_asset)
        .expect("Should load crate-local flagship manifest");
    assert_eq!(crate_loaded.count, 143);
    assert_eq!(crate_loaded.icons.len(), 143);

    let brand_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../Brand-guideline/cowok-brand-product-system-v4.3/iconography/icon-manifest.json",
    );
    if brand_path.exists() {
        let brand_loaded =
            IconValidator::load_from_file(&brand_path).expect("Should load brand manifest");
        assert_eq!(brand_loaded.count, crate_loaded.count);
        assert_eq!(brand_loaded.icons.len(), crate_loaded.icons.len());
    }
}

#[test]
fn test_valid_icon_validation() {
    let manifest = IconValidator::get_flagship_manifest();

    let report1 = IconValidator::validate_icon(&manifest, "activity");
    assert!(report1.is_valid);
    assert_eq!(report1.requested_icon, "activity");
    assert!(report1.suggestions.is_empty());

    let report2 = IconValidator::validate_icon(&manifest, "ai-agent");
    assert!(report2.is_valid);
    assert!(report2.suggestions.is_empty());

    // Case-insensitive matching
    let report3 = IconValidator::validate_icon(&manifest, "SETTINGS");
    assert!(report3.is_valid);
    assert!(report3.suggestions.is_empty());
}

#[test]
fn test_invalid_icon_returns_top3_suggestions_via_levenshtein() {
    let manifest = IconValidator::get_flagship_manifest();

    // Typo: "activty" -> should suggest "activity"
    let report = IconValidator::validate_icon(&manifest, "activty");
    assert!(!report.is_valid);
    assert_eq!(report.suggestions.len(), 3);
    assert_eq!(report.suggestions[0], "activity");

    // Typo: "ai_agent" -> should suggest "ai-agent"
    let report2 = IconValidator::validate_icon(&manifest, "ai_agent");
    assert!(!report2.is_valid);
    assert_eq!(report2.suggestions.len(), 3);
    assert!(report2.suggestions.contains(&"ai-agent".to_string()));

    // Typo: "settngs" -> should suggest "settings"
    let report3 = IconValidator::validate_icon(&manifest, "settngs");
    assert!(!report3.is_valid);
    assert_eq!(report3.suggestions.len(), 3);
    assert_eq!(report3.suggestions[0], "settings");
}

#[test]
fn test_empty_icon_name_returns_invalid_with_fallback() {
    let manifest = IconValidator::get_flagship_manifest();
    let report = IconValidator::validate_icon(&manifest, "   ");
    assert!(!report.is_valid);
    assert_eq!(report.suggestions.len(), 3);
}

#[test]
fn test_custom_icon_manifest_parsing() {
    let custom_json = r#"{
        "name": "custom-icon-manifest",
        "status": "ready",
        "count": 2,
        "icons": [
            { "name": "custom-home", "tier": "tier1", "file": "custom-home.svg", "source": "local", "tags": ["nav"] },
            { "name": "custom-user", "tier": "tier1", "file": "custom-user.svg", "source": "local", "tags": ["auth"] }
        ]
    }"#;

    let manifest = IconValidator::parse_manifest(custom_json).expect("should parse valid JSON");
    assert_eq!(manifest.count, 2);

    let report_valid = IconValidator::validate_icon(&manifest, "custom-home");
    assert!(report_valid.is_valid);

    let report_invalid = IconValidator::validate_icon(&manifest, "custom-hom");
    assert!(!report_invalid.is_valid);
    assert_eq!(report_invalid.suggestions[0], "custom-home");
}

#[test]
fn test_typography_contract_validation() {
    let contract = TypographyContract::default();

    // Valid configuration
    let valid_result =
        IconValidator::validate_typography(&contract, "Inter", 400, Some("16px"), 4.8);
    assert!(valid_result.is_ok());

    // Disallowed font family
    let invalid_font =
        IconValidator::validate_typography(&contract, "Comic Sans MS", 400, Some("16px"), 5.0);
    assert!(invalid_font.is_err());
    let errors = invalid_font.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("Comic Sans MS")));

    // Disallowed font weight
    let invalid_weight =
        IconValidator::validate_typography(&contract, "Geist", 300, Some("16px"), 5.0);
    assert!(invalid_weight.is_err());
    let errors = invalid_weight.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("300")));

    // Disallowed font size
    let invalid_size =
        IconValidator::validate_typography(&contract, "Geist", 400, Some("99px"), 5.0);
    assert!(invalid_size.is_err());
    let errors = invalid_size.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("99px")));

    // Contrast ratio below WCAG AA threshold 4.5
    let invalid_contrast = IconValidator::validate_typography(&contract, "Geist", 500, None, 3.2);
    assert!(invalid_contrast.is_err());
    let errors = invalid_contrast.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("WCAG")));

    // CSS rem/px normalization equivalence (1rem == 16px, 0.875rem == 14px)
    let rem_ok = IconValidator::validate_typography(&contract, "Geist", 500, Some("1rem"), 4.6);
    assert!(rem_ok.is_ok(), "1rem should match 16px in contract");
    let report_ok =
        IconValidator::validate_typography_report(&contract, "Geist", 400, Some("0.875rem"), 4.5);
    assert!(report_ok.is_valid);
    assert!(report_ok.errors.is_empty());
}

#[test]
fn test_fail_closed_jail_and_manifest_bounds() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(outside.path().join("secret.json"), "{}").expect("write outside");

    // Path traversal blocked
    let err_trav = IconValidator::resolve_jailed_manifest_path(tmp.path(), "../escape.json");
    assert!(err_trav.is_err());

    // Missing file blocked
    let err_missing =
        IconValidator::resolve_jailed_manifest_path(tmp.path(), "does-not-exist.json");
    assert!(err_missing.is_err());

    // Symlink escaping jail blocked
    #[cfg(unix)]
    {
        let link = tmp.path().join("sym-escape.json");
        std::os::unix::fs::symlink(outside.path().join("secret.json"), &link).expect("symlink");
        let err_sym = IconValidator::resolve_jailed_manifest_path(tmp.path(), "sym-escape.json");
        assert!(err_sym.is_err(), "Symlink outside jail must be rejected");
    }

    // Mismatched count fails parse_manifest
    let bad_count = r#"{"name":"x","status":"ok","count":99,"icons":[]}"#;
    assert!(IconValidator::parse_manifest(bad_count).is_err());

    // Over-long identifier (> 64 chars) rejected
    let long_name = "a".repeat(80);
    let too_long = serde_json::json!({
        "name": "test",
        "status": "ok",
        "count": 1,
        "icons": [{ "name": long_name, "tier": "brand", "file": "a.svg", "source": "custom", "tags": [] }]
    })
    .to_string();
    assert!(IconValidator::parse_manifest(&too_long).is_err());

    // Over 2048 icons rejected (DoS guard)
    let big_icons: Vec<_> = (0..2049)
        .map(|i| IconManifestEntry {
            name: format!("icon-{i}"),
            tier: "gen".to_string(),
            file: format!("icon-{i}.svg"),
            source: "custom".to_string(),
            tags: Vec::new(),
        })
        .collect();
    let too_many = serde_json::to_string(&IconManifest {
        name: "huge".to_string(),
        status: Some("ok".to_string()),
        count: big_icons.len(),
        icons: big_icons,
    })
    .expect("serialize big");
    assert!(IconValidator::parse_manifest(&too_many).is_err());
}

#[test]
fn test_allowed_icon_filter_and_component_registry() {
    let flagship = IconValidator::get_flagship_manifest();
    let filtered = IconValidator::apply_allowed_icon_filter(
        flagship,
        &["activity".to_string(), "bell".to_string()],
    );
    assert_eq!(filtered.count, 2);
    assert!(IconValidator::validate_icon(&filtered, "activity").is_valid);
    assert!(!IconValidator::validate_icon(&filtered, "search").is_valid);

    let md = r#"
# UI Spec
## 07_COMPONENT_REGISTRY
- `PrimaryButton` - Action CTA
- `DataTableView` - Tabular data grid
## 08_NEXT
"#;
    let contract = IconValidator::extract_component_registry_from_markdown(md, true);
    assert!(contract.forbid_unregistered);
    assert_eq!(contract.components.len(), 2);
    assert!(IconValidator::validate_component(&contract, "PrimaryButton").is_valid);
    let typo_comp = IconValidator::validate_component(&contract, "PrimaryButon");
    assert!(!typo_comp.is_valid);
    assert_eq!(typo_comp.suggestions[0], "PrimaryButton");
}

#[test]
fn test_ast_linter_enforces_icons_typography_and_components() {
    let flagship = IconValidator::get_flagship_manifest();
    let typo = TypographyContract::default();
    let comp = ComponentRegistryContract {
        components: vec![ComponentRegistryEntry {
            name: "ActionButton".to_string(),
            source_spec: Some("07_COMPONENT_REGISTRY".to_string()),
            allowed_props: vec![],
        }],
        forbid_unregistered: true,
    };
    let rules = vec![ActiveConstraint {
        rule_code: "ENFORCE_DESIGN_SYSTEM".to_string(),
        severity: "ERROR".to_string(),
        config_json: "{}".to_string(),
    }];

    // Clean TSX passes (1rem == 16px, valid flagship icon 'activity', registered ActionButton, HTML span)
    let clean_tsx = r#"
export function Card() {
  return (
    <ActionButton>
      <BrandIcon name="activity" />
      <span style={{ fontFamily: "Geist", fontSize: "1rem", fontWeight: 500 }}>OK</span>
    </ActionButton>
  );
}
"#;
    let clean_v = AstLinter::lint_source_with_context(
        clean_tsx,
        &rules,
        Some(&flagship),
        Some(&typo),
        Some(&comp),
    )
    .expect("lint clean");
    assert!(clean_v.is_empty(), "Expected 0 violations, got {clean_v:?}");

    // Bad TSX flags icon typo, inline typography violations, and unregistered JSX component
    let bad_tsx = r#"
export function Card() {
  return (
    <PhantomModal>
      <BrandIcon name="activty" />
      <span style={{ fontFamily: "Comic Sans MS", fontSize: "99px", fontWeight: 250 }}>Bad</span>
    </PhantomModal>
  );
}
"#;
    let bad_v = AstLinter::lint_source_with_context(
        bad_tsx,
        &rules,
        Some(&flagship),
        Some(&typo),
        Some(&comp),
    )
    .expect("lint bad");
    assert!(bad_v.iter().any(|v| v.rule_code == "ENFORCE_ICON_MANIFEST"));
    assert!(bad_v.iter().any(|v| v.rule_code == "ENFORCE_TYPOGRAPHY"));
    assert!(bad_v
        .iter()
        .any(|v| v.rule_code == "ENFORCE_COMPONENT_REGISTRY"));
}

#[test]
fn test_ast_linter_wcag_ternary_icon_and_allowed_props() {
    let flagship = IconValidator::get_flagship_manifest();
    let typo = TypographyContract::default();
    let comp = ComponentRegistryContract {
        components: vec![ComponentRegistryEntry {
            name: "ActionButton".to_string(),
            source_spec: Some("07_COMPONENT_REGISTRY".to_string()),
            allowed_props: vec!["variant".to_string()],
        }],
        forbid_unregistered: true,
    };
    let rules = vec![ActiveConstraint {
        rule_code: "ENFORCE_DESIGN_SYSTEM".to_string(),
        severity: "BLOCKER".to_string(),
        config_json: "{}".to_string(),
    }];

    // Low contrast (#999 on #aaa => ~1.26 < 4.5), ternary icon invalid branch, forbidden prop, and JSX inside if-statement
    let tsx = r##"
export function ConditionalView({ ok }: { ok: boolean }) {
  if (ok) {
    return (
      <ActionButton variant="primary" rogueProp="1">
        <CowokIcon name={ok ? "activity" : "missing-icon-branch"} />
        <span style={{ fontFamily: "Geist", fontSize: "1em", color: "#999999", backgroundColor: "#aaaaaa" }}>LowContrast</span>
      </ActionButton>
    );
  }
  return null;
}
"##;
    let v = AstLinter::lint_source_with_context(
        tsx,
        &rules,
        Some(&flagship),
        Some(&typo),
        Some(&comp),
    )
    .expect("lint tsx");

    assert!(v.iter().any(|x| x.rule_code == "ENFORCE_ICON_MANIFEST" && x.message.contains("missing-icon-branch")));
    assert!(v.iter().any(|x| x.rule_code == "ENFORCE_TYPOGRAPHY" && x.message.contains("WCAG")));
    assert!(v.iter().any(|x| x.rule_code == "ENFORCE_COMPONENT_REGISTRY" && x.message.contains("rogueProp")));
}

#[test]
fn test_component_registry_extract_stops_at_next_heading() {
    let md = r#"
# UI Spec
## 07_COMPONENT_REGISTRY
- `PrimaryButton` - Action CTA
### Sub-section inside registry
- `SubPanel` - Nested entry
## 08_NEXT_SECTION
- `OutsideHeadingComponent` - Must NOT be imported
"#;
    let contract = IconValidator::extract_component_registry_from_markdown(md, true);
    let names: Vec<&str> = contract.components.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"PrimaryButton"));
    assert!(names.contains(&"SubPanel"));
    assert!(!names.contains(&"OutsideHeadingComponent"));
}
