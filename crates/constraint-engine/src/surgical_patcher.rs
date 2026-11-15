// @story 03.3
// @implements AC-01, AC-02, AC-03
use domain::HarnessError;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement, JSXExpression,
    Statement,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

#[cfg(feature = "ts")]
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum PatchKind {
    TextContent,
    MetricValue,
    CssVariable,
    AttributeValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct SurgicalPatchRequest {
    pub ff_id: String,
    pub patch_kind: PatchKind,
    pub new_value: String,
    pub source: String,
    #[serde(default)]
    pub target_attribute: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct SurgicalPatchResponse {
    pub ff_id: String,
    pub success: bool,
    pub original_text: Option<String>,
    pub patched_source: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub execution_time_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct AstInjectionRequest {
    pub source: String,
    #[serde(default)]
    pub component_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct AstInjectionResponse {
    pub total_elements_tagged: usize,
    pub tagged_source: String,
    pub generated_ids: Vec<String>,
}

enum TargetKind {
    JsxText,
    EmptyJsxElement,
    JsStringLiteral { quote: char },
    NumericLiteral,
    CssVarStringLiteral { quote: char },
    AttributeStringLiteral { attr_name: String, quote: char },
}

pub struct SurgicalPatcher;

impl SurgicalPatcher {
    /// Maximum TSX/JSX source size accepted for <1ms AST surgical operations (512 KB).
    pub const MAX_SOURCE_BYTES: usize = 512 * 1024;

    fn validate_source_size_and_parse<'a>(
        allocator: &'a Allocator,
        source: &'a str,
    ) -> Result<oxc_parser::ParserReturn<'a>, HarnessError> {
        if source.len() > Self::MAX_SOURCE_BYTES {
            return Err(HarnessError::BoundaryViolation {
                reason: format!(
                    "INPUT_TOO_LARGE: Source size ({} bytes) exceeds max budget of {} bytes",
                    source.len(),
                    Self::MAX_SOURCE_BYTES
                ),
            });
        }
        let parsed = Parser::new(allocator, source, SourceType::tsx()).parse();
        if !parsed.diagnostics.is_empty() {
            return Err(HarnessError::ValidationFailed {
                reason: format!("Malformed JSX/TSX syntax: {}", parsed.diagnostics[0]),
            });
        }
        Ok(parsed)
    }

    fn read_existing_ff_id(el: &JSXElement<'_>) -> Option<String> {
        el.opening_element.attributes.iter().find_map(|attr| {
            if let JSXAttributeItem::Attribute(a) = attr {
                let name = Self::jsx_attr_name(&a.name);
                if name == "data-ff-id" {
                    if let Some(JSXAttributeValue::StringLiteral(lit)) = &a.value {
                        return lit.value.as_str().map(|s| s.to_string());
                    }
                }
            }
            None
        })
    }

    fn jsx_attr_name<'a>(name: &'a JSXAttributeName<'a>) -> Cow<'a, str> {
        match name {
            JSXAttributeName::Identifier(id) => Cow::Borrowed(id.name.as_str()),
            JSXAttributeName::NamespacedName(ns) => Cow::Owned(format!(
                "{}:{}",
                ns.namespace.name.as_str(),
                ns.name.name.as_str()
            )),
        }
    }

    fn jsx_tag_name<'a>(name: &'a oxc_ast::ast::JSXElementName<'a>) -> &'a str {
        match name {
            oxc_ast::ast::JSXElementName::Identifier(id) => id.name.as_str(),
            oxc_ast::ast::JSXElementName::IdentifierReference(id) => id.name.as_str(),
            oxc_ast::ast::JSXElementName::NamespacedName(ns) => ns.name.name.as_str(),
            oxc_ast::ast::JSXElementName::MemberExpression(m) => m.property.name.as_str(),
            oxc_ast::ast::JSXElementName::ThisExpression(_) => "this",
        }
    }

    /// Injects deterministic `data-ff-id` attributes into all JSX elements in `source`
    /// that do not already have one.
    pub fn inject_ff_ids(
        source: &str,
        component_name: Option<&str>,
    ) -> Result<AstInjectionResponse, HarnessError> {
        let allocator = Allocator::default();
        let parsed = Self::validate_source_size_and_parse(&allocator, source)?;

        let raw_cmp = component_name.unwrap_or("Root");
        let sanitized: String = raw_cmp
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        let cmp = if sanitized.is_empty() {
            "root".to_string()
        } else {
            sanitized.to_lowercase()
        };
        let mut elements = Vec::new();
        for stmt in &parsed.program.body {
            Self::collect_elements_stmt(stmt, &mut elements);
        }

        let mut existing_ids = HashSet::new();
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        let cmp_prefix = format!("ff-{}-", cmp);

        // Pre-seed tag_counts and existing_ids so repeated runs never collide with pre-existing IDs
        for el in &elements {
            if let Some(existing) = Self::read_existing_ff_id(el) {
                if let Some(rest) = existing.strip_prefix(&cmp_prefix) {
                    if let Some((tag_part, num_part)) = rest.rsplit_once('-') {
                        if let Ok(num) = num_part.parse::<usize>() {
                            let entry = tag_counts.entry(tag_part.to_lowercase()).or_insert(0);
                            *entry = (*entry).max(num);
                        }
                    }
                }
                existing_ids.insert(existing);
            }
        }

        let mut generated_ids = Vec::new();
        let mut insertions: Vec<(usize, String)> = Vec::new();

        for el in elements {
            if let Some(existing) = Self::read_existing_ff_id(el) {
                generated_ids.push(existing);
                continue;
            }

            let clean_tag = Self::jsx_tag_name(&el.opening_element.name).to_lowercase();
            let count = tag_counts.entry(clean_tag.clone()).or_insert(0);
            let ff_id = loop {
                *count += 1;
                let candidate = format!("ff-{}-{}-{}", cmp, clean_tag, *count);
                if existing_ids.insert(candidate.clone()) {
                    break candidate;
                }
            };
            generated_ids.push(ff_id.clone());

            let insert_offset = el
                .opening_element
                .type_arguments
                .as_ref()
                .map_or(el.opening_element.name.span().end, |ta| ta.span.end)
                as usize;
            insertions.push((insert_offset, format!(" data-ff-id=\"{}\"", ff_id)));
        }

        let total_elements_tagged = insertions.len();
        insertions.sort_by_key(|a| std::cmp::Reverse(a.0));

        let mut tagged_source = source.to_string();
        for (offset, text) in insertions {
            if offset <= tagged_source.len() && tagged_source.is_char_boundary(offset) {
                tagged_source.insert_str(offset, &text);
            }
        }

        // Verify post-injection output remains valid TSX/JSX
        let verify_alloc = Allocator::default();
        let _ = Self::validate_source_size_and_parse(&verify_alloc, &tagged_source)?;

        Ok(AstInjectionResponse {
            total_elements_tagged,
            tagged_source,
            generated_ids,
        })
    }

    fn is_forbidden_attribute_name(attr_name: &str) -> bool {
        let lower = attr_name.trim().to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "dangerouslysetinnerhtml" | "srcdoc" | "data-ff-id"
        ) || lower.starts_with("on:")
        {
            return true;
        }
        lower.starts_with("on") && lower.len() > 2 && lower.as_bytes()[2].is_ascii_alphabetic()
    }

    fn decode_html_entities_for_url_check(raw: &str) -> String {
        let bytes = raw.as_bytes();
        let mut out = String::with_capacity(raw.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'&' {
                let rest = &raw[i + 1..];
                if let Some(decoded_and_len) = Self::try_decode_single_entity(rest) {
                    out.push(decoded_and_len.0);
                    i += 1 + decoded_and_len.1;
                    continue;
                }
            }
            if let Some(ch) = raw[i..].chars().next() {
                out.push(ch);
                i += ch.len_utf8();
            } else {
                break;
            }
        }
        out
    }

    fn try_decode_single_entity(rest: &str) -> Option<(char, usize)> {
        if let Some(after_hash) = rest.strip_prefix('#') {
            let (is_hex, digits_src, prefix_len) =
                if let Some(h) = after_hash.strip_prefix(['x', 'X']) {
                    (true, h, 2)
                } else {
                    (false, after_hash, 1)
                };
            let max_digits = digits_src
                .bytes()
                .take(8)
                .take_while(|b| {
                    if is_hex {
                        b.is_ascii_hexdigit()
                    } else {
                        b.is_ascii_digit()
                    }
                })
                .count();
            if max_digits == 0 {
                return None;
            }
            let code = u32::from_str_radix(&digits_src[..max_digits], if is_hex { 16 } else { 10 })
                .ok()?;
            let ch = char::from_u32(code).unwrap_or('\u{FFFD}');
            let semi = usize::from(digits_src.as_bytes().get(max_digits) == Some(&b';'));
            return Some((ch, prefix_len + max_digits + semi));
        }
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(rest.len());
        if end == 0 || end > 16 || rest.as_bytes().get(end) != Some(&b';') {
            return None;
        }
        let ch = match &rest[..end].to_ascii_lowercase()[..] {
            "colon" => ':',
            "tab" => '\t',
            "newline" => '\n',
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            "sol" => '/',
            _ => return None,
        };
        Some((ch, end + 1))
    }

    fn has_forbidden_url_scheme(value: &str) -> bool {
        let mut current = value.to_string();
        for _ in 0..3 {
            let normalized: String = current
                .chars()
                .filter(|c| !c.is_control() && !c.is_whitespace())
                .collect::<String>()
                .to_ascii_lowercase();
            if normalized.starts_with("javascript:")
                || normalized.starts_with("vbscript:")
                || normalized.starts_with("data:text/html")
                || normalized.starts_with("data:image/svg+xml")
            {
                return true;
            }
            let decoded = Self::decode_html_entities_for_url_check(&current);
            if decoded == current {
                break;
            }
            current = decoded;
        }
        false
    }

    fn validate_url_attribute_value(attr_name: &str, value: &str) -> Result<(), HarnessError> {
        let lower_attr = attr_name.trim().to_ascii_lowercase();
        let is_url_attr = matches!(
            lower_attr.as_str(),
            "href" | "src" | "action" | "formaction" | "xlink:href" | "poster" | "data"
        );
        if !is_url_attr {
            return Ok(());
        }
        if Self::has_forbidden_url_scheme(value) {
            return Err(HarnessError::BoundaryViolation {
                reason: format!(
                    "UNSAFE_URL_SCHEME_BLOCKED: Attribute '{}' rejects executable URL scheme",
                    attr_name
                ),
            });
        }
        Ok(())
    }

    fn escape_jsx_text(raw: &str) -> Result<String, HarnessError> {
        if raw.contains('\0') {
            return Err(HarnessError::ValidationFailed {
                reason: "NULL_BYTE_FORBIDDEN: Patch text cannot contain null bytes".to_string(),
            });
        }
        let mut out = String::with_capacity(raw.len());
        for ch in raw.chars() {
            match ch {
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '{' => out.push_str("&#123;"),
                '}' => out.push_str("&#125;"),
                _ => out.push(ch),
            }
        }
        Ok(out)
    }

    fn has_unescaped_quote(inner: &str, quote: char) -> bool {
        let mut escaped = false;
        for ch in inner.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                return true;
            }
        }
        escaped
    }

    fn unwrap_matching_outer_quote(raw: &str, preferred_quote: char) -> &str {
        for q in [preferred_quote, '\'', '"'] {
            if raw.len() >= 2 && raw.starts_with(q) && raw.ends_with(q) {
                let inner = &raw[q.len_utf8()..raw.len() - q.len_utf8()];
                if !Self::has_unescaped_quote(inner, q) {
                    return inner;
                }
            }
        }
        raw
    }

    fn escape_js_string_literal(raw: &str, quote: char) -> Result<String, HarnessError> {
        if raw.contains('\0') {
            return Err(HarnessError::ValidationFailed {
                reason: "NULL_BYTE_FORBIDDEN: String value cannot contain null bytes".to_string(),
            });
        }
        let mut out = String::with_capacity(raw.len() + 2);
        out.push(quote);
        for ch in raw.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                c if c == quote => {
                    out.push('\\');
                    out.push(c);
                }
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\u{2028}' => out.push_str("\\u2028"),
                '\u{2029}' => out.push_str("\\u2029"),
                _ => out.push(ch),
            }
        }
        out.push(quote);
        Ok(out)
    }

    fn escape_jsx_attr_string_literal(
        attr_name: &str,
        raw: &str,
        quote: char,
    ) -> Result<String, HarnessError> {
        if raw.contains('\0') {
            return Err(HarnessError::ValidationFailed {
                reason: "NULL_BYTE_FORBIDDEN: Attribute value cannot contain null bytes"
                    .to_string(),
            });
        }
        Self::validate_url_attribute_value(attr_name, raw)?;
        let mut out = String::with_capacity(raw.len() + 2);
        out.push(quote);
        for ch in raw.chars() {
            match ch {
                '"' if quote == '"' => out.push_str("&quot;"),
                '\'' if quote == '\'' => out.push_str("&#39;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '\n' => out.push_str("&#10;"),
                '\r' => out.push_str("&#13;"),
                _ => out.push(ch),
            }
        }
        out.push(quote);
        Ok(out)
    }

    fn format_replacement(kind: TargetKind, new_value: &str) -> Result<String, HarnessError> {
        match kind {
            TargetKind::JsxText | TargetKind::EmptyJsxElement => Self::escape_jsx_text(new_value),
            TargetKind::JsStringLiteral { quote } => {
                Self::escape_js_string_literal(new_value, quote)
            }
            TargetKind::NumericLiteral => {
                let trimmed = new_value.trim();
                let parsed: f64 = trimmed
                    .parse()
                    .map_err(|_| HarnessError::ValidationFailed {
                        reason: format!(
                            "INVALID_NUMERIC_PATCH: Expected finite numeric literal, got '{}'",
                            trimmed
                        ),
                    })?;
                if !parsed.is_finite() {
                    return Err(HarnessError::ValidationFailed {
                        reason: "INVALID_NUMERIC_PATCH: Value must be finite".to_string(),
                    });
                }
                Ok(trimmed.to_string())
            }
            TargetKind::CssVarStringLiteral { quote } => {
                let inner = Self::unwrap_matching_outer_quote(new_value, quote);
                let lower = inner.to_ascii_lowercase();
                if lower.contains("url(")
                    || lower.contains("expression(")
                    || lower.contains("@import")
                    || lower.contains("</")
                    || inner.contains([';', '{', '}', '\n', '\r'])
                {
                    return Err(HarnessError::BoundaryViolation {
                        reason: "UNSAFE_CSS_VARIABLE_VALUE: CSS variable value contains forbidden syntax".to_string(),
                    });
                }
                Self::escape_js_string_literal(inner, quote)
            }
            TargetKind::AttributeStringLiteral { attr_name, quote } => {
                Self::escape_jsx_attr_string_literal(&attr_name, new_value, quote)
            }
        }
    }

    /// Surgically patches text, metric, CSS variable, or literal attribute of the JSX element
    /// identified by `data-ff-id` in < 1ms without mutating surrounding AST structure or formatting (AC-03 / EC-E03-03).
    pub fn patch_by_ff_id(
        source: &str,
        req: &SurgicalPatchRequest,
    ) -> Result<SurgicalPatchResponse, HarnessError> {
        let start = Instant::now();

        if matches!(req.patch_kind, PatchKind::AttributeValue) {
            let target_attr = req.target_attribute.as_deref().unwrap_or("style");
            if Self::is_forbidden_attribute_name(target_attr) {
                return Err(HarnessError::BoundaryViolation {
                    reason: format!(
                        "FORBIDDEN_ATTRIBUTE_PATCH: Attribute '{}' cannot be surgically patched",
                        target_attr
                    ),
                });
            }
        }
        if matches!(req.patch_kind, PatchKind::CssVariable) {
            if let Some(target_attr) = req.target_attribute.as_deref() {
                if !target_attr.starts_with("--") {
                    return Err(HarnessError::BoundaryViolation {
                        reason: format!(
                            "INVALID_CSS_VARIABLE_NAME: Expected '--*' custom property, got '{}'",
                            target_attr
                        ),
                    });
                }
            }
        }

        let allocator = Allocator::default();
        let parsed = Self::validate_source_size_and_parse(&allocator, source)?;

        let mut elements = Vec::new();
        for stmt in &parsed.program.body {
            Self::collect_elements_stmt(stmt, &mut elements);
        }

        let mut target_span: Option<(Span, String, TargetKind)> = None;
        for el in elements {
            if Self::read_existing_ff_id(el).as_deref() == Some(req.ff_id.as_str()) {
                target_span = Self::extract_target_from_element(
                    el,
                    &req.patch_kind,
                    req.target_attribute.as_deref(),
                    source,
                )?;
                break;
            }
        }

        if let Some((span, original_text, kind)) = target_span {
            let start_byte = span.start as usize;
            let end_byte = span.end as usize;

            if start_byte <= end_byte
                && end_byte <= source.len()
                && source.is_char_boundary(start_byte)
                && source.is_char_boundary(end_byte)
            {
                let replacement = Self::format_replacement(kind, &req.new_value)?;
                let mut patched_source = String::with_capacity(source.len() + replacement.len());
                patched_source.push_str(&source[..start_byte]);
                patched_source.push_str(&replacement);
                patched_source.push_str(&source[end_byte..]);

                // Fail-closed post-patch OXC validation guarantees no syntax/AST breakout
                let post_alloc = Allocator::default();
                let _ = Self::validate_source_size_and_parse(&post_alloc, &patched_source)?;

                let execution_time_micros = start.elapsed().as_micros() as u64;
                return Ok(SurgicalPatchResponse {
                    ff_id: req.ff_id.clone(),
                    success: true,
                    original_text: Some(original_text),
                    patched_source,
                    execution_time_micros,
                });
            }
        }

        let execution_time_micros = start.elapsed().as_micros() as u64;
        Ok(SurgicalPatchResponse {
            ff_id: req.ff_id.clone(),
            success: false,
            original_text: None,
            patched_source: source.to_string(),
            execution_time_micros,
        })
    }

    fn collect_elements_class<'a>(
        class_decl: &'a oxc_ast::ast::Class<'a>,
        elements: &mut Vec<&'a JSXElement<'a>>,
    ) {
        for elem in &class_decl.body.body {
            match elem {
                oxc_ast::ast::ClassElement::MethodDefinition(m) => {
                    if let Some(body) = &m.value.body {
                        for s in &body.statements {
                            Self::collect_elements_stmt(s, elements);
                        }
                    }
                }
                oxc_ast::ast::ClassElement::PropertyDefinition(p) => {
                    if let Some(val) = &p.value {
                        Self::collect_elements_expr(val, elements);
                    }
                }
                oxc_ast::ast::ClassElement::StaticBlock(b) => {
                    for s in &b.body {
                        Self::collect_elements_stmt(s, elements);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_elements_stmt<'a>(stmt: &'a Statement<'a>, elements: &mut Vec<&'a JSXElement<'a>>) {
        match stmt {
            Statement::ExpressionStatement(e) => {
                Self::collect_elements_expr(&e.expression, elements);
            }
            Statement::ExportDefaultDeclaration(d) => match &d.declaration {
                oxc_ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                    if let Some(body) = &f.body {
                        for s in &body.statements {
                            Self::collect_elements_stmt(s, elements);
                        }
                    }
                }
                oxc_ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(c) => {
                    Self::collect_elements_class(c, elements);
                }
                _ => {
                    if let Some(expr) = d.declaration.as_expression() {
                        Self::collect_elements_expr(expr, elements);
                    }
                }
            },
            Statement::ExportDeclaration(d) => match &d.declaration {
                oxc_ast::ast::Declaration::VariableDeclaration(v) => {
                    for decl in &v.declarations {
                        if let Some(init) = &decl.init {
                            Self::collect_elements_expr(init, elements);
                        }
                    }
                }
                oxc_ast::ast::Declaration::FunctionDeclaration(f) => {
                    if let Some(body) = &f.body {
                        for s in &body.statements {
                            Self::collect_elements_stmt(s, elements);
                        }
                    }
                }
                oxc_ast::ast::Declaration::ClassDeclaration(c) => {
                    Self::collect_elements_class(c, elements);
                }
                _ => {}
            },
            Statement::VariableDeclaration(v) => {
                for decl in &v.declarations {
                    if let Some(init) = &decl.init {
                        Self::collect_elements_expr(init, elements);
                    }
                }
            }
            Statement::FunctionDeclaration(f) => {
                if let Some(body) = &f.body {
                    for s in &body.statements {
                        Self::collect_elements_stmt(s, elements);
                    }
                }
            }
            Statement::ClassDeclaration(c) => {
                Self::collect_elements_class(c, elements);
            }
            Statement::BlockStatement(b) => {
                for s in &b.body {
                    Self::collect_elements_stmt(s, elements);
                }
            }
            Statement::ReturnStatement(r) => {
                if let Some(arg) = &r.argument {
                    Self::collect_elements_expr(arg, elements);
                }
            }
            Statement::IfStatement(i) => {
                Self::collect_elements_expr(&i.test, elements);
                Self::collect_elements_stmt(&i.consequent, elements);
                if let Some(alt) = &i.alternate {
                    Self::collect_elements_stmt(alt, elements);
                }
            }
            Statement::SwitchStatement(sw) => {
                Self::collect_elements_expr(&sw.discriminant, elements);
                for case in &sw.cases {
                    if let Some(test) = &case.test {
                        Self::collect_elements_expr(test, elements);
                    }
                    for s in &case.consequent {
                        Self::collect_elements_stmt(s, elements);
                    }
                }
            }
            Statement::ForStatement(f) => {
                if let Some(test) = &f.test {
                    Self::collect_elements_expr(test, elements);
                }
                if let Some(update) = &f.update {
                    Self::collect_elements_expr(update, elements);
                }
                Self::collect_elements_stmt(&f.body, elements);
            }
            Statement::ForOfStatement(f) => {
                Self::collect_elements_expr(&f.right, elements);
                Self::collect_elements_stmt(&f.body, elements);
            }
            Statement::ForInStatement(f) => {
                Self::collect_elements_expr(&f.right, elements);
                Self::collect_elements_stmt(&f.body, elements);
            }
            Statement::WhileStatement(w) => {
                Self::collect_elements_expr(&w.test, elements);
                Self::collect_elements_stmt(&w.body, elements);
            }
            Statement::DoWhileStatement(d) => {
                Self::collect_elements_stmt(&d.body, elements);
                Self::collect_elements_expr(&d.test, elements);
            }
            Statement::TryStatement(t) => {
                for s in &t.block.body {
                    Self::collect_elements_stmt(s, elements);
                }
                if let Some(handler) = &t.handler {
                    for s in &handler.body.body {
                        Self::collect_elements_stmt(s, elements);
                    }
                }
                if let Some(fin) = &t.finalizer {
                    for s in &fin.body {
                        Self::collect_elements_stmt(s, elements);
                    }
                }
            }
            Statement::LabeledStatement(l) => {
                Self::collect_elements_stmt(&l.body, elements);
            }
            Statement::ThrowStatement(th) => {
                Self::collect_elements_expr(&th.argument, elements);
            }
            _ => {}
        }
    }

    fn collect_jsx_element<'a>(el: &'a JSXElement<'a>, elements: &mut Vec<&'a JSXElement<'a>>) {
        elements.push(el);
        for attr in &el.opening_element.attributes {
            if let JSXAttributeItem::Attribute(a) = attr {
                if let Some(JSXAttributeValue::ExpressionContainer(c)) = &a.value {
                    if let Some(expr) = c.expression.as_expression() {
                        Self::collect_elements_expr(expr, elements);
                    }
                }
            }
        }
        for child in &el.children {
            Self::collect_elements_child(child, elements);
        }
    }

    fn collect_elements_expr<'a>(
        expr: &'a oxc_ast::ast::Expression<'a>,
        elements: &mut Vec<&'a JSXElement<'a>>,
    ) {
        match expr {
            oxc_ast::ast::Expression::JSXElement(el) => {
                Self::collect_jsx_element(el, elements);
            }
            oxc_ast::ast::Expression::JSXFragment(fr) => {
                for child in &fr.children {
                    Self::collect_elements_child(child, elements);
                }
            }
            oxc_ast::ast::Expression::ParenthesizedExpression(p) => {
                Self::collect_elements_expr(&p.expression, elements);
            }
            oxc_ast::ast::Expression::ArrowFunctionExpression(a) => match &a.body {
                oxc_ast::ast::ArrowFunctionBody::FunctionBody(b) => {
                    for s in &b.statements {
                        Self::collect_elements_stmt(s, elements);
                    }
                }
                _ => {
                    if let Some(expr) = a.body.as_expression() {
                        Self::collect_elements_expr(expr, elements);
                    }
                }
            },
            oxc_ast::ast::Expression::FunctionExpression(f) => {
                if let Some(body) = &f.body {
                    for s in &body.statements {
                        Self::collect_elements_stmt(s, elements);
                    }
                }
            }
            oxc_ast::ast::Expression::ClassExpression(c) => {
                Self::collect_elements_class(c, elements);
            }
            oxc_ast::ast::Expression::ConditionalExpression(c) => {
                Self::collect_elements_expr(&c.consequent, elements);
                Self::collect_elements_expr(&c.alternate, elements);
            }
            oxc_ast::ast::Expression::LogicalExpression(l) => {
                Self::collect_elements_expr(&l.left, elements);
                Self::collect_elements_expr(&l.right, elements);
            }
            oxc_ast::ast::Expression::BinaryExpression(b) => {
                Self::collect_elements_expr(&b.left, elements);
                Self::collect_elements_expr(&b.right, elements);
            }
            oxc_ast::ast::Expression::UnaryExpression(u) => {
                Self::collect_elements_expr(&u.argument, elements);
            }
            oxc_ast::ast::Expression::AwaitExpression(a) => {
                Self::collect_elements_expr(&a.argument, elements);
            }
            oxc_ast::ast::Expression::AssignmentExpression(a) => {
                Self::collect_elements_expr(&a.right, elements);
            }
            oxc_ast::ast::Expression::SequenceExpression(seq) => {
                for sub in &seq.expressions {
                    Self::collect_elements_expr(sub, elements);
                }
            }
            oxc_ast::ast::Expression::ArrayExpression(arr) => {
                for el in &arr.elements {
                    if let Some(el_expr) = el.as_expression() {
                        Self::collect_elements_expr(el_expr, elements);
                    } else if let oxc_ast::ast::ArrayExpressionElement::SpreadElement(sp) = el {
                        Self::collect_elements_expr(&sp.argument, elements);
                    }
                }
            }
            oxc_ast::ast::Expression::ObjectExpression(obj) => {
                for prop_kind in &obj.properties {
                    match prop_kind {
                        oxc_ast::ast::ObjectPropertyKind::ObjectProperty(prop) => {
                            Self::collect_elements_expr(&prop.value, elements);
                        }
                        oxc_ast::ast::ObjectPropertyKind::SpreadProperty(sp) => {
                            Self::collect_elements_expr(&sp.argument, elements);
                        }
                    }
                }
            }
            oxc_ast::ast::Expression::TemplateLiteral(t) => {
                for sub in &t.expressions {
                    Self::collect_elements_expr(sub, elements);
                }
            }
            oxc_ast::ast::Expression::CallExpression(c) => {
                Self::collect_elements_expr(&c.callee, elements);
                for arg in &c.arguments {
                    if let Some(expr) = arg.as_expression() {
                        Self::collect_elements_expr(expr, elements);
                    }
                }
            }
            oxc_ast::ast::Expression::ChainExpression(ch) => {
                if let oxc_ast::ast::ChainElement::CallExpression(c) = &ch.expression {
                    Self::collect_elements_expr(&c.callee, elements);
                    for arg in &c.arguments {
                        if let Some(expr) = arg.as_expression() {
                            Self::collect_elements_expr(expr, elements);
                        }
                    }
                }
            }
            oxc_ast::ast::Expression::TSAsExpression(ts) => {
                Self::collect_elements_expr(&ts.expression, elements);
            }
            oxc_ast::ast::Expression::TSSatisfiesExpression(ts) => {
                Self::collect_elements_expr(&ts.expression, elements);
            }
            oxc_ast::ast::Expression::TSTypeAssertion(ts) => {
                Self::collect_elements_expr(&ts.expression, elements);
            }
            oxc_ast::ast::Expression::TSNonNullExpression(ts) => {
                Self::collect_elements_expr(&ts.expression, elements);
            }
            _ => {}
        }
    }

    fn collect_elements_child<'a>(child: &'a JSXChild<'a>, elements: &mut Vec<&'a JSXElement<'a>>) {
        match child {
            JSXChild::Element(el) => {
                Self::collect_jsx_element(el, elements);
            }
            JSXChild::Fragment(fr) => {
                for c in &fr.children {
                    Self::collect_elements_child(c, elements);
                }
            }
            JSXChild::ExpressionContainer(c) => {
                if let Some(expr) = c.expression.as_expression() {
                    Self::collect_elements_expr(expr, elements);
                }
            }
            _ => {}
        }
    }

    fn extract_target_from_element(
        el: &JSXElement<'_>,
        patch_kind: &PatchKind,
        target_attr: Option<&str>,
        source: &str,
    ) -> Result<Option<(Span, String, TargetKind)>, HarnessError> {
        match patch_kind {
            PatchKind::TextContent | PatchKind::MetricValue => {
                for child in &el.children {
                    match child {
                        JSXChild::Text(t) => {
                            let orig =
                                source[t.span.start as usize..t.span.end as usize].to_string();
                            if !orig.trim().is_empty() || el.children.len() == 1 {
                                return Ok(Some((t.span, orig, TargetKind::JsxText)));
                            }
                        }
                        JSXChild::ExpressionContainer(c) => match &c.expression {
                            JSXExpression::StringLiteral(lit) => {
                                let orig = source[lit.span.start as usize..lit.span.end as usize]
                                    .to_string();
                                let quote = orig
                                    .chars()
                                    .next()
                                    .filter(|q| *q == '"' || *q == '\'')
                                    .unwrap_or('"');
                                return Ok(Some((
                                    lit.span,
                                    orig,
                                    TargetKind::JsStringLiteral { quote },
                                )));
                            }
                            JSXExpression::NumericLiteral(num) => {
                                let orig = source[num.span.start as usize..num.span.end as usize]
                                    .to_string();
                                return Ok(Some((num.span, orig, TargetKind::NumericLiteral)));
                            }
                            _ => {
                                // Dynamic expressions (ternaries, calls, etc.) stay protected
                                return Ok(None);
                            }
                        },
                        _ => {}
                    }
                }
                if el.children.is_empty() {
                    if let Some(closing) = &el.closing_element {
                        let span = Span::new(el.opening_element.span.end, closing.span.start);
                        return Ok(Some((span, String::new(), TargetKind::EmptyJsxElement)));
                    }
                }
            }
            PatchKind::CssVariable => {
                for attr in &el.opening_element.attributes {
                    if let JSXAttributeItem::Attribute(a) = attr {
                        if Self::jsx_attr_name(&a.name) == "style" {
                            if let Some(JSXAttributeValue::ExpressionContainer(c)) = &a.value {
                                if let Some(oxc_ast::ast::Expression::ObjectExpression(obj)) =
                                    c.expression.as_expression()
                                {
                                    for prop in &obj.properties {
                                        if let oxc_ast::ast::ObjectPropertyKind::ObjectProperty(p) =
                                            prop
                                        {
                                            let key_name = match &p.key {
                                                oxc_ast::ast::PropertyKey::StaticIdentifier(id) => {
                                                    Some(id.name.as_str())
                                                }
                                                oxc_ast::ast::PropertyKey::StringLiteral(s) => {
                                                    s.value.as_str()
                                                }
                                                _ => None,
                                            };
                                            let is_match = if let Some(target) = target_attr {
                                                key_name == Some(target)
                                            } else {
                                                key_name.is_some_and(|k| k.starts_with("--"))
                                            };
                                            if is_match {
                                                if let oxc_ast::ast::Expression::StringLiteral(
                                                    lit,
                                                ) = &p.value
                                                {
                                                    let orig = source[lit.span.start as usize
                                                        ..lit.span.end as usize]
                                                        .to_string();
                                                    let quote = orig
                                                        .chars()
                                                        .next()
                                                        .filter(|q| *q == '"' || *q == '\'')
                                                        .unwrap_or('"');
                                                    return Ok(Some((
                                                        lit.span,
                                                        orig,
                                                        TargetKind::CssVarStringLiteral { quote },
                                                    )));
                                                }
                                                return Err(HarnessError::BoundaryViolation {
                                                    reason: "CSS_VAR_NON_LITERAL: Target CSS variable must be a static string literal".to_string(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            PatchKind::AttributeValue => {
                let target_name = target_attr.unwrap_or("style");
                for attr in &el.opening_element.attributes {
                    if let JSXAttributeItem::Attribute(a) = attr {
                        let name = Self::jsx_attr_name(&a.name);
                        if name == target_name {
                            match &a.value {
                                Some(JSXAttributeValue::StringLiteral(lit)) => {
                                    let orig = source
                                        [lit.span.start as usize..lit.span.end as usize]
                                        .to_string();
                                    let quote = orig
                                        .chars()
                                        .next()
                                        .filter(|q| *q == '"' || *q == '\'')
                                        .unwrap_or('"');
                                    return Ok(Some((
                                        lit.span,
                                        orig,
                                        TargetKind::AttributeStringLiteral {
                                            attr_name: name.to_string(),
                                            quote,
                                        },
                                    )));
                                }
                                Some(_) => {
                                    return Err(HarnessError::BoundaryViolation {
                                        reason: format!(
                                            "ATTRIBUTE_EXPRESSION_BLOCKED: Attribute '{}' can only be patched when it is a string literal",
                                            name
                                        ),
                                    });
                                }
                                None => return Ok(None),
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}
