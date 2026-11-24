// @story 05.6
// @implements AC-03
use std::collections::{BTreeMap, BTreeSet};

use markup5ever_rcdom::{Handle, NodeData};
use serde::{Deserialize, Serialize};

use crate::patcher::parse_html5_dom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopularComponentCategory {
    Feedback,
    Overlay,
    Navigation,
    Input,
    Display,
    Deck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopularComponentStatus {
    DetectedInSketch,
    SuggestedContextual,
    Optional,
    NotNeeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogComponentEntry {
    pub component_kind: String,
    pub category: PopularComponentCategory,
    pub default_reason: String,
    #[serde(default)]
    pub contextual_triggers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopularComponentAuditItem {
    pub component_kind: String,
    pub category: PopularComponentCategory,
    pub status: PopularComponentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub selected: bool,
    pub occurrence_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SketchComponentScanResult {
    pub extracted_kinds: Vec<String>,
    pub audit_items: Vec<PopularComponentAuditItem>,
    pub detected_count: usize,
    pub suggested_count: usize,
    pub not_needed_count: usize,
}

fn entry(
    kind: &str,
    category: PopularComponentCategory,
    reason: &str,
    triggers: &[&str],
) -> CatalogComponentEntry {
    CatalogComponentEntry {
        component_kind: kind.to_string(),
        category,
        default_reason: reason.to_string(),
        contextual_triggers: triggers.iter().map(|s| (*s).to_string()).collect(),
    }
}

/// Canonical `popular-components.fe.json` catalog baseline.
pub fn default_popular_fe_catalog() -> Vec<CatalogComponentEntry> {
    vec![
        entry(
            "button",
            PopularComponentCategory::Input,
            "Primary & secondary interactive actions",
            &[],
        ),
        entry(
            "card",
            PopularComponentCategory::Display,
            "Grouped surface container for metrics or records",
            &[],
        ),
        entry(
            "data-table",
            PopularComponentCategory::Display,
            "Structured tabular dataset view",
            &[],
        ),
        entry(
            "form-input",
            PopularComponentCategory::Input,
            "Editable form fields and controls",
            &[],
        ),
        entry(
            "navigation-bar",
            PopularComponentCategory::Navigation,
            "Primary navigation landmark",
            &[],
        ),
        entry(
            "toast",
            PopularComponentCategory::Feedback,
            "Transient feedback after save, submit, or mutation actions",
            &["button", "form-input"],
        ),
        entry(
            "confirm-dialog",
            PopularComponentCategory::Overlay,
            "Confirmation guard for destructive or high-impact actions",
            &["button", "data-table"],
        ),
        entry(
            "tooltip",
            PopularComponentCategory::Overlay,
            "Inline micro-explanation for compact icons and dense metrics",
            &["kpi-metric", "navigation-bar", "button"],
        ),
        entry(
            "dropdown",
            PopularComponentCategory::Navigation,
            "Compact action menu for overflow options and row actions",
            &["data-table", "navigation-bar"],
        ),
        entry(
            "empty-error",
            PopularComponentCategory::Feedback,
            "Resilient Empty and Error fallback states for dynamic views",
            &["data-table", "card", "form-input"],
        ),
        entry(
            "drawer",
            PopularComponentCategory::Overlay,
            "Side inspector drawer for detail editing without leaving context",
            &["data-table"],
        ),
        entry(
            "kpi-metric",
            PopularComponentCategory::Display,
            "Compact KPI metric readout",
            &[],
        ),
    ]
}

/// Canonical `popular-components.deck.json` catalog baseline.
pub fn default_popular_deck_catalog() -> Vec<CatalogComponentEntry> {
    vec![
        entry(
            "slide-hero",
            PopularComponentCategory::Deck,
            "Opening title slide hero structure",
            &[],
        ),
        entry(
            "kpi-metric",
            PopularComponentCategory::Deck,
            "Executive KPI strip on 16:9 canvas",
            &["slide-hero"],
        ),
        entry(
            "chart-card",
            PopularComponentCategory::Deck,
            "Data visualization container",
            &["data-table", "kpi-metric"],
        ),
        entry(
            "callout-banner",
            PopularComponentCategory::Deck,
            "Key takeaway emphasis block",
            &["slide-hero"],
        ),
    ]
}

fn normalize_kind(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() || trimmed.len() > 64 {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn infer_kinds_from_element(
    tag: &str,
    attrs_map: &BTreeMap<String, String>,
    counts: &mut BTreeMap<String, usize>,
) {
    if let Some(explicit) = attrs_map
        .get("data-component-kind")
        .and_then(|v| normalize_kind(v))
    {
        *counts.entry(explicit).or_insert(0) += 1;
    }
    let mapped_tag = match tag {
        "button" => Some("button"),
        "table" => Some("data-table"),
        "form" | "input" | "select" | "textarea" => Some("form-input"),
        "dialog" => Some("confirm-dialog"),
        "nav" => Some("navigation-bar"),
        "aside" => Some("drawer"),
        "details" => Some("dropdown"),
        _ => None,
    };
    if let Some(kind) = mapped_tag {
        *counts.entry(kind.to_string()).or_insert(0) += 1;
    }
    if let Some(role) = attrs_map.get("role") {
        let mapped_role = match role.to_ascii_lowercase().as_str() {
            "alert" | "status" => Some("toast"),
            "dialog" | "alertdialog" => Some("confirm-dialog"),
            "tooltip" => Some("tooltip"),
            "menu" | "listbox" => Some("dropdown"),
            "navigation" => Some("navigation-bar"),
            "tablist" => Some("tabs"),
            _ => None,
        };
        if let Some(kind) = mapped_role {
            *counts.entry(kind.to_string()).or_insert(0) += 1;
        }
    }
    if let Some(cls) = attrs_map.get("class") {
        let lower = cls.to_ascii_lowercase();
        if lower.contains("card") {
            *counts.entry("card".to_string()).or_insert(0) += 1;
        }
        if lower.contains("kpi") {
            *counts.entry("kpi-metric".to_string()).or_insert(0) += 1;
        }
    }
}

fn walk_dom_for_components(handle: &Handle, counts: &mut BTreeMap<String, usize>) {
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        let tag: String = AsRef::<str>::as_ref(&name.local).to_ascii_lowercase();
        let mut attrs_map = BTreeMap::new();
        for attr in attrs.borrow().iter() {
            let k: String = AsRef::<str>::as_ref(&attr.name.local).to_ascii_lowercase();
            attrs_map.insert(k, attr.value.to_string());
        }
        infer_kinds_from_element(&tag, &attrs_map, counts);
    }
    for child in handle.children.borrow().iter() {
        walk_dom_for_components(child, counts);
    }
}

/// Scans Sketch Phase 1 HTML documents using `html5ever` AST and classifies all catalog items
/// into `detected_in_sketch`, `suggested_contextual`, and `not_needed`.
pub fn scan_sketch_components(
    sketch_htmls: &[&str],
    catalog_items: &[CatalogComponentEntry],
) -> SketchComponentScanResult {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for html in sketch_htmls {
        let dom = parse_html5_dom(html);
        walk_dom_for_components(&dom.document, &mut counts);
    }
    let detected_set: BTreeSet<String> = counts.keys().cloned().collect();
    let mut audit_items = Vec::new();
    let mut covered_kinds = BTreeSet::new();

    for item in catalog_items {
        let count = counts.get(&item.component_kind).copied().unwrap_or(0);
        covered_kinds.insert(item.component_kind.clone());
        let (status, selected) = if count > 0 {
            (PopularComponentStatus::DetectedInSketch, true)
        } else if item
            .contextual_triggers
            .iter()
            .any(|tr| detected_set.contains(tr))
        {
            (PopularComponentStatus::SuggestedContextual, false)
        } else {
            (PopularComponentStatus::NotNeeded, false)
        };
        audit_items.push(PopularComponentAuditItem {
            component_kind: item.component_kind.clone(),
            category: item.category,
            status,
            reason: Some(item.default_reason.clone()),
            selected,
            occurrence_count: count,
        });
    }

    for (kind, count) in &counts {
        if !covered_kinds.contains(kind) {
            audit_items.push(PopularComponentAuditItem {
                component_kind: kind.clone(),
                category: PopularComponentCategory::Display,
                status: PopularComponentStatus::DetectedInSketch,
                reason: Some("Custom component extracted from Sketch DOM".to_string()),
                selected: true,
                occurrence_count: *count,
            });
        }
    }

    let detected_count = audit_items
        .iter()
        .filter(|i| i.status == PopularComponentStatus::DetectedInSketch)
        .count();
    let suggested_count = audit_items
        .iter()
        .filter(|i| i.status == PopularComponentStatus::SuggestedContextual)
        .count();
    let not_needed_count = audit_items
        .iter()
        .filter(|i| i.status == PopularComponentStatus::NotNeeded)
        .count();

    SketchComponentScanResult {
        extracted_kinds: detected_set.into_iter().collect(),
        audit_items,
        detected_count,
        suggested_count,
        not_needed_count,
    }
}
