// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use html_forge::visual_qc::{
    apply_agent_structural_patch, apply_deterministic_lint_fix, audit_e2e_flow_completeness,
    compute_layout_sha1, compute_rgba_ssim, elements_from_point_occlusion, registered_qc_rules,
    scan_html_static_rules, DomGeometryNode, FlowAuditEdge, FlowAuditNode, FlowCtaNode,
    PooledCdpEvaluator, QcBoundingBox, QcIssue, QcSeverity, QcTargetKind, QcVerdict,
    VisualQcEvalInput, MAX_AUTOFIX_ATTEMPTS, MAX_CDP_CONCURRENCY, SLIDE_CONTENT_MAX_Y,
};

fn solid_rgba(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    let mut buf = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..(width * height) {
        buf.extend_from_slice(&rgba);
    }
    buf
}

fn sample_node(ff_id: &str, x: f64, y: f64, width: f64, height: f64) -> DomGeometryNode {
    DomGeometryNode {
        ff_id: ff_id.to_string(),
        tag: "div".to_string(),
        role: None,
        bbox: QcBoundingBox {
            x,
            y,
            width,
            height,
        },
        z_index: 1,
        scroll_width: width,
        client_width: width,
        scroll_height: height,
        client_height: height,
        font_size_px: 16.0,
        line_height_ratio: 1.5,
        fg_hex: Some("#111827".into()),
        bg_hex: Some("#FFFFFF".into()),
        border_radius_px: None,
        parent_radius_px: None,
        parent_padding_px: None,
        is_interactive: false,
        is_visible: true,
        pointer_events_none: false,
        text_len: 24,
        has_aria_label: true,
        has_focus_ring: true,
    }
}

#[test]
fn ut_07_1_01_rule_catalog_has_25_plus_rules_and_layout_sha1_is_composite_deterministic() {
    assert!(registered_qc_rules().len() >= 25);
    assert_eq!(MAX_CDP_CONCURRENCY, 1);
    assert_eq!(MAX_AUTOFIX_ATTEMPTS, 3);

    let sha_a = compute_layout_sha1(
        "<main>Hello</main>",
        r#"{"radius":"balanced"}"#,
        &[1440, 390, 768],
        "fixture-v1",
    );
    let sha_b = compute_layout_sha1(
        "<main>Hello</main>",
        r#"{"radius":"balanced"}"#,
        &[390, 768, 1440],
        "fixture-v1",
    );
    let sha_c = compute_layout_sha1(
        "<main>Hello</main>",
        r#"{"radius":"sharp"}"#,
        &[1440, 390, 768],
        "fixture-v1",
    );
    assert_eq!(sha_a.len(), 40);
    assert_eq!(sha_a, sha_b, "viewport order must not alter cache key");
    assert_ne!(
        sha_a, sha_c,
        "changing PROJECT_CONSTRAINTS must invalidate cache key"
    );
}

#[test]
fn ut_07_1_02_pooled_cdp_enforces_max_1_concurrency_and_recovers_after_crash() {
    let evaluator = Arc::new(PooledCdpEvaluator::with_breaker_config(
        Some("/mock/chromium".to_string()),
        2,
        Duration::from_millis(25),
    ));

    let mut handles = Vec::new();
    for i in 0..4 {
        let ev = Arc::clone(&evaluator);
        handles.push(thread::spawn(move || {
            let permit = ev.acquire_permit();
            assert!(
                ev.active_workers() <= 1,
                "Pooled CDP must strictly enforce <= 1 concurrent Chromium worker"
            );
            assert!(permit
                .incognito_context_id
                .starts_with("incognito-cdp-ctx-"));
            thread::sleep(Duration::from_millis(5));
            i
        }));
    }
    for h in handles {
        h.join().expect("thread join");
    }
    assert_eq!(evaluator.active_workers(), 0);
    assert!(evaluator.peak_concurrency() <= 1);

    // Circuit breaker crash/OOM recovery
    assert!(evaluator.can_attempt_cdp());
    let st1 = evaluator.record_worker_crash("CDP OOM killed >250MB");
    assert!(!st1.is_open);
    let st2 = evaluator.record_worker_crash("CDP target crashed");
    assert!(st2.is_open);
    assert!(!evaluator.can_attempt_cdp());

    // During open breaker cooldown, evaluate falls back honestly to UNVERIFIED_NO_BROWSER
    let fallback_res = evaluator.evaluate(&VisualQcEvalInput {
        html: "<main><p>Healthy copy</p></main>".into(),
        ..VisualQcEvalInput::default()
    });
    assert_eq!(fallback_res.verdict, QcVerdict::UnverifiedNoBrowser);
    assert!(fallback_res.used_fallback);
    assert_eq!(
        fallback_res.fallback_mode.as_deref(),
        Some("iframe_bridge_fallback")
    );

    // After cooldown expires, worker auto-recovers cleanly without hanging daemon
    thread::sleep(Duration::from_millis(35));
    assert!(evaluator.can_attempt_cdp());
    let status = evaluator.breaker_status();
    assert!(!status.is_open);
    assert!(status.total_restarts >= 1);
}

#[test]
fn ut_07_1_03_no_browser_never_fakes_pass_verdict() {
    let evaluator = PooledCdpEvaluator::new(None);
    let res = evaluator.evaluate(&VisualQcEvalInput {
        target_kind: QcTargetKind::AppScreen,
        target_id: "screen-home".into(),
        html: "<main><h1>Clean page</h1></main>".into(),
        viewports: vec![1440, 1024, 768, 390],
        ..VisualQcEvalInput::default()
    });
    assert_eq!(res.verdict, QcVerdict::UnverifiedNoBrowser);
    assert!(res.used_fallback);
    assert_eq!(res.fallback_mode.as_deref(), Some("iframe_bridge_fallback"));
    assert!(res
        .issues
        .iter()
        .any(|i| i.rule_id == "R26_UNVERIFIED_NO_BROWSER_CDP"));
}

#[test]
fn ut_07_1_04_real_rgba_ssim_and_elements_from_point_occlusion() {
    let a = solid_rgba(16, 16, [240, 240, 240, 255]);
    let b = solid_rgba(16, 16, [240, 240, 240, 255]);
    let same = compute_rgba_ssim(&a, &b, 16, 16).expect("identical ssim");
    assert!((same - 1.0).abs() < 1e-6);

    let mut occluded = a.clone();
    // Paint dark occluding rectangle across 75% of pixels
    for y in 0..16 {
        for x in 0..12 {
            let idx = ((y * 16 + x) * 4) as usize;
            occluded[idx] = 10;
            occluded[idx + 1] = 10;
            occluded[idx + 2] = 10;
        }
    }
    let degraded = compute_rgba_ssim(&a, &occluded, 16, 16).expect("degraded ssim");
    assert!(
        degraded < 0.70,
        "occluded image SSIM must drop sharply, got {degraded}"
    );
    assert!(compute_rgba_ssim(&a[..10], &b, 16, 16).is_err());

    // elementsFromPoint occlusion check
    let mut cta = sample_node("ff-primary-cta", 100.0, 200.0, 160.0, 48.0);
    cta.is_interactive = true;
    cta.z_index = 1;
    let mut blocker = sample_node("ff-sticky-banner", 80.0, 180.0, 300.0, 100.0);
    blocker.z_index = 50;

    let occ_issues = elements_from_point_occlusion(&[cta, blocker], 1440);
    assert_eq!(occ_issues.len(), 1);
    assert_eq!(occ_issues[0].rule_id, "R05_ELEMENTS_FROM_POINT_OCCLUSION");
    assert_eq!(occ_issues[0].severity, QcSeverity::P0);
}

#[test]
fn ut_07_1_05_slide_16_9_content_max_y_overlap_overflow_and_concentric_radii() {
    let evaluator = PooledCdpEvaluator::new(Some("/mock/chromium".into()));

    let mut body_block = sample_node("ff-slide-body", 80.0, 720.0, 1400.0, 310.0);
    body_block.scroll_height = 420.0;
    body_block.client_height = 310.0;
    body_block.border_radius_px = Some(16.0);
    body_block.parent_radius_px = Some(16.0);
    body_block.parent_padding_px = Some(12.0); // expected inner radius = 4px -> drift 12px

    let mut overlapping = sample_node("ff-slide-card-2", 1200.0, 780.0, 600.0, 160.0);
    overlapping.fg_hex = Some("#CCCCCC".into());
    overlapping.bg_hex = Some("#FFFFFF".into());

    assert!(body_block.bbox.bottom() > SLIDE_CONTENT_MAX_Y);

    let res = evaluator.evaluate(&VisualQcEvalInput {
        target_kind: QcTargetKind::PresentationSlide,
        target_id: "slide-03".into(),
        html: "<section><iframe src=\"https://evil.example/embed\"></iframe></section>".into(),
        geometry_nodes: vec![body_block, overlapping],
        ..VisualQcEvalInput::default()
    });

    assert_eq!(res.verdict, QcVerdict::Fail);
    assert_eq!(res.viewports, vec![1920]);
    let rules: Vec<&str> = res.issues.iter().map(|i| i.rule_id.as_str()).collect();
    assert!(rules.contains(&"R04_SLIDE_FOOTER_RAIL_CONTENT_MAX_Y"));
    assert!(rules.contains(&"R02_VERTICAL_OVERFLOW"));
    assert!(rules.contains(&"R01_RTREE_ELEMENT_OVERLAP"));
    assert!(rules.contains(&"R09_CONCENTRIC_RADII_MISMATCH"));
    assert!(rules.contains(&"R07_WCAG_AA_TEXT_CONTRAST"));
    assert!(rules.contains(&"R11_CROSS_ORIGIN_IFRAME_SANDBOX"));
}

#[test]
fn ut_07_1_06_e2e_flow_completeness_auditor_catches_dead_ends_orphans_and_substates() {
    let nodes = vec![
        FlowAuditNode {
            id: "screen-entry".into(),
            label: "Landing".into(),
            is_entry: true,
            sub_states: vec![
                "default".into(),
                "loading".into(),
                "empty".into(),
                "error".into(),
            ],
            ctas: vec![
                FlowCtaNode {
                    cta_id: "cta-start".into(),
                    label: "Start".into(),
                    target_screen_id: Some("screen-dashboard".into()),
                },
                FlowCtaNode {
                    cta_id: "cta-dead".into(),
                    label: "Broken Link".into(),
                    target_screen_id: None,
                },
            ],
        },
        FlowAuditNode {
            id: "screen-dashboard".into(),
            label: "Dashboard".into(),
            is_entry: false,
            sub_states: vec!["default".into()], // missing loading, empty, error
            ctas: vec![],
        },
        FlowAuditNode {
            id: "screen-orphan".into(),
            label: "Orphan Settings".into(),
            is_entry: false,
            sub_states: vec![
                "default".into(),
                "loading".into(),
                "empty".into(),
                "error".into(),
            ],
            ctas: vec![],
        },
    ];
    let edges = vec![FlowAuditEdge {
        source: "screen-entry".into(),
        target: "screen-dashboard".into(),
        trigger_cta_id: Some("cta-start".into()),
    }];

    let report = audit_e2e_flow_completeness(&nodes, &edges);
    assert!(!report.is_complete);
    assert_eq!(report.dead_end_ctas, vec!["screen-entry:cta-dead"]);
    assert_eq!(report.orphan_screens, vec!["screen-orphan"]);
    assert_eq!(
        report.missing_substates.get("screen-dashboard"),
        Some(&vec![
            "loading".to_string(),
            "empty".to_string(),
            "error".to_string()
        ])
    );
}

#[test]
fn ut_07_1_07_deterministic_lint_fix_runs_under_10ms_with_zero_tokens_and_snapshot() {
    let dirty_html = r#"<main><iframe src="https://x.example"></iframe><p style="color: #cccccc; font-size: 10px; margin: -12px;">Tiny low-contrast</p><button style="outline: none; height: 18px;">Go</button><img src="/hero.png" /></main>"#;
    let evaluator = PooledCdpEvaluator::new(Some("/mock/chromium".into()));
    let clean_geom = vec![sample_node("ff-main", 0.0, 0.0, 1200.0, 600.0)];
    let eval = evaluator.evaluate(&VisualQcEvalInput {
        target_kind: QcTargetKind::AppScreen,
        target_id: "screen-dirty".into(),
        html: dirty_html.into(),
        viewports: vec![1440],
        geometry_nodes: clean_geom.clone(),
        ..VisualQcEvalInput::default()
    });
    assert!(!eval.issues.is_empty());

    let fix = apply_deterministic_lint_fix(dirty_html, &eval.issues);
    assert_eq!(fix.llm_tokens_used, 0);
    assert!(
        fix.elapsed_ms < 10.0,
        "deterministic lint_fix must execute in <10ms, took {:.3}ms",
        fix.elapsed_ms
    );
    assert_eq!(fix.previous_html, dirty_html);
    assert!(fix.patched_html.contains("sandbox=\"allow-scripts\""));
    assert!(fix.patched_html.contains("font-size: 14px"));
    assert!(fix.patched_html.contains("margin: 0px"));
    assert!(fix.patched_html.contains("alt=\"\""));
    assert!(!fix
        .patched_html
        .to_ascii_lowercase()
        .contains("outline: none"));
    assert!(!fix.patched_html.to_ascii_lowercase().contains("#cccccc"));
    assert!(fix.fixed_issue_ids.len() >= 5);

    // Re-evaluating the fixed HTML with browser geometry clears all deterministic violations
    let re_eval = evaluator.evaluate(&VisualQcEvalInput {
        target_kind: QcTargetKind::AppScreen,
        target_id: "screen-dirty".into(),
        html: fix.patched_html,
        viewports: vec![1440],
        geometry_nodes: clean_geom,
        ..VisualQcEvalInput::default()
    });
    assert_eq!(re_eval.verdict, QcVerdict::Pass);
    assert_eq!(re_eval.p0_count, 0);
    assert_eq!(re_eval.p1_count, 0);
}

#[test]
fn ut_07_1_08_defect_2_fails_closed_without_browser_geometry_or_rgba_frames() {
    let evaluator = PooledCdpEvaluator::new(Some("/mock/chromium".into()));

    // Empty geometry_nodes AND no RGBA frames -> MUST fail closed to UNVERIFIED_NO_BROWSER
    let unverified = evaluator.evaluate(&VisualQcEvalInput {
        target_kind: QcTargetKind::AppScreen,
        target_id: "screen-no-frames".into(),
        html: "<main><h1>Clean page</h1></main>".into(),
        viewports: vec![1440],
        ..VisualQcEvalInput::default()
    });
    assert_eq!(unverified.verdict, QcVerdict::UnverifiedNoBrowser);
    assert!(unverified.used_fallback);
    assert_eq!(
        unverified.fallback_mode.as_deref(),
        Some("iframe_bridge_fallback")
    );
    assert_eq!(unverified.ssim_score, None);

    // Supplying valid RGBA frames alongside browser binary -> verified PASS
    let rgba = solid_rgba(8, 8, [220, 220, 220, 255]);
    let verified_rgba = evaluator.evaluate(&VisualQcEvalInput {
        target_kind: QcTargetKind::AppScreen,
        target_id: "screen-rgba".into(),
        html: "<main><h1>Clean page</h1></main>".into(),
        viewports: vec![1440],
        reference_rgba: Some(rgba.clone()),
        candidate_rgba: Some(rgba),
        frame_width: 8,
        frame_height: 8,
        ..VisualQcEvalInput::default()
    });
    assert_eq!(verified_rgba.verdict, QcVerdict::Pass);
    assert!(!verified_rgba.used_fallback);
    assert!(verified_rgba.ssim_score.unwrap_or(0.0) > 0.99);
}

#[test]
fn ut_07_1_09_defect_3_lol_html_iframe_sandbox_scanner_and_fixer() {
    let cases = [
        // Query param spoof (?sandbox=allow-scripts) without real sandbox attr
        r#"<iframe src="https://evil.example/?sandbox=allow-scripts"></iframe>"#,
        // Quoted '>' inside title + self-closing tag
        r#"<iframe src="https://evil.example" title="a > b" />"#,
        // Dangerous extra tokens
        r#"<iframe src="https://evil.example" sandbox="allow-scripts allow-same-origin"></iframe>"#,
        r#"<iframe src="https://evil.example" sandbox="allow-top-navigation"></iframe>"#,
        r#"<iframe src="https://evil.example" sandbox="allow-popups"></iframe>"#,
        r#"<iframe src="https://evil.example" sandbox=""></iframe>"#,
    ];

    for raw in cases {
        let issues = scan_html_static_rules(raw, QcTargetKind::AppScreen, 1440);
        assert!(
            issues
                .iter()
                .any(|i| i.rule_id == "R11_CROSS_ORIGIN_IFRAME_SANDBOX"),
            "Expected R11 violation for: {raw}"
        );

        let fix = apply_deterministic_lint_fix(raw, &issues);
        assert!(!fix.fixed_issue_ids.is_empty(), "Expected fix for: {raw}");
        assert!(
            fix.patched_html.contains(r#"sandbox="allow-scripts""#),
            "Expected sandbox=\"allow-scripts\" in: {}",
            fix.patched_html
        );
        let after = scan_html_static_rules(&fix.patched_html, QcTargetKind::AppScreen, 1440);
        assert!(
            !after
                .iter()
                .any(|i| i.rule_id == "R11_CROSS_ORIGIN_IFRAME_SANDBOX"),
            "R11 should clear after fix for: {} -> {}",
            raw,
            fix.patched_html
        );
    }

    // Exact sandbox="allow-scripts" (case-insensitive) must pass
    let safe_html = r#"<iframe src="https://safe.example/?x=1" title="a > b" sandbox="ALLOW-SCRIPTS"></iframe>"#;
    let safe_issues = scan_html_static_rules(safe_html, QcTargetKind::AppScreen, 1440);
    assert!(!safe_issues
        .iter()
        .any(|i| i.rule_id == "R11_CROSS_ORIGIN_IFRAME_SANDBOX"));
}

#[test]
fn ut_07_1_10_defect_6_multi_viewport_dedup_and_r12_r14_r15_fixers() {
    let html = r#"<main><iframe src="https://x.example"></iframe><canvas data-src="https://cdn.example/c.png"></canvas><p data-ff-id="ff-copy" style="line-height: 1.05; color: #cccccc;">Cramped text</p><button></button><button>Keep Label</button><a href="/next"></a></main>"#;

    let mut issues = Vec::new();
    for vp in [1440, 1024, 768, 390] {
        issues.extend(scan_html_static_rules(html, QcTargetKind::AppScreen, vp));
        issues.push(QcIssue {
            id: format!("lh-{vp}-ff-copy"),
            rule_id: "R14_LINE_HEIGHT_CRAMPED".into(),
            category: "wcag".into(),
            severity: QcSeverity::P1,
            message: "Cramped line-height".into(),
            viewport_width: vp,
            ff_id: Some("ff-copy".into()),
            bbox: None,
            deterministic_fixable: true,
            suggested_fix: Some("Set line-height: 1.45".into()),
            target_node_id: None,
        });
        issues.push(QcIssue {
            id: format!("aria-{vp}-btn"),
            rule_id: "R15_MISSING_ACCESSIBLE_NAME".into(),
            category: "wcag".into(),
            severity: QcSeverity::P1,
            message: "Missing accessible name".into(),
            viewport_width: vp,
            ff_id: None,
            bbox: None,
            deterministic_fixable: true,
            suggested_fix: Some("Add aria-label".into()),
            target_node_id: None,
        });
    }

    let total_issues = issues.len();
    let fix = apply_deterministic_lint_fix(html, &issues);
    assert_eq!(
        fix.unfixable_issue_ids,
        Vec::<String>::new(),
        "Multi-viewport duplicates of fixed static rules must not leak to unfixable_issue_ids"
    );
    assert_eq!(fix.fixed_issue_ids.len(), total_issues);
    assert!(fix.patched_html.contains(r#"data-cowok-isolated="true""#));
    assert!(fix.patched_html.contains("line-height: 1.45"));
    assert!(fix
        .patched_html
        .contains(r#"<button aria-label="Action"></button>"#));
    assert!(fix.patched_html.contains("<button>Keep Label</button>"));
    assert!(fix
        .patched_html
        .contains(r#"<a href="/next" aria-label="Action"></a>"#));
}

#[test]
fn ut_07_1_11_defect_9_agent_structural_patch_and_noop_detection() {
    let html = r#"<main><div data-ff-id="ff-panel" style="width: 500px;">Overflowing</div><button data-ff-id="ff-cta">Buy</button></main>"#;
    let issues = vec![
        QcIssue {
            id: "ovf-v-1440-ff-panel".into(),
            rule_id: "R02_VERTICAL_OVERFLOW".into(),
            category: "layout".into(),
            severity: QcSeverity::P0,
            message: "Vertical overflow".into(),
            viewport_width: 1440,
            ff_id: Some("ff-panel".into()),
            bbox: None,
            deterministic_fixable: false,
            suggested_fix: None,
            target_node_id: None,
        },
        QcIssue {
            id: "occ-1440-ff-cta".into(),
            rule_id: "R05_ELEMENTS_FROM_POINT_OCCLUSION".into(),
            category: "occlusion".into(),
            severity: QcSeverity::P0,
            message: "Occluded CTA".into(),
            viewport_width: 1440,
            ff_id: Some("ff-cta".into()),
            bbox: None,
            deterministic_fixable: false,
            suggested_fix: None,
            target_node_id: None,
        },
    ];

    let patched = apply_agent_structural_patch(html, &issues);
    assert_ne!(patched.patched_html, html);
    assert!(patched.patched_html.contains("overflow-y: auto"));
    assert!(patched.patched_html.contains("z-index: 100"));
    assert!(patched
        .patched_html
        .contains(r#"data-qc-patched="R02_VERTICAL_OVERFLOW""#));
    assert_eq!(patched.fixed_issue_ids.len(), 2);
    assert!(patched.unfixable_issue_ids.is_empty());

    // Zero patch ops when target ff_id does not exist in HTML
    let missing_issue = vec![QcIssue {
        id: "ovf-v-1440-ghost".into(),
        rule_id: "R02_VERTICAL_OVERFLOW".into(),
        category: "layout".into(),
        severity: QcSeverity::P0,
        message: "Missing element".into(),
        viewport_width: 1440,
        ff_id: Some("ff-does-not-exist".into()),
        bbox: None,
        deterministic_fixable: false,
        suggested_fix: None,
        target_node_id: None,
    }];
    let noop = apply_agent_structural_patch(html, &missing_issue);
    assert_eq!(noop.patched_html, html);
    assert!(noop.fixed_issue_ids.is_empty());
    assert_eq!(noop.unfixable_issue_ids, vec!["ovf-v-1440-ghost"]);
}
