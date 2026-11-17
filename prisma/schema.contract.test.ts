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
  // Mirror pool.rs idempotent V9 column guards prior to V9 12-step rebuild of skill_plugins
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
      path.join(migrationsDir, "V9__epic08_plugins_learn_cron.sql"),
      "utf-8"
    )
  );
  db.exec("PRAGMA foreign_keys = ON;");
  return db;
}

describe("P1 Persistence Contract & SQLite WAL V1-V9 Mirror Verification (ADR-091)", () => {
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
    expect(mappedTables).toContain("knowledge_cards");
    expect(mappedTables).toContain("learn_consumed_nonces");
    expect(mappedTables).toContain("plugin_cron_schedules");
    expect(mappedTables).toHaveLength(45);
  });

  it("UT-DB-02: executes V1-V9 Rust migrations cleanly and matches 1:1 with schema.prisma @@map tables", () => {
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

  it("UT-DB-06: verifies Story 08.4 V9 SQLite index parity with Prisma partialIndexes metadata", () => {
    const db = openMigratedMemoryDb();
    const schemaText = fs.readFileSync(path.resolve(__dirname, "schema.prisma"), "utf8");
    const sqliteIndexes = db
      .prepare(
        "SELECT name, sql FROM sqlite_master WHERE type = 'index' AND tbl_name IN ('skill_plugins', 'knowledge_cards', 'plugin_cron_schedules')"
      )
      .all() as Array<{ name: string; sql: string | null }>;
    const byName = new Map(sqliteIndexes.map((idx) => [idx.name, idx.sql ?? ""]));

    expect(byName.has("uq_skill_plugins_scope_slug_ver")).toBe(true);
    expect(byName.has("uq_skill_plugins_active_slug")).toBe(true);
    expect(byName.get("uq_skill_plugins_active_slug")).toContain("WHERE lifecycle_state = 'ACTIVE'");
    expect(byName.has("uq_knowledge_cards_idempotency")).toBe(true);
    expect(byName.get("uq_knowledge_cards_idempotency")).toContain("WHERE idempotency_key IS NOT NULL");
    expect(byName.has("uq_plugin_cron_schedules_scope_plugin")).toBe(true);
    expect(byName.has("skill_plugins_project_slug_ver_key")).toBe(false);

    expect(schemaText).toContain('previewFeatures = ["partialIndexes"]');
    expect(schemaText).toContain('map: "uq_skill_plugins_active_slug"');
    expect(schemaText).toContain('map: "uq_knowledge_cards_idempotency"');
    expect(schemaText).toContain('map: "uq_plugin_cron_schedules_scope_plugin"');

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
});
