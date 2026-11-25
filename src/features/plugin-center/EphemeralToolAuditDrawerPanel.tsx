// @story 08.5
// @implements AC-02, AC-05 (HUMAN_ONLY Controlled Ephemeral Tool Jail Audit Drawer Panel)
import React, { useState } from "react";
import { DeRuntimeApiError, executeEphemeralTool } from "../../api/de-runtime";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import {
  DEFAULT_TOOL_TIMEOUT_MS,
  MAX_TOOL_TIMEOUT_MS,
  MIN_TOOL_TIMEOUT_MS,
  type EphemeralRuntime,
  type ToolJailAuditReceiptDto,
} from "../../lib/validations/de-runtime";
import "../plugins/plugins.css";

const ICON_SIZE = 14;
const SHA_PREFIX_LEN = 12;
const RUNTIMES: readonly EphemeralRuntime[] = ["sh", "python3", "node"];

export interface EphemeralToolAuditDrawerPanelProps {
  projectId: string;
}

interface JailFormState {
  toolName: string;
  runtime: EphemeralRuntime;
  timeoutMs: number;
  scriptSource: string;
}

const DEFAULT_JAIL_FORM: JailFormState = {
  toolName: "token-line-counter",
  runtime: "sh",
  timeoutMs: DEFAULT_TOOL_TIMEOUT_MS,
  scriptSource: "echo 'JAIL_ISOLATION_OK'",
};

const JailAuditReceiptCard: React.FC<{ receipt: ToolJailAuditReceiptDto }> = ({ receipt }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div className="plugin-eco-card" data-testid="ephemeral-tool-receipt">
      <div className="plugin-eco-card-row">
        <span className="plugin-eco-slug">
          {receipt.toolName} ({receipt.runtime}) • {t("deRuntime.exitCodeLabel", "Mã thoát")}{" "}
          {formatNumber(receipt.exitCode)}
        </span>
        <span className="plugin-eco-badge plugin-eco-badge-active">
          {formatNumber(receipt.durationMs)}ms • {receipt.scriptSha256.slice(0, SHA_PREFIX_LEN)}
        </span>
      </div>
      <div className="plugin-eco-actions">
        <span className="plugin-eco-badge">env_clear: {String(receipt.envCleared)}</span>
        <span className="plugin-eco-badge">O_NOFOLLOW: {String(receipt.nofollowVerified)}</span>
        <span className="plugin-eco-badge">net_isolated: {String(receipt.networkIsolated)}</span>
      </div>
      <pre className="plugin-eco-markdown-preview" data-testid="ephemeral-tool-stdout">
        {receipt.stdout || receipt.stderr || t("deRuntime.noOutputPlaceholder", "(không có đầu ra)")}
      </pre>
    </div>
  );
};

const JailControlsRow: React.FC<{
  form: JailFormState;
  onPatch: (p: Partial<JailFormState>) => void;
}> = ({ form, onPatch }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-schedule-form">
      <input
        type="text"
        className="plugin-eco-input"
        data-testid="input-jail-tool-name"
        value={form.toolName}
        onChange={(e) => onPatch({ toolName: e.target.value })}
        placeholder={t("deRuntime.toolNamePlaceholder", "ephemeral-check")}
      />
      <select
        className="plugin-eco-input"
        data-testid="select-jail-runtime"
        value={form.runtime}
        onChange={(e) => onPatch({ runtime: e.target.value as EphemeralRuntime })}
        aria-label={t("deRuntime.runtimeLabel", "Runtime")}
      >
        {RUNTIMES.map((rt) => (
          <option key={rt} value={rt}>{rt}</option>
        ))}
      </select>
      <input
        type="number"
        className="plugin-eco-input"
        data-testid="input-jail-timeout-ms"
        min={MIN_TOOL_TIMEOUT_MS}
        max={MAX_TOOL_TIMEOUT_MS}
        value={form.timeoutMs}
        onChange={(e) =>
          onPatch({ timeoutMs: Number(e.target.value) || DEFAULT_TOOL_TIMEOUT_MS })
        }
        aria-label={t("deRuntime.timeoutLabel", "Timeout (ms)")}
      />
    </div>
  );
};

const JailScriptEditor: React.FC<{
  form: JailFormState;
  running: boolean;
  onPatch: (p: Partial<JailFormState>) => void;
  onExecute: () => void;
}> = ({ form, running, onPatch, onExecute }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-form-grid">
      <JailControlsRow form={form} onPatch={onPatch} />
      <textarea
        className="plugin-eco-textarea"
        data-testid="textarea-jail-script"
        value={form.scriptSource}
        onChange={(e) => onPatch({ scriptSource: e.target.value })}
        placeholder={t("deRuntime.scriptPlaceholder", "echo 'Audit verified'")}
      />
      <div className="plugin-eco-actions">
        <button
          type="button"
          className="plugin-eco-btn plugin-eco-btn-danger"
          data-testid="btn-execute-ephemeral-tool"
          disabled={running || !form.scriptSource.trim()}
          onClick={onExecute}
        >
          {running
            ? t("deRuntime.jailExecutingBtn", "Executing in Isolated Jail...")
            : t("deRuntime.jailExecuteBtn", "Execute in HUMAN_ONLY Jail")}
        </button>
      </div>
    </div>
  );
};

const JailDrawerHeader: React.FC<{ expanded: boolean; onToggle: () => void }> = ({
  expanded,
  onToggle,
}) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-header">
      <div className="plugin-eco-title-group">
        <BrandIcon name="security" size={ICON_SIZE} color="var(--state-danger)" />
        <h3 className="plugin-eco-title">
          {t("deRuntime.jailDrawerTitle", "Ephemeral Tool Audit Drawer")}
        </h3>
      </div>
      <div className="plugin-eco-actions">
        <span
          className="plugin-eco-badge plugin-eco-badge-human-only"
          data-testid="jail-human-only-badge"
        >
          {t("deRuntime.jailIsolationBadge", "Chạy cô lập · Chỉ người dùng thao tác")}
        </span>
        <button
          type="button"
          className="plugin-eco-btn"
          data-testid="btn-toggle-jail-drawer"
          onClick={onToggle}
        >
          {expanded ? t("deRuntime.collapseBtn", "Collapse") : t("deRuntime.expandBtn", "Expand")}
        </button>
      </div>
    </div>
  );
};

function useJailDrawerController(projectId: string) {
  const [expanded, setExpanded] = useState(true);
  const [form, setForm] = useState<JailFormState>(DEFAULT_JAIL_FORM);
  const [running, setRunning] = useState(false);
  const [receipt, setReceipt] = useState<ToolJailAuditReceiptDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  const patchForm = (p: Partial<JailFormState>) => setForm((prev) => ({ ...prev, ...p }));
  const toggleExpanded = () => setExpanded((prev) => !prev);

  const execute = async () => {
    setRunning(true);
    setError(null);
    try {
      const res = await executeEphemeralTool({
        projectId,
        toolName: form.toolName.trim() || "ephemeral-tool",
        runtime: form.runtime,
        scriptSource: form.scriptSource,
        timeoutMs: Math.min(Math.max(form.timeoutMs, MIN_TOOL_TIMEOUT_MS), MAX_TOOL_TIMEOUT_MS),
      });
      setReceipt(res.data);
    } catch (err) {
      setError(err instanceof DeRuntimeApiError ? `[${err.code}] ${err.message}` : String(err));
    } finally {
      setRunning(false);
    }
  };

  return { expanded, form, running, receipt, error, patchForm, toggleExpanded, execute };
}

export const EphemeralToolAuditDrawerPanel: React.FC<EphemeralToolAuditDrawerPanelProps> = ({
  projectId,
}) => {
  const ctrl = useJailDrawerController(projectId);

  return (
    <aside
      className="plugin-eco-section plugin-eco-drawer-panel"
      data-ff-id="ephemeral-tool-audit-drawer"
      data-testid="ephemeral-tool-audit-drawer"
    >
      <JailDrawerHeader expanded={ctrl.expanded} onToggle={ctrl.toggleExpanded} />
      {ctrl.expanded && (
        <>
          <JailScriptEditor
            form={ctrl.form}
            running={ctrl.running}
            onPatch={ctrl.patchForm}
            onExecute={() => void ctrl.execute()}
          />
          {ctrl.error && (
            <div className="plugin-eco-error-box" role="alert" data-testid="ephemeral-tool-error">
              <span>{ctrl.error}</span>
            </div>
          )}
          {ctrl.receipt && <JailAuditReceiptCard receipt={ctrl.receipt} />}
        </>
      )}
    </aside>
  );
};
