// @story 05.6
// @implements AC-01, AC-02, AC-06
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::oklch::{evaluate_oklch_contrast_guard, OklchContrastReport, WCAG_AA_MIN_CONTRAST};
use crate::patcher::{
    compute_sha256_hex, escape_html_attr, sanitize_html_with_profile, scan_forbidden_css_tokens,
    strip_css_comments_fail_closed, validate_and_scrub_css_tokens, HtmlForgeError, SanitizeProfile,
};

pub const MAX_THEME_MODES: usize = 6;
pub const MAX_INSPIRATION_REFS: usize = 6;
pub const MAX_REPRESENTATIVE_SKETCH_PAGES: usize = 3;
pub const MAX_MODE_ID_LEN: usize = 32;
pub const MAX_TOKEN_VALUE_LEN: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandModeTokens {
    pub mode_id: String,
    pub label: String,
    pub tokens: BTreeMap<String, String>,
    pub wcag_aa_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontPairing {
    pub heading: String,
    pub body: String,
    pub mono: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RadiusScale {
    Sharp,
    Balanced,
    Soft,
    Pill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DensityScale {
    Airy,
    Comfortable,
    Compact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandProposalV1 {
    pub id: String,
    pub brand_name: String,
    pub rationale: String,
    pub inspiration_refs: Vec<String>,
    pub theme_modes: Vec<BrandModeTokens>,
    pub font_pairing: FontPairing,
    pub radius_scale: RadiusScale,
    pub density: DensityScale,
    #[serde(default)]
    pub component_style_overrides: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SketchPageInput {
    pub screen_id: String,
    pub title: String,
    #[serde(default)]
    pub page_role: String,
    pub html: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizedPagePreview {
    pub screen_id: String,
    pub title: String,
    pub page_role: String,
    pub mode_id: String,
    pub rendered_html: String,
    pub content_sha256: String,
    pub wcag_aa_verified: bool,
    pub contrast_reports: Vec<OklchContrastReport>,
    pub render_time_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandPreviewBundle {
    pub proposal_id: String,
    pub active_mode_id: String,
    pub requested_modes: Vec<String>,
    pub total_sketch_pages: usize,
    pub capped_to_representative: bool,
    pub pages: Vec<TokenizedPagePreview>,
    pub total_render_time_micros: u64,
}

/// Validates a theme mode ID against `^[a-z0-9][a-z0-9-]{0,31}$`.
pub fn is_valid_theme_mode_id(mode_id: &str) -> bool {
    let bytes = mode_id.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_MODE_ID_LEN {
        return false;
    }
    let first = bytes[0];
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_valid_css_token_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("--") else {
        return false;
    };
    !rest.is_empty()
        && rest.len() <= 64
        && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn validate_safe_font_stack(font: &str, field: &str) -> Result<String, HtmlForgeError> {
    let trimmed = font.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_TOKEN_VALUE_LEN {
        return Err(HtmlForgeError::InvalidPayload(format!(
            "Font pairing field '{field}' must be 1..={MAX_TOKEN_VALUE_LEN} chars"
        )));
    }
    let stripped = strip_css_comments_fail_closed(trimmed)?;
    scan_forbidden_css_tokens(&stripped)?;
    let valid_chars = stripped.bytes().all(|b| {
        b.is_ascii_alphanumeric() || matches!(b, b' ' | b',' | b'-' | b'_' | b'\'' | b'"')
    });
    if !valid_chars {
        return Err(HtmlForgeError::CssTokenBlocked(format!(
            "unsafe-font:{field}"
        )));
    }
    Ok(stripped)
}

fn sanitize_token_value(key: &str, raw_val: &str) -> Result<String, HtmlForgeError> {
    let val = raw_val.trim();
    if val.is_empty() || val.len() > MAX_TOKEN_VALUE_LEN {
        return Err(HtmlForgeError::CssTokenBlocked(format!(
            "token-length:{key}"
        )));
    }
    let stripped = strip_css_comments_fail_closed(val)?;
    scan_forbidden_css_tokens(&stripped)?;
    if stripped.contains(';') || stripped.contains('{') || stripped.contains('}') {
        return Err(HtmlForgeError::CssTokenBlocked(format!(
            "token-delimiter:{key}"
        )));
    }
    if key.starts_with("--font-") {
        return validate_safe_font_stack(&stripped, key);
    }
    let decl = format!("{key}: {stripped}");
    validate_and_scrub_css_tokens(&decl, true)?;
    Ok(stripped)
}

fn is_valid_css_property_name(prop: &str) -> bool {
    let bytes = prop.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    let first = bytes[0];
    if !(first.is_ascii_alphabetic() || first == b'-') {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
}

fn enforce_mode_oklch_contrast(
    tokens: &mut BTreeMap<String, String>,
) -> Result<(bool, Vec<OklchContrastReport>), HtmlForgeError> {
    let mut reports = Vec::new();
    let Some(bg_surface) = tokens.get("--bg-surface").cloned() else {
        return Ok((false, reports));
    };
    let Some(fg_primary) = tokens.get("--text-primary").cloned() else {
        return Ok((false, reports));
    };
    let rep_surface =
        evaluate_oklch_contrast_guard(&fg_primary, &bg_surface, WCAG_AA_MIN_CONTRAST)?;
    if rep_surface.clamped {
        tokens.insert(
            "--text-primary".to_string(),
            rep_surface.adjusted_fg_hex.clone(),
        );
    }
    reports.push(rep_surface);

    if let Some(bg_canvas) = tokens.get("--bg-canvas").cloned() {
        let current_fg = tokens.get("--text-primary").cloned().unwrap_or(fg_primary);
        let rep_canvas =
            evaluate_oklch_contrast_guard(&current_fg, &bg_canvas, WCAG_AA_MIN_CONTRAST)?;
        if rep_canvas.clamped {
            tokens.insert(
                "--text-primary".to_string(),
                rep_canvas.adjusted_fg_hex.clone(),
            );
        }
        reports.push(rep_canvas);
    }

    if let (Some(accent_bg), Some(on_accent)) = (
        tokens.get("--action-primary").cloned(),
        tokens.get("--text-on-accent").cloned(),
    ) {
        let rep = evaluate_oklch_contrast_guard(&on_accent, &accent_bg, WCAG_AA_MIN_CONTRAST)?;
        if rep.clamped {
            tokens.insert("--text-on-accent".to_string(), rep.adjusted_fg_hex.clone());
        }
        reports.push(rep);
    } else if tokens.contains_key("--action-primary") ^ tokens.contains_key("--text-on-accent") {
        return Ok((false, reports));
    }

    let final_fg = tokens.get("--text-primary").cloned().unwrap_or_default();
    let final_surface_ok =
        evaluate_oklch_contrast_guard(&final_fg, &bg_surface, WCAG_AA_MIN_CONTRAST)
            .is_ok_and(|r| r.original_ratio >= WCAG_AA_MIN_CONTRAST);
    let all_pass = !reports.is_empty()
        && final_surface_ok
        && reports
            .iter()
            .all(|r| r.adjusted_ratio >= WCAG_AA_MIN_CONTRAST);
    Ok((all_pass, reports))
}

fn validate_single_mode(
    mode: &BrandModeTokens,
) -> Result<(BrandModeTokens, Vec<OklchContrastReport>), HtmlForgeError> {
    if !is_valid_theme_mode_id(&mode.mode_id) {
        return Err(HtmlForgeError::InvalidPayload(format!(
            "Invalid theme modeId '{}'",
            mode.mode_id
        )));
    }
    let label = mode.label.trim();
    if label.is_empty() || label.len() > 48 {
        return Err(HtmlForgeError::InvalidPayload(format!(
            "Invalid label for mode '{}'",
            mode.mode_id
        )));
    }
    let mut clean_tokens = BTreeMap::new();
    for (k, v) in &mode.tokens {
        if !is_valid_css_token_name(k) {
            return Err(HtmlForgeError::CssTokenBlocked(format!("invalid-key:{k}")));
        }
        clean_tokens.insert(k.clone(), sanitize_token_value(k, v)?);
    }
    let (verified, reports) = enforce_mode_oklch_contrast(&mut clean_tokens)?;
    Ok((
        BrandModeTokens {
            mode_id: mode.mode_id.clone(),
            label: label.to_string(),
            tokens: clean_tokens,
            wcag_aa_verified: verified,
        },
        reports,
    ))
}

fn validate_component_overrides(
    overrides: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<BTreeMap<String, BTreeMap<String, String>>, HtmlForgeError> {
    let mut clean = BTreeMap::new();
    for (kind, rules) in overrides {
        if !is_valid_theme_mode_id(kind) {
            return Err(HtmlForgeError::CssTokenBlocked(format!(
                "invalid-component-kind:{kind}"
            )));
        }
        let mut clean_rules = BTreeMap::new();
        for (prop, val) in rules {
            if !is_valid_css_property_name(prop) {
                return Err(HtmlForgeError::CssTokenBlocked(format!(
                    "invalid-css-prop:{prop}"
                )));
            }
            let decl = format!("{prop}: {val}");
            let scrubbed = validate_and_scrub_css_tokens(&decl, true)?;
            if let Some((_, normalized_val)) = scrubbed.split_once(':') {
                clean_rules.insert(prop.to_ascii_lowercase(), normalized_val.trim().to_string());
            }
        }
        clean.insert(kind.clone(), clean_rules);
    }
    Ok(clean)
}

/// Validates and sanitizes a `BrandProposalV1` fail-closed.
/// Strictly preserves user-requested `theme_modes` without injecting Light/Dark defaults.
pub fn validate_brand_proposal(
    proposal: &BrandProposalV1,
) -> Result<(BrandProposalV1, BTreeMap<String, Vec<OklchContrastReport>>), HtmlForgeError> {
    if proposal.id.trim().is_empty() || proposal.id.len() > 64 {
        return Err(HtmlForgeError::InvalidPayload(
            "proposal.id must be 1..=64 chars".to_string(),
        ));
    }
    if proposal.theme_modes.is_empty() || proposal.theme_modes.len() > MAX_THEME_MODES {
        return Err(HtmlForgeError::InvalidPayload(format!(
            "themeModes must contain 1..={MAX_THEME_MODES} modes"
        )));
    }
    if proposal.inspiration_refs.len() > MAX_INSPIRATION_REFS {
        return Err(HtmlForgeError::InvalidPayload(format!(
            "inspirationRefs exceeds max {MAX_INSPIRATION_REFS}"
        )));
    }
    let font_pairing = FontPairing {
        heading: validate_safe_font_stack(&proposal.font_pairing.heading, "heading")?,
        body: validate_safe_font_stack(&proposal.font_pairing.body, "body")?,
        mono: validate_safe_font_stack(&proposal.font_pairing.mono, "mono")?,
    };
    let mut seen_modes = HashSet::new();
    let mut clean_modes = Vec::with_capacity(proposal.theme_modes.len());
    let mut reports_by_mode = BTreeMap::new();
    for mode in &proposal.theme_modes {
        if !seen_modes.insert(mode.mode_id.clone()) {
            return Err(HtmlForgeError::InvalidPayload(format!(
                "Duplicate theme mode '{}'",
                mode.mode_id
            )));
        }
        let (clean_mode, reports) = validate_single_mode(mode)?;
        reports_by_mode.insert(clean_mode.mode_id.clone(), reports);
        clean_modes.push(clean_mode);
    }
    let clean_overrides = validate_component_overrides(&proposal.component_style_overrides)?;
    Ok((
        BrandProposalV1 {
            id: proposal.id.trim().to_string(),
            brand_name: proposal.brand_name.trim().to_string(),
            rationale: proposal.rationale.trim().to_string(),
            inspiration_refs: proposal.inspiration_refs.clone(),
            theme_modes: clean_modes,
            font_pairing,
            radius_scale: proposal.radius_scale,
            density: proposal.density,
            component_style_overrides: clean_overrides,
        },
        reports_by_mode,
    ))
}

fn classify_sketch_role(page: &SketchPageInput) -> &'static str {
    let haystack = format!(
        "{} {} {}",
        page.page_role.to_ascii_lowercase(),
        page.title.to_ascii_lowercase(),
        page.screen_id.to_ascii_lowercase()
    );
    if haystack.contains("form")
        || haystack.contains("action")
        || haystack.contains("checkout")
        || haystack.contains("create")
        || page.html.to_ascii_lowercase().contains("<form")
    {
        return "action_form";
    }
    if haystack.contains("list")
        || haystack.contains("detail")
        || haystack.contains("table")
        || haystack.contains("catalog")
        || page.html.to_ascii_lowercase().contains("<table")
    {
        return "list_detail";
    }
    "hero"
}

/// Deterministically caps representative Sketch pages to `<= 3` (`Hero`, `List/Detail`, `Action Form`)
/// when `sketch_pages.len() > 3` (especially `> 20` screens per `AC-06`).
pub fn select_representative_sketch_pages(
    sketch_pages: &[SketchPageInput],
) -> (Vec<(&SketchPageInput, &'static str)>, bool) {
    if sketch_pages.len() <= MAX_REPRESENTATIVE_SKETCH_PAGES {
        let items = sketch_pages
            .iter()
            .map(|p| (p, classify_sketch_role(p)))
            .collect();
        return (items, false);
    }
    let mut picked_idx: Vec<usize> = Vec::with_capacity(MAX_REPRESENTATIVE_SKETCH_PAGES);
    let mut picked_roles: Vec<&'static str> = Vec::with_capacity(MAX_REPRESENTATIVE_SKETCH_PAGES);
    for target_role in ["hero", "list_detail", "action_form"] {
        if let Some((idx, _)) = sketch_pages
            .iter()
            .enumerate()
            .find(|(i, p)| !picked_idx.contains(i) && classify_sketch_role(p) == target_role)
        {
            picked_idx.push(idx);
            picked_roles.push(target_role);
        }
    }
    for (idx, p) in sketch_pages.iter().enumerate() {
        if picked_idx.len() >= MAX_REPRESENTATIVE_SKETCH_PAGES {
            break;
        }
        if !picked_idx.contains(&idx) {
            picked_idx.push(idx);
            picked_roles.push(classify_sketch_role(p));
        }
    }
    let selected = picked_idx
        .into_iter()
        .zip(picked_roles)
        .map(|(i, role)| (&sketch_pages[i], role))
        .collect();
    (selected, true)
}

fn radius_css_value(scale: RadiusScale) -> &'static str {
    match scale {
        RadiusScale::Sharp => "2px",
        RadiusScale::Balanced => "8px",
        RadiusScale::Soft => "14px",
        RadiusScale::Pill => "999px",
    }
}

fn density_spacing_css(density: DensityScale) -> (&'static str, &'static str) {
    match density {
        DensityScale::Airy => ("24px", "18px"),
        DensityScale::Comfortable => ("16px", "12px"),
        DensityScale::Compact => ("10px", "8px"),
    }
}

fn build_active_mode_style_css(
    proposal: &BrandProposalV1,
    active_mode: &BrandModeTokens,
) -> Result<String, HtmlForgeError> {
    let (pad, gap) = density_spacing_css(proposal.density);
    let radius = radius_css_value(proposal.radius_scale);
    let mut vars = String::new();
    for (k, v) in &active_mode.tokens {
        vars.push_str(&format!("  {k}: {v};\n"));
    }
    vars.push_str(&format!("  --brand-radius: {radius};\n"));
    vars.push_str(&format!("  --brand-pad: {pad};\n"));
    vars.push_str(&format!("  --brand-gap: {gap};\n"));
    vars.push_str(&format!(
        "  --font-heading: {};\n",
        proposal.font_pairing.heading
    ));
    vars.push_str(&format!("  --font-body: {};\n", proposal.font_pairing.body));
    vars.push_str(&format!("  --font-mono: {};\n", proposal.font_pairing.mono));

    let mut overrides_css = String::new();
    for (kind, rules) in &proposal.component_style_overrides {
        let mut decls = String::new();
        for (prop, val) in rules {
            decls.push_str(&format!(" {prop}: {val};"));
        }
        overrides_css.push_str(&format!("[data-component-kind=\"{kind}\"] {{{decls} }}\n"));
    }

    let css = format!(
        ":root[data-brand-mode=\"{mode}\"], [data-brand-mode=\"{mode}\"] {{\n{vars}}}\n\
         body {{ background-color: var(--bg-canvas, var(--bg-surface, #FFFFFF)); color: var(--text-primary, #111111); font-family: var(--font-body); margin: 0; padding: var(--brand-pad); }}\n\
         h1, h2, h3 {{ font-family: var(--font-heading); }}\n\
         code, pre {{ font-family: var(--font-mono); }}\n\
         .card, section {{ background-color: var(--bg-surface, #FFFFFF); border: 1px solid var(--border-default, #D4D1C7); border-radius: var(--brand-radius); padding: var(--brand-pad); }}\n\
         button {{ background-color: var(--action-primary, #2563EB); color: var(--text-on-accent, #FFFFFF); border-radius: var(--brand-radius); border: 1px solid transparent; padding: 8px 14px; font-weight: 600; }}\n\
         {overrides_css}",
        mode = active_mode.mode_id
    );
    validate_and_scrub_css_tokens(&css, false)
}

fn inject_brand_style_into_html(
    sanitized_html: &str,
    proposal_id: &str,
    mode_id: &str,
    style_css: &str,
) -> String {
    let safe_prop = escape_html_attr(proposal_id);
    let safe_mode = escape_html_attr(mode_id);
    let style_tag = format!(
        "<style data-brand-proposal=\"{safe_prop}\" data-brand-mode=\"{safe_mode}\">{style_css}</style>"
    );
    let with_mode = if sanitized_html.contains("<html") {
        sanitized_html.replacen(
            "<html",
            &format!("<html data-brand-mode=\"{safe_mode}\""),
            1,
        )
    } else {
        format!("<div data-brand-mode=\"{safe_mode}\">{sanitized_html}</div>")
    };
    if let Some(pos) = with_mode.find("</head>") {
        let mut out = String::with_capacity(with_mode.len() + style_tag.len());
        out.push_str(&with_mode[..pos]);
        out.push_str(&style_tag);
        out.push_str(&with_mode[pos..]);
        out
    } else {
        format!("{style_tag}{with_mode}")
    }
}

/// Renders `1× Semantic Sketch DOM × BrandProposalV1` for the requested `active_mode_id`
/// in `<5ms` with fail-closed CSS sanitization, OKLCH AA verification, and `<= 3` page cap.
pub fn render_tokenized_brand_preview(
    sketch_pages: &[SketchPageInput],
    proposal: &BrandProposalV1,
    active_mode_id: &str,
) -> Result<BrandPreviewBundle, HtmlForgeError> {
    let bundle_start = Instant::now();
    let (validated, reports_by_mode) = validate_brand_proposal(proposal)?;
    let active_mode = validated
        .theme_modes
        .iter()
        .find(|m| m.mode_id == active_mode_id)
        .ok_or_else(|| {
            HtmlForgeError::InvalidPayload(format!(
                "Requested mode '{active_mode_id}' is not in user-configured themeModes"
            ))
        })?;
    let style_css = build_active_mode_style_css(&validated, active_mode)?;
    let (selected_pages, capped) = select_representative_sketch_pages(sketch_pages);
    let mode_reports = reports_by_mode
        .get(active_mode_id)
        .cloned()
        .unwrap_or_default();

    let mut previews = Vec::with_capacity(selected_pages.len());
    for (page, resolved_role) in selected_pages {
        let page_start = Instant::now();
        let (sanitized, _) = sanitize_html_with_profile(&page.html, SanitizeProfile::AppPreview)?;
        let rendered =
            inject_brand_style_into_html(&sanitized, &validated.id, active_mode_id, &style_css);
        let sha = compute_sha256_hex(rendered.as_bytes());
        previews.push(TokenizedPagePreview {
            screen_id: page.screen_id.clone(),
            title: page.title.clone(),
            page_role: resolved_role.to_string(),
            mode_id: active_mode_id.to_string(),
            rendered_html: rendered,
            content_sha256: sha,
            wcag_aa_verified: active_mode.wcag_aa_verified,
            contrast_reports: mode_reports.clone(),
            render_time_micros: page_start.elapsed().as_micros() as u64,
        });
    }

    let requested_modes = validated
        .theme_modes
        .iter()
        .map(|m| m.mode_id.clone())
        .collect();
    Ok(BrandPreviewBundle {
        proposal_id: validated.id,
        active_mode_id: active_mode_id.to_string(),
        requested_modes,
        total_sketch_pages: sketch_pages.len(),
        capped_to_representative: capped,
        pages: previews,
        total_render_time_micros: bundle_start.elapsed().as_micros() as u64,
    })
}

fn palette_for_mode(mode_id: &str, accent_hex: &str, dark_accent_hex: &str) -> BrandModeTokens {
    let mut tokens = BTreeMap::new();
    let (label, bg_canvas, bg_surface, text_primary, border, accent, on_accent) = match mode_id {
        "dark" => (
            "Dark",
            "#0D1016",
            "#151923",
            "#F3F4F6",
            "#2A3140",
            dark_accent_hex,
            "#0B0E14",
        ),
        "night" => (
            "Night",
            "#0A0C10",
            "#13161E",
            "#F5F5F4",
            "#282D3A",
            dark_accent_hex,
            "#0A0C10",
        ),
        "dim" => (
            "Dim",
            "#181B22",
            "#222631",
            "#ECEEF2",
            "#353C4B",
            dark_accent_hex,
            "#11141B",
        ),
        "warm" => (
            "Warm", "#F6F2EA", "#FDFBF7", "#23211D", "#DDD6C7", accent_hex, "#FFFFFF",
        ),
        "high-contrast" => (
            "High Contrast",
            "#FFFFFF",
            "#FFFFFF",
            "#000000",
            "#000000",
            "#0038A8",
            "#FFFFFF",
        ),
        other => (
            if other == "light" { "Light" } else { other },
            "#F8FAFC",
            "#FFFFFF",
            "#111827",
            "#E2E8F0",
            accent_hex,
            "#FFFFFF",
        ),
    };
    tokens.insert("--bg-canvas".to_string(), bg_canvas.to_string());
    tokens.insert("--bg-surface".to_string(), bg_surface.to_string());
    tokens.insert("--text-primary".to_string(), text_primary.to_string());
    tokens.insert("--border-default".to_string(), border.to_string());
    tokens.insert("--action-primary".to_string(), accent.to_string());
    tokens.insert("--text-on-accent".to_string(), on_accent.to_string());
    BrandModeTokens {
        mode_id: mode_id.to_string(),
        label: label.to_string(),
        tokens,
        wcag_aa_verified: true,
    }
}

/// Generates 3 deterministic `BrandProposalV1` proposals strictly matching `requested_modes`.
pub fn generate_default_brand_proposals(
    reference_apps: &[String],
    requested_modes: &[String],
    locale: &str,
) -> Result<Vec<BrandProposalV1>, HtmlForgeError> {
    if requested_modes.is_empty() || requested_modes.len() > MAX_THEME_MODES {
        return Err(HtmlForgeError::InvalidPayload(format!(
            "requested_modes must contain 1..={MAX_THEME_MODES} modes"
        )));
    }
    for m in requested_modes {
        if !is_valid_theme_mode_id(m) {
            return Err(HtmlForgeError::InvalidPayload(format!(
                "Invalid theme mode '{m}'"
            )));
        }
    }
    let refs: Vec<String> = if reference_apps.is_empty() {
        vec!["Linear".to_string(), "Stripe".to_string()]
    } else {
        reference_apps
            .iter()
            .take(MAX_INSPIRATION_REFS)
            .cloned()
            .collect()
    };
    let is_vi = locale.eq_ignore_ascii_case("vi");
    let presets = [
        (
            "brand-atelier-precision",
            if is_vi {
                "Atelier Chuẩn Xác"
            } else {
                "Precision Atelier"
            },
            if is_vi {
                "Nhấn mạnh hệ phân cấp rõ ràng, đường biên 1px sắc nét và khoảng thở cân bằng cho màn hình nghiệp vụ."
            } else {
                "Emphasizes crisp 1px hierarchy, tabular clarity, and balanced rhythm for high-trust operational flows."
            },
            "#1D4ED8",
            "#93C5FD",
            RadiusScale::Balanced,
            DensityScale::Comfortable,
        ),
        (
            "brand-editorial-warmth",
            if is_vi {
                "Biên Tập Ấm Áp"
            } else {
                "Editorial Studio"
            },
            if is_vi {
                "Bề mặt ấm dịu mắt kết hợp chữ tiêu đề sắc sảo, tối ưu trải nghiệm đọc lâu dài và trình bày."
            } else {
                "Warm tactile surfaces paired with refined typography for sustained reading and executive storytelling."
            },
            "#0F766E",
            "#5EEAD4",
            RadiusScale::Soft,
            DensityScale::Airy,
        ),
        (
            "brand-kinetic-signal",
            if is_vi {
                "Tín Hiệu Mật Độ Cao"
            } else {
                "Kinetic Signal"
            },
            if is_vi {
                "Bố cục gọn mật độ cao, góc bo dứt khoát và điểm nhấn tương phản mạnh cho thao tác tốc độ."
            } else {
                "High-density layout with sharp geometry and unmistakable action contrast for rapid operator workflows."
            },
            "#B45309",
            "#FCD34D",
            RadiusScale::Sharp,
            DensityScale::Compact,
        ),
    ];

    let mut out = Vec::with_capacity(presets.len());
    for (id, name, rationale, accent, dark_accent, radius, density) in presets {
        let modes = requested_modes
            .iter()
            .map(|m| palette_for_mode(m, accent, dark_accent))
            .collect();
        let raw = BrandProposalV1 {
            id: id.to_string(),
            brand_name: name.to_string(),
            rationale: rationale.to_string(),
            inspiration_refs: refs.clone(),
            theme_modes: modes,
            font_pairing: FontPairing {
                heading: "Inter, system-ui, sans-serif".to_string(),
                body: "Inter, system-ui, sans-serif".to_string(),
                mono: "JetBrains Mono, monospace".to_string(),
            },
            radius_scale: radius,
            density,
            component_style_overrides: BTreeMap::new(),
        };
        let (validated, _) = validate_brand_proposal(&raw)?;
        out.push(validated);
    }
    Ok(out)
}
