// @story 06.1, 06.2, 06.3, 06.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
pub mod chart_normalizer;
pub mod charts_2d;
pub mod cms_patcher;
pub mod deck_generator;
pub mod protocol;
pub mod templates;
pub mod ulid;

pub use chart_normalizer::{
    extract_pdf_table_rows, largest_remainder_percentages, normalize_chart_source,
    parse_delimited_table, parse_localized_number_with_unit, ChartNormalizeOptions,
    ChartNormalizerError, ChartSourceFormat, NormalizedChartKind, NormalizedChartOutput,
    NormalizedPointRaw, ParsedNumericCell, MAX_CSV_BYTES, MAX_PDF_BYTES, MAX_PDF_PAGES, MAX_POINTS,
    MAX_SERIES,
};

pub use charts_2d::{
    estimate_svg_primitives, validate_chart_json_str, validate_chart_spec_v12, ChartColorToken,
    ChartDataPoint, ChartKind2D, ChartSeries, ChartSpecV12, ChartValidationError,
    ValidatedChartSpec, CHART_MAX_ABS_VALUE, CHART_MAX_JSON_BYTES, CHART_MAX_LABEL_CHARS,
    CHART_MAX_POINTS_PER_SERIES, CHART_MAX_SERIES, CHART_MAX_SVG_PRIMITIVES,
    CHART_PROPORTIONAL_SUM_TOLERANCE,
};

pub use deck_generator::{
    count_words, enforce_slide_word_budget_and_split, generate_stageshow_deck,
    normalize_bullets_to_budget, render_stageshow_slide_html, synthesize_slide_rewrite,
    DeckGeneratorError, DraftSlideInput, GenerateDeckSpecInput, GeneratedDeckOutput,
    GeneratedSlideArtifact, GeneratedWordingSlot, SlideRewriteInput, SlideRewriteOutput,
    StageshowFramePreset, MAX_GENERATED_SLIDES, STAGESHOW_MAX_BULLET_WORDS,
    STAGESHOW_MAX_HEADLINE_WORDS, STAGESHOW_MAX_WORDS_PER_SLIDE, STAGESHOW_MIN_BULLET_WORDS,
};

pub use cms_patcher::{
    extract_slide_uids_in_html, inspect_slide_html_convergence,
    inspect_slide_html_convergence_scoped, is_safe_asset_url, is_valid_ff_id,
    patch_slide_slots_lol_html, sanitize_slide_html_deep, sanitize_slide_html_deep_scoped,
    validate_and_scrub_css_tokens, CmsPatcherError, CmsSlotKind, DeepSanitizeOutput, ExtractedSlot,
    SlotPatchInput, SlotPatchOutput, CUSTOM_OVERRIDE_ATTR, MAX_SLOT_TEXT_CHARS,
};

pub use protocol::{
    rewrite_slide_html_for_duplicate, sanitize_slide_html_fragment,
    validate_slide_protocol_structure, ProtocolValidationError, ValidatedSlideHtml,
    MAX_SLIDE_HTML_BYTES,
};
pub use templates::{
    compute_sha256_hex, render_branded_slide, RenderSlideRequest, RenderedSlideOutput,
    SlideLayoutKind,
};
pub use ulid::{generate_slide_uid, is_valid_slide_uid};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
