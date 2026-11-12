-- Migration V2: Project Boundaries, Chat Sessions, Change Proposals, Snapshots, PRs, Tombstones & Recovery Commits
-- Extends the schema to 24 tables with centralized, non-fragmented DDL (AC-08).

-- 1. spec_tombstones (AC-07)
CREATE TABLE IF NOT EXISTS spec_tombstones (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    spec_key TEXT NOT NULL,
    deleted_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_by TEXT NOT NULL,
    content_sha256_at_deletion TEXT NOT NULL,
    UNIQUE(project_id, spec_key)
);

-- 2. project_boundaries
CREATE TABLE IF NOT EXISTS project_boundaries (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    boundary_type TEXT NOT NULL DEFAULT 'managed_scratch',
    workspace_path TEXT NOT NULL DEFAULT '',
    config_dir TEXT NOT NULL DEFAULT '.opendesign',
    is_git_repository INTEGER NOT NULL DEFAULT 0,
    promoted_at BIGINT,
    allowed_extensions_json TEXT NOT NULL DEFAULT '["md"]',
    max_file_size_bytes INTEGER NOT NULL DEFAULT 10485760,
    created_at BIGINT NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    updated_at BIGINT NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

-- 3. chat_sessions
CREATE TABLE IF NOT EXISTS chat_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    model_provider TEXT NOT NULL DEFAULT 'default',
    created_at BIGINT NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    updated_at BIGINT NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);
CREATE INDEX IF NOT EXISTS chat_sessions_project_idx ON chat_sessions(project_id);

-- 4. change_proposals
CREATE TABLE IF NOT EXISTS change_proposals (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    spec_key TEXT NOT NULL,
    diff_content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 5. version_snapshots
CREATE TABLE IF NOT EXISTS version_snapshots (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    snapshot_tag TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, snapshot_tag)
);

-- 6. project_workspace_files
CREATE TABLE IF NOT EXISTS project_workspace_files (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    file_sha256 TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, relative_path)
);

-- 7. github_pull_requests
CREATE TABLE IF NOT EXISTS github_pull_requests (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    pr_number INTEGER NOT NULL,
    branch_name TEXT NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, pr_number)
);

-- 8. recovery_commits
CREATE TABLE IF NOT EXISTS recovery_commits (
    journal_id TEXT NOT NULL PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    spec_key TEXT NOT NULL,
    next_rev INTEGER NOT NULL,
    applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status TEXT NOT NULL DEFAULT 'pending',
    UNIQUE(project_id, spec_key, next_rev)
);

-- Index ensuring multi-project workspace root isolation
CREATE UNIQUE INDEX IF NOT EXISTS projects_workspace_root_unique ON projects(workspace_root);
