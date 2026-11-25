// @story 14.3
// @implements AC-03, AC-05 (Deterministic Zero-PII A/B Experiments Manager — Split Presets, Funnel/Crash Attribution & Instant Kill-Switch)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { translateGlobal, useTranslation } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import {
  ASCII_IDENTIFIER_REGEX,
  type AbExperiment,
  type ExperimentStatus,
  type ExperimentTargetSurface,
  type SignedExperimentsManifest,
  type UpsertExperimentRequest,
} from "../../../lib/validations/distribution-ops";

export interface AbExperimentsTabProps {
  experiments: AbExperiment[];
  signedManifest: SignedExperimentsManifest | null;
  operatorRole?: CloudAdminRole;
  submitting: boolean;
  onUpsertExperiment: (input: UpsertExperimentRequest) => Promise<boolean>;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 15;
const MIN_TITLE_LEN = 3;
const MIN_REASON_LEN = 5;
const SPLIT_HALF = 50;
const SPLIT_TEN = 10;
const SPLIT_NINETY = 90;

const TARGET_SURFACE_I18N: Record<ExperimentTargetSurface, { key: string; fallback: string }> = {
  ONBOARDING_DEFAULTS: {
    key: "superadmin.ops.surfaceOnboarding",
    fallback: "Mặc định Khởi tạo (Onboarding)",
  },
  PLUGIN_RANKING: {
    key: "superadmin.ops.surfacePluginRanking",
    fallback: "Thứ hạng Gợi ý Plugin",
  },
  UX_COPY_LAYOUT: {
    key: "superadmin.ops.surfaceUxCopyLayout",
    fallback: "Nội dung & Bố cục Giao diện",
  },
};

const EXPERIMENT_STATUS_I18N: Record<ExperimentStatus, { key: string; fallback: string }> = {
  DRAFT: { key: "superadmin.ops.expStatusDraft", fallback: "DRAFT" },
  RUNNING: { key: "superadmin.ops.expStatusRunning", fallback: "RUNNING" },
  PAUSED: { key: "superadmin.ops.expStatusPaused", fallback: "PAUSED" },
  KILLED: { key: "superadmin.ops.expStatusKilled", fallback: "KILLED" },
  COMPLETED: { key: "superadmin.ops.expStatusCompleted", fallback: "COMPLETED" },
};

const CARD_STYLE: React.CSSProperties = {
  backgroundColor: "var(--bg-card, var(--bg-surface))",
  border: "1px solid var(--border-default)",
  borderRadius: "10px",
  padding: "16px",
  display: "flex",
  flexDirection: "column",
  gap: "12px",
};

const INPUT_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "8px 10px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "13px",
  boxSizing: "border-box",
};

function canEditExperiments(role?: CloudAdminRole): boolean {
  return role === "SUPER_ADMIN" || role === "SKILL_CURATOR";
}

function statusBadgeStyle(status: string, killSwitch: boolean): { bg: string; fg: string } {
  if (killSwitch || status === "KILLED") {
    return { bg: "var(--state-danger-soft)", fg: "var(--state-danger)" };
  }
  if (status === "RUNNING") {
    return { bg: "var(--state-success-soft)", fg: "var(--state-success)" };
  }
  if (status === "PAUSED") {
    return { bg: "var(--state-warning-soft)", fg: "var(--state-warning)" };
  }
  return { bg: "var(--bg-subtle)", fg: "var(--text-muted)" };
}

function ExperimentCard({
  exp,
  canEdit,
  submitting,
  onKill,
}: {
  exp: AbExperiment;
  canEdit: boolean;
  submitting: boolean;
  onKill: (exp: AbExperiment) => void;
}) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const formatPct = (val: number) => formatNumber(val, Number.isInteger(val) ? 0 : 1);
  const badge = statusBadgeStyle(exp.status, exp.killSwitch);
  const surfaceMeta = TARGET_SURFACE_I18N[exp.targetSurface];
  const effectiveStatus: ExperimentStatus = exp.killSwitch ? "KILLED" : exp.status;
  const statusMeta = EXPERIMENT_STATUS_I18N[effectiveStatus];

  return (
    <article style={CARD_STYLE} data-testid={`ab-exp-card-${exp.experimentKey}`}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
        <div>
          <strong style={{ fontSize: "14px", color: "var(--text-primary)" }}>{exp.title}</strong>
          <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
            {exp.experimentKey} · {t(surfaceMeta.key, surfaceMeta.fallback)} ·{" "}
            {t("superadmin.ops.seqNumberLabel", "chuỗi #{{seq}}", { seq: formatNumber(exp.manifestSeq) })}
          </div>
        </div>
        <div style={{ display: "inline-flex", alignItems: "center", gap: "8px" }}>
          <span style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: badge.bg, color: badge.fg, fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
            {t(statusMeta.key, statusMeta.fallback)}
          </span>
          {canEdit && !exp.killSwitch && exp.status === "RUNNING" && (
            <button
              type="button"
              disabled={submitting}
              data-testid={`ab-kill-btn-${exp.experimentKey}`}
              onClick={() => onKill(exp)}
              style={{
                padding: "4px 10px",
                borderRadius: "6px",
                border: "1px solid var(--state-danger)",
                backgroundColor: "var(--state-danger-soft)",
                color: "var(--state-danger)",
                fontSize: "11px",
                fontWeight: FONT_WEIGHT_BOLD,
                cursor: "pointer",
              }}
            >
              {t("superadmin.ops.killSwitchBtn", "Ngắt khẩn cấp (Kill-Switch)")}
            </button>
          )}
        </div>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: "8px" }}>
        {exp.variants.map((variant) => {
          const attr = exp.attribution.find((a) => a.variantKey === variant.key);
          const convText = `${formatPct(attr?.conversionRatePct ?? 0)}% (${formatNumber(attr?.funnelExport ?? 0)}/${formatNumber(attr?.installs ?? 0)})`;
          const crashText = `${formatPct(attr?.crashRatePct ?? 0)}% (${formatNumber(attr?.crashEvents ?? 0)})`;
          return (
            <div key={variant.key} style={{ padding: "10px", borderRadius: "8px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", fontSize: "12px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", fontWeight: FONT_WEIGHT_BOLD }}>
                <span>{variant.key}</span>
                <span>{formatNumber(variant.weight)}%</span>
              </div>
              <div style={{ marginTop: "6px", color: "var(--text-secondary)", fontSize: "11px" }}>
                {t("superadmin.ops.abFunnelConv", "Chuyển đổi Phễu")}: {convText}
              </div>
              <div style={{ color: attr && attr.crashEvents > 0 ? "var(--state-danger)" : "var(--text-secondary)", fontSize: "11px" }}>
                {t("superadmin.ops.abCrashRate", "Tỷ lệ sự cố")}: {crashText}
              </div>
            </div>
          );
        })}
      </div>
    </article>
  );
}

export const AbExperimentsTab: React.FC<AbExperimentsTabProps> = ({
  experiments,
  signedManifest,
  operatorRole,
  submitting,
  onUpsertExperiment,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const canEdit = canEditExperiments(operatorRole);
  const [experimentKey, setExperimentKey] = useState("");
  const [title, setTitle] = useState("");
  const [targetSurface, setTargetSurface] = useState<ExperimentTargetSurface>("ONBOARDING_DEFAULTS");
  const [splitPreset, setSplitPreset] = useState<"50/50" | "10/90">("50/50");
  const [reason, setReason] = useState("");

  const valid =
    ASCII_IDENTIFIER_REGEX.test(experimentKey.trim()) &&
    title.trim().length >= MIN_TITLE_LEN &&
    reason.trim().length >= MIN_REASON_LEN;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canEdit || !valid) return;
    const controlWeight = splitPreset === "50/50" ? SPLIT_HALF : SPLIT_NINETY;
    const treatmentWeight = splitPreset === "50/50" ? SPLIT_HALF : SPLIT_TEN;
    const ok = await onUpsertExperiment({
      experimentKey: experimentKey.trim(),
      title: title.trim(),
      targetSurface,
      status: "RUNNING",
      killSwitch: false,
      reason: reason.trim(),
      variants: [
        { key: "control", weight: controlWeight, config: { mode: "baseline" } },
        { key: "treatment", weight: treatmentWeight, config: { mode: "variant_b" } },
      ],
    });
    if (ok) {
      setExperimentKey("");
      setTitle("");
      setReason("");
    }
  };

  const handleKill = (exp: AbExperiment) => {
    void onUpsertExperiment({
      experimentKey: exp.experimentKey,
      title: exp.title,
      targetSurface: exp.targetSurface,
      status: "KILLED",
      variants: exp.variants,
      killSwitch: true,
      reason: translateGlobal(
        "superadmin.ops.abKillDefaultReason",
        "Triggered instant Kill-Switch from SuperAdmin A/B console"
      ),
    });
  };

  return (
    <section
      data-ff-id="superadmin-ab-experiments-panel"
      data-testid="superadmin-panel-ab-experiments"
      style={{ display: "flex", flexDirection: "column", gap: "14px" }}
    >
      <div style={CARD_STYLE}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <div style={{ display: "inline-flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="sliders" size={ICON_SIZE_SM} color="var(--action-primary)" />
            <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.ops.abTitle", "Thử nghiệm A/B Xác định & Bảo vệ Phễu Kích hoạt")}
            </h2>
          </div>
          <span style={{ padding: "3px 10px", borderRadius: "999px", backgroundColor: "var(--state-info-soft)", color: "var(--state-info)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
            {signedManifest
              ? t("superadmin.ops.abManifestSeqBadge", "Ed25519 · experiments.json · chuỗi #{{seq}}", {
                  seq: formatNumber(signedManifest.manifestSeq),
                })
              : t("superadmin.ops.abManifestReadyBadge", "Ed25519 · experiments.json · đã sẵn sàng")}
          </span>
        </div>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "8px", fontSize: "11px", color: "var(--text-secondary)" }}>
          <code style={{ padding: "3px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
            hash(install_id + key) % 100
          </code>
          <code style={{ padding: "3px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
            0% PII
          </code>
          <code style={{ padding: "3px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
            {t("superadmin.ops.abSecurityGuard", "Cấm tuyệt đối thử nghiệm nới lỏng rào chắn bảo mật")}
          </code>
        </div>
      </div>

      <form onSubmit={handleSubmit} style={CARD_STYLE} data-testid="ab-create-form">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <strong style={{ fontSize: "13px" }}>
            {t("superadmin.ops.abFormTitle", "Tạo & Xuất bản Thử nghiệm A/B mới")}
          </strong>
          {!canEdit && (
            <span data-testid="ab-readonly-lock" style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.ops.readOnlyBadge", "Chế độ chỉ đọc (RBAC)")}
            </span>
          )}
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: "10px" }}>
          <input type="text" disabled={!canEdit} value={experimentKey} onChange={(e) => setExperimentKey(e.target.value)} data-testid="ab-key-input" placeholder={t("superadmin.ops.abKeyPlaceholder", "Khóa thử nghiệm (vd: onboarding_pack_v2)")} style={INPUT_STYLE} />
          <input type="text" disabled={!canEdit} value={title} onChange={(e) => setTitle(e.target.value)} data-testid="ab-title-input" placeholder={t("superadmin.ops.abTitlePlaceholder", "Tên thử nghiệm A/B")} style={INPUT_STYLE} />
          <select disabled={!canEdit} value={targetSurface} onChange={(e) => setTargetSurface(e.target.value as ExperimentTargetSurface)} data-testid="ab-surface-select" style={INPUT_STYLE}>
            <option value="ONBOARDING_DEFAULTS">{t(TARGET_SURFACE_I18N.ONBOARDING_DEFAULTS.key, TARGET_SURFACE_I18N.ONBOARDING_DEFAULTS.fallback)}</option>
            <option value="PLUGIN_RANKING">{t(TARGET_SURFACE_I18N.PLUGIN_RANKING.key, TARGET_SURFACE_I18N.PLUGIN_RANKING.fallback)}</option>
            <option value="UX_COPY_LAYOUT">{t(TARGET_SURFACE_I18N.UX_COPY_LAYOUT.key, TARGET_SURFACE_I18N.UX_COPY_LAYOUT.fallback)}</option>
          </select>
          <select disabled={!canEdit} value={splitPreset} onChange={(e) => setSplitPreset(e.target.value as "50/50" | "10/90")} data-testid="ab-split-select" style={INPUT_STYLE}>
            <option value="50/50">{t("superadmin.ops.abSplit5050", "Chia nhóm 50 / 50")}</option>
            <option value="10/90">{t("superadmin.ops.abSplit9010", "Chia nhóm 90 / 10 (Canary)")}</option>
          </select>
        </div>
        <input
          type="text"
          disabled={!canEdit}
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          data-testid="ab-reason-input"
          placeholder={t("superadmin.ops.abReasonPlaceholder", "Lý do kiểm toán bắt buộc (>= 5 ký tự)")}
          style={INPUT_STYLE}
        />
        <button
          type="submit"
          disabled={!canEdit || submitting || !valid}
          data-testid="ab-submit-btn"
          style={{
            alignSelf: "flex-start",
            height: "34px",
            padding: "0 14px",
            borderRadius: "8px",
            border: "none",
            backgroundColor: canEdit && valid ? "var(--action-primary)" : "var(--bg-subtle)",
            color: canEdit && valid ? "var(--action-primary-fg)" : "var(--text-disabled)",
            fontSize: "13px",
            fontWeight: FONT_WEIGHT_SEMIBOLD,
            cursor: canEdit && valid ? "pointer" : "not-allowed",
          }}
        >
          {t("superadmin.ops.abSubmitBtn", "Xuất bản Thử nghiệm & Ký Ed25519")}
        </button>
      </form>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(320px, 1fr))", gap: "12px" }}>
        {experiments.length === 0 ? (
          <div style={{ ...CARD_STYLE, color: "var(--text-muted)", fontSize: "13px" }}>
            {t("superadmin.ops.abEmpty", "Chưa có thử nghiệm A/B nào đang hoạt động.")}
          </div>
        ) : (
          experiments.map((exp) => (
            <ExperimentCard key={exp.experimentKey} exp={exp} canEdit={canEdit} submitting={submitting} onKill={handleKill} />
          ))
        )}
      </div>
    </section>
  );
};
