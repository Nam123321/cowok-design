-- CreateTable flow_nodes
CREATE TABLE IF NOT EXISTS "flow_nodes" (
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
    "created_at" INTEGER NOT NULL,
    "updated_at" INTEGER NOT NULL
);

-- CreateTable flow_transitions
CREATE TABLE IF NOT EXISTS "flow_transitions" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "tenant_id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "source_node_id" TEXT NOT NULL,
    "target_node_id" TEXT NOT NULL,
    "trigger_event" TEXT NOT NULL DEFAULT 'onClick',
    "guard_condition" TEXT,
    "created_at" INTEGER NOT NULL,
    "updated_at" INTEGER NOT NULL,
    FOREIGN KEY ("source_node_id") REFERENCES "flow_nodes" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    FOREIGN KEY ("target_node_id") REFERENCES "flow_nodes" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateIndex
CREATE INDEX IF NOT EXISTS "flow_nodes_tenant_id_idx" ON "flow_nodes"("tenant_id");
CREATE INDEX IF NOT EXISTS "flow_nodes_project_id_idx" ON "flow_nodes"("project_id");
CREATE INDEX IF NOT EXISTS "flow_nodes_project_id_portal_id_idx" ON "flow_nodes"("project_id", "portal_id");
CREATE INDEX IF NOT EXISTS "flow_nodes_project_id_screen_key_idx" ON "flow_nodes"("project_id", "screen_key");

CREATE INDEX IF NOT EXISTS "flow_transitions_tenant_id_idx" ON "flow_transitions"("tenant_id");
CREATE INDEX IF NOT EXISTS "flow_transitions_project_id_idx" ON "flow_transitions"("project_id");
CREATE INDEX IF NOT EXISTS "flow_transitions_source_node_id_idx" ON "flow_transitions"("source_node_id");
CREATE INDEX IF NOT EXISTS "flow_transitions_target_node_id_idx" ON "flow_transitions"("target_node_id");
