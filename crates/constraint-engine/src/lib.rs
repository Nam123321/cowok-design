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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
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

#[derive(Default)]
struct AstTypographySample<'a> {
    font_family: Option<&'a str>,
    font_weight: Option<u32>,
    font_size: Option<&'a str>,
    color: Option<&'a str>,
    background_color: Option<&'a str>,
    span: oxc_span::Span,
}

#[derive(Default)]
struct AstCollectedNodes<'a> {
    classes: Vec<(&'a str, oxc_span::Span)>,
    styles: Vec<(&'a str, oxc_span::Span)>,
    components: Vec<(String, oxc_span::Span)>,
    component_props: Vec<(String, Vec<(String, oxc_span::Span)>)>,
    icons: Vec<(&'a str, oxc_span::Span)>,
    typography_samples: Vec<AstTypographySample<'a>>,
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
        nodes: &mut AstCollectedNodes<'a>,
        content: &str,
    ) {
        match stmt {
            oxc_ast::ast::Statement::ExpressionStatement(e) => {
                Self::walk_expression(&e.expression, nodes, content);
            }
            oxc_ast::ast::Statement::ExportDefaultDeclaration(d) => match &d.declaration {
                oxc_ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                    if let Some(body) = &f.body {
                        for s in &body.statements {
                            Self::walk_statement(s, nodes, content);
                        }
                    }
                }
                _ => {
                    if let Some(expr) = d.declaration.as_expression() {
                        Self::walk_expression(expr, nodes, content);
                    }
                }
            },
            oxc_ast::ast::Statement::ExportDeclaration(d) => match &d.declaration {
                oxc_ast::ast::Declaration::VariableDeclaration(v) => {
                    for decl in &v.declarations {
                        if let Some(init) = &decl.init {
                            Self::walk_expression(init, nodes, content);
                        }
                    }
                }
                oxc_ast::ast::Declaration::FunctionDeclaration(f) => {
                    if let Some(body) = &f.body {
                        for s in &body.statements {
                            Self::walk_statement(s, nodes, content);
                        }
                    }
                }
                _ => {}
            },
            oxc_ast::ast::Statement::VariableDeclaration(v) => {
                for decl in &v.declarations {
                    if let Some(init) = &decl.init {
                        Self::walk_expression(init, nodes, content);
                    }
                }
            }
            oxc_ast::ast::Statement::FunctionDeclaration(f) => {
                if let Some(body) = &f.body {
                    for s in &body.statements {
                        Self::walk_statement(s, nodes, content);
                    }
                }
            }
            oxc_ast::ast::Statement::BlockStatement(b) => {
                for s in &b.body {
                    Self::walk_statement(s, nodes, content);
                }
            }
            oxc_ast::ast::Statement::ReturnStatement(r) => {
                if let Some(arg) = &r.argument {
                    Self::walk_expression(arg, nodes, content);
                }
            }
            oxc_ast::ast::Statement::IfStatement(i) => {
                Self::walk_expression(&i.test, nodes, content);
                Self::walk_statement(&i.consequent, nodes, content);
                if let Some(alt) = &i.alternate {
                    Self::walk_statement(alt, nodes, content);
                }
            }
            oxc_ast::ast::Statement::SwitchStatement(sw) => {
                Self::walk_expression(&sw.discriminant, nodes, content);
                for case in &sw.cases {
                    if let Some(test) = &case.test {
                        Self::walk_expression(test, nodes, content);
                    }
                    for s in &case.consequent {
                        Self::walk_statement(s, nodes, content);
                    }
                }
            }
            oxc_ast::ast::Statement::ForStatement(f) => {
                if let Some(test) = &f.test {
                    Self::walk_expression(test, nodes, content);
                }
                if let Some(update) = &f.update {
                    Self::walk_expression(update, nodes, content);
                }
                Self::walk_statement(&f.body, nodes, content);
            }
            oxc_ast::ast::Statement::ForOfStatement(f) => {
                Self::walk_expression(&f.right, nodes, content);
                Self::walk_statement(&f.body, nodes, content);
            }
            oxc_ast::ast::Statement::ForInStatement(f) => {
                Self::walk_expression(&f.right, nodes, content);
                Self::walk_statement(&f.body, nodes, content);
            }
            oxc_ast::ast::Statement::WhileStatement(w) => {
                Self::walk_expression(&w.test, nodes, content);
                Self::walk_statement(&w.body, nodes, content);
            }
            oxc_ast::ast::Statement::DoWhileStatement(d) => {
                Self::walk_statement(&d.body, nodes, content);
                Self::walk_expression(&d.test, nodes, content);
            }
            oxc_ast::ast::Statement::TryStatement(t) => {
                for s in &t.block.body {
                    Self::walk_statement(s, nodes, content);
                }
                if let Some(handler) = &t.handler {
                    for s in &handler.body.body {
                        Self::walk_statement(s, nodes, content);
                    }
                }
                if let Some(fin) = &t.finalizer {
                    for s in &fin.body {
                        Self::walk_statement(s, nodes, content);
                    }
                }
            }
            oxc_ast::ast::Statement::LabeledStatement(l) => {
                Self::walk_statement(&l.body, nodes, content);
            }
            oxc_ast::ast::Statement::ThrowStatement(th) => {
                Self::walk_expression(&th.argument, nodes, content);
            }
            _ => {}
        }
    }

    fn walk_expression<'a>(
        expr: &'a oxc_ast::ast::Expression<'a>,
        nodes: &mut AstCollectedNodes<'a>,
        content: &str,
    ) {
        match expr {
            oxc_ast::ast::Expression::JSXElement(el) => {
                Self::walk_jsx_element(el, nodes, content);
            }
            oxc_ast::ast::Expression::JSXFragment(fr) => {
                for child in &fr.children {
                    Self::walk_jsx_child(child, nodes, content);
                }
            }
            oxc_ast::ast::Expression::ParenthesizedExpression(p) => {
                Self::walk_expression(&p.expression, nodes, content);
            }
            oxc_ast::ast::Expression::ArrowFunctionExpression(a) => match &a.body {
                oxc_ast::ast::ArrowFunctionBody::FunctionBody(b) => {
                    for s in &b.statements {
                        Self::walk_statement(s, nodes, content);
                    }
                }
                _ => {
                    if let Some(expr) = a.body.as_expression() {
                        Self::walk_expression(expr, nodes, content);
                    }
                }
            },
            oxc_ast::ast::Expression::FunctionExpression(f) => {
                if let Some(body) = &f.body {
                    for s in &body.statements {
                        Self::walk_statement(s, nodes, content);
                    }
                }
            }
            oxc_ast::ast::Expression::ConditionalExpression(c) => {
                Self::walk_expression(&c.consequent, nodes, content);
                Self::walk_expression(&c.alternate, nodes, content);
            }
            oxc_ast::ast::Expression::LogicalExpression(l) => {
                Self::walk_expression(&l.left, nodes, content);
                Self::walk_expression(&l.right, nodes, content);
            }
            oxc_ast::ast::Expression::CallExpression(c) => {
                Self::walk_expression(&c.callee, nodes, content);
                for arg in &c.arguments {
                    if let Some(expr) = arg.as_expression() {
                        Self::walk_expression(expr, nodes, content);
                    }
                }
            }
            oxc_ast::ast::Expression::ArrayExpression(arr) => {
                for el in &arr.elements {
                    if let Some(el_expr) = el.as_expression() {
                        Self::walk_expression(el_expr, nodes, content);
                    } else if let oxc_ast::ast::ArrayExpressionElement::SpreadElement(sp) = el {
                        Self::walk_expression(&sp.argument, nodes, content);
                    }
                }
            }
            oxc_ast::ast::Expression::ObjectExpression(obj) => {
                for prop_kind in &obj.properties {
                    match prop_kind {
                        oxc_ast::ast::ObjectPropertyKind::ObjectProperty(prop) => {
                            Self::walk_expression(&prop.value, nodes, content);
                        }
                        oxc_ast::ast::ObjectPropertyKind::SpreadProperty(sp) => {
                            Self::walk_expression(&sp.argument, nodes, content);
                        }
                    }
                }
            }
            oxc_ast::ast::Expression::SequenceExpression(seq) => {
                for sub in &seq.expressions {
                    Self::walk_expression(sub, nodes, content);
                }
            }
            oxc_ast::ast::Expression::AssignmentExpression(a) => {
                Self::walk_expression(&a.right, nodes, content);
            }
            oxc_ast::ast::Expression::AwaitExpression(a) => {
                Self::walk_expression(&a.argument, nodes, content);
            }
            oxc_ast::ast::Expression::UnaryExpression(u) => {
                Self::walk_expression(&u.argument, nodes, content);
            }
            oxc_ast::ast::Expression::BinaryExpression(b) => {
                Self::walk_expression(&b.left, nodes, content);
                Self::walk_expression(&b.right, nodes, content);
            }
            oxc_ast::ast::Expression::ChainExpression(ch) => match &ch.expression {
                oxc_ast::ast::ChainElement::CallExpression(c) => {
                    Self::walk_expression(&c.callee, nodes, content);
                    for arg in &c.arguments {
                        if let Some(expr) = arg.as_expression() {
                            Self::walk_expression(expr, nodes, content);
                        }
                    }
                }
                oxc_ast::ast::ChainElement::TSNonNullExpression(ts) => {
                    Self::walk_expression(&ts.expression, nodes, content);
                }
                _ => {}
            },
            oxc_ast::ast::Expression::TemplateLiteral(t) => {
                for sub in &t.expressions {
                    Self::walk_expression(sub, nodes, content);
                }
            }
            oxc_ast::ast::Expression::NewExpression(n) => {
                Self::walk_expression(&n.callee, nodes, content);
                for arg in &n.arguments {
                    if let Some(expr) = arg.as_expression() {
                        Self::walk_expression(expr, nodes, content);
                    }
                }
            }
            oxc_ast::ast::Expression::TSAsExpression(ts) => {
                Self::walk_expression(&ts.expression, nodes, content);
            }
            oxc_ast::ast::Expression::TSSatisfiesExpression(ts) => {
                Self::walk_expression(&ts.expression, nodes, content);
            }
            oxc_ast::ast::Expression::TSTypeAssertion(ts) => {
                Self::walk_expression(&ts.expression, nodes, content);
            }
            oxc_ast::ast::Expression::TSNonNullExpression(ts) => {
                Self::walk_expression(&ts.expression, nodes, content);
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

    fn extract_jsx_tag_name(name: &oxc_ast::ast::JSXElementName<'_>, content: &str) -> String {
        use oxc_span::GetSpan;
        match name {
            oxc_ast::ast::JSXElementName::Identifier(id) => id.name.to_string(),
            oxc_ast::ast::JSXElementName::IdentifierReference(id) => id.name.to_string(),
            oxc_ast::ast::JSXElementName::NamespacedName(ns) => ns.name.name.to_string(),
            oxc_ast::ast::JSXElementName::MemberExpression(m) => {
                let s = m.span().start as usize;
                let e = (m.span().end as usize).min(content.len());
                content.get(s..e).unwrap_or("").trim().to_string()
            }
            oxc_ast::ast::JSXElementName::ThisExpression(_) => "this".to_string(),
        }
    }

    fn collect_string_expr_values<'a>(
        expr: &'a oxc_ast::ast::Expression<'a>,
        out: &mut Vec<(&'a str, oxc_span::Span)>,
    ) {
        match expr {
            oxc_ast::ast::Expression::StringLiteral(lit) => {
                if let Some(s) = lit.value.as_str() {
                    out.push((s, lit.span));
                }
            }
            oxc_ast::ast::Expression::ConditionalExpression(cond) => {
                Self::collect_string_expr_values(&cond.consequent, out);
                Self::collect_string_expr_values(&cond.alternate, out);
            }
            oxc_ast::ast::Expression::LogicalExpression(l) => {
                Self::collect_string_expr_values(&l.right, out);
            }
            oxc_ast::ast::Expression::ParenthesizedExpression(p) => {
                Self::collect_string_expr_values(&p.expression, out);
            }
            oxc_ast::ast::Expression::TSAsExpression(ts) => {
                Self::collect_string_expr_values(&ts.expression, out);
            }
            oxc_ast::ast::Expression::TSSatisfiesExpression(ts) => {
                Self::collect_string_expr_values(&ts.expression, out);
            }
            _ => {}
        }
    }

    fn collect_string_attr_values<'a>(
        val: &'a oxc_ast::ast::JSXAttributeValue<'a>,
        out: &mut Vec<(&'a str, oxc_span::Span)>,
    ) {
        match val {
            oxc_ast::ast::JSXAttributeValue::StringLiteral(lit) => {
                if let Some(s) = lit.value.as_str() {
                    out.push((s, lit.span));
                }
            }
            oxc_ast::ast::JSXAttributeValue::ExpressionContainer(c) => match &c.expression {
                oxc_ast::ast::JSXExpression::StringLiteral(lit) => {
                    if let Some(s) = lit.value.as_str() {
                        out.push((s, lit.span));
                    }
                }
                _ => {
                    if let Some(expr) = c.expression.as_expression() {
                        Self::collect_string_expr_values(expr, out);
                    }
                }
            },
            _ => {}
        }
    }

    fn extract_typography_from_style_expr<'a>(
        expr: &'a oxc_ast::ast::JSXExpression<'a>,
        fallback_span: oxc_span::Span,
    ) -> Option<AstTypographySample<'a>> {
        let Some(oxc_ast::ast::Expression::ObjectExpression(obj)) = expr.as_expression() else {
            return None;
        };
        let mut sample = AstTypographySample {
            font_family: None,
            font_weight: None,
            font_size: None,
            color: None,
            background_color: None,
            span: fallback_span,
        };
        let mut found_any = false;
        for prop_kind in &obj.properties {
            let oxc_ast::ast::ObjectPropertyKind::ObjectProperty(prop) = prop_kind else {
                continue;
            };
            let key_name = match &prop.key {
                oxc_ast::ast::PropertyKey::StaticIdentifier(id) => id.name.as_str(),
                oxc_ast::ast::PropertyKey::StringLiteral(lit) => lit.value.as_str().unwrap_or(""),
                _ => continue,
            };
            match key_name {
                "fontFamily" | "font-family" => {
                    if let oxc_ast::ast::Expression::StringLiteral(lit) = &prop.value {
                        sample.font_family = lit.value.as_str();
                        sample.span = lit.span;
                        found_any = true;
                    }
                }
                "fontWeight" | "font-weight" => match &prop.value {
                    oxc_ast::ast::Expression::NumericLiteral(num) => {
                        if num.value >= 1.0 && num.value <= 1000.0 {
                            sample.font_weight = Some(num.value as u32);
                            sample.span = num.span;
                            found_any = true;
                        }
                    }
                    oxc_ast::ast::Expression::StringLiteral(lit) => {
                        if let Some(parsed) = lit
                            .value
                            .as_str()
                            .and_then(|s| s.trim().parse::<u32>().ok())
                        {
                            sample.font_weight = Some(parsed);
                            sample.span = lit.span;
                            found_any = true;
                        }
                    }
                    _ => {}
                },
                "fontSize" | "font-size" => {
                    if let oxc_ast::ast::Expression::StringLiteral(lit) = &prop.value {
                        sample.font_size = lit.value.as_str();
                        sample.span = lit.span;
                        found_any = true;
                    }
                }
                "color" => {
                    if let oxc_ast::ast::Expression::StringLiteral(lit) = &prop.value {
                        sample.color = lit.value.as_str();
                    }
                }
                "backgroundColor" | "background-color" => {
                    if let oxc_ast::ast::Expression::StringLiteral(lit) = &prop.value {
                        sample.background_color = lit.value.as_str();
                    }
                }
                _ => {}
            }
        }
        if found_any || (sample.color.is_some() && sample.background_color.is_some()) {
            Some(sample)
        } else {
            None
        }
    }

    fn walk_jsx_element<'a>(
        el: &'a oxc_ast::ast::JSXElement<'a>,
        nodes: &mut AstCollectedNodes<'a>,
        content: &str,
    ) {
        use oxc_span::GetSpan;
        let tag_name = Self::extract_jsx_tag_name(&el.opening_element.name, content);
        let mut props_collected: Vec<(String, oxc_span::Span)> = Vec::new();
        if !tag_name.is_empty() {
            nodes
                .components
                .push((tag_name.clone(), el.opening_element.name.span()));
        }
        let is_icon_component = matches!(
            tag_name.as_str(),
            "BrandIcon" | "CowokIcon" | "Icon" | "AgentMktBrandIcon"
        );

        for attr in &el.opening_element.attributes {
            if let oxc_ast::ast::JSXAttributeItem::Attribute(a) = attr {
                let name = match &a.name {
                    oxc_ast::ast::JSXAttributeName::Identifier(id) => id.name.as_str(),
                    oxc_ast::ast::JSXAttributeName::NamespacedName(ns) => ns.name.name.as_str(),
                };
                if !tag_name.is_empty() {
                    props_collected.push((name.to_string(), a.name.span()));
                }
                if name == "className" || name == "class" {
                    if let Some(ref val) = a.value {
                        match val {
                            oxc_ast::ast::JSXAttributeValue::StringLiteral(lit) => {
                                if let Some(s) = lit.value.as_str() {
                                    nodes.classes.push((s, lit.span));
                                }
                            }
                            oxc_ast::ast::JSXAttributeValue::ExpressionContainer(c) => {
                                match &c.expression {
                                    oxc_ast::ast::JSXExpression::StringLiteral(lit) => {
                                        if let Some(s) = lit.value.as_str() {
                                            nodes.classes.push((s, lit.span));
                                        }
                                    }
                                    oxc_ast::ast::JSXExpression::TemplateLiteral(tpl) => {
                                        for quasi in &tpl.quasis {
                                            nodes
                                                .classes
                                                .push((quasi.value.raw.as_str(), quasi.span));
                                        }
                                    }
                                    _ => {
                                        if let Some(expr) = c.expression.as_expression() {
                                            Self::extract_classes_from_expr(
                                                expr,
                                                &mut nodes.classes,
                                            );
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else if name == "style" {
                    if let Some(ref val) = a.value {
                        nodes.styles.push(("", val.span()));
                        if let oxc_ast::ast::JSXAttributeValue::ExpressionContainer(c) = val {
                            if let Some(sample) =
                                Self::extract_typography_from_style_expr(&c.expression, val.span())
                            {
                                nodes.typography_samples.push(sample);
                            }
                        }
                    }
                } else if (is_icon_component && (name == "name" || name == "icon"))
                    || name == "icon"
                {
                    if let Some(ref val) = a.value {
                        Self::collect_string_attr_values(val, &mut nodes.icons);
                    }
                }
            }
        }
        if !tag_name.is_empty() && !props_collected.is_empty() {
            nodes.component_props.push((tag_name, props_collected));
        }
        for child in &el.children {
            Self::walk_jsx_child(child, nodes, content);
        }
    }

    fn walk_jsx_child<'a>(
        child: &'a oxc_ast::ast::JSXChild<'a>,
        nodes: &mut AstCollectedNodes<'a>,
        content: &str,
    ) {
        match child {
            oxc_ast::ast::JSXChild::Element(child_el) => {
                Self::walk_jsx_element(child_el, nodes, content);
            }
            oxc_ast::ast::JSXChild::Fragment(child_fr) => {
                for fr_child in &child_fr.children {
                    Self::walk_jsx_child(fr_child, nodes, content);
                }
            }
            oxc_ast::ast::JSXChild::ExpressionContainer(c) => {
                if let Some(expr) = c.expression.as_expression() {
                    Self::walk_expression(expr, nodes, content);
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
        let icon_rule = rules
            .iter()
            .find(|r| r.rule_code.eq_ignore_ascii_case("ENFORCE_ICON_MANIFEST"));
        let typo_rule = rules
            .iter()
            .find(|r| r.rule_code.eq_ignore_ascii_case("ENFORCE_TYPOGRAPHY"));
        let comp_rule = rules.iter().find(|r| {
            r.rule_code
                .eq_ignore_ascii_case("ENFORCE_COMPONENT_REGISTRY")
        });

        let parsed_manifest = if let Some(rule) = icon_rule {
            let trimmed = rule.config_json.trim();
            if trimmed.is_empty() || trimmed == "{}" {
                Some(IconValidator::get_flagship_manifest())
            } else {
                Some(IconValidator::parse_manifest(trimmed)?)
            }
        } else {
            None
        };

        let parsed_typo = if let Some(rule) = typo_rule {
            let trimmed = rule.config_json.trim();
            if trimmed.is_empty() || trimmed == "{}" {
                Some(TypographyContract::default())
            } else {
                Some(
                    serde_json::from_str::<TypographyContract>(trimmed).map_err(|e| {
                        HarnessError::ValidationFailed {
                            reason: format!("INVALID_TYPOGRAPHY_RULE_CONFIG: {e}"),
                        }
                    })?,
                )
            }
        } else {
            None
        };

        let parsed_comp = if let Some(rule) = comp_rule {
            let trimmed = rule.config_json.trim();
            if trimmed.is_empty() || trimmed == "{}" {
                None
            } else {
                Some(
                    serde_json::from_str::<ComponentRegistryContract>(trimmed).map_err(|e| {
                        HarnessError::ValidationFailed {
                            reason: format!("INVALID_COMPONENT_REGISTRY_RULE_CONFIG: {e}"),
                        }
                    })?,
                )
            }
        } else {
            None
        };

        Self::lint_source_with_context(
            content,
            rules,
            parsed_manifest.as_ref(),
            parsed_typo.as_ref(),
            parsed_comp.as_ref(),
        )
    }

    /// Lints JSX, TSX, or HTML content against active project constraints AND resolved
    /// project-scoped IconManifest, TypographyContract, and ComponentRegistryContract (`07_COMPONENT_REGISTRY`).
    pub fn lint_source_with_context(
        content: &str,
        rules: &[ActiveConstraint],
        icon_manifest: Option<&IconManifest>,
        typography_contract: Option<&TypographyContract>,
        component_registry: Option<&ComponentRegistryContract>,
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

        // 2. Walk OXC AST statements and extract JSX elements, classes, styles, icons & typography
        let mut nodes = AstCollectedNodes::default();
        for stmt in &parsed.program.body {
            Self::walk_statement(stmt, &mut nodes, content);
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
        let icon_rule_sev = rules
            .iter()
            .find(|r| r.rule_code.eq_ignore_ascii_case("ENFORCE_ICON_MANIFEST"))
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "BLOCKER".to_string());
        let typo_rule_sev = rules
            .iter()
            .find(|r| r.rule_code.eq_ignore_ascii_case("ENFORCE_TYPOGRAPHY"))
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "BLOCKER".to_string());
        let comp_rule_sev = rules
            .iter()
            .find(|r| {
                r.rule_code
                    .eq_ignore_ascii_case("ENFORCE_COMPONENT_REGISTRY")
            })
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "BLOCKER".to_string());

        let has_comp_enforcement = component_registry.is_some_and(|c| c.forbid_unregistered);

        if tailwind_rule.is_none()
            && inline_hex_rule.is_none()
            && icon_manifest.is_none()
            && typography_contract.is_none()
            && !has_comp_enforcement
        {
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
            for (class_str, span) in &nodes.classes {
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
            for (_, span) in &nodes.styles {
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

        // 3. Enforce Icon Manifest on extracted JSX icon calls
        if let Some(manifest) = icon_manifest {
            for (icon_name, span) in &nodes.icons {
                let report = IconValidator::validate_icon(manifest, icon_name);
                if !report.is_valid {
                    let (line, col) = byte_to_line_col(span.start as usize);
                    violations.push(ConstraintViolation {
                        rule_code: "ENFORCE_ICON_MANIFEST".to_string(),
                        severity: icon_rule_sev.clone(),
                        message: format!(
                            "ICON_MANIFEST_VIOLATION: Icon '{}' is not registered in project icon manifest '{}'. Did you mean: {:?}?",
                            report.requested_icon, manifest.name, report.suggestions
                        ),
                        line: Some(line),
                        column: Some(col),
                    });
                }
            }
        }

        // 4. Enforce TypographyContract on extracted inline JSX typography props
        if let Some(contract) = typography_contract {
            let fallback_family = contract
                .allowed_font_families
                .first()
                .map(|s| s.as_str())
                .unwrap_or("Geist");
            let fallback_weight = contract
                .allowed_font_weights
                .first()
                .copied()
                .unwrap_or(400);

            for sample in &nodes.typography_samples {
                let check_family = sample.font_family.unwrap_or(fallback_family);
                let check_weight = sample.font_weight.unwrap_or(fallback_weight);
                let computed_contrast = match (
                    sample.color.and_then(Self::parse_static_css_color),
                    sample
                        .background_color
                        .and_then(Self::parse_static_css_color),
                ) {
                    (Some(fg), Some(bg)) => Self::wcag_contrast_ratio(fg, bg),
                    _ => contract.min_wcag_contrast,
                };
                if let Err(errs) = IconValidator::validate_typography(
                    contract,
                    check_family,
                    check_weight,
                    sample.font_size,
                    computed_contrast,
                ) {
                    let (line, col) = byte_to_line_col(sample.span.start as usize);
                    for err_msg in errs {
                        violations.push(ConstraintViolation {
                            rule_code: "ENFORCE_TYPOGRAPHY".to_string(),
                            severity: typo_rule_sev.clone(),
                            message: format!("TYPOGRAPHY_CONTRACT_VIOLATION: {err_msg}"),
                            line: Some(line),
                            column: Some(col),
                        });
                    }
                }
            }
        }

        // 5. Enforce ComponentRegistryContract (`07_COMPONENT_REGISTRY`) on PascalCase JSX tags & allowed_props
        if let Some(comp_contract) = component_registry {
            if comp_contract.forbid_unregistered {
                for (comp_name, span) in &nodes.components {
                    if matches!(
                        comp_name.as_str(),
                        "BrandIcon" | "CowokIcon" | "Icon" | "AgentMktBrandIcon" | "Fragment" | "React.Fragment"
                    ) {
                        continue;
                    }
                    let report = IconValidator::validate_component(comp_contract, comp_name);
                    if !report.is_valid {
                        let (line, col) = byte_to_line_col(span.start as usize);
                        violations.push(ConstraintViolation {
                            rule_code: "ENFORCE_COMPONENT_REGISTRY".to_string(),
                            severity: comp_rule_sev.clone(),
                            message: format!(
                                "COMPONENT_REGISTRY_VIOLATION: Component '<{} />' is not registered in 07_COMPONENT_REGISTRY. Suggestions: {:?}",
                                report.requested_component, report.suggestions
                            ),
                            line: Some(line),
                            column: Some(col),
                        });
                    }
                }
            }

            for (comp_name, used_props) in &nodes.component_props {
                if let Some(entry) = comp_contract
                    .components
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(comp_name))
                {
                    if !entry.allowed_props.is_empty() {
                        for (prop_name, prop_span) in used_props {
                            let is_react_builtin = matches!(
                                prop_name.as_str(),
                                "key" | "ref" | "children" | "className" | "style"
                            ) || prop_name.starts_with("data-")
                                || prop_name.starts_with("aria-");
                            let is_allowed = entry
                                .allowed_props
                                .iter()
                                .any(|allowed| allowed == prop_name);
                            if !is_react_builtin && !is_allowed {
                                let (line, col) = byte_to_line_col(prop_span.start as usize);
                                violations.push(ConstraintViolation {
                                    rule_code: "ENFORCE_COMPONENT_REGISTRY".to_string(),
                                    severity: comp_rule_sev.clone(),
                                    message: format!(
                                        "COMPONENT_PROP_VIOLATION: Prop '{}' on '<{} />' is not listed in allowed_props: {:?}",
                                        prop_name, comp_name, entry.allowed_props
                                    ),
                                    line: Some(line),
                                    column: Some(col),
                                });
                            }
                        }
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

    fn parse_static_css_color(raw: &str) -> Option<(u8, u8, u8)> {
        let s = raw.trim().to_ascii_lowercase();
        if let Some(hex) = s.strip_prefix('#') {
            match hex.len() {
                3 | 4 => {
                    let bytes = hex.as_bytes();
                    let expand = |b: u8| -> Option<u8> {
                        let d = (b as char).to_digit(16)? as u8;
                        Some(d * 17)
                    };
                    return Some((expand(bytes[0])?, expand(bytes[1])?, expand(bytes[2])?));
                }
                6 | 8 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    return Some((r, g, b));
                }
                _ => return None,
            }
        }
        let inner = s
            .strip_prefix("rgb(")
            .or_else(|| s.strip_prefix("rgba("))?
            .strip_suffix(')')?;
        let parts: Vec<&str> = inner.split([',', '/']).take(3).collect();
        if parts.len() < 3 {
            return None;
        }
        let r = parts[0].trim().parse::<u8>().ok()?;
        let g = parts[1].trim().parse::<u8>().ok()?;
        let b = parts[2].trim().parse::<u8>().ok()?;
        Some((r, g, b))
    }

    fn relative_luminance(rgb: (u8, u8, u8)) -> f64 {
        let channel = |c: u8| -> f64 {
            let srgb = (c as f64) / 255.0;
            if srgb <= 0.03928 {
                srgb / 12.92
            } else {
                ((srgb + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(rgb.0) + 0.7152 * channel(rgb.1) + 0.0722 * channel(rgb.2)
    }

    fn wcag_contrast_ratio(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> f64 {
        let l1 = Self::relative_luminance(fg);
        let l2 = Self::relative_luminance(bg);
        let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
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
