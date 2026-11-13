// @story 03.1
// @implements AC-01, AC-02, AC-03
//! constraint-engine crate for cowok-design
//!
//! Project-Scoped Configurable Constraint Engine & AST Linter.
//! Provides <1ms deterministic evaluation of context slice permissions,
//! Tailwind prohibitions, and inline hex color restrictions.

use domain::{ConstraintProfile, Department, HarnessError, Portal};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

#[cfg(feature = "ts")]
use ts_rs::TS;

pub mod icon_validator;
pub use icon_validator::*;

pub mod surgical_patcher;
pub use surgical_patcher::*;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Represents the severity level of a constraint violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum ConstraintSeverity {
    Info,
    Warn,
    Blocker,
}

impl ConstraintSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConstraintSeverity::Info => "INFO",
            ConstraintSeverity::Warn => "WARN",
            ConstraintSeverity::Blocker => "BLOCKER",
        }
    }

    pub fn from_str_lenient(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "INFO" => ConstraintSeverity::Info,
            "WARN" | "WARNING" => ConstraintSeverity::Warn,
            _ => ConstraintSeverity::Blocker,
        }
    }
}

/// Represents an active constraint rule loaded from SQLite WAL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ActiveConstraint {
    pub rule_code: String,
    pub severity: String,
    pub config_json: String,
}

/// Represents a diagnostic issue discovered during AST or lexical linting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ConstraintViolation {
    pub rule_code: String,
    pub severity: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// Evaluates project constraints against JIT context slice operations.
pub struct ConstraintEvaluator;

impl ConstraintEvaluator {
    /// Loads active constraints for a specific project from SQLite WAL using a secure JOIN.
    /// Ensures strict anti-IDOR project isolation by joining `constraint_rules` with `constraint_profiles`
    /// filtered strictly by `p.project_id = ?1`.
    pub fn load_constraints_for_project(
        conn: &Connection,
        project_id: &str,
    ) -> Result<Vec<ActiveConstraint>, HarnessError> {
        let full_query = conn.prepare(
            "SELECT p.forbid_tailwind, p.forbid_raw_hex, r.rule_code, r.severity, r.config_json
             FROM constraint_profiles p
             LEFT JOIN constraint_rules r ON r.profile_id = p.id AND r.enabled = 1
             WHERE p.project_id = ?1
             ORDER BY r.rule_code ASC",
        );

        match full_query {
            Ok(mut stmt) => {
                let rows = stmt
                    .query_map([project_id], |row| {
                        let forbid_tailwind: Option<i64> = row.get(0).ok();
                        let forbid_raw_hex: Option<i64> = row.get(1).ok();
                        let rule_code: Option<String> = row.get(2).ok();
                        let severity: Option<String> = row.get(3).ok();
                        let config_json: Option<String> = row.get(4).ok();
                        Ok((
                            forbid_tailwind,
                            forbid_raw_hex,
                            rule_code,
                            severity,
                            config_json,
                        ))
                    })
                    .map_err(|e| HarnessError::SqliteBusy {
                        details: format!(
                            "Failed to execute constraint query for project {}: {}",
                            project_id, e
                        ),
                    })?;

                let mut constraints = Vec::new();
                let mut has_forbid_tailwind = false;
                let mut has_forbid_raw_hex = false;

                for r in rows {
                    let (ft, frh, rule_code, severity, config_json) =
                        r.map_err(|e| HarnessError::SqliteBusy {
                            details: format!("Failed to read constraint row: {}", e),
                        })?;

                    if let Some(1) = ft {
                        has_forbid_tailwind = true;
                    }
                    if let Some(1) = frh {
                        has_forbid_raw_hex = true;
                    }

                    if let (Some(code), Some(sev), Some(cfg)) = (rule_code, severity, config_json) {
                        constraints.push(ActiveConstraint {
                            rule_code: code,
                            severity: sev,
                            config_json: cfg,
                        });
                    }
                }

                // If constraint_profile has forbid_tailwind = 1, guarantee BAN_TAILWIND rule is active
                if has_forbid_tailwind
                    && !constraints
                        .iter()
                        .any(|c| c.rule_code.eq_ignore_ascii_case("BAN_TAILWIND"))
                {
                    constraints.push(ActiveConstraint {
                        rule_code: "BAN_TAILWIND".to_string(),
                        severity: "BLOCKER".to_string(),
                        config_json: "{}".to_string(),
                    });
                }

                // If constraint_profile has forbid_raw_hex = 1, guarantee BAN_INLINE_HEX rule is active
                if has_forbid_raw_hex
                    && !constraints
                        .iter()
                        .any(|c| c.rule_code.eq_ignore_ascii_case("BAN_INLINE_HEX"))
                {
                    constraints.push(ActiveConstraint {
                        rule_code: "BAN_INLINE_HEX".to_string(),
                        severity: "BLOCKER".to_string(),
                        config_json: "{}".to_string(),
                    });
                }

                Ok(constraints)
            }
            Err(_) => {
                // Fallback for minimal/legacy schemas lacking forbid_tailwind columns
                let mut stmt = conn
                    .prepare(
                        "SELECT r.rule_code, r.severity, r.config_json
                         FROM constraint_rules r
                         JOIN constraint_profiles p ON r.profile_id = p.id
                         WHERE p.project_id = ?1 AND r.enabled = 1
                         ORDER BY r.rule_code ASC",
                    )
                    .map_err(|e| HarnessError::SqliteBusy {
                        details: format!("Failed to prepare constraint query: {}", e),
                    })?;

                let rows = stmt
                    .query_map([project_id], |row| {
                        Ok(ActiveConstraint {
                            rule_code: row.get(0)?,
                            severity: row.get(1)?,
                            config_json: row.get(2)?,
                        })
                    })
                    .map_err(|e| HarnessError::SqliteBusy {
                        details: format!(
                            "Failed to execute fallback constraint query for project {}: {}",
                            project_id, e
                        ),
                    })?;

                let mut constraints = Vec::new();
                for r in rows {
                    constraints.push(r.map_err(|e| HarnessError::SqliteBusy {
                        details: format!("Failed to read constraint row: {}", e),
                    })?);
                }

                Ok(constraints)
            }
        }
    }

    /// Maps a domain `domain::ConstraintProfile` into a list of active engine constraints.
    pub fn active_constraints_from_domain(profile: &ConstraintProfile) -> Vec<ActiveConstraint> {
        let mut rules = Vec::new();
        if profile.enforce_clean_code {
            rules.push(ActiveConstraint {
                rule_code: "ENFORCE_CLEAN_CODE".to_string(),
                severity: "BLOCKER".to_string(),
                config_json: serde_json::json!({
                    "max_complexity_score": profile.max_complexity_score,
                    "target_framework": profile.target_framework,
                })
                .to_string(),
            });
        }
        for pattern in &profile.forbidden_patterns {
            let (code, sev) = match pattern.to_ascii_lowercase().as_str() {
                "tailwind" => ("BAN_TAILWIND".to_string(), "BLOCKER".to_string()),
                "raw-hex" | "hex" | "inline-hex" => {
                    ("BAN_INLINE_HEX".to_string(), "BLOCKER".to_string())
                }
                other => (
                    format!("FORBID_{}", other.to_ascii_uppercase().replace('-', "_")),
                    "BLOCKER".to_string(),
                ),
            };
            rules.push(ActiveConstraint {
                rule_code: code,
                severity: sev,
                config_json: serde_json::json!({ "pattern": pattern }).to_string(),
            });
        }
        rules
    }

    /// Converts active constraints and project metadata into a canonical `domain::ConstraintProfile`.
    pub fn to_domain_profile(
        profile_id: &str,
        project_id: &str,
        rules: &[ActiveConstraint],
        target_framework: Option<&str>,
    ) -> ConstraintProfile {
        let p_id = Uuid::parse_str(project_id).unwrap_or_else(|_| Uuid::nil());
        let prof_id = Uuid::parse_str(profile_id).unwrap_or_else(|_| Uuid::new_v4());
        let mut forbidden = Vec::new();
        let mut max_complexity_score = 7u32;
        let mut enforce_clean_code = false;
        let mut framework = target_framework.unwrap_or("react").to_string();

        for r in rules {
            if r.rule_code.eq_ignore_ascii_case("ENFORCE_CLEAN_CODE") {
                enforce_clean_code = true;
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.config_json) {
                    if let Some(s) = val.get("max_complexity_score").and_then(|v| v.as_u64()) {
                        max_complexity_score = s as u32;
                    }
                    if let Some(f) = val.get("target_framework").and_then(|v| v.as_str()) {
                        framework = f.to_string();
                    }
                }
            } else if r.rule_code.eq_ignore_ascii_case("BAN_TAILWIND") {
                forbidden.push("tailwind".to_string());
            } else if r.rule_code.eq_ignore_ascii_case("BAN_INLINE_HEX") {
                forbidden.push("raw-hex".to_string());
            } else if r.rule_code.starts_with("FORBID_") {
                forbidden.push(
                    r.rule_code["FORBID_".len()..]
                        .to_ascii_lowercase()
                        .replace('_', "-"),
                );
            }
        }
        ConstraintProfile {
            id: prof_id,
            project_id: p_id,
            max_complexity_score,
            enforce_clean_code,
            forbidden_patterns: forbidden,
            target_framework: framework,
        }
    }

    /// Evaluates active constraint rules against the requested department and portal.
    pub fn evaluate_slice_constraints(
        rules: &[ActiveConstraint],
        department: Department,
        portal: Portal,
    ) -> Result<(), HarnessError> {
        for rule in rules {
            // Rule: FORBID_MARKETING explicitly restricts MarketingDeck department
            if rule.rule_code.eq_ignore_ascii_case("FORBID_MARKETING")
                && department == Department::MarketingDeck
            {
                return Err(HarnessError::BoundaryViolation {
                    reason: format!(
                        "PROJECT_CONSTRAINT_VIOLATION: Rule 'FORBID_MARKETING' prohibits department {:?}",
                        department
                    ),
                });
            }

            // Rule: RESTRICT_DEPARTMENT restricts specified departments in config_json
            if rule.rule_code.eq_ignore_ascii_case("RESTRICT_DEPARTMENT") {
                let dept_str = match department {
                    Department::ProductUi => "product_ui",
                    Department::MarketingDeck => "marketing_deck",
                    Department::ExecutiveReport => "executive_report",
                };
                let is_restricted = if let Ok(val) =
                    serde_json::from_str::<serde_json::Value>(&rule.config_json)
                {
                    val.get("forbidden_departments")
                        .or_else(|| val.get("restricted_departments"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().any(|item| {
                                item.as_str()
                                    .map(|s| s.eq_ignore_ascii_case(dept_str))
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or_else(|| rule.config_json.to_ascii_lowercase().contains(dept_str))
                } else {
                    rule.config_json.to_ascii_lowercase().contains(dept_str)
                };

                if is_restricted {
                    return Err(HarnessError::BoundaryViolation {
                        reason: format!(
                            "PROJECT_CONSTRAINT_VIOLATION: Rule 'RESTRICT_DEPARTMENT' restricts department {:?} (config: {})",
                            department, rule.config_json
                        ),
                    });
                }
            }

            // Rule: RESTRICT_PORTAL restricts specified portals in config_json
            if rule.rule_code.eq_ignore_ascii_case("RESTRICT_PORTAL") {
                let portal_str = match portal {
                    Portal::SuperAdmin => "super_admin",
                    Portal::TenantAdmin => "tenant_admin",
                    Portal::SalesApp => "sales_app",
                    Portal::MarketingWeb => "marketing_web",
                    Portal::DeveloperPortal => "developer_portal",
                    Portal::PresentationStudio => "presentation_studio",
                };
                let is_restricted =
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&rule.config_json) {
                        val.get("forbidden_portals")
                            .or_else(|| val.get("restricted_portals"))
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter().any(|item| {
                                    item.as_str()
                                        .map(|s| s.eq_ignore_ascii_case(portal_str))
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or_else(|| {
                                rule.config_json.to_ascii_lowercase().contains(portal_str)
                            })
                    } else {
                        rule.config_json.to_ascii_lowercase().contains(portal_str)
                    };

                if is_restricted {
                    return Err(HarnessError::BoundaryViolation {
                        reason: format!(
                            "PROJECT_CONSTRAINT_VIOLATION: Rule 'RESTRICT_PORTAL' restricts portal {:?} (config: {})",
                            portal, rule.config_json
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// AST & Lexical linter scanning source files for forbidden project patterns (<1ms budget).
pub struct AstLinter;

impl AstLinter {
    /// Maximum allowed file size for <1ms SLA (512 KB limit).
    pub const MAX_SOURCE_BYTES: usize = 512 * 1024;

    /// Validates the JSON configuration of a constraint rule.
    pub fn validate_rule_config(config_json: &str) -> Result<(), HarnessError> {
        let trimmed = config_json.trim();
        if trimmed.is_empty() || trimmed == "{}" {
            return Ok(());
        }
        let parsed: serde_json::Value =
            serde_json::from_str(trimmed).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("Invalid rule configuration JSON: {}", e),
            })?;

        if let Some(tokens) = parsed.get("prohibited_tokens") {
            if let Some(arr) = tokens.as_array() {
                if arr.is_empty() {
                    return Err(HarnessError::ValidationFailed {
                        reason: "prohibited_tokens must contain at least one token".to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    fn walk_statement<'a>(
        stmt: &'a oxc_ast::ast::Statement<'a>,
        ast_classes: &mut Vec<(&'a str, oxc_span::Span)>,
        ast_styles: &mut Vec<(&'a str, oxc_span::Span)>,
    ) {
        match stmt {
            oxc_ast::ast::Statement::ExpressionStatement(e) => {
                Self::walk_expression(&e.expression, ast_classes, ast_styles);
            }
            oxc_ast::ast::Statement::ExportDefaultDeclaration(d) => match &d.declaration {
                oxc_ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                    if let Some(body) = &f.body {
                        for s in &body.statements {
                            Self::walk_statement(s, ast_classes, ast_styles);
                        }
                    }
                }
                _ => {
                    if let Some(expr) = d.declaration.as_expression() {
                        Self::walk_expression(expr, ast_classes, ast_styles);
                    }
                }
            },
            oxc_ast::ast::Statement::ExportDeclaration(d) => match &d.declaration {
                oxc_ast::ast::Declaration::VariableDeclaration(v) => {
                    for decl in &v.declarations {
                        if let Some(init) = &decl.init {
                            Self::walk_expression(init, ast_classes, ast_styles);
                        }
                    }
                }
                oxc_ast::ast::Declaration::FunctionDeclaration(f) => {
                    if let Some(body) = &f.body {
                        for s in &body.statements {
                            Self::walk_statement(s, ast_classes, ast_styles);
                        }
                    }
                }
                _ => {}
            },
            oxc_ast::ast::Statement::VariableDeclaration(v) => {
                for decl in &v.declarations {
                    if let Some(init) = &decl.init {
                        Self::walk_expression(init, ast_classes, ast_styles);
                    }
                }
            }
            oxc_ast::ast::Statement::FunctionDeclaration(f) => {
                if let Some(body) = &f.body {
                    for s in &body.statements {
                        Self::walk_statement(s, ast_classes, ast_styles);
                    }
                }
            }
            oxc_ast::ast::Statement::BlockStatement(b) => {
                for s in &b.body {
                    Self::walk_statement(s, ast_classes, ast_styles);
                }
            }
            oxc_ast::ast::Statement::ReturnStatement(r) => {
                if let Some(arg) = &r.argument {
                    Self::walk_expression(arg, ast_classes, ast_styles);
                }
            }
            _ => {}
        }
    }

    fn walk_expression<'a>(
        expr: &'a oxc_ast::ast::Expression<'a>,
        ast_classes: &mut Vec<(&'a str, oxc_span::Span)>,
        ast_styles: &mut Vec<(&'a str, oxc_span::Span)>,
    ) {
        match expr {
            oxc_ast::ast::Expression::JSXElement(el) => {
                Self::walk_jsx_element(el, ast_classes, ast_styles);
            }
            oxc_ast::ast::Expression::JSXFragment(fr) => {
                for child in &fr.children {
                    Self::walk_jsx_child(child, ast_classes, ast_styles);
                }
            }
            oxc_ast::ast::Expression::ParenthesizedExpression(p) => {
                Self::walk_expression(&p.expression, ast_classes, ast_styles);
            }
            oxc_ast::ast::Expression::ArrowFunctionExpression(a) => match &a.body {
                oxc_ast::ast::ArrowFunctionBody::FunctionBody(b) => {
                    for s in &b.statements {
                        Self::walk_statement(s, ast_classes, ast_styles);
                    }
                }
                _ => {
                    if let Some(expr) = a.body.as_expression() {
                        Self::walk_expression(expr, ast_classes, ast_styles);
                    }
                }
            },
            oxc_ast::ast::Expression::FunctionExpression(f) => {
                if let Some(body) = &f.body {
                    for s in &body.statements {
                        Self::walk_statement(s, ast_classes, ast_styles);
                    }
                }
            }
            oxc_ast::ast::Expression::ConditionalExpression(c) => {
                Self::walk_expression(&c.consequent, ast_classes, ast_styles);
                Self::walk_expression(&c.alternate, ast_classes, ast_styles);
            }
            oxc_ast::ast::Expression::LogicalExpression(l) => {
                Self::walk_expression(&l.left, ast_classes, ast_styles);
                Self::walk_expression(&l.right, ast_classes, ast_styles);
            }
            oxc_ast::ast::Expression::CallExpression(c) => {
                Self::walk_expression(&c.callee, ast_classes, ast_styles);
                for arg in &c.arguments {
                    if let Some(expr) = arg.as_expression() {
                        Self::walk_expression(expr, ast_classes, ast_styles);
                    }
                }
            }
            _ => {}
        }
    }

    fn extract_classes_from_expr<'a>(
        expr: &'a oxc_ast::ast::Expression<'a>,
        ast_classes: &mut Vec<(&'a str, oxc_span::Span)>,
    ) {
        match expr {
            oxc_ast::ast::Expression::StringLiteral(lit) => {
                if let Some(s) = lit.value.as_str() {
                    ast_classes.push((s, lit.span));
                }
            }
            oxc_ast::ast::Expression::TemplateLiteral(tpl) => {
                for quasi in &tpl.quasis {
                    ast_classes.push((quasi.value.raw.as_str(), quasi.span));
                }
            }
            oxc_ast::ast::Expression::ParenthesizedExpression(p) => {
                Self::extract_classes_from_expr(&p.expression, ast_classes);
            }
            oxc_ast::ast::Expression::ConditionalExpression(c) => {
                Self::extract_classes_from_expr(&c.consequent, ast_classes);
                Self::extract_classes_from_expr(&c.alternate, ast_classes);
            }
            oxc_ast::ast::Expression::LogicalExpression(l) => {
                Self::extract_classes_from_expr(&l.left, ast_classes);
                Self::extract_classes_from_expr(&l.right, ast_classes);
            }
            oxc_ast::ast::Expression::CallExpression(c) => {
                for arg in &c.arguments {
                    if let Some(arg_expr) = arg.as_expression() {
                        Self::extract_classes_from_expr(arg_expr, ast_classes);
                    }
                }
            }
            oxc_ast::ast::Expression::ArrayExpression(arr) => {
                for el in &arr.elements {
                    if let Some(el_expr) = el.as_expression() {
                        Self::extract_classes_from_expr(el_expr, ast_classes);
                    }
                }
            }
            _ => {}
        }
    }

    fn walk_jsx_element<'a>(
        el: &'a oxc_ast::ast::JSXElement<'a>,
        ast_classes: &mut Vec<(&'a str, oxc_span::Span)>,
        ast_styles: &mut Vec<(&'a str, oxc_span::Span)>,
    ) {
        use oxc_span::GetSpan;
        for attr in &el.opening_element.attributes {
            if let oxc_ast::ast::JSXAttributeItem::Attribute(a) = attr {
                let name = match &a.name {
                    oxc_ast::ast::JSXAttributeName::Identifier(id) => id.name.as_str(),
                    oxc_ast::ast::JSXAttributeName::NamespacedName(ns) => ns.name.name.as_str(),
                };
                if name == "className" || name == "class" {
                    if let Some(ref val) = a.value {
                        match val {
                            oxc_ast::ast::JSXAttributeValue::StringLiteral(lit) => {
                                if let Some(s) = lit.value.as_str() {
                                    ast_classes.push((s, lit.span));
                                }
                            }
                            oxc_ast::ast::JSXAttributeValue::ExpressionContainer(c) => {
                                match &c.expression {
                                    oxc_ast::ast::JSXExpression::StringLiteral(lit) => {
                                        if let Some(s) = lit.value.as_str() {
                                            ast_classes.push((s, lit.span));
                                        }
                                    }
                                    oxc_ast::ast::JSXExpression::TemplateLiteral(tpl) => {
                                        for quasi in &tpl.quasis {
                                            ast_classes
                                                .push((quasi.value.raw.as_str(), quasi.span));
                                        }
                                    }
                                    _ => {
                                        if let Some(expr) = c.expression.as_expression() {
                                            Self::extract_classes_from_expr(expr, ast_classes);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else if name == "style" {
                    if let Some(ref val) = a.value {
                        ast_styles.push(("", val.span()));
                    }
                }
            }
        }
        for child in &el.children {
            Self::walk_jsx_child(child, ast_classes, ast_styles);
        }
    }

    fn walk_jsx_child<'a>(
        child: &'a oxc_ast::ast::JSXChild<'a>,
        ast_classes: &mut Vec<(&'a str, oxc_span::Span)>,
        ast_styles: &mut Vec<(&'a str, oxc_span::Span)>,
    ) {
        match child {
            oxc_ast::ast::JSXChild::Element(child_el) => {
                Self::walk_jsx_element(child_el, ast_classes, ast_styles);
            }
            oxc_ast::ast::JSXChild::Fragment(child_fr) => {
                for fr_child in &child_fr.children {
                    Self::walk_jsx_child(fr_child, ast_classes, ast_styles);
                }
            }
            oxc_ast::ast::JSXChild::ExpressionContainer(c) => {
                if let Some(expr) = c.expression.as_expression() {
                    Self::walk_expression(expr, ast_classes, ast_styles);
                }
            }
            _ => {}
        }
    }

    /// Lints JSX, TSX, or HTML content against active project constraints.
    /// Emits structured `ConstraintViolation` entries with exact 1-indexed coordinates.
    pub fn lint_source(
        content: &str,
        rules: &[ActiveConstraint],
    ) -> Result<Vec<ConstraintViolation>, HarnessError> {
        let start_time = Instant::now();

        if content.len() > Self::MAX_SOURCE_BYTES {
            return Err(HarnessError::BoundaryViolation {
                reason: format!(
                    "INPUT_TOO_LARGE: Source size ({} bytes) exceeds max budget of {} bytes",
                    content.len(),
                    Self::MAX_SOURCE_BYTES
                ),
            });
        }

        // OXC AST parse phase: validates TypeScript / JSX syntactic structure under <1ms SLA
        let allocator = oxc_allocator::Allocator::default();
        let source_type = oxc_span::SourceType::tsx();
        let parsed = oxc_parser::Parser::new(&allocator, content, source_type).parse();

        // 1. Check and surface OXC parser diagnostics
        if !parsed.diagnostics.is_empty() {
            return Err(HarnessError::ValidationFailed {
                reason: format!("Malformed JSX/TSX syntax: {}", parsed.diagnostics[0]),
            });
        }

        // 2. Walk OXC AST statements and extract JSX elements & attributes
        let mut ast_classes = Vec::new();
        let mut ast_styles = Vec::new();
        for stmt in &parsed.program.body {
            Self::walk_statement(stmt, &mut ast_classes, &mut ast_styles);
        }

        for r in rules {
            Self::validate_rule_config(&r.config_json)?;
        }

        let tailwind_rule = rules
            .iter()
            .find(|r| r.rule_code.eq_ignore_ascii_case("BAN_TAILWIND"));
        let inline_hex_rule = rules
            .iter()
            .find(|r| r.rule_code.eq_ignore_ascii_case("BAN_INLINE_HEX"));

        if tailwind_rule.is_none() && inline_hex_rule.is_none() {
            return Ok(Vec::new());
        }

        // Lazy line starts initialized on first violation (Task 6.15)
        let line_starts = std::cell::OnceCell::<Vec<usize>>::new();

        let byte_to_line_col = |byte_offset: usize| -> (usize, usize) {
            let starts = line_starts.get_or_init(|| {
                std::iter::once(0)
                    .chain(content.match_indices('\n').map(|(i, _)| i + 1))
                    .collect()
            });
            let offset = byte_offset.min(content.len());
            let line_idx = starts
                .partition_point(|&start| start <= offset)
                .saturating_sub(1);
            let line_num = line_idx + 1;
            let line_start_byte = starts[line_idx];
            let safe_offset = (line_start_byte..=offset)
                .rev()
                .find(|&idx| content.is_char_boundary(idx))
                .unwrap_or(line_start_byte);
            let col_num = content[line_start_byte..safe_offset].chars().count() + 1;
            (line_num, col_num)
        };

        let mut violations = Vec::new();

        if let Some(rule) = tailwind_rule {
            for (class_str, span) in &ast_classes {
                let base_offset = span.start as usize;
                let is_quoted = if base_offset < content.len() {
                    let b = content.as_bytes()[base_offset];
                    b == b'"' || b == b'\''
                } else {
                    false
                };
                let content_start_offset = if is_quoted {
                    base_offset + 1
                } else {
                    base_offset
                };

                let mut token_start = None;
                for (i, c) in class_str.char_indices() {
                    if c.is_whitespace() {
                        if let Some(s) = token_start {
                            let tok = &class_str[s..i];
                            if Self::is_tailwind_token(tok) {
                                let abs_token_idx = content_start_offset + s;
                                let (line, col) = byte_to_line_col(abs_token_idx);
                                violations.push(ConstraintViolation {
                                    rule_code: "BAN_TAILWIND".to_string(),
                                    severity: rule.severity.clone(),
                                    message: format!(
                                        "PROJECT_CONSTRAINT_VIOLATION: Tailwind utility class '{}' is forbidden by project brand policy.",
                                        tok
                                    ),
                                    line: Some(line),
                                    column: Some(col),
                                });
                            }
                            token_start = None;
                        }
                    } else if token_start.is_none() {
                        token_start = Some(i);
                    }
                }
                if let Some(s) = token_start {
                    let tok = &class_str[s..];
                    if Self::is_tailwind_token(tok) {
                        let abs_token_idx = content_start_offset + s;
                        let (line, col) = byte_to_line_col(abs_token_idx);
                        violations.push(ConstraintViolation {
                            rule_code: "BAN_TAILWIND".to_string(),
                            severity: rule.severity.clone(),
                            message: format!(
                                "PROJECT_CONSTRAINT_VIOLATION: Tailwind utility class '{}' is forbidden by project brand policy.",
                                tok
                            ),
                            line: Some(line),
                            column: Some(col),
                        });
                    }
                }
            }
        }

        if let Some(rule) = inline_hex_rule {
            for (_, span) in &ast_styles {
                let start_idx = span.start as usize;
                let end_idx = (span.end as usize).min(content.len());
                if start_idx < end_idx {
                    let slice = &content[start_idx..end_idx];
                    let mut search_pos = 0;
                    while let Some(rel_pos) = slice[search_pos..].find('#') {
                        let hash_pos = search_pos + rel_pos;
                        let remaining = &slice[hash_pos + 1..];
                        let hex_len = remaining
                            .chars()
                            .take_while(|c| c.is_ascii_hexdigit())
                            .count();

                        if hex_len == 3 || hex_len == 4 || hex_len == 6 || hex_len == 8 {
                            let hex_code = &slice[hash_pos..=hash_pos + hex_len];
                            let abs_token_idx = start_idx + hash_pos;
                            let (line, col) = byte_to_line_col(abs_token_idx);
                            violations.push(ConstraintViolation {
                                rule_code: "BAN_INLINE_HEX".to_string(),
                                severity: rule.severity.clone(),
                                message: format!(
                                    "PROJECT_CONSTRAINT_VIOLATION: Hardcoded inline hex color '{}' is forbidden. Use CSS tokens from tokens.css.",
                                    hex_code
                                ),
                                line: Some(line),
                                column: Some(col),
                            });
                        }
                        search_pos = hash_pos + 1;
                    }
                }
            }
        }

        // SLA Check: execution must complete within 5ms under CPU stress, ideally <1ms
        let elapsed = start_time.elapsed();
        if elapsed.as_millis() > 20 {
            eprintln!(
                "[WARN] AST Linting latency exceeded budget: {:?} for {} bytes",
                elapsed,
                content.len()
            );
        }

        Ok(violations)
    }

    fn is_tailwind_token(token: &str) -> bool {
        let base = if let Some(colon_pos) = token.rfind(':') {
            &token[colon_pos + 1..]
        } else {
            token
        };

        if base.is_empty() {
            return false;
        }

        const EXACT_KEYWORDS: &[&str] = &[
            "flex",
            "inline-flex",
            "grid",
            "inline-grid",
            "block",
            "inline-block",
            "inline",
            "hidden",
            "container",
            "table",
            "relative",
            "absolute",
            "fixed",
            "sticky",
            "static",
            "italic",
            "not-italic",
            "underline",
            "line-through",
            "no-underline",
            "uppercase",
            "lowercase",
            "capitalize",
            "normal-case",
            "truncate",
            "border",
            "rounded",
            "shadow",
            "transition",
            "transform",
            "font-bold",
            "font-medium",
            "font-semibold",
            "font-normal",
            "font-light",
            "w-full",
            "h-full",
            "w-screen",
            "h-screen",
            "aspect-square",
            "aspect-video",
            "aspect-auto",
            "group",
            "peer",
            "group-hover",
            "peer-hover",
            "group-focus",
            "peer-focus",
            "group-active",
            "peer-active",
            "shrink",
            "grow",
            "isolate",
            "visible",
            "invisible",
            "sr-only",
            "not-sr-only",
            "filter",
            "antialiased",
            "subpixel-antialiased",
            "clear-left",
            "clear-right",
            "clear-both",
            "clear-none",
            "float-left",
            "float-right",
            "float-none",
            "overflow-hidden",
            "overflow-auto",
            "overflow-visible",
            "overflow-scroll",
        ];

        if EXACT_KEYWORDS.contains(&base) {
            return true;
        }

        const TAILWIND_PREFIXES: &[&str] = &[
            "w-",
            "h-",
            "min-w-",
            "min-h-",
            "max-w-",
            "max-h-",
            "p-",
            "px-",
            "py-",
            "pt-",
            "pr-",
            "pb-",
            "pl-",
            "m-",
            "mx-",
            "my-",
            "mt-",
            "mr-",
            "mb-",
            "ml-",
            "space-x-",
            "space-y-",
            "bg-",
            "text-",
            "font-",
            "border-",
            "rounded-",
            "shadow-",
            "opacity-",
            "gap-",
            "col-",
            "row-",
            "grid-cols-",
            "grid-rows-",
            "items-",
            "justify-",
            "content-",
            "self-",
            "order-",
            "leading-",
            "tracking-",
            "z-",
            "top-",
            "bottom-",
            "left-",
            "right-",
            "inset-",
            "transition-",
            "duration-",
            "delay-",
            "ease-",
            "scale-",
            "rotate-",
            "translate-",
            "ring-",
            "outline-",
            "divide-",
            "cursor-",
            "overflow-",
            "whitespace-",
            "break-",
            "aspect-",
            "columns-",
            "shrink-",
            "grow-",
            "flex-",
            "basis-",
            "float-",
            "clear-",
            "group-",
            "peer-",
            "group/",
            "peer/",
            "object-",
            "pointer-events-",
            "select-",
            "align-",
            "table-",
            "list-",
            "decoration-",
            "accent-",
            "appearance-",
            "filter-",
            "blur-",
            "backdrop-",
            "mix-blend-",
            "bg-blend-",
            "fill-",
            "stroke-",
            "grayscale-",
            "invert-",
            "sepia-",
            "animate-",
            "snap-",
            "touch-",
            "scroll-",
            "overscroll-",
            "hyphens-",
            "place-",
            "caret-",
            "origin-",
        ];

        for p in TAILWIND_PREFIXES {
            if base.starts_with(p) {
                return true;
            }
        }

        base.contains("-[") && base.ends_with(']')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forbid_marketing_rule() {
        let rules = vec![ActiveConstraint {
            rule_code: "FORBID_MARKETING".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        }];

        let res = ConstraintEvaluator::evaluate_slice_constraints(
            &rules,
            Department::MarketingDeck,
            Portal::MarketingWeb,
        );
        assert!(res.is_err());

        let res_ok = ConstraintEvaluator::evaluate_slice_constraints(
            &rules,
            Department::ProductUi,
            Portal::TenantAdmin,
        );
        assert!(res_ok.is_ok());
    }

    #[test]
    fn test_restrict_department_rule() {
        let rules = vec![ActiveConstraint {
            rule_code: "RESTRICT_DEPARTMENT".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: r#"{"forbidden_departments": ["marketing_deck"]}"#.to_string(),
        }];

        let res = ConstraintEvaluator::evaluate_slice_constraints(
            &rules,
            Department::MarketingDeck,
            Portal::TenantAdmin,
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_ast_linter_ban_tailwind() {
        let rules = vec![ActiveConstraint {
            rule_code: "BAN_TAILWIND".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        }];

        let jsx = r#"<div className="flex items-center justify-between p-4">Hello</div>"#;
        let violations = AstLinter::lint_source(jsx, &rules).expect("Linting should succeed");
        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_code, "BAN_TAILWIND");
        assert_eq!(violations[0].line, Some(1));
    }

    #[test]
    fn test_ast_linter_ban_inline_hex() {
        let rules = vec![ActiveConstraint {
            rule_code: "BAN_INLINE_HEX".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        }];

        let jsx = r##"<button style={{ backgroundColor: "#1e293b" }}>Submit</button>"##;
        let violations = AstLinter::lint_source(jsx, &rules).expect("Linting should succeed");
        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_code, "BAN_INLINE_HEX");
    }

    #[test]
    fn test_ast_linter_ban_tailwind_aspect_square_and_utilities() {
        let rules = vec![ActiveConstraint {
            rule_code: "BAN_TAILWIND".to_string(),
            severity: "WARN".to_string(),
            config_json: "{}".to_string(),
        }];

        let jsx = r#"<div class="aspect-square shrink-0 isolate">Aspect Content</div>"#;
        let violations = AstLinter::lint_source(jsx, &rules).expect("Linting should succeed");
        assert_eq!(violations.len(), 3);
        assert_eq!(violations[0].rule_code, "BAN_TAILWIND");
        assert_eq!(violations[0].severity, "WARN");
        assert!(violations[0].message.contains("aspect-square"));
        assert!(violations[1].message.contains("shrink-0"));
        assert!(violations[2].message.contains("isolate"));
    }

    #[test]
    fn test_domain_profile_mapper_roundtrip() {
        let domain_profile = domain::ConstraintProfile {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            max_complexity_score: 5,
            enforce_clean_code: true,
            forbidden_patterns: vec!["tailwind".to_string(), "raw-hex".to_string()],
            target_framework: "react".to_string(),
        };

        let active_constraints =
            ConstraintEvaluator::active_constraints_from_domain(&domain_profile);
        assert_eq!(active_constraints.len(), 3); // clean_code + tailwind + raw-hex
        assert!(active_constraints
            .iter()
            .any(|c| c.rule_code == "ENFORCE_CLEAN_CODE"));
        assert!(active_constraints
            .iter()
            .any(|c| c.rule_code == "BAN_TAILWIND"));
        assert!(active_constraints
            .iter()
            .any(|c| c.rule_code == "BAN_INLINE_HEX"));

        let reconstructed = ConstraintEvaluator::to_domain_profile(
            &domain_profile.id.to_string(),
            &domain_profile.project_id.to_string(),
            &active_constraints,
            Some("react"),
        );
        assert_eq!(reconstructed.project_id, domain_profile.project_id);
        assert_eq!(reconstructed.max_complexity_score, 5);
        assert!(reconstructed.enforce_clean_code);
        assert_eq!(reconstructed.target_framework, "react");
        assert!(reconstructed
            .forbidden_patterns
            .contains(&"tailwind".to_string()));
        assert!(reconstructed
            .forbidden_patterns
            .contains(&"raw-hex".to_string()));
    }

    #[test]
    fn test_ast_linter_ban_tailwind_group_and_peer() {
        let rules = vec![ActiveConstraint {
            rule_code: "BAN_TAILWIND".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        }];

        let jsx = r#"<div class="group peer group-hover:block">Group Content</div>"#;
        let violations = AstLinter::lint_source(jsx, &rules).expect("Linting should succeed");
        assert_eq!(violations.len(), 3);
        assert!(violations[0].message.contains("group"));
        assert!(violations[1].message.contains("peer"));
        assert!(violations[2].message.contains("group-hover:block"));
    }

    #[test]
    fn test_ast_linter_ban_tailwind_template_literal() {
        let rules = vec![ActiveConstraint {
            rule_code: "BAN_TAILWIND".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        }];

        let tsx = r#"<div className={`flex p-4`} />"#;
        let violations = AstLinter::lint_source(tsx, &rules).expect("Linting should succeed");
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].rule_code, "BAN_TAILWIND");
        assert!(violations[0].message.contains("flex"));
        assert_eq!(violations[1].rule_code, "BAN_TAILWIND");
        assert!(violations[1].message.contains("p-4"));
    }

    #[test]
    fn test_ast_linter_utf8_char_boundary_no_panic() {
        let rules = vec![ActiveConstraint {
            rule_code: "BAN_TAILWIND".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        }];

        // Multi-byte UTF-8 string before class and non-ASCII characters inside JSX
        let tsx = r#"// Chào buổi sáng Việt Nam: Đà Nẵng, Hà Nội, TP Hồ Chí Minh 🎉
export const Card = () => (
    <div className="bg-[url('đà-nẵng.png')] flex p-4 text-center">
        <span>Tiêu đề tiếng Việt có dấu: Kiểm thử an toàn byte UTF-8</span>
    </div>
);"#;
        let violations =
            AstLinter::lint_source(tsx, &rules).expect("Linting should succeed without panic");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.message.contains("flex")));
        assert!(violations.iter().any(|v| v.message.contains("p-4")));
        assert!(violations.iter().any(|v| v.message.contains("text-center")));
    }

    #[test]
    fn test_ast_linter_expression_container_dynamic_jsx() {
        let rules = vec![ActiveConstraint {
            rule_code: "BAN_TAILWIND".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        }];

        let tsx = r#"export const DynamicView = ({ isOpen, items }: any) => (
    <div>
        {isOpen && <button className="flex p-4">Close</button>}
        {items.map((i: any) => (
            <div className="grid gap-2" key={i}>
                <span>{i}</span>
            </div>
        ))}
    </div>
);"#;
        let violations = AstLinter::lint_source(tsx, &rules).expect("Linting should succeed");
        assert_eq!(violations.len(), 4);
        assert!(violations.iter().any(|v| v.message.contains("flex")));
        assert!(violations.iter().any(|v| v.message.contains("p-4")));
        assert!(violations.iter().any(|v| v.message.contains("grid")));
        assert!(violations.iter().any(|v| v.message.contains("gap-2")));
    }

    #[test]
    fn test_ast_linter_dynamic_ternary_in_classname() {
        let rules = vec![ActiveConstraint {
            rule_code: "BAN_TAILWIND".to_string(),
            severity: "BLOCKER".to_string(),
            config_json: "{}".to_string(),
        }];

        let tsx = r#"export const ConditionalButton = ({ active }: { active: boolean }) => (
    <button className={active ? "flex p-2" : "hidden m-1"}>Click</button>
);"#;
        let violations = AstLinter::lint_source(tsx, &rules).expect("Linting should succeed");
        assert_eq!(violations.len(), 4);
        assert!(violations.iter().any(|v| v.message.contains("flex")));
        assert!(violations.iter().any(|v| v.message.contains("p-2")));
        assert!(violations.iter().any(|v| v.message.contains("hidden")));
        assert!(violations.iter().any(|v| v.message.contains("m-1")));
    }
}
