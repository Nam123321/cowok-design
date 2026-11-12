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

pub struct SurgicalPatcher;

impl SurgicalPatcher {
    /// Injects deterministic `data-ff-id` attributes into all JSX elements in `source`
    /// that do not already have one.
    pub fn inject_ff_ids(
        source: &str,
        component_name: Option<&str>,
    ) -> Result<AstInjectionResponse, HarnessError> {
        let allocator = Allocator::default();
        let source_type = SourceType::tsx();
        let parsed = Parser::new(&allocator, source, source_type).parse();

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

        let mut generated_ids = Vec::new();
        // Stores (insertion_offset, attribute_string)
        let mut insertions: Vec<(usize, String)> = Vec::new();

        let mut tag_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for el in elements {
            // Check if element already has data-ff-id
            let existing_ff_id = el.opening_element.attributes.iter().find_map(|attr| {
                if let JSXAttributeItem::Attribute(a) = attr {
                    let name = match &a.name {
                        JSXAttributeName::Identifier(id) => id.name.as_str(),
                        JSXAttributeName::NamespacedName(ns) => ns.name.name.as_str(),
                    };
                    if name == "data-ff-id" {
                        if let Some(JSXAttributeValue::StringLiteral(lit)) = &a.value {
                            return lit.value.as_str().map(|s| s.to_string());
                        }
                    }
                }
                None
            });

            if let Some(existing) = existing_ff_id {
                generated_ids.push(existing);
                continue;
            }

            let tag_name = match &el.opening_element.name {
                oxc_ast::ast::JSXElementName::Identifier(id) => id.name.as_str(),
                oxc_ast::ast::JSXElementName::NamespacedName(ns) => ns.name.name.as_str(),
                oxc_ast::ast::JSXElementName::MemberExpression(m) => m.property.name.as_str(),
                _ => "el",
            };

            let clean_tag = tag_name.to_lowercase();
            let count = tag_counts.entry(clean_tag.clone()).or_insert(0);
            *count += 1;

            let ff_id = format!("ff-{}-{}-{}", cmp, clean_tag, count);
            generated_ids.push(ff_id.clone());

            // Determine insertion point right after tag name in opening element
            let insert_offset = el.opening_element.name.span().end as usize;
            insertions.push((insert_offset, format!(" data-ff-id=\"{}\"", ff_id)));
        }

        let total_elements_tagged = insertions.len();

        // Sort descending by offset so insertions don't shift prior byte offsets
        insertions.sort_by(|a, b| b.0.cmp(&a.0));

        let mut tagged_source = source.to_string();
        for (offset, text) in insertions {
            if offset <= tagged_source.len() {
                tagged_source.insert_str(offset, &text);
            }
        }

        Ok(AstInjectionResponse {
            total_elements_tagged,
            tagged_source,
            generated_ids,
        })
    }

    /// Surgically patches text, metric, CSS variable, or attribute of the JSX element identified
    /// by `data-ff-id` in < 1ms without mutating surrounding AST structure or formatting (AC-03 / EC-E03-03).
    pub fn patch_by_ff_id(
        source: &str,
        req: &SurgicalPatchRequest,
    ) -> Result<SurgicalPatchResponse, HarnessError> {
        let start = Instant::now();

        let allocator = Allocator::default();
        let source_type = SourceType::tsx();
        let parsed = Parser::new(&allocator, source, source_type).parse();

        let mut target_span: Option<(Span, String)> = None;

        for stmt in &parsed.program.body {
            if let Some(found) = Self::find_target_in_stmt(
                stmt,
                &req.ff_id,
                &req.patch_kind,
                req.target_attribute.as_deref(),
                source,
            ) {
                target_span = Some(found);
                break;
            }
        }

        let elapsed = start.elapsed().as_micros() as u64;

        if let Some((span, original_text)) = target_span {
            let start_byte = span.start as usize;
            let end_byte = span.end as usize;

            if start_byte <= end_byte && end_byte <= source.len() {
                let replacement = if matches!(
                    req.patch_kind,
                    PatchKind::AttributeValue | PatchKind::CssVariable
                ) {
                    if original_text.starts_with('"')
                        && original_text.ends_with('"')
                        && !req.new_value.starts_with('"')
                    {
                        format!("\"{}\"", req.new_value)
                    } else if original_text.starts_with('\'')
                        && original_text.ends_with('\'')
                        && !req.new_value.starts_with('\'')
                    {
                        format!("'{}'", req.new_value)
                    } else {
                        req.new_value.clone()
                    }
                } else {
                    req.new_value.clone()
                };

                let mut patched_source = String::with_capacity(source.len() + replacement.len());
                patched_source.push_str(&source[..start_byte]);
                patched_source.push_str(&replacement);
                patched_source.push_str(&source[end_byte..]);

                return Ok(SurgicalPatchResponse {
                    ff_id: req.ff_id.clone(),
                    success: true,
                    original_text: Some(original_text),
                    patched_source,
                    execution_time_micros: elapsed,
                });
            }
        }

        Ok(SurgicalPatchResponse {
            ff_id: req.ff_id.clone(),
            success: false,
            original_text: None,
            patched_source: source.to_string(),
            execution_time_micros: elapsed,
        })
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
            _ => {}
        }
    }

    fn collect_elements_expr<'a>(
        expr: &'a oxc_ast::ast::Expression<'a>,
        elements: &mut Vec<&'a JSXElement<'a>>,
    ) {
        match expr {
            oxc_ast::ast::Expression::JSXElement(el) => {
                elements.push(el);
                for child in &el.children {
                    Self::collect_elements_child(child, elements);
                }
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
            oxc_ast::ast::Expression::ConditionalExpression(c) => {
                Self::collect_elements_expr(&c.consequent, elements);
                Self::collect_elements_expr(&c.alternate, elements);
            }
            oxc_ast::ast::Expression::LogicalExpression(l) => {
                Self::collect_elements_expr(&l.left, elements);
                Self::collect_elements_expr(&l.right, elements);
            }
            oxc_ast::ast::Expression::CallExpression(c) => {
                Self::collect_elements_expr(&c.callee, elements);
                for arg in &c.arguments {
                    if let Some(expr) = arg.as_expression() {
                        Self::collect_elements_expr(expr, elements);
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_elements_child<'a>(child: &'a JSXChild<'a>, elements: &mut Vec<&'a JSXElement<'a>>) {
        match child {
            JSXChild::Element(el) => {
                elements.push(el);
                for c in &el.children {
                    Self::collect_elements_child(c, elements);
                }
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

    fn find_target_in_stmt<'a>(
        stmt: &'a Statement<'a>,
        target_ff_id: &str,
        patch_kind: &PatchKind,
        target_attr: Option<&str>,
        source: &str,
    ) -> Option<(Span, String)> {
        match stmt {
            Statement::ExpressionStatement(e) => Self::find_target_in_expr(
                &e.expression,
                target_ff_id,
                patch_kind,
                target_attr,
                source,
            ),
            Statement::ExportDefaultDeclaration(d) => match &d.declaration {
                oxc_ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                    if let Some(body) = &f.body {
                        for s in &body.statements {
                            if let Some(found) = Self::find_target_in_stmt(
                                s,
                                target_ff_id,
                                patch_kind,
                                target_attr,
                                source,
                            ) {
                                return Some(found);
                            }
                        }
                    }
                    None
                }
                _ => {
                    if let Some(expr) = d.declaration.as_expression() {
                        Self::find_target_in_expr(
                            expr,
                            target_ff_id,
                            patch_kind,
                            target_attr,
                            source,
                        )
                    } else {
                        None
                    }
                }
            },
            Statement::ExportDeclaration(d) => match &d.declaration {
                oxc_ast::ast::Declaration::VariableDeclaration(v) => {
                    for decl in &v.declarations {
                        if let Some(init) = &decl.init {
                            if let Some(found) = Self::find_target_in_expr(
                                init,
                                target_ff_id,
                                patch_kind,
                                target_attr,
                                source,
                            ) {
                                return Some(found);
                            }
                        }
                    }
                    None
                }
                oxc_ast::ast::Declaration::FunctionDeclaration(f) => {
                    if let Some(body) = &f.body {
                        for s in &body.statements {
                            if let Some(found) = Self::find_target_in_stmt(
                                s,
                                target_ff_id,
                                patch_kind,
                                target_attr,
                                source,
                            ) {
                                return Some(found);
                            }
                        }
                    }
                    None
                }
                _ => None,
            },
            Statement::VariableDeclaration(v) => {
                for decl in &v.declarations {
                    if let Some(init) = &decl.init {
                        if let Some(found) = Self::find_target_in_expr(
                            init,
                            target_ff_id,
                            patch_kind,
                            target_attr,
                            source,
                        ) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            Statement::FunctionDeclaration(f) => {
                if let Some(body) = &f.body {
                    for s in &body.statements {
                        if let Some(found) = Self::find_target_in_stmt(
                            s,
                            target_ff_id,
                            patch_kind,
                            target_attr,
                            source,
                        ) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            Statement::BlockStatement(b) => {
                for s in &b.body {
                    if let Some(found) =
                        Self::find_target_in_stmt(s, target_ff_id, patch_kind, target_attr, source)
                    {
                        return Some(found);
                    }
                }
                None
            }
            Statement::ReturnStatement(r) => {
                if let Some(arg) = &r.argument {
                    Self::find_target_in_expr(arg, target_ff_id, patch_kind, target_attr, source)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn find_target_in_expr<'a>(
        expr: &'a oxc_ast::ast::Expression<'a>,
        target_ff_id: &str,
        patch_kind: &PatchKind,
        target_attr: Option<&str>,
        source: &str,
    ) -> Option<(Span, String)> {
        match expr {
            oxc_ast::ast::Expression::JSXElement(el) => {
                Self::find_target_in_jsx_element(el, target_ff_id, patch_kind, target_attr, source)
            }
            oxc_ast::ast::Expression::JSXFragment(fr) => {
                for child in &fr.children {
                    if let Some(found) = Self::find_target_in_child(
                        child,
                        target_ff_id,
                        patch_kind,
                        target_attr,
                        source,
                    ) {
                        return Some(found);
                    }
                }
                None
            }
            oxc_ast::ast::Expression::ParenthesizedExpression(p) => Self::find_target_in_expr(
                &p.expression,
                target_ff_id,
                patch_kind,
                target_attr,
                source,
            ),
            oxc_ast::ast::Expression::ArrowFunctionExpression(a) => match &a.body {
                oxc_ast::ast::ArrowFunctionBody::FunctionBody(b) => {
                    for s in &b.statements {
                        if let Some(found) = Self::find_target_in_stmt(
                            s,
                            target_ff_id,
                            patch_kind,
                            target_attr,
                            source,
                        ) {
                            return Some(found);
                        }
                    }
                    None
                }
                _ => {
                    if let Some(expr) = a.body.as_expression() {
                        Self::find_target_in_expr(
                            expr,
                            target_ff_id,
                            patch_kind,
                            target_attr,
                            source,
                        )
                    } else {
                        None
                    }
                }
            },
            oxc_ast::ast::Expression::FunctionExpression(f) => {
                if let Some(body) = &f.body {
                    for s in &body.statements {
                        if let Some(found) = Self::find_target_in_stmt(
                            s,
                            target_ff_id,
                            patch_kind,
                            target_attr,
                            source,
                        ) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            oxc_ast::ast::Expression::ConditionalExpression(c) => {
                if let Some(found) = Self::find_target_in_expr(
                    &c.consequent,
                    target_ff_id,
                    patch_kind,
                    target_attr,
                    source,
                ) {
                    return Some(found);
                }
                Self::find_target_in_expr(
                    &c.alternate,
                    target_ff_id,
                    patch_kind,
                    target_attr,
                    source,
                )
            }
            oxc_ast::ast::Expression::LogicalExpression(l) => {
                if let Some(found) = Self::find_target_in_expr(
                    &l.left,
                    target_ff_id,
                    patch_kind,
                    target_attr,
                    source,
                ) {
                    return Some(found);
                }
                Self::find_target_in_expr(&l.right, target_ff_id, patch_kind, target_attr, source)
            }
            oxc_ast::ast::Expression::CallExpression(c) => {
                if let Some(found) = Self::find_target_in_expr(
                    &c.callee,
                    target_ff_id,
                    patch_kind,
                    target_attr,
                    source,
                ) {
                    return Some(found);
                }
                for arg in &c.arguments {
                    if let Some(expr) = arg.as_expression() {
                        if let Some(found) = Self::find_target_in_expr(
                            expr,
                            target_ff_id,
                            patch_kind,
                            target_attr,
                            source,
                        ) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn find_target_in_child<'a>(
        child: &'a JSXChild<'a>,
        target_ff_id: &str,
        patch_kind: &PatchKind,
        target_attr: Option<&str>,
        source: &str,
    ) -> Option<(Span, String)> {
        match child {
            JSXChild::Element(el) => {
                Self::find_target_in_jsx_element(el, target_ff_id, patch_kind, target_attr, source)
            }
            JSXChild::Fragment(fr) => {
                for c in &fr.children {
                    if let Some(found) =
                        Self::find_target_in_child(c, target_ff_id, patch_kind, target_attr, source)
                    {
                        return Some(found);
                    }
                }
                None
            }
            JSXChild::ExpressionContainer(c) => {
                if let Some(expr) = c.expression.as_expression() {
                    Self::find_target_in_expr(expr, target_ff_id, patch_kind, target_attr, source)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn find_target_in_jsx_element<'a>(
        el: &'a JSXElement<'a>,
        target_ff_id: &str,
        patch_kind: &PatchKind,
        target_attr: Option<&str>,
        source: &str,
    ) -> Option<(Span, String)> {
        let is_target = el.opening_element.attributes.iter().any(|attr| {
            if let JSXAttributeItem::Attribute(a) = attr {
                let name = match &a.name {
                    JSXAttributeName::Identifier(id) => id.name.as_str(),
                    JSXAttributeName::NamespacedName(ns) => ns.name.name.as_str(),
                };
                if name == "data-ff-id" {
                    if let Some(JSXAttributeValue::StringLiteral(lit)) = &a.value {
                        return lit.value.as_str() == Some(target_ff_id);
                    }
                }
            }
            false
        });

        if is_target {
            match patch_kind {
                PatchKind::TextContent | PatchKind::MetricValue => {
                    // Search children for Text or ExpressionContainer
                    for child in &el.children {
                        match child {
                            JSXChild::Text(t) => {
                                let orig =
                                    source[t.span.start as usize..t.span.end as usize].to_string();
                                if !orig.trim().is_empty() || el.children.len() == 1 {
                                    return Some((t.span, orig));
                                }
                            }
                            JSXChild::ExpressionContainer(c) => match &c.expression {
                                JSXExpression::StringLiteral(lit) => {
                                    let orig = source
                                        [lit.span.start as usize..lit.span.end as usize]
                                        .to_string();
                                    return Some((lit.span, orig));
                                }
                                JSXExpression::NumericLiteral(num) => {
                                    let orig = source
                                        [num.span.start as usize..num.span.end as usize]
                                        .to_string();
                                    return Some((num.span, orig));
                                }
                                _ => {
                                    // Task 6.3 (P0-3): Dynamic expressions (ternaries, calls, etc.) are protected; do not overwrite reactive logic!
                                    return None;
                                }
                            },
                            _ => {}
                        }
                    }
                    // If no child text exists, only target inner span if element has ZERO children (empty element)
                    if el.children.is_empty() {
                        if let Some(closing) = &el.closing_element {
                            let span = Span::new(el.opening_element.span.end, closing.span.start);
                            return Some((span, String::new()));
                        }
                    }
                }
                PatchKind::CssVariable => {
                    // Task 6.4 (P0-4): Target specific CSS variable property inside style={{ ... }} without wiping style object
                    for attr in &el.opening_element.attributes {
                        if let JSXAttributeItem::Attribute(a) = attr {
                            let name = match &a.name {
                                JSXAttributeName::Identifier(id) => id.name.as_str(),
                                JSXAttributeName::NamespacedName(ns) => ns.name.name.as_str(),
                            };
                            if name == "style" {
                                if let Some(JSXAttributeValue::ExpressionContainer(c)) = &a.value {
                                    if let Some(oxc_ast::ast::Expression::ObjectExpression(obj)) =
                                        c.expression.as_expression()
                                    {
                                        for prop in &obj.properties {
                                            if let oxc_ast::ast::ObjectPropertyKind::ObjectProperty(p) = prop {
                                                let key_name = match &p.key {
                                                    oxc_ast::ast::PropertyKey::StaticIdentifier(id) => Some(id.name.as_str()),
                                                    oxc_ast::ast::PropertyKey::StringLiteral(s) => s.value.as_str(),
                                                    _ => None,
                                                };
                                                let is_match = if let Some(target) = target_attr {
                                                    key_name == Some(target)
                                                } else {
                                                    key_name.map_or(false, |k| k.starts_with("--"))
                                                };
                                                if is_match {
                                                    let span = p.value.span();
                                                    let orig = source[span.start as usize..span.end as usize].to_string();
                                                    return Some((span, orig));
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
                    // Search attributes for target attribute
                    for attr in &el.opening_element.attributes {
                        if let JSXAttributeItem::Attribute(a) = attr {
                            let name = match &a.name {
                                JSXAttributeName::Identifier(id) => id.name.as_str(),
                                JSXAttributeName::NamespacedName(ns) => ns.name.name.as_str(),
                            };
                            let is_matching = if let Some(target) = target_attr {
                                name == target
                            } else {
                                name == "style"
                            };
                            if is_matching {
                                if let Some(val) = &a.value {
                                    let span = val.span();
                                    let orig =
                                        source[span.start as usize..span.end as usize].to_string();
                                    return Some((span, orig));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Recursively inspect children
        for child in &el.children {
            if let Some(found) =
                Self::find_target_in_child(child, target_ff_id, patch_kind, target_attr, source)
            {
                return Some(found);
            }
        }

        None
    }
}
