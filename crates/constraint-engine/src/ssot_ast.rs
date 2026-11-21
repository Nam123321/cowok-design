// @story 02.4
// @implements AC-02, AC-04
use std::collections::BTreeSet;

use domain::HarnessError;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPattern, Declaration, ExportDefaultDeclarationKind, Expression, JSXAttributeItem,
    JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement, JSXExpression, ObjectPropertyKind,
    PropertyKey, Statement, VariableDeclarator,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ConstraintSeverity, ConstraintViolation};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedComponentSignature {
    pub component_key: String,
    pub display_name: String,
    pub source_path: String,
    pub prop_names: Vec<String>,
    pub css_variables: Vec<String>,
    pub ff_ids: Vec<String>,
    pub jsx_tags: Vec<String>,
    pub ast_signature_sha256: String,
    pub canonical_json: String,
}

#[derive(Debug, Default)]
struct ComponentCollector {
    prop_names: BTreeSet<String>,
    css_variables: BTreeSet<String>,
    ff_ids: BTreeSet<String>,
    jsx_tags: BTreeSet<String>,
}

fn collect_css_vars_from_str(text: &str, out: &mut BTreeSet<String>) {
    let mut rest = text;
    while let Some(idx) = rest.find("--") {
        let after = &rest[idx..];
        let token: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if token.len() > 2 {
            out.insert(token);
        }
        rest = &after[2..];
    }
}

fn collect_jsx_element(el: &JSXElement<'_>, out: &mut ComponentCollector) {
    if let oxc_ast::ast::JSXElementName::Identifier(ident) = &el.opening_element.name {
        out.jsx_tags.insert(ident.name.as_str().to_string());
    } else if let oxc_ast::ast::JSXElementName::IdentifierReference(ident) =
        &el.opening_element.name
    {
        out.jsx_tags.insert(ident.name.as_str().to_string());
    }

    for attr in &el.opening_element.attributes {
        if let JSXAttributeItem::Attribute(a) = attr {
            let attr_name = match &a.name {
                JSXAttributeName::Identifier(id) => id.name.as_str(),
                JSXAttributeName::NamespacedName(ns) => ns.name.name.as_str(),
            };
            if let Some(JSXAttributeValue::StringLiteral(lit)) = &a.value {
                if let Some(s) = lit.value.as_str() {
                    if attr_name == "data-ff-id" || attr_name == "data-testid" {
                        out.ff_ids.insert(s.to_string());
                    }
                    collect_css_vars_from_str(s, &mut out.css_variables);
                }
            } else if let Some(JSXAttributeValue::ExpressionContainer(expr_box)) = &a.value {
                collect_css_vars_from_jsx_expr(&expr_box.expression, out);
            }
        }
    }

    for child in &el.children {
        match child {
            JSXChild::Element(nested) => collect_jsx_element(nested, out),
            JSXChild::Fragment(frag) => {
                for fc in &frag.children {
                    if let JSXChild::Element(nested) = fc {
                        collect_jsx_element(nested, out);
                    }
                }
            }
            JSXChild::ExpressionContainer(expr_box) => {
                collect_css_vars_from_jsx_expr(&expr_box.expression, out);
            }
            _ => {}
        }
    }
}

fn collect_css_vars_from_jsx_expr(expr: &JSXExpression<'_>, out: &mut ComponentCollector) {
    if let Some(inner) = expr.as_expression() {
        collect_from_expr(inner, out);
    }
}

fn collect_from_expr(expr: &Expression<'_>, out: &mut ComponentCollector) {
    match expr {
        Expression::StringLiteral(s) => {
            if let Some(raw) = s.value.as_str() {
                collect_css_vars_from_str(raw, &mut out.css_variables);
            }
        }
        Expression::TemplateLiteral(tpl) => {
            for q in &tpl.quasis {
                collect_css_vars_from_str(q.value.raw.as_str(), &mut out.css_variables);
            }
        }
        Expression::JSXElement(el) => collect_jsx_element(el, out),
        Expression::JSXFragment(frag) => {
            for fc in &frag.children {
                if let JSXChild::Element(nested) = fc {
                    collect_jsx_element(nested, out);
                }
            }
        }
        Expression::ArrowFunctionExpression(arrow) => {
            for param in &arrow.params.items {
                collect_props_from_pattern(&param.pattern, &mut out.prop_names);
            }
            match &arrow.body {
                oxc_ast::ast::ArrowFunctionBody::FunctionBody(b) => {
                    collect_from_statements(&b.statements, out);
                }
                _ => {
                    if let Some(expr) = arrow.body.as_expression() {
                        collect_from_expr(expr, out);
                    }
                }
            }
        }
        Expression::FunctionExpression(func) => {
            for param in &func.params.items {
                collect_props_from_pattern(&param.pattern, &mut out.prop_names);
            }
            if let Some(body) = &func.body {
                collect_from_statements(&body.statements, out);
            }
        }
        Expression::ParenthesizedExpression(p) => collect_from_expr(&p.expression, out),
        Expression::ConditionalExpression(c) => {
            collect_from_expr(&c.consequent, out);
            collect_from_expr(&c.alternate, out);
        }
        Expression::LogicalExpression(l) => {
            collect_from_expr(&l.left, out);
            collect_from_expr(&l.right, out);
        }
        Expression::ObjectExpression(obj) => {
            for prop in &obj.properties {
                if let ObjectPropertyKind::ObjectProperty(p) = prop {
                    match &p.key {
                        PropertyKey::StaticIdentifier(id) => {
                            collect_css_vars_from_str(id.name.as_str(), &mut out.css_variables);
                        }
                        PropertyKey::StringLiteral(s) => {
                            if let Some(raw) = s.value.as_str() {
                                collect_css_vars_from_str(raw, &mut out.css_variables);
                            }
                        }
                        _ => {}
                    }
                    collect_from_expr(&p.value, out);
                }
            }
        }
        Expression::ArrayExpression(arr) => {
            for el in &arr.elements {
                if let Some(inner) = el.as_expression() {
                    collect_from_expr(inner, out);
                }
            }
        }
        Expression::CallExpression(call) => {
            collect_from_expr(&call.callee, out);
            for arg in &call.arguments {
                if let Some(expr_arg) = arg.as_expression() {
                    collect_from_expr(expr_arg, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_from_statements(stmts: &[Statement<'_>], out: &mut ComponentCollector) {
    for stmt in stmts {
        collect_from_stmt(stmt, out);
    }
}

fn collect_from_stmt(stmt: &Statement<'_>, out: &mut ComponentCollector) {
    match stmt {
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                collect_from_expr(arg, out);
            }
        }
        Statement::ExpressionStatement(es) => collect_from_expr(&es.expression, out),
        Statement::VariableDeclaration(vd) => {
            for decl in &vd.declarations {
                if let Some(init) = &decl.init {
                    collect_from_expr(init, out);
                }
            }
        }
        Statement::BlockStatement(bs) => {
            for s in &bs.body {
                collect_from_stmt(s, out);
            }
        }
        Statement::IfStatement(ifs) => {
            collect_from_stmt(&ifs.consequent, out);
            if let Some(alt) = &ifs.alternate {
                collect_from_stmt(alt, out);
            }
        }
        _ => {}
    }
}

fn collect_props_from_pattern(pat: &BindingPattern<'_>, props: &mut BTreeSet<String>) {
    match pat {
        BindingPattern::ObjectPattern(obj) => {
            for prop in &obj.properties {
                if let PropertyKey::StaticIdentifier(id) = &prop.key {
                    props.insert(id.name.as_str().to_string());
                }
            }
            if let Some(rest) = &obj.rest {
                if let BindingPattern::BindingIdentifier(id) = &rest.argument {
                    props.insert(format!("...{}", id.name.as_str()));
                }
            }
        }
        BindingPattern::BindingIdentifier(id) => {
            let name = id.name.as_str();
            if name != "props" {
                props.insert(name.to_string());
            }
        }
        BindingPattern::AssignmentPattern(ap) => {
            collect_props_from_pattern(&ap.left, props);
        }
        _ => {}
    }
}

fn build_signature(
    component_name: &str,
    source_path: &str,
    collector: ComponentCollector,
) -> ExtractedComponentSignature {
    let prop_names: Vec<String> = collector.prop_names.into_iter().collect();
    let css_variables: Vec<String> = collector.css_variables.into_iter().collect();
    let ff_ids: Vec<String> = collector.ff_ids.into_iter().collect();
    let jsx_tags: Vec<String> = collector.jsx_tags.into_iter().collect();

    let canonical_payload = serde_json::json!({
        "componentKey": component_name,
        "sourcePath": source_path,
        "props": &prop_names,
        "cssVariables": &css_variables,
        "ffIds": &ff_ids,
        "jsxTags": &jsx_tags,
    });
    let canonical_json =
        serde_json::to_string(&canonical_payload).unwrap_or_else(|_| "{}".to_string());
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    let ast_signature_sha256 = format!("{:x}", hasher.finalize());

    ExtractedComponentSignature {
        component_key: component_name.to_string(),
        display_name: component_name.to_string(),
        source_path: source_path.to_string(),
        prop_names,
        css_variables,
        ff_ids,
        jsx_tags,
        ast_signature_sha256,
        canonical_json,
    }
}

fn process_var_decl(
    decl: &VariableDeclarator<'_>,
    source_path: &str,
    out: &mut Vec<ExtractedComponentSignature>,
) {
    let name = match &decl.id {
        BindingPattern::BindingIdentifier(id) => id.name.as_str().to_string(),
        _ => return,
    };
    let Some(init) = &decl.init else {
        return;
    };
    let is_component_name = name
        .chars()
        .next()
        .map(|c| c.is_ascii_uppercase())
        .unwrap_or(false);
    let mut collector = ComponentCollector::default();
    collect_from_expr(init, &mut collector);
    if is_component_name || !collector.jsx_tags.is_empty() || !collector.ff_ids.is_empty() {
        out.push(build_signature(&name, source_path, collector));
    }
}

/// Parses TSX/JSX/TS source code with `oxc_parser` and extracts deterministic component
/// signatures (props, CSS theme variables, `data-ff-id` anchors, JSX tags, SHA-256 digest).
pub fn extract_component_signatures(
    source_path: &str,
    source_code: &str,
) -> Result<Vec<ExtractedComponentSignature>, HarnessError> {
    if source_path.trim().is_empty() {
        return Err(HarnessError::ValidationFailed {
            reason: "AST_SOURCE_PATH_EMPTY".to_string(),
        });
    }
    let allocator = Allocator::default();
    let source_type = SourceType::tsx();
    let parsed = Parser::new(&allocator, source_code, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(HarnessError::ValidationFailed {
            reason: format!("AST_PARSE_ERROR: failed to parse {source_path}"),
        });
    }

    let mut signatures = Vec::new();
    for stmt in &parsed.program.body {
        match stmt {
            Statement::ExportDeclaration(exp) => match &exp.declaration {
                Declaration::FunctionDeclaration(func) => {
                    if let Some(id) = &func.id {
                        let mut collector = ComponentCollector::default();
                        for param in &func.params.items {
                            collect_props_from_pattern(&param.pattern, &mut collector.prop_names);
                        }
                        if let Some(body) = &func.body {
                            collect_from_statements(&body.statements, &mut collector);
                        }
                        signatures.push(build_signature(id.name.as_str(), source_path, collector));
                    }
                }
                Declaration::VariableDeclaration(vd) => {
                    for decl in &vd.declarations {
                        process_var_decl(decl, source_path, &mut signatures);
                    }
                }
                _ => {}
            },
            Statement::ExportDefaultDeclaration(exp) => match &exp.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                    let name = func
                        .id
                        .as_ref()
                        .map(|i| i.name.as_str())
                        .unwrap_or("DefaultExportComponent");
                    let mut collector = ComponentCollector::default();
                    for param in &func.params.items {
                        collect_props_from_pattern(&param.pattern, &mut collector.prop_names);
                    }
                    if let Some(body) = &func.body {
                        collect_from_statements(&body.statements, &mut collector);
                    }
                    signatures.push(build_signature(name, source_path, collector));
                }
                other => {
                    if let Some(expr) = other.as_expression() {
                        let mut collector = ComponentCollector::default();
                        collect_from_expr(expr, &mut collector);
                        if !collector.jsx_tags.is_empty() || !collector.ff_ids.is_empty() {
                            signatures.push(build_signature(
                                "DefaultExportComponent",
                                source_path,
                                collector,
                            ));
                        }
                    }
                }
            },
            Statement::FunctionDeclaration(func) => {
                if let Some(id) = &func.id {
                    let name = id.name.as_str();
                    if name
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_uppercase())
                        .unwrap_or(false)
                    {
                        let mut collector = ComponentCollector::default();
                        for param in &func.params.items {
                            collect_props_from_pattern(&param.pattern, &mut collector.prop_names);
                        }
                        if let Some(body) = &func.body {
                            collect_from_statements(&body.statements, &mut collector);
                        }
                        if !collector.jsx_tags.is_empty() || !collector.ff_ids.is_empty() {
                            signatures.push(build_signature(name, source_path, collector));
                        }
                    }
                }
            }
            Statement::VariableDeclaration(vd) => {
                for decl in &vd.declarations {
                    process_var_decl(decl, source_path, &mut signatures);
                }
            }
            _ => {}
        }
    }

    Ok(signatures)
}

/// Evaluates whether modifying `pattern_key` violates an active `app_shell_lock` held by another actor.
pub fn check_appshell_lock_constraint(
    pattern_key: &str,
    active_lock_owner: Option<&str>,
    caller_actor: &str,
    active_expires_at: Option<i64>,
    now_ms: i64,
) -> Option<ConstraintViolation> {
    let owner = active_lock_owner.unwrap_or("").trim();
    if owner.is_empty() {
        return None;
    }
    let expires_at = active_expires_at.unwrap_or(0);
    if expires_at <= now_ms {
        return None;
    }
    if owner == caller_actor.trim() {
        return None;
    }

    Some(ConstraintViolation {
        rule_code: "APPSHELL_LOCK_QUARANTINE".to_string(),
        severity: ConstraintSeverity::Blocker.as_str().to_string(),
        message: format!(
            "Pattern `{pattern_key}` is actively locked by `{owner}` until {expires_at}; reverse sync from `{caller_actor}` must be quarantined."
        ),
        line: None,
        column: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tsx_component_props_css_vars_and_ff_ids() {
        let tsx = r#"
            export function AppShellPanel({ portalId, isLocked }: { portalId: string; isLocked: boolean }) {
                return (
                    <section data-ff-id="appshell-lock-panel" style={{ background: "var(--bg-card)", color: "var(--text-primary)" }}>
                        <button data-testid="lock-btn">{portalId}</button>
                    </section>
                );
            }
        "#;
        let sigs = extract_component_signatures("src/features/ssot/AppShellPanel.tsx", tsx)
            .expect("oxc should parse tsx");
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].component_key, "AppShellPanel");
        assert_eq!(sigs[0].prop_names, vec!["isLocked", "portalId"]);
        assert_eq!(
            sigs[0].css_variables,
            vec!["--bg-card", "--text-primary"]
        );
        assert_eq!(
            sigs[0].ff_ids,
            vec!["appshell-lock-panel", "lock-btn"]
        );
        assert_eq!(sigs[0].ast_signature_sha256.len(), 64);
    }

    #[test]
    fn lock_constraint_blocks_other_actor_until_expiry() {
        assert!(check_appshell_lock_constraint(
            "AppShell",
            Some("user-a"),
            "sync-bot",
            Some(2_000),
            1_000,
        )
        .is_some());

        assert!(check_appshell_lock_constraint(
            "AppShell",
            Some("user-a"),
            "user-a",
            Some(2_000),
            1_000,
        )
        .is_none());

        assert!(check_appshell_lock_constraint(
            "AppShell",
            Some("user-a"),
            "sync-bot",
            Some(1_000),
            1_000,
        )
        .is_none());
    }
}
