// @story 03.3
// @cover AC-01, AC-02, AC-03
use constraint_engine::{PatchKind, SurgicalPatchRequest, SurgicalPatcher};

#[test]
fn test_deterministic_data_ff_id_injection() {
    let source = r#"
        export default function Dashboard() {
            return (
                <div className="container">
                    <h1 data-ff-id="existing-id">Dashboard Title</h1>
                    <p className="description">Welcome to the analytics portal</p>
                    <button onClick={() => alert("clicked")}>Submit</button>
                </div>
            );
        }
    "#;

    let res = SurgicalPatcher::inject_ff_ids(source, Some("Dashboard"))
        .expect("injection should succeed");

    // 1. Should not tag h1 since it already has data-ff-id="existing-id"
    assert!(res.generated_ids.contains(&"existing-id".to_string()));
    assert!(res.tagged_source.contains(r#"data-ff-id="existing-id""#));

    // 2. Should tag div, p, button
    assert!(res.total_elements_tagged >= 3);
    assert!(res
        .tagged_source
        .contains(r#"<div data-ff-id="ff-dashboard-div-1""#));
    assert!(res
        .tagged_source
        .contains(r#"<p data-ff-id="ff-dashboard-p-1""#));
    assert!(res
        .tagged_source
        .contains(r#"<button data-ff-id="ff-dashboard-button-1""#));

    // 3. Determinism check: running again on identical source must produce identical results
    let res2 = SurgicalPatcher::inject_ff_ids(source, Some("Dashboard")).expect("second run");
    assert_eq!(res.generated_ids, res2.generated_ids);
    assert_eq!(res.tagged_source, res2.tagged_source);
}

#[test]
fn test_sub_millisecond_surgical_patch_text() {
    let source = r#"
        <div data-ff-id="ff-card-1">
            <span data-ff-id="ff-title-1">Old Metric Title</span>
        </div>
    "#;

    let req = SurgicalPatchRequest {
        ff_id: "ff-title-1".to_string(),
        patch_kind: PatchKind::TextContent,
        new_value: "Live Revenue Metrics".to_string(),
        source: source.to_string(),
        target_attribute: None,
    };

    // Warmup parser
    let _ = SurgicalPatcher::patch_by_ff_id(source, &req);

    let resp = SurgicalPatcher::patch_by_ff_id(source, &req).expect("patch should succeed");

    assert!(resp.success);
    assert_eq!(resp.ff_id, "ff-title-1");
    assert_eq!(resp.original_text.as_deref(), Some("Old Metric Title"));
    assert!(resp
        .patched_source
        .contains("<span data-ff-id=\"ff-title-1\">Live Revenue Metrics</span>"));
    assert!(!resp.patched_source.contains("Old Metric Title"));

    // Sub-millisecond SLA assertion: execution time under 1000 microseconds (1ms) in release
    println!("Surgical patch latency: {} µs", resp.execution_time_micros);
    let threshold_micros = if cfg!(debug_assertions) { 10000 } else { 1000 };
    assert!(
        resp.execution_time_micros < threshold_micros,
        "Must be under sub-millisecond range (or debug threshold)"
    );
}

#[test]
fn test_sub_millisecond_surgical_patch_metric() {
    let source = r#"
        <div data-ff-id="ff-kpi-box">
            <h2 data-ff-id="ff-kpi-value">$12,450.00</h2>
        </div>
    "#;

    let req = SurgicalPatchRequest {
        ff_id: "ff-kpi-value".to_string(),
        patch_kind: PatchKind::MetricValue,
        new_value: "$98,750.50".to_string(),
        source: source.to_string(),
        target_attribute: None,
    };

    let resp = SurgicalPatcher::patch_by_ff_id(source, &req).expect("patch should succeed");

    assert!(resp.success);
    assert_eq!(resp.original_text.as_deref(), Some("$12,450.00"));
    assert!(resp
        .patched_source
        .contains("<h2 data-ff-id=\"ff-kpi-value\">$98,750.50</h2>"));
}

#[test]
fn test_ast_and_formatting_preservation_with_special_chars_and_multiline_ac03() {
    // AC-03 / EC-E03-03: Preserves comments, whitespace, indentation, multiline content, and special chars
    let source = r#"
        // Header comment before component
        export default function ComplexView() {
            /* Block comment explaining layout */
            return (
                <main className="p-8">
                    {/* JSX comment that must be 100% preserved */}
                    <article data-ff-id="ff-article-target">
                        Original Text &amp; Entities &lt; &gt; " '
                        Line 2 with special symbols: 🚀 100%
                    </article>
                    <footer className="mt-4">
                        Footer Copyright 2026
                    </footer>
                </main>
            );
        }
    "#;

    let new_multiline_text = "Updated Multi-Line Content:\n                        - Point A &amp; B &lt;Alpha&gt;\n                        - Point C &gt; 'Beta' ✨";

    let req = SurgicalPatchRequest {
        ff_id: "ff-article-target".to_string(),
        patch_kind: PatchKind::TextContent,
        new_value: new_multiline_text.to_string(),
        source: source.to_string(),
        target_attribute: None,
    };

    let resp = SurgicalPatcher::patch_by_ff_id(source, &req).expect("patch should succeed");

    assert!(resp.success);
    // 1. Surrounding comments and whitespace must remain completely intact
    assert!(resp
        .patched_source
        .contains("// Header comment before component"));
    assert!(resp
        .patched_source
        .contains("/* Block comment explaining layout */"));
    assert!(resp
        .patched_source
        .contains("{/* JSX comment that must be 100% preserved */}"));
    assert!(resp.patched_source.contains("<footer className=\"mt-4\">"));
    assert!(resp.patched_source.contains("Footer Copyright 2026"));

    // 2. Patched content is cleanly substituted
    assert!(resp.patched_source.contains(new_multiline_text));
}

#[test]
fn test_patch_nonexistent_ff_id_fails_gracefully() {
    let source = "<div data-ff-id=\"ff-real-1\">Real Content</div>";

    let req = SurgicalPatchRequest {
        ff_id: "ff-ghost-404".to_string(),
        patch_kind: PatchKind::TextContent,
        new_value: "Ghost Content".to_string(),
        source: source.to_string(),
        target_attribute: None,
    };

    let resp = SurgicalPatcher::patch_by_ff_id(source, &req).expect("should handle gracefully");

    assert!(!resp.success);
    assert_eq!(resp.original_text, None);
    assert_eq!(resp.patched_source, source);
}

#[test]
fn test_named_export_and_target_attribute_patching() {
    let source = r#"
        export const ProfileCard = () => (
            <div data-ff-id="ff-card-1">
                <img data-ff-id="ff-avatar-1" src="old-avatar.png" alt="User" />
                <span data-ff-id="ff-name-1">Alice</span>
            </div>
        );
    "#;

    let req = SurgicalPatchRequest {
        ff_id: "ff-avatar-1".to_string(),
        patch_kind: PatchKind::AttributeValue,
        new_value: "new-avatar.png".to_string(),
        source: source.to_string(),
        target_attribute: Some("src".to_string()),
    };

    let resp = SurgicalPatcher::patch_by_ff_id(source, &req).expect("patch should succeed");
    assert!(resp.success);
    assert_eq!(resp.original_text.as_deref(), Some("\"old-avatar.png\""));
    assert!(resp.patched_source.contains("src=\"new-avatar.png\""));
}

#[test]
fn test_component_name_sanitization_prevents_xss() {
    let source = "<div><span /></div>";
    let malicious_name = "test\" onclick=\"alert(1)";
    let res = SurgicalPatcher::inject_ff_ids(source, Some(malicious_name)).expect("inject");
    // Ensure attribute breakout is prevented: no double quote or injected attribute syntax
    assert!(!res.tagged_source.contains("\" onclick="));
    assert!(!res.tagged_source.contains("alert(1)"));
    assert!(res
        .tagged_source
        .contains("data-ff-id=\"ff-testonclickalert1-div-1\""));
}

#[test]
fn test_dynamic_ternary_expression_is_protected() {
    let source = r#"
        <div data-ff-id="ff-metric-1">
            {isLoading ? <Spinner /> : "$120.00"}
        </div>
    "#;

    let req = SurgicalPatchRequest {
        ff_id: "ff-metric-1".to_string(),
        patch_kind: PatchKind::TextContent,
        new_value: "$500.00".to_string(),
        source: source.to_string(),
        target_attribute: None,
    };

    let resp = SurgicalPatcher::patch_by_ff_id(source, &req).expect("should handle gracefully");
    // Must NOT destroy ternary logic or overwrite with raw text
    assert!(!resp.success);
    assert_eq!(resp.patched_source, source);
}

#[test]
fn test_single_quote_attribute_preservation() {
    let source = r#"<button data-ff-id="ff-save-btn" title='Save Draft'>Submit</button>"#;

    let req = SurgicalPatchRequest {
        ff_id: "ff-save-btn".to_string(),
        patch_kind: PatchKind::AttributeValue,
        new_value: "Save Changes".to_string(),
        source: source.to_string(),
        target_attribute: Some("title".to_string()),
    };

    let resp = SurgicalPatcher::patch_by_ff_id(source, &req).expect("patch should succeed");
    assert!(resp.success);
    assert_eq!(resp.original_text.as_deref(), Some("'Save Draft'"));
    // Must preserve single quotes!
    assert!(resp.patched_source.contains("title='Save Changes'"));
}

#[test]
fn test_css_variable_property_patching() {
    let source = r#"<div data-ff-id="ff-theme-box" style={{ color: 'red', '--sidebar-width': '240px' }}>Content</div>"#;

    let req = SurgicalPatchRequest {
        ff_id: "ff-theme-box".to_string(),
        patch_kind: PatchKind::CssVariable,
        new_value: "'320px'".to_string(),
        source: source.to_string(),
        target_attribute: Some("--sidebar-width".to_string()),
    };

    let resp = SurgicalPatcher::patch_by_ff_id(source, &req).expect("patch should succeed");
    assert!(resp.success);
    // color: 'red' must be preserved, only --sidebar-width value is patched!
    assert!(resp.patched_source.contains("color: 'red'"));
    assert!(resp.patched_source.contains("'--sidebar-width': '320px'"));
}

#[test]
fn test_act_on_01_surgical_patch_inside_expression_container() {
    let source = r#"
        export const NotificationModal = ({ isOpen }: { isOpen: boolean }) => (
            <div data-ff-id="ff-modal-wrap">
                {isOpen && (
                    <div data-ff-id="ff-modal-content">
                        <span data-ff-id="ff-modal-msg">Initial Notification</span>
                    </div>
                )}
            </div>
        );
    "#;

    let req = SurgicalPatchRequest {
        ff_id: "ff-modal-msg".to_string(),
        patch_kind: PatchKind::TextContent,
        new_value: "Updated Alert Notification".to_string(),
        source: source.to_string(),
        target_attribute: None,
    };

    let resp = SurgicalPatcher::patch_by_ff_id(source, &req).expect("patch should succeed");
    assert!(resp.success);
    assert_eq!(resp.original_text.as_deref(), Some("Initial Notification"));
    assert!(resp
        .patched_source
        .contains("<span data-ff-id=\"ff-modal-msg\">Updated Alert Notification</span>"));

    // Also test injection inside dynamic expression container
    let raw_source = r#"
        export const DynamicContainer = ({ hasData }: any) => (
            <div>
                {hasData && <section><p>Dynamic Paragraph</p></section>}
            </div>
        );
    "#;
    let injected =
        SurgicalPatcher::inject_ff_ids(raw_source, Some("DynamicContainer")).expect("inject");
    assert!(injected
        .tagged_source
        .contains("data-ff-id=\"ff-dynamiccontainer-section-1\""));
    assert!(injected
        .tagged_source
        .contains("data-ff-id=\"ff-dynamiccontainer-p-1\""));
}

#[test]
fn test_act_on_02_css_variable_quote_preservation() {
    // 1. Double-quote preservation when caller provides unquoted string
    let source_double = r##"<div data-ff-id="ff-theme-double" style={{ "--primary-color": "#ff0000" }}>Double</div>"##;
    let req_double = SurgicalPatchRequest {
        ff_id: "ff-theme-double".to_string(),
        patch_kind: PatchKind::CssVariable,
        new_value: "#00ff00".to_string(), // Unquoted by caller!
        source: source_double.to_string(),
        target_attribute: Some("--primary-color".to_string()),
    };
    let resp_double =
        SurgicalPatcher::patch_by_ff_id(source_double, &req_double).expect("patch double");
    assert!(resp_double.success);
    assert!(resp_double
        .patched_source
        .contains(r##""--primary-color": "#00ff00""##));

    // 2. Single-quote preservation when caller provides unquoted string
    let source_single = r##"<div data-ff-id="ff-theme-single" style={{ '--primary-color': '#ff0000' }}>Single</div>"##;
    let req_single = SurgicalPatchRequest {
        ff_id: "ff-theme-single".to_string(),
        patch_kind: PatchKind::CssVariable,
        new_value: "#0000ff".to_string(), // Unquoted by caller!
        source: source_single.to_string(),
        target_attribute: Some("--primary-color".to_string()),
    };
    let resp_single =
        SurgicalPatcher::patch_by_ff_id(source_single, &req_single).expect("patch single");
    assert!(resp_single.success);
    assert!(resp_single
        .patched_source
        .contains(r##"'--primary-color': '#0000ff'"##));
}
