// @story 05.4
// @implements AC-02, AC-04, AC-05
//! Shared `html-forge` crate (`TDR-006`) providing mXSS-hardened `lol_html + html5ever`
//! HTML sanitization & surgical patching for both `SanitizeProfile::Slide` and
//! `SanitizeProfile::AppPreview`, structural `data-ff-id` reconciliation, and deterministic
//! OKLCH WCAG AA contrast clamping.

pub mod brand_studio;
pub mod export_bundler;
pub mod fe_contract;
pub mod oklch;
pub mod patcher;
pub mod reconcile;
pub mod sketch_scanner;
pub mod visual_qc;

pub use brand_studio::{
    generate_default_brand_proposals, is_valid_theme_mode_id, render_tokenized_brand_preview,
    select_representative_sketch_pages, validate_brand_proposal, BrandModeTokens,
    BrandPreviewBundle, BrandProposalV1, DensityScale, FontPairing, RadiusScale, SketchPageInput,
    TokenizedPagePreview, MAX_INSPIRATION_REFS, MAX_MODE_ID_LEN, MAX_REPRESENTATIVE_SKETCH_PAGES,
    MAX_THEME_MODES,
};
pub use fe_contract::{
    extract_fe_data_contract, render_fe_data_contract_ts, FeActionInput, FeDataContractDraft,
    FeViewModelField, InferredFieldType, FE_CONTRACT_BOUNDARY_NOTICE,
};
pub use oklch::{
    contrast_ratio_hex, evaluate_oklch_contrast_guard, hex_to_oklch, oklch_to_hex, OklchColor,
    OklchContrastReport, WCAG_AA_MIN_CONTRAST,
};
pub use patcher::{
    compute_sha256_hex, escape_html_attr, escape_html_text, is_safe_asset_url, is_valid_ff_id,
    parse_html5_dom, patch_html_element, sanitize_html_with_profile, scan_forbidden_css_tokens,
    strip_css_comments_fail_closed, validate_and_scrub_css_tokens, HtmlForgeError,
    HtmlPatchOperation, HtmlPatchOutput, SanitizeProfile, MAX_FF_ID_LEN, MAX_HTML_BYTES,
    MAX_SLOT_TEXT_CHARS,
};
pub use reconcile::{reconcile_ff_ids, ReconcileReport, DEFAULT_RECONCILE_MIN_CONFIDENCE};
pub use sketch_scanner::{
    default_popular_deck_catalog, default_popular_fe_catalog, scan_sketch_components,
    CatalogComponentEntry, PopularComponentAuditItem, PopularComponentCategory,
    PopularComponentStatus, SketchComponentScanResult,
};
