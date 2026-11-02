---
schema: "agent-enablement/v1.1"
story_id: "Epic-01"
story_hash: "sha256:epic-01-agent-surface-map-v1"
contract_hash: "sha256:epic-01-agent-surface-contract-v1"
status: "PLAN_BOUND"
components:
  - component_id: "crates/domain"
    trust_owner: "KERNEL"
    exposure: "NOT_NEEDED"
    form: "NONE"
    rationale: "Compile-time Rust structs/enums and ts-rs TypeScript type generator; no runtime agent tool surface."
  - component_id: "crates/spec-registry"
    trust_owner: "KERNEL"
    exposure: "AGENT_FACING"
    form: "TOOL"
    rationale: "Exposes deterministic read/write access to the 9 Foundational Specs Vault and 16-table SQLite WAL index for the cowok Orchestrator and advisor Meta-Agent with mandatory SHA-256 StaleFile OCC."
    owners:
      platform_owner: "core-harness-team"
      domain_owner: "spec-registry-lead"
      approver: "system-architect"
      on_call: "harness-oncall"
    authority:
      identity: "delegated_user"
      pbac_inherit: true
    state_change:
      persistence: "NEW_SCHEMA"
      migration_required: true
      seed_required: true
      rollback_kind: "DOWN_MIGRATION"
      rationale: "Initializes the 16 SQLite WAL tables via refinery migrations and syncs with product/specs/*.md."
      refs:
        - status: "planned"
          path: "crates/spec-registry/src/lib.rs"
    agent_ops_harness:
      execution_lane: "LANE_1_DETERMINISTIC"
      stream_alignment: "S1_IDENTITY_CORE"
      memory_scope:
        tenant_rls_strict: true
        context_token_budget: 0
        cross_tenant_leakage_guard: true
      pipeline_9stage:
        intent_gate:
          mode: "ZOD_AND_SERDE_STRICT"
          ref:
            status: "planned"
            path: "crates/domain/src/lib.rs"
        budget_guard:
          mode: "ZERO_TOKEN_DETERMINISTIC"
          ref:
            status: "planned"
            path: "crates/spec-registry/src/lib.rs"
        retrieval:
          mode: "SQLITE_WAL_INDEXED_LOOKUP"
          ref:
            status: "planned"
            path: "crates/spec-registry/src/lib.rs"
        model_invoker:
          mode: "NONE_ZERO_TOKEN"
          ref:
            status: "planned"
            path: "crates/spec-registry/src/lib.rs"
        output_parser:
          mode: "TYPED_RUST_STRUCT"
          ref:
            status: "planned"
            path: "crates/domain/src/lib.rs"
        safety_guard:
          mode: "SHA256_STALE_FILE_OCC_AND_PATH_JAIL"
          ref:
            status: "planned"
            path: "crates/spec-registry/src/lib.rs"
        approval_gate:
          mode: "OCC_HASH_MATCH_REQUIRED"
          ref:
            status: "planned"
            path: "crates/spec-registry/src/lib.rs"
        execution_port:
          mode: "RUSQLITE_WAL_TRANSACTION"
          ref:
            status: "planned"
            path: "crates/spec-registry/src/lib.rs"
        audit_ledger:
          mode: "SPEC_REVISIONS_IMMUTABLE_LOG"
          ref:
            status: "planned"
            path: "crates/spec-registry/src/lib.rs"
      control_plane:
        registered_in_catalog: true
        canary_supported: true
        rollback_version_pointer: "v0.1.0-sqlite-genesis"
        memory_limit_mb: 64
        timeout_ms: 5000
    tools:
      - name: "uxui_read_project_specs"
        effect_class: "READ_ONLY"
        idempotency: true
        hitl: false
        base_version: false
        mutates_shared_artifact: false
        zod_schema_ref:
          status: "planned"
          path: "packages/shared-types/src/index.ts"
        error_codes:
          - code: "PROJECT_NOT_FOUND"
            retryable: false
            hint: "Verify project_id exists in PROJECTS table before reading specs."
        description: "Reads the 9 Foundational Project Context Specs and their SHA-256 hashes + provenance tags (CONFIRMED/INFERRED/MISSING)."
      - name: "uxui_update_project_spec"
        effect_class: "IDEMPOTENT_WRITE"
        idempotency: true
        hitl: false
        base_version: true
        mutates_shared_artifact: true
        zod_schema_ref:
          status: "planned"
          path: "packages/shared-types/src/index.ts"
        error_codes:
          - code: "STALE_FILE"
            retryable: true
            hint: "Re-read the spec to obtain the latest expected_sha256 before retrying."
          - code: "INVALID_SPEC_KEY"
            retryable: false
            hint: "Use one of the 9 canonical spec keys (01-product-overview .. 09-data-and-copy)."
        description: "Atomically updates a foundational spec in both product/specs/*.md and SQLite PROJECT_SPECS with mandatory expected_sha256 OCC check."
    consistency:
      single_core_port:
        status: "planned"
        path: "crates/spec-registry/src/lib.rs"
      ui_agent_parity_test:
        status: "planned"
        path: "crates/spec-registry/tests/occ_parity_test.rs"
    safety:
      tool_output_untrusted: true
      poisoning_scan_receipt:
        status: "planned"
        path: "crates/spec-registry/src/lib.rs"
  - component_id: "crates/harness-daemon"
    trust_owner: "KERNEL"
    exposure: "AGENT_PROPOSE_ONLY"
    form: "PLAYBOOK"
    rationale: "Synthesizes the 9 Foundational Project Context Specs from a raw Product Brief or seeded repo, marking inferred sections as INFERRED/MISSING for human or advisor review before locking."
    owners:
      platform_owner: "core-harness-team"
      domain_owner: "daemon-lead"
      approver: "system-architect"
      on_call: "harness-oncall"
    authority:
      identity: "delegated_user"
      pbac_inherit: true
    state_change:
      persistence: "EXISTING_SCHEMA"
      migration_required: false
      seed_required: false
      rollback_kind: "VERSION_POINTER"
      rationale: "Writes synthesized spec drafts into PROJECT_SPECS and SPEC_REVISIONS tables created by crates/spec-registry."
      refs:
        - status: "planned"
          path: "crates/harness-daemon/src/main.rs"
    agent_ops_harness:
      execution_lane: "LANE_2_OPERATIONAL_NLP"
      stream_alignment: "S2_WORKFLOW_ENGINE"
      memory_scope:
        tenant_rls_strict: true
        context_token_budget: 3500
        cross_tenant_leakage_guard: true
      pipeline_9stage:
        intent_gate:
          mode: "AXUM_TYPED_PAYLOAD_VALIDATION"
          ref:
            status: "planned"
            path: "crates/harness-daemon/src/main.rs"
        budget_guard:
          mode: "SYNTHESIS_TOKEN_CAP_3500"
          ref:
            status: "planned"
            path: "crates/harness-daemon/src/main.rs"
        retrieval:
          mode: "BRIEF_AND_REUSABLE_BRAND_PROFILE_FETCH"
          ref:
            status: "planned"
            path: "crates/harness-daemon/src/main.rs"
        model_invoker:
          mode: "STRUCTURED_9SPEC_SYNTHESIZER"
          ref:
            status: "planned"
            path: "crates/harness-daemon/src/main.rs"
        output_parser:
          mode: "PROVENANCE_TAG_VALIDATOR"
          ref:
            status: "planned"
            path: "crates/harness-daemon/src/main.rs"
        safety_guard:
          mode: "PATH_JAIL_AND_SECRET_SCRUBBER"
          ref:
            status: "planned"
            path: "crates/harness-daemon/src/main.rs"
        approval_gate:
          mode: "HITL_PROPOSAL_CARD"
          ref:
            status: "planned"
            path: "crates/harness-daemon/src/main.rs"
        execution_port:
          mode: "SPEC_REGISTRY_ATOMIC_BATCH_WRITE"
          ref:
            status: "planned"
            path: "crates/spec-registry/src/lib.rs"
        audit_ledger:
          mode: "SPEC_REVISIONS_PROVENANCE_LEDGER"
          ref:
            status: "planned"
            path: "crates/spec-registry/src/lib.rs"
      control_plane:
        registered_in_catalog: true
        canary_supported: true
        rollback_version_pointer: "v0.1.0-daemon-genesis"
        memory_limit_mb: 128
        timeout_ms: 15000
    tools:
      - name: "uxui_synthesize_9specs_from_brief"
        effect_class: "IDEMPOTENT_WRITE"
        idempotency: true
        hitl: true
        base_version: true
        mutates_shared_artifact: true
        zod_schema_ref:
          status: "planned"
          path: "packages/shared-types/src/index.ts"
        error_codes:
          - code: "BRIEF_EMPTY"
            retryable: false
            hint: "Provide a non-empty product_brief or seed_repo_path."
          - code: "STALE_FILE"
            retryable: true
            hint: "Refresh project state before re-running 9-Spec synthesis."
        description: "Synthesizes all 9 Foundational Project Context Specs from a raw Product Brief with CONFIRMED/INFERRED/MISSING provenance badges."
    playbook:
      states:
        - "INTAKE_BRIEF"
        - "EXTRACT_KNOWN_FACTS"
        - "TAG_INFERRED_AND_MISSING"
        - "PROPOSE_9SPECS_FOR_REVIEW"
        - "COMMIT_TO_VAULT"
      max_self_heal: 2
      hitl_points:
        - "PROPOSE_9SPECS_FOR_REVIEW"
      timeouts:
        synthesize_ms: 15000
---

# Epic 01 — Agent Surface & Enablement Map (`agent-surface-map.md`)

## 1. Overview (`Axis 2a: Agent Exposure`)
`Epic 01` establishes the foundational data and state substrate (`crates/domain`, `crates/spec-registry`, `crates/harness-daemon`) that powers both the **Human UI Shell** and the **Autonomous Agent Fleet** (`cowok` Orchestrator, `advisor` Meta-Agent / Dr. Eggbot, and single-purpose Execution Bots).

## 2. Component Exposure Matrix

| Component ID | Trust Owner | Exposure | Form | Execution Lane | Key Agent Capabilities |
| :--- | :---: | :---: | :---: | :---: | :--- |
| `crates/domain` | `KERNEL` | `NOT_NEEDED` | `NONE` | N/A | Compile-time Rust $\leftrightarrow$ TypeScript (`ts-rs`) contract definitions. |
| `crates/spec-registry` | `KERNEL` | `AGENT_FACING` | `TOOL` | `LANE_1_DETERMINISTIC` | Zero-token deterministic read/write access (`uxui_read_project_specs`, `uxui_update_project_spec`) with mandatory SHA-256 `StaleFile` OCC. |
| `crates/harness-daemon` | `KERNEL` | `AGENT_PROPOSE_ONLY` | `PLAYBOOK` | `LANE_2_OPERATIONAL_NLP` | Structured 9-Spec synthesis (`uxui_synthesize_9specs_from_brief`) with `CONFIRMED` / `INFERRED` / `MISSING` provenance badges and HITL review card. |
