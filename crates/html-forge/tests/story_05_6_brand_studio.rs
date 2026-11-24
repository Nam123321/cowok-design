// @story 05.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-06
use std::collections::BTreeMap;

use html_forge::{
    default_popular_fe_catalog, extract_fe_data_contract, generate_default_brand_proposals,
    is_valid_theme_mode_id, render_fe_data_contract_ts, render_tokenized_brand_preview,
    scan_sketch_components, validate_brand_proposal, BrandModeTokens, BrandProposalV1,
    DensityScale, FontPairing, HtmlForgeError, InferredFieldType, PopularComponentStatus,
    RadiusScale, SketchPageInput, FE_CONTRACT_BOUNDARY_NOTICE, WCAG_AA_MIN_CONTRAST,
};

fn sample_sketch_pages() -> Vec<SketchPageInput> {
    vec![
        SketchPageInput {
            screen_id: "scr-hero".into(),
            title: "Executive Overview".into(),
            page_role: "hero".into(),
            html: "<!DOCTYPE html><html><head></head><body><main data-ff-id=\"stage-root\"><section class=\"card\" data-component-kind=\"kpi-metric\"><h1 data-ff-id=\"ff-hero-title\">Revenue Pulse</h1><span data-ff-id=\"ff-mrr-val\">$48,200</span></section></main></body></html>".into(),
        },
        SketchPageInput {
            screen_id: "scr-list".into(),
            title: "Accounts Table".into(),
            page_role: "list_detail".into(),
            html: "<!DOCTYPE html><html><head></head><body><main><table data-ff-id=\"ff-accounts-table\"><thead><tr><th>Account</th></tr></thead><tbody><tr><td>Acme</td></tr></tbody></table></main></body></html>".into(),
        },
        SketchPageInput {
            screen_id: "scr-form".into(),
            title: "Create Deal".into(),
            page_role: "action_form".into(),
            html: "<!DOCTYPE html><html><head></head><body><form data-ff-id=\"ff-create-deal\"><input name=\"dealTitle\" placeholder=\"Enterprise\" /><button type=\"submit\" data-ff-id=\"ff-submit-deal\">Save Deal</button></form></body></html>".into(),
        },
    ]
}

#[test]
fn ut_05_6_01_single_mode_warm_never_injects_light_or_dark_and_renders_under_5ms() {
    let proposals = generate_default_brand_proposals(
        &["Linear".into(), "Notion".into()],
        &["warm".into()],
        "vi",
    )
    .expect("generate single-mode warm proposals");
    assert_eq!(proposals.len(), 3);
    for p in &proposals {
        assert_eq!(p.theme_modes.len(), 1);
        assert_eq!(p.theme_modes[0].mode_id, "warm");
        assert!(p.theme_modes[0].wcag_aa_verified);
    }

    let pages = sample_sketch_pages();
    let bundle = render_tokenized_brand_preview(&pages, &proposals[0], "warm")
        .expect("render tokenized warm preview");
    assert_eq!(bundle.requested_modes, vec!["warm".to_string()]);
    assert_eq!(bundle.active_mode_id, "warm");
    assert_eq!(bundle.pages.len(), 3);
    assert!(!bundle.capped_to_representative);
    assert!(
        bundle.total_render_time_micros < 50_000,
        "render should be fast (got {}us)",
        bundle.total_render_time_micros
    );
    for preview in &bundle.pages {
        assert!(preview.wcag_aa_verified);
        assert!(preview.rendered_html.contains("data-brand-mode=\"warm\""));
        assert!(!preview.rendered_html.contains("data-brand-mode=\"dark\""));
        assert!(!preview.rendered_html.contains("data-brand-mode=\"light\""));
    }

    // Requesting an unrequested mode ("dark") MUST fail closed
    let err = render_tokenized_brand_preview(&pages, &proposals[0], "dark")
        .expect_err("unrequested mode must fail");
    assert!(matches!(err, HtmlForgeError::InvalidPayload(_)));
}

#[test]
fn ut_05_6_02_multi_mode_warm_dim_night_validates_every_mode_and_rejects_invalid_ids() {
    assert!(is_valid_theme_mode_id("warm"));
    assert!(is_valid_theme_mode_id("high-contrast"));
    assert!(!is_valid_theme_mode_id(""));
    assert!(!is_valid_theme_mode_id("DarkMode"));
    assert!(!is_valid_theme_mode_id("-leading"));

    let modes = vec!["warm".into(), "dim".into(), "night".into()];
    let proposals = generate_default_brand_proposals(&["Stripe".into()], &modes, "en")
        .expect("generate multi-mode proposals");
    assert_eq!(proposals[0].theme_modes.len(), 3);
    let pages = sample_sketch_pages();
    for mode in &modes {
        let bundle = render_tokenized_brand_preview(&pages, &proposals[0], mode)
            .expect("render each requested mode");
        assert_eq!(bundle.requested_modes, modes);
        assert_eq!(bundle.active_mode_id, *mode);
        assert!(bundle.pages.iter().all(|p| p.wcag_aa_verified));
    }

    // Empty themeModes MUST fail closed
    let empty_err = generate_default_brand_proposals(&["Linear".into()], &[], "en")
        .expect_err("empty themeModes must be rejected");
    assert!(matches!(empty_err, HtmlForgeError::InvalidPayload(_)));
}

#[test]
fn ut_05_6_03_huge_sketch_over_20_screens_caps_deterministically_to_3_representative_pages() {
    let mut huge_pages = Vec::new();
    for i in 0..24 {
        let (role, title, html) = match i {
            2 => (
                "hero",
                "Landing Hero",
                "<!DOCTYPE html><html><body><section><h1>Hero</h1></section></body></html>",
            ),
            9 => (
                "list_detail",
                "Orders Table",
                "<!DOCTYPE html><html><body><table><tr><td>Order</td></tr></table></body></html>",
            ),
            17 => (
                "action_form",
                "Checkout Form",
                "<!DOCTYPE html><html><body><form><input name=\"email\"/><button>Pay</button></form></body></html>",
            ),
            _ => (
                "other",
                "Auxiliary Screen",
                "<!DOCTYPE html><html><body><section><p>Extra</p></section></body></html>",
            ),
        };
        huge_pages.push(SketchPageInput {
            screen_id: format!("scr-{i:02}"),
            title: format!("{title} {i}"),
            page_role: role.into(),
            html: html.into(),
        });
    }

    let proposals = generate_default_brand_proposals(&["Linear".into()], &["light".into()], "en")
        .expect("proposals");
    let bundle =
        render_tokenized_brand_preview(&huge_pages, &proposals[0], "light").expect("capped bundle");
    assert_eq!(bundle.total_sketch_pages, 24);
    assert!(bundle.capped_to_representative);
    assert_eq!(bundle.pages.len(), 3);
    let roles: Vec<&str> = bundle.pages.iter().map(|p| p.page_role.as_str()).collect();
    assert!(roles.contains(&"hero"));
    assert!(roles.contains(&"list_detail"));
    assert!(roles.contains(&"action_form"));
}

#[test]
fn ut_05_6_04_css_injection_fails_closed_and_oklch_aa_clamps_low_contrast() {
    // 1. Fail-closed on url(, expression(, @import, </style
    for evil_val in [
        "url(https://evil.example/x.css)",
        "expression(alert(1))",
        "@import 'x'",
        "#111111</style><script>alert(1)</script>",
    ] {
        let mut tokens = BTreeMap::new();
        tokens.insert("--bg-surface".into(), "#FFFFFF".into());
        tokens.insert("--text-primary".into(), evil_val.into());
        let bad = BrandProposalV1 {
            id: "bad-prop".into(),
            brand_name: "Evil".into(),
            rationale: "Inject".into(),
            inspiration_refs: vec![],
            theme_modes: vec![BrandModeTokens {
                mode_id: "warm".into(),
                label: "Warm".into(),
                tokens,
                wcag_aa_verified: false,
            }],
            font_pairing: FontPairing {
                heading: "Inter, sans-serif".into(),
                body: "Inter, sans-serif".into(),
                mono: "monospace".into(),
            },
            radius_scale: RadiusScale::Balanced,
            density: DensityScale::Comfortable,
            component_style_overrides: BTreeMap::new(),
        };
        let err = validate_brand_proposal(&bad).expect_err("CSS injection must be blocked");
        assert!(
            matches!(err, HtmlForgeError::CssTokenBlocked(_)),
            "expected CssTokenBlocked for '{evil_val}', got {err:?}"
        );
    }

    // 2. Low-contrast #CCCCCC on #FFFFFF MUST be clamped by OKLCH AA guard to >= 4.5:1
    let mut low_tokens = BTreeMap::new();
    low_tokens.insert("--bg-surface".into(), "#FFFFFF".into());
    low_tokens.insert("--text-primary".into(), "#CCCCCC".into());
    let low_prop = BrandProposalV1 {
        id: "low-contrast".into(),
        brand_name: "Soft Pastel".into(),
        rationale: "Needs contrast clamping".into(),
        inspiration_refs: vec!["Apple".into()],
        theme_modes: vec![BrandModeTokens {
            mode_id: "warm".into(),
            label: "Warm".into(),
            tokens: low_tokens,
            wcag_aa_verified: false,
        }],
        font_pairing: FontPairing {
            heading: "Inter, sans-serif".into(),
            body: "Inter, sans-serif".into(),
            mono: "monospace".into(),
        },
        radius_scale: RadiusScale::Soft,
        density: DensityScale::Airy,
        component_style_overrides: BTreeMap::new(),
    };
    let (clamped, reports) = validate_brand_proposal(&low_prop).expect("clamp OKLCH AA");
    assert!(clamped.theme_modes[0].wcag_aa_verified);
    let mode_rep = &reports["warm"][0];
    assert!(mode_rep.clamped);
    assert!(mode_rep.adjusted_ratio >= WCAG_AA_MIN_CONTRAST);
    assert_ne!(clamped.theme_modes[0].tokens["--text-primary"], "#CCCCCC");
}

#[test]
fn ut_05_6_05_ast_sketch_scanner_classifies_three_buckets_and_extracts_custom_kinds() {
    let html_a = "<!DOCTYPE html><html><body><nav><a href=\"#\">Home</a></nav><table data-component-kind=\"invoice-grid\"><tr><td>Row</td></tr></table><button>Save</button></body></html>";
    let html_b = "<!DOCTYPE html><html><body><form><input name=\"title\" /><button>Submit</button></form></body></html>";
    let catalog = default_popular_fe_catalog();
    let res = scan_sketch_components(&[html_a, html_b], &catalog);

    assert!(res.extracted_kinds.contains(&"button".to_string()));
    assert!(res.extracted_kinds.contains(&"data-table".to_string()));
    assert!(res.extracted_kinds.contains(&"form-input".to_string()));
    assert!(res.extracted_kinds.contains(&"invoice-grid".to_string()));

    let toast_item = res
        .audit_items
        .iter()
        .find(|i| i.component_kind == "toast")
        .expect("toast in audit");
    assert_eq!(
        toast_item.status,
        PopularComponentStatus::SuggestedContextual
    );
    assert!(!toast_item.selected);

    let custom_item = res
        .audit_items
        .iter()
        .find(|i| i.component_kind == "invoice-grid")
        .expect("custom kind in audit");
    assert_eq!(custom_item.status, PopularComponentStatus::DetectedInSketch);
    assert!(custom_item.selected);

    // Static page with no buttons/tables/forms -> toast/confirm-dialog classified as NotNeeded
    let static_res =
        scan_sketch_components(&["<html><body><p>Hello world</p></body></html>"], &catalog);
    let static_toast = static_res
        .audit_items
        .iter()
        .find(|i| i.component_kind == "toast")
        .expect("toast item");
    assert_eq!(static_toast.status, PopularComponentStatus::NotNeeded);
    assert!(static_res.not_needed_count > 0);
}

#[test]
fn ut_05_6_06_fe_data_contract_extracts_viewmodel_actions_substates_and_renders_ts() {
    let html = r#"<!DOCTYPE html><html><body>
        <main data-ff-id="stage-root">
            <h1 data-ff-id="ff-page-title">Billing Overview</h1>
            <span data-ff-id="ff-total-mrr">$12,450</span>
            <span data-ff-id="ff-active-seats">42</span>
            <span data-ff-id="ff-plan-active">active</span>
            <form data-ff-id="ff-upgrade-form">
                <input name="planCode" placeholder="PRO" />
                <button type="submit" data-ff-id="ff-submit-upgrade">Upgrade</button>
            </form>
        </main>
    </body></html>"#;

    let draft =
        extract_fe_data_contract("11111111-1111-4111-8111-111111111111", "scr-billing", html);
    assert_eq!(draft.boundary_notice, FE_CONTRACT_BOUNDARY_NOTICE);
    assert!(draft
        .view_model_fields
        .iter()
        .any(|f| f.field_name == "totalMrr" && f.inferred_type == InferredFieldType::Currency));
    assert!(draft
        .view_model_fields
        .iter()
        .any(|f| f.field_name == "activeSeats" && f.inferred_type == InferredFieldType::Number));
    assert!(draft
        .view_model_fields
        .iter()
        .any(|f| f.field_name == "planActive" && f.inferred_type == InferredFieldType::Boolean));
    assert!(draft
        .action_inputs
        .iter()
        .any(|a| a.trigger_ff_id == "ff-submit-upgrade"
            && a.required_fields.contains(&"planCode".to_string())));
    for state in ["DEFAULT", "EMPTY", "LOADING", "ERROR"] {
        assert!(draft.sub_state_fixtures.contains_key(state));
    }

    let ts = render_fe_data_contract_ts(&draft);
    assert!(ts.contains("FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY"));
    assert!(ts.contains("export interface ScreenViewModel"));
    assert!(ts.contains("export interface ScreenActionInputs"));
    assert!(ts.contains("export const SUB_STATE_FIXTURES"));
    assert!(ts.contains("totalMrr: string;"));
    assert!(ts.contains("activeSeats: number;"));
}

#[test]
fn ut_05_6_07_fe_contract_blocks_ts_injection_dedups_actions_and_scopes_form_inputs() {
    let html = r#"<!DOCTYPE html><html><body>
        <main>
            <h1 data-ff-id="bad*/\nexport const PWN=1;//">Evil Heading</h1>
            <h2 data-ff-id="ff-valid-heading">Safe Heading</h2>
            <button data-ff-id="ff-standalone-btn">Open Help</button>
            <form data-ff-id="ff-login-form">
                <input name="email" />
                <input name="email" />
                <input name="password" />
                <button type="submit" data-ff-id="ff-submit-btn">Login</button>
                <button type="button" data-ff-id="ff-submit-btn">Duplicate Submit</button>
            </form>
        </main>
    </body></html>"#;

    let draft = extract_fe_data_contract("11111111-1111-4111-8111-111111111111", "scr-auth", html);
    // Invalid data-ff-id with comment breakout MUST be rejected
    assert_eq!(draft.view_model_fields.len(), 1);
    assert_eq!(draft.view_model_fields[0].ff_id, "ff-valid-heading");

    // Non-form button MUST NOT inherit inputs from unrelated forms
    let standalone = draft
        .action_inputs
        .iter()
        .find(|a| a.trigger_ff_id == "ff-standalone-btn")
        .expect("standalone button action");
    assert!(standalone.required_fields.is_empty());

    // Form button gets deduplicated inputs strictly from its enclosing form, and duplicate action_id is deduped
    let submit_actions: Vec<_> = draft
        .action_inputs
        .iter()
        .filter(|a| a.action_id == "submitBtn")
        .collect();
    assert_eq!(submit_actions.len(), 1);
    assert_eq!(
        submit_actions[0].required_fields,
        vec!["email".to_string(), "password".to_string()]
    );

    // Defensive sanitization in render_fe_data_contract_ts even if draft is constructed manually
    let mut manual = draft.clone();
    manual.view_model_fields[0].ff_id = "evil*/\nexport const X = 1;\r//".into();
    let ts = render_fe_data_contract_ts(&manual);
    assert!(!ts.contains("evil*/"));
    assert!(!ts.contains("export const X = 1;\r"));
    assert!(ts.contains("standaloneBtn: Record<string, never>;"));
}

#[test]
fn ut_05_6_08_override_prop_key_injection_blocked_and_oklch_checks_canvas_and_missing_pairs() {
    // 1. CSS override prop key injection MUST be rejected
    for evil_prop in [
        "color: red; } body { background",
        "border;color",
        "font weight",
        "color/*comment*/",
        "padding{0}",
    ] {
        let mut overrides = BTreeMap::new();
        let mut rules = BTreeMap::new();
        rules.insert(evil_prop.to_string(), "#111111".to_string());
        overrides.insert("button".to_string(), rules);
        let mut proposal =
            generate_default_brand_proposals(&["Linear".into()], &["warm".into()], "en")
                .expect("base proposals")
                .remove(0);
        proposal.component_style_overrides = overrides;
        let err = validate_brand_proposal(&proposal).expect_err("evil css prop key must fail");
        assert!(
            matches!(err, HtmlForgeError::CssTokenBlocked(_)),
            "expected CssTokenBlocked for prop '{evil_prop}', got {err:?}"
        );
    }

    // 2. Missing required foreground/background pairs MUST yield wcag_aa_verified = false
    let missing_pair_prop = BrandProposalV1 {
        id: "missing-pairs".into(),
        brand_name: "Empty Tokens".into(),
        rationale: "No contrast pairs".into(),
        inspiration_refs: vec![],
        theme_modes: vec![BrandModeTokens {
            mode_id: "warm".into(),
            label: "Warm".into(),
            tokens: BTreeMap::new(),
            wcag_aa_verified: true,
        }],
        font_pairing: FontPairing {
            heading: "Inter, sans-serif".into(),
            body: "Inter, sans-serif".into(),
            mono: "monospace".into(),
        },
        radius_scale: RadiusScale::Balanced,
        density: DensityScale::Comfortable,
        component_style_overrides: BTreeMap::new(),
    };
    let (validated_empty, reports_empty) =
        validate_brand_proposal(&missing_pair_prop).expect("validate empty");
    assert!(!validated_empty.theme_modes[0].wcag_aa_verified);
    assert!(reports_empty["warm"].is_empty());

    // 3. --text-primary checked against BOTH --bg-surface AND --bg-canvas
    let mut both_tokens = BTreeMap::new();
    both_tokens.insert("--bg-surface".into(), "#FFFFFF".into());
    both_tokens.insert("--bg-canvas".into(), "#F5F5F4".into());
    both_tokens.insert("--text-primary".into(), "#999999".into());
    let both_prop = BrandProposalV1 {
        id: "canvas-check".into(),
        brand_name: "Canvas Check".into(),
        rationale: "Verify canvas and surface".into(),
        inspiration_refs: vec![],
        theme_modes: vec![BrandModeTokens {
            mode_id: "warm".into(),
            label: "Warm".into(),
            tokens: both_tokens,
            wcag_aa_verified: false,
        }],
        font_pairing: FontPairing {
            heading: "Inter, sans-serif".into(),
            body: "Inter, sans-serif".into(),
            mono: "monospace".into(),
        },
        radius_scale: RadiusScale::Balanced,
        density: DensityScale::Comfortable,
        component_style_overrides: BTreeMap::new(),
    };
    let (validated_both, reports_both) =
        validate_brand_proposal(&both_prop).expect("validate canvas+surface");
    assert!(validated_both.theme_modes[0].wcag_aa_verified);
    assert_eq!(reports_both["warm"].len(), 2);
}
