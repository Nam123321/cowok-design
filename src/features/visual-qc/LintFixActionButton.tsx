// @story 07.1
// @implements AC-03, AC-04 (1-Click Deterministic <10ms lint_fix, Max 3 Agent Attempts & Cmd+Z Rollback)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import { MAX_AUTOFIX_ATTEMPTS, type QcAutoFixResponseData } from "../../lib/validations/visual-qc";

const ICON_SM = 13;
const STROKE_MONO = 1.85;
const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;
const MAX_DIFF_SUMMARY_LINES = 3;
const DISABLED_OPACITY = 0.65;

export interface LintFixActionButtonProps {
  deterministicCount: number;
  totalIssueCount: number;
  isFixing: boolean;
  agentAttemptsUsed: number;
  isAgentFixLocked: boolean;
  canRollback: boolean;
  lastFixResult: QcAutoFixResponseData | null;
  onDeterministicFix: () => void;
  onAgentFix?: () => void;
  onRollback: () => void;
}

const FixDiffSummaryStrip: React.FC<{ result: QcAutoFixResponseData }> = ({ result }) => {
  const { t } = useTranslation();
  return (
    <div
      data-testid="qc-autofix-diff-summary"
      style={{
        display: "flex", flexDirection: "column", gap: "4px", padding: "8px 10px",
        borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)",
        backgroundColor: "var(--bg-subtle)", color: "var(--text-primary)", fontSize: "11px",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
        <span>{t("visualQc.fixSummaryHeader", "Auto-Fix Applied")} ({result.fixedIssueIds.length})</span>
        <span style={{ color: "var(--text-secondary)" }}>
          {result.elapsedMs.toFixed(1)}ms · {result.llmTokensUsed} {t("visualQc.tokensUnit", "tokens")}
        </span>
      </div>
      {result.diffSummary.slice(0, MAX_DIFF_SUMMARY_LINES).map((line, idx) => (
        <span key={idx} style={{ color: "var(--text-secondary)" }}>{line}</span>
      ))}
    </div>
  );
};

const AgentAndRollbackControls: React.FC<{
  totalIssueCount: number;
  isFixing: boolean;
  agentAttemptsUsed: number;
  isAgentFixLocked: boolean;
  canRollback: boolean;
  onAgentFix?: () => void;
  onRollback: () => void;
}> = ({ totalIssueCount, isFixing, agentAttemptsUsed, isAgentFixLocked, canRollback, onAgentFix, onRollback }) => {
  const { t } = useTranslation();
  const agentDisabled = isFixing || isAgentFixLocked || totalIssueCount === 0;
  return (
    <>
      {onAgentFix && (
        <button
          type="button"
          data-testid="agent-autofix-button"
          disabled={agentDisabled}
          onClick={onAgentFix}
          style={{
            display: "inline-flex", alignItems: "center", gap: "5px", height: "30px", padding: "0 10px",
            borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)",
            backgroundColor: "var(--bg-surface)", fontSize: "11.5px", fontWeight: FONT_WEIGHT_MEDIUM,
            color: isAgentFixLocked ? "var(--text-secondary)" : "var(--text-primary)",
            cursor: agentDisabled ? "not-allowed" : "pointer",
          }}
        >
          <BrandIcon name="refresh-cw" size={ICON_SM} strokeWidth={STROKE_MONO} />
          <span>
            {isAgentFixLocked
              ? t("visualQc.agentFixLocked", "Agent Fix Locked (3/3)")
              : `${t("visualQc.agentFix", "Agent Fix")} (${agentAttemptsUsed}/${MAX_AUTOFIX_ATTEMPTS})`}
          </span>
        </button>
      )}
      {canRollback && (
        <button
          type="button"
          data-testid="qc-rollback-button"
          onClick={onRollback}
          style={{
            display: "inline-flex", alignItems: "center", gap: "5px", height: "30px", padding: "0 9px",
            borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)",
            backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", fontSize: "11.5px", cursor: "pointer",
          }}
        >
          <BrandIcon name="undo" size={ICON_SM} strokeWidth={STROKE_MONO} />
          <span>{t("visualQc.rollbackUndo", "Undo (Cmd+Z)")}</span>
        </button>
      )}
    </>
  );
};

export const LintFixActionButton: React.FC<LintFixActionButtonProps> = (props) => {
  const { t } = useTranslation();
  const active = props.deterministicCount > 0;
  const disabled = props.isFixing || !active;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
        <button
          type="button"
          data-ff-id="one-click-lint-fix-button"
          data-testid="one-click-lint-fix-button"
          disabled={disabled}
          onClick={props.onDeterministicFix}
          style={{
            display: "inline-flex", alignItems: "center", gap: "6px", height: "30px", padding: "0 10px",
            borderRadius: "var(--radius-xs)", border: "1px solid var(--action-primary)",
            backgroundColor: active ? "var(--action-primary)" : "var(--bg-subtle)",
            color: active ? "var(--action-primary-fg)" : "var(--text-secondary)",
            fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD,
            cursor: disabled ? "not-allowed" : "pointer",
            opacity: disabled ? DISABLED_OPACITY : 1,
          }}
        >
          <BrandIcon name="check-circle" size={ICON_SM} strokeWidth={STROKE_MONO} />
          <span>
            {props.isFixing
              ? t("visualQc.fixing", "Fixing...")
              : `${t("visualQc.oneClickLintFix", "1-Click Auto-Fix (<10ms · 0 tokens)")} (${props.deterministicCount})`}
          </span>
        </button>
        <AgentAndRollbackControls
          totalIssueCount={props.totalIssueCount}
          isFixing={props.isFixing}
          agentAttemptsUsed={props.agentAttemptsUsed}
          isAgentFixLocked={props.isAgentFixLocked}
          canRollback={props.canRollback}
          onAgentFix={props.onAgentFix}
          onRollback={props.onRollback}
        />
      </div>
      {props.lastFixResult && <FixDiffSummaryStrip result={props.lastFixResult} />}
    </div>
  );
};
