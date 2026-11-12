-- RedefineTables
PRAGMA defer_foreign_keys=ON;
PRAGMA foreign_keys=OFF;

CREATE TABLE "new_constraint_profiles" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "project_id" TEXT NOT NULL,
    "preset_profile" TEXT NOT NULL DEFAULT 'DEFAULT',
    "styling_policy" TEXT NOT NULL DEFAULT 'CSS_MODULES',
    "forbid_tailwind" INTEGER NOT NULL DEFAULT 1,
    "forbid_raw_hex" INTEGER NOT NULL DEFAULT 1,
    "tokens_css_path" TEXT NOT NULL DEFAULT 'src/styles/tokens.css',
    "icon_manifest_path" TEXT NOT NULL DEFAULT 'icons/icon-manifest.json',
    "allowed_icon_set_json" TEXT NOT NULL DEFAULT '[]',
    "typography_contract_json" TEXT NOT NULL DEFAULT '{}',
    "min_wcag_contrast" REAL NOT NULL DEFAULT 4.5,
    "updated_at" BIGINT NOT NULL
);
INSERT INTO "new_constraint_profiles" ("allowed_icon_set_json", "forbid_raw_hex", "forbid_tailwind", "icon_manifest_path", "id", "min_wcag_contrast", "preset_profile", "project_id", "styling_policy", "tokens_css_path", "typography_contract_json", "updated_at") SELECT "allowed_icon_set_json", "forbid_raw_hex", "forbid_tailwind", "icon_manifest_path", "id", "min_wcag_contrast", "preset_profile", "project_id", "styling_policy", "tokens_css_path", "typography_contract_json", "updated_at" FROM "constraint_profiles";
DROP TABLE "constraint_profiles";
ALTER TABLE "new_constraint_profiles" RENAME TO "constraint_profiles";
CREATE INDEX "constraint_profiles_project_id_idx" ON "constraint_profiles"("project_id");
CREATE UNIQUE INDEX "constraint_profiles_project_id_preset_profile_key" ON "constraint_profiles"("project_id", "preset_profile");

CREATE TABLE "new_constraint_rules" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "profile_id" TEXT NOT NULL,
    "rule_code" TEXT NOT NULL,
    "severity" TEXT NOT NULL,
    "config_json" TEXT NOT NULL DEFAULT '{}',
    "enabled" INTEGER NOT NULL DEFAULT 1,
    "updated_at" BIGINT NOT NULL,
    CONSTRAINT "constraint_rules_profile_id_fkey" FOREIGN KEY ("profile_id") REFERENCES "constraint_profiles" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_constraint_rules" ("config_json", "enabled", "id", "profile_id", "rule_code", "severity", "updated_at") SELECT "config_json", "enabled", "id", "profile_id", "rule_code", "severity", "updated_at" FROM "constraint_rules";
DROP TABLE "constraint_rules";
ALTER TABLE "new_constraint_rules" RENAME TO "constraint_rules";
CREATE INDEX "constraint_rules_profile_id_idx" ON "constraint_rules"("profile_id");
CREATE UNIQUE INDEX "constraint_rules_profile_id_rule_code_key" ON "constraint_rules"("profile_id", "rule_code");

CREATE TABLE "new_flow_nodes" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "tenant_id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "portal_id" TEXT NOT NULL DEFAULT 'default',
    "screen_key" TEXT NOT NULL,
    "title" TEXT NOT NULL,
    "route_path" TEXT NOT NULL DEFAULT '/',
    "component_entry_path" TEXT NOT NULL DEFAULT '',
    "fixture_ref" TEXT NOT NULL DEFAULT 'Default',
    "fixture_json" TEXT NOT NULL DEFAULT '{}',
    "viewport_preset" TEXT NOT NULL DEFAULT 'desktop-1440',
    "pos_x" REAL NOT NULL DEFAULT 0.0,
    "pos_y" REAL NOT NULL DEFAULT 0.0,
    "version" INTEGER NOT NULL DEFAULT 1,
    "created_at" BIGINT NOT NULL,
    "updated_at" BIGINT NOT NULL
);
INSERT INTO "new_flow_nodes" ("component_entry_path", "created_at", "fixture_json", "fixture_ref", "id", "portal_id", "pos_x", "pos_y", "project_id", "route_path", "screen_key", "tenant_id", "title", "updated_at", "viewport_preset") SELECT "component_entry_path", "created_at", "fixture_json", "fixture_ref", "id", "portal_id", "pos_x", "pos_y", "project_id", "route_path", "screen_key", "tenant_id", "title", "updated_at", "viewport_preset" FROM "flow_nodes";
DROP TABLE "flow_nodes";
ALTER TABLE "new_flow_nodes" RENAME TO "flow_nodes";
CREATE INDEX "flow_nodes_tenant_id_idx" ON "flow_nodes"("tenant_id");
CREATE INDEX "flow_nodes_project_id_idx" ON "flow_nodes"("project_id");
CREATE INDEX "flow_nodes_project_id_portal_id_idx" ON "flow_nodes"("project_id", "portal_id");
CREATE INDEX "flow_nodes_project_id_screen_key_idx" ON "flow_nodes"("project_id", "screen_key");
CREATE UNIQUE INDEX "flow_nodes_project_id_screen_key_key" ON "flow_nodes"("project_id", "screen_key");
CREATE UNIQUE INDEX "flow_nodes_id_tenant_id_project_id_key" ON "flow_nodes"("id", "tenant_id", "project_id");

CREATE TABLE "new_flow_transitions" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "tenant_id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "source_node_id" TEXT NOT NULL,
    "target_node_id" TEXT NOT NULL,
    "trigger_event" TEXT NOT NULL DEFAULT 'onClick',
    "guard_condition" TEXT,
    "version" INTEGER NOT NULL DEFAULT 1,
    "created_at" BIGINT NOT NULL,
    "updated_at" BIGINT NOT NULL,
    CONSTRAINT "flow_transitions_source_node_id_tenant_id_project_id_fkey" FOREIGN KEY ("source_node_id", "tenant_id", "project_id") REFERENCES "flow_nodes" ("id", "tenant_id", "project_id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "flow_transitions_target_node_id_tenant_id_project_id_fkey" FOREIGN KEY ("target_node_id", "tenant_id", "project_id") REFERENCES "flow_nodes" ("id", "tenant_id", "project_id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "new_flow_transitions" ("created_at", "guard_condition", "id", "project_id", "source_node_id", "target_node_id", "tenant_id", "trigger_event", "updated_at") SELECT "created_at", "guard_condition", "id", "project_id", "source_node_id", "target_node_id", "tenant_id", "trigger_event", "updated_at" FROM "flow_transitions";
DROP TABLE "flow_transitions";
ALTER TABLE "new_flow_transitions" RENAME TO "flow_transitions";
CREATE INDEX "flow_transitions_tenant_id_idx" ON "flow_transitions"("tenant_id");
CREATE INDEX "flow_transitions_project_id_idx" ON "flow_transitions"("project_id");
CREATE INDEX "flow_transitions_source_node_id_idx" ON "flow_transitions"("source_node_id");
CREATE INDEX "flow_transitions_target_node_id_idx" ON "flow_transitions"("target_node_id");
CREATE UNIQUE INDEX "flow_transitions_project_id_source_node_id_target_node_id_trigger_event_key" ON "flow_transitions"("project_id", "source_node_id", "target_node_id", "trigger_event");

PRAGMA foreign_keys=ON;
PRAGMA defer_foreign_keys=OFF;
