// @story 08.2
// @implements AC-03, AC-05, AC-07
use crate::staging::{shadow_nfkc_fold, validate_staged_prompt_content};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepackageAssetBucket {
    Scripts,
    References,
    AssetsText,
    AssetsBinaryImage,
}

impl RepackageAssetBucket {
    pub fn target_subdir(self) -> &'static str {
        match self {
            Self::Scripts => "scripts",
            Self::References => "references",
            Self::AssetsText | Self::AssetsBinaryImage => "assets",
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepackageSecurityError {
    #[error("Detected dangerous command execution pattern: {pattern} at line {line}")]
    CommandInjectionTell { pattern: String, line: usize },
    #[error("Detected arbitrary code evaluation (eval / Function constructor) at line {line}")]
    ArbitraryCodeEvaluation { line: usize },
    #[error("Detected suspicious network socket or exfiltration call: {pattern} at line {line}")]
    UnauthorizedNetworkCall { pattern: String, line: usize },
    #[error("Detected path traversal or unsafe file access: {pattern} at line {line}")]
    PathTraversal { pattern: String, line: usize },
    #[error("Detected indirect prompt injection or exfiltration marker ({label}): {detail}")]
    PromptInjectionOrBeacon { label: String, detail: String },
    #[error("Rejected unsupported, non-UTF-8, or executable-spoofed asset '{filename}': {reason}")]
    UnsupportedOrBinaryAsset { filename: String, reason: String },
}

const RESERVED_ROOT_FILENAMES: &[&str] = &[
    "skill.md",
    "plugin.json",
    "discovery-stub.json",
    "skills-lock.json",
];

const SCRIPT_EXTENSIONS: &[&str] = &[
    "py", "js", "mjs", "cjs", "ts", "tsx", "jsx", "sh", "bash", "zsh", "rb", "pl",
];

const REFERENCE_EXTENSIONS: &[&str] = &["md", "markdown", "txt", "json", "yaml", "yml", "toml"];
const ASSET_TEXT_EXTENSIONS: &[&str] = &["svg", "html"];
const ASSET_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];
const BLOCKED_BINARY_EXTENSIONS: &[&str] = &[
    "so", "dylib", "dll", "exe", "node", "wasm", "bin", "o", "a", "zip", "tar", "gz", "bz2", "xz",
    "7z",
];

pub fn is_reserved_root_filename(rel_path: &str) -> bool {
    !rel_path.contains('/')
        && RESERVED_ROOT_FILENAMES
            .iter()
            .any(|name| rel_path.eq_ignore_ascii_case(name))
}

fn extension_lowercase(filename: &str) -> Option<String> {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let (stem, ext) = base.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

fn is_hidden_or_bidi(c: char) -> bool {
    matches!(
        c as u32,
        0x200B..=0x200F | 0x2028..=0x202E | 0x2066..=0x2069 | 0xFEFF | 0xE0001..=0xE007F
    )
}

fn strip_hidden_and_join_continuations(content: &str) -> String {
    let cleaned: String = content.chars().filter(|&c| !is_hidden_or_bidi(c)).collect();
    let unified = cleaned.replace("\r\n", "\n").replace('\r', "\n");
    let without_bs_newlines = unified.replace("\\\n", "");
    fold_multiline_string_concat(&without_bs_newlines)
}

fn fold_multiline_string_concat(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if (c == '\'' || c == '"' || c == '`') && i + 1 < chars.len() {
            let mut j = i + 1;
            while j < chars.len() && matches!(chars[j], ' ' | '\t' | '\n') {
                j += 1;
            }
            if j < chars.len() && chars[j] == '+' {
                j += 1;
                while j < chars.len() && matches!(chars[j], ' ' | '\t' | '\n') {
                    j += 1;
                }
                if j < chars.len() && matches!(chars[j], '\'' | '"' | '`') {
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn fold_template_and_array_joins(line: &str) -> String {
    let mut s = line
        .replace("${'", "")
        .replace("'+\"", "")
        .replace("\"+'", "")
        .replace("'}", "")
        .replace("${\"", "")
        .replace("\"}", "")
        .replace("','", "")
        .replace("\",\"", "")
        .replace("', '", "")
        .replace("\", \"", "")
        .replace("\", '", "")
        .replace("', \"", "")
        .replace("\" + '", "")
        .replace("' + \"", "")
        .replace("\" + \"", "")
        .replace("' + '", "");
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0usize;
    while i < chars.len() {
        // Fold shell empty-quote splitting (e.g. ev""al or ev''al), inline backslash escapes (ev\al), and adjacent quoted strings ("ev" "al")
        if chars[i] == '\\'
            && i > 0
            && is_ident_char(chars[i - 1])
            && i + 1 < chars.len()
            && is_ident_char(chars[i + 1])
        {
            i += 1;
            continue;
        }
        if (chars[i] == '"' || chars[i] == '\'') && i + 1 < chars.len() && chars[i + 1] == chars[i]
        {
            let prev_id = i > 0 && is_ident_char(chars[i - 1]);
            let next_id = i + 2 < chars.len() && is_ident_char(chars[i + 2]);
            if prev_id && next_id {
                i += 2;
                continue;
            }
        }
        if (chars[i] == '"' || chars[i] == '\'')
            && i + 2 < chars.len()
            && chars[i + 1] == ' '
            && (chars[i + 2] == '"' || chars[i + 2] == '\'')
            && i > 0
            && is_ident_char(chars[i - 1])
            && i + 3 < chars.len()
            && is_ident_char(chars[i + 3])
        {
            i += 3;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    s = out;
    s
}

fn collapse_horizontal_ws(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut prev_ws = false;
    for c in line.chars() {
        if c == ' ' || c == '\t' {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out
}

fn normalize_pipe_spacing(lower: &str) -> String {
    let mut out = String::with_capacity(lower.len());
    let chars: Vec<char> = lower.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' && (i + 1 < chars.len() && chars[i + 1] == '|') {
            continue;
        }
        if c == ' ' && (i > 0 && chars[i - 1] == '|') {
            continue;
        }
        out.push(c);
    }
    out
}

fn normalize_script_for_scan_dual(content: &str) -> Vec<(String, String)> {
    strip_hidden_and_join_continuations(content)
        .lines()
        .map(|line| {
            let collapsed = collapse_horizontal_ws(line);
            let folded = fold_template_and_array_joins(&collapsed);
            let cased = normalize_pipe_spacing(&folded);
            let lower = cased.to_ascii_lowercase();
            (cased, lower)
        })
        .collect()
}

/// Normalizes continuations, whitespace, hidden chars, quotes, templates, and pipe spacing per line.
pub fn normalize_script_for_scan(content: &str) -> Vec<String> {
    normalize_script_for_scan_dual(content)
        .into_iter()
        .map(|(_, lower)| lower)
        .collect()
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn contains_call_or_indirect(haystack: &str, ident: &str) -> bool {
    let mut start = 0usize;
    while let Some(pos) = haystack[start..].find(ident) {
        let abs = start + pos;
        let left_ok = abs == 0
            || !haystack[..abs]
                .chars()
                .next_back()
                .is_some_and(is_ident_char);
        let after_ident = &haystack[abs + ident.len()..];
        let right_ok = after_ident.chars().next().is_none_or(|c| !is_ident_char(c));
        if left_ok && right_ok {
            let trimmed = after_ident.trim_start_matches([' ', '\t']);
            if trimmed.starts_with('(')
                || trimmed.starts_with("?.(")
                || trimmed.starts_with(')')
                || trimmed.starts_with(',')
            {
                return true;
            }
            let before = haystack[..abs].trim_end();
            let is_assign = before.ends_with('=')
                && !before.ends_with("==")
                && !before.ends_with("!=")
                && !before.ends_with("<=")
                && !before.ends_with(">=");
            if before.ends_with(',') || is_assign || trimmed.starts_with(';') {
                return true;
            }
        }
        start = abs + ident.len();
    }
    false
}

fn contains_bracket_quoted(haystack: &str, ident: &str) -> bool {
    let quotes = ['"', '\'', '`'];
    let bytes = haystack.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            if j < bytes.len() && quotes.contains(&(bytes[j] as char)) {
                let start = j + 1;
                if let Some(rest) = haystack.get(start..) {
                    if rest.starts_with(ident) {
                        let mut k = start + ident.len();
                        if k < bytes.len() && quotes.contains(&(bytes[k] as char)) {
                            k += 1;
                            while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
                                k += 1;
                            }
                            if k < bytes.len() && bytes[k] == b']' {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }
    false
}

fn contains_bare_word(lower: &str, word: &str) -> bool {
    let mut start = 0usize;
    while let Some(pos) = lower[start..].find(word) {
        let abs = start + pos;
        let left_ok = abs == 0 || !lower[..abs].chars().next_back().is_some_and(is_ident_char);
        let right = &lower[abs + word.len()..];
        let right_ok = right.chars().next().is_none_or(|c| !is_ident_char(c));
        if left_ok && right_ok {
            return true;
        }
        start = abs + word.len();
    }
    false
}

fn detect_command_tell(filename: &str, lower: &str) -> Option<&'static str> {
    let shell_tells = [
        "curl|bash",
        "curl|sh",
        "wget -o-|bash",
        "wget -o-|sh",
        "wget|bash",
        "wget|sh",
        "child_process.exec",
        "child_process.spawn",
        "[\"exec\"]",
        "['exec']",
        "[`exec`]",
        "[\"execsync\"]",
        "['execsync']",
        "[`execsync`]",
        "execsync(",
        "execfilesync(",
        "spawnsync(",
        "execfile(",
        "system(",
        "popen(",
        "subprocess.call",
        "subprocess.popen",
        "subprocess.run",
        "subprocess.check_output",
        "os.exec",
        "std::process::command",
        "deno.command",
        "bun.spawn",
        "bun.$",
        "os.system",
        "rm -rf /",
        "env -s",
    ];
    if let Some(found) = shell_tells
        .iter()
        .copied()
        .find(|tell| lower.contains(tell))
    {
        return Some(found);
    }
    if contains_bracket_quoted(lower, "exec") || contains_bracket_quoted(lower, "execsync") {
        return Some("bracket exec");
    }
    if contains_call_or_indirect(lower, "spawn") {
        return Some("spawn(");
    }
    let ext = extension_lowercase(filename).unwrap_or_default();
    if matches!(ext.as_str(), "rb" | "pl") && (lower.contains("%x{") || lower.contains("qx/")) {
        return Some("ruby/perl shell subcommand");
    }
    None
}

fn detect_eval_tell(filename: &str, cased: &str, lower: &str) -> bool {
    let eval_tells = [
        "window.eval",
        "globalthis.eval",
        "self[\"eval\"]",
        "self['eval']",
        "global[\"eval\"]",
        "global['eval']",
        "[\"eval\"]",
        "['eval']",
        "[`eval`]",
        "base64 -d",
        "base64 --decode",
        "buffer.from(",
        "fromcharcode(",
        "fromcodepoint(",
        concat!("dangerously", "setinnerhtml"),
        "reflect.construct(function",
        ".constructor(",
        "__import__(",
    ];
    if contains_call_or_indirect(lower, "eval")
        || contains_bracket_quoted(lower, "eval")
        || contains_call_or_indirect(lower, "atob")
        || contains_call_or_indirect(cased, "Function")
        || eval_tells.iter().any(|t| lower.contains(t))
    {
        return true;
    }
    let ext = extension_lowercase(filename).unwrap_or_default();
    if matches!(ext.as_str(), "sh" | "bash" | "zsh") && contains_bare_word(lower, "eval") {
        return true;
    }
    let exec_exts = ["py", "js", "mjs", "cjs", "ts", "tsx", "jsx"];
    exec_exts.contains(&ext.as_str()) && contains_call_or_indirect(lower, "exec")
}

fn detect_network_or_traversal(
    raw_line: &str,
    lower: &str,
    line_num: usize,
) -> Result<(), RepackageSecurityError> {
    let net_tells = [
        "nc -e",
        "/bin/sh",
        "/bin/bash",
        "socket.connect",
        "telemetry-exfil.com",
        "requests.",
        "urllib.",
        "http.request",
        "https.request",
        "axios.",
        "[\"fetch\"]",
        "['fetch']",
        "[`fetch`]",
        "curl ",
        "curl|",
        "wget ",
        "wget|",
    ];
    for tell in net_tells {
        if lower.contains(tell) {
            return Err(RepackageSecurityError::UnauthorizedNetworkCall {
                pattern: tell.to_string(),
                line: line_num,
            });
        }
    }
    if contains_call_or_indirect(lower, "fetch") || contains_bracket_quoted(lower, "fetch") {
        return Err(RepackageSecurityError::UnauthorizedNetworkCall {
            pattern: "fetch".to_string(),
            line: line_num,
        });
    }
    for pat in [
        "../",
        "..\\",
        "~/.ssh",
        ".ssh/",
        "/etc/passwd",
        "/etc/shadow",
        "/root/",
        "/proc/self",
    ] {
        if lower.contains(pat) {
            return Err(RepackageSecurityError::PathTraversal {
                pattern: raw_line.trim().to_string(),
                line: line_num,
            });
        }
    }
    Ok(())
}

/// Zero-Trust security scanner for external scripts and imported assets (AC-03, AC-05, EC-E08-02).
pub fn scan_external_script_security(
    filename: &str,
    content: &str,
) -> Result<(), RepackageSecurityError> {
    for (idx, (cased, lower)) in normalize_script_for_scan_dual(content)
        .into_iter()
        .enumerate()
    {
        let line_num = idx + 1;
        if let Some(tell) = detect_command_tell(filename, &lower) {
            return Err(RepackageSecurityError::CommandInjectionTell {
                pattern: tell.to_string(),
                line: line_num,
            });
        }
        if detect_eval_tell(filename, &cased, &lower) {
            return Err(RepackageSecurityError::ArbitraryCodeEvaluation { line: line_num });
        }
        detect_network_or_traversal(&lower, &lower, line_num)?;
    }
    Ok(())
}

fn contains_ref_markdown_image(lower: &str) -> bool {
    let mut idx = 0usize;
    while let Some(pos) = lower[idx..].find("![") {
        let rest = &lower[idx + pos + 2..];
        if let Some(close) = rest.find(']') {
            let after = rest[close + 1..].trim_start();
            if after.starts_with('[') && after[1..].contains(']') {
                return true;
            }
        }
        idx += pos + 2;
    }
    false
}

fn contains_event_handler_attr(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    let mut in_tag = false;
    let mut quote: Option<u8> = None;
    for i in 0..bytes.len() {
        if !in_tag {
            if bytes[i] == b'<' && bytes.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
                in_tag = true;
                quote = None;
            }
            continue;
        }
        if let Some(q) = quote {
            if bytes[i] == q {
                quote = None;
            }
            continue;
        }
        if matches!(bytes[i], b'"' | b'\'') {
            quote = Some(bytes[i]);
            continue;
        }
        if bytes[i] == b'>' {
            in_tag = false;
            continue;
        }
        if i + 3 >= bytes.len() {
            continue;
        }
        let prev_sep = matches!(
            bytes[i - 1],
            b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'"' | b'\''
        );
        if prev_sep
            && bytes[i] == b'o'
            && bytes[i + 1] == b'n'
            && bytes[i + 2].is_ascii_alphabetic()
        {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                j += 1;
            }
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'=' {
                return true;
            }
        }
    }
    false
}

fn decode_colon_entities(lower: &str) -> String {
    let mut out = String::with_capacity(lower.len());
    let bytes = lower.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'&' && bytes.get(i + 1) == Some(&b'#') {
            let mut j = i + 2;
            let is_hex = bytes.get(j) == Some(&b'x');
            if is_hex {
                j += 1;
            }
            while j < bytes.len() && bytes[j] == b'0' {
                j += 1;
            }
            let matched = if is_hex {
                bytes.get(j..j + 3) == Some(b"3a;")
            } else {
                bytes.get(j..j + 3) == Some(b"58;")
            };
            if matched {
                out.push(':');
                i = j + 3;
                continue;
            }
        }
        out.push(lower[i..].chars().next().unwrap_or_default());
        i += lower[i..].chars().next().map_or(1, char::len_utf8);
    }
    out
}

fn scan_normalized_markdown_slice(label: &str, text: &str) -> Result<(), RepackageSecurityError> {
    let entity_norm = decode_colon_entities(&text.to_ascii_lowercase());
    let compact_scheme: String = entity_norm.chars().filter(|c| !c.is_whitespace()).collect();
    if contains_ref_markdown_image(&entity_norm)
        || compact_scheme.contains("javascript:")
        || compact_scheme.contains("data:text/html")
    {
        return Err(RepackageSecurityError::PromptInjectionOrBeacon {
            label: label.to_string(),
            detail: "Disallowed reference image beacon or javascript/data URI".to_string(),
        });
    }
    for forbidden in [
        "<important",
        "</important",
        "<system",
        "</system",
        "<script",
        "<iframe",
        "<object",
        "<embed",
        "<image",
        "@import",
        concat!("dangerously", "setinnerhtml"),
    ] {
        if entity_norm.contains(forbidden) {
            return Err(RepackageSecurityError::PromptInjectionOrBeacon {
                label: label.to_string(),
                detail: format!("Disallowed active markup tag '{forbidden}'"),
            });
        }
    }
    if contains_event_handler_attr(&entity_norm) {
        return Err(RepackageSecurityError::PromptInjectionOrBeacon {
            label: label.to_string(),
            detail: "Disallowed inline event handler attribute (on*=)".to_string(),
        });
    }
    Ok(())
}

/// Validates Markdown, SKILL.md metadata, references, SVG, and HTML against IPI, XSS, and beacons.
pub fn scan_skill_markdown_safety(
    label: &str,
    content: &str,
) -> Result<(), RepackageSecurityError> {
    validate_staged_prompt_content(content).map_err(|err| {
        RepackageSecurityError::PromptInjectionOrBeacon {
            label: label.to_string(),
            detail: err.to_string(),
        }
    })?;
    let stripped: String = content.chars().filter(|&c| !is_hidden_or_bidi(c)).collect();
    scan_normalized_markdown_slice(label, &stripped)?;
    let shadow = shadow_nfkc_fold(&stripped);
    if shadow != stripped {
        scan_normalized_markdown_slice(label, &shadow)?;
    }
    Ok(())
}

/// Rejects metadata strings containing YAML delimiter breakout lines (`---`) or IPI markers.
pub fn validate_skill_metadata_text(
    field: &str,
    value: &str,
) -> Result<(), RepackageSecurityError> {
    scan_skill_markdown_safety(field, value)?;
    for candidate in [value.to_string(), shadow_nfkc_fold(value)] {
        let normalized = candidate
            .replace("\r\n", "\n")
            .replace(['\r', '\u{2028}', '\u{2029}'], "\n");
        if normalized.lines().any(|line| line.trim() == "---") {
            return Err(RepackageSecurityError::PromptInjectionOrBeacon {
                label: field.to_string(),
                detail: "YAML delimiter '---' line forbidden inside frontmatter field".to_string(),
            });
        }
    }
    Ok(())
}

fn has_executable_magic_bytes(raw: &[u8]) -> Option<&'static str> {
    if raw.starts_with(b"\x7fELF") {
        return Some("ELF executable header");
    }
    if raw.starts_with(b"\x00asm") {
        return Some("WebAssembly binary header");
    }
    if raw.len() >= 4 {
        let head = &raw[..4];
        if matches!(
            head,
            [0xfe, 0xed, 0xfa, 0xce]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xca, 0xfe, 0xba, 0xbf]
        ) {
            return Some("Mach-O executable header");
        }
    }
    if raw.starts_with(b"MZ") {
        return Some("PE/MZ executable header");
    }
    None
}

fn has_valid_image_magic(ext: &str, raw: &[u8]) -> bool {
    match ext {
        "png" => raw.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => raw.starts_with(&[0xFF, 0xD8, 0xFF]),
        "gif" => raw.starts_with(b"GIF87a") || raw.starts_with(b"GIF89a"),
        "webp" => raw.len() >= 12 && &raw[..4] == b"RIFF" && &raw[8..12] == b"WEBP",
        _ => false,
    }
}

/// Classifies source files deterministically and blocks disguised binaries/non-UTF-8 smuggling (EC-E08-06).
pub fn classify_and_validate_source_asset(
    rel_path: &str,
    raw_bytes: &[u8],
) -> Result<RepackageAssetBucket, RepackageSecurityError> {
    if let Some(magic_desc) = has_executable_magic_bytes(raw_bytes) {
        return Err(RepackageSecurityError::UnsupportedOrBinaryAsset {
            filename: rel_path.to_string(),
            reason: format!("Disallowed executable payload ({magic_desc})"),
        });
    }
    let ext = extension_lowercase(rel_path).ok_or_else(|| {
        RepackageSecurityError::UnsupportedOrBinaryAsset {
            filename: rel_path.to_string(),
            reason: "Missing explicit file extension or invalid dotfile".to_string(),
        }
    })?;
    if BLOCKED_BINARY_EXTENSIONS.contains(&ext.as_str()) {
        return Err(RepackageSecurityError::UnsupportedOrBinaryAsset {
            filename: rel_path.to_string(),
            reason: format!("Blocked native/archive extension '.{ext}'"),
        });
    }
    if ASSET_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        if !has_valid_image_magic(&ext, raw_bytes) {
            return Err(RepackageSecurityError::UnsupportedOrBinaryAsset {
                filename: rel_path.to_string(),
                reason: format!("Spoofed or malformed .{ext} raster header"),
            });
        }
        return Ok(RepackageAssetBucket::AssetsBinaryImage);
    }

    let bucket = if SCRIPT_EXTENSIONS.contains(&ext.as_str()) {
        RepackageAssetBucket::Scripts
    } else if REFERENCE_EXTENSIONS.contains(&ext.as_str()) {
        RepackageAssetBucket::References
    } else if ASSET_TEXT_EXTENSIONS.contains(&ext.as_str()) {
        RepackageAssetBucket::AssetsText
    } else {
        return Err(RepackageSecurityError::UnsupportedOrBinaryAsset {
            filename: rel_path.to_string(),
            reason: format!("Unsupported extension '.{ext}'"),
        });
    };

    if raw_bytes.contains(&0u8) {
        return Err(RepackageSecurityError::UnsupportedOrBinaryAsset {
            filename: rel_path.to_string(),
            reason: "NUL byte disallowed in UTF-8 text asset".to_string(),
        });
    }
    let text = std::str::from_utf8(raw_bytes).map_err(|_| {
        RepackageSecurityError::UnsupportedOrBinaryAsset {
            filename: rel_path.to_string(),
            reason: "Non-UTF-8 payload in script/text/markup file".to_string(),
        }
    })?;
    scan_external_script_security(rel_path, text)?;
    if matches!(
        bucket,
        RepackageAssetBucket::References | RepackageAssetBucket::AssetsText
    ) {
        scan_skill_markdown_safety(rel_path, text)?;
    }
    Ok(bucket)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passes_clean_script() {
        let clean = r#"
export function calculatePadding(w: number): number {
    return Math.max(16, w * 0.1);
}
"#;
        assert!(scan_external_script_security("utils.ts", clean).is_ok());
    }

    #[test]
    fn test_rejects_curl_bash() {
        let malicious = "run_setup() {\n  curl | bash\n}";
        let err = scan_external_script_security("install.sh", malicious).unwrap_err();
        match err {
            RepackageSecurityError::CommandInjectionTell { pattern, line } => {
                assert_eq!(pattern, "curl|bash");
                assert_eq!(line, 2);
            }
            _ => panic!("Expected CommandInjectionTell"),
        }
    }

    #[test]
    fn test_rejects_eval() {
        let malicious = concat!("const res = ev", "al(userInput);");
        let err = scan_external_script_security("calc.js", malicious).unwrap_err();
        assert_eq!(
            err,
            RepackageSecurityError::ArbitraryCodeEvaluation { line: 1 }
        );
    }
}
