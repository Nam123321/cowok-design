// @story 05.4
// @implements AC-02, AC-04, AC-05
use std::collections::BTreeMap;

use html_forge::{
    evaluate_oklch_contrast_guard, patch_html_element, reconcile_ff_ids,
    sanitize_html_with_profile, HtmlForgeError, HtmlPatchOperation, SanitizeProfile,
    DEFAULT_RECONCILE_MIN_CONFIDENCE, WCAG_AA_MIN_CONTRAST,
};

#[test]
fn ut_html_forge_01_sanitize_profiles_differentiate_slide_vs_app_preview_and_block_mxss() {
    let app_html = r#"<main data-ff-id="root-1"><button data-ff-id="btn-1" type="button">Save</button><input data-ff-id="in-1" type="text" placeholder="Name"></main>"#;
    let (app_sanitized, app_sha) =
        sanitize_html_with_profile(app_html, SanitizeProfile::AppPreview)
            .expect("AppPreview must allow interactive button/input");
    assert!(app_sanitized.contains("<button"));
    assert!(app_sanitized.contains("<input"));
    assert_eq!(app_sha.len(), 64);

    let slide_err = sanitize_html_with_profile(app_html, SanitizeProfile::Slide)
        .expect_err("Slide profile must block form/input elements");
    assert!(matches!(slide_err, HtmlForgeError::MxssBlocked(_)));

    let xss_payload =
        r#"<div data-ff-id="x-1" onclick="alert(1)"><script>alert(1)</script><p>Hi</p></div>"#;
    assert!(matches!(
        sanitize_html_with_profile(xss_payload, SanitizeProfile::AppPreview),
        Err(HtmlForgeError::MxssBlocked(_))
    ));

    let svg_escape = r#"<svg><foreignObject><body onload="alert(1)"></body></foreignObject></svg>"#;
    assert!(matches!(
        sanitize_html_with_profile(svg_escape, SanitizeProfile::AppPreview),
        Err(HtmlForgeError::MxssBlocked(_))
    ));
}

#[test]
fn ut_html_forge_02_surgical_patch_updates_text_tokens_and_steering_chip_safely() {
    let doc = r#"<!DOCTYPE html><html><body><section data-ff-id="hero-card"><h1 data-ff-id="hero-title">Old Title</h1><p data-ff-id="hero-body">Body copy</p></section></body></html>"#;

    let text_res = patch_html_element(
        doc,
        "hero-title",
        &HtmlPatchOperation::TextEdit {
            text: "New <Safe> Title".to_string(),
        },
        SanitizeProfile::AppPreview,
    )
    .expect("text edit succeeds");
    assert_eq!(text_res.patched_count, 1);
    assert!(text_res.patched_html.contains("New &lt;Safe&gt; Title"));
    assert!(text_res.patched_html.contains("Body copy"));

    let mut vars = BTreeMap::new();
    vars.insert("--accent-primary".to_string(), "#2563EB".to_string());
    let token_res = patch_html_element(
        &text_res.patched_html,
        "hero-card",
        &HtmlPatchOperation::TokenSwap { css_vars: vars },
        SanitizeProfile::AppPreview,
    )
    .expect("token swap succeeds");
    assert!(token_res.patched_html.contains("--accent-primary: #2563EB"));

    let mut chip_styles = BTreeMap::new();
    chip_styles.insert("padding".to_string(), "24px".to_string());
    chip_styles.insert("border-radius".to_string(), "16px".to_string());
    let chip_res = patch_html_element(
        &token_res.patched_html,
        "hero-card",
        &HtmlPatchOperation::SteeringChip {
            chip_id: "more-whitespace".to_string(),
            safe_styles: chip_styles,
        },
        SanitizeProfile::AppPreview,
    )
    .expect("steering chip succeeds");
    assert!(chip_res
        .patched_html
        .contains("data-steering-chip=\"more-whitespace\""));
    assert!(chip_res.patched_html.contains("padding: 24px"));

    let mut bad_styles = BTreeMap::new();
    bad_styles.insert(
        "background-color".to_string(),
        "url(javascript:alert(1))".to_string(),
    );
    assert!(matches!(
        patch_html_element(
            doc,
            "hero-card",
            &HtmlPatchOperation::StylePatch {
                safe_styles: bad_styles
            },
            SanitizeProfile::AppPreview
        ),
        Err(HtmlForgeError::CssTokenBlocked(_))
    ));
}

#[test]
fn ut_html_forge_03_reconcile_ff_ids_preserves_and_remaps_or_fails_closed_stale_mapping() {
    let prev = r#"<main><h1 data-ff-id="hero-h1" class="title-lg">Launch Workspace</h1><button data-ff-id="cta-btn" role="button" aria-label="Create project">Create project</button></main>"#;
    let next_missing_attr = r#"<main><h1 class="title-lg">Launch Workspace</h1><button role="button" aria-label="Create project">Create project</button></main>"#;

    let report = reconcile_ff_ids(prev, next_missing_attr, DEFAULT_RECONCILE_MIN_CONFIDENCE)
        .expect("high-confidence structural match must reconcile");
    assert_eq!(report.preserved_count, 0);
    assert_eq!(report.remapped_count, 2);
    assert!(report.reconciled_html.contains("data-ff-id=\"hero-h1\""));
    assert!(report.reconciled_html.contains("data-ff-id=\"cta-btn\""));

    // Ambiguous identical siblings must NEVER silently bind to a wrong sibling -> 409 STALE_MAPPING
    let prev_sibling = r#"<ul><li data-ff-id="item-alpha" class="row">Alpha</li></ul>"#;
    let next_ambiguous = r#"<ul><li class="row">Beta</li><li class="row">Beta</li></ul>"#;
    let stale_err = reconcile_ff_ids(
        prev_sibling,
        next_ambiguous,
        DEFAULT_RECONCILE_MIN_CONFIDENCE,
    )
    .expect_err("ambiguous or low-confidence rewrite must return StaleMapping");
    assert!(matches!(stale_err, HtmlForgeError::StaleMapping { .. }));
}

#[test]
fn ut_html_forge_04_oklch_contrast_guard_clamps_unreadable_tint_to_wcag_aa() {
    let low_contrast = evaluate_oklch_contrast_guard("#D9E2EC", "#FAF9F6", WCAG_AA_MIN_CONTRAST)
        .expect("oklch evaluation succeeds");
    assert!(low_contrast.clamped);
    assert!(low_contrast.original_ratio < WCAG_AA_MIN_CONTRAST);
    assert!(low_contrast.adjusted_ratio >= WCAG_AA_MIN_CONTRAST);
    assert!(low_contrast.warning.is_some());

    let sketch_ink = evaluate_oklch_contrast_guard("#2E2D2B", "#FAF9F6", WCAG_AA_MIN_CONTRAST)
        .expect("sketch paper ink check succeeds");
    assert!(!sketch_ink.clamped);
    assert!(sketch_ink.original_ratio >= WCAG_AA_MIN_CONTRAST);
    assert!(sketch_ink.warning.is_none());

    // Regression CRITICAL-4: multi-byte UTF-8 hex strings must NEVER panic
    assert!(matches!(
        evaluate_oklch_contrast_guard("#€", "#FFFFFF", WCAG_AA_MIN_CONTRAST),
        Err(HtmlForgeError::InvalidPayload(_))
    ));
    assert!(matches!(
        evaluate_oklch_contrast_guard("#€€", "#FFFFFF", WCAG_AA_MIN_CONTRAST),
        Err(HtmlForgeError::InvalidPayload(_))
    ));
}

#[test]
fn ut_html_forge_05_regression_identical_siblings_table_tbody_proto_keys_and_nan_css() {
    // Regression HIGH-1a: two identical high-scoring sibling buttons MUST fail closed (StaleMapping)
    let prev = r#"<div><button data-ff-id="btn-save" class="btn primary">Save</button></div>"#;
    let next_dup = r#"<div><button class="btn primary">Save</button><button class="btn primary">Save</button></div>"#;
    assert!(matches!(
        reconcile_ff_ids(prev, next_dup, DEFAULT_RECONCILE_MIN_CONFIDENCE),
        Err(HtmlForgeError::StaleMapping { .. })
    ));

    // Regression HIGH-1b: synthetic <tbody> in <table><tr> must not shift data-ff-id onto wrong element
    let prev_tbl = r#"<table><tr><td data-ff-id="cell-rev">$42,000</td></tr></table>"#;
    let next_tbl = r#"<table><tr><td>$42,000</td></tr></table>"#;
    let rep = reconcile_ff_ids(prev_tbl, next_tbl, DEFAULT_RECONCILE_MIN_CONFIDENCE)
        .expect("table reconciliation succeeds without cursor drift");
    assert!(rep
        .reconciled_html
        .contains("<td data-ff-id=\"cell-rev\">$42,000</td>"));

    // Regression MEDIUM-1 (Round 2): <style> rawtext preserves '>' combinator and quoted fonts
    let prev_styled = r#"<!DOCTYPE html><html><head><style>.card > h1 { font-family: 'Inter'; }</style></head><body><section class="card"><h1 data-ff-id="hero-h1">Billing Overview</h1></section></body></html>"#;
    let next_styled = r#"<!DOCTYPE html><html><head><style>.card > h1 { font-family: 'Inter'; }</style></head><body><section class="card"><h1>Billing Overview</h1></section></body></html>"#;
    let styled_rep = reconcile_ff_ids(prev_styled, next_styled, DEFAULT_RECONCILE_MIN_CONFIDENCE)
        .expect("styled reconciliation succeeds");
    assert!(styled_rep
        .reconciled_html
        .contains(".card > h1 { font-family: 'Inter'; }"));
    assert!(!styled_rep.reconciled_html.contains("&gt;"));
    assert!(!styled_rep.reconciled_html.contains("&#39;"));

    // Regression: AppPreview sanitizer accepts benign flex/margin inline styles + var(--tok, Fallback) and blocks unsafe var fallback
    let live_preview_doc = r#"<!DOCTYPE html><html lang="vi"><head><style>body{margin:0;padding:16px}.card{padding:12px}</style></head><body data-ff-substate="DEFAULT"><section class="card" data-ff-id="sec-header"><div style="font-size:10px;font-weight:600;color:var(--human-accent,LinkText);margin-bottom:4px"><span data-ff-id="ff-slide-eyebrow">SCR-02</span></div><h2 data-ff-id="ff-slide-headline" style="margin:0 0 4px;font-size:16px">Xác thực</h2></section><section class="card" data-ff-id="sec-biometric" style="display:flex;align-items:center;justify-content:space-between;gap:12px"><button type="button" data-ff-id="ff-action-cta">Ký</button></section></body></html>"#;
    let (sanitized_live, _sha) =
        sanitize_html_with_profile(live_preview_doc, SanitizeProfile::AppPreview)
            .expect("live preview document must sanitize cleanly");
    assert!(sanitized_live.contains("data-ff-id=\"sec-header\""));
    assert!(matches!(
        html_forge::validate_and_scrub_css_tokens(
            "color: var(--x, url(https://evil.example))",
            true
        ),
        Err(HtmlForgeError::CssTokenBlocked(_))
    ));

    // Regression MEDIUM-5: prototype keys & non-finite CSS lengths rejected fail-closed
    assert!(!html_forge::is_valid_ff_id("__proto__"));
    assert!(!html_forge::is_valid_ff_id("constructor"));
    assert!(!html_forge::is_valid_ff_id("prototype"));
    assert!(matches!(
        html_forge::validate_and_scrub_css_tokens("padding: NaNpx", true),
        Err(HtmlForgeError::CssTokenBlocked(_))
    ));
}
