-- Migration: ConstraintProfile and ConstraintRule tables for project constraint engine

CREATE TABLE IF NOT EXISTS constraint_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    preset_profile TEXT NOT NULL DEFAULT 'DEFAULT',
    styling_policy TEXT NOT NULL DEFAULT 'CSS_MODULES',
    forbid_tailwind INTEGER NOT NULL DEFAULT 1,
    forbid_raw_hex INTEGER NOT NULL DEFAULT 1,
    tokens_css_path TEXT NOT NULL DEFAULT 'src/styles/tokens.css',
    icon_manifest_path TEXT NOT NULL DEFAULT 'icons/icon-manifest.json',
    allowed_icon_set_json TEXT NOT NULL DEFAULT '[]',
    typography_contract_json TEXT NOT NULL DEFAULT '{}',
    min_wcag_contrast REAL NOT NULL DEFAULT 4.5,
    updated_at BIGINT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS constraint_profiles_project_id_preset_profile_key ON constraint_profiles(project_id, preset_profile);
CREATE INDEX IF NOT EXISTS constraint_profiles_project_id_idx ON constraint_profiles(project_id);

CREATE TABLE IF NOT EXISTS constraint_rules (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES constraint_profiles(id) ON DELETE CASCADE,
    rule_code TEXT NOT NULL,
    severity TEXT NOT NULL CHECK(severity IN ('INFO', 'WARN', 'BLOCKER')),
    config_json TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    updated_at BIGINT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS constraint_rules_profile_id_rule_code_key ON constraint_rules(profile_id, rule_code);
CREATE INDEX IF NOT EXISTS constraint_rules_profile_id_idx ON constraint_rules(profile_id);
