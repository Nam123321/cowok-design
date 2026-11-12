// @story 03.1
// @cover AC-01, AC-02, AC-03
use constraint_engine::{ActiveConstraint, AstLinter, ConstraintEvaluator};
use domain::{Department, Portal};
use rusqlite::Connection;
use std::time::Instant;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("open in-memory db");
    conn.execute_batch(
        "CREATE TABLE constraint_profiles (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            preset_profile TEXT NOT NULL DEFAULT 'DEFAULT',
            styling_policy TEXT NOT NULL DEFAULT 'CSS_MODULES',
            forbid_tailwind INTEGER NOT NULL DEFAULT 1,
            forbid_raw_hex INTEGER NOT NULL DEFAULT 1,
            tokens_css_path TEXT NOT NULL DEFAULT 'src/styles/tokens.css',
            icon_manifest_path TEXT NOT NULL DEFAULT 'icons/icon-manifest.json',
            allowed_icon_set_json TEXT NOT NULL DEFAULT '[]',
            typography_contract_json TEXT NOT NULL DEFAULT '{}',
            min_wcag_contrast REAL NOT NULL DEFAULT 4.5,
            revision INTEGER NOT NULL DEFAULT 1,
            updated_at BIGINT NOT NULL
        );
        CREATE TABLE constraint_rules (
            id TEXT PRIMARY KEY NOT NULL,
            profile_id TEXT NOT NULL REFERENCES constraint_profiles(id) ON DELETE CASCADE,
            rule_code TEXT NOT NULL,
            severity TEXT NOT NULL,
            config_json TEXT NOT NULL DEFAULT '{}',
            enabled INTEGER NOT NULL DEFAULT 1,
            updated_at BIGINT NOT NULL
        );",
    )
    .expect("create schema");

    // Insert Project A (Strict Fintech: forbid_tailwind=1, forbid_raw_hex=0)
    conn.execute(
        "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, forbid_tailwind, forbid_raw_hex, updated_at)
         VALUES ('prof-a', 'proj-alpha', 'FINTECH', 'CSS_MODULES', 1, 0, 100)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
         VALUES ('r-1', 'prof-a', 'BAN_TAILWIND', 'BLOCKER', '{}', 1, 100)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
         VALUES ('r-2', 'prof-a', 'FORBID_MARKETING', 'BLOCKER', '{}', 1, 100)",
        [],
    )
    .unwrap();
    // Add disabled rule to prof-a; should be ignored by loader
    conn.execute(
        "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
         VALUES ('r-disabled', 'prof-a', 'DISABLED_RULE', 'BLOCKER', '{}', 0, 100)",
        [],
    )
    .unwrap();

    // Insert Project B (Game Studio: forbid_tailwind=0, forbid_raw_hex=1)
    conn.execute(
        "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, forbid_tailwind, forbid_raw_hex, updated_at)
         VALUES ('prof-b', 'proj-beta', 'GAME', 'CSS_MODULES', 0, 1, 200)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
         VALUES ('r-3', 'prof-b', 'BAN_INLINE_HEX', 'WARN', '{}', 1, 200)",
        [],
    )
    .unwrap();

    conn
}

#[test]
fn test_multi_project_sqlite_isolation() {
    let conn = setup_test_db();

    // Project Alpha constraints
    let rules_a = ConstraintEvaluator::load_constraints_for_project(&conn, "proj-alpha")
        .expect("load alpha constraints");
    assert_eq!(rules_a.len(), 2);
    assert!(rules_a.iter().any(|r| r.rule_code == "BAN_TAILWIND"));
    assert!(rules_a.iter().any(|r| r.rule_code == "FORBID_MARKETING"));
    assert!(!rules_a.iter().any(|r| r.rule_code == "BAN_INLINE_HEX"));
    assert!(!rules_a.iter().any(|r| r.rule_code == "DISABLED_RULE"));

    // Project Beta constraints
    let rules_b = ConstraintEvaluator::load_constraints_for_project(&conn, "proj-beta")
        .expect("load beta constraints");
    assert_eq!(rules_b.len(), 1);
    assert_eq!(rules_b[0].rule_code, "BAN_INLINE_HEX");
    assert!(!rules_b.iter().any(|r| r.rule_code == "BAN_TAILWIND"));

    // Non-existent project
    let rules_none = ConstraintEvaluator::load_constraints_for_project(&conn, "proj-unknown")
        .expect("load unknown constraints");
    assert!(rules_none.is_empty());
}

#[test]
fn test_project_isolation_evaluation() {
    let conn = setup_test_db();
    let rules_a = ConstraintEvaluator::load_constraints_for_project(&conn, "proj-alpha").unwrap();
    let rules_b = ConstraintEvaluator::load_constraints_for_project(&conn, "proj-beta").unwrap();

    // Project Alpha forbids marketing deck
    let res_a = ConstraintEvaluator::evaluate_slice_constraints(
        &rules_a,
        Department::MarketingDeck,
        Portal::MarketingWeb,
    );
    assert!(res_a.is_err());

    // Project Beta allows marketing deck
    let res_b = ConstraintEvaluator::evaluate_slice_constraints(
        &rules_b,
        Department::MarketingDeck,
        Portal::MarketingWeb,
    );
    assert!(res_b.is_ok());
}

#[test]
fn test_ast_linter_submillisecond_performance() {
    let rules = vec![
        ActiveConstraint {
            rule_code: "BAN_TAILWIND".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        },
        ActiveConstraint {
            rule_code: "BAN_INLINE_HEX".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        },
    ];

    let jsx = r#"
        import React from 'react';
        export const DashboardCard = () => (
            <div className="card-container" style={{ padding: '16px' }}>
                <h1 className="card-title">Analytics</h1>
                <p className="card-desc">Real-time overview</p>
            </div>
        );
    "#;

    // Warmup
    let _ = AstLinter::lint_source(jsx, &rules);

    let start = Instant::now();
    let violations = AstLinter::lint_source(jsx, &rules).expect("linting should succeed");
    let duration = start.elapsed();

    assert!(violations.is_empty());
    // Sub-millisecond execution verification (< 1000 microseconds in release mode)
    let threshold_micros = if cfg!(debug_assertions) { 5000 } else { 1000 };
    assert!(
        duration.as_micros() < threshold_micros,
        "Linting must complete within budget, took {:?}",
        duration
    );
}

#[test]
fn test_ast_linter_detection_and_coordinates() {
    let rules = vec![
        ActiveConstraint {
            rule_code: "BAN_TAILWIND".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        },
        ActiveConstraint {
            rule_code: "BAN_INLINE_HEX".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        },
    ];

    let jsx = "<div>\n  <span className=\"p-4 flex text-sm\">Bad</span>\n  <p style=\"color: #ff5733\">Hardcoded</p>\n</div>";
    let violations = AstLinter::lint_source(jsx, &rules).expect("linting should succeed");

    assert_eq!(violations.len(), 4);
    assert_eq!(violations[0].rule_code, "BAN_TAILWIND");
    assert_eq!(violations[0].line, Some(2));
    assert!(violations[0].message.contains("p-4"));

    assert_eq!(violations[1].rule_code, "BAN_TAILWIND");
    assert_eq!(violations[1].line, Some(2));
    assert!(violations[1].message.contains("flex"));

    assert_eq!(violations[2].rule_code, "BAN_TAILWIND");
    assert_eq!(violations[2].line, Some(2));
    assert!(violations[2].message.contains("text-sm"));

    assert_eq!(violations[3].rule_code, "BAN_INLINE_HEX");
    assert_eq!(violations[3].line, Some(3));
    assert!(violations[3].message.contains("#ff5733"));
}

#[test]
fn test_ast_linter_max_size_cap() {
    let rules = vec![ActiveConstraint {
        rule_code: "BAN_TAILWIND".to_string(),
        severity: "BLOCKER".to_string(),
        config_json: "{}".to_string(),
    }];

    // Construct content exceeding 500KB cap
    let large_content = "a".repeat(AstLinter::MAX_SOURCE_BYTES + 100);
    let res = AstLinter::lint_source(&large_content, &rules);
    assert!(res.is_err());
}

#[test]
fn test_case_insensitive_canonicalization() {
    let rules = vec![
        ActiveConstraint {
            rule_code: "ban_tailwind".to_string(), // lowercase
            severity: "blocker".to_string(),
            config_json: "{}".to_string(),
        },
        ActiveConstraint {
            rule_code: "Ban_Inline_Hex".to_string(), // mixed case
            severity: "warn".to_string(),
            config_json: "{}".to_string(),
        },
    ];

    let jsx =
        "<div>\n  <span className=\"flex\">Flex</span>\n  <p style=\"color: #abc\">Hex</p>\n</div>";
    let violations =
        AstLinter::lint_source(jsx, &rules).expect("case-insensitive linting should succeed");

    assert_eq!(violations.len(), 2);
    assert_eq!(violations[0].rule_code, "BAN_TAILWIND");
    assert_eq!(violations[1].rule_code, "BAN_INLINE_HEX");
}

#[test]
fn test_invalid_json_config() {
    let rules = vec![ActiveConstraint {
        rule_code: "BAN_TAILWIND".to_string(),
        severity: "BLOCKER".to_string(),
        config_json: "{ malformed json".to_string(),
    }];

    let res = AstLinter::lint_source("<div>Normal</div>", &rules);
    assert!(res.is_err());
}

#[test]
fn test_empty_prohibited_tokens() {
    let rules = vec![ActiveConstraint {
        rule_code: "BAN_TAILWIND".to_string(),
        severity: "BLOCKER".to_string(),
        config_json: r#"{"prohibited_tokens": []}"#.to_string(),
    }];

    let res = AstLinter::lint_source("<div>Normal</div>", &rules);
    assert!(res.is_err());
}

#[test]
fn test_crlf_and_unicode_scalar_coordinates() {
    let rules = vec![ActiveConstraint {
        rule_code: "BAN_TAILWIND".to_string(),
        severity: "BLOCKER".to_string(),
        config_json: "{}".to_string(),
    }];

    // Line 1: Emoji (multibyte UTF-8) before attribute on CRLF line break
    let content = "const rocket = '🚀';\r\n<div className=\"flex\">Rocket</div>";
    let violations = AstLinter::lint_source(content, &rules).expect("linting CRLF with emojis");

    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].line, Some(2));
    assert_eq!(violations[0].column, Some(17));
}

#[test]
fn test_p2002_p2003_sqlite_constraints() {
    let conn = Connection::open_in_memory().expect("open in-memory db");
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
        CREATE TABLE constraint_profiles (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            preset_profile TEXT NOT NULL DEFAULT 'DEFAULT',
            styling_policy TEXT NOT NULL DEFAULT 'CSS_MODULES',
            forbid_tailwind INTEGER NOT NULL DEFAULT 1,
            forbid_raw_hex INTEGER NOT NULL DEFAULT 1,
            tokens_css_path TEXT NOT NULL DEFAULT 'src/styles/tokens.css',
            icon_manifest_path TEXT NOT NULL DEFAULT 'icons/icon-manifest.json',
            allowed_icon_set_json TEXT NOT NULL DEFAULT '[]',
            typography_contract_json TEXT NOT NULL DEFAULT '{}',
            min_wcag_contrast REAL NOT NULL DEFAULT 4.5,
            revision INTEGER NOT NULL DEFAULT 1,
            updated_at BIGINT NOT NULL,
            UNIQUE(project_id, preset_profile)
        );
        CREATE TABLE constraint_rules (
            id TEXT PRIMARY KEY NOT NULL,
            profile_id TEXT NOT NULL REFERENCES constraint_profiles(id) ON DELETE CASCADE,
            rule_code TEXT NOT NULL,
            severity TEXT NOT NULL,
            config_json TEXT NOT NULL DEFAULT '{}',
            enabled INTEGER NOT NULL DEFAULT 1,
            updated_at BIGINT NOT NULL,
            UNIQUE(profile_id, rule_code)
        );",
    )
    .expect("create schema");

    // Insert valid default profile
    conn.execute(
        "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, forbid_tailwind, forbid_raw_hex, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, min_wcag_contrast, revision, updated_at)
         VALUES ('p1', 'proj-1', 'DEFAULT', 'CSS_MODULES', 1, 1, 'src/styles/tokens.css', 'icons/icon-manifest.json', '[]', '{}', 4.5, 1, 100)",
        [],
    )
    .unwrap();

    // Multiple preset profiles on the same project are allowed (e.g. MARKETING preset)
    let ok_diff_preset = conn.execute(
        "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, forbid_tailwind, forbid_raw_hex, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, min_wcag_contrast, revision, updated_at)
         VALUES ('p2', 'proj-1', 'MARKETING', 'CSS_MODULES', 1, 1, 'src/styles/tokens.css', 'icons/icon-manifest.json', '[]', '{}', 4.5, 1, 200)",
        [],
    );
    assert!(ok_diff_preset.is_ok());

    // Duplicate (project_id, preset_profile) (P2002 unique collision)
    let err_p2002 = conn.execute(
        "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, forbid_tailwind, forbid_raw_hex, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, min_wcag_contrast, revision, updated_at)
         VALUES ('p3', 'proj-1', 'DEFAULT', 'CSS_MODULES', 1, 1, 'src/styles/tokens.css', 'icons/icon-manifest.json', '[]', '{}', 4.5, 1, 300)",
        [],
    );
    assert!(err_p2002.is_err());

    // Foreign key violation (P2003 reference missing profile_id)
    let err_p2003 = conn.execute(
        "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
         VALUES ('r1', 'non-existent-profile', 'BAN_TAILWIND', 'BLOCKER', '{}', 1, 100)",
        [],
    );
    assert!(err_p2003.is_err());
}

#[test]
fn test_malformed_html_fragment() {
    let rules = vec![ActiveConstraint {
        rule_code: "BAN_TAILWIND".to_string(),
        severity: "BLOCKER".to_string(),
        config_json: "{}".to_string(),
    }];

    let malformed_html = "<div class=\"flex\" <span style=\"color: #fff\">Bad Nesting</div>";
    let res = AstLinter::lint_source(malformed_html, &rules);
    assert!(res.is_err());
}
