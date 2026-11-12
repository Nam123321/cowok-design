// @story 02.2
// @implements AC-01, AC-02, AC-03
//! Integration test suite for Story 02.2: Token Budget Governor (<= 1,500 Input Tokens)
//! & Deterministic Prompt Compiler (`crates/domain/src/token_budget.rs`, `crates/domain/src/prompt_compiler.rs`).

use domain::{
    AuthorizedContextSlice, CanonicalSpecKey, CompilationInput, ContextSliceDecision, Department,
    DeterministicPromptCompiler, Portal, PriorityTier, PromptSection, ReasonCode, SpecSlice,
    TaskHorizon, TokenBudgetGovernor, TOKEN_BUDGET_CEILING,
};
use uuid::Uuid;

#[test]
fn it_02_2_ac01_deterministic_prompt_compiler_and_1500_ceiling() {
    let compiler = DeterministicPromptCompiler::new(TOKEN_BUDGET_CEILING);
    let project_id = Uuid::new_v4();

    let make_input = || CompilationInput {
        project_id: project_id.to_string(),
        system_instruction: "You are the Cowok-Design deterministic prompt compiler.".into(),
        project_constraints: vec![
            "Embedded SQLite WAL only".into(),
            "Strict <= 1500 input token budget".into(),
        ],
        context_slice: Some(AuthorizedContextSlice {
            project_id,
            decision: ContextSliceDecision {
                horizon: TaskHorizon::Mid,
                department: Department::ProductUi,
                portal: Portal::TenantAdmin,
                allowed_spec_keys: vec![
                    CanonicalSpecKey::ProductOverview,
                    CanonicalSpecKey::Architecture,
                ],
                denied_spec_keys: vec![],
                token_ceiling: 1000,
                primary_reason: ReasonCode::Success,
            },
            slices: vec![
                SpecSlice {
                    spec_key: CanonicalSpecKey::ProductOverview,
                    title: "01-product-overview.md".into(),
                    content: "# Product Overview\n- [CONFIRMED] Rust core harness.".into(),
                    estimated_tokens: 20,
                    priority_rank: 1,
                },
                SpecSlice {
                    spec_key: CanonicalSpecKey::Architecture,
                    title: "04-architecture.md".into(),
                    content: "# Architecture\n- [CONFIRMED] 11-crate workspace.".into(),
                    estimated_tokens: 20,
                    priority_rank: 2,
                },
            ],
            total_estimated_tokens: 40,
            atomic_complete: true,
        }),
        task_instructions: "Render a DataTableView and MetricLine dashboard.".into(),
        optional_components: vec![
            "DataTableView".into(),
            "MetricLine".into(),
            "RightDrawer".into(),
        ],
    };

    let out_a = compiler.compile(make_input()).expect("compile run A");
    let out_b = compiler.compile(make_input()).expect("compile run B");

    assert_eq!(out_a.compiled_prompt, out_b.compiled_prompt);
    assert_eq!(out_a.sha256_digest, out_b.sha256_digest);
    assert!(out_a.token_count <= TOKEN_BUDGET_CEILING);
    assert_eq!(out_a.token_ceiling, TOKEN_BUDGET_CEILING);
}

#[test]
fn it_02_2_ac02_and_ac03_semantic_relevance_compression_under_tight_budget() {
    let governor = TokenBudgetGovernor::new(75);
    let sections = vec![
        PromptSection::new(
            "p0-sys",
            PriorityTier::P0Invariant,
            "System",
            "# SYSTEM INSTRUCTIONS\nEnforce project constraints.",
        ),
        PromptSection::new(
            "p2-ui",
            PriorityTier::P2OptionalUi,
            "Optional UI",
            "# OPTIONAL COMPONENTS\n- [COMPONENT relevance=45] DataTableView\n- [COMPONENT relevance=35] MetricLine\n- [COMPONENT relevance=25] AdvancedFilterBar\n- [COMPONENT relevance=5] Accordion\n- [COMPONENT relevance=5] Avatar\n- [COMPONENT relevance=5] Carousel\n".repeat(3),
        ),
    ];

    let governed = governor
        .govern_sections(sections)
        .expect("govern_sections within 75 tokens");
    assert!(governed.total_tokens <= 75);
    assert!(
        !governed.compressed_section_ids.is_empty() || !governed.pruned_section_ids.is_empty(),
        "P2 optional UI must be compressed or pruned when exceeding budget"
    );
}
