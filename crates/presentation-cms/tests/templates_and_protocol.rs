// @story 06.1
// @implements AC-01, AC-04, AC-05, UT-02, UT-03, UT-06
use presentation_cms::{
    render_branded_slide, rewrite_slide_html_for_duplicate, sanitize_slide_html_fragment,
    validate_slide_protocol_structure, ProtocolValidationError, RenderSlideRequest,
    SlideLayoutKind, MAX_SLIDE_HTML_BYTES,
};

#[test]
fn ut_02_all_8_templates_render_protocol_and_enforce_size_cap() {
    for kind in SlideLayoutKind::all() {
        let out = render_branded_slide(&RenderSlideRequest {
            slide_uid: None,
            layout_kind: *kind,
            theme_preset: "night".to_string(),
            aspect_ratio: "16:9".to_string(),
            eyebrow_tag: "Q4 BRIEF".to_string(),
            headline: format!("Headline for {}", kind.as_str()),
            subtitle: "Inherited from PROJECT_CONSTRAINTS".to_string(),
            primary_tokens_css: ":root { --custom-token: #38BDF8; }".to_string(),
            icon_manifest_version: "1.85px-monoline".to_string(),
        });

        assert_eq!(out.slide_uid.len(), 30);
        assert!(out.html.contains("--type-display: 56px"));
        assert!(out.html.contains("--type-micro: 16px"));
        let validated =
            validate_slide_protocol_structure(&out.html).expect("valid template protocol");
        assert!(validated.has_active_slide);
        assert!(validated.has_deck_protocol_attr);
        assert!(validated.unique_ff_ids.len() >= 4);
    }

    let oversized = format!(
        "<section class=\"slide active\"><h1 data-ff-id=\"ff-1\">{}</h1></section>",
        "A".repeat(MAX_SLIDE_HTML_BYTES + 128)
    );
    let err = validate_slide_protocol_structure(&oversized).expect_err("must reject > 512KB");
    assert!(matches!(
        err,
        ProtocolValidationError::PayloadTooLarge { .. }
    ));
}

#[test]
fn ut_03_duplicate_rewrite_generates_unique_ff_ids_and_detects_duplicates() {
    let dup_bad = "<section class=\"slide active\"><h1 data-ff-id=\"ff-1\">A</h1><p data-ff-id=\"ff-1\">B</p></section>";
    let err = validate_slide_protocol_structure(dup_bad).expect_err("duplicate ff_id rejected");
    assert_eq!(
        err,
        ProtocolValidationError::DuplicateFfId("ff-1".to_string())
    );

    let src = render_branded_slide(&RenderSlideRequest {
        slide_uid: Some("SLD-01J00000000000000000000001".to_string()),
        layout_kind: SlideLayoutKind::HeroCover,
        theme_preset: "night".to_string(),
        aspect_ratio: "16:10".to_string(),
        eyebrow_tag: "TAG".to_string(),
        headline: "Slide Title".to_string(),
        subtitle: "Subtitle".to_string(),
        primary_tokens_css: String::new(),
        icon_manifest_version: "v4.3".to_string(),
    });

    let (cloned_html, id_map) =
        rewrite_slide_html_for_duplicate(&src.html, "SLD-01J00000000000000000000002", "c2")
            .expect("rewrite clone");
    assert!(cloned_html.contains("data-slide-uid=\"SLD-01J00000000000000000000002\""));
    for (old_id, new_id) in &id_map {
        assert_ne!(old_id, new_id);
        assert!(new_id.len() <= 64);
        assert!(cloned_html.contains(&format!("data-ff-id=\"{new_id}\"")));
    }

    // Boundary check: 64-character data-ff-id values remain <= 64 chars and unique after duplicate
    let max_a = "ff-".to_string() + &"a".repeat(61);
    let max_b = "ff-".to_string() + &"a".repeat(60) + "b";
    let edge_html = format!(
        "<section class=\"slide active\" data-cowok-deck-protocol=\"1\" data-slide-uid=\"SLD-01J00000000000000000000001\"><h1 data-ff-id=\"{max_a}\">A</h1><p data-ff-id=\"{max_b}\">B</p></section>"
    );
    let (dup_html, dup_map) =
        rewrite_slide_html_for_duplicate(&edge_html, "SLD-01J00000000000000000000003", "dup123456")
            .expect("duplicate with 64-char ffIds");
    assert_eq!(dup_map.len(), 2);
    assert_ne!(dup_map[0].1, dup_map[1].1);
    for (_, next_id) in &dup_map {
        assert!(next_id.len() <= 64, "bounded ffId exceeded 64: {next_id}");
        assert!(presentation_cms::is_valid_ff_id(next_id));
        assert!(dup_html.contains(&format!("data-ff-id=\"{next_id}\"")));
    }
}

#[test]
fn ut_06_adversarial_scripts_event_handlers_meta_base_and_javascript_urls_blocked() {
    let hostile = "<section class=\"slide active\"><meta http-equiv=\"refresh\"><base href=\"//evil\"><script>window.parent.postMessage('steal')</script><iframe srcdoc=\"bad\"></iframe><h1 data-ff-id=\"ff-safe\" onclick=\"steal()\" onload=\"steal()\">Title</h1></section>";
    let sanitized = sanitize_slide_html_fragment(hostile).expect("strips script & handlers");
    let lower = sanitized.to_ascii_lowercase();
    assert!(!lower.contains("<script"));
    assert!(!lower.contains("<meta"));
    assert!(!lower.contains("<base"));
    assert!(!lower.contains("<iframe"));
    assert!(!lower.contains("onclick="));
    assert!(!lower.contains("onload="));

    let js_url = "<section class=\"slide active\"><a data-ff-id=\"ff-link\" href=\"javascript:alert(1)\">click</a></section>";
    let err =
        validate_slide_protocol_structure(js_url).expect_err("javascript: URI must fail closed");
    assert!(matches!(
        err,
        ProtocolValidationError::ForbiddenHtmlConstruct(_)
    ));

    // Adversarial bypass checks: tab/slash event handlers, unquoted ontoggle, </STYLE>, &#106;avascript:, prose slide+active, single-quoted dup ff-id
    let bypass_vec = "<section class=\"slide active\"><svg><script>alert(1)</script></svg><metadata>keep</metadata><h1 data-ff-id=\"ff-1\"\tonanimationstart=\"evil()\" /onerror=evil() ontoggle='evil()'>Hi</h1></section>";
    let cleaned = sanitize_slide_html_fragment(bypass_vec)
        .expect("strips svg, onanimationstart, onerror, ontoggle");
    let cl = cleaned.to_ascii_lowercase();
    assert!(!cl.contains("<svg"));
    assert!(cl.contains("<metadata>keep</metadata>"));
    assert!(!cl.contains("onanimationstart"));
    assert!(!cl.contains("onerror"));
    assert!(!cl.contains("ontoggle"));

    let entity_js = "<section class=\"slide active\"><a data-ff-id=\"ff-e\" href=\"&#106;avascript:alert(1)\">x</a></section>";
    assert!(matches!(
        validate_slide_protocol_structure(entity_js),
        Err(ProtocolValidationError::ForbiddenHtmlConstruct(_))
    ));

    let prose_false_pos =
        "<section><p data-ff-id=\"ff-p\">slide transition is active here</p></section>";
    assert_eq!(
        validate_slide_protocol_structure(prose_false_pos).unwrap_err(),
        ProtocolValidationError::MissingActiveSlide
    );

    let single_quote_dup = "<section class='slide active'><h1 data-ff-id='ff-sq'>A</h1><p data-ff-id='ff-sq'>B</p></section>";
    assert_eq!(
        validate_slide_protocol_structure(single_quote_dup).unwrap_err(),
        ProtocolValidationError::DuplicateFfId("ff-sq".to_string())
    );

    let auto_slide = render_branded_slide(&RenderSlideRequest {
        slide_uid: Some("SLD-01J00000000000000000000004".to_string()),
        layout_kind: SlideLayoutKind::HeroCover,
        theme_preset: "night".to_string(),
        aspect_ratio: "Auto".to_string(),
        eyebrow_tag: "T".to_string(),
        headline: "Auto".to_string(),
        subtitle: "S".to_string(),
        primary_tokens_css: String::new(),
        icon_manifest_version: "v4.3".to_string(),
    });
    assert!(auto_slide.html.contains("--slide-w: 1440px;"));
    assert!(auto_slide.html.contains("--slide-h: 900px;"));

    let style_breakout = render_branded_slide(&RenderSlideRequest {
        slide_uid: Some("SLD-01J00000000000000000000003".to_string()),
        layout_kind: SlideLayoutKind::HeroCover,
        theme_preset: "night".to_string(),
        aspect_ratio: "16:9".to_string(),
        eyebrow_tag: "T".to_string(),
        headline: "H".to_string(),
        subtitle: "S".to_string(),
        primary_tokens_css: ":root{--x:1}</STYLE><script>alert(1)</script>".to_string(),
        icon_manifest_version: "v4.3".to_string(),
    });
    assert!(!style_breakout.html.contains("</STYLE>"));

    // Round 3 security regressions: unquoted slash in style, missing space before style, unclosed quote/style tag
    assert!(sanitize_slide_html_fragment(
        "<section class=\"slide active\"><div style=a/background:expression(1)>x</div></section>"
    )
    .is_err());
    assert!(sanitize_slide_html_fragment(
        "<section class=\"slide active\"><div class=\"x\"style=background:expression(1)>x</div></section>"
    )
    .is_err());
    assert!(sanitize_slide_html_fragment(
        "<section class=\"slide active\"><div style=\"color:red>x</div></section>"
    )
    .is_err());
    assert!(sanitize_slide_html_fragment(
        "<section class=\"slide active\"><style body{color:red}</section>"
    )
    .is_err());
}
