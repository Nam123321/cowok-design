// @story 05.6
// @implements AC-04
use std::collections::{BTreeMap, BTreeSet};

use markup5ever_rcdom::{Handle, NodeData};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::patcher::{is_valid_ff_id, parse_html5_dom};

pub const FE_CONTRACT_BOUNDARY_NOTICE: &str = "FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferredFieldType {
    String,
    Number,
    Currency,
    Boolean,
    Date,
    Enum,
    Array,
    ChartSeries,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeViewModelField {
    pub ff_id: String,
    pub field_name: String,
    pub inferred_type: InferredFieldType,
    pub sample_value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeActionInput {
    pub action_id: String,
    pub trigger_ff_id: String,
    pub required_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeDataContractDraft {
    pub project_id: String,
    pub screen_id: String,
    pub boundary_notice: String,
    pub view_model_fields: Vec<FeViewModelField>,
    pub action_inputs: Vec<FeActionInput>,
    pub sub_state_fixtures: BTreeMap<String, Value>,
}

fn to_camel_case_ident(raw: &str) -> String {
    let trimmed = raw.strip_prefix("ff-").unwrap_or(raw);
    let mut out = String::new();
    let mut upper_next = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            if out.is_empty() {
                if ch.is_ascii_digit() {
                    out.push('f');
                }
                out.push(ch.to_ascii_lowercase());
            } else if upper_next {
                out.push(ch.to_ascii_uppercase());
            } else {
                out.push(ch);
            }
            upper_next = false;
        } else {
            upper_next = true;
        }
    }
    if out.is_empty() {
        "field".to_string()
    } else {
        out
    }
}

fn collect_text(handle: &Handle, buf: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => buf.push_str(&contents.borrow()),
        NodeData::Element { .. } | NodeData::Document => {
            for c in handle.children.borrow().iter() {
                collect_text(c, buf);
            }
        }
        _ => {}
    }
}

fn infer_field_type_and_sample(
    tag: &str,
    attrs: &BTreeMap<String, String>,
    text: &str,
) -> (InferredFieldType, Value) {
    let trimmed = text.trim();
    if tag == "svg" || attrs.contains_key("data-chart-kind") {
        return (
            InferredFieldType::ChartSeries,
            json!([{ "label": "Q1", "value": 42 }]),
        );
    }
    if matches!(tag, "table" | "ul" | "ol") {
        return (InferredFieldType::Array, json!([trimmed]));
    }
    if tag == "select" {
        return (InferredFieldType::Enum, json!(trimmed));
    }
    if attrs.get("type").is_some_and(|t| t == "date")
        || (trimmed.len() == 10
            && trimmed.chars().nth(4) == Some('-')
            && trimmed.chars().nth(7) == Some('-'))
    {
        return (InferredFieldType::Date, json!(trimmed));
    }
    if attrs.get("type").is_some_and(|t| t == "checkbox")
        || matches!(
            trimmed.to_ascii_lowercase().as_str(),
            "true" | "false" | "active" | "inactive"
        )
    {
        let b = !matches!(trimmed.to_ascii_lowercase().as_str(), "false" | "inactive");
        return (InferredFieldType::Boolean, json!(b));
    }
    if trimmed.starts_with('$')
        || trimmed.starts_with('€')
        || trimmed.contains("USD")
        || trimmed.contains("VND")
    {
        return (InferredFieldType::Currency, json!(trimmed));
    }
    let numeric_candidate = trimmed
        .trim_end_matches('%')
        .trim_end_matches("ms")
        .replace(',', "");
    if !numeric_candidate.is_empty() && numeric_candidate.parse::<f64>().is_ok() {
        let num = numeric_candidate.parse::<f64>().unwrap_or(0.0);
        return (InferredFieldType::Number, json!(num));
    }
    let sample = if !trimmed.is_empty() {
        trimmed.to_string()
    } else if let Some(ph) = attrs.get("placeholder") {
        ph.clone()
    } else {
        String::new()
    };
    (InferredFieldType::String, json!(sample))
}

struct ExtractionCollector {
    seen_fields: BTreeSet<String>,
    seen_actions: BTreeSet<String>,
    fields: Vec<FeViewModelField>,
    actions: Vec<FeActionInput>,
}

fn collect_attrs_map(
    attrs: &std::cell::RefCell<Vec<html5ever::Attribute>>,
) -> BTreeMap<String, String> {
    let mut attrs_map = BTreeMap::new();
    for attr in attrs.borrow().iter() {
        let k: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
        attrs_map.insert(k, attr.value.to_string());
    }
    attrs_map
}

fn collect_form_input_names(handle: &Handle, seen: &mut BTreeSet<String>, out: &mut Vec<String>) {
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        let tag: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
        if matches!(tag.as_str(), "input" | "select" | "textarea") {
            let attrs_map = collect_attrs_map(attrs);
            let candidate = attrs_map
                .get("name")
                .or_else(|| attrs_map.get("data-ff-id").filter(|id| is_valid_ff_id(id)))
                .cloned()
                .unwrap_or_else(|| format!("input_{}", out.len() + 1));
            let ident = to_camel_case_ident(&candidate);
            if seen.insert(ident.clone()) {
                out.push(ident);
            }
        }
    }
    for child in handle.children.borrow().iter() {
        collect_form_input_names(child, seen, out);
    }
}

fn collect_from_node(
    handle: &Handle,
    enclosing_form_inputs: Option<&[String]>,
    col: &mut ExtractionCollector,
) {
    let mut owned_form_inputs: Option<Vec<String>> = None;
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        let tag: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
        let attrs_map = collect_attrs_map(attrs);
        if tag == "form" {
            let mut seen = BTreeSet::new();
            let mut inputs = Vec::new();
            collect_form_input_names(handle, &mut seen, &mut inputs);
            owned_form_inputs = Some(inputs);
        }
        let scoped_inputs: Vec<String> = owned_form_inputs
            .as_deref()
            .or(enclosing_form_inputs)
            .map(|s| s.to_vec())
            .unwrap_or_default();

        if let Some(ff_id) = attrs_map
            .get("data-ff-id")
            .filter(|id| is_valid_ff_id(id))
            .cloned()
        {
            if matches!(tag.as_str(), "button" | "form") {
                let action_id = to_camel_case_ident(&ff_id);
                if col.seen_actions.insert(action_id.clone()) {
                    col.actions.push(FeActionInput {
                        action_id,
                        trigger_ff_id: ff_id,
                        required_fields: scoped_inputs,
                    });
                }
            } else if !matches!(
                tag.as_str(),
                "main" | "section" | "div" | "header" | "footer"
            ) {
                let mut text = String::new();
                collect_text(handle, &mut text);
                let field_name = to_camel_case_ident(&ff_id);
                if col.seen_fields.insert(field_name.clone()) {
                    let (inferred_type, sample_value) =
                        infer_field_type_and_sample(&tag, &attrs_map, &text);
                    col.fields.push(FeViewModelField {
                        ff_id,
                        field_name,
                        inferred_type,
                        sample_value,
                    });
                }
            }
        }
    }
    let next_form_inputs = owned_form_inputs.as_deref().or(enclosing_form_inputs);
    for child in handle.children.borrow().iter() {
        collect_from_node(child, next_form_inputs, col);
    }
}

fn build_sub_state_fixtures(fields: &[FeViewModelField]) -> BTreeMap<String, Value> {
    let mut default_obj = serde_json::Map::new();
    let mut empty_obj = serde_json::Map::new();
    for f in fields {
        default_obj.insert(f.field_name.clone(), f.sample_value.clone());
        let empty_val = match f.inferred_type {
            InferredFieldType::Array | InferredFieldType::ChartSeries => json!([]),
            InferredFieldType::Number => json!(0),
            InferredFieldType::Boolean => json!(false),
            _ => json!(""),
        };
        empty_obj.insert(f.field_name.clone(), empty_val);
    }
    let mut fixtures = BTreeMap::new();
    fixtures.insert("DEFAULT".to_string(), Value::Object(default_obj));
    fixtures.insert(
        "EMPTY".to_string(),
        json!({ "isEmpty": true, "fields": Value::Object(empty_obj) }),
    );
    fixtures.insert(
        "LOADING".to_string(),
        json!({ "isLoading": true, "skeletonCount": fields.len().max(1) }),
    );
    fixtures.insert(
        "ERROR".to_string(),
        json!({ "errorCode": "VIEW_DATA_UNAVAILABLE", "retryable": true }),
    );
    fixtures
}

/// Deterministically extracts FE-Required ViewModel fields, Action Inputs, and 4-State Fixtures
/// from a Sketch/Polished HTML document. Explicitly scoped to FE contract only (never BE DB DDL).
pub fn extract_fe_data_contract(
    project_id: &str,
    screen_id: &str,
    html_doc: &str,
) -> FeDataContractDraft {
    let dom = parse_html5_dom(html_doc);
    let mut col = ExtractionCollector {
        seen_fields: BTreeSet::new(),
        seen_actions: BTreeSet::new(),
        fields: Vec::new(),
        actions: Vec::new(),
    };
    collect_from_node(&dom.document, None, &mut col);
    let sub_state_fixtures = build_sub_state_fixtures(&col.fields);
    FeDataContractDraft {
        project_id: project_id.to_string(),
        screen_id: screen_id.to_string(),
        boundary_notice: FE_CONTRACT_BOUNDARY_NOTICE.to_string(),
        view_model_fields: col.fields,
        action_inputs: col.actions,
        sub_state_fixtures,
    }
}

fn sanitize_ts_comment(raw: &str) -> String {
    raw.replace("*/", "* /")
        .replace(['\r', '\n'], " ")
        .replace('"', "'")
}

fn sanitize_ts_string_literal(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ")
}

fn ts_type_for_field(kind: InferredFieldType) -> &'static str {
    match kind {
        InferredFieldType::String
        | InferredFieldType::Currency
        | InferredFieldType::Date
        | InferredFieldType::Enum => "string",
        InferredFieldType::Number => "number",
        InferredFieldType::Boolean => "boolean",
        InferredFieldType::Array => "unknown[]",
        InferredFieldType::ChartSeries => "Array<{ label: string; value: number }>",
    }
}

/// Renders clean, copy-ready `fe-data-contract.draft.ts` source from a `FeDataContractDraft`.
pub fn render_fe_data_contract_ts(draft: &FeDataContractDraft) -> String {
    let safe_boundary_comment = sanitize_ts_comment(&draft.boundary_notice);
    let safe_boundary_lit = sanitize_ts_string_literal(&draft.boundary_notice);
    let mut out = String::new();
    out.push_str("// Auto-extracted by cowok-design html-forge (Story 05.6)\n");
    out.push_str(&format!("// BOUNDARY: {safe_boundary_comment}\n"));
    out.push_str(
        "// Frontend ViewModel & Action Inputs ONLY — does NOT prescribe backend DB schema.\n\n",
    );
    out.push_str(&format!(
        "export const FE_CONTRACT_BOUNDARY = \"{safe_boundary_lit}\" as const;\n\n"
    ));
    out.push_str("export interface ScreenViewModel {\n");
    let mut rendered_fields = BTreeSet::new();
    for f in &draft.view_model_fields {
        let safe_field = to_camel_case_ident(&f.field_name);
        if !rendered_fields.insert(safe_field.clone()) {
            continue;
        }
        let safe_ff_id = sanitize_ts_comment(&f.ff_id);
        out.push_str(&format!(
            "  /** data-ff-id=\"{}\" */\n  {}: {};\n",
            safe_ff_id,
            safe_field,
            ts_type_for_field(f.inferred_type)
        ));
    }
    out.push_str("}\n\nexport interface ScreenActionInputs {\n");
    let mut rendered_actions = BTreeSet::new();
    for a in &draft.action_inputs {
        let safe_action = to_camel_case_ident(&a.action_id);
        if !rendered_actions.insert(safe_action.clone()) {
            continue;
        }
        let mut seen_req = BTreeSet::new();
        let dedup_req: Vec<String> = a
            .required_fields
            .iter()
            .map(|rf| to_camel_case_ident(rf))
            .filter(|rf| seen_req.insert(rf.clone()))
            .collect();
        let fields_sig = if dedup_req.is_empty() {
            "Record<string, never>".to_string()
        } else {
            let inner: Vec<String> = dedup_req.iter().map(|rf| format!("{rf}: string")).collect();
            format!("{{ {} }}", inner.join("; "))
        };
        let safe_trigger = sanitize_ts_comment(&a.trigger_ff_id);
        out.push_str(&format!(
            "  /** trigger=\"{}\" */\n  {}: {};\n",
            safe_trigger, safe_action, fields_sig
        ));
    }
    out.push_str("}\n\n");
    let fixtures_json = serde_json::to_string_pretty(&draft.sub_state_fixtures)
        .unwrap_or_else(|_| "{}".to_string());
    out.push_str(&format!(
        "export const SUB_STATE_FIXTURES = {fixtures_json} as const;\n"
    ));
    out
}
