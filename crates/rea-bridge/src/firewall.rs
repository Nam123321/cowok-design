// @story 09.1
// @implements AC-02, AC-03 (License/ToS Firewall, SSRF Guard & Design-Only Secret/SQL Sanitizer)
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

/// Prohibited infrastructure ports that must never be reached by SSRF or reference fetches.
pub const PROHIBITED_PORTS: &[u16] = &[
    3689, 5000, 6333, 6334, 6379, 6380, 6381, 6382, 6383, 6390, 7000, 11434, 54321, 54322, 54323,
    54324,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FirewallVerdict {
    #[serde(rename = "ADOPT_PRIOR_ART")]
    AdoptPriorArt,
    #[serde(rename = "REFERENCE_ONLY")]
    ReferenceOnly,
    #[serde(rename = "REJECT")]
    Reject,
}

impl FirewallVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AdoptPriorArt => "ADOPT_PRIOR_ART",
            Self::ReferenceOnly => "REFERENCE_ONLY",
            Self::Reject => "REJECT",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw {
            "ADOPT_PRIOR_ART" => Some(Self::AdoptPriorArt),
            "REFERENCE_ONLY" => Some(Self::ReferenceOnly),
            "REJECT" => Some(Self::Reject),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirewallDecision {
    pub verdict: FirewallVerdict,
    pub normalized_license: String,
    pub reason: String,
    pub policy_rule: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FirewallError {
    #[error("Invalid URL syntax: {0}")]
    InvalidUrl(String),
    #[error("Disallowed URL scheme '{0}': only http and https are permitted")]
    DisallowedScheme(String),
    #[error("Embedded URL credentials are forbidden")]
    CredentialsInUrl,
    #[error("SSRF blocked host '{host}': {reason}")]
    SsrfBlockedHost { host: String, reason: String },
    #[error("SSRF blocked prohibited port {port}")]
    ProhibitedPort { port: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SanitizationReport {
    pub sanitized_text: String,
    pub sanitized_secrets_count: usize,
    pub stripped_categories: Vec<String>,
}

const TOS_REJECT_SIGNALS: &[(&str, &str)] = &[
    ("bypass tos", "ToS bypass directive detected"),
    (
        "anti scraping bypass",
        "Anti-scraping bypass directive detected",
    ),
    (
        "scrape behind login",
        "Authenticated scraping bypass detected",
    ),
    (
        "scraping prohibited",
        "Source ToS explicitly prohibits scraping",
    ),
    (
        "scraping is prohibited",
        "Source ToS explicitly prohibits scraping",
    ),
    ("do not scrape", "Source ToS explicitly prohibits scraping"),
    (
        "credential exfil",
        "Credential exfiltration signal detected",
    ),
    (
        "exfiltrate credentials",
        "Credential exfiltration signal detected",
    ),
    (
        "steal cookies",
        "Cookie theft / session hijack signal detected",
    ),
    ("session hijack", "Session hijack signal detected"),
    (
        "confidential do not distribute",
        "Confidential proprietary source detected",
    ),
    (
        "all rights reserved",
        "Proprietary all-rights-reserved source detected",
    ),
];

const CONTENT_LICENSE_REJECT_SIGNALS: &[(&str, &str)] = &[
    (
        "affero general public license",
        "Copyleft AGPL license detected in source content",
    ),
    (
        "agpl 3.0",
        "Copyleft AGPL license detected in source content",
    ),
    (
        "agpl 2.0",
        "Copyleft AGPL license detected in source content",
    ),
    (
        "gpl 3.0",
        "Strong copyleft GPL license detected in source content",
    ),
    (
        "gpl 2.0",
        "Strong copyleft GPL license detected in source content",
    ),
    (
        "sspl 1.0",
        "Source-available restrictive license detected in source content",
    ),
    (
        "bsl 1.1",
        "Source-available restrictive BSL license detected in source content",
    ),
    (
        "busl 1.1",
        "Source-available restrictive BUSL license detected in source content",
    ),
];

const CONTENT_REFERENCE_ONLY_SIGNALS: &[&str] = &[
    "gnu lesser general public license",
    "gnu library general public license",
    "lgpl 2.1",
    "lgpl 3.0",
    "mozilla public license",
    "mpl 2.0",
    "cc by 4.0",
    "cc by 3.0",
    "for reference only",
];

const ADOPT_LICENSES: &[&str] = &[
    "MIT",
    "APACHE-2.0",
    "APACHE 2.0",
    "BSD-2-CLAUSE",
    "BSD-3-CLAUSE",
    "ISC",
    "CC0-1.0",
    "0BSD",
    "UNLICENSE",
];

fn normalize_signal_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_was_space = true;
    for ch in raw.chars() {
        if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
            last_was_space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

fn has_license_token(upper: &str, needle: &str) -> bool {
    upper
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|tok| tok == needle)
}

fn is_adopt_atom(atom: &str) -> bool {
    let cleaned = atom.trim().trim_matches(|c| c == '(' || c == ')').trim();
    ADOPT_LICENSES.contains(&cleaned)
}

fn is_adopt_expression(normalized: &str) -> bool {
    let conjuncts: Vec<&str> = normalized.split(" AND ").collect();
    !conjuncts.is_empty()
        && conjuncts
            .iter()
            .all(|conjunct| conjunct.split(" OR ").any(is_adopt_atom))
}

fn is_reject_license(upper: &str) -> Option<&'static str> {
    let norm_spaces = normalize_signal_text(upper).to_ascii_uppercase();
    if upper.contains("AGPL") || upper.contains("AFFERO") {
        return Some("Copyleft AGPL/Affero license is rejected by Design-Only Firewall");
    }
    if upper.contains("SSPL") || upper.contains("BUSL") || has_license_token(upper, "BSL") {
        return Some("Source-available restrictive license is rejected by Firewall");
    }
    // Check GPL / GENERAL PUBLIC LICENSE without matching LGPL / LESSER / LIBRARY GPL
    let stripped_lgpl = upper.replace("LGPL", "");
    let stripped_lesser_gpl = norm_spaces
        .replace("LESSER GENERAL PUBLIC LICENSE", "")
        .replace("LIBRARY GENERAL PUBLIC LICENSE", "");
    if stripped_lgpl.contains("GPL") || stripped_lesser_gpl.contains("GENERAL PUBLIC LICENSE") {
        return Some("Strong copyleft GPL license is rejected by Design-Only Firewall");
    }
    if upper.contains("PROPRIETARY")
        || norm_spaces.contains("ALL RIGHTS RESERVED")
        || norm_spaces.contains("COMMERCIAL ONLY")
        || norm_spaces.contains("CLOSED SOURCE")
        || upper.contains("RESTRICTED")
        || has_license_token(upper, "UNLICENSED")
    {
        return Some("Proprietary or all-rights-reserved source is rejected by Firewall");
    }
    None
}

fn check_content_license_reject(normalized_content: &str) -> Option<&'static str> {
    let stripped_lesser = normalized_content
        .replace("lesser general public license", "")
        .replace("library general public license", "");
    if stripped_lesser.contains("gnu general public license") {
        return Some("Strong copyleft GPL license detected in source content");
    }
    for (needle, reason) in CONTENT_LICENSE_REJECT_SIGNALS {
        if normalized_content.contains(needle) {
            return Some(reason);
        }
    }
    None
}

fn has_content_reference_only_signal(normalized_content: &str) -> bool {
    CONTENT_REFERENCE_ONLY_SIGNALS
        .iter()
        .any(|needle| normalized_content.contains(needle))
}

fn is_reference_only_license(upper: &str) -> bool {
    upper.is_empty()
        || upper == "UNKNOWN"
        || upper == "UNKNOWN_PUBLIC"
        || upper.starts_with("LGPL")
        || upper.starts_with("MPL")
        || upper.starts_with("CC-BY")
        || upper.starts_with("EPL")
        || upper.starts_with("CDDL")
}

/// Evaluates License SPDX + URL + content signals through the License/ToS Firewall.
pub fn evaluate_license_and_tos(
    license_spdx: &str,
    source_url: &str,
    content_hint: &str,
) -> FirewallDecision {
    let combined_normalized = normalize_signal_text(&format!("{source_url}\n{content_hint}"));
    for (needle, reason) in TOS_REJECT_SIGNALS {
        if combined_normalized.contains(needle) {
            return FirewallDecision {
                verdict: FirewallVerdict::Reject,
                normalized_license: normalize_license_label(license_spdx),
                reason: (*reason).to_string(),
                policy_rule: "TOS_FIREWALL_BLOCK".to_string(),
            };
        }
    }

    let normalized_content = normalize_signal_text(content_hint);
    if let Some(reason) = check_content_license_reject(&normalized_content) {
        return FirewallDecision {
            verdict: FirewallVerdict::Reject,
            normalized_license: normalize_license_label(license_spdx),
            reason: reason.to_string(),
            policy_rule: "CONTENT_LICENSE_FIREWALL_REJECT".to_string(),
        };
    }

    let normalized = normalize_license_label(license_spdx);
    if let Some(reject_reason) = is_reject_license(&normalized) {
        return FirewallDecision {
            verdict: FirewallVerdict::Reject,
            normalized_license: normalized,
            reason: reject_reason.to_string(),
            policy_rule: "LICENSE_FIREWALL_REJECT".to_string(),
        };
    }

    if is_adopt_expression(&normalized) {
        if has_content_reference_only_signal(&normalized_content) {
            return FirewallDecision {
                verdict: FirewallVerdict::ReferenceOnly,
                normalized_license: normalized,
                reason:
                    "Archive or content contains weak-copyleft or reference-only notice; downgraded to REFERENCE_ONLY"
                        .to_string(),
                policy_rule: "CONTENT_LICENSE_REFERENCE_ONLY".to_string(),
            };
        }
        return FirewallDecision {
            verdict: FirewallVerdict::AdoptPriorArt,
            normalized_license: normalized,
            reason: "Permissive open-source license cleared for ADOPT_PRIOR_ART".to_string(),
            policy_rule: "LICENSE_PERMISSIVE_ADOPT".to_string(),
        };
    }

    if is_reference_only_license(&normalized) {
        return FirewallDecision {
            verdict: FirewallVerdict::ReferenceOnly,
            normalized_license: if normalized.is_empty() {
                "UNKNOWN_PUBLIC".to_string()
            } else {
                normalized
            },
            reason: "Weak-copyleft or public-web reference restricted to REFERENCE_ONLY"
                .to_string(),
            policy_rule: "LICENSE_REFERENCE_ONLY".to_string(),
        };
    }

    FirewallDecision {
        verdict: FirewallVerdict::ReferenceOnly,
        normalized_license: normalized,
        reason: "Unrecognized non-proprietary license downgraded to REFERENCE_ONLY".to_string(),
        policy_rule: "LICENSE_UNKNOWN_REFERENCE_ONLY".to_string(),
    }
}

fn normalize_license_label(raw: &str) -> String {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "UNKNOWN_PUBLIC".to_string()
    } else {
        collapsed.to_ascii_uppercase()
    }
}

/// Checks whether a port is prohibited by the workspace isolation policy.
pub fn is_prohibited_port(port: u16) -> bool {
    PROHIBITED_PORTS.contains(&port)
}

/// Checks whether an IP address is loopback, private RFC1918, link-local, ULA, or non-public.
pub fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_forbidden_ipv4(v4),
        IpAddr::V6(v6) => is_forbidden_ipv6(v6),
    }
}

fn is_forbidden_ipv4(v4: Ipv4Addr) -> bool {
    let [a, b, c, _] = v4.octets();
    a == 0
        || v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast()
        || v4.is_multicast()
        || v4.is_documentation()
        // CGNAT 100.64.0.0/10
        || (a == 100 && (64..=127).contains(&b))
        // Benchmarking 198.18.0.0/15
        || (a == 198 && (b == 18 || b == 19))
        // IETF protocol assignments 192.0.0.0/24
        || (a == 192 && b == 0 && c == 0)
        // Reserved 240.0.0.0/4
        || a >= 240
}

fn is_forbidden_ipv6(v6: Ipv6Addr) -> bool {
    if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
        return true;
    }
    if let Some(mapped) = v6.to_ipv4_mapped() {
        return is_forbidden_ipv4(mapped);
    }
    let segs = v6.segments();
    // IPv4-compatible IPv6 (::w.x.y.z, deprecated)
    if segs[0] == 0 && segs[1] == 0 && segs[2] == 0 && segs[3] == 0 && segs[4] == 0 && segs[5] == 0
    {
        let embedded = Ipv4Addr::new(
            (segs[6] >> 8) as u8,
            (segs[6] & 0xff) as u8,
            (segs[7] >> 8) as u8,
            (segs[7] & 0xff) as u8,
        );
        if is_forbidden_ipv4(embedded) {
            return true;
        }
    }
    let is_ula = (segs[0] & 0xfe00) == 0xfc00;
    let is_link_local = (segs[0] & 0xffc0) == 0xfe80;
    let is_site_local = (segs[0] & 0xffc0) == 0xfec0;
    // 2002::/16 (6to4)
    let is_6to4 = segs[0] == 0x2002;
    // ::ffff:0:0:0/96 (IPv4-translated, RFC 2765)
    let is_ipv4_translated = segs[0] == 0
        && segs[1] == 0
        && segs[2] == 0
        && segs[3] == 0
        && segs[4] == 0xffff
        && segs[5] == 0;
    // 64:ff9b:1::/48 (NAT64 local-use, RFC 8215)
    let is_nat64_local = segs[0] == 0x0064 && segs[1] == 0xff9b && segs[2] == 0x0001;
    // 2001::/23 (Teredo / IETF protocol assignments)
    let is_ietf_2001_23 = segs[0] == 0x2001 && (segs[1] & 0xfe00) == 0x0000;
    // 2001:db8::/32 (documentation)
    let is_documentation = segs[0] == 0x2001 && segs[1] == 0x0db8;
    // 0100::/64 (discard-only, RFC 6666)
    let is_discard_only = segs[0] == 0x0100 && segs[1] == 0 && segs[2] == 0 && segs[3] == 0;
    let is_nat64_well_known = segs[0] == 0x0064
        && segs[1] == 0xff9b
        && segs[2] == 0
        && segs[3] == 0
        && segs[4] == 0
        && segs[5] == 0;
    if is_nat64_well_known {
        let embedded = Ipv4Addr::new(
            (segs[6] >> 8) as u8,
            (segs[6] & 0xff) as u8,
            (segs[7] >> 8) as u8,
            (segs[7] & 0xff) as u8,
        );
        if is_forbidden_ipv4(embedded) {
            return true;
        }
    }
    is_ula
        || is_link_local
        || is_site_local
        || is_6to4
        || is_ipv4_translated
        || is_nat64_local
        || is_ietf_2001_23
        || is_documentation
        || is_discard_only
}

/// Validates a resolved `SocketAddr` against SSRF IP + prohibited port policies.
pub fn validate_socket_addr_ssrf(addr: SocketAddr) -> Result<(), FirewallError> {
    if is_prohibited_port(addr.port()) {
        return Err(FirewallError::ProhibitedPort { port: addr.port() });
    }
    if is_forbidden_ip(addr.ip()) {
        return Err(FirewallError::SsrfBlockedHost {
            host: addr.ip().to_string(),
            reason: "Non-public or internal IP address is forbidden".to_string(),
        });
    }
    Ok(())
}

/// Validates an outbound reference URL against SSRF, private IPs, IPv4-mapped IPv6, and prohibited ports.
pub fn validate_public_url_ssrf(raw_url: &str) -> Result<Url, FirewallError> {
    let trimmed = raw_url.trim();
    let parsed = Url::parse(trimmed).map_err(|e| FirewallError::InvalidUrl(e.to_string()))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(FirewallError::DisallowedScheme(scheme.to_string()));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(FirewallError::CredentialsInUrl);
    }
    let Some(host_str) = parsed.host_str() else {
        return Err(FirewallError::InvalidUrl("Missing host in URL".to_string()));
    };
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| FirewallError::InvalidUrl("Could not resolve port".to_string()))?;
    if is_prohibited_port(port) {
        return Err(FirewallError::ProhibitedPort { port });
    }

    let lower_host = host_str
        .trim_matches(['[', ']'])
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if lower_host.is_empty()
        || lower_host == "localhost"
        || lower_host.ends_with(".localhost")
        || lower_host.ends_with(".local")
        || lower_host.ends_with(".internal")
        || lower_host == "metadata"
        || lower_host == "metadata.goog"
        || lower_host.ends_with(".metadata.goog")
        || lower_host == "instance-data"
        || lower_host == "0.0.0.0"
    {
        return Err(FirewallError::SsrfBlockedHost {
            host: host_str.to_string(),
            reason: "Loopback, local, or cloud metadata hostname is forbidden".to_string(),
        });
    }

    if let Ok(ip) = lower_host.parse::<IpAddr>() {
        if is_forbidden_ip(ip) {
            return Err(FirewallError::SsrfBlockedHost {
                host: host_str.to_string(),
                reason: "Private, loopback, link-local, or ULA IP is forbidden".to_string(),
            });
        }
    } else if let Ok(int_ip) = lower_host.parse::<u32>() {
        let v4 = Ipv4Addr::from(int_ip);
        if is_forbidden_ipv4(v4) {
            return Err(FirewallError::SsrfBlockedHost {
                host: host_str.to_string(),
                reason: "Numeric IPv4 literal resolves to forbidden range".to_string(),
            });
        }
    }

    Ok(parsed)
}

struct SanitizerRule {
    category: &'static str,
    regex: Regex,
    replacement: &'static str,
}

fn sanitizer_rules() -> &'static [SanitizerRule] {
    static RULES: OnceLock<Vec<SanitizerRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        vec![
            SanitizerRule {
                category: "private_key",
                regex: Regex::new(
                    r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
                )
                .expect("valid private key regex"),
                replacement: "[REDACTED_PRIVATE_KEY]",
            },
            SanitizerRule {
                category: "api_key",
                regex: Regex::new(
                    r"\b(?:sk-(?:proj-|ant-)?[A-Za-z0-9_-]{12,}|(?:sk|rk)_live_[A-Za-z0-9]{12,}|AIza[0-9A-Za-z_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,})\b",
                )
                .expect("valid api key regex"),
                replacement: "[REDACTED_API_KEY]",
            },
            SanitizerRule {
                category: "jwt_or_bearer",
                regex: Regex::new(
                    r"(?i)\b(?:Bearer\s+[A-Za-z0-9._~+/-]{12,}=*|Authorization\s*:\s*Basic\s+[A-Za-z0-9+/=_-]{6,})|\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b",
                )
                .expect("valid jwt/bearer regex"),
                replacement: "[REDACTED_TOKEN]",
            },
            SanitizerRule {
                category: "db_connection_string",
                regex: Regex::new(
                    r"(?i)\b(?:postgres(?:ql)?|mysql|mongodb(?:\+srv)?|rediss?|amqps?)://[^\s'\x22<>]+",
                )
                .expect("valid db uri regex"),
                replacement: "[REDACTED_DB_URI]",
            },
            SanitizerRule {
                category: "sql_statement",
                regex: Regex::new(
                    r"(?is)\b(?:SELECT\s+[^;]{1,400}?\s+FROM\s+[a-zA-Z0-9_.\x22`]+|INSERT\s+INTO\s+[a-zA-Z0-9_.\x22`]+[^;]{0,400}|DELETE\s+FROM\s+[a-zA-Z0-9_.\x22`]+[^;]{0,400}|UPDATE\s+[a-zA-Z0-9_.\x22`]+\s+SET\s+[^;]{1,400}|DROP\s+TABLE\s+[a-zA-Z0-9_.\x22`]+)",
                )
                .expect("valid sql regex"),
                replacement: "[REDACTED_SQL]",
            },
            SanitizerRule {
                category: "internal_backend_url",
                regex: Regex::new(
                    r"(?i)(?:https?://(?:127\.0\.0\.1|0\.0\.0\.0|localhost|\[::1\]|\[f[cd][0-9a-f:]+\]|10\.\d{1,3}\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|169\.254\.\d{1,3}\.\d{1,3}|[a-z0-9-]+(?:\.[a-z0-9-]+)*\.(?:internal|local))(?::\d+)?[^\s'\x22<>]*|/api/admin(?:/[^\s'\x22<>]*)?|/internal/(?:admin|debug|vault|metrics)(?:/[^\s'\x22<>]*)?)",
                )
                .expect("valid internal endpoint regex"),
                replacement: "[REDACTED_INTERNAL_ENDPOINT]",
            },
        ]
    })
}

/// Strips SQL queries, API keys, Bearer/JWT tokens, DB URIs, and internal `/api/admin` endpoints.
pub fn sanitize_design_only_text(raw: &str) -> SanitizationReport {
    let mut current = raw.to_string();
    let mut total_count = 0usize;
    let mut categories: Vec<String> = Vec::new();

    for rule in sanitizer_rules() {
        let matches = rule.regex.find_iter(&current).count();
        if matches > 0 {
            total_count += matches;
            if !categories.iter().any(|c| c == rule.category) {
                categories.push(rule.category.to_string());
            }
            current = rule
                .regex
                .replace_all(&current, rule.replacement)
                .into_owned();
        }
    }

    SanitizationReport {
        sanitized_text: current,
        sanitized_secrets_count: total_count,
        stripped_categories: categories,
    }
}

/// Recursively sanitizes all string leaves and object keys inside a JSON value, accumulating secret counts.
pub fn sanitize_design_json(value: &serde_json::Value) -> (serde_json::Value, usize, Vec<String>) {
    let mut total = 0usize;
    let mut categories: Vec<String> = Vec::new();
    let sanitized = sanitize_json_inner(value, &mut total, &mut categories);
    (sanitized, total, categories)
}

fn sanitize_json_inner(
    value: &serde_json::Value,
    total: &mut usize,
    categories: &mut Vec<String>,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            let report = sanitize_design_only_text(s);
            *total += report.sanitized_secrets_count;
            for cat in report.stripped_categories {
                if !categories.iter().any(|c| c == &cat) {
                    categories.push(cat);
                }
            }
            serde_json::Value::String(report.sanitized_text)
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| sanitize_json_inner(item, total, categories))
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                let key_report = sanitize_design_only_text(k);
                *total += key_report.sanitized_secrets_count;
                for cat in key_report.stripped_categories {
                    if !categories.iter().any(|c| c == &cat) {
                        categories.push(cat);
                    }
                }
                out.insert(
                    key_report.sanitized_text,
                    sanitize_json_inner(v, total, categories),
                );
            }
            serde_json::Value::Object(out)
        }
        other => other.clone(),
    }
}
