// @story 04.4, 04.5
// @implements Clean-Room EV-002 (5-Hour & Weekly Provider Quota Sync Popover + Searchable Model Switcher)
import React, { useMemo, useState } from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";
import { useFormatter } from "@/hooks/useFormatter";
import { collectSelectableModels, useDoctorProvidersUsage } from "@/hooks/useDoctorProvidersUsage";
import type { ProviderUsageDto, QuotaWindowDto } from "@/lib/validations/workbench-settings";

export interface ModelPickerQuotaPopoverProps {
  selectedModel: { id: string; label: string };
  onSelectModel: (model: { id: string; label: string }) => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
}

const EFFORT_OPTIONS = ["auto", "low", "medium", "high"] as const;

function formatCheckedAge(checkedAtMs: number | null, t: (k: string, f: string, p?: Record<string, unknown>) => string): string {
  if (!checkedAtMs) return t("quota.notCheckedYet", "chưa đồng bộ");
  const diffMin = Math.max(0, Math.round((Date.now() - checkedAtMs) / 60_000));
  if (diffMin < 1) return t("quota.checkedJustNow", "vừa kiểm tra");
  return t("quota.checkedMinutesAgo", `đã kiểm tra ${diffMin} phút trước`, { minutes: diffMin });
}

function formatResetClock(
  window: QuotaWindowDto,
  formatDate: (d: number) => string,
  formatTime: (d: number) => string
): string | null {
  if (!window.resetsAtEpochMs) return null;
  const dateLabel = formatDate(window.resetsAtEpochMs);
  const timeLabel = formatTime(window.resetsAtEpochMs);
  return dateLabel ? `${dateLabel} ${timeLabel}` : timeLabel;
}

function barFillColor(usedPercent: number): string {
  if (usedPercent >= 90) return "var(--state-danger)";
  if (usedPercent >= 70) return "var(--state-warning)";
  return "var(--state-success)";
}

const QuotaWindowBar: React.FC<{
  label: string;
  window: QuotaWindowDto;
}> = ({ label, window }) => {
  const { t, locale } = useTranslation();
  const { formatDate, formatTime } = useFormatter({ locale });
  const used = Math.min(100, Math.max(0, Math.round(window.usedPercent ?? 0)));
  const left = Math.min(100, Math.max(0, Math.round(window.remainingPercent ?? 100 - used)));
  const resetStr = formatResetClock(window, formatDate, formatTime);
  const atLimit = used >= 100 && !window.resetPassed;
  const statusHeadline = atLimit
    ? t("quota.atLimit", "Đã chạm hạn mức")
    : `${left}% ${t("quota.percentLeft", "còn lại")}`;

  return (
    <div data-testid={`quota-window-${label}`} style={{ display: "flex", flexDirection: "column", gap: "3px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", fontSize: "10.5px", gap: "8px" }}>
        <span style={{ color: "var(--text-secondary)", fontWeight: 500 }}>{label}</span>
        <span style={{ color: atLimit ? "var(--state-warning)" : "var(--text-primary)", fontWeight: atLimit ? 600 : 500 }}>
          {statusHeadline}
          {resetStr ? ` · ${t("quota.resetsAt", "đặt lại")} ${resetStr}` : ""}
        </span>
      </div>
      <div style={{ height: "5px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-subtle)", overflow: "hidden", border: "1px solid var(--border-subtle)" }}>
        <div style={{ height: "100%", width: `${atLimit ? 100 : used}%`, backgroundColor: barFillColor(atLimit ? 100 : used), transition: "width var(--duration-fast)" }} />
      </div>
    </div>
  );
};

function pickRelevantUsageProvider(
  providers: ProviderUsageDto[],
  selectedModelId: string
): ProviderUsageDto | undefined {
  const byModel = providers.find((p) => (p.models ?? []).some((m) => m.id === selectedModelId));
  if (byModel) return byModel;
  return providers.find((p) => p.fiveHour?.available || p.weekly?.available) ?? providers.find((p) => p.ok) ?? providers[0];
}

const UsageHeaderBlock: React.FC<{
  provider: ProviderUsageDto | undefined;
  checkedAtMs: number | null;
  loading: boolean;
  onRefresh: () => void;
}> = ({ provider, checkedAtMs, loading, onRefresh }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="composer-usage-block" style={{ padding: "8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "6px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
        <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-primary)" }}>
          {provider ? `${provider.name}` : t("quota.usageTitle", "Hạn mức gói AI")}
        </span>
        <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
          {formatCheckedAge(checkedAtMs, t)}
        </span>
      </div>
      {provider?.fiveHour?.available && (
        <QuotaWindowBar label={t("quota.fiveHourWindow", "5 giờ")} window={provider.fiveHour} />
      )}
      {provider?.weekly?.available && (
        <QuotaWindowBar label={t("quota.weeklyWindow", "Tuần")} window={provider.weekly} />
      )}
      {!provider?.fiveHour?.available && !provider?.weekly?.available && (
        <div style={{ fontSize: "10px", color: "var(--text-muted)", lineHeight: 1.35 }}>
          {provider?.note || t("quota.noQuotaFeed", "Nhà cung cấp chưa xuất dữ liệu 5h/Tuần cục bộ.")}
        </div>
      )}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", fontSize: "10px", paddingTop: "2px" }}>
        <span style={{ color: "var(--text-muted)" }}>
          {provider?.plan || provider?.accountLabel || t("quota.autoClearHint", "Tự đồng bộ khi có phản hồi mới.")}
        </span>
        <button
          type="button"
          data-testid="quota-check-again-btn"
          onClick={onRefresh}
          disabled={loading}
          style={{ border: "none", background: "transparent", color: "var(--action-primary)", textDecoration: "underline", fontSize: "10px", fontWeight: 600, cursor: loading ? "wait" : "pointer", padding: 0 }}
        >
          {loading ? "..." : t("quota.checkAgain", "Kiểm tra lại")}
        </button>
      </div>
    </div>
  );
};

export const ModelPickerQuotaPopover: React.FC<ModelPickerQuotaPopoverProps> = ({
  selectedModel,
  onSelectModel,
  onOpenSettings,
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [effort, setEffort] = useState<(typeof EFFORT_OPTIONS)[number]>("auto");
  const doctor = useDoctorProvidersUsage(true);

  const models = useMemo(
    () => collectSelectableModels(doctor.readyTools, doctor.usageProviders),
    [doctor.readyTools, doctor.usageProviders]
  );
  const filteredModels = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    return q ? models.filter((m) => m.label.toLowerCase().includes(q) || m.providerName.toLowerCase().includes(q)) : models;
  }, [models, searchQuery]);

  const activeUsage = useMemo(
    () => pickRelevantUsageProvider(doctor.usageProviders, selectedModel.id),
    [doctor.usageProviders, selectedModel.id]
  );
  const activeOption = models.find((m) => m.id === selectedModel.id) ?? selectedModel;
  const weeklyLeft = activeUsage?.weekly?.available
    ? Math.max(0, Math.round(activeUsage.weekly.remainingPercent ?? 100 - (activeUsage.weekly.usedPercent ?? 0)))
    : null;

  return (
    <div style={{ position: "relative" }}>
      <button
        type="button"
        data-testid="composer-model-picker-btn"
        onClick={() => setOpen((v) => !v)}
        title={t("chat.selectModel", "Chọn mô hình AI")}
        aria-label={t("chat.selectModel", "Chọn mô hình AI")}
        style={{ height: "24px", padding: "0 8px", display: "flex", alignItems: "center", gap: "5px", fontSize: "11px", fontWeight: 500, borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", color: "var(--text-primary)", cursor: "pointer" }}
      >
        <span style={{ maxWidth: "150px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {activeOption.label}
        </span>
        {weeklyLeft !== null && (
          <span data-testid="composer-quota-pill" style={{ fontSize: "9.5px", fontWeight: 600, padding: "0 4px", borderRadius: "var(--radius-xs)", backgroundColor: weeklyLeft <= 10 ? "var(--state-warning-soft)" : "var(--state-success-soft)", color: weeklyLeft <= 10 ? "var(--state-warning)" : "var(--state-success)" }}>
            {weeklyLeft}%
          </span>
        )}
        <BrandIcon name="chevron-down" size={10} color="var(--text-muted)" />
      </button>

      {open && (
        <div data-testid="composer-model-popover" style={{ position: "absolute", bottom: "30px", left: 0, width: "292px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", boxShadow: "var(--shadow-lg)", padding: "8px", zIndex: 30, display: "flex", flexDirection: "column", gap: "7px" }}>
          <UsageHeaderBlock
            provider={activeUsage}
            checkedAtMs={doctor.checkedAtEpochMs}
            loading={doctor.loading}
            onRefresh={() => { void doctor.refresh({ active: false, refreshUsage: true }); }}
          />
          <input
            type="text"
            data-testid="composer-model-search"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t("quota.searchModels", "Tìm mô hình...")}
            style={{ width: "100%", height: "26px", padding: "0 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", boxSizing: "border-box" }}
          />
          <div style={{ maxHeight: "176px", overflowY: "auto", display: "flex", flexDirection: "column", gap: "2px" }}>
            {filteredModels.map((m) => {
              const selected = selectedModel.id === m.id;
              return (
                <button
                  key={m.id}
                  type="button"
                  data-testid={`composer-model-option-${m.id}`}
                  onClick={() => { onSelectModel({ id: m.id, label: m.label }); setOpen(false); }}
                  style={{ width: "100%", textAlign: "left", padding: "6px 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", cursor: "pointer", border: "none", borderLeft: selected ? "2px solid var(--action-primary)" : "2px solid transparent", backgroundColor: selected ? "var(--bg-selected)" : "transparent", color: "var(--text-primary)", fontWeight: selected ? 600 : 400, display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}
                >
                  <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{m.label}</span>
                  <span style={{ fontSize: "9.5px", color: m.ready ? "var(--state-success)" : "var(--text-muted)", flexShrink: 0 }}>
                    {selected ? "✓" : m.ready ? t("settings.providerReady", "Sẵn sàng") : m.providerName}
                  </span>
                </button>
              );
            })}
          </div>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "4px 6px", borderTop: "1px solid var(--border-subtle)", borderBottom: "1px solid var(--border-subtle)", fontSize: "11px" }}>
            <span style={{ color: "var(--text-secondary)" }}>{t("quota.thinkingEffort", "Mức suy luận")}</span>
            <select
              value={effort}
              onChange={(e) => setEffort(e.target.value as (typeof EFFORT_OPTIONS)[number])}
              style={{ border: "none", background: "transparent", color: "var(--text-primary)", fontSize: "11px", fontWeight: 600, cursor: "pointer" }}
            >
              {EFFORT_OPTIONS.map((lvl) => (
                <option key={lvl} value={lvl}>{lvl.toUpperCase()}</option>
              ))}
            </select>
          </div>
          <button
            type="button"
            data-testid="manage-ai-accounts-btn"
            onClick={() => { setOpen(false); onOpenSettings?.("models"); }}
            style={{ padding: "5px 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "none", background: "transparent", cursor: "pointer", color: "var(--action-primary)", fontWeight: 500, display: "flex", alignItems: "center", justifyContent: "space-between", gap: "5px" }}
          >
            <span style={{ display: "inline-flex", alignItems: "center", gap: "5px" }}>
              <BrandIcon name="settings" size={11} color="currentColor" />
              <span>{t("quota.manageAiAccounts", "Quản lý tài khoản & BYOS AI")}</span>
            </span>
            <span>›</span>
          </button>
        </div>
      )}
    </div>
  );
};
