// @story 02.1
// @implements AC-01, AC-03
use crate::types::{ContextAssemblyCommand, ContextCompilerError, Horizon, PortalId};
use domain::{Department, Portal, TaskHorizon, VisibilityPolicy};

const MAX_INTENT_CODE_POINTS: usize = 2000;

const PRIVILEGED_MARKERS: &[&str] = &[
    "audit_logs",
    "billing_subscriptions",
    "platform_secrets",
    "superadmin",
    "super_admin",
    "super-admin",
    "super admin",
    "master_key",
    "internal_admin_token",
    "infra_secret",
    "cluster_private",
    "p6_security_internal",
    "06_security_spec",
    "08_deployment_spec",
    "cross_tenant_admin",
    "root_vault_keys",
];

const COLLAPSED_MARKERS: &[&str] = &[
    "auditlogs",
    "billingsubscriptions",
    "platformsecrets",
    "superadmin",
    "masterkey",
    "internaladmintoken",
    "infrasecret",
    "clusterprivate",
    "p6securityinternal",
    "06securityspec",
    "08deploymentspec",
    "crosstenantadmin",
    "rootvaultkeys",
];

fn compose_latin_pair(base: char, mark: char) -> Option<char> {
    match (base, mark) {
        ('A', '\u{0300}') => Some('À'),
        ('a', '\u{0300}') => Some('à'),
        ('A', '\u{0301}') => Some('Á'),
        ('a', '\u{0301}') => Some('á'),
        ('A', '\u{0302}') => Some('Â'),
        ('a', '\u{0302}') => Some('â'),
        ('A', '\u{0303}') => Some('Ã'),
        ('a', '\u{0303}') => Some('ã'),
        ('A', '\u{0306}') => Some('Ă'),
        ('a', '\u{0306}') => Some('ă'),
        ('A', '\u{0309}') => Some('Ả'),
        ('a', '\u{0309}') => Some('ả'),
        ('A', '\u{0323}') => Some('Ạ'),
        ('a', '\u{0323}') => Some('ạ'),
        ('E', '\u{0300}') => Some('È'),
        ('e', '\u{0300}') => Some('è'),
        ('E', '\u{0301}') => Some('É'),
        ('e', '\u{0301}') => Some('é'),
        ('E', '\u{0302}') => Some('Ê'),
        ('e', '\u{0302}') => Some('ê'),
        ('E', '\u{0303}') => Some('Ẽ'),
        ('e', '\u{0303}') => Some('ẽ'),
        ('E', '\u{0309}') => Some('Ẻ'),
        ('e', '\u{0309}') => Some('ẻ'),
        ('E', '\u{0323}') => Some('Ẹ'),
        ('e', '\u{0323}') => Some('ẹ'),
        ('I', '\u{0300}') => Some('Ì'),
        ('i', '\u{0300}') => Some('ì'),
        ('I', '\u{0301}') => Some('Í'),
        ('i', '\u{0301}') => Some('í'),
        ('I', '\u{0303}') => Some('Ĩ'),
        ('i', '\u{0303}') => Some('ĩ'),
        ('I', '\u{0309}') => Some('Ỉ'),
        ('i', '\u{0309}') => Some('ỉ'),
        ('I', '\u{0323}') => Some('Ị'),
        ('i', '\u{0323}') => Some('ị'),
        ('O', '\u{0300}') => Some('Ò'),
        ('o', '\u{0300}') => Some('ò'),
        ('O', '\u{0301}') => Some('Ó'),
        ('o', '\u{0301}') => Some('ó'),
        ('O', '\u{0302}') => Some('Ô'),
        ('o', '\u{0302}') => Some('ô'),
        ('O', '\u{0303}') => Some('Õ'),
        ('o', '\u{0303}') => Some('õ'),
        ('O', '\u{0309}') => Some('Ỏ'),
        ('o', '\u{0309}') => Some('ỏ'),
        ('O', '\u{031B}') => Some('Ơ'),
        ('o', '\u{031B}') => Some('ơ'),
        ('O', '\u{0323}') => Some('Ọ'),
        ('o', '\u{0323}') => Some('ọ'),
        ('U', '\u{0300}') => Some('Ù'),
        ('u', '\u{0300}') => Some('ù'),
        ('U', '\u{0301}') => Some('Ú'),
        ('u', '\u{0301}') => Some('ú'),
        ('U', '\u{0303}') => Some('Ũ'),
        ('u', '\u{0303}') => Some('ũ'),
        ('U', '\u{0309}') => Some('Ủ'),
        ('u', '\u{0309}') => Some('ủ'),
        ('U', '\u{031B}') => Some('Ư'),
        ('u', '\u{031B}') => Some('ư'),
        ('U', '\u{0323}') => Some('Ụ'),
        ('u', '\u{0323}') => Some('ụ'),
        ('Y', '\u{0300}') => Some('Ỳ'),
        ('y', '\u{0300}') => Some('ỳ'),
        ('Y', '\u{0301}') => Some('Ý'),
        ('y', '\u{0301}') => Some('ý'),
        ('Y', '\u{0303}') => Some('Ỹ'),
        ('y', '\u{0303}') => Some('ỹ'),
        ('Y', '\u{0309}') => Some('Ỷ'),
        ('y', '\u{0309}') => Some('ỷ'),
        ('Y', '\u{0323}') => Some('Ỵ'),
        ('y', '\u{0323}') => Some('ỵ'),
        ('D', '\u{0335}') => Some('Đ'),
        ('d', '\u{0335}') => Some('đ'),
        ('Â', '\u{0301}') => Some('Ấ'),
        ('â', '\u{0301}') => Some('ấ'),
        ('Â', '\u{0300}') => Some('Ầ'),
        ('â', '\u{0300}') => Some('ầ'),
        ('Â', '\u{0323}') => Some('Ậ'),
        ('â', '\u{0323}') => Some('ậ'),
        ('Ê', '\u{0301}') => Some('Ế'),
        ('ê', '\u{0301}') => Some('ế'),
        ('Ê', '\u{0300}') => Some('Ề'),
        ('ê', '\u{0300}') => Some('ề'),
        ('Ê', '\u{0323}') => Some('Ệ'),
        ('ê', '\u{0323}') => Some('ệ'),
        ('Ô', '\u{0301}') => Some('Ố'),
        ('ô', '\u{0301}') => Some('ố'),
        ('Ô', '\u{0300}') => Some('Ồ'),
        ('ô', '\u{0300}') => Some('ồ'),
        ('Ô', '\u{0323}') => Some('Ộ'),
        ('ô', '\u{0323}') => Some('ộ'),
        _ => None,
    }
}

fn canonicalize_to_nfc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if let Some(prev) = out.chars().last() {
            if let Some(composed) = compose_latin_pair(prev, ch) {
                let last_len = prev.len_utf8();
                out.truncate(out.len() - last_len);
                out.push(composed);
                continue;
            }
        }
        out.push(ch);
    }
    out
}

fn is_ignored_security_char(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{00AD}'
            | '\u{034F}'
            | '\u{180E}'
            | '\u{0300}'..='\u{036F}'
    ) || (c.is_control() && c != '\n' && c != '\r' && c != '\t')
}

fn fold_fullwidth_and_accented(c: char) -> char {
    if ('\u{FF01}'..='\u{FF5E}').contains(&c) {
        return char::from_u32((c as u32) - 0xFEE0).unwrap_or(c);
    }
    if c == '\u{3000}' {
        return ' ';
    }
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ă' | 'ạ' | 'ả' | 'ấ' | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' | 'ằ'
        | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'ā' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ẽ' | 'ẻ' | 'ẹ' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => {
            'e'
        }
        'ğ' | 'ĝ' | 'ġ' | 'ģ' => 'g',
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' | 'ĩ' | 'ỉ' | 'ị' => 'i',
        'ķ' => 'k',
        'ł' | 'ĺ' | 'ľ' | 'ļ' => 'l',
        'ñ' | 'ń' | 'ň' | 'ņ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ơ' | 'ỏ' | 'ọ' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ'
        | 'ớ' | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
        'ř' | 'ŕ' | 'ŗ' => 'r',
        'ś' | 'š' | 'ş' | 'ŝ' => 's',
        'ţ' | 'ť' | 'ŧ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ũ' | 'ư' | 'ủ' | 'ụ' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => {
            'u'
        }
        'ý' | 'ÿ' | 'ỳ' | 'ỹ' | 'ỷ' | 'ỵ' => 'y',
        'ž' | 'ź' | 'ż' => 'z',
        other => other,
    }
}

fn map_confusable_ascii(c: char) -> char {
    let folded = fold_fullwidth_and_accented(c);
    match folded {
        '\u{0430}' | '\u{03B1}' => 'a',
        '\u{0432}' | '\u{03B2}' | '\u{044C}' | '\u{044A}' | '\u{0184}' => 'b',
        '\u{0441}' | '\u{03F2}' | '\u{03C2}' => 'c',
        '\u{0501}' | '\u{0434}' => 'd',
        '\u{0435}' | '\u{0454}' | '\u{03B5}' => 'e',
        '\u{0444}' => 'f',
        '\u{0433}' | '\u{0491}' | '\u{0261}' => 'g',
        '\u{04BB}' | '\u{043D}' | '\u{03B7}' => 'h',
        '\u{0456}' | '\u{03B9}' | '\u{04CF}' => 'i',
        '\u{0458}' => 'j',
        '\u{043A}' | '\u{03BA}' => 'k',
        '\u{043B}' => 'l',
        '\u{043C}' | '\u{03BC}' => 'm',
        '\u{03BD}' | '\u{043F}' => 'n',
        '\u{043E}' | '\u{03BF}' => 'o',
        '\u{0440}' | '\u{03C1}' => 'p',
        '\u{051B}' => 'q',
        '\u{0455}' => 's',
        '\u{0442}' | '\u{03C4}' => 't',
        '\u{03C5}' => 'u',
        '\u{0475}' => 'v',
        '\u{051D}' => 'w',
        '\u{0443}' | '\u{04AF}' => 'y',
        '\u{0445}' | '\u{03C7}' => 'x',
        '\u{0437}' => 'z',
        other => other,
    }
}

pub fn normalize_intent_nfc(raw: &str) -> Result<String, ContextCompilerError> {
    let nfc = canonicalize_to_nfc(raw.trim());
    if nfc.trim().is_empty() {
        return Err(ContextCompilerError::InvalidRequest {
            message: "intent must not be blank".to_string(),
            field: Some("intent".to_string()),
        });
    }
    if nfc.chars().count() > MAX_INTENT_CODE_POINTS {
        return Err(ContextCompilerError::InvalidRequest {
            message: format!("intent exceeds {MAX_INTENT_CODE_POINTS} characters"),
            field: Some("intent".to_string()),
        });
    }
    Ok(nfc)
}

pub fn map_portal_and_department(portal: PortalId) -> (Portal, Department) {
    match portal {
        PortalId::SuperAdmin => (Portal::SuperAdmin, Department::ExecutiveReport),
        PortalId::TenantAdmin => (Portal::TenantAdmin, Department::ProductUi),
        PortalId::SalesApp => (Portal::SalesApp, Department::ProductUi),
        PortalId::MarketingWeb => (Portal::MarketingWeb, Department::MarketingDeck),
        PortalId::DeveloperPortal => (Portal::DeveloperPortal, Department::ProductUi),
        PortalId::Presentation => (Portal::PresentationStudio, Department::MarketingDeck),
    }
}

pub fn map_horizon(horizon: Horizon) -> TaskHorizon {
    match horizon {
        Horizon::Short => TaskHorizon::Short,
        Horizon::Mid => TaskHorizon::Mid,
        Horizon::Long => TaskHorizon::Long,
    }
}

pub fn contains_privileged_marker(text: &str) -> Option<&'static str> {
    let stripped: String = text
        .chars()
        .filter(|c| !is_ignored_security_char(*c))
        .map(map_confusable_ascii)
        .flat_map(char::to_lowercase)
        .map(map_confusable_ascii)
        .collect();
    if let Some(m) = PRIVILEGED_MARKERS
        .iter()
        .copied()
        .find(|m| stripped.contains(m))
    {
        return Some(m);
    }
    let collapsed: String = stripped.chars().filter(|c| c.is_alphanumeric()).collect();
    COLLAPSED_MARKERS
        .iter()
        .copied()
        .find(|m| collapsed.contains(m))
}

pub fn scan_text_for_portal(
    portal_id: PortalId,
    label: &str,
    text: &str,
) -> Result<(), ContextCompilerError> {
    if portal_id == PortalId::SuperAdmin {
        return Ok(());
    }
    if let Some(marker) = contains_privileged_marker(text) {
        return Err(ContextCompilerError::CrossPortalLeakBlocked {
            reason: format!(
                "portal '{}' denied marker '{marker}' in {label}",
                portal_id.as_str()
            ),
        });
    }
    VisibilityPolicy::apply_terminal_redaction(text, Portal::MarketingWeb).map_err(|err| {
        ContextCompilerError::CrossPortalLeakBlocked {
            reason: format!("{label}: {err}"),
        }
    })?;
    Ok(())
}

pub fn validate_portal_isolation(
    portal_id: PortalId,
    intent: &str,
    constraints: &[String],
) -> Result<(Portal, Department), ContextCompilerError> {
    let mapped = map_portal_and_department(portal_id);
    if portal_id != PortalId::SuperAdmin {
        scan_text_for_portal(portal_id, "intent", intent)?;
        for rule in constraints {
            scan_text_for_portal(portal_id, "constraint", rule)?;
        }
    }
    Ok(mapped)
}

pub fn validate_all_portal_inputs(
    cmd: &ContextAssemblyCommand,
) -> Result<(), ContextCompilerError> {
    if cmd.portal_id == PortalId::SuperAdmin {
        return Ok(());
    }
    scan_text_for_portal(cmd.portal_id, "intent", &cmd.intent)?;
    scan_text_for_portal(
        cmd.portal_id,
        "brand_tokens_prefix",
        &cmd.brand_tokens_prefix,
    )?;
    scan_text_for_portal(
        cmd.portal_id,
        "active_skill_prefix",
        &cmd.active_skill_prefix,
    )?;
    for rule in &cmd.extra_constraints {
        scan_text_for_portal(cmd.portal_id, "constraint", rule)?;
    }
    for blk in &cmd.optional_blocks {
        scan_text_for_portal(cmd.portal_id, "optional_block.id", &blk.id)?;
        scan_text_for_portal(cmd.portal_id, "optional_block.content", &blk.content)?;
    }
    for node in &cmd.dom_outline_nodes {
        scan_text_for_portal(cmd.portal_id, "dom_outline.ff_id", &node.ff_id)?;
        scan_text_for_portal(cmd.portal_id, "dom_outline.tag", &node.tag)?;
        if let Some(ref role) = node.role {
            scan_text_for_portal(cmd.portal_id, "dom_outline.role", role)?;
        }
        if let Some(ref summary) = node.summary {
            scan_text_for_portal(cmd.portal_id, "dom_outline.summary", summary)?;
        }
    }
    Ok(())
}
