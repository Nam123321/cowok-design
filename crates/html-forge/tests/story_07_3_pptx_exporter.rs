// @story 07.3
// @implements AC-01, AC-02
//! Comprehensive test suite for the Built-in Native Rust OOXML `.pptx` Exporter covering
//! all 6 core algorithms, 18 fail-closed crash-prevention invariants, ZIP decompression,
//! `quick-xml::Reader` well-formedness verification, and zero dangling `rId` integrity.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Cursor, Read};

use html_forge::export_bundler::ExportBundlerError;
use html_forge::pptx_ir::{
    apply_text_width_buffer, border_radius_to_geom, css_box_shadow_to_ooxml,
    css_linear_gradient_deg_to_ooxml, encode_solid_png, font_size_px_to_ooxml, parse_css_color,
    parse_css_linear_gradient, px_rect_to_emu, sanitize_xml_text, sha256_bytes,
    slugify_pptx_filename, PptxColor, PptxDeckIr, PptxExportMode, PptxFidelityWarningCode,
    PptxFill, PptxMediaAsset, PptxMediaFormat, PptxNode, PptxRectEmu, PptxShapeGeom, PptxSlideIr,
    PptxSlideTransition, PptxTableCellKind, PptxTableGrid, PptxTableRawCell, PptxTextAlign,
    EMU_PER_PX, MAX_SHAPES_PER_SLIDE, MAX_SLIDES_PER_DECK, SLIDE_H_EMU, SLIDE_W_EMU,
};
use html_forge::pptx_layout::{
    CdpDeckSnapshot, CdpElementSnapshot, CdpSlideSnapshot, PptxExportInput, PptxLayoutExtractor,
};
use html_forge::pptx_ooxml::{build_pptx_deck, emit_pptx_bundle, encode_base64};
use html_forge::visual_qc::PooledCdpEvaluator;
use quick_xml::events::Event;
use quick_xml::Reader;
use unicode_normalization::UnicodeNormalization;
use zip::ZipArchive;

/// Unpacks the generated `.pptx` ZIP archive, verifies every `.xml`/`.rels` entry parses cleanly
/// with `quick-xml::Reader`, checks required OPC parts exist, and asserts zero dangling `rId`s.
fn verify_ooxml_zip_integrity(pptx_bytes: &[u8]) -> HashMap<String, String> {
    assert!(pptx_bytes.len() >= 4, "ZIP must have magic header");
    assert_eq!(
        &pptx_bytes[0..4],
        b"PK\x03\x04",
        "must start with ZIP local file header"
    );

    let mut archive = ZipArchive::new(Cursor::new(pptx_bytes)).expect("valid ZIP archive");
    let mut all_entries: HashSet<String> = HashSet::new();
    let mut xml_contents: HashMap<String, String> = HashMap::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).expect("zip entry");
        let name = file.name().to_string();
        all_entries.insert(name.clone());
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).expect("read zip entry");
        if name.ends_with(".xml") || name.ends_with(".rels") || name.ends_with(".svg") {
            let text = String::from_utf8(buf).expect("UTF-8 XML/SVG entry");
            assert_xml_well_formed(&name, &text);
            xml_contents.insert(name, text);
        } else {
            // Ensure zero WebP/AVIF leaked into ppt/media/
            assert!(
                !name.ends_with(".webp") && !name.ends_with(".avif"),
                "WebP/AVIF must never leak into ppt/media/: {name}"
            );
        }
    }

    for required in [
        "[Content_Types].xml",
        "_rels/.rels",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
        "ppt/slideMasters/slideMaster1.xml",
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        "ppt/slideLayouts/slideLayout1.xml",
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        "ppt/theme/theme1.xml",
        "ppt/slides/slide1.xml",
        "ppt/slides/_rels/slide1.xml.rels",
    ] {
        assert!(
            all_entries.contains(required),
            "missing required OPC part: {required}"
        );
    }

    // Verify every `r:id` and `r:embed` referenced in slide*.xml exists in its corresponding `.rels`,
    // and every `.rels` Target resolves to an actual entry inside the ZIP archive.
    for (entry_name, xml) in &xml_contents {
        if entry_name.ends_with(".rels") {
            let base_dir = rels_base_dir(entry_name);
            let targets = extract_rel_targets(xml);
            for (_rid, target) in targets {
                let resolved = resolve_opc_path(&base_dir, &target);
                assert!(
                    all_entries.contains(&resolved),
                    "dangling relationship in {entry_name}: target '{target}' -> '{resolved}' missing in ZIP"
                );
            }
        }
    }

    for (entry_name, xml) in &xml_contents {
        if entry_name.starts_with("ppt/slides/slide") && entry_name.ends_with(".xml") {
            let file_stem = entry_name.strip_prefix("ppt/slides/").unwrap();
            let rels_path = format!("ppt/slides/_rels/{file_stem}.rels");
            let rels_xml = xml_contents.get(&rels_path).expect("slide rels exist");
            let rel_ids: HashSet<String> = extract_rel_targets(rels_xml)
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            for referenced in extract_referenced_rids(xml) {
                assert!(
                    rel_ids.contains(&referenced),
                    "dangling rId '{referenced}' in {entry_name} not present in {rels_path}"
                );
            }
        }
    }

    xml_contents
}

fn assert_xml_well_formed(name: &str, xml: &str) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().check_end_names = true;
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut root_count = 0usize;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                if depth == 0 {
                    root_count += 1;
                }
                depth += 1;
                for attr in e.attributes() {
                    if let Err(err) = attr {
                        panic!("invalid/duplicate XML attribute in {name}: {err}\nXML:\n{xml}");
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                if depth == 0 {
                    root_count += 1;
                }
                for attr in e.attributes() {
                    if let Err(err) = attr {
                        panic!("invalid/duplicate XML attribute in {name}: {err}\nXML:\n{xml}");
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(_) => {}
            Err(e) => panic!("malformed XML in {name}: {e}\nXML:\n{xml}"),
        }
        buf.clear();
    }
    assert_eq!(
        root_count, 1,
        "XML document {name} must have exactly 1 root element (found {root_count})\nXML:\n{xml}"
    );
    assert_eq!(depth, 0, "unclosed XML tags in {name}\nXML:\n{xml}");
}

fn rels_base_dir(rels_path: &str) -> String {
    if rels_path == "_rels/.rels" {
        return String::new();
    }
    if let Some((prefix, _)) = rels_path.split_once("_rels/") {
        prefix.trim_end_matches('/').to_string()
    } else {
        String::new()
    }
}

fn resolve_opc_path(base_dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if base_dir.is_empty() {
        Vec::new()
    } else {
        base_dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn extract_rel_targets(rels_xml: &str) -> Vec<(String, String)> {
    let mut reader = Reader::from_str(rels_xml);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"Relationship" => {
                let mut id = String::new();
                let mut target = String::new();
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"Id" {
                        id = String::from_utf8_lossy(&attr.value).to_string();
                    } else if attr.key.as_ref() == b"Target" {
                        target = String::from_utf8_lossy(&attr.value).to_string();
                    }
                }
                if !id.is_empty() && !target.is_empty() {
                    out.push((id, target));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("rels parse error: {e}"),
        }
        buf.clear();
    }
    out
}

fn extract_referenced_rids(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"r:embed" || attr.key.as_ref() == b"r:id" {
                        out.push(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("xml scan error: {e}"),
        }
        buf.clear();
    }
    out
}

#[test]
fn ut_07_3_01_emu_conversion_clamping_and_text_width_buffer() {
    assert_eq!(SLIDE_W_EMU, 12_192_000);
    assert_eq!(SLIDE_H_EMU, 6_858_000);
    assert_eq!(EMU_PER_PX, 6_350);

    let full = px_rect_to_emu(0.0, 0.0, 1920.0, 1080.0).expect("valid full slide");
    assert_eq!(full.x, 0);
    assert_eq!(full.y, 0);
    assert_eq!(full.cx, SLIDE_W_EMU);
    assert_eq!(full.cy, SLIDE_H_EMU);

    // Edge case 15: NaN, Infinity, zero/negative size, and off-canvas (-9999px) rejected cleanly
    assert!(px_rect_to_emu(f64::NAN, 0.0, 100.0, 50.0).is_none());
    assert!(px_rect_to_emu(0.0, f64::INFINITY, 100.0, 50.0).is_none());
    assert!(px_rect_to_emu(10.0, 10.0, 0.0, 50.0).is_none());
    assert!(px_rect_to_emu(10.0, 10.0, 100.0, -5.0).is_none());
    assert!(px_rect_to_emu(-9999.0, 20.0, 200.0, 50.0).is_none());

    // Partial edge overflow clamps safely without negative x or exceeding slide edge
    let clamped = px_rect_to_emu(-40.0, 1000.0, 200.0, 200.0).expect("partially visible clamps");
    assert_eq!(clamped.x, 0);
    assert!(clamped.x + clamped.cx <= SLIDE_W_EMU);
    assert!(clamped.y + clamped.cy <= SLIDE_H_EMU);

    // +4% text width safety buffer
    let box_rect = px_rect_to_emu(100.0, 100.0, 500.0, 60.0).unwrap();
    let buffered = apply_text_width_buffer(box_rect, None);
    assert_eq!(buffered.cx, ((box_rect.cx as f64) * 1.04).round() as i64);
    assert_eq!(font_size_px_to_ooxml(16.0), 1200);
}

#[test]
fn ut_07_3_02_xml_sanitizer_unicode_nfc_and_vietnamese_filename_slugify() {
    // Edge cases 1 & 2: Invalid XML 1.0 control chars removed while keeping tabs/newlines
    let dirty = "Hello\x00\x01\x08\x0B\x0C\x1F & <World> \"Quoted\"\tLine2\n";
    let clean = sanitize_xml_text(dirty);
    assert!(!clean.contains('\x00'));
    assert!(!clean.contains('\x0B'));
    assert!(!clean.contains('\x1F'));
    assert!(clean.contains("& <World> \"Quoted\"\tLine2\n"));

    // Edge case 3: macOS decomposed Vietnamese NFD -> normalized NFC
    let nfd_vi: String = "Tiếng Việt đẹp chuẩn".nfd().collect();
    let normalized = sanitize_xml_text(&nfd_vi);
    let expected_nfc: String = "Tiếng Việt đẹp chuẩn".nfc().collect();
    assert_eq!(normalized, expected_nfc);

    // Edge case 18: Vietnamese transliteration + path traversal strip
    assert_eq!(
        slugify_pptx_filename("../../Kế hoạch Quý 4 — Đội ngũ!"),
        "ke-hoach-quy-4-doi-ngu.pptx"
    );
    assert_eq!(slugify_pptx_filename("../../../"), "presentation.pptx");
}

#[test]
fn ut_07_3_03_color_shadow_gradient_and_border_radius_algorithms() {
    // Edge case 5: hex, rgba, oklch, transparent -> 6-hex sRGB + alpha (zero 8-digit hex/oklch leak)
    assert_eq!(parse_css_color("#0f8").unwrap().hex, "00FF88");
    let rgba = parse_css_color("rgba(37, 99, 235, 0.5)").unwrap();
    assert_eq!(rgba.hex, "2563EB");
    assert_eq!(rgba.alpha_ooxml, 50_000);
    let hex8 = parse_css_color("#2563eb80").unwrap();
    assert_eq!(hex8.hex.len(), 6);
    assert!(hex8.alpha_ooxml > 49_000 && hex8.alpha_ooxml < 51_000);
    let oklch = parse_css_color("oklch(0.62 0.21 255 / 0.8)").unwrap();
    assert_eq!(oklch.hex.len(), 6);
    assert_eq!(oklch.alpha_ooxml, 80_000);
    assert!(parse_css_color("transparent").unwrap().is_transparent());

    // Algorithm 3: Cartesian -> Polar shadow & CSS -> OOXML gradient angle
    let sh =
        css_box_shadow_to_ooxml(0.0, 10.0, 20.0, PptxColor::with_alpha("000000", 0.3)).unwrap();
    assert_eq!(sh.dist_emu, 10 * EMU_PER_PX);
    assert_eq!(sh.dir_ooxml, 90 * 60_000); // pointing down (+Y) = 90 deg
    assert_eq!(sh.blur_rad_emu, 20 * EMU_PER_PX);

    assert_eq!(css_linear_gradient_deg_to_ooxml(90.0), 0); // CSS 90deg (to right) -> OOXML 0
    assert_eq!(css_linear_gradient_deg_to_ooxml(180.0), 90 * 60_000); // CSS 180deg (to bottom) -> OOXML 90deg
    assert_eq!(css_linear_gradient_deg_to_ooxml(0.0), 270 * 60_000); // CSS 0deg (to top) -> OOXML 270deg
    let grad =
        parse_css_linear_gradient("linear-gradient(135deg, #2563eb 0%, #10b981 100%)").unwrap();
    assert_eq!(grad.ang_ooxml, 45 * 60_000);
    assert_eq!(grad.stops.len(), 2);

    // Edge case 7: circular avatar -> ellipse preset
    assert_eq!(
        border_radius_to_geom(0.0, 100.0, 100.0, true),
        PptxShapeGeom::Ellipse
    );
    assert!(matches!(
        border_radius_to_geom(12.0, 200.0, 100.0, false),
        PptxShapeGeom::RoundRect { .. }
    ));
}

#[test]
fn ut_07_3_04_table_2d_merge_grid_colspan_rowspan_and_ooxml_roundtrip() {
    // Edge case 4: colspan + rowspan MUST emit dummy hMerge / vMerge cells
    let rect = px_rect_to_emu(100.0, 200.0, 1200.0, 300.0).unwrap();
    let grid = PptxTableGrid::from_raw_rows(
        &[
            vec![
                PptxTableRawCell {
                    text: "Quarter".into(),
                    colspan: 1,
                    rowspan: 2,
                    is_header: true,
                    fill: Some(PptxColor::opaque("F1F5F9")),
                    text_color: PptxColor::opaque("0F172A"),
                    font_size_px: 16.0,
                    align: PptxTextAlign::Left,
                },
                PptxTableRawCell {
                    text: "Revenue & Margin".into(),
                    colspan: 2,
                    rowspan: 1,
                    is_header: true,
                    fill: Some(PptxColor::opaque("F1F5F9")),
                    text_color: PptxColor::opaque("0F172A"),
                    font_size_px: 16.0,
                    align: PptxTextAlign::Center,
                },
            ],
            vec![
                PptxTableRawCell {
                    text: "ARR".into(),
                    colspan: 1,
                    rowspan: 1,
                    is_header: false,
                    fill: None,
                    text_color: PptxColor::opaque("0F172A"),
                    font_size_px: 14.0,
                    align: PptxTextAlign::Right,
                },
                PptxTableRawCell {
                    text: "GM%".into(),
                    colspan: 1,
                    rowspan: 1,
                    is_header: false,
                    fill: None,
                    text_color: PptxColor::opaque("0F172A"),
                    font_size_px: 14.0,
                    align: PptxTextAlign::Right,
                },
            ],
        ],
        rect,
    );
    assert_eq!(grid.rows.len(), 2);
    assert_eq!(grid.rows[0].len(), 3);
    assert_eq!(grid.rows[1].len(), 3);
    assert!(matches!(grid.rows[0][2], PptxTableCellKind::HMerge));
    assert!(matches!(
        grid.rows[1][0],
        PptxTableCellKind::VMerge { grid_span: 1 }
    ));

    let html = r#"
    <section data-slide="1" data-transition="morph">
      <h1>Báo cáo Tài chính &amp; Tăng trưởng &lt;Q4&gt;</h1>
      <table>
        <tr><th rowspan="2">Quý</th><th colspan="2">Hiệu suất</th></tr>
        <tr><td>Doanh thu</td><td>Biên lợi nhuận</td></tr>
      </table>
    </section>"#;
    let bundle = build_pptx_deck(
        &PptxExportInput {
            title: "Báo cáo Quý 4".into(),
            html: html.into(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: Some("vi-VN".into()),
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("build pptx");

    let xmls = verify_ooxml_zip_integrity(&bundle.pptx_bytes);
    let slide1 = xmls.get("ppt/slides/slide1.xml").unwrap();
    assert!(slide1.contains("gridSpan=\"2\""));
    assert!(slide1.contains("rowSpan=\"2\""));
    assert!(slide1.contains("hMerge=\"1\""));
    assert!(slide1.contains("vMerge=\"1\""));
    // Morph transition wrapped in mc:AlternateContent with p:fade fallback
    assert!(slide1.contains("mc:AlternateContent"));
    assert!(slide1.contains("p159:morph"));
    assert!(slide1.contains("<p:fade/>"));
    // Dual-font + normAutofit
    assert!(slide1.contains("<a:latin"));
    assert!(slide1.contains("<a:ea"));
    assert!(slide1.contains("<a:cs"));
    assert!(slide1.contains("<a:normAutofit/>"));
}

#[test]
fn ut_07_3_05_dual_blip_svg_sha256_media_dedup_ssrf_block_and_fidelity_warnings() {
    let logo_png = encode_solid_png(16, 16, &PptxColor::opaque("2563EB"));
    let logo_data_uri = format!("data:image/png;base64,{}", encode_base64(&logo_png));
    let webp_data_uri =
        "data:image/webp;base64,UklGRh4AAABXRUJQVlA4TBEAAAAvAAAAAAfQ//73v/+BiOh/AAA=";

    let nul = '\x00';
    let html = format!(
        r##"
        <style>
          @keyframes floatUp {{ from {{ opacity: 0; }} to {{ opacity: 1; }} }}
          .btn:hover {{ transform: scale(1.05); }}
        </style>
        <section data-slide="1" data-transition="fade">
          <h1 style="font-family: 'MyCustomBrandDisplay', sans-serif; color: oklch(0.55 0.2 260);">日本語とTiếng Việt{nul}</h1>
          <div style="backdrop-filter: blur(16px);">Glass Card</div>
          <canvas width="400" height="200"></canvas>
          <svg width="120" height="80" viewBox="0 0 120 80"><rect width="120" height="80" fill="#2563eb"/></svg>
          <img src="{logo_data_uri}" style="border-radius: 50%;" width="80" height="80" alt="Logo"/>
          <img src="https://evil.example.com/ssrf.png" alt="Blocked"/>
          <img src="file:///etc/passwd" alt="LocalFileBlocked"/>
          <img src="{webp_data_uri}" width="64" height="64" alt="WebP"/>
        </section>
        <section data-slide="2" data-transition="push">
          <h2>Slide 2 Repeated Logo</h2>
          <img src="{logo_data_uri}" width="80" height="80" alt="Logo Again"/>
        </section>
        "##
    );

    let bundle = build_pptx_deck(
        &PptxExportInput {
            title: "Fidelity & Dedup Deck".into(),
            html,
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: Some("ja-JP".into()),
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("build pptx");

    assert_eq!(bundle.slide_count, 2);
    assert!(bundle.used_static_fallback);
    assert_eq!(bundle.engine, "rust-ooxml-static-fallback");

    let codes: Vec<PptxFidelityWarningCode> =
        bundle.fidelity_warnings.iter().map(|w| w.code).collect();
    assert!(codes.contains(&PptxFidelityWarningCode::StaticLayoutFallback));
    assert!(codes.contains(&PptxFidelityWarningCode::BackdropFilterRasterized));
    assert!(codes.contains(&PptxFidelityWarningCode::CssAnimationOmitted));
    assert!(codes.contains(&PptxFidelityWarningCode::HoverStateOmitted));
    assert!(codes.contains(&PptxFidelityWarningCode::CanvasOr3dRasterized));
    assert!(codes.contains(&PptxFidelityWarningCode::CustomFontFallback));
    assert!(codes.contains(&PptxFidelityWarningCode::ExternalMediaBlocked));

    let xmls = verify_ooxml_zip_integrity(&bundle.pptx_bytes);
    let slide1 = xmls.get("ppt/slides/slide1.xml").unwrap();
    let slide1_rels = xmls.get("ppt/slides/_rels/slide1.xml.rels").unwrap();
    let slide2_rels = xmls.get("ppt/slides/_rels/slide2.xml.rels").unwrap();

    // Dual-Blip SVG + PNG surrogate
    assert!(slide1.contains("a16:svgBlip"));
    assert!(slide1.contains("{96DAC541-7B7A-43D3-8B79-37D633B846F1}"));
    // Circular avatar uses ellipse mask
    assert!(slide1.contains("prst=\"ellipse\""));
    // Zero oklch() leak into slide XML
    assert!(!slide1.contains("oklch("));
    // Zero external URLs in slide rels
    assert!(!slide1_rels.contains("evil.example.com"));
    assert!(!slide1_rels.contains("/etc/passwd"));

    // Logo PNG deduplicated across Slide 1 & Slide 2 (same SHA-256 media filename!)
    let s1_targets: HashSet<String> = extract_rel_targets(slide1_rels)
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    let s2_targets: HashSet<String> = extract_rel_targets(slide2_rels)
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    let shared_media: Vec<&String> = s1_targets
        .intersection(&s2_targets)
        .filter(|t| t.contains("../media/media_"))
        .collect();
    assert_eq!(
        shared_media.len(),
        1,
        "logo PNG must be deduplicated across slides"
    );
}

#[test]
fn ut_07_3_06_cdp_snapshot_shape_budget_visual_exact_empty_malformed_and_25mib_cap() {
    let pool = PooledCdpEvaluator::new(None);

    // 1. Live CDP snapshot with > MAX_SHAPES_PER_SLIDE enforces shape budget cleanly
    let mut elements = Vec::new();
    for i in 0..(MAX_SHAPES_PER_SLIDE + 10) {
        elements.push(CdpElementSnapshot {
            tag: "div".into(),
            label: Some(format!("card-{i}")),
            x: (i % 10) as f64 * 100.0,
            y: (i / 10) as f64 * 20.0,
            width: 80.0,
            height: 18.0,
            z_index: 1,
            paint_order: i,
            opacity: 1.0,
            visible: true,
            text: None,
            background_color: Some("#2563eb".into()),
            background_image: None,
            border_color: None,
            border_width_px: 0.0,
            border_radius_px: 4.0,
            is_circle_percent: false,
            box_shadow: None,
            color: None,
            font_family: None,
            font_size_px: 14.0,
            font_weight: 400,
            italic: false,
            text_align: None,
            padding_px: [0.0; 4],
            has_backdrop_filter: false,
            has_mix_blend_mode: false,
            has_css_animation: false,
            has_hover_state: false,
            is_canvas_or_3d: false,
            svg_markup: None,
            svg_node_count: 0,
            svg_has_filter: false,
            image_src: None,
            raster_png_base64: None,
            table_rows: Vec::new(),
        });
    }
    let cdp_bundle = build_pptx_deck(
        &PptxExportInput {
            title: "CDP Budget Test".into(),
            html: String::new(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: Some("en-US".into()),
            slide_count_hint: None,
            cdp_snapshot: Some(CdpDeckSnapshot {
                slides: vec![CdpSlideSnapshot {
                    index: 1,
                    transition: Some("wipe".into()),
                    background_color: Some("#ffffff".into()),
                    background_image: None,
                    full_slide_png_base64: None,
                    elements,
                }],
            }),
            max_bytes: None,
        },
        Some(&pool),
    )
    .expect("cdp build succeeds");
    assert_eq!(cdp_bundle.engine, "rust-ooxml-cdp");
    assert!(!cdp_bundle.used_static_fallback);
    assert!(cdp_bundle
        .fidelity_warnings
        .iter()
        .any(|w| w.code == PptxFidelityWarningCode::ShapeBudgetRasterized));
    verify_ooxml_zip_integrity(&cdp_bundle.pptx_bytes);

    // 2. Visual-Exact mode with cdp_snapshot: None preserves real slide content (non-blank) + warning
    let ve_bundle = build_pptx_deck(
        &PptxExportInput {
            title: "Visual Exact Deck".into(),
            html: "<section data-slide=\"1\"><h1>Hero</h1><p>Real presentation body</p></section>"
                .into(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::VisualExact,
            locale: Some("en-US".into()),
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("visual_exact succeeds");
    assert_eq!(ve_bundle.export_mode, PptxExportMode::VisualExact);
    assert!(ve_bundle
        .fidelity_warnings
        .iter()
        .any(|w| w.code == PptxFidelityWarningCode::VisualExactRasterUnavailable));
    let ve_xmls = verify_ooxml_zip_integrity(&ve_bundle.pptx_bytes);
    let ve_slide1 = ve_xmls.get("ppt/slides/slide1.xml").unwrap();
    assert!(ve_slide1.contains("Hero"));
    assert!(ve_slide1.contains("Real presentation body"));

    // 3. Edge case 14: Empty or unclosed malformed HTML never panics and yields valid blank slide
    let malformed = build_pptx_deck(
        &PptxExportInput {
            title: "Broken HTML".into(),
            html: "<section><h1>Unclosed heading<div><table><tr><td>orphan".into(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: None,
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("malformed html heals cleanly");
    assert_eq!(malformed.slide_count, 1);
    verify_ooxml_zip_integrity(&malformed.pptx_bytes);

    // 4. Edge case 13: Size limit guard returns typed AssetSizeExceeded
    let too_big = build_pptx_deck(
        &PptxExportInput {
            title: "Size Cap Test".into(),
            html: "<section><h1>Hello</h1></section>".into(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: None,
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: Some(128),
        },
        None,
    );
    assert!(matches!(
        too_big,
        Err(ExportBundlerError::AssetSizeExceeded { .. })
    ));
}

#[test]
fn ut_07_3_07_adv01_stage1_media_pool_dedup_and_cumulative_cap() {
    // A single ~48 KiB PNG referenced 500 times across 5 slides (would be ~24 MiB without Stage-1 dedup)
    let large_png = encode_solid_png(120, 100, &PptxColor::opaque("2563EB"));
    assert!(large_png.len() > 40_000, "fixture PNG should be >40 KiB");
    let data_uri = format!("data:image/png;base64,{}", encode_base64(&large_png));

    let mut cas_assets = BTreeMap::new();
    cas_assets.insert("cas://hero-logo".to_string(), data_uri);

    let mut html = String::new();
    for s in 1..=5 {
        html.push_str(&format!("<section data-slide=\"{s}\">"));
        for _ in 0..100 {
            html.push_str("<img src=\"cas://hero-logo\" width=\"64\" height=\"64\" alt=\"Logo\"/>");
        }
        html.push_str("</section>");
    }

    // Tight 256 KiB cap: 500x clones would blow past 256 KiB by ~90x, but Stage-1 dedup keeps 1 buffer!
    let input = PptxExportInput {
        title: "ADV-01 Dedup Deck".into(),
        html,
        inline_css: None,
        cas_assets,
        export_mode: PptxExportMode::Editable,
        locale: Some("en-US".into()),
        slide_count_hint: None,
        cdp_snapshot: None,
        max_bytes: Some(256 * 1024),
    };
    let deck = PptxLayoutExtractor::extract_deck(&input, None).expect("stage 1 dedup stays in cap");
    assert_eq!(
        deck.media_pool.len(),
        1,
        "500 identical references must be stored ONCE in Stage-1 media_pool"
    );

    let bundle = emit_pptx_bundle(&deck, input.max_bytes).expect("emit succeeds cleanly");
    assert_eq!(bundle.deduplicated_media_count, 1);
    verify_ooxml_zip_integrity(&bundle.pptx_bytes);

    // Exceeding cumulative unique media cap MUST fail closed in Stage 1 with typed AssetSizeExceeded
    let png1 = encode_solid_png(120, 100, &PptxColor::opaque("EF4444"));
    let png2 = encode_solid_png(120, 100, &PptxColor::opaque("10B981"));
    let png3 = encode_solid_png(120, 100, &PptxColor::opaque("F59E0B"));
    let mut cap_assets = BTreeMap::new();
    cap_assets.insert(
        "k1".into(),
        format!("data:image/png;base64,{}", encode_base64(&png1)),
    );
    cap_assets.insert(
        "k2".into(),
        format!("data:image/png;base64,{}", encode_base64(&png2)),
    );
    cap_assets.insert(
        "k3".into(),
        format!("data:image/png;base64,{}", encode_base64(&png3)),
    );
    let over_cap_input = PptxExportInput {
        title: "ADV-01 Over Cap".into(),
        html: "<section><img src=\"k1\"/><img src=\"k2\"/><img src=\"k3\"/></section>".into(),
        inline_css: None,
        cas_assets: cap_assets,
        export_mode: PptxExportMode::Editable,
        locale: None,
        slide_count_hint: None,
        cdp_snapshot: None,
        max_bytes: Some(100_000),
    };
    let stage1_err = PptxLayoutExtractor::extract_deck(&over_cap_input, None);
    assert!(
        matches!(
            stage1_err,
            Err(ExportBundlerError::AssetSizeExceeded { .. })
        ),
        "Stage 1 must fail closed when unique decoded media exceeds max_bytes: {stage1_err:?}"
    );
}

#[test]
fn ut_07_3_08_adv02_svg_sanitizer_inline_and_data_uri_xxe_script_ssrf_and_complexity() {
    let malicious_data_svg = r##"<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" onload="alert('xss')" width="100" height="80"><script>alert(1)</script><image xlink:href="https://evil.example/a.png" width="50" height="50"/><defs><linearGradient id="localGrad"><stop offset="0%" stop-color="#2563eb"/></linearGradient></defs><use xlink:href="#localGrad"/><rect style="fill: url(https://evil.example/css.png)" width="80" height="60" fill="#2563eb" onclick="steal()"/></svg>"##;
    let data_svg_uri = format!(
        "data:image/svg+xml;base64,{}",
        encode_base64(malicious_data_svg.as_bytes())
    );

    let complex_data_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><defs><filter id="blur"><feGaussianBlur stdDeviation="5"/></filter></defs><rect width="80" height="80" filter="url(#blur)"/></svg>"##;
    let complex_svg_uri = format!(
        "data:image/svg+xml;base64,{}",
        encode_base64(complex_data_svg.as_bytes())
    );

    let html = format!(
        r##"
        <section data-slide="1">
          <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" onload="evilInline()" width="120" height="80">
            <script>window.pwn = true;</script>
            <image xlink:href="https://evil.example/a.png" width="40" height="40"/>
            <defs><rect id="box1" width="20" height="20"/></defs>
            <use xlink:href="#box1"/>
            <rect width="120" height="80" fill="#10b981" onmouseover="bad()"/>
          </svg>
          <img src="{data_svg_uri}" width="100" height="80" alt="Data SVG"/>
          <img src="{complex_svg_uri}" width="80" height="80" alt="Complex Data SVG"/>
        </section>
        "##
    );

    let bundle = build_pptx_deck(
        &PptxExportInput {
            title: "ADV-02 SVG Hardening".into(),
            html,
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: Some("en-US".into()),
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("sanitized SVG deck builds cleanly");

    assert!(bundle
        .fidelity_warnings
        .iter()
        .any(|w| w.code == PptxFidelityWarningCode::ExternalMediaBlocked));
    assert!(bundle
        .fidelity_warnings
        .iter()
        .any(|w| w.code == PptxFidelityWarningCode::ComplexSvgRasterized));

    // verify_ooxml_zip_integrity now XML-parses EVERY ppt/media/*.svg entry as well!
    let xmls = verify_ooxml_zip_integrity(&bundle.pptx_bytes);
    let svg_entries: Vec<(&String, &String)> = xmls
        .iter()
        .filter(|(name, _)| name.starts_with("ppt/media/") && name.ends_with(".svg"))
        .collect();
    assert_eq!(
        svg_entries.len(),
        2,
        "inline + simple data SVG emitted, complex data SVG rasterized to PNG"
    );
    for (name, svg) in svg_entries {
        let lower = svg.to_ascii_lowercase();
        assert!(!lower.contains("<script"), "{name} leaked <script>: {svg}");
        assert!(!lower.contains("onload"), "{name} leaked onload: {svg}");
        assert!(!lower.contains("onclick"), "{name} leaked onclick: {svg}");
        assert!(
            !lower.contains("onmouseover"),
            "{name} leaked onmouseover: {svg}"
        );
        assert!(!lower.contains("<!doctype"), "{name} leaked DOCTYPE: {svg}");
        assert!(!lower.contains("<!entity"), "{name} leaked ENTITY: {svg}");
        assert!(
            !lower.contains("evil.example"),
            "{name} leaked external URL: {svg}"
        );
        assert!(
            !lower.contains("/etc/passwd"),
            "{name} leaked file:// XXE: {svg}"
        );
        // Preserves internal xlink:href="#..." AND declares xmlns:xlink cleanly
        assert!(
            svg.contains("xlink:href=\"#"),
            "{name} should preserve internal xlink:href: {svg}"
        );
        assert!(
            svg.contains("xmlns:xlink="),
            "{name} should preserve xmlns:xlink: {svg}"
        );
    }
}

#[test]
fn ut_07_3_09_adv03_slide_count_limit_and_uncompressed_xml_zip_bomb() {
    // 1. 201 <section> slides must fail closed with typed InvalidInput error
    let mut too_many_slides = String::new();
    for i in 1..=(MAX_SLIDES_PER_DECK + 1) {
        too_many_slides.push_str(&format!("<section><h1>Slide {i}</h1></section>"));
    }
    let res_slides = build_pptx_deck(
        &PptxExportInput {
            title: "Too Many Slides".into(),
            html: too_many_slides,
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: None,
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    );
    assert!(
        matches!(res_slides, Err(ExportBundlerError::InvalidInput(_))),
        "deck with 201 slides must fail closed: {res_slides:?}"
    );

    // 2. Repetitive text compresses well under 16 KiB in deflate, but explodes uncompressed XML > 32 KiB
    let repeated_block = "A".repeat(4000);
    let mut bomb_html = String::new();
    for i in 1..=12 {
        bomb_html.push_str(&format!("<section><p>{i}-{repeated_block}</p></section>"));
    }
    let res_uncompressed = build_pptx_deck(
        &PptxExportInput {
            title: "XML Zip Bomb".into(),
            html: bomb_html,
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: None,
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: Some(16_384),
        },
        None,
    );
    assert!(
        matches!(
            res_uncompressed,
            Err(ExportBundlerError::AssetSizeExceeded { .. })
        ),
        "uncompressed XML exceeding 2x max_bytes must fail closed: {res_uncompressed:?}"
    );
}

#[test]
fn ut_07_3_10_adv04_visual_exact_non_blank_fallback_and_real_cdp_png() {
    // 1. VisualExact with cdp_snapshot having NO full_slide_png_base64 preserves CDP elements
    let cdp_no_png = build_pptx_deck(
        &PptxExportInput {
            title: "CDP Visual Exact Fallback".into(),
            html: String::new(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::VisualExact,
            locale: Some("en-US".into()),
            slide_count_hint: None,
            cdp_snapshot: Some(CdpDeckSnapshot {
                slides: vec![CdpSlideSnapshot {
                    index: 1,
                    transition: None,
                    background_color: Some("#ffffff".into()),
                    background_image: None,
                    full_slide_png_base64: None,
                    elements: vec![CdpElementSnapshot {
                        tag: "h1".into(),
                        label: Some("headline".into()),
                        x: 100.0,
                        y: 80.0,
                        width: 1000.0,
                        height: 120.0,
                        z_index: 1,
                        paint_order: 0,
                        opacity: 1.0,
                        visible: true,
                        text: Some("Non-Blank Preserved Headline".into()),
                        background_color: None,
                        background_image: None,
                        border_color: None,
                        border_width_px: 0.0,
                        border_radius_px: 0.0,
                        is_circle_percent: false,
                        box_shadow: None,
                        color: Some("#0f172a".into()),
                        font_family: Some("Inter".into()),
                        font_size_px: 48.0,
                        font_weight: 700,
                        italic: false,
                        text_align: Some("left".into()),
                        padding_px: [0.0; 4],
                        has_backdrop_filter: false,
                        has_mix_blend_mode: false,
                        has_css_animation: false,
                        has_hover_state: false,
                        is_canvas_or_3d: false,
                        svg_markup: None,
                        svg_node_count: 0,
                        svg_has_filter: false,
                        image_src: None,
                        raster_png_base64: None,
                        table_rows: Vec::new(),
                    }],
                }],
            }),
            max_bytes: None,
        },
        None,
    )
    .expect("visual_exact fallback builds non-blank deck");

    assert!(cdp_no_png
        .fidelity_warnings
        .iter()
        .any(|w| w.code == PptxFidelityWarningCode::VisualExactRasterUnavailable));
    let fallback_xmls = verify_ooxml_zip_integrity(&cdp_no_png.pptx_bytes);
    let fallback_slide1 = fallback_xmls.get("ppt/slides/slide1.xml").unwrap();
    assert!(fallback_slide1.contains("Non-Blank Preserved Headline"));

    // 2. VisualExact with VALID full_slide_png_base64 embeds the full-slide raster PNG as designed
    let real_png = encode_solid_png(32, 18, &PptxColor::opaque("1E3A8A"));
    let cdp_with_png = build_pptx_deck(
        &PptxExportInput {
            title: "CDP Visual Exact Raster".into(),
            html: String::new(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::VisualExact,
            locale: Some("en-US".into()),
            slide_count_hint: None,
            cdp_snapshot: Some(CdpDeckSnapshot {
                slides: vec![CdpSlideSnapshot {
                    index: 1,
                    transition: None,
                    background_color: Some("#ffffff".into()),
                    background_image: None,
                    full_slide_png_base64: Some(encode_base64(&real_png)),
                    elements: Vec::new(),
                }],
            }),
            max_bytes: None,
        },
        None,
    )
    .expect("visual_exact with valid PNG succeeds");

    assert!(!cdp_with_png
        .fidelity_warnings
        .iter()
        .any(|w| w.code == PptxFidelityWarningCode::VisualExactRasterUnavailable));
    let png_xmls = verify_ooxml_zip_integrity(&cdp_with_png.pptx_bytes);
    let png_slide1 = png_xmls.get("ppt/slides/slide1.xml").unwrap();
    assert!(png_slide1.contains("<p:pic>"));
    assert!(png_slide1.contains("cx=\"12192000\" cy=\"6858000\""));
}

#[test]
fn ut_07_3_11_adv05_ecma376_2d_table_merge_and_malformed_overlap_clamping() {
    let rect = px_rect_to_emu(100.0, 100.0, 1200.0, 300.0).unwrap();

    // (a) 2x2 merged cell (colspan=2, rowspan=2) in a 2x3 table
    let html_2x2 = r#"
    <section>
      <table>
        <tr><th colspan="2" rowspan="2">2x2 Block</th><th>Col3</th></tr>
        <tr><td>Row2Col3</td></tr>
      </table>
    </section>"#;
    let bundle_2x2 = build_pptx_deck(
        &PptxExportInput {
            title: "2x2 Merge Table".into(),
            html: html_2x2.into(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: Some("en-US".into()),
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("2x2 table builds");
    let xmls_2x2 = verify_ooxml_zip_integrity(&bundle_2x2.pptx_bytes);
    let slide1 = xmls_2x2.get("ppt/slides/slide1.xml").unwrap();
    assert!(
        slide1.contains("<a:tc gridSpan=\"2\" rowSpan=\"2\">"),
        "origin cell must carry gridSpan=2 rowSpan=2: {slide1}"
    );
    assert!(
        slide1.contains("<a:tc gridSpan=\"2\" vMerge=\"1\">"),
        "row 2 col 1 must carry gridSpan=2 alongside vMerge=1 per ECMA-376: {slide1}"
    );
    assert!(
        slide1.contains("<a:tc hMerge=\"1\" vMerge=\"1\">"),
        "row 2 col 2 must carry both hMerge=1 and vMerge=1: {slide1}"
    );

    // (b) Malformed overlapping rowspan/colspan must NEVER overwrite VMerge with HMerge
    let overlapping_grid = PptxTableGrid::from_raw_rows(
        &[
            vec![
                PptxTableRawCell {
                    text: "A".into(),
                    colspan: 1,
                    rowspan: 1,
                    is_header: false,
                    fill: None,
                    text_color: PptxColor::opaque("0F172A"),
                    font_size_px: 14.0,
                    align: PptxTextAlign::Left,
                },
                PptxTableRawCell {
                    text: "B".into(),
                    colspan: 1,
                    rowspan: 2,
                    is_header: false,
                    fill: None,
                    text_color: PptxColor::opaque("0F172A"),
                    font_size_px: 14.0,
                    align: PptxTextAlign::Left,
                },
            ],
            vec![PptxTableRawCell {
                text: "OverlapAttempt".into(),
                colspan: 2, // collides with B's VMerge at (1, 1)!
                rowspan: 1,
                is_header: false,
                fill: None,
                text_color: PptxColor::opaque("0F172A"),
                font_size_px: 14.0,
                align: PptxTextAlign::Left,
            }],
        ],
        rect,
    );
    assert_eq!(overlapping_grid.rows.len(), 2);
    assert_eq!(overlapping_grid.rows[1].len(), 2);
    assert!(matches!(
        overlapping_grid.rows[1][0],
        PptxTableCellKind::Origin { grid_span: 1, .. }
    ));
    assert!(
        matches!(
            overlapping_grid.rows[1][1],
            PptxTableCellKind::VMerge { grid_span: 1 }
        ),
        "VMerge slot at (1,1) must NEVER be overwritten by overlapping colspan"
    );

    let overlap_html = r#"
    <section>
      <table>
        <tr><td>A</td><td rowspan="2">B</td></tr>
        <tr><td colspan="2">OverlapAttempt</td></tr>
      </table>
    </section>"#;
    let overlap_bundle = build_pptx_deck(
        &PptxExportInput {
            title: "Malformed Overlap Table".into(),
            html: overlap_html.into(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: None,
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("overlapping table heals cleanly");
    verify_ooxml_zip_integrity(&overlap_bundle.pptx_bytes);
}

#[test]
fn ut_07_3_12_adv06_element_label_truncation_utf8_safe_max_120() {
    // > 200 UTF-8 characters mixing Vietnamese diacritics + CJK + ASCII
    let long_font = format!(
        "PhôngChữThươngHiệuSiêuDài日本語フォント{}",
        "ĐẹpChuẩn漢字".repeat(25)
    );
    assert!(long_font.chars().count() > 200);

    let html = format!(
        "<section><h1 style=\"font-family: '{long_font}', sans-serif;\">Headline</h1></section>"
    );
    let bundle = build_pptx_deck(
        &PptxExportInput {
            title: "ADV-06 Label Truncation".into(),
            html,
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: Some("vi-VN".into()),
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("builds cleanly");

    let font_warn = bundle
        .fidelity_warnings
        .iter()
        .find(|w| w.code == PptxFidelityWarningCode::CustomFontFallback)
        .expect("custom font warning emitted");
    let label = font_warn.element_label.as_ref().expect("element_label set");
    assert!(
        label.chars().count() <= 120,
        "element_label chars {} must be <= 120",
        label.chars().count()
    );
    assert!(
        label.encode_utf16().count() <= 120,
        "element_label UTF-16 units {} must be <= 120 for Zod",
        label.encode_utf16().count()
    );
    assert!(label.ends_with('…'));
}

#[test]
fn ut_07_3_13_adv07_magic_byte_validation_and_media_store_format_key() {
    // 1. Spoofed PNG and JPEG data URIs with non-image bytes are rejected cleanly with warning
    let spoofed_png = format!("data:image/png;base64,{}", encode_base64(b"NOT_A_REAL_PNG"));
    let spoofed_jpg = format!(
        "data:image/jpeg;base64,{}",
        encode_base64(b"NOT_A_REAL_JPEG")
    );
    let html = format!(
        "<section><h1>Safe</h1><img src=\"{spoofed_png}\" alt=\"FakePng\"/><img src=\"{spoofed_jpg}\" alt=\"FakeJpg\"/></section>"
    );
    let bundle = build_pptx_deck(
        &PptxExportInput {
            title: "ADV-07 Spoofed Magic".into(),
            html,
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: None,
            slide_count_hint: None,
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("spoofed images rejected without crash");
    assert_eq!(
        bundle.deduplicated_media_count, 0,
        "spoofed PNG/JPEG bytes must never be embedded into ppt/media/"
    );
    assert!(bundle
        .fidelity_warnings
        .iter()
        .any(|w| w.code == PptxFidelityWarningCode::InvalidMediaRejected));
    verify_ooxml_zip_integrity(&bundle.pptx_bytes);

    // 2. Identical byte payload with distinct PptxMediaFormat MUST NOT collide in MediaStore
    let shared_bytes = encode_solid_png(8, 8, &PptxColor::opaque("2563EB"));
    let shared_hash = sha256_bytes(&shared_bytes);
    let rect = PptxRectEmu {
        x: 0,
        y: 0,
        cx: 600_000,
        cy: 400_000,
    };
    let deck = PptxDeckIr {
        title: "ADV-07 Format Key".into(),
        locale: "en-US".into(),
        export_mode: PptxExportMode::Editable,
        slides: vec![PptxSlideIr {
            index: 1,
            background: PptxFill::Solid(PptxColor::opaque("FFFFFF")),
            transition: PptxSlideTransition::None,
            nodes: vec![
                PptxNode::Image {
                    id: 2,
                    name: "AsPng".into(),
                    rect,
                    geom: PptxShapeGeom::Rect,
                    shadow: None,
                    media_index: 0,
                    alt_text: "png".into(),
                },
                PptxNode::Image {
                    id: 3,
                    name: "AsJpg".into(),
                    rect,
                    geom: PptxShapeGeom::Rect,
                    shadow: None,
                    media_index: 1,
                    alt_text: "jpg".into(),
                },
            ],
        }],
        media_pool: vec![
            PptxMediaAsset {
                sha256: shared_hash,
                format: PptxMediaFormat::Png,
                bytes: shared_bytes.clone(),
            },
            PptxMediaAsset {
                sha256: shared_hash,
                format: PptxMediaFormat::Jpeg,
                bytes: shared_bytes,
            },
        ],
        fidelity_warnings: Vec::new(),
        used_static_fallback: true,
        engine: "rust-ooxml-static-fallback".into(),
    };
    let fmt_bundle = emit_pptx_bundle(&deck, None).expect("distinct formats do not collide");
    assert_eq!(
        fmt_bundle.deduplicated_media_count, 2,
        "MediaStore must key by (sha256, format) so distinct formats never collide"
    );
    let xmls = verify_ooxml_zip_integrity(&fmt_bundle.pptx_bytes);
    let rels = xmls.get("ppt/slides/_rels/slide1.xml.rels").unwrap();
    let targets: Vec<String> = extract_rel_targets(rels)
        .into_iter()
        .map(|(_, t)| t)
        .filter(|t| t.starts_with("../media/media_"))
        .collect();
    assert_eq!(targets.len(), 2);
    assert!(targets.iter().any(|t| t.ends_with(".png")));
    assert!(targets.iter().any(|t| t.ends_with(".jpg")));
    for t in &targets {
        // media_<16 hex chars>.<ext>
        let fname = t.strip_prefix("../media/").unwrap();
        let stem = fname.split('.').next().unwrap();
        let hex_part = stem.strip_prefix("media_").unwrap();
        assert_eq!(
            hex_part.len(),
            16,
            "media filename must use 16 hex chars from SHA-256: {fname}"
        );
    }
}

#[test]
fn ut_07_3_14_r2_02_multi_root_svg_rejected_cleanly() {
    let html = r#"
      <section data-slide="1">
        <h1>Multi-root SVG Security Test</h1>
        <img src="data:image/svg+xml;utf8,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%2F%3E%3Crect%20width%3D%2210%22%20height%3D%2210%22%2F%3E" alt="bad-multi-root-1" />
        <img src="data:image/svg+xml;utf8,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%3E%3Ccircle%20r%3D%224%22%2F%3E%3C%2Fsvg%3E%3Crect%20width%3D%2210%22%20height%3D%2210%22%2F%3E" alt="bad-multi-root-2" />
      </section>
    "#;
    let out = build_pptx_deck(
        &PptxExportInput {
            title: "Multi-Root SVG Guard".into(),
            html: html.into(),
            inline_css: None,
            cas_assets: BTreeMap::new(),
            export_mode: PptxExportMode::Editable,
            locale: Some("en-US".into()),
            slide_count_hint: Some(1),
            cdp_snapshot: None,
            max_bytes: None,
        },
        None,
    )
    .expect("deck builds cleanly while rejecting multi-root SVG");

    let xmls = verify_ooxml_zip_integrity(&out.pptx_bytes);
    assert!(
        !xmls.keys().any(|k| k.ends_with(".svg")),
        "multi-root SVG payloads must never be embedded into ppt/media/*.svg"
    );
}
