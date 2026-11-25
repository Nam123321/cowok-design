// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (Unit & Integration Tests for 4-Axis Tournament, AST Distance Gate <=0.78 + Re-roll Cap 2, Position-Swapped RubricShield, Cross-Variant Borrow & advisor 6-Step Kernel Guards)
use std::collections::BTreeSet;
use std::fs;

use bot_fleet::decision_cascade::evaluate_rubric_pairwise_with_judge;
use bot_fleet::design_tournament::{
    assert_advisor_target_allowed, compute_ast_structural_similarity, generate_axis_html,
    is_safe_tournament_id, is_valid_ff_id, merge_cross_variant_borrow, run_advisor_gepa_loop,
    run_design_tournament, AdvisorEvolveInput, AdvisorStepKind, CreativeMemorySignal, DesignAxis,
    StudioCorrectionCluster, TournamentError, TournamentStartConfig, VariantBranchOverride,
    HUMAN_APPROVAL_POLICY, MAX_AST_REROLLS, MAX_AST_SIMILARITY, MAX_TOURNAMENT_BUDGET_MS,
};
use domain::decision::{DecisionCandidate, RubricPairwiseInput};
use tempfile::tempdir;

#[test]
fn four_orthogonal_axes_pass_ast_distance_and_emit_anonymized_blind_labels() {
    let out = run_design_tournament(&TournamentStartConfig {
        project_id: "proj-103".to_string(),
        prompt: "Design a calm analytics workspace for operations leads".to_string(),
        axes: vec![],
        budget_ms: MAX_TOURNAMENT_BUDGET_MS,
        branch_overrides: vec![],
        rubric_criteria: vec![],
    })
    .expect("4-axis tournament must succeed");

    assert_eq!(out.variants.len(), 4);
    assert!(!out.degraded_min_variants);
    assert!(!out.low_diversity_warning);
    assert!(out.elapsed_ms <= MAX_TOURNAMENT_BUDGET_MS);
    assert!(out.rubric_summary.all_pairs_swap_consistent);
    assert_eq!(out.rubric_summary.pairwise_comparisons.len(), 6);

    let labels: Vec<&str> = out
        .variants
        .iter()
        .map(|v| v.display_label.as_str())
        .collect();
    assert_eq!(labels, vec!["Option A", "Option B", "Option C", "Option D"]);

    for v in &out.variants {
        assert!(
            v.ast_similarity <= MAX_AST_SIMILARITY,
            "axis {:?} similarity {} exceeded {}",
            v.axis,
            v.ast_similarity,
            MAX_AST_SIMILARITY
        );
        assert!(v.clone_distance_passed);
        assert!(!v.low_diversity_warning);
        let lower_label = v.display_label.to_ascii_lowercase();
        assert!(!lower_label.contains("claude"));
        assert!(!lower_label.contains("gemini"));
        assert!(!lower_label.contains("gpt"));
    }
}

#[test]
fn timeout_degrades_gracefully_when_at_least_two_variants_complete() {
    let out = run_design_tournament(&TournamentStartConfig {
        project_id: "proj-103-timeout".to_string(),
        prompt: "Checkout flow redesign".to_string(),
        axes: DesignAxis::ALL.to_vec(),
        budget_ms: 15_000,
        branch_overrides: vec![
            VariantBranchOverride {
                axis: DesignAxis::VisualMetaphor,
                initial_html: None,
                reroll_htmls: vec![],
                simulated_latency_ms: Some(18_500),
            },
            VariantBranchOverride {
                axis: DesignAxis::InteractionRhythm,
                initial_html: None,
                reroll_htmls: vec![],
                simulated_latency_ms: Some(20_000),
            },
        ],
        rubric_criteria: vec![],
    })
    .expect("should degrade cleanly to 2 surviving variants");

    assert_eq!(out.variants.len(), 2);
    assert!(out.degraded_min_variants);
    assert_eq!(out.timed_out_axes.len(), 2);

    // If fewer than 2 survive, tournament must fail cleanly with InsufficientVariants.
    let err = run_design_tournament(&TournamentStartConfig {
        project_id: "proj-103-timeout-fail".to_string(),
        prompt: "Checkout flow redesign".to_string(),
        axes: DesignAxis::ALL.to_vec(),
        budget_ms: 15_000,
        branch_overrides: vec![
            VariantBranchOverride {
                axis: DesignAxis::NavigationTopology,
                initial_html: None,
                reroll_htmls: vec![],
                simulated_latency_ms: Some(16_000),
            },
            VariantBranchOverride {
                axis: DesignAxis::VisualMetaphor,
                initial_html: None,
                reroll_htmls: vec![],
                simulated_latency_ms: Some(17_000),
            },
            VariantBranchOverride {
                axis: DesignAxis::InteractionRhythm,
                initial_html: None,
                reroll_htmls: vec![],
                simulated_latency_ms: Some(18_000),
            },
        ],
        rubric_criteria: vec![],
    })
    .expect_err("must reject when < 2 variants survive");

    assert!(matches!(
        err,
        TournamentError::InsufficientVariants {
            generated: 1,
            required: 2
        }
    ));
}

#[test]
fn ast_structural_distance_gate_rerolls_clones_and_caps_at_max_two_rerolls() {
    let base_html = generate_axis_html("Pricing Console", DesignAxis::InformationDensity, 0);
    // Same DOM/CSS skeleton with slightly different copy still has AST similarity > 0.78!
    let copy_only_clone = base_html.replace("Executive overview", "Alternative wording");
    let sim = compute_ast_structural_similarity(&base_html, &copy_only_clone);
    assert!(
        sim > MAX_AST_SIMILARITY,
        "copy-only clone must score > {MAX_AST_SIMILARITY} (got {sim})"
    );

    // 1. Clone recovers on re-roll (default orthogonal generator on attempt 1 drops similarity <= 0.78).
    let recovered = run_design_tournament(&TournamentStartConfig {
        project_id: "proj-reroll-ok".to_string(),
        prompt: "Pricing Console".to_string(),
        axes: vec![
            DesignAxis::InformationDensity,
            DesignAxis::NavigationTopology,
        ],
        budget_ms: 15_000,
        branch_overrides: vec![VariantBranchOverride {
            axis: DesignAxis::NavigationTopology,
            initial_html: Some(copy_only_clone.clone()),
            reroll_htmls: vec![],
            simulated_latency_ms: None,
        }],
        rubric_criteria: vec![],
    })
    .expect("recovered run");
    assert_eq!(recovered.variants[1].reroll_attempts, 1);
    assert!(recovered.variants[1].clone_distance_passed);
    assert!(!recovered.low_diversity_warning);

    // 2. Persistent clone hits MAX_AST_REROLLS = 2 cap without infinite loop, sets low_diversity_warning.
    let capped = run_design_tournament(&TournamentStartConfig {
        project_id: "proj-reroll-cap".to_string(),
        prompt: "Pricing Console".to_string(),
        axes: vec![
            DesignAxis::InformationDensity,
            DesignAxis::NavigationTopology,
        ],
        budget_ms: 15_000,
        branch_overrides: vec![VariantBranchOverride {
            axis: DesignAxis::NavigationTopology,
            initial_html: Some(copy_only_clone.clone()),
            reroll_htmls: vec![copy_only_clone.clone(), copy_only_clone.clone()],
            simulated_latency_ms: None,
        }],
        rubric_criteria: vec![],
    })
    .expect("capped run must terminate cleanly");

    assert_eq!(capped.variants[1].reroll_attempts, MAX_AST_REROLLS);
    assert!(!capped.variants[1].clone_distance_passed);
    assert!(capped.variants[1].low_diversity_warning);
    assert!(capped.low_diversity_warning);
}

#[test]
fn rubric_shield_position_swap_detects_biased_judge_and_breaks_ties_deterministically() {
    let pair = RubricPairwiseInput {
        candidate_x: DecisionCandidate {
            id: "var-a".to_string(),
            label: "Option A".to_string(),
            text: "Editorial spacious layout".to_string(),
            de_score_bps: 8_400,
        },
        candidate_y: DecisionCandidate {
            id: "var-b".to_string(),
            label: "Option B".to_string(),
            text: "Split rail workspace".to_string(),
            de_score_bps: 9_100,
        },
        rubric_criteria: vec!["clarity".to_string()],
    };

    // Biased judge always picks whichever candidate is presented first -> (A,B)=A, (B,A)=B.
    let outcome = evaluate_rubric_pairwise_with_judge(&pair, |first, _second, _| first.id.clone());
    assert!(!outcome.swap_consistent);
    assert!(outcome.tie_break_applied);
    assert_eq!(outcome.winner_id, "var-b");
    assert_eq!(outcome.winner_label, "Option B");
}

#[test]
fn cross_variant_borrow_merges_zone_css_rules_and_eliminates_duplicate_ff_ids() {
    let winner_html = r#"<!DOCTYPE html><html><head><style>
:root { --winner-bg: #fff; }
.hero { display: flex; }
</style></head><body><main data-ff-id="ff-main">
  <header class="hero" data-ff-id="ff-hero-banner"><h1>Winner</h1></header>
  <section data-ff-id="ff-kpi-summary"><p>KPI</p></section>
</main></body></html>"#;

    let donor_html = r#"<!DOCTYPE html><html><head><style>
:root { --donor-accent: #7c3aed; }
.donor-hero { display: grid; gap: 12px; color: var(--donor-accent); }
</style></head><body><main data-ff-id="ff-main">
  <header class="donor-hero" data-ff-id="ff-hero-banner">
    <h2 data-ff-id="ff-kpi-summary">Borrowed Banner</h2>
  </header>
</main></body></html>"#;

    let res = merge_cross_variant_borrow(winner_html, donor_html, "ff-hero-banner")
        .expect("borrow merge must succeed");

    assert_eq!(res.duplicate_ff_ids_after_merge, 0);
    assert!(res.borrowed_ff_id.starts_with("ff-hero-banner-borrowed-"));
    assert_eq!(res.remapped_ff_ids.len(), 2);
    assert!(res
        .extracted_css_rules
        .iter()
        .any(|r| r.contains("--donor-accent")));
    assert!(res
        .extracted_css_rules
        .iter()
        .any(|r| r.contains(".donor-hero")));
    assert!(res.merged_html.contains("--donor-accent"));
    assert!(res.merged_html.contains("Borrowed Banner"));

    // Verify every data-ff-id in merged HTML is strictly unique.
    let mut seen = BTreeSet::new();
    for part in res.merged_html.split("data-ff-id=\"").skip(1) {
        if let Some(id) = part.split('"').next() {
            assert!(
                seen.insert(id.to_string()),
                "duplicate data-ff-id '{id}' found"
            );
        }
    }
}

#[test]
fn advisor_gepa_loop_runs_six_steps_and_enforces_kernel_security_guards() {
    let dir = tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("tests/eval/sealed")).expect("mkdir sealed");
    fs::write(dir.path().join("tests/eval/sealed/suite.json"), "{}").expect("seed sealed");
    fs::create_dir_all(dir.path().join("skills/design-craft")).expect("mkdir skills");

    // 1. Happy path -> all 6 steps executed, STAGING_DRAFT + HUMAN_ONLY.
    let out = run_advisor_gepa_loop(
        Some(dir.path()),
        &AdvisorEvolveInput {
            project_id: "proj-103".to_string(),
            target_skill_path: "skills/design-craft/SKILL.md".to_string(),
            reference_paths: vec!["skills/design-craft/references/typography.md".to_string()],
            promote_directly: false,
            target_channel: "DESIGN_CRAFT_STAGING".to_string(),
            memory_signals: vec![CreativeMemorySignal {
                event_id: "evt-1".to_string(),
                event_kind: "TOURNAMENT_BORROW".to_string(),
                summary: "Users repeatedly borrowed split-rail nav".to_string(),
            }],
            studio_correction_clusters: vec![
                StudioCorrectionCluster {
                    ledger_id: "led-1".to_string(),
                    rule_id: "palette_contrast_fix".to_string(),
                    sample_message: "Raised muted label contrast >= 4.5:1".to_string(),
                    hit_count: 4,
                },
                StudioCorrectionCluster {
                    ledger_id: "led-2".to_string(),
                    rule_id: "minor_tweak".to_string(),
                    sample_message: "Single occurrence ignored".to_string(),
                    hit_count: 1,
                },
            ],
        },
    )
    .expect("advisor staging loop must succeed");

    let step_kinds: Vec<AdvisorStepKind> = out.steps.iter().map(|s| s.step).collect();
    assert_eq!(step_kinds, AdvisorStepKind::ALL.to_vec());
    assert_eq!(out.staging_status, "STAGING_DRAFT");
    assert_eq!(out.approval_policy, HUMAN_APPROVAL_POLICY);
    assert!(!out.promoted_to_production);
    assert!(out.sealed_eval_untouched);
    assert!(out.regression_passed);
    assert_eq!(out.signals_consumed, 1);
    assert_eq!(out.studio_corrections_consumed, 1);

    // 2. Kernel Guard 1: SEALED_EVAL_TAMPER_ERROR across traversal, backslash, percent-encoding, symlink.
    let tamper_paths = [
        "tests/eval/sealed/suite.json",
        "../tests/eval/sealed/suite.json",
        "skills/design-craft/../../tests/eval/sealed/suite.json",
        r"tests\eval\sealed\suite.json",
        "%2e%2e/tests/eval/sealed/suite.json",
        "skills/%252e%252e/tests/eval/sealed/suite.json",
    ];
    for bad in tamper_paths {
        let err = assert_advisor_target_allowed(Some(dir.path()), bad)
            .expect_err(&format!("must reject tampered path '{bad}'"));
        assert!(
            matches!(err, TournamentError::SealedEvalTamper { .. }),
            "expected SealedEvalTamper for '{bad}', got {err:?}"
        );
    }

    #[cfg(unix)]
    {
        let link_path = dir.path().join("skills/design-craft/sneaky-link");
        std::os::unix::fs::symlink(dir.path().join("tests/eval/sealed"), &link_path)
            .expect("create symlink");
        let err = assert_advisor_target_allowed(
            Some(dir.path()),
            "skills/design-craft/sneaky-link/suite.json",
        )
        .expect_err("symlink into tests/eval/sealed must be rejected");
        assert!(matches!(err, TournamentError::SealedEvalTamper { .. }));
    }

    // 3. Kernel Guard 2: HUMAN_APPROVAL_REQUIRED on promote_directly or non-staging channel.
    let promote_err = run_advisor_gepa_loop(
        Some(dir.path()),
        &AdvisorEvolveInput {
            project_id: "proj-103".to_string(),
            target_skill_path: "skills/design-craft/SKILL.md".to_string(),
            reference_paths: vec![],
            promote_directly: true,
            target_channel: "DESIGN_CRAFT_STAGING".to_string(),
            memory_signals: vec![],
            studio_correction_clusters: vec![],
        },
    )
    .expect_err("promote_directly=true must be blocked");
    assert!(matches!(
        promote_err,
        TournamentError::HumanApprovalRequired { .. }
    ));

    let prod_channel_err = run_advisor_gepa_loop(
        Some(dir.path()),
        &AdvisorEvolveInput {
            project_id: "proj-103".to_string(),
            target_skill_path: "skills/design-craft/SKILL.md".to_string(),
            reference_paths: vec![],
            promote_directly: false,
            target_channel: "PRODUCTION".to_string(),
            memory_signals: vec![],
            studio_correction_clusters: vec![],
        },
    )
    .expect_err("PRODUCTION channel must be blocked");
    assert!(matches!(
        prod_channel_err,
        TournamentError::HumanApprovalRequired { .. }
    ));
}

#[test]
fn symmetric_ast_distance_flags_variant_zero_equally_when_clones_persist() {
    let base_html = generate_axis_html("Symmetric Gate Test", DesignAxis::InformationDensity, 0);
    let clone_html = base_html.replace("Executive overview", "Cloned copy");

    let run = run_design_tournament(&TournamentStartConfig {
        project_id: "proj-sym-ast".to_string(),
        prompt: "Symmetric Gate Test".to_string(),
        axes: vec![
            DesignAxis::InformationDensity,
            DesignAxis::NavigationTopology,
        ],
        budget_ms: 15_000,
        branch_overrides: vec![VariantBranchOverride {
            axis: DesignAxis::NavigationTopology,
            initial_html: Some(clone_html.clone()),
            reroll_htmls: vec![clone_html.clone(), clone_html.clone()],
            simulated_latency_ms: None,
        }],
        rubric_criteria: vec![],
    })
    .expect("run must complete symmetrically");

    assert_eq!(run.variants.len(), 2);
    // Variant 0 MUST NOT receive artificial ast_similarity = 0.0 when Variant 1 clones it!
    assert!(
        run.variants[0].ast_similarity > MAX_AST_SIMILARITY,
        "Variant 0 ast_similarity must reflect symmetric max_sim (got {})",
        run.variants[0].ast_similarity
    );
    assert!(
        (run.variants[0].ast_similarity - run.variants[1].ast_similarity).abs() < 1e-9,
        "Variant 0 ({}) and Variant 1 ({}) must have symmetric ast_similarity",
        run.variants[0].ast_similarity,
        run.variants[1].ast_similarity
    );
    assert!(!run.variants[0].clone_distance_passed);
    assert!(run.variants[0].low_diversity_warning);
    assert!(!run.variants[1].clone_distance_passed);
    assert!(run.variants[1].low_diversity_warning);
    assert_eq!(run.variants[0].de_score_bps, run.variants[1].de_score_bps);
}

#[test]
fn borrow_rejects_attr_breakout_xss_handles_unquoted_ff_ids_void_tags_and_scopes_root_css() {
    let winner_html = r#"<!DOCTYPE html><html><head><style>
:root { --accent: #111111; }
.win-card { color: var(--accent); }
</style></head><body><main data-ff-id="ff-main">
  <section class="win-card" data-ff-id=ff-hero-banner><h1>Winner</h1></section>
</main></body></html>"#;

    // 1. Attribute-breakout XSS inside data-ff-id MUST be rejected.
    let xss_donor = r#"<!DOCTYPE html><html><body><main>
      <header data-ff-id='ff-bad" onmouseover="alert(1)'><h2>Evil</h2></header>
    </main></body></html>"#;
    assert!(!is_valid_ff_id(r#"ff-bad" onmouseover="alert(1)"#));
    let xss_err = merge_cross_variant_borrow(winner_html, xss_donor, "ff-bad")
        .expect_err("attribute breakout XSS in data-ff-id must be rejected");
    assert!(matches!(xss_err, TournamentError::InvalidInput(_)));

    // 2. Unquoted data-ff-id collision + void element + `>` inside quoted attribute +
    //    `</div-badge>` word boundary + `@media` balanced braces + scoped `:root` CSS.
    let rich_donor = r#"<!DOCTYPE html><html><head><style>
:root { --accent: #ff00aa; --donor-only: #22c55e; }
.donor-box { display: grid; color: var(--accent); }
@media (max-width: 640px) {
  .donor-box { display: flex; gap: 8px; }
}
</style></head><body><main>
  <div class="donor-box" title="step 1 > step 2" data-ff-id=ff-hero-banner>
    <input type="text" value="a > b" data-ff-id=ff-void-input>
    <div-badge>Keep going</div-badge>
    <span data-ff-id="ff-after-badge">Survived Boundary</span>
  </div>
</main></body></html>"#;

    let merged = merge_cross_variant_borrow(winner_html, rich_donor, "ff-hero-banner")
        .expect("rich donor borrow must succeed");

    // Unquoted collision remapped cleanly with zero duplicates!
    assert_eq!(merged.duplicate_ff_ids_after_merge, 0);
    assert!(merged
        .borrowed_ff_id
        .starts_with("ff-hero-banner-borrowed-"));
    assert!(merged.merged_html.contains("Survived Boundary"));
    assert!(merged.merged_html.contains("ff-void-input"));

    // Winner's `:root { --accent: #111111; }` MUST NOT be overridden by donor `:root`;
    // donor `:root` variables MUST be scoped onto `.cross-variant-borrow-slot`.
    assert!(merged
        .extracted_css_rules
        .iter()
        .any(|r| r.contains(".cross-variant-borrow-slot") && r.contains("--donor-only")));
    assert!(
        !merged
            .extracted_css_rules
            .iter()
            .any(|r| r.trim_start().starts_with(":root")),
        "borrowed CSS rules must never emit unscoped :root: {:?}",
        merged.extracted_css_rules
    );
    // `@media` block MUST remain syntactically balanced (not split on inner `}`).
    let media_rule = merged
        .extracted_css_rules
        .iter()
        .find(|r| r.starts_with("@media"))
        .expect("@media rule must be preserved intact");
    assert_eq!(
        media_rule.matches('{').count(),
        media_rule.matches('}').count(),
        "balanced braces required in @media rule: {media_rule}"
    );
}

#[test]
fn kernel_guard_blocks_post_decode_nul_4x_encoding_non_skill_paths_and_dot_id() {
    let dir = tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("skills/design-craft/references")).expect("mkdir");
    fs::create_dir_all(dir.path().join(".cowok-design/staging/design-craft")).expect("mkdir");

    // Reject single-dot identifier `.`
    assert!(!is_safe_tournament_id("."));
    assert!(!is_safe_tournament_id(".."));
    assert!(is_safe_tournament_id("run-103.v1"));

    // Valid allowlisted paths
    assert!(
        assert_advisor_target_allowed(Some(dir.path()), "skills/design-craft/SKILL.md").is_ok()
    );
    assert!(assert_advisor_target_allowed(
        Some(dir.path()),
        "skills/design-craft/references/typography.md"
    )
    .is_ok());
    assert!(assert_advisor_target_allowed(
        Some(dir.path()),
        ".cowok-design/staging/design-craft/adv-1/SKILL.draft.md"
    )
    .is_ok());

    let blocked_cases = [
        // Post-decode NUL byte (`%00`) and double-encoded NUL (`%2500`)
        "skills/design-craft/SKILL.md%00.txt",
        "skills/design-craft/SKILL.md%2500",
        // Post-decode control character (`%0a`)
        "skills/design-craft/SKILL.md%0a",
        // 4x-encoded traversal into tests/eval/sealed
        "%2525252e%2525252e/tests/eval/sealed/suite.json",
        // 9x-encoded residual percent-encoding (exceeds 8-round fixpoint cap -> fail closed)
        "skills/design-craft/%2525252525252525252e%2525252525252525252e/SKILL.md",
        // Non-skill paths outside explicit allowlist
        "crates/bot-fleet/src/design_tournament.rs",
        "src/App.tsx",
        "Cargo.toml",
        "skills/design-craft/exploit.sh",
        "skills/design-craft/references/data.json",
        "skills/SKILL.md",
    ];

    for bad in blocked_cases {
        let err = assert_advisor_target_allowed(Some(dir.path()), bad)
            .expect_err(&format!("expected SealedEvalTamper for '{bad}'"));
        assert!(
            matches!(err, TournamentError::SealedEvalTamper { .. }),
            "expected SealedEvalTamper for '{bad}', got {err:?}"
        );
    }
}
