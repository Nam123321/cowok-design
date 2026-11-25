// @story 08.4
// @implements AC-02, AC-04, AC-05 (Staging Diff & Human Promote/Discard Gate)
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { PluginSummaryDto } from "../../lib/validations/plugins";

const STAGING_ICON_SIZE = 13;
const MIN_RATIONALE_CHARS = 4;
const MAX_RATIONALE_CHARS = 500;

export interface StagingDiffCardProps {
  plugin: PluginSummaryDto;
  pendingActionId: string | null;
  onPromote: (plugin: PluginSummaryDto, reason: string) => void;
  onDiscard: (plugin: PluginSummaryDto, reason: string) => void;
}

const StagingReasonsList: React.FC<{ reasons: string[] }> = ({ reasons }) => {
  if (reasons.length === 0) return null;
  return (
    <ul className="plugin-eco-meta" style={{ margin: 0, paddingLeft: "16px" }}>
      {reasons.map((reason, idx) => (
        <li key={`${idx}:${reason}`}>{reason}</li>
      ))}
    </ul>
  );
};

const StagingActionBar: React.FC<{
  plugin: PluginSummaryDto;
  pendingActionId: string | null;
  onPromote: (plugin: PluginSummaryDto, reason: string) => void;
  onDiscard: (plugin: PluginSummaryDto, reason: string) => void;
}> = ({ plugin, pendingActionId, onPromote, onDiscard }) => {
  const { t } = useTranslation();
  const [reason, setReason] = useState("");
  const isPromoting = pendingActionId === `promote:${plugin.id}`;
  const isDiscarding = pendingActionId === `discard:${plugin.id}`;
  const busy = isPromoting || isDiscarding;
  const reasonValid = reason.trim().length >= MIN_RATIONALE_CHARS;
  return (
    <>
      <input
        type="text"
        className="plugin-eco-rationale-input"
        data-testid={`input-staging-reason-${plugin.pluginSlug}`}
        minLength={MIN_RATIONALE_CHARS}
        maxLength={MAX_RATIONALE_CHARS}
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        placeholder={t("plugins.rationalePlaceholder", "Enter rationale (>= 4 chars)...")}
      />
      <div className="plugin-eco-actions">
        <button
          type="button"
          className="plugin-eco-btn plugin-eco-btn-primary"
          data-testid={`btn-promote-staging-${plugin.pluginSlug}`}
          disabled={busy || !reasonValid}
          onClick={() => onPromote(plugin, reason.trim())}
        >
          {isPromoting ? t("plugins.promoting", "Promoting...") : t("plugins.promoteBtn", "Promote Staging -> ACTIVE")}
        </button>
        <button
          type="button"
          className="plugin-eco-btn plugin-eco-btn-danger"
          data-testid={`btn-discard-staging-${plugin.pluginSlug}`}
          disabled={busy || !reasonValid}
          onClick={() => onDiscard(plugin, reason.trim())}
        >
          {isDiscarding ? t("plugins.discarding", "Discarding...") : t("plugins.discardBtn", "Discard Candidate")}
        </button>
      </div>
    </>
  );
};

export const StagingDiffCard: React.FC<StagingDiffCardProps> = ({
  plugin,
  pendingActionId,
  onPromote,
  onDiscard,
}) => {
  const { t } = useTranslation();
  if (!plugin.breakingChangeIsolated || !plugin.stagingVersion) return null;

  const reasonCount = plugin.stagingDiffReasons.length;
  const impactSummary = t(
    "plugins.stagingImpactSummary",
    `Updating v${plugin.version} -> v${plugin.stagingVersion} introduces ${reasonCount} breaking change(s); active runs stay pinned to v${plugin.version} until promoted.`
  )
    .replace("{{from}}", plugin.version)
    .replace("{{to}}", plugin.stagingVersion)
    .replace("{{count}}", String(reasonCount));

  return (
    <div
      className="plugin-eco-staging-diff"
      data-ff-id="staging-diff-card"
      data-testid={`staging-diff-card-${plugin.pluginSlug}`}
    >
      <div className="plugin-eco-card-row">
        <div className="plugin-eco-title-group">
          <BrandIcon name="workflow" size={STAGING_ICON_SIZE} color="var(--state-warning)" />
          <span className="plugin-eco-title">
            {t("plugins.stagingHeader", "Isolated Staging Candidate")}: v{plugin.stagingVersion} (v{plugin.version})
          </span>
        </div>
        <span className="plugin-eco-badge plugin-eco-badge-human">
          {t("mcpGateway.tierHumanOnly", "Chỉ người dùng thao tác")}
        </span>
      </div>
      <p className="plugin-eco-meta" data-testid={`staging-impact-summary-${plugin.pluginSlug}`} style={{ margin: 0 }}>
        {impactSummary}
      </p>
      <StagingReasonsList reasons={plugin.stagingDiffReasons} />
      <StagingActionBar
        plugin={plugin}
        pendingActionId={pendingActionId}
        onPromote={onPromote}
        onDiscard={onDiscard}
      />
    </div>
  );
};
