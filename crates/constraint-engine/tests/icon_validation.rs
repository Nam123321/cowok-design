// @story 03.2
// @cover AC-01, AC-02, AC-03
use constraint_engine::{IconValidator, TypographyContract};

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
    let path = std::path::Path::new(
        "Brand-guideline/cowok-brand-product-system-v4.3/iconography/icon-manifest.json",
    );
    if path.exists() {
        let loaded = IconValidator::load_from_file(path).expect("Should load manifest from file");
        assert_eq!(loaded.count, 143);
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
}
