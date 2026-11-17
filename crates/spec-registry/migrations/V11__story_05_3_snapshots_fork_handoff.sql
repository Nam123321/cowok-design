-- V11__story_05_3_snapshots_fork_handoff.sql
-- Story 05.3: Immutable turn snapshots, conversation forking, snapshot ancestry,
-- and real GitHub PR / handoff manifest tracking.
--
-- Columns added idempotently via SpecRegistryPool::apply_v11_columns:
--   chat_sessions: forked_from_session_id, forked_from_message_id
--   chat_artifact_snapshots: proposal_id, parent_snapshot_id, node_snapshot_json
--   version_snapshots: proposal_id, parent_snapshot_id, chat_artifact_snapshot_id
--   github_pull_requests: snapshot_id, handoff_manifest_sha256, visual_diff_summary_json

CREATE INDEX IF NOT EXISTS idx_chat_sessions_forked_from
    ON chat_sessions(forked_from_session_id, forked_from_message_id);

CREATE INDEX IF NOT EXISTS idx_artifact_snapshots_proposal
    ON chat_artifact_snapshots(proposal_id);

CREATE INDEX IF NOT EXISTS idx_artifact_snapshots_parent
    ON chat_artifact_snapshots(parent_snapshot_id);

CREATE INDEX IF NOT EXISTS idx_version_snapshots_proposal
    ON version_snapshots(proposal_id);

CREATE INDEX IF NOT EXISTS idx_version_snapshots_parent
    ON version_snapshots(parent_snapshot_id);

CREATE INDEX IF NOT EXISTS idx_github_prs_snapshot
    ON github_pull_requests(snapshot_id);
