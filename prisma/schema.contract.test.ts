import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { DatabaseSync } = require("node:sqlite");

describe("P1 Persistence Contract & SQLite DDL Verification", () => {
  const schemaPath = path.resolve(__dirname, "schema.prisma");
  const migrationPath = path.resolve(
    __dirname,
    "migrations/20261102000000_init_story_01_1/migration.sql"
  );

  it("UT-DB-01: verifies schema.prisma defines exactly 3 P1 models", () => {
    expect(fs.existsSync(schemaPath)).toBe(true);
    const content = fs.readFileSync(schemaPath, "utf-8");

    expect(content).toContain("model Project");
    expect(content).toContain("model ProjectSpec");
    expect(content).toContain("model SpecRevision");
    expect(content).toContain('@@map("projects")');
    expect(content).toContain('@@map("project_specs")');
    expect(content).toContain('@@map("spec_revisions")');
  });

  it("UT-DB-02: executes migration DDL on clean SQLite DB and validates table schema", () => {
    expect(fs.existsSync(migrationPath)).toBe(true);
    const sql = fs.readFileSync(migrationPath, "utf-8");

    const db = new DatabaseSync(":memory:");
    db.exec(sql);

    // Verify tables exist
    const tables = db
      .prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('projects', 'project_specs', 'spec_revisions') ORDER BY name"
      )
      .all() as { name: string }[];

    const tableNames = tables.map((t) => t.name);
    expect(tableNames).toEqual(["project_specs", "projects", "spec_revisions"]);
  });

  it("UT-DB-03: enforces unique constraints and foreign key relationships", () => {
    const sql = fs.readFileSync(migrationPath, "utf-8");
    const db = new DatabaseSync(":memory:");
    db.exec("PRAGMA foreign_keys = ON;");
    db.exec(sql);

    const projectId = "123e4567-e89b-12d3-a456-426614174000";
    const specId = "223e4567-e89b-12d3-a456-426614174000";
    const revId = "323e4567-e89b-12d3-a456-426614174000";
    const now = 1730500000;

    // 1. Insert Project
    db.prepare(
      "INSERT INTO projects (id, name, slug, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
    ).run(projectId, "Cowok-Design Platform", "cowok-design", now, now);

    // Duplicate project slug must throw unique constraint violation
    expect(() => {
      db.prepare(
        "INSERT INTO projects (id, name, slug, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
      ).run("other-id", "Duplicate Slug", "cowok-design", now, now);
    }).toThrow();

    // 2. Insert ProjectSpec
    db.prepare(
      "INSERT INTO project_specs (id, project_id, spec_key, status, updated_at) VALUES (?, ?, ?, ?, ?)"
    ).run(specId, projectId, "01-product-overview.md", "CONFIRMED", now);

    // Duplicate (project_id, spec_key) must throw unique violation
    expect(() => {
      db.prepare(
        "INSERT INTO project_specs (id, project_id, spec_key, status, updated_at) VALUES (?, ?, ?, ?, ?)"
      ).run("other-spec-id", projectId, "01-product-overview.md", "INFERRED", now);
    }).toThrow();

    // 3. Insert SpecRevision
    db.prepare(
      "INSERT INTO spec_revisions (id, spec_id, revision_number, content_hash, created_at) VALUES (?, ?, ?, ?, ?)"
    ).run(revId, specId, 1, "sha256-hash-abc", now);

    // Duplicate (spec_id, revision_number) must throw unique violation
    expect(() => {
      db.prepare(
        "INSERT INTO spec_revisions (id, spec_id, revision_number, content_hash, created_at) VALUES (?, ?, ?, ?, ?)"
      ).run("other-rev-id", specId, 1, "sha256-hash-diff", now);
    }).toThrow();

    // 4. Cascade Delete
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

  it("UT-DB-04: verifies schema.prisma defines ConstraintProfile and ConstraintRule models", () => {
    const content = fs.readFileSync(schemaPath, "utf-8");
    expect(content).toContain("model ConstraintProfile");
    expect(content).toContain("model ConstraintRule");
    expect(content).toContain('@@map("constraint_profiles")');
    expect(content).toContain('@@map("constraint_rules")');
  });

  it("UT-DB-05: executes Story 03.1 DDL and verifies constraint profiles and rules", () => {
    const migration31 = path.resolve(
      __dirname,
      "migrations/20261105000000_init_constraint_engine/migration.sql"
    );
    expect(fs.existsSync(migration31)).toBe(true);
    const sql = fs.readFileSync(migration31, "utf-8");

    const db = new DatabaseSync(":memory:");
    db.exec("PRAGMA foreign_keys = ON;");
    // Run initial migration first
    db.exec(fs.readFileSync(migrationPath, "utf-8"));
    db.exec(sql);

    const now = Date.now();
    db.prepare("INSERT INTO projects (id, name, slug, created_at, updated_at) VALUES ('p1', 'P1', 'p1', ?, ?)").run(now, now);
    db.prepare("INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, updated_at) VALUES ('cp1', 'p1', 'DEFAULT', 'CSS_MODULES', 'src/styles/tokens.css', 'icons/icon-manifest.json', '[]', '{}', ?)").run(now);
    db.prepare("INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at) VALUES ('r1', 'cp1', 'BAN_TAILWIND', 'BLOCKER', '{}', 1, ?)").run(now);

    // Duplicate (profile_id, rule_code) must throw
    expect(() => {
      db.prepare("INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at) VALUES ('r2', 'cp1', 'BAN_TAILWIND', 'BLOCKER', '{}', 1, ?)").run(now);
    }).toThrow();
  });

  it("UT-DB-06: verifies schema.prisma defines FlowNode and FlowTransition models", () => {
    const content = fs.readFileSync(schemaPath, "utf-8");
    expect(content).toContain("model FlowNode");
    expect(content).toContain("model FlowTransition");
    expect(content).toContain('@@map("flow_nodes")');
    expect(content).toContain('@@map("flow_edges")');
  });

  it("UT-DB-07: executes Story 04.1 DDL and verifies node and transition cascade deletion", () => {
    const migration41 = path.resolve(
      __dirname,
      "migrations/20261107000000_init_flow_workbench/migration.sql"
    );
    expect(fs.existsSync(migration41)).toBe(true);
    const sql = fs.readFileSync(migration41, "utf-8");

    const db = new DatabaseSync(":memory:");
    db.exec("PRAGMA foreign_keys = ON;");
    db.exec(sql);

    const now = Date.now();
    // Insert 2 nodes
    db.prepare(`
      INSERT INTO flow_nodes (id, tenant_id, project_id, portal_id, screen_key, title, route_path, created_at, updated_at)
      VALUES ('node-1', 'tenant-1', 'proj-1', 'default', 'home', 'Home Screen', '/', ?, ?)
    `).run(now, now);

    db.prepare(`
      INSERT INTO flow_nodes (id, tenant_id, project_id, portal_id, screen_key, title, route_path, created_at, updated_at)
      VALUES ('node-2', 'tenant-1', 'proj-1', 'default', 'settings', 'Settings Screen', '/settings', ?, ?)
    `).run(now, now);

    // Insert transition from node-1 to node-2
    db.prepare(`
      INSERT INTO flow_transitions (id, tenant_id, project_id, source_node_id, target_node_id, trigger_event, created_at, updated_at)
      VALUES ('trans-1', 'tenant-1', 'proj-1', 'node-1', 'node-2', 'onClick', ?, ?)
    `).run(now, now);

    const transCount = db.prepare("SELECT COUNT(*) as count FROM flow_transitions").get() as { count: number };
    expect(transCount.count).toBe(1);

    // Cascade delete node-1 -> trans-1 must be deleted automatically
    db.prepare("DELETE FROM flow_nodes WHERE id = 'node-1'").run();
    const remainingTrans = db.prepare("SELECT COUNT(*) as count FROM flow_transitions").get() as { count: number };
    expect(remainingTrans.count).toBe(0);
  });
});
