// @story 07.3
// @implements AC-01, AC-02, AC-03
//! Pure-Rust ECMA-376 OOXML `.pptx` Emitter (`quick-xml::Writer` + `zip::ZipWriter`).
//! Zero Node.js, zero LibreOffice, zero raw XML string formatting of user text, deterministic
//! ZIP timestamps, SHA-256 media deduplication, Dual-Blip SVG+PNG support, and strict 25 MiB cap.

use std::collections::{BTreeSet, HashMap};
use std::io::{Cursor, Write};

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::export_bundler::{ExportBundlerError, MAX_BUNDLE_BYTES};
use crate::pptx_ir::{
    apply_text_width_buffer, font_size_px_to_ooxml, sanitize_xml_text, sha256_bytes,
    slugify_pptx_filename, PptxColor, PptxDeckIr, PptxExportMode, PptxFidelityWarning, PptxFill,
    PptxMediaAsset, PptxMediaFormat, PptxNode, PptxParagraph, PptxRectEmu, PptxShadow,
    PptxShapeGeom, PptxSlideIr, PptxSlideTransition, PptxTableCellKind, PptxTableGrid,
    DEFAULT_CS_FONT, MAX_SLIDES_PER_DECK, SLIDE_H_EMU, SLIDE_W_EMU,
};
use crate::pptx_layout::{PptxExportInput, PptxLayoutExtractor};
use crate::visual_qc::PooledCdpEvaluator;

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const NS_P159: &str = "http://schemas.microsoft.com/office/powerpoint/2015/09/main";
const NS_A16: &str = "http://schemas.microsoft.com/office/drawing/2016/SVG/main";
const NS_PKG_RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const NS_CONTENT_TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

const REL_OFFICE_DOC: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const REL_SLIDE_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
const REL_SLIDE_LAYOUT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const REL_THEME: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
const REL_SLIDE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

const SVG_BLIP_EXT_URI: &str = "{96DAC541-7B7A-43D3-8B79-37D633B846F1}";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxExportBundle {
    pub file_name: String,
    #[serde(skip)]
    pub pptx_bytes: Vec<u8>,
    pub pptx_base64: String,
    pub sha256: String,
    pub byte_size: usize,
    pub slide_count: usize,
    pub export_mode: PptxExportMode,
    pub engine: String,
    pub used_static_fallback: bool,
    pub fidelity_warnings: Vec<PptxFidelityWarning>,
    pub deduplicated_media_count: usize,
}

#[derive(Clone)]
struct MediaEntry {
    file_name: String,
    format: PptxMediaFormat,
    bytes: Vec<u8>,
}

struct MediaStore {
    by_hash: HashMap<([u8; 32], PptxMediaFormat), usize>,
    entries: Vec<MediaEntry>,
    total_bytes: usize,
}

impl MediaStore {
    fn new() -> Self {
        Self {
            by_hash: HashMap::new(),
            entries: Vec::new(),
            total_bytes: 0,
        }
    }

    fn insert(
        &mut self,
        bytes: &[u8],
        format: PptxMediaFormat,
        max_bytes: usize,
    ) -> Result<String, ExportBundlerError> {
        let hash = sha256_bytes(bytes);
        let key = (hash, format);
        if let Some(&idx) = self.by_hash.get(&key) {
            return Ok(self.entries[idx].file_name.clone());
        }
        let next_total = self.total_bytes.saturating_add(bytes.len());
        if next_total > max_bytes {
            return Err(ExportBundlerError::AssetSizeExceeded {
                actual_bytes: next_total,
                limit_bytes: max_bytes,
            });
        }
        let hash16 = format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7]
        );
        let file_name = format!("media_{hash16}.{}", format.extension());
        let idx = self.entries.len();
        self.entries.push(MediaEntry {
            file_name: file_name.clone(),
            format,
            bytes: bytes.to_vec(),
        });
        self.by_hash.insert(key, idx);
        self.total_bytes = next_total;
        Ok(file_name)
    }

    fn used_extensions(&self) -> BTreeSet<(&'static str, &'static str)> {
        self.entries
            .iter()
            .map(|e| (e.format.extension(), e.format.content_type()))
            .collect()
    }
}

/// High-level Built-in Native Rust PPTX builder combining Stage 1 layout extraction and Stage 3 OOXML emission.
pub fn build_pptx_deck(
    input: &PptxExportInput,
    cdp_pool: Option<&PooledCdpEvaluator>,
) -> Result<PptxExportBundle, ExportBundlerError> {
    let deck = PptxLayoutExtractor::extract_deck(input, cdp_pool)?;
    emit_pptx_bundle(&deck, input.max_bytes)
}

/// Emits a complete, valid ECMA-376 `.pptx` archive from a `PptxDeckIr`.
pub fn emit_pptx_bundle(
    deck: &PptxDeckIr,
    max_bytes_override: Option<usize>,
) -> Result<PptxExportBundle, ExportBundlerError> {
    if deck.slides.len() > MAX_SLIDES_PER_DECK {
        return Err(ExportBundlerError::InvalidInput(format!(
            "Presentation exceeds maximum allowed slide count of {MAX_SLIDES_PER_DECK} slides (found {})",
            deck.slides.len()
        )));
    }

    let max_bytes = max_bytes_override.unwrap_or(MAX_BUNDLE_BYTES);
    // Cumulative uncompressed XML + media cap tied to `max_bytes` to prevent deflate ZIP bombs.
    let max_uncompressed_bytes = max_bytes.saturating_mul(2);
    let mut media_store = MediaStore::new();

    // Pre-build each slide XML and its slide-level relationships so Content_Types knows all media extensions.
    let mut slide_xmls: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(deck.slides.len().max(1));
    let mut slide_xml_bytes = 0usize;
    for slide in &deck.slides {
        let (xml_bytes, rels_bytes) =
            build_slide_xml_and_rels(slide, &deck.media_pool, &mut media_store, max_bytes)?;
        slide_xml_bytes = slide_xml_bytes
            .saturating_add(xml_bytes.len())
            .saturating_add(rels_bytes.len());
        let running_uncompressed = slide_xml_bytes.saturating_add(media_store.total_bytes);
        if running_uncompressed > max_uncompressed_bytes {
            return Err(ExportBundlerError::AssetSizeExceeded {
                actual_bytes: running_uncompressed,
                limit_bytes: max_uncompressed_bytes,
            });
        }
        slide_xmls.push((xml_bytes, rels_bytes));
    }

    let slide_count = slide_xmls.len();
    let content_types = build_content_types_xml(slide_count, &media_store.used_extensions())?;
    let root_rels = build_rels_xml(&[("rId1", REL_OFFICE_DOC, "ppt/presentation.xml")])?;
    let presentation_xml = build_presentation_xml(slide_count)?;
    let presentation_rels = build_presentation_rels_xml(slide_count)?;
    let slide_master_xml = build_slide_master_xml()?;
    let slide_master_rels = build_rels_xml(&[
        ("rId1", REL_SLIDE_LAYOUT, "../slideLayouts/slideLayout1.xml"),
        ("rId2", REL_THEME, "../theme/theme1.xml"),
    ])?;
    let slide_layout_xml = build_slide_layout_xml()?;
    let slide_layout_rels =
        build_rels_xml(&[("rId1", REL_SLIDE_MASTER, "../slideMasters/slideMaster1.xml")])?;
    let theme_xml = build_theme_xml()?;

    let deterministic_dt = DateTime::from_date_and_time(2026, 1, 1, 0, 0, 0).unwrap_or_default();
    let zip_opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(deterministic_dt);

    let mut uncompressed_total = 0usize;
    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        write_zip_entry(
            &mut zip,
            "[Content_Types].xml",
            &content_types,
            zip_opts,
            &mut uncompressed_total,
            max_uncompressed_bytes,
        )?;
        write_zip_entry(
            &mut zip,
            "_rels/.rels",
            &root_rels,
            zip_opts,
            &mut uncompressed_total,
            max_uncompressed_bytes,
        )?;
        write_zip_entry(
            &mut zip,
            "ppt/presentation.xml",
            &presentation_xml,
            zip_opts,
            &mut uncompressed_total,
            max_uncompressed_bytes,
        )?;
        write_zip_entry(
            &mut zip,
            "ppt/_rels/presentation.xml.rels",
            &presentation_rels,
            zip_opts,
            &mut uncompressed_total,
            max_uncompressed_bytes,
        )?;
        write_zip_entry(
            &mut zip,
            "ppt/slideMasters/slideMaster1.xml",
            &slide_master_xml,
            zip_opts,
            &mut uncompressed_total,
            max_uncompressed_bytes,
        )?;
        write_zip_entry(
            &mut zip,
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            &slide_master_rels,
            zip_opts,
            &mut uncompressed_total,
            max_uncompressed_bytes,
        )?;
        write_zip_entry(
            &mut zip,
            "ppt/slideLayouts/slideLayout1.xml",
            &slide_layout_xml,
            zip_opts,
            &mut uncompressed_total,
            max_uncompressed_bytes,
        )?;
        write_zip_entry(
            &mut zip,
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            &slide_layout_rels,
            zip_opts,
            &mut uncompressed_total,
            max_uncompressed_bytes,
        )?;
        write_zip_entry(
            &mut zip,
            "ppt/theme/theme1.xml",
            &theme_xml,
            zip_opts,
            &mut uncompressed_total,
            max_uncompressed_bytes,
        )?;

        for (i, (s_xml, s_rels)) in slide_xmls.iter().enumerate() {
            let n = i + 1;
            write_zip_entry(
                &mut zip,
                &format!("ppt/slides/slide{n}.xml"),
                s_xml,
                zip_opts,
                &mut uncompressed_total,
                max_uncompressed_bytes,
            )?;
            write_zip_entry(
                &mut zip,
                &format!("ppt/slides/_rels/slide{n}.xml.rels"),
                s_rels,
                zip_opts,
                &mut uncompressed_total,
                max_uncompressed_bytes,
            )?;
        }

        for entry in &media_store.entries {
            write_zip_entry(
                &mut zip,
                &format!("ppt/media/{}", entry.file_name),
                &entry.bytes,
                zip_opts,
                &mut uncompressed_total,
                max_uncompressed_bytes,
            )?;
        }

        zip.finish()
            .map_err(|e| ExportBundlerError::HtmlRewrite(format!("zip finish: {e}")))?;
    }

    let pptx_bytes = cursor.into_inner();
    if pptx_bytes.len() > max_bytes {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: pptx_bytes.len(),
            limit_bytes: max_bytes,
        });
    }

    let sha256 = sha256_hex(&pptx_bytes);
    let pptx_base64 = encode_base64(&pptx_bytes);
    let byte_size = pptx_bytes.len();
    let file_name = slugify_pptx_filename(&deck.title);

    Ok(PptxExportBundle {
        file_name,
        pptx_bytes,
        pptx_base64,
        sha256,
        byte_size,
        slide_count,
        export_mode: deck.export_mode,
        engine: deck.engine.clone(),
        used_static_fallback: deck.used_static_fallback,
        fidelity_warnings: deck.fidelity_warnings.clone(),
        deduplicated_media_count: media_store.entries.len(),
    })
}

fn write_zip_entry(
    zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
    path: &str,
    data: &[u8],
    opts: SimpleFileOptions,
    uncompressed_total: &mut usize,
    max_uncompressed_bytes: usize,
) -> Result<(), ExportBundlerError> {
    let next_uncompressed = uncompressed_total.saturating_add(data.len());
    if next_uncompressed > max_uncompressed_bytes {
        return Err(ExportBundlerError::AssetSizeExceeded {
            actual_bytes: next_uncompressed,
            limit_bytes: max_uncompressed_bytes,
        });
    }
    *uncompressed_total = next_uncompressed;
    zip.start_file(path, opts)
        .map_err(|e| ExportBundlerError::HtmlRewrite(format!("zip start {path}: {e}")))?;
    zip.write_all(data)
        .map_err(|e| ExportBundlerError::HtmlRewrite(format!("zip write {path}: {e}")))?;
    Ok(())
}

fn xml_err(e: std::io::Error) -> ExportBundlerError {
    ExportBundlerError::HtmlRewrite(format!("quick-xml write error: {e}"))
}

fn new_xml_writer() -> Result<Writer<Cursor<Vec<u8>>>, ExportBundlerError> {
    let mut w = Writer::new(Cursor::new(Vec::new()));
    w.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("yes"),
    )))
    .map_err(xml_err)?;
    Ok(w)
}

fn write_empty_el(
    w: &mut Writer<Cursor<Vec<u8>>>,
    name: &str,
    attrs: &[(&str, &str)],
) -> Result<(), ExportBundlerError> {
    let mut el = BytesStart::new(name);
    for &(k, v) in attrs {
        el.push_attribute((k, v));
    }
    w.write_event(Event::Empty(el)).map_err(xml_err)
}

fn write_start_el(
    w: &mut Writer<Cursor<Vec<u8>>>,
    name: &str,
    attrs: &[(&str, &str)],
) -> Result<(), ExportBundlerError> {
    let mut el = BytesStart::new(name);
    for &(k, v) in attrs {
        el.push_attribute((k, v));
    }
    w.write_event(Event::Start(el)).map_err(xml_err)
}

fn write_end_el(w: &mut Writer<Cursor<Vec<u8>>>, name: &str) -> Result<(), ExportBundlerError> {
    w.write_event(Event::End(BytesEnd::new(name)))
        .map_err(xml_err)
}

fn build_content_types_xml(
    slide_count: usize,
    media_exts: &BTreeSet<(&'static str, &'static str)>,
) -> Result<Vec<u8>, ExportBundlerError> {
    let mut w = new_xml_writer()?;
    write_start_el(&mut w, "Types", &[("xmlns", NS_CONTENT_TYPES)])?;
    write_empty_el(
        &mut w,
        "Default",
        &[
            ("Extension", "rels"),
            (
                "ContentType",
                "application/vnd.openxmlformats-package.relationships+xml",
            ),
        ],
    )?;
    write_empty_el(
        &mut w,
        "Default",
        &[("Extension", "xml"), ("ContentType", "application/xml")],
    )?;
    for &(ext, mime) in media_exts {
        write_empty_el(
            &mut w,
            "Default",
            &[("Extension", ext), ("ContentType", mime)],
        )?;
    }
    write_empty_el(
        &mut w,
        "Override",
        &[
            ("PartName", "/ppt/presentation.xml"),
            (
                "ContentType",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
            ),
        ],
    )?;
    write_empty_el(
        &mut w,
        "Override",
        &[
            ("PartName", "/ppt/slideMasters/slideMaster1.xml"),
            (
                "ContentType",
                "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml",
            ),
        ],
    )?;
    write_empty_el(
        &mut w,
        "Override",
        &[
            ("PartName", "/ppt/slideLayouts/slideLayout1.xml"),
            (
                "ContentType",
                "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml",
            ),
        ],
    )?;
    write_empty_el(
        &mut w,
        "Override",
        &[
            ("PartName", "/ppt/theme/theme1.xml"),
            (
                "ContentType",
                "application/vnd.openxmlformats-officedocument.theme+xml",
            ),
        ],
    )?;
    for idx in 1..=slide_count {
        let part = format!("/ppt/slides/slide{idx}.xml");
        write_empty_el(
            &mut w,
            "Override",
            &[
                ("PartName", &part),
                (
                    "ContentType",
                    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
                ),
            ],
        )?;
    }
    write_end_el(&mut w, "Types")?;
    Ok(w.into_inner().into_inner())
}

fn build_rels_xml(rels: &[(&str, &str, &str)]) -> Result<Vec<u8>, ExportBundlerError> {
    let mut w = new_xml_writer()?;
    write_start_el(&mut w, "Relationships", &[("xmlns", NS_PKG_RELS)])?;
    for &(id, rel_type, target) in rels {
        write_empty_el(
            &mut w,
            "Relationship",
            &[("Id", id), ("Type", rel_type), ("Target", target)],
        )?;
    }
    write_end_el(&mut w, "Relationships")?;
    Ok(w.into_inner().into_inner())
}

fn build_presentation_xml(slide_count: usize) -> Result<Vec<u8>, ExportBundlerError> {
    let mut w = new_xml_writer()?;
    write_start_el(
        &mut w,
        "p:presentation",
        &[
            ("xmlns:a", NS_A),
            ("xmlns:r", NS_R),
            ("xmlns:p", NS_P),
            ("saveSubsetFonts", "1"),
        ],
    )?;
    write_start_el(&mut w, "p:sldMasterIdLst", &[])?;
    write_empty_el(
        &mut w,
        "p:sldMasterId",
        &[("id", "2147483648"), ("r:id", "rId1")],
    )?;
    write_end_el(&mut w, "p:sldMasterIdLst")?;

    write_start_el(&mut w, "p:sldIdLst", &[])?;
    for idx in 0..slide_count {
        let sld_id = (256 + idx).to_string();
        let rid = format!("rId{}", idx + 2);
        write_empty_el(&mut w, "p:sldId", &[("id", &sld_id), ("r:id", &rid)])?;
    }
    write_end_el(&mut w, "p:sldIdLst")?;

    let w_str = SLIDE_W_EMU.to_string();
    let h_str = SLIDE_H_EMU.to_string();
    write_empty_el(
        &mut w,
        "p:sldSz",
        &[("cx", &w_str), ("cy", &h_str), ("type", "screen16x9")],
    )?;
    write_empty_el(&mut w, "p:notesSz", &[("cx", "6858000"), ("cy", "9144000")])?;
    write_end_el(&mut w, "p:presentation")?;
    Ok(w.into_inner().into_inner())
}

fn build_presentation_rels_xml(slide_count: usize) -> Result<Vec<u8>, ExportBundlerError> {
    let mut owned: Vec<(String, &'static str, String)> = Vec::with_capacity(slide_count + 1);
    owned.push((
        "rId1".to_string(),
        REL_SLIDE_MASTER,
        "slideMasters/slideMaster1.xml".to_string(),
    ));
    for idx in 1..=slide_count {
        owned.push((
            format!("rId{}", idx + 1),
            REL_SLIDE,
            format!("slides/slide{idx}.xml"),
        ));
    }
    let refs: Vec<(&str, &str, &str)> = owned
        .iter()
        .map(|(id, t, target)| (id.as_str(), *t, target.as_str()))
        .collect();
    build_rels_xml(&refs)
}

fn build_slide_master_xml() -> Result<Vec<u8>, ExportBundlerError> {
    let mut w = new_xml_writer()?;
    write_start_el(
        &mut w,
        "p:sldMaster",
        &[("xmlns:a", NS_A), ("xmlns:r", NS_R), ("xmlns:p", NS_P)],
    )?;
    write_start_el(&mut w, "p:cSld", &[])?;
    write_empty_sp_tree(&mut w)?;
    write_end_el(&mut w, "p:cSld")?;
    write_empty_el(
        &mut w,
        "p:clrMap",
        &[
            ("bg1", "lt1"),
            ("tx1", "dk1"),
            ("bg2", "lt2"),
            ("tx2", "dk2"),
            ("accent1", "accent1"),
            ("accent2", "accent2"),
            ("accent3", "accent3"),
            ("accent4", "accent4"),
            ("accent5", "accent5"),
            ("accent6", "accent6"),
            ("hlink", "hlink"),
            ("folHlink", "folHlink"),
        ],
    )?;
    write_start_el(&mut w, "p:sldLayoutIdLst", &[])?;
    write_empty_el(
        &mut w,
        "p:sldLayoutId",
        &[("id", "2147483649"), ("r:id", "rId1")],
    )?;
    write_end_el(&mut w, "p:sldLayoutIdLst")?;
    write_end_el(&mut w, "p:sldMaster")?;
    Ok(w.into_inner().into_inner())
}

fn build_slide_layout_xml() -> Result<Vec<u8>, ExportBundlerError> {
    let mut w = new_xml_writer()?;
    write_start_el(
        &mut w,
        "p:sldLayout",
        &[
            ("xmlns:a", NS_A),
            ("xmlns:r", NS_R),
            ("xmlns:p", NS_P),
            ("type", "blank"),
            ("preserve", "1"),
        ],
    )?;
    write_start_el(&mut w, "p:cSld", &[("name", "Blank")])?;
    write_empty_sp_tree(&mut w)?;
    write_end_el(&mut w, "p:cSld")?;
    write_start_el(&mut w, "p:clrMapOvr", &[])?;
    write_empty_el(&mut w, "a:masterClrMapping", &[])?;
    write_end_el(&mut w, "p:clrMapOvr")?;
    write_end_el(&mut w, "p:sldLayout")?;
    Ok(w.into_inner().into_inner())
}

fn write_empty_sp_tree(w: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), ExportBundlerError> {
    write_start_el(w, "p:spTree", &[])?;
    write_sp_tree_header(w)?;
    write_end_el(w, "p:spTree")
}

fn write_sp_tree_header(w: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), ExportBundlerError> {
    write_start_el(w, "p:nvGrpSpPr", &[])?;
    write_empty_el(w, "p:cNvPr", &[("id", "1"), ("name", "")])?;
    write_empty_el(w, "p:cNvGrpSpPr", &[])?;
    write_empty_el(w, "p:nvPr", &[])?;
    write_end_el(w, "p:nvGrpSpPr")?;
    write_start_el(w, "p:grpSpPr", &[])?;
    write_start_el(w, "a:xfrm", &[])?;
    write_empty_el(w, "a:off", &[("x", "0"), ("y", "0")])?;
    write_empty_el(w, "a:ext", &[("cx", "0"), ("cy", "0")])?;
    write_empty_el(w, "a:chOff", &[("x", "0"), ("y", "0")])?;
    write_empty_el(w, "a:chExt", &[("cx", "0"), ("cy", "0")])?;
    write_end_el(w, "a:xfrm")?;
    write_end_el(w, "p:grpSpPr")
}

fn build_theme_xml() -> Result<Vec<u8>, ExportBundlerError> {
    let mut w = new_xml_writer()?;
    write_start_el(
        &mut w,
        "a:theme",
        &[("xmlns:a", NS_A), ("name", "CowokDesign")],
    )?;
    write_start_el(&mut w, "a:themeElements", &[])?;

    write_start_el(&mut w, "a:clrScheme", &[("name", "Cowok")])?;
    for &(slot, hex) in &[
        ("a:dk1", "0F172A"),
        ("a:lt1", "FFFFFF"),
        ("a:dk2", "334155"),
        ("a:lt2", "F8FAFC"),
        ("a:accent1", "2563EB"),
        ("a:accent2", "0EA5E9"),
        ("a:accent3", "10B981"),
        ("a:accent4", "F59E0B"),
        ("a:accent5", "8B5CF6"),
        ("a:accent6", "EC4899"),
        ("a:hlink", "2563EB"),
        ("a:folHlink", "7C3AED"),
    ] {
        write_start_el(&mut w, slot, &[])?;
        write_empty_el(&mut w, "a:srgbClr", &[("val", hex)])?;
        write_end_el(&mut w, slot)?;
    }
    write_end_el(&mut w, "a:clrScheme")?;

    write_start_el(&mut w, "a:fontScheme", &[("name", "CowokFonts")])?;
    for slot in ["a:majorFont", "a:minorFont"] {
        write_start_el(&mut w, slot, &[])?;
        write_empty_el(&mut w, "a:latin", &[("typeface", "Inter")])?;
        write_empty_el(&mut w, "a:ea", &[("typeface", "Noto Sans CJK JP")])?;
        write_empty_el(&mut w, "a:cs", &[("typeface", "Inter")])?;
        write_end_el(&mut w, slot)?;
    }
    write_end_el(&mut w, "a:fontScheme")?;

    write_start_el(&mut w, "a:fmtScheme", &[("name", "CowokFmt")])?;
    write_start_el(&mut w, "a:fillStyleLst", &[])?;
    for _ in 0..3 {
        write_start_el(&mut w, "a:solidFill", &[])?;
        write_empty_el(&mut w, "a:schemeClr", &[("val", "phClr")])?;
        write_end_el(&mut w, "a:solidFill")?;
    }
    write_end_el(&mut w, "a:fillStyleLst")?;
    write_start_el(&mut w, "a:lnStyleLst", &[])?;
    for _ in 0..3 {
        write_start_el(&mut w, "a:ln", &[("w", "9525")])?;
        write_start_el(&mut w, "a:solidFill", &[])?;
        write_empty_el(&mut w, "a:schemeClr", &[("val", "phClr")])?;
        write_end_el(&mut w, "a:solidFill")?;
        write_end_el(&mut w, "a:ln")?;
    }
    write_end_el(&mut w, "a:lnStyleLst")?;
    write_start_el(&mut w, "a:effectStyleLst", &[])?;
    for _ in 0..3 {
        write_start_el(&mut w, "a:effectStyle", &[])?;
        write_empty_el(&mut w, "a:effectLst", &[])?;
        write_end_el(&mut w, "a:effectStyle")?;
    }
    write_end_el(&mut w, "a:effectStyleLst")?;
    write_start_el(&mut w, "a:bgFillStyleLst", &[])?;
    for _ in 0..3 {
        write_start_el(&mut w, "a:solidFill", &[])?;
        write_empty_el(&mut w, "a:schemeClr", &[("val", "phClr")])?;
        write_end_el(&mut w, "a:solidFill")?;
    }
    write_end_el(&mut w, "a:bgFillStyleLst")?;
    write_end_el(&mut w, "a:fmtScheme")?;

    write_end_el(&mut w, "a:themeElements")?;
    write_end_el(&mut w, "a:theme")?;
    Ok(w.into_inner().into_inner())
}

fn build_slide_xml_and_rels(
    slide: &PptxSlideIr,
    media_pool: &[PptxMediaAsset],
    media_store: &mut MediaStore,
    max_bytes: usize,
) -> Result<(Vec<u8>, Vec<u8>), ExportBundlerError> {
    let mut rels: Vec<(String, &'static str, String)> = vec![(
        "rId1".to_string(),
        REL_SLIDE_LAYOUT,
        "../slideLayouts/slideLayout1.xml".to_string(),
    )];
    let mut media_rids: HashMap<String, String> = HashMap::new();
    let mut next_rid = 2usize;

    let mut register_media = |bytes: &[u8],
                              format: PptxMediaFormat,
                              store: &mut MediaStore|
     -> Result<String, ExportBundlerError> {
        let fname = store.insert(bytes, format, max_bytes)?;
        if let Some(existing_rid) = media_rids.get(&fname) {
            return Ok(existing_rid.clone());
        }
        let rid = format!("rId{next_rid}");
        next_rid += 1;
        rels.push((rid.clone(), REL_IMAGE, format!("../media/{fname}")));
        media_rids.insert(fname, rid.clone());
        Ok(rid)
    };

    let mut w = new_xml_writer()?;
    write_start_el(
        &mut w,
        "p:sld",
        &[("xmlns:a", NS_A), ("xmlns:r", NS_R), ("xmlns:p", NS_P)],
    )?;
    write_start_el(&mut w, "p:cSld", &[])?;

    if slide.background != PptxFill::None {
        write_start_el(&mut w, "p:bg", &[])?;
        write_start_el(&mut w, "p:bgPr", &[])?;
        write_fill(&mut w, &slide.background)?;
        write_empty_el(&mut w, "a:effectLst", &[])?;
        write_end_el(&mut w, "p:bgPr")?;
        write_end_el(&mut w, "p:bg")?;
    }

    write_start_el(&mut w, "p:spTree", &[])?;
    write_sp_tree_header(&mut w)?;

    for node in &slide.nodes {
        match node {
            PptxNode::TextBox {
                id,
                name,
                rect,
                fill,
                border_color,
                border_width_emu,
                geom,
                shadow,
                padding,
                paragraphs,
                auto_fit,
            } => {
                let buffered_rect = apply_text_width_buffer(*rect, None);
                write_textbox_node(
                    &mut w,
                    *id,
                    name,
                    buffered_rect,
                    fill,
                    border_color.as_ref(),
                    *border_width_emu,
                    *geom,
                    shadow.as_ref(),
                    *padding,
                    paragraphs,
                    *auto_fit,
                )?;
            }
            PptxNode::Shape {
                id,
                name,
                rect,
                fill,
                border_color,
                border_width_emu,
                geom,
                shadow,
            } => {
                write_shape_node(
                    &mut w,
                    *id,
                    name,
                    *rect,
                    fill,
                    border_color.as_ref(),
                    *border_width_emu,
                    *geom,
                    shadow.as_ref(),
                )?;
            }
            PptxNode::Table {
                id,
                name,
                rect,
                grid,
                lang,
                font_latin,
                font_ea,
            } => {
                write_table_node(&mut w, *id, name, *rect, grid, lang, font_latin, font_ea)?;
            }
            PptxNode::Image {
                id,
                name,
                rect,
                geom,
                shadow,
                media_index,
                alt_text,
            } => {
                let asset = media_pool.get(*media_index).ok_or_else(|| {
                    ExportBundlerError::InvalidInput(format!(
                        "missing media_pool asset at index {media_index}"
                    ))
                })?;
                let rid = register_media(&asset.bytes, asset.format, media_store)?;
                write_image_node(
                    &mut w,
                    *id,
                    name,
                    *rect,
                    *geom,
                    shadow.as_ref(),
                    &rid,
                    None,
                    alt_text,
                )?;
            }
            PptxNode::DualSvg {
                id,
                name,
                rect,
                geom,
                shadow,
                svg_media_index,
                png_fallback_media_index,
                alt_text,
            } => {
                let png_asset = media_pool.get(*png_fallback_media_index).ok_or_else(|| {
                    ExportBundlerError::InvalidInput(format!(
                        "missing media_pool png asset at index {png_fallback_media_index}"
                    ))
                })?;
                let svg_asset = media_pool.get(*svg_media_index).ok_or_else(|| {
                    ExportBundlerError::InvalidInput(format!(
                        "missing media_pool svg asset at index {svg_media_index}"
                    ))
                })?;
                let png_rid = register_media(&png_asset.bytes, png_asset.format, media_store)?;
                let svg_rid = register_media(&svg_asset.bytes, svg_asset.format, media_store)?;
                write_image_node(
                    &mut w,
                    *id,
                    name,
                    *rect,
                    *geom,
                    shadow.as_ref(),
                    &png_rid,
                    Some(&svg_rid),
                    alt_text,
                )?;
            }
        }
    }

    write_end_el(&mut w, "p:spTree")?;
    write_end_el(&mut w, "p:cSld")?;

    write_start_el(&mut w, "p:clrMapOvr", &[])?;
    write_empty_el(&mut w, "a:masterClrMapping", &[])?;
    write_end_el(&mut w, "p:clrMapOvr")?;

    write_slide_transition(&mut w, slide.transition)?;

    write_end_el(&mut w, "p:sld")?;

    let rel_refs: Vec<(&str, &str, &str)> = rels
        .iter()
        .map(|(id, t, target)| (id.as_str(), *t, target.as_str()))
        .collect();
    let rels_bytes = build_rels_xml(&rel_refs)?;
    Ok((w.into_inner().into_inner(), rels_bytes))
}

fn write_color_srgb(
    w: &mut Writer<Cursor<Vec<u8>>>,
    color: &PptxColor,
) -> Result<(), ExportBundlerError> {
    if color.alpha_ooxml < 100_000 {
        write_start_el(w, "a:srgbClr", &[("val", &color.hex)])?;
        let alpha_s = color.alpha_ooxml.to_string();
        write_empty_el(w, "a:alpha", &[("val", &alpha_s)])?;
        write_end_el(w, "a:srgbClr")
    } else {
        write_empty_el(w, "a:srgbClr", &[("val", &color.hex)])
    }
}

fn write_fill(w: &mut Writer<Cursor<Vec<u8>>>, fill: &PptxFill) -> Result<(), ExportBundlerError> {
    match fill {
        PptxFill::None => write_empty_el(w, "a:noFill", &[]),
        PptxFill::Solid(color) => {
            if color.is_transparent() {
                write_empty_el(w, "a:noFill", &[])
            } else {
                write_start_el(w, "a:solidFill", &[])?;
                write_color_srgb(w, color)?;
                write_end_el(w, "a:solidFill")
            }
        }
        PptxFill::LinearGradient(grad) => {
            write_start_el(w, "a:gradFill", &[])?;
            write_start_el(w, "a:gsLst", &[])?;
            for stop in &grad.stops {
                let pos_s = stop.pos_ooxml.to_string();
                write_start_el(w, "a:gs", &[("pos", &pos_s)])?;
                write_color_srgb(w, &stop.color)?;
                write_end_el(w, "a:gs")?;
            }
            write_end_el(w, "a:gsLst")?;
            let ang_s = grad.ang_ooxml.to_string();
            write_empty_el(w, "a:lin", &[("ang", &ang_s), ("scaled", "1")])?;
            write_end_el(w, "a:gradFill")
        }
    }
}

fn write_xfrm(
    w: &mut Writer<Cursor<Vec<u8>>>,
    rect: PptxRectEmu,
) -> Result<(), ExportBundlerError> {
    let x = rect.x.to_string();
    let y = rect.y.to_string();
    let cx = rect.cx.to_string();
    let cy = rect.cy.to_string();
    write_start_el(w, "a:xfrm", &[])?;
    write_empty_el(w, "a:off", &[("x", &x), ("y", &y)])?;
    write_empty_el(w, "a:ext", &[("cx", &cx), ("cy", &cy)])?;
    write_end_el(w, "a:xfrm")
}

fn write_prst_geom(
    w: &mut Writer<Cursor<Vec<u8>>>,
    geom: PptxShapeGeom,
) -> Result<(), ExportBundlerError> {
    match geom {
        PptxShapeGeom::Rect => {
            write_start_el(w, "a:prstGeom", &[("prst", "rect")])?;
            write_empty_el(w, "a:avLst", &[])?;
            write_end_el(w, "a:prstGeom")
        }
        PptxShapeGeom::Ellipse => {
            write_start_el(w, "a:prstGeom", &[("prst", "ellipse")])?;
            write_empty_el(w, "a:avLst", &[])?;
            write_end_el(w, "a:prstGeom")
        }
        PptxShapeGeom::RoundRect { adj } => {
            write_start_el(w, "a:prstGeom", &[("prst", "roundRect")])?;
            write_start_el(w, "a:avLst", &[])?;
            let fmla = format!("val {adj}");
            write_empty_el(w, "a:gd", &[("name", "adj"), ("fmla", &fmla)])?;
            write_end_el(w, "a:avLst")?;
            write_end_el(w, "a:prstGeom")
        }
    }
}

fn write_shadow(
    w: &mut Writer<Cursor<Vec<u8>>>,
    shadow: Option<&PptxShadow>,
) -> Result<(), ExportBundlerError> {
    let Some(sh) = shadow else {
        return Ok(());
    };
    let blur = sh.blur_rad_emu.to_string();
    let dist = sh.dist_emu.to_string();
    let dir = sh.dir_ooxml.to_string();
    write_start_el(w, "a:effectLst", &[])?;
    write_start_el(
        w,
        "a:outerShdw",
        &[
            ("blurRad", &blur),
            ("dist", &dist),
            ("dir", &dir),
            ("rotWithShape", "0"),
        ],
    )?;
    write_color_srgb(w, &sh.color)?;
    write_end_el(w, "a:outerShdw")?;
    write_end_el(w, "a:effectLst")
}

fn write_border(
    w: &mut Writer<Cursor<Vec<u8>>>,
    border_color: Option<&PptxColor>,
    border_width_emu: i64,
) -> Result<(), ExportBundlerError> {
    let Some(color) = border_color else {
        return Ok(());
    };
    if color.is_transparent() || border_width_emu <= 0 {
        return Ok(());
    }
    let width_s = border_width_emu.to_string();
    write_start_el(w, "a:ln", &[("w", &width_s)])?;
    write_start_el(w, "a:solidFill", &[])?;
    write_color_srgb(w, color)?;
    write_end_el(w, "a:solidFill")?;
    write_end_el(w, "a:ln")
}

#[allow(clippy::too_many_arguments)]
fn write_textbox_node(
    w: &mut Writer<Cursor<Vec<u8>>>,
    id: u32,
    name: &str,
    rect: PptxRectEmu,
    fill: &PptxFill,
    border_color: Option<&PptxColor>,
    border_width_emu: i64,
    geom: PptxShapeGeom,
    shadow: Option<&PptxShadow>,
    padding: crate::pptx_ir::PptxPaddingEmu,
    paragraphs: &[PptxParagraph],
    auto_fit: bool,
) -> Result<(), ExportBundlerError> {
    let id_s = id.to_string();
    let clean_name = sanitize_xml_text(name);
    write_start_el(w, "p:sp", &[])?;
    write_start_el(w, "p:nvSpPr", &[])?;
    write_empty_el(w, "p:cNvPr", &[("id", &id_s), ("name", &clean_name)])?;
    write_empty_el(w, "p:cNvSpPr", &[("txBox", "1")])?;
    write_empty_el(w, "p:nvPr", &[])?;
    write_end_el(w, "p:nvSpPr")?;

    write_start_el(w, "p:spPr", &[])?;
    write_xfrm(w, rect)?;
    write_prst_geom(w, geom)?;
    write_fill(w, fill)?;
    write_border(w, border_color, border_width_emu)?;
    write_shadow(w, shadow)?;
    write_end_el(w, "p:spPr")?;

    write_start_el(w, "p:txBody", &[])?;
    let l_ins = padding.left.to_string();
    let r_ins = padding.right.to_string();
    let t_ins = padding.top.to_string();
    let b_ins = padding.bottom.to_string();
    write_start_el(
        w,
        "a:bodyPr",
        &[
            ("wrap", "square"),
            ("lIns", &l_ins),
            ("tIns", &t_ins),
            ("rIns", &r_ins),
            ("bIns", &b_ins),
        ],
    )?;
    if auto_fit {
        write_empty_el(w, "a:normAutofit", &[])?;
    }
    write_end_el(w, "a:bodyPr")?;
    write_empty_el(w, "a:lstStyle", &[])?;

    if paragraphs.is_empty() {
        write_empty_el(w, "a:p", &[])?;
    } else {
        for para in paragraphs {
            write_start_el(w, "a:p", &[])?;
            write_start_el(w, "a:pPr", &[("algn", para.align.as_ooxml())])?;
            if let Some(pct) = para.line_spacing_pct {
                let pct_s = pct.to_string();
                write_start_el(w, "a:lnSpc", &[])?;
                write_empty_el(w, "a:spcPct", &[("val", &pct_s)])?;
                write_end_el(w, "a:lnSpc")?;
            }
            write_end_el(w, "a:pPr")?;

            for run in &para.runs {
                let sz_s = run.font_size_hundredths_pt.to_string();
                let b_s = if run.bold { "1" } else { "0" };
                let i_s = if run.italic { "1" } else { "0" };
                let u_s = if run.underline { "sng" } else { "none" };
                let lang = sanitize_xml_text(&run.lang);
                write_start_el(w, "a:r", &[])?;
                write_start_el(
                    w,
                    "a:rPr",
                    &[
                        ("lang", &lang),
                        ("sz", &sz_s),
                        ("b", b_s),
                        ("i", i_s),
                        ("u", u_s),
                        ("dirty", "0"),
                    ],
                )?;
                write_start_el(w, "a:solidFill", &[])?;
                write_color_srgb(w, &run.color)?;
                write_end_el(w, "a:solidFill")?;
                let latin = sanitize_xml_text(&run.font_latin);
                let ea = sanitize_xml_text(&run.font_ea);
                let cs = sanitize_xml_text(&run.font_cs);
                write_empty_el(w, "a:latin", &[("typeface", &latin)])?;
                write_empty_el(w, "a:ea", &[("typeface", &ea)])?;
                write_empty_el(w, "a:cs", &[("typeface", &cs)])?;
                write_end_el(w, "a:rPr")?;

                write_start_el(w, "a:t", &[])?;
                let clean_txt = sanitize_xml_text(&run.text);
                w.write_event(Event::Text(BytesText::new(&clean_txt)))
                    .map_err(xml_err)?;
                write_end_el(w, "a:t")?;
                write_end_el(w, "a:r")?;
            }
            write_end_el(w, "a:p")?;
        }
    }

    write_end_el(w, "p:txBody")?;
    write_end_el(w, "p:sp")
}

#[allow(clippy::too_many_arguments)]
fn write_shape_node(
    w: &mut Writer<Cursor<Vec<u8>>>,
    id: u32,
    name: &str,
    rect: PptxRectEmu,
    fill: &PptxFill,
    border_color: Option<&PptxColor>,
    border_width_emu: i64,
    geom: PptxShapeGeom,
    shadow: Option<&PptxShadow>,
) -> Result<(), ExportBundlerError> {
    let id_s = id.to_string();
    let clean_name = sanitize_xml_text(name);
    write_start_el(w, "p:sp", &[])?;
    write_start_el(w, "p:nvSpPr", &[])?;
    write_empty_el(w, "p:cNvPr", &[("id", &id_s), ("name", &clean_name)])?;
    write_empty_el(w, "p:cNvSpPr", &[])?;
    write_empty_el(w, "p:nvPr", &[])?;
    write_end_el(w, "p:nvSpPr")?;

    write_start_el(w, "p:spPr", &[])?;
    write_xfrm(w, rect)?;
    write_prst_geom(w, geom)?;
    write_fill(w, fill)?;
    write_border(w, border_color, border_width_emu)?;
    write_shadow(w, shadow)?;
    write_end_el(w, "p:spPr")?;

    write_start_el(w, "p:txBody", &[])?;
    write_empty_el(w, "a:bodyPr", &[])?;
    write_empty_el(w, "a:lstStyle", &[])?;
    write_empty_el(w, "a:p", &[])?;
    write_end_el(w, "p:txBody")?;
    write_end_el(w, "p:sp")
}

#[allow(clippy::too_many_arguments)]
fn write_table_node(
    w: &mut Writer<Cursor<Vec<u8>>>,
    id: u32,
    name: &str,
    rect: PptxRectEmu,
    grid: &PptxTableGrid,
    lang: &str,
    font_latin: &str,
    font_ea: &str,
) -> Result<(), ExportBundlerError> {
    let id_s = id.to_string();
    let clean_name = sanitize_xml_text(name);
    let clean_lang = sanitize_xml_text(lang);
    let clean_latin = sanitize_xml_text(font_latin);
    let clean_ea = sanitize_xml_text(font_ea);

    write_start_el(w, "p:graphicFrame", &[])?;
    write_start_el(w, "p:nvGraphicFramePr", &[])?;
    write_empty_el(w, "p:cNvPr", &[("id", &id_s), ("name", &clean_name)])?;
    write_empty_el(w, "p:cNvGraphicFramePr", &[])?;
    write_empty_el(w, "p:nvPr", &[])?;
    write_end_el(w, "p:nvGraphicFramePr")?;

    let x = rect.x.to_string();
    let y = rect.y.to_string();
    let cx = rect.cx.to_string();
    let cy = rect.cy.to_string();
    write_start_el(w, "p:xfrm", &[])?;
    write_empty_el(w, "a:off", &[("x", &x), ("y", &y)])?;
    write_empty_el(w, "a:ext", &[("cx", &cx), ("cy", &cy)])?;
    write_end_el(w, "p:xfrm")?;

    write_start_el(w, "a:graphic", &[])?;
    write_start_el(
        w,
        "a:graphicData",
        &[(
            "uri",
            "http://schemas.openxmlformats.org/drawingml/2006/table",
        )],
    )?;
    write_start_el(w, "a:tbl", &[])?;
    write_empty_el(w, "a:tblPr", &[("firstRow", "1"), ("bandRow", "1")])?;

    write_start_el(w, "a:tblGrid", &[])?;
    for col_w in &grid.col_widths_emu {
        let w_s = col_w.to_string();
        write_empty_el(w, "a:gridCol", &[("w", &w_s)])?;
    }
    write_end_el(w, "a:tblGrid")?;

    for (r_idx, row) in grid.rows.iter().enumerate() {
        let row_h = grid
            .row_heights_emu
            .get(r_idx)
            .copied()
            .unwrap_or(300_000)
            .to_string();
        write_start_el(w, "a:tr", &[("h", &row_h)])?;
        for cell in row {
            match cell {
                PptxTableCellKind::Origin {
                    text,
                    grid_span,
                    row_span,
                    is_header,
                    fill,
                    text_color,
                    font_size_px,
                    align,
                } => {
                    let mut attrs: Vec<(&str, String)> = Vec::new();
                    if *grid_span > 1 {
                        attrs.push(("gridSpan", grid_span.to_string()));
                    }
                    if *row_span > 1 {
                        attrs.push(("rowSpan", row_span.to_string()));
                    }
                    let attr_refs: Vec<(&str, &str)> =
                        attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
                    write_start_el(w, "a:tc", &attr_refs)?;

                    write_start_el(w, "a:txBody", &[])?;
                    write_empty_el(w, "a:bodyPr", &[])?;
                    write_empty_el(w, "a:lstStyle", &[])?;
                    write_start_el(w, "a:p", &[])?;
                    write_empty_el(w, "a:pPr", &[("algn", align.as_ooxml())])?;
                    write_start_el(w, "a:r", &[])?;
                    let sz_s = font_size_px_to_ooxml(*font_size_px).to_string();
                    let b_s = if *is_header { "1" } else { "0" };
                    write_start_el(
                        w,
                        "a:rPr",
                        &[
                            ("lang", &clean_lang),
                            ("sz", &sz_s),
                            ("b", b_s),
                            ("dirty", "0"),
                        ],
                    )?;
                    write_start_el(w, "a:solidFill", &[])?;
                    write_color_srgb(w, text_color)?;
                    write_end_el(w, "a:solidFill")?;
                    write_empty_el(w, "a:latin", &[("typeface", &clean_latin)])?;
                    write_empty_el(w, "a:ea", &[("typeface", &clean_ea)])?;
                    write_empty_el(w, "a:cs", &[("typeface", DEFAULT_CS_FONT)])?;
                    write_end_el(w, "a:rPr")?;
                    write_start_el(w, "a:t", &[])?;
                    let clean_text = sanitize_xml_text(text);
                    w.write_event(Event::Text(BytesText::new(&clean_text)))
                        .map_err(xml_err)?;
                    write_end_el(w, "a:t")?;
                    write_end_el(w, "a:r")?;
                    write_end_el(w, "a:p")?;
                    write_end_el(w, "a:txBody")?;

                    write_start_el(w, "a:tcPr", &[])?;
                    if let Some(bg) = fill {
                        if !bg.is_transparent() {
                            write_start_el(w, "a:solidFill", &[])?;
                            write_color_srgb(w, bg)?;
                            write_end_el(w, "a:solidFill")?;
                        }
                    }
                    write_end_el(w, "a:tcPr")?;
                    write_end_el(w, "a:tc")?;
                }
                PptxTableCellKind::HMerge => {
                    write_dummy_merge_cell(w, &[("hMerge", "1")])?;
                }
                PptxTableCellKind::VMerge { grid_span } => {
                    if *grid_span > 1 {
                        let gs = grid_span.to_string();
                        write_dummy_merge_cell(w, &[("gridSpan", gs.as_str()), ("vMerge", "1")])?;
                    } else {
                        write_dummy_merge_cell(w, &[("vMerge", "1")])?;
                    }
                }
                PptxTableCellKind::HvMerge => {
                    write_dummy_merge_cell(w, &[("hMerge", "1"), ("vMerge", "1")])?;
                }
            }
        }
        write_end_el(w, "a:tr")?;
    }

    write_end_el(w, "a:tbl")?;
    write_end_el(w, "a:graphicData")?;
    write_end_el(w, "a:graphic")?;
    write_end_el(w, "p:graphicFrame")
}

fn write_dummy_merge_cell(
    w: &mut Writer<Cursor<Vec<u8>>>,
    attrs: &[(&str, &str)],
) -> Result<(), ExportBundlerError> {
    write_start_el(w, "a:tc", attrs)?;
    write_start_el(w, "a:txBody", &[])?;
    write_empty_el(w, "a:bodyPr", &[])?;
    write_empty_el(w, "a:lstStyle", &[])?;
    write_empty_el(w, "a:p", &[])?;
    write_end_el(w, "a:txBody")?;
    write_empty_el(w, "a:tcPr", &[])?;
    write_end_el(w, "a:tc")
}

#[allow(clippy::too_many_arguments)]
fn write_image_node(
    w: &mut Writer<Cursor<Vec<u8>>>,
    id: u32,
    name: &str,
    rect: PptxRectEmu,
    geom: PptxShapeGeom,
    shadow: Option<&PptxShadow>,
    primary_rid: &str,
    dual_svg_rid: Option<&str>,
    alt_text: &str,
) -> Result<(), ExportBundlerError> {
    let id_s = id.to_string();
    let clean_name = sanitize_xml_text(name);
    let clean_alt = sanitize_xml_text(alt_text);

    write_start_el(w, "p:pic", &[])?;
    write_start_el(w, "p:nvPicPr", &[])?;
    write_empty_el(
        w,
        "p:cNvPr",
        &[("id", &id_s), ("name", &clean_name), ("descr", &clean_alt)],
    )?;
    write_start_el(w, "p:cNvPicPr", &[])?;
    write_empty_el(w, "a:picLocks", &[("noChangeAspect", "1")])?;
    write_end_el(w, "p:cNvPicPr")?;
    write_empty_el(w, "p:nvPr", &[])?;
    write_end_el(w, "p:nvPicPr")?;

    write_start_el(w, "p:blipFill", &[])?;
    if let Some(svg_rid) = dual_svg_rid {
        write_start_el(w, "a:blip", &[("r:embed", primary_rid)])?;
        write_start_el(w, "a:extLst", &[])?;
        write_start_el(w, "a:ext", &[("uri", SVG_BLIP_EXT_URI)])?;
        write_empty_el(
            w,
            "a16:svgBlip",
            &[("xmlns:a16", NS_A16), ("r:embed", svg_rid)],
        )?;
        write_end_el(w, "a:ext")?;
        write_end_el(w, "a:extLst")?;
        write_end_el(w, "a:blip")?;
    } else {
        write_empty_el(w, "a:blip", &[("r:embed", primary_rid)])?;
    }
    write_start_el(w, "a:stretch", &[])?;
    write_empty_el(w, "a:fillRect", &[])?;
    write_end_el(w, "a:stretch")?;
    write_end_el(w, "p:blipFill")?;

    write_start_el(w, "p:spPr", &[])?;
    write_xfrm(w, rect)?;
    write_prst_geom(w, geom)?;
    write_shadow(w, shadow)?;
    write_end_el(w, "p:spPr")?;
    write_end_el(w, "p:pic")
}

fn write_slide_transition(
    w: &mut Writer<Cursor<Vec<u8>>>,
    transition: PptxSlideTransition,
) -> Result<(), ExportBundlerError> {
    match transition {
        PptxSlideTransition::None => Ok(()),
        PptxSlideTransition::Fade => {
            write_start_el(w, "p:transition", &[("spd", "med")])?;
            write_empty_el(w, "p:fade", &[])?;
            write_end_el(w, "p:transition")
        }
        PptxSlideTransition::Push => {
            write_start_el(w, "p:transition", &[("spd", "med")])?;
            write_empty_el(w, "p:push", &[("dir", "l")])?;
            write_end_el(w, "p:transition")
        }
        PptxSlideTransition::Wipe => {
            write_start_el(w, "p:transition", &[("spd", "med")])?;
            write_empty_el(w, "p:wipe", &[("dir", "l")])?;
            write_end_el(w, "p:transition")
        }
        PptxSlideTransition::Morph => {
            write_start_el(w, "mc:AlternateContent", &[("xmlns:mc", NS_MC)])?;
            write_start_el(
                w,
                "mc:Choice",
                &[("Requires", "p159"), ("xmlns:p159", NS_P159)],
            )?;
            write_start_el(w, "p:transition", &[("spd", "med")])?;
            write_empty_el(w, "p159:morph", &[("option", "byObject")])?;
            write_end_el(w, "p:transition")?;
            write_end_el(w, "mc:Choice")?;
            write_start_el(w, "mc:Fallback", &[])?;
            write_start_el(w, "p:transition", &[("spd", "med")])?;
            write_empty_el(w, "p:fade", &[])?;
            write_end_el(w, "p:transition")?;
            write_end_el(w, "mc:Fallback")?;
            write_end_el(w, "mc:AlternateContent")
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha256_bytes(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i];
        let b1 = if i + 1 < input.len() { input[i + 1] } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] } else { 0 };
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < input.len() {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < input.len() {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}
