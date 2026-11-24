import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { DatabaseSync } = require("node:sqlite");

function openMigratedMemoryDb(): InstanceType<typeof DatabaseSync> {
  const migrationsDir = path.resolve(__dirname, "../crates/spec-registry/migrations");
  const db = new DatabaseSync(":memory:");
  db.exec("PRAGMA foreign_keys = OFF; PRAGMA legacy_alter_table = OFF;");

  db.exec(fs.readFileSync(path.join(migrationsDir, "V1__initial_16_tables.sql"), "utf-8"));
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V2__project_boundaries_and_chat_sessions.sql"),
      "utf-8"
    )
  );
  // Mirror pool.rs idempotent tenant_id guard prior to V3 12-step rebuild
  db.exec("ALTER TABLE projects ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';");
  db.exec(fs.readFileSync(path.join(migrationsDir, "V3__flow_workbench_dag.sql"), "utf-8"));
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V4__storyboard_chat_and_execution_engine.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V5__proposals_snapshots_runtime_columns.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V6__epic04_complete_contracts.sql"),
      "utf-8"
    )
  );
  // Mirror pool.rs idempotent V6/V7 column guards prior to V7 12-step rebuild
  db.exec(`
    ALTER TABLE projects ADD COLUMN project_code TEXT;
    ALTER TABLE projects ADD COLUMN trash_deleted_at BIGINT;
    ALTER TABLE projects ADD COLUMN provenance TEXT NOT NULL DEFAULT 'scratch';
    ALTER TABLE chat_sessions ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
    ALTER TABLE chat_sessions ADD COLUMN archived_at BIGINT;
    ALTER TABLE chat_messages ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
    ALTER TABLE node_execution_jobs ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
    ALTER TABLE chat_artifact_snapshots ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
    ALTER TABLE chat_message_node_scopes ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
    ALTER TABLE element_comments ADD COLUMN artifact_sha256 TEXT NOT NULL DEFAULT '';
    ALTER TABLE element_comments ADD COLUMN anchor_kind TEXT NOT NULL DEFAULT 'html_element';
    ALTER TABLE execution_events ADD COLUMN project_seq INTEGER;
    ALTER TABLE execution_events ADD COLUMN composite_scope_key TEXT NOT NULL DEFAULT '';
    ALTER TABLE agent_settings ADD COLUMN permission_mode TEXT NOT NULL DEFAULT 'ask-every-time';
    ALTER TABLE agent_settings ADD COLUMN vault_mode TEXT NOT NULL DEFAULT 'keychain';
    ALTER TABLE agent_settings ADD COLUMN onboarding_done INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE node_chat_sessions ADD COLUMN composite_scope_key TEXT NOT NULL DEFAULT '';
    ALTER TABLE node_chat_sessions ADD COLUMN unread_count INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE node_chat_sessions ADD COLUMN last_message_id TEXT;
  `);
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V7__epic04_cas_seq_and_vault_convergence.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V8__daemon_auth_sessions_and_nonces.sql"),
      "utf-8"
    )
  );
  // Mirror pool.rs idempotent V9 column guards prior to V9 12-step rebuild
  db.exec(`
    ALTER TABLE sdi_intents ADD COLUMN routing_mode TEXT NOT NULL DEFAULT 'direct_edit';
    ALTER TABLE sdi_intents ADD COLUMN gap_questions_json TEXT;
    ALTER TABLE sdi_intents ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE flow_nodes ADD COLUMN portal_group TEXT;
    ALTER TABLE flow_nodes ADD COLUMN shared_app_shell_id TEXT;
    ALTER TABLE flow_nodes ADD COLUMN mini_skeleton_html TEXT;
    ALTER TABLE flow_nodes ADD COLUMN cta_pills_json TEXT NOT NULL DEFAULT '[]';
    ALTER TABLE flow_nodes ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE screen_fixtures ADD COLUMN preset_key TEXT;
    ALTER TABLE screen_fixtures ADD COLUMN sample_rows_json TEXT NOT NULL DEFAULT '[]';
    ALTER TABLE screen_fixtures ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE flow_graphs ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
  `);
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V9__epic05_storyboard_proposals_handoff.sql"),
      "utf-8"
    )
  );
  // Mirror pool.rs idempotent V10 column guards
  db.exec(`
    ALTER TABLE change_proposals ADD COLUMN node_id TEXT;
    ALTER TABLE change_proposals ADD COLUMN batch_id TEXT;
    ALTER TABLE change_proposals ADD COLUMN composite_scope_key TEXT NOT NULL DEFAULT '';
    ALTER TABLE change_proposals ADD COLUMN content_mode TEXT NOT NULL DEFAULT 'full_file';
    ALTER TABLE change_proposals ADD COLUMN base_file_hashes_json TEXT NOT NULL DEFAULT '{}';
    ALTER TABLE change_proposals ADD COLUMN visual_diff_json TEXT NOT NULL DEFAULT '{}';
    ALTER TABLE change_proposals ADD COLUMN gate_receipt_id TEXT;
  `);
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V10__story_05_2_proposals_receipts_tradeoffs.sql"),
      "utf-8"
    )
  );
  // Mirror pool.rs idempotent V11 column guards
  db.exec(`
    ALTER TABLE chat_sessions ADD COLUMN forked_from_session_id TEXT REFERENCES chat_sessions(id) ON DELETE SET NULL;
    ALTER TABLE chat_sessions ADD COLUMN forked_from_message_id TEXT REFERENCES chat_messages(id) ON DELETE SET NULL;
    ALTER TABLE chat_artifact_snapshots ADD COLUMN proposal_id TEXT REFERENCES change_proposals(id) ON DELETE SET NULL;
    ALTER TABLE chat_artifact_snapshots ADD COLUMN parent_snapshot_id TEXT REFERENCES chat_artifact_snapshots(id) ON DELETE SET NULL;
    ALTER TABLE chat_artifact_snapshots ADD COLUMN node_snapshot_json TEXT NOT NULL DEFAULT '{}';
    ALTER TABLE version_snapshots ADD COLUMN proposal_id TEXT REFERENCES change_proposals(id) ON DELETE SET NULL;
    ALTER TABLE version_snapshots ADD COLUMN parent_snapshot_id TEXT REFERENCES version_snapshots(id) ON DELETE SET NULL;
    ALTER TABLE version_snapshots ADD COLUMN chat_artifact_snapshot_id TEXT REFERENCES chat_artifact_snapshots(id) ON DELETE SET NULL;
    ALTER TABLE github_pull_requests ADD COLUMN snapshot_id TEXT REFERENCES version_snapshots(id) ON DELETE SET NULL;
    ALTER TABLE github_pull_requests ADD COLUMN handoff_manifest_sha256 TEXT NOT NULL DEFAULT '';
    ALTER TABLE github_pull_requests ADD COLUMN visual_diff_summary_json TEXT NOT NULL DEFAULT '{}';
  `);
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V11__story_05_3_snapshots_fork_handoff.sql"),
      "utf-8"
    )
  );
  // Mirror pool.rs idempotent V12 column guards prior to V12 12-step rebuild of skill_plugins
  db.exec(`
    ALTER TABLE skill_plugins ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
    ALTER TABLE skill_plugins ADD COLUMN source_type TEXT NOT NULL DEFAULT 'local';
    ALTER TABLE skill_plugins ADD COLUMN upstream_repo_url TEXT;
    ALTER TABLE skill_plugins ADD COLUMN upstream_commit_sha TEXT;
    ALTER TABLE skill_plugins ADD COLUMN cron_schedule TEXT;
    ALTER TABLE skill_plugins ADD COLUMN skill_md_path TEXT NOT NULL DEFAULT 'SKILL.md';
    ALTER TABLE skill_plugins ADD COLUMN plugin_json_path TEXT NOT NULL DEFAULT 'plugin.json';
    ALTER TABLE skill_plugins ADD COLUMN last_synced_at BIGINT;
    ALTER TABLE skill_plugins ADD COLUMN revision BIGINT NOT NULL DEFAULT 1;
    ALTER TABLE skill_plugins ADD COLUMN staging_version TEXT;
    ALTER TABLE skill_plugins ADD COLUMN staging_manifest_json TEXT;
    ALTER TABLE skill_plugins ADD COLUMN staging_diff_reasons_json TEXT;
    ALTER TABLE skill_plugins ADD COLUMN staging_bundle_dir TEXT;
    ALTER TABLE skill_plugins ADD COLUMN staging_package_sha256 TEXT;
    ALTER TABLE skill_plugins ADD COLUMN breaking_change_isolated INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE skill_plugins ADD COLUMN quarantined INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE skill_plugins ADD COLUMN quarantine_reason TEXT;
  `);
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V12__epic08_plugins_learn_cron.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V13__epic08_audit_remediation.sql"),
      "utf-8"
    )
  );
  // Mirror pool.rs apply_v14_columns idempotent column guards prior to V14 index creation
  db.exec(`
    ALTER TABLE gate_reports ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
    ALTER TABLE gate_reports ADD COLUMN correlation_id TEXT NOT NULL DEFAULT '';
    ALTER TABLE gate_reports ADD COLUMN target_id TEXT NOT NULL DEFAULT '';
    ALTER TABLE gate_reports ADD COLUMN action TEXT NOT NULL DEFAULT '';
    ALTER TABLE gate_reports ADD COLUMN revision BIGINT NOT NULL DEFAULT 0;
    ALTER TABLE gate_reports ADD COLUMN fencing_token BIGINT NOT NULL DEFAULT 0;
    ALTER TABLE gate_reports ADD COLUMN reason_code TEXT NOT NULL DEFAULT '';
    ALTER TABLE daemon_sessions ADD COLUMN actor_kind TEXT NOT NULL DEFAULT 'AGENT_HMAC';
  `);
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V14__epic08_party_remediation.sql"),
      "utf-8"
    )
  );
  // Mirror pool.rs apply_v15_columns idempotent column guards prior to V15 index creation
  db.exec(`
    ALTER TABLE presentation_decks ADD COLUMN aspect_ratio TEXT NOT NULL DEFAULT '16:9';
    ALTER TABLE presentation_decks ADD COLUMN deck_protocol_version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE presentation_decks ADD COLUMN deck_mode TEXT NOT NULL DEFAULT 'multi-page';
    ALTER TABLE presentation_decks ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE deck_slides ADD COLUMN slide_uid TEXT;
    ALTER TABLE deck_slides ADD COLUMN speaker_notes TEXT NOT NULL DEFAULT '';
    ALTER TABLE deck_slides ADD COLUMN narrative_arc TEXT NOT NULL DEFAULT '';
    ALTER TABLE deck_slides ADD COLUMN eyebrow_tag TEXT NOT NULL DEFAULT '';
    ALTER TABLE deck_slides ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE wording_slots ADD COLUMN slot_kind TEXT NOT NULL DEFAULT 'text';
    ALTER TABLE wording_slots ADD COLUMN charts_json TEXT NOT NULL DEFAULT '[]';
    ALTER TABLE wording_slots ADD COLUMN safe_styles_json TEXT NOT NULL DEFAULT '{}';
    ALTER TABLE wording_slots ADD COLUMN updated_by TEXT NOT NULL DEFAULT 'human';
    ALTER TABLE wording_slots ADD COLUMN ast_revision INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE change_proposals ADD COLUMN source_comment_id TEXT;
  `);
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V15__epic06_presentation_cms_and_charts.sql"),
      "utf-8"
    )
  );
  // Mirror pool.rs apply_v16_columns idempotent guard prior to V16 SQL
  db.exec("ALTER TABLE chat_messages ADD COLUMN seq_no BIGINT;");
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V16__story_04_6_cas_blobs.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V17__story_02_1_context_telemetry.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V18__story_02_4_hidden_ssot.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V19__story_05_4_html_forge_and_sketch_polish.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V20__story_08_5_de_dr_learn_ledger.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V21__story_09_2_decision_audit_log.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V22__story_05_5_flow_node_substates.sql"),
      "utf-8"
    )
  );
  db.exec(
    fs.readFileSync(
      path.join(migrationsDir, "V23__story_14_1_support_telemetry_outbox.sql"),
      "utf-8"
    )
  );
  db.exec("PRAGMA foreign_keys = ON;");
  return db;
}

const PRISMA_SCALAR_TYPES = new Set([
  "String",
  "Int",
  "BigInt",
  "Boolean",
  "Float",
  "Bytes",
  "DateTime",
  "Decimal",
  "Json",
]);

function parsePrismaModelColumns(schemaText: string, modelName: string): string[] {
  const modelRe = new RegExp(`model\\s+${modelName}\\s+\\{([\\s\\S]*?)\\n\\}`, "m");
  const match = modelRe.exec(schemaText);
  if (!match) throw new Error(`Missing Prisma model ${modelName}`);
  const cols: string[] = [];
  for (const rawLine of match[1].split("\n")) {
    const line = rawLine.trim();
    if (!line || line.startsWith("//") || line.startsWith("@@")) continue;
    const fieldMatch = /^([a-zA-Z_][a-zA-Z0-9_]*)\s+([A-Za-z0-9_]+)([\?\[]*)/.exec(line);
    if (!fieldMatch) continue;
    const [, fieldName, fieldType, suffix] = fieldMatch;
    if (!PRISMA_SCALAR_TYPES.has(fieldType) || suffix.includes("[")) continue;
    const mapMatch = /@map\("([^"]+)"\)/.exec(line);
    cols.push(mapMatch ? mapMatch[1] : fieldName);
  }
  return cols.sort();
}

describe("P1 Persistence Contract & SQLite WAL V1-V15 Mirror Verification (ADR-091)", () => {
  const schemaPath = path.resolve(__dirname, "schema.prisma");
  const migrationsDir = path.resolve(__dirname, "../crates/spec-registry/migrations");

  it("UT-DB-01: verifies ADR-091 declarative mirror has zero prisma/migrations or root dev.db", () => {
    expect(fs.existsSync(schemaPath)).toBe(true);
    expect(fs.existsSync(path.resolve(__dirname, "migrations"))).toBe(false);
    expect(fs.existsSync(path.resolve(__dirname, "../dev.db"))).toBe(false);

    const content = fs.readFileSync(schemaPath, "utf-8");
    const mappedTables = [...content.matchAll(/@@map\("([^"]+)"\)/g)].map((m) => m[1]);
    expect(mappedTables).toContain("projects");
    expect(mappedTables).toContain("project_specs");
    expect(mappedTables).toContain("spec_revisions");
    expect(mappedTables).toContain("sdi_intents");
    expect(mappedTables).toContain("daemon_sessions");
    expect(mappedTables).toContain("daemon_replay_nonces");
    expect(mappedTables).toContain("workspace_sync_intents");
    expect(mappedTables).toContain("idempotency_records");
    expect(mappedTables).toContain("proposal_target_nodes");
    expect(mappedTables).toContain("proposal_gate_receipts");
    expect(mappedTables).toContain("proposal_tradeoffs");
    expect(mappedTables).toContain("knowledge_cards");
    expect(mappedTables).toContain("learn_consumed_nonces");
    expect(mappedTables).toContain("plugin_cron_schedules");
    expect(mappedTables).toContain("cas_blobs");
    expect(mappedTables).toContain("cas_attachments");
    expect(mappedTables).toContain("tenant_cas_blob_usage");
    expect(mappedTables).toContain("cas_upload_ingress");
    expect(mappedTables).toContain("cas_command_idempotency");
    expect(mappedTables).toContain("vault_command_idempotency");
    expect(mappedTables).toContain("context_assembly_telemetry");
    expect(mappedTables).toContain("component_patterns");
    expect(mappedTables).toContain("app_shell_locks");
    expect(mappedTables).toContain("screen_artifact_patches");
    expect(mappedTables).toContain("sdi_clarification_sessions");
    expect(mappedTables).toContain("user_skill_registry");
    expect(mappedTables).toContain("gate_failure_ledger");
    expect(mappedTables).toContain("decision_audit_log");
    expect(mappedTables).toContain("flow_node_substates");
    expect(mappedTables).toContain("telemetry_outbox");
    expect(mappedTables).toContain("local_support_tickets");
    expect(mappedTables).toContain("telemetry_preferences");
    expect(mappedTables).toHaveLength(67);
  });

  it("UT-DB-02: executes V1-V23 Rust migrations cleanly and matches 1:1 with schema.prisma @@map tables", () => {
    expect(fs.existsSync(migrationsDir)).toBe(true);
    const db = openMigratedMemoryDb();

    const tables = db
      .prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
      )
      .all() as { name: string }[];
    const sqliteTableNames = tables.map((t) => t.name);

    const schemaContent = fs.readFileSync(schemaPath, "utf-8");
    const prismaTableNames = [...schemaContent.matchAll(/@@map\("([^"]+)"\)/g)]
      .map((m) => m[1])
      .sort();

    expect(sqliteTableNames).toEqual(prismaTableNames);
  });

  it("UT-DB-03: enforces unique constraints and cascade deletion on projects -> project_specs -> spec_revisions", () => {
    const db = openMigratedMemoryDb();

    const projectId = "123e4567-e89b-12d3-a456-426614174000";
    const specId = "223e4567-e89b-12d3-a456-426614174000";
    const revId = "323e4567-e89b-12d3-a456-426614174000";
    const now = 1730500000;

    db.prepare(
      "INSERT INTO projects (id, name, slug, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
    ).run(projectId, "Cowok-Design Platform", "cowok-design", now, now);

    expect(() => {
      db.prepare(
        "INSERT INTO projects (id, name, slug, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
      ).run("other-id", "Duplicate Slug", "cowok-design", now, now);
    }).toThrow();

    db.prepare(
      "INSERT INTO project_specs (id, project_id, spec_key, status, updated_at) VALUES (?, ?, ?, ?, ?)"
    ).run(specId, projectId, "01-product-overview.md", "CONFIRMED", now);

    expect(() => {
      db.prepare(
        "INSERT INTO project_specs (id, project_id, spec_key, status, updated_at) VALUES (?, ?, ?, ?, ?)"
      ).run("other-spec-id", projectId, "01-product-overview.md", "INFERRED", now);
    }).toThrow();

    db.prepare(
      "INSERT INTO spec_revisions (id, spec_id, project_id, spec_key, revision_number, operation_id, content_hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    ).run(revId, specId, projectId, "01-product-overview.md", 1, "op-1", "sha256-hash-abc", now);

    expect(() => {
      db.prepare(
        "INSERT INTO spec_revisions (id, spec_id, project_id, spec_key, revision_number, operation_id, content_hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
      ).run("other-rev-id", specId, projectId, "01-product-overview.md", 1, "op-2", "sha256-hash-diff", now);
    }).toThrow();

    db.prepare("DELETE FROM projects WHERE id = ?").run(projectId);

    const remainingSpecs = db.prepare("SELECT COUNT(*) as count FROM project_specs").get() as {
      count: number;
    };
    const remainingRevs = db.prepare("SELECT COUNT(*) as count FROM spec_revisions").get() as {
      count: number;
    };
    expect(remainingSpecs.count).toBe(0);
    expect(remainingRevs.count).toBe(0);
  });

  it("UT-DB-04: verifies Story 03.1 constraint engine DDL and unique rule_code per profile", () => {
    const db = openMigratedMemoryDb();
    const now = Date.now();
    db.prepare(
      "INSERT INTO projects (id, name, slug, created_at, updated_at) VALUES ('p1', 'P1', 'p1', ?, ?)"
    ).run(now, now);
    db.prepare(
      "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, updated_at) VALUES ('cp1', 'p1', 'DEFAULT', 'CSS_MODULES', 'src/styles/tokens.css', 'icons/icon-manifest.json', '[]', '{}', ?)"
    ).run(now);
    db.prepare(
      "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at) VALUES ('r1', 'cp1', 'BAN_TAILWIND', 'BLOCKER', '{}', 1, ?)"
    ).run(now);

    expect(() => {
      db.prepare(
        "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at) VALUES ('r2', 'cp1', 'BAN_TAILWIND', 'BLOCKER', '{}', 1, ?)"
      ).run(now);
    }).toThrow();
  });

  it("UT-DB-05: verifies Story 04.1 flow_graphs -> flow_nodes -> flow_edges cascade deletion", () => {
    const db = openMigratedMemoryDb();
    const now = Date.now();

    db.prepare(
      "INSERT INTO projects (id, tenant_id, name, slug, created_at, updated_at) VALUES ('proj-1', 'tenant-1', 'Proj 1', 'proj-1', ?, ?)"
    ).run(now, now);

    db.prepare(
      "INSERT INTO flow_graphs (id, tenant_id, project_id, portal_key, title, updated_at) VALUES ('g1', 'tenant-1', 'proj-1', 'default', 'Main Flow', ?)"
    ).run(now);

    db.prepare(
      "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, route_path, created_at, updated_at) VALUES ('node-1', 'g1', 'tenant-1', 'proj-1', 'default', 'home', 'Home Screen', '/', ?, ?)"
    ).run(now, now);

    db.prepare(
      "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, route_path, created_at, updated_at) VALUES ('node-2', 'g1', 'tenant-1', 'proj-1', 'default', 'settings', 'Settings Screen', '/settings', ?, ?)"
    ).run(now, now);

    db.prepare(
      "INSERT INTO flow_edges (id, graph_id, tenant_id, project_id, source_node_id, target_node_id, trigger_event, created_at, updated_at) VALUES ('edge-1', 'g1', 'tenant-1', 'proj-1', 'node-1', 'node-2', 'onClick', ?, ?)"
    ).run(now, now);

    const edgeCount = db.prepare("SELECT COUNT(*) as count FROM flow_edges").get() as { count: number };
    expect(edgeCount.count).toBe(1);

    db.prepare("DELETE FROM flow_nodes WHERE id = 'node-1'").run();
    const remainingEdges = db.prepare("SELECT COUNT(*) as count FROM flow_edges").get() as { count: number };
    expect(remainingEdges.count).toBe(0);
  });

  it("UT-DB-06: verifies Story 05.3 V11 & Story 08.4 V12-V14 columns, indexes, and Prisma partialIndexes parity", () => {
    const db = openMigratedMemoryDb();
    const schemaText = fs.readFileSync(path.resolve(__dirname, "schema.prisma"), "utf8");
    const sqliteIndexes = db
      .prepare(
        "SELECT name, sql FROM sqlite_master WHERE type = 'index' AND tbl_name IN ('skill_plugins', 'knowledge_cards', 'plugin_cron_schedules', 'gate_reports', 'chat_sessions', 'chat_artifact_snapshots', 'version_snapshots', 'github_pull_requests')"
      )
      .all() as Array<{ name: string; sql: string | null }>;
    const byName = new Map(sqliteIndexes.map((idx) => [idx.name, idx.sql ?? ""]));

    expect(byName.has("uq_skill_plugins_scope_slug_ver")).toBe(true);
    expect(byName.has("uq_skill_plugins_active_slug")).toBe(true);
    expect(byName.get("uq_skill_plugins_active_slug")).toContain("WHERE lifecycle_state = 'ACTIVE'");
    expect(byName.has("uq_knowledge_cards_idempotency")).toBe(true);
    expect(byName.get("uq_knowledge_cards_idempotency")).toContain("WHERE idempotency_key IS NOT NULL");
    expect(byName.has("uq_knowledge_cards_active_hash")).toBe(true);
    expect(byName.get("uq_knowledge_cards_active_hash")).toContain("WHERE status != 'REJECTED'");
    expect(byName.has("uq_plugin_cron_schedules_scope_plugin")).toBe(true);
    expect(byName.has("idx_gate_reports_project_gate_target")).toBe(true);
    expect(byName.has("skill_plugins_project_slug_ver_key")).toBe(false);
    for (const expectedIdx of [
      "idx_chat_sessions_forked_from",
      "idx_artifact_snapshots_proposal",
      "idx_artifact_snapshots_parent",
      "idx_version_snapshots_proposal",
      "idx_version_snapshots_parent",
      "idx_github_prs_snapshot",
    ]) {
      expect(byName.has(expectedIdx)).toBe(true);
    }

    expect(schemaText).toContain('previewFeatures = ["partialIndexes"]');
    expect(schemaText).toContain('map: "uq_skill_plugins_active_slug"');
    expect(schemaText).toContain('map: "uq_knowledge_cards_idempotency"');
    expect(schemaText).toContain('map: "uq_knowledge_cards_active_hash"');
    expect(schemaText).toContain('map: "uq_plugin_cron_schedules_scope_plugin"');
    expect(schemaText).toContain('map: "idx_gate_reports_project_gate_target"');

    // Column-level PRAGMA table_info parity across V12-V14 Epic 08 models
    const tableModelPairs: Array<[string, string]> = [
      ["skill_plugins", "SkillPlugin"],
      ["knowledge_cards", "KnowledgeCard"],
      ["learn_consumed_nonces", "LearnConsumedNonce"],
      ["plugin_cron_schedules", "PluginCronSchedule"],
      ["gate_reports", "GateReport"],
      ["daemon_sessions", "DaemonSession"],
    ];
    for (const [tableName, modelName] of tableModelPairs) {
      const pragmaRows = db.prepare(`PRAGMA table_info(${tableName})`).all() as Array<{ name: string }>;
      const sqliteCols = pragmaRows.map((r) => r.name).sort();
      const prismaCols = parsePrismaModelColumns(schemaText, modelName);
      expect(sqliteCols, `Column parity for ${tableName} <-> ${modelName}`).toEqual(prismaCols);
    }

    // Verify Story 05.3 V11 additive columns exist in both SQLite and schema.prisma
    for (const [table, cols] of [
      ["chat_sessions", ["forked_from_session_id", "forked_from_message_id"]],
      ["chat_artifact_snapshots", ["proposal_id", "parent_snapshot_id", "node_snapshot_json"]],
      ["version_snapshots", ["proposal_id", "parent_snapshot_id", "chat_artifact_snapshot_id"]],
      ["github_pull_requests", ["snapshot_id", "handoff_manifest_sha256", "visual_diff_summary_json"]],
    ] as const) {
      const tableCols = (db.prepare(`PRAGMA table_info(${table})`).all() as { name: string }[]).map(
        (r) => r.name
      );
      for (const col of cols) {
        expect(tableCols).toContain(col);
        expect(schemaText).toContain(`@map("${col}")`);
      }
    }

    const now = Date.now();
    db.prepare(
      "INSERT INTO projects (id, tenant_id, name, slug, created_at, updated_at) VALUES ('p-084', 'tenant-a', 'Proj 084', 'p-084', ?, ?)"
    ).run(now, now);
    db.prepare(
      "INSERT INTO skill_plugins (id, tenant_id, project_id, plugin_slug, version, lifecycle_state, source_type, source_repo_url, updated_at) VALUES ('sp-1', 'tenant-a', 'p-084', 'tokens-skill', '1.0.0', 'ACTIVE', 'local', 'https://github.com/org/repo', ?)"
    ).run(now);
    db.prepare(
      "INSERT INTO skill_plugins (id, tenant_id, project_id, plugin_slug, version, lifecycle_state, source_type, source_repo_url, updated_at) VALUES ('sp-2', 'tenant-a', 'p-084', 'tokens-skill', '2.0.0', 'INSTALLED', 'local', 'https://github.com/org/repo', ?)"
    ).run(now);
    expect(() => {
      db.prepare(
        "INSERT INTO skill_plugins (id, tenant_id, project_id, plugin_slug, version, lifecycle_state, source_type, source_repo_url, updated_at) VALUES ('sp-3', 'tenant-a', 'p-084', 'tokens-skill', '3.0.0', 'ACTIVE', 'local', 'https://github.com/org/repo', ?)"
      ).run(now);
    }).toThrow();
    db.prepare(
      "INSERT INTO skill_plugins (id, tenant_id, project_id, plugin_slug, version, lifecycle_state, source_type, source_repo_url, updated_at) VALUES ('sp-4', 'tenant-b', 'p-084', 'tokens-skill', '1.0.0', 'ACTIVE', 'local', 'https://github.com/org/repo', ?)"
    ).run(now);
  });

  it("UT-DB-07: verifies Story 06.1 / 05.2 V15 additive columns, slide_uid unique index, and Prisma parity", () => {
    const db = openMigratedMemoryDb();
    const schemaContent = fs.readFileSync(schemaPath, "utf-8");

    for (const [table, cols] of [
      ["presentation_decks", ["aspect_ratio", "deck_protocol_version", "deck_mode", "revision"]],
      ["deck_slides", ["slide_uid", "speaker_notes", "narrative_arc", "eyebrow_tag", "revision"]],
      ["wording_slots", ["slot_kind", "charts_json", "safe_styles_json", "updated_by", "ast_revision"]],
      ["change_proposals", ["source_comment_id"]],
    ] as const) {
      const tableCols = (db.prepare(`PRAGMA table_info(${table})`).all() as { name: string }[]).map(
        (r) => r.name
      );
      for (const col of cols) {
        expect(tableCols).toContain(col);
        if (col.includes("_")) {
          expect(schemaContent).toContain(`@map("${col}")`);
        }
      }
    }

    const idxNames = (
      db
        .prepare("SELECT name FROM sqlite_master WHERE type='index'")
        .all() as { name: string }[]
    ).map((r) => r.name);
    expect(idxNames).toContain("deck_slides_slide_uid_key");
  });

  it("UT-DB-08: verifies Story 04.6 V16 chat seq_no, CAS/quota/idempotency separation, FK cascade, and Prisma parity", () => {
    const db = openMigratedMemoryDb();
    const schemaText = fs.readFileSync(schemaPath, "utf-8");

    const chatCols = (db.prepare("PRAGMA table_info(chat_messages)").all() as Array<{ name: string }>).map(
      (r) => r.name
    );
    expect(chatCols).toContain("seq_no");
    expect(schemaText).toContain('@map("seq_no")');

    const tableModelPairs: Array<[string, string]> = [
      ["cas_blobs", "CasBlob"],
      ["cas_attachments", "CasAttachment"],
      ["tenant_cas_blob_usage", "TenantCasBlobUsage"],
      ["cas_upload_ingress", "CasUploadIngress"],
      ["cas_command_idempotency", "CasCommandIdempotency"],
      ["vault_command_idempotency", "VaultCommandIdempotency"],
    ];
    for (const [tableName, modelName] of tableModelPairs) {
      const pragmaRows = db.prepare(`PRAGMA table_info(${tableName})`).all() as Array<{ name: string }>;
      const sqliteCols = pragmaRows.map((r) => r.name).sort();
      const prismaCols = parsePrismaModelColumns(schemaText, modelName);
      expect(sqliteCols, `Column parity for ${tableName} <-> ${modelName}`).toEqual(prismaCols);
    }

    const attachmentCols = (db.prepare("PRAGMA table_info(cas_attachments)").all() as Array<{ name: string }>).map(
      (r) => r.name
    );
    const ingressCols = (db.prepare("PRAGMA table_info(cas_upload_ingress)").all() as Array<{ name: string }>).map(
      (r) => r.name
    );
    expect(attachmentCols).not.toContain("expires_at");
    expect(ingressCols).not.toContain("expires_at");

    const fks = db.prepare("PRAGMA foreign_key_list(cas_command_idempotency)").all() as Array<{
      table: string;
      from: string;
      on_delete: string;
    }>;
    expect(
      fks.some(
        (fk) =>
          fk.table === "cas_attachments" &&
          fk.from === "attachment_id" &&
          fk.on_delete.toUpperCase() === "CASCADE"
      )
    ).toBe(true);
  });

  it("UT-DB-09: verifies Story 02.1 V17 context_assembly_telemetry Prisma parity, indexes, and project cascade", () => {
    const db = openMigratedMemoryDb();
    const schemaText = fs.readFileSync(schemaPath, "utf-8");

    const pragmaRows = db
      .prepare("PRAGMA table_info(context_assembly_telemetry)")
      .all() as Array<{ name: string }>;
    const sqliteCols = pragmaRows.map((r) => r.name).sort();
    const prismaCols = parsePrismaModelColumns(schemaText, "ContextAssemblyTelemetry");
    expect(sqliteCols).toEqual(prismaCols);

    const fks = db
      .prepare("PRAGMA foreign_key_list(context_assembly_telemetry)")
      .all() as Array<{ table: string; from: string; on_delete: string }>;
    expect(
      fks.some(
        (fk) =>
          fk.table === "projects" &&
          fk.from === "project_id" &&
          fk.on_delete.toUpperCase() === "CASCADE"
      )
    ).toBe(true);

    const idxRows = db
      .prepare("PRAGMA index_list(context_assembly_telemetry)")
      .all() as Array<{ name: string }>;
    const idxNames = idxRows.map((r) => r.name);
    expect(idxNames).toContain("idx_ctx_telemetry_proj_portal");
    expect(idxNames).toContain("idx_ctx_telemetry_tenant_proj");
    expect(idxNames).toContain("uq_ctx_telemetry_idem");
  });

  it("UT-DB-10: verifies Story 02.4 V18 component_patterns & app_shell_locks Prisma parity, composite FK cascade, and indexes", () => {
    const db = openMigratedMemoryDb();
    const schemaText = fs.readFileSync(schemaPath, "utf-8");

    for (const [modelName, tableName] of [
      ["ComponentPattern", "component_patterns"],
      ["AppShellLock", "app_shell_locks"],
    ] as const) {
      const pragmaRows = db
        .prepare(`PRAGMA table_info(${tableName})`)
        .all() as Array<{ name: string }>;
      const sqliteCols = pragmaRows.map((r) => r.name).sort();
      const prismaCols = parsePrismaModelColumns(schemaText, modelName);
      expect(sqliteCols).toEqual(prismaCols);

      const fks = db
        .prepare(`PRAGMA foreign_key_list(${tableName})`)
        .all() as Array<{ table: string; from: string; to: string; on_delete: string }>;
      const projFks = fks.filter(
        (fk) => fk.table === "projects" && fk.on_delete.toUpperCase() === "CASCADE"
      );
      expect(projFks.some((fk) => fk.from === "project_id" && fk.to === "id")).toBe(true);
      expect(projFks.some((fk) => fk.from === "tenant_id" && fk.to === "tenant_id")).toBe(true);
    }

    const cpIdx = (
      db.prepare("PRAGMA index_list(component_patterns)").all() as Array<{ name: string }>
    ).map((r) => r.name);
    expect(cpIdx).toContain("idx_component_patterns_proj_portal");

    const lockIdx = (
      db.prepare("PRAGMA index_list(app_shell_locks)").all() as Array<{ name: string }>
    ).map((r) => r.name);
    expect(lockIdx).toContain("idx_app_shell_locks_proj_portal");

    const projIdx = (
      db.prepare("PRAGMA index_list(projects)").all() as Array<{ name: string }>
    ).map((r) => r.name);
    expect(projIdx).toContain("uq_projects_id_tenant");
  });

  it("UT-DB-11: verifies Story 08.5 V20 user_skill_registry & gate_failure_ledger Prisma parity, CHECK constraints, and cascade", () => {
    const db = openMigratedMemoryDb();
    const schemaText = fs.readFileSync(schemaPath, "utf-8");

    for (const [modelName, tableName] of [
      ["UserSkillRegistry", "user_skill_registry"],
      ["GateFailureLedger", "gate_failure_ledger"],
    ] as const) {
      const pragmaRows = db
        .prepare(`PRAGMA table_info(${tableName})`)
        .all() as Array<{ name: string }>;
      const sqliteCols = pragmaRows.map((r) => r.name).sort();
      const prismaCols = parsePrismaModelColumns(schemaText, modelName);
      expect(sqliteCols, `Column parity for ${tableName} <-> ${modelName}`).toEqual(prismaCols);
    }

    const now = Date.now();
    db.prepare(
      "INSERT INTO projects (id, tenant_id, name, slug, created_at, updated_at) VALUES ('p-085', 'tenant-a', 'Proj 085', 'p-085', ?, ?)"
    ).run(now, now);

    // Header tokens > 100 must fail CHECK constraint
    expect(() => {
      db.prepare(
        "INSERT INTO user_skill_registry (id, tenant_id, project_id, skill_slug, header_tokens, security_scan_verdict, license_classification, skill_md_path, created_at) VALUES ('usr-bad', 'tenant-a', 'p-085', 'bad-skill', 101, 'PASS', 'ADOPT_PRIOR_ART', 'skills/bad/SKILL.md', ?)"
      ).run(now);
    }).toThrow();

    db.prepare(
      "INSERT INTO user_skill_registry (id, tenant_id, project_id, skill_slug, header_tokens, security_scan_verdict, license_classification, skill_md_path, created_at) VALUES ('usr-1', 'tenant-a', 'p-085', 'good-skill', 42, 'PASS', 'ADOPT_PRIOR_ART', 'skills/good/SKILL.md', ?)"
    ).run(now);

    db.prepare(
      "INSERT INTO gate_failure_ledger (id, tenant_id, project_id, rule_id, fingerprint_sha256, sample_message, hit_count, slpr_score, status, last_seen_at) VALUES ('gfl-1', 'tenant-a', 'p-085', 'TELLS-001', 'abc123', 'AI slop detected', 3, 0.96, 'PROPOSED', ?)"
    ).run(now);

    // Project delete cascades cleanly
    db.prepare("DELETE FROM projects WHERE id = 'p-085'").run();
    const skillCount = (
      db.prepare("SELECT COUNT(*) as c FROM user_skill_registry").get() as { c: number }
    ).c;
    const ledgerCount = (
      db.prepare("SELECT COUNT(*) as c FROM gate_failure_ledger").get() as { c: number }
    ).c;
    expect(skillCount).toBe(0);
    expect(ledgerCount).toBe(0);
  });

  it("UT-DB-12: verifies Story 09.2 V21 decision_audit_log Prisma parity, CHECK constraints, index, and cascade", () => {
    const db = openMigratedMemoryDb();
    const schemaText = fs.readFileSync(schemaPath, "utf-8");

    const pragmaRows = db
      .prepare("PRAGMA table_info(decision_audit_log)")
      .all() as Array<{ name: string }>;
    const sqliteCols = pragmaRows.map((r) => r.name).sort();
    const prismaCols = parsePrismaModelColumns(schemaText, "DecisionAuditLog");
    expect(sqliteCols).toEqual(prismaCols);

    const idxNames = (
      db.prepare("PRAGMA index_list(decision_audit_log)").all() as Array<{ name: string }>
    ).map((r) => r.name);
    expect(idxNames).toContain("idx_decision_audit_log_tenant_project");

    const now = Date.now();
    db.prepare(
      "INSERT INTO projects (id, tenant_id, name, slug, created_at, updated_at) VALUES ('p-092', 'tenant-a', 'Proj 092', 'p-092', ?, ?)"
    ).run(now, now);

    // Invalid attempt_number > 3 must fail CHECK constraint
    expect(() => {
      db.prepare(
        "INSERT INTO decision_audit_log (id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind, verdict_label, attempt_number, breaker_tripped, latency_ms, created_at) VALUES ('dal-bad', 'tenant-a', 'p-092', 'DECISION_1', 'LOCAL_FTS5_BM25', 'ExactMatch', 'ACCEPT_ARTIFACT', 4, 0, 2, ?)"
      ).run(now);
    }).toThrow();

    // Invalid confidence_kind must fail CHECK constraint
    expect(() => {
      db.prepare(
        "INSERT INTO decision_audit_log (id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind, verdict_label, attempt_number, breaker_tripped, latency_ms, created_at) VALUES ('dal-bad-conf', 'tenant-a', 'p-092', 'DECISION_1', 'LOCAL_FTS5_BM25', 'RawFloat', 'ACCEPT_ARTIFACT', 1, 0, 2, ?)"
      ).run(now);
    }).toThrow();

    db.prepare(
      "INSERT INTO decision_audit_log (id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind, verdict_label, rubric_swap_consistent, attempt_number, breaker_tripped, stale_file_sha256, latency_ms, created_at) VALUES ('dal-1', 'tenant-a', 'p-092', 'RUBRIC_SHIELD', 'LOCAL_FTS5_BM25', 'ExactMatch', 'CANDIDATE_X', 1, 1, 0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3, ?)"
    ).run(now);

    // Project deletion cascades cleanly
    db.prepare("DELETE FROM projects WHERE id = 'p-092'").run();
    const auditCount = (
      db.prepare("SELECT COUNT(*) as c FROM decision_audit_log").get() as { c: number }
    ).c;
    expect(auditCount).toBe(0);
  });
});
