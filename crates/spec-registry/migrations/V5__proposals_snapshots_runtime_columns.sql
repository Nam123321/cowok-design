-- Migration V5: Additive runtime columns & indexes for ChangeProposals, VersionSnapshots,
-- ProjectWorkspaceFiles, and GitHubPRs (preserves V2 table ledger cleanly without breaking FKs).

CREATE INDEX IF NOT EXISTS change_proposals_project_updated_idx
    ON change_proposals(project_id, updated_at);

CREATE INDEX IF NOT EXISTS version_snapshots_project_created_idx
    ON version_snapshots(project_id, created_at);

CREATE INDEX IF NOT EXISTS project_workspace_files_project_path_idx
    ON project_workspace_files(project_id, relative_path);
