// @story 07.2
// @implements AC-01, AC-03, AC-04
use std::collections::BTreeMap;

use html_forge::export_bundler::{
    build_developer_handoff, build_offline_csp_header, build_vector_pdf, bundle_offline_html,
    count_external_cdn_refs, DeveloperHandoffInput, ExportBundlerError, OfflineHtmlInput,
    VectorPdfInput, VectorPdfSlide, MAX_BUNDLE_BYTES, OFFLINE_CSP_CONTENT,
};

#[test]
fn ut_07_2_01_offline_html_strips_external_cdns_injects_csp_and_stageshow_presenter() {
    let mut cas_assets = BTreeMap::new();
    cas_assets.insert(
        "cas://logo-1".to_string(),
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_string(),
    );

    let html = r#"<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="https://cdn.example.com/tailwind.min.css">
  <script src="https://cdn.example.com/analytics.js"></script>
  <style>
    @import url('https://fonts.googleapis.com/css2?family=Inter');
    .hero { background-image: url("https://cdn.example.com/bg.png"); }
  </style>
</head>
<body>
  <section data-slide="1" style="background: url(https://cdn.example.com/card.jpg);">
    <h1>Quarterly Deck</h1>
    <img src="cas://logo-1" alt="Brand Logo">
    <img src="https://cdn.example.com/remote-photo.jpg" alt="Remote">
    <svg viewBox="0 0 120 80"><rect width="120" height="80" /></svg>
  </section>
  <section data-slide="2">
    <h2>Roadmap</h2>
    <svg><circle cx="20" cy="20" r="10" /></svg>
  </section>
</body>
</html>"#;

    let bundle = bundle_offline_html(&OfflineHtmlInput {
        title: "Q4 Investor Deck".to_string(),
        html: html.to_string(),
        inline_css: Some(
            "@import url('https://fonts.example.com/a.css'); .slide { padding: 24px; }".to_string(),
        ),
        cas_assets,
        enable_presenter: true,
        max_bytes: None,
    })
    .expect("offline bundle should succeed");

    assert_eq!(bundle.external_cdn_refs_count, 0);
    assert_eq!(count_external_cdn_refs(&bundle.html_content), 0);
    assert!(bundle.stripped_external_refs_count >= 5);
    assert!(bundle.html_content.contains(OFFLINE_CSP_CONTENT));
    assert!(bundle.presenter_enabled);
    assert!(bundle.html_content.contains("ArrowRight"));
    assert!(bundle.html_content.contains("ArrowLeft"));
    assert!(bundle.html_content.contains("data-stageshow-presenter"));
    assert_eq!(bundle.slide_count, 2);
    assert_eq!(bundle.svg_count, 2);
    assert_eq!(bundle.broken_svg_count, 1);
    assert_eq!(bundle.inline_assets_count, 1);
    assert_eq!(bundle.broken_cas_assets_count, 0);
    assert_eq!(bundle.sha256.len(), 64);
    assert!(bundle.html_content.contains("data:image/png;base64,"));
    assert!(!bundle.html_content.contains("https://cdn.example.com"));
}

#[test]
fn ut_07_2_02_enforces_25mb_asset_size_limit_before_emitting() {
    let oversized = "x".repeat(MAX_BUNDLE_BYTES + 64);
    let err = bundle_offline_html(&OfflineHtmlInput {
        title: "Huge Deck".to_string(),
        html: format!("<section>{oversized}</section>"),
        inline_css: None,
        cas_assets: BTreeMap::new(),
        enable_presenter: true,
        max_bytes: None,
    })
    .expect_err("oversized bundle must be rejected");

    match err {
        ExportBundlerError::AssetSizeExceeded {
            actual_bytes,
            limit_bytes,
        } => {
            assert_eq!(limit_bytes, MAX_BUNDLE_BYTES);
            assert!(actual_bytes > MAX_BUNDLE_BYTES);
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn ut_07_2_03_vector_pdf_emits_valid_16_9_pdf_1_4_stream_and_deterministic_sha256() {
    let input = VectorPdfInput {
        title: "Executive Summary".to_string(),
        slides: vec![
            VectorPdfSlide {
                heading: "Slide 1: Vision".to_string(),
                body_lines: vec!["Self-contained offline presentation engine.".to_string()],
                footer: Some("Confidential".to_string()),
            },
            VectorPdfSlide {
                heading: "Slide 2: Architecture".to_string(),
                body_lines: vec!["Shared Rust html-forge kernel.".to_string()],
                footer: None,
            },
        ],
        html: None,
        max_bytes: None,
    };

    let pdf1 = build_vector_pdf(&input).expect("pdf generation should succeed");
    let pdf2 = build_vector_pdf(&input).expect("pdf generation should be deterministic");

    assert_eq!(pdf1.sha256, pdf2.sha256);
    assert_eq!(pdf1.page_count, 2);
    assert_eq!(pdf1.width_pt, 1440.0);
    assert_eq!(pdf1.height_pt, 810.0);
    assert_eq!(pdf1.engine, "html_forge_vector");
    assert!(pdf1.byte_size > 200);
    assert!(pdf1.pdf_base64.starts_with("JVBERi0xLjQK")); // "%PDF-1.4\n" in base64
}

#[test]
fn ut_07_2_04_developer_handoff_packages_canonical_files_deterministically() {
    let bundle = build_developer_handoff(&DeveloperHandoffInput {
        project_name: "Billing Portal".to_string(),
        spec_markdown: None,
        design_tokens_css: None,
        design_tokens_json: None,
        screen_tsx: None,
        html_source: Some("<main><h1>Billing</h1></main>".to_string()),
        e2e_spec_ts: None,
    })
    .expect("developer handoff should succeed");

    let paths: Vec<&str> = bundle.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "DESIGN_TOKENS.css",
            "DESIGN_TOKENS.json",
            "SPEC.md",
            "Screen.tsx",
            "flow.e2e.spec.ts"
        ]
    );
    assert_eq!(bundle.sha256.len(), 64);
    assert!(bundle.byte_size > 100);
}

#[test]
fn ut_07_2_05_stored_xss_style_breakout_and_comment_spoofed_head_body_neutralized() {
    let malicious_html = r#"<!-- <html><head></head><body> -->
<section data-slide="1">
  <base href="https://evil.example.com/">
  <meta http-equiv="refresh" content="0;url=https://evil.example.com/phish">
  <script>window.__xss = 1;</script>
  <script src="https://evil.example.com/pwn.js"></script>
  <iframe src="https://evil.example.com/frame"></iframe>
  <object data="https://evil.example.com/obj.swf"></object>
  <embed src="https://evil.example.com/emb.swf">
  <style>
    .x { color: red; }
    </StYlE><script>window.__styleBreakout = 1;</script><style>
  </style>
  <a href="javascript:alert(1)" onclick="alert(2)">Bad Link</a>
  <a href="vbscript:msgbox(1)" onmouseover="alert(3)">VB Link</a>
  <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=" onerror="alert(4)">
</section>"#;

    let bundle = bundle_offline_html(&OfflineHtmlInput {
        title: "Security Audit".to_string(),
        html: malicious_html.to_string(),
        inline_css: Some(
            ".a{color:blue}</STYLE><script>window.__inlineCssBreakout=1;</script>".to_string(),
        ),
        cas_assets: BTreeMap::new(),
        enable_presenter: true,
        max_bytes: None,
    })
    .expect("bundle should sanitize XSS and succeed");

    let expected_csp = build_offline_csp_header(true);
    assert_eq!(bundle.csp_header, expected_csp);
    assert!(bundle.csp_header.contains("base-uri 'none'"));
    assert!(bundle.csp_header.contains("form-action 'none'"));
    assert!(bundle.csp_header.contains("object-src 'none'"));
    assert!(bundle.csp_header.contains("frame-src 'none'"));
    assert!(bundle.csp_header.contains("script-src 'sha256-"));
    assert!(!bundle.csp_header.contains("script-src 'unsafe-inline'"));
    assert!(bundle.html_content.contains(&expected_csp));
    assert!(bundle
        .html_content
        .contains("data-stageshow-runtime=\"true\""));

    // Zero attacker scripts, event handlers, or active elements survive
    assert!(!bundle.html_content.contains("window.__xss"));
    assert!(!bundle.html_content.contains("window.__styleBreakout"));
    assert!(!bundle.html_content.contains("window.__inlineCssBreakout"));
    assert!(!bundle.html_content.contains("onclick="));
    assert!(!bundle.html_content.contains("onmouseover="));
    assert!(!bundle.html_content.contains("onerror="));
    assert!(!bundle.html_content.contains("javascript:"));
    assert!(!bundle.html_content.contains("vbscript:"));
    assert!(!bundle.html_content.contains("<iframe"));
    assert!(!bundle.html_content.contains("<object"));
    assert!(!bundle.html_content.contains("<embed"));
    assert!(!bundle.html_content.contains("<base"));
    assert!(!bundle.html_content.contains("http-equiv=\"refresh\""));
    assert_eq!(count_external_cdn_refs(&bundle.html_content), 0);
}

#[test]
fn ut_07_2_06_external_cdn_loaders_srcset_css_escapes_and_import_semicolons_stripped() {
    let html = r#"<!DOCTYPE html>
<html>
<head>
  <link rel="preload" as="image" imagesrcset="https://cdn.evil.com/a.png 1x, data:image/png;base64,iVBORw0KGgo= 2x">
  <style>
    /* comment */ @import url("https://fonts.googleapis.com/css2?family=Inter:wght@400;700&display=swap");
    .esc { background: \75 rl("https://cdn.evil.com/escaped.png"); }
    .imgset { background-image: image-set("https://cdn.evil.com/set.png" 1x, "data:image/png;base64,iVBORw0KGgo=" 2x); }
  </style>
</head>
<body>
  <section data-slide="1">
    <video poster="https://cdn.evil.com/poster.jpg" src="https://cdn.evil.com/vid.mp4">
      <source srcset="https://cdn.evil.com/v1.webp 1x, data:image/png;base64,iVBORw0KGgo= 2x">
      <track src="https://cdn.evil.com/subs.vtt">
    </video>
    <input type="image" src="https://cdn.evil.com/btn.png">
    <img srcset="https://cdn.evil.com/hi.png 2x, //cdn.evil.com/proto.png 3x, data:image/png;base64,iVBORw0KGgo= 1x" src="data:image/png;base64,iVBORw0KGgo=">
    <svg viewBox="0 0 100 100">
      <image href="https://cdn.evil.com/svg-img.png" xlink:href="https://cdn.evil.com/svg-xlink.png" />
      <use href="https://cdn.evil.com/sprite.svg#icon" xlink:href="https://cdn.evil.com/sprite2.svg#icon" />
      <feImage href="https://cdn.evil.com/filter.png" />
    </svg>
  </section>
</body>
</html>"#;

    let bundle = bundle_offline_html(&OfflineHtmlInput {
        title: "Loader Hardening".to_string(),
        html: html.to_string(),
        inline_css: None,
        cas_assets: BTreeMap::new(),
        enable_presenter: false,
        max_bytes: None,
    })
    .expect("bundle should strip all external loaders cleanly");

    assert_eq!(bundle.external_cdn_refs_count, 0);
    assert_eq!(count_external_cdn_refs(&bundle.html_content), 0);
    assert!(!bundle.html_content.contains("cdn.evil.com"));
    assert!(!bundle.html_content.contains("fonts.googleapis.com"));
    // Valid data URI candidate inside srcset & image-set preserved
    assert!(bundle
        .html_content
        .contains("data:image/png;base64,iVBORw0KGgo="));
    assert!(bundle.csp_header.contains("script-src 'none'"));
}

#[test]
fn ut_07_2_07_cas_reference_amplification_aborts_incrementally_and_pdf_dev_handoff_bounds() {
    let mut cas_assets = BTreeMap::new();
    let payload = format!("data:image/png;base64,{}", "A".repeat(40 * 1024));
    cas_assets.insert("cas://big-img".to_string(), payload);

    // Reference the single 40 KiB CAS asset 20 times -> ~800 KiB expansion, exceeding 256 KiB cap
    let imgs = "<img src=\"cas://big-img\">".repeat(20);
    let err = bundle_offline_html(&OfflineHtmlInput {
        title: "Amplification Attack".to_string(),
        html: format!("<section>{imgs}</section>"),
        inline_css: None,
        cas_assets,
        enable_presenter: false,
        max_bytes: Some(256 * 1024),
    })
    .expect_err("CAS amplification must abort with AssetSizeExceeded");
    assert!(matches!(err, ExportBundlerError::AssetSizeExceeded { .. }));

    // Vector PDF upfront input bound
    let pdf_err = build_vector_pdf(&VectorPdfInput {
        title: "Huge PDF".to_string(),
        slides: vec![VectorPdfSlide {
            heading: "A".repeat(4096),
            body_lines: vec![],
            footer: None,
        }],
        html: None,
        max_bytes: Some(1024),
    })
    .expect_err("oversized PDF input must fail upfront");
    assert!(matches!(
        pdf_err,
        ExportBundlerError::AssetSizeExceeded { .. }
    ));

    // Developer handoff upfront input bound
    let dev_err = build_developer_handoff(&DeveloperHandoffInput {
        project_name: "Oversized".to_string(),
        spec_markdown: Some("x".repeat(MAX_BUNDLE_BYTES + 16)),
        design_tokens_css: None,
        design_tokens_json: None,
        screen_tsx: None,
        html_source: None,
        e2e_spec_ts: None,
    })
    .expect_err("oversized dev handoff must fail upfront");
    assert!(matches!(
        dev_err,
        ExportBundlerError::AssetSizeExceeded { .. }
    ));
}

#[test]
fn ut_07_2_08_jsx_safe_escaping_and_vietnamese_pdf_transliteration() {
    let handoff = build_developer_handoff(&DeveloperHandoffInput {
        project_name: "Billing }<script>alert(1)</script>{ Portal".to_string(),
        spec_markdown: None,
        design_tokens_css: None,
        design_tokens_json: None,
        screen_tsx: None,
        html_source: None,
        e2e_spec_ts: None,
    })
    .expect("developer handoff should succeed");

    let screen = handoff
        .files
        .iter()
        .find(|f| f.path == "Screen.tsx")
        .expect("Screen.tsx present");
    assert!(screen
        .content
        .contains("<h1>{\"Billing }<script>alert(1)</script>{ Portal\"}</h1>"));

    let pdf = build_vector_pdf(&VectorPdfInput {
        title: "Báo cáo Quý 4".to_string(),
        slides: vec![VectorPdfSlide {
            heading: "Báo cáo Quý 4 — Kế hoạch Đổi mới".to_string(),
            body_lines: vec!["Triển khai hệ thống xuất bản ngoại tuyến.".to_string()],
            footer: Some("Bảo mật • Trang 1".to_string()),
        }],
        html: None,
        max_bytes: None,
    })
    .expect("Vietnamese PDF should succeed");

    // Decode base64 back to ASCII/Latin PDF bytes and assert clean transliteration (zero `?`)
    let decoded_str = {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut lut = [255u8; 256];
        for (idx, &b) in TABLE.iter().enumerate() {
            lut[b as usize] = idx as u8;
        }
        let mut out = Vec::new();
        let bytes = pdf.pdf_base64.as_bytes();
        let mut i = 0;
        while i + 3 < bytes.len() {
            let a = lut[bytes[i] as usize] as u32;
            let b = lut[bytes[i + 1] as usize] as u32;
            let c = if bytes[i + 2] == b'=' {
                0
            } else {
                lut[bytes[i + 2] as usize] as u32
            };
            let d = if bytes[i + 3] == b'=' {
                0
            } else {
                lut[bytes[i + 3] as usize] as u32
            };
            let n = (a << 18) | (b << 12) | (c << 6) | d;
            out.push(((n >> 16) & 0xff) as u8);
            if bytes[i + 2] != b'=' {
                out.push(((n >> 8) & 0xff) as u8);
            }
            if bytes[i + 3] != b'=' {
                out.push((n & 0xff) as u8);
            }
            i += 4;
        }
        String::from_utf8_lossy(&out).to_string()
    };

    assert!(decoded_str.contains("(Bao cao Quy 4 - Ke hoach Doi moi)"));
    assert!(decoded_str.contains("(Trien khai he thong xuat ban ngoai tuyen.)"));
    assert!(decoded_str.contains("(Bao mat * Trang 1)"));
}
