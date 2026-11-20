// @story 06.2
// @implements AC-02, AC-04, AC-05, UT-01, UT-02, UT-03, UT-04
use presentation_cms::{
    inspect_slide_html_convergence, patch_slide_slots_lol_html, sanitize_slide_html_deep,
    validate_and_scrub_css_tokens, CmsPatcherError, CmsSlotKind, SlotPatchInput,
    CUSTOM_OVERRIDE_ATTR, MAX_SLIDE_HTML_BYTES,
};

const SLIDE_UID_1: &str = "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV";
const SLIDE_UID_2: &str = "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAW";

#[test]
fn ut_01_lol_html_patches_all_slot_kinds_and_scopes_one_page_strictly() {
    let one_page_html = format!(
        r#"<!DOCTYPE html>
<html lang="vi"><body>
  <section class="slide active" data-slide-uid="{SLIDE_UID_1}">
    <h1 data-ff-id="ff-title">Old Title 1</h1>
    <p data-ff-id="ff-kpi">0%</p>
    <ul data-ff-id="ff-bullets"><li>Old</li></ul>
    <img data-ff-id="ff-hero-img" src="/assets/old.png" alt="Hero" />
    <span data-ff-id="ff-accent">Accent</span>
    <div data-ff-id="ff-chart" data-charts-json="[]">Chart 0</div>
  </section>
  <section class="slide" data-slide-uid="{SLIDE_UID_2}">
    <h1 data-ff-id="ff-title">Untouched Sibling Title</h1>
  </section>
</body></html>"#
    );

    let patches = vec![
        SlotPatchInput {
            ff_id: "ff-title".into(),
            slot_kind: CmsSlotKind::Heading,
            text_value: "Kiến trúc Cowok-design Tiếng Việt <safe>".into(),
            bullet_items: None,
            charts_json: None,
            safe_styles_json: Some(r##"{"color":"#0057FF","font-weight":"700"}"##.into()),
        },
        SlotPatchInput {
            ff_id: "ff-kpi".into(),
            slot_kind: CmsSlotKind::Metric,
            text_value: "+42.8% ARR".into(),
            bullet_items: None,
            charts_json: None,
            safe_styles_json: None,
        },
        SlotPatchInput {
            ff_id: "ff-bullets".into(),
            slot_kind: CmsSlotKind::Bullet,
            text_value: String::new(),
            bullet_items: Some(vec![
                "Điểm 1 <bold>".into(),
                "Điểm 2 — 100% Pure CSS".into(),
            ]),
            charts_json: None,
            safe_styles_json: None,
        },
        SlotPatchInput {
            ff_id: "ff-hero-img".into(),
            slot_kind: CmsSlotKind::Image,
            text_value: "https://cdn.example.com/hero.png".into(),
            bullet_items: None,
            charts_json: None,
            safe_styles_json: None,
        },
        SlotPatchInput {
            ff_id: "ff-accent".into(),
            slot_kind: CmsSlotKind::Color,
            text_value: "#E89A35".into(),
            bullet_items: None,
            charts_json: None,
            safe_styles_json: None,
        },
        SlotPatchInput {
            ff_id: "ff-chart".into(),
            slot_kind: CmsSlotKind::ChartJson,
            text_value: "Q4 Velocity".into(),
            bullet_items: None,
            charts_json: Some(r#"{"kind":"bar","title":"Q4 Velocity","series":[{"name":"Velocity","points":[{"label":"M1","value":12},{"label":"M2","value":24},{"label":"M3","value":48}]}]}"#.into()),
            safe_styles_json: None,
        },
    ];

    let out = patch_slide_slots_lol_html(&one_page_html, Some(SLIDE_UID_1), &patches)
        .expect("lol_html patch should succeed");

    assert_eq!(out.patched_slots_count, 6);
    assert!(out
        .patched_html
        .contains("Kiến trúc Cowok-design Tiếng Việt &lt;safe&gt;"));
    assert!(out.patched_html.contains("+42.8% ARR"));
    assert!(out
        .patched_html
        .contains("<li>Điểm 1 &lt;bold&gt;</li><li>Điểm 2 — 100% Pure CSS</li>"));
    assert!(out
        .patched_html
        .contains(r#"src="https://cdn.example.com/hero.png""#));
    assert!(out.patched_html.contains("color: #E89A35"));
    assert!(out.patched_html.contains("Q4 Velocity"));
    assert!(
        out.patched_html.contains("Untouched Sibling Title"),
        "one-page patch MUST NOT touch sibling slide with same ff_id"
    );
    assert_eq!(out.content_sha256.len(), 64);

    // Reject CSS selector injection in ff_id
    let bad_ff = SlotPatchInput {
        ff_id: r#"ff-1"], script[src=""#.into(),
        slot_kind: CmsSlotKind::Text,
        text_value: "x".into(),
        bullet_items: None,
        charts_json: None,
        safe_styles_json: None,
    };
    let err = patch_slide_slots_lol_html(&one_page_html, None, &[bad_ff]).unwrap_err();
    assert!(matches!(err, CmsPatcherError::InvalidFfId(_)));
}

#[test]
fn ut_02_lol_html_preserves_utf8_cjk_and_rejects_unsafe_image_uri() {
    let html = r#"<section class="slide active"><p data-ff-id="ff-body">Hello</p><img data-ff-id="ff-img" src="/assets/a.png" /></section>"#;
    let out = patch_slide_slots_lol_html(
        html,
        None,
        &[SlotPatchInput {
            ff_id: "ff-body".into(),
            slot_kind: CmsSlotKind::Text,
            text_value: "日本語テスト • Tiếng Việt Telex ăâêôơưđ".into(),
            bullet_items: None,
            charts_json: None,
            safe_styles_json: None,
        }],
    )
    .expect("utf8 patch ok");
    assert!(out
        .patched_html
        .contains("日本語テスト • Tiếng Việt Telex ăâêôơưđ"));

    let bad_img = patch_slide_slots_lol_html(
        html,
        None,
        &[SlotPatchInput {
            ff_id: "ff-img".into(),
            slot_kind: CmsSlotKind::Image,
            text_value: "javascript:alert(1)".into(),
            bullet_items: None,
            charts_json: None,
            safe_styles_json: None,
        }],
    )
    .unwrap_err();
    assert!(matches!(bad_img, CmsPatcherError::MxssBlocked(_)));
}

#[test]
fn ut_03_fail_closed_css_grammar_and_html5ever_mxss_blocking() {
    // CSS fail-closed vectors
    for bad_css in [
        r"color: \65 xpression(alert(1))",
        "color: red /* unclosed",
        "@import url(https://evil.test/x.css);",
        "background-color: url(https://evil.test/x.png)",
        "color: expression(alert(1))",
        "-moz-binding: url(x.xml)",
        "content: 'unsafe-inline'",
        "position: fixed",
        "color: red !important",
    ] {
        assert!(
            matches!(
                validate_and_scrub_css_tokens(bad_css, true),
                Err(CmsPatcherError::CssTokenBlocked(_))
            ),
            "Expected CSS vector to be blocked: {bad_css}"
        );
    }

    // Valid inline CSS passes
    let ok_css = validate_and_scrub_css_tokens(
        "color: #0057FF; font-weight: 700; padding: 12px 16px; background-color: var(--bg-surface)",
        true,
    )
    .expect("valid safe CSS should pass");
    assert!(ok_css.contains("color: #0057FF"));

    // HTML5 mXSS / dangerous tags / attributes (including AC-05 exact oracle & nested @media protected selectors)
    for bad_html in [
        r#"<section class="slide active"><script>alert(1)</script></section>"#,
        r#"<section class="slide active"><svg><foreignObject><body onload=alert(1)></body></foreignObject></svg></section>"#,
        r#"<section class="slide active"><img src="/assets/x.png" onerror="alert(1)" /></section>"#,
        r#"<section class="slide active"><a href="javascript:alert(1)">click</a></section>"#,
        r#"<section class="slide active"><style>@import url(https://evil.test/a.css);</style></section>"#,
        r#"<section class="slide active"><style></styl/*x*/e><img onerror=alert(1)></style></section>"#,
        r#"<section class="slide active"><style>@media all { body { display: none !important; } }</style></section>"#,
        r#"<section class="slide active"><style>html.dark { color: red !important; }</style></section>"#,
        r#"<section class="slide active"><iframe srcdoc="<script>1</script>"></iframe></section>"#,
    ] {
        let res = sanitize_slide_html_deep(bad_html, &[], true);
        assert!(
            matches!(
                res,
                Err(CmsPatcherError::MxssBlocked(_)) | Err(CmsPatcherError::CssTokenBlocked(_))
            ),
            "Expected mXSS vector to fail closed: {bad_html}, got {res:?}"
        );
    }

    // Multibyte UTF-8 exceeding 524,288 bytes -> PayloadTooLarge (413)
    let huge = format!(
        r#"<section class="slide active"><p data-ff-id="ff-1">{}</p></section>"#,
        "ệ".repeat((MAX_SLIDE_HTML_BYTES / 3) + 64)
    );
    assert!(huge.len() > MAX_SLIDE_HTML_BYTES);
    assert!(matches!(
        sanitize_slide_html_deep(&huge, &[], true),
        Err(CmsPatcherError::PayloadTooLarge { .. })
    ));
}

#[test]
fn ut_04_safe_svg_passes_and_custom_override_convergence_roundtrips() {
    let valid_svg_slide = r#"<!DOCTYPE html>
<html><body>
  <section class="slide active" data-slide-uid="SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV">
    <h1 data-ff-id="ff-h1" data-slot-label="Headline">Executive Summary</h1>
    <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.85">
      <path d="M4 12h16" />
      <circle cx="12" cy="12" r="8" />
    </svg>
  </section>
</body></html>"#;

    // Missing expected slot `ff-kpi` stamps `data-cowok-custom-override="1"`
    let diverged =
        sanitize_slide_html_deep(valid_svg_slide, &["ff-h1".into(), "ff-kpi".into()], true)
            .expect("safe svg slide should pass sanitizer");
    assert!(diverged.custom_html_override);
    assert_eq!(diverged.unmapped_slot_ids, vec!["ff-kpi".to_string()]);
    assert!(diverged
        .sanitized_html
        .contains(&format!(r#"{CUSTOM_OVERRIDE_ATTR}="1""#)));
    assert!(diverged.sanitized_html.contains("<svg "));
    assert!(diverged.sanitized_html.contains("<path "));

    // Convergence inspection confirms override survives reload/restart without DB migration
    let (is_override, unmapped) = inspect_slide_html_convergence(
        &diverged.sanitized_html,
        &["ff-h1".into(), "ff-kpi".into()],
    );
    assert!(is_override);
    assert_eq!(unmapped, vec!["ff-kpi".to_string()]);

    // Re-Sync with matching slots clears `data-cowok-custom-override`
    let synced = sanitize_slide_html_deep(valid_svg_slide, &["ff-h1".into()], true)
        .expect("re-synced slide should pass");
    assert!(!synced.custom_html_override);
    assert!(synced.unmapped_slot_ids.is_empty());
    assert!(!synced.sanitized_html.contains(CUSTOM_OVERRIDE_ATTR));
}
