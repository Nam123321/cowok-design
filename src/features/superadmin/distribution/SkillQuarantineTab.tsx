// @story 14.3
// @implements AC-02, AC-05 (Global Emergency Skill Quarantine Advisory Console — Mandatory Audit Reason & Signed Ed25519 advisories.json)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import type {
  AdvisoryAction,
  AdvisorySeverity,
  IssueQuarantineRequest,
  SignedAdvisoriesManifest,
  SkillAdvisory,
} from "../../../lib/validations/distribution-ops";

export interface SkillQuarantineTabProps {
  advisories: SkillAdvisory[];
  signedManifest: SignedAdvisoriesManifest | null;
  operatorRole?: CloudAdminRole;
  submitting: boolean;
  onIssueQuarantine: (input: IssueQuarantineRequest) => Promise<boolean>;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 15;
const MIN_REASON_LEN = 5;

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

const TH_STYLE: React.CSSProperties = {
  textAlign: "left",
  padding: "8px 10px",
  fontSize: "12px",
  color: "var(--text-muted)",
  borderBottom: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-subtle)",
};

const TD_STYLE: React.CSSProperties = {
  padding: "9px 10px",
  fontSize: "12px",
  color: "var(--text-primary)",
  borderBottom: "1px solid var(--border-subtle)",
};

function severityPalette(severity: AdvisorySeverity): { bg: string; fg: string } {
  if (severity === "CRITICAL") {
    return { bg: "var(--state-danger-soft)", fg: "var(--state-danger)" };
  }
  if (severity === "HIGH") {
    return { bg: "var(--state-warning-soft)", fg: "var(--state-warning)" };
  }
  return { bg: "var(--state-info-soft)", fg: "var(--state-info)" };
}

export const SkillQuarantineTab: React.FC<SkillQuarantineTabProps> = ({
  advisories,
  signedManifest,
  operatorRole,
  submitting,
  onIssueQuarantine,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const canEdit = operatorRole === "SUPER_ADMIN" || operatorRole === "SKILL_CURATOR";
  const [skillOrPluginId, setSkillOrPluginId] = useState("");
  const [severity, setSeverity] = useState<AdvisorySeverity>("CRITICAL");
  const [action, setAction] = useState<AdvisoryAction>("QUARANTINE");
  const [reason, setReason] = useState("");
  const [affectedSha256, setAffectedSha256] = useState("");

  const valid = skillOrPluginId.trim().length > 0 && reason.trim().length >= MIN_REASON_LEN;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canEdit || !valid) return;
    const ok = await onIssueQuarantine({
      skillOrPluginId: skillOrPluginId.trim(),
      severity,
      action,
      reason: reason.trim(),
      affectedSha256: affectedSha256.trim() || null,
    });
    if (ok) {
      setSkillOrPluginId("");
      setReason("");
      setAffectedSha256("");
    }
  };

  return (
    <section
      data-ff-id="superadmin-quarantine-panel"
      data-testid="superadmin-panel-quarantine"
      style={{ display: "flex", flexDirection: "column", gap: "14px" }}
    >
      <div style={CARD_STYLE}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <div style={{ display: "inline-flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="shield" size={ICON_SIZE_SM} color="var(--state-danger)" />
            <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.ops.quarantineTitle", "Cách ly Khẩn cấp Skill & Plugin Toàn cầu")}
            </h2>
          </div>
          <span style={{ padding: "3px 10px", borderRadius: "999px", backgroundColor: "var(--state-danger-soft)", color: "var(--state-danger)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
            {signedManifest
              ? t("superadmin.ops.quarantineManifestSeqBadge", "Ed25519 · advisories.json · chuỗi #{{seq}}", {
                  seq: formatNumber(signedManifest.manifestSeq),
                })
              : t("superadmin.ops.quarantineManifestReadyBadge", "Ed25519 · advisories.json · đã sẵn sàng")}
          </span>
        </div>
        <p style={{ margin: 0, fontSize: "12px", color: "var(--text-secondary)" }}>
          {t(
            "superadmin.ops.quarantineSummary",
            "Lệnh cách ly toàn cục có hiệu lực bảo vệ ưu tiên cao nhất trên máy khách nhằm chặn các skill/plugin phát hiện lỗ hổng chuỗi cung ứng. Mọi lệnh đều bắt buộc ghi lý do kiểm toán."
          )}
        </p>
      </div>

      <form onSubmit={handleSubmit} style={CARD_STYLE} data-testid="quarantine-issue-form">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <strong style={{ fontSize: "13px" }}>
            {t("superadmin.ops.quarantineFormTitle", "Phát hành Bản tin Cách ly / Cảnh báo")}
          </strong>
          {!canEdit && (
            <span data-testid="quarantine-readonly-lock" style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.ops.readOnlyBadge", "Chế độ chỉ đọc (RBAC)")}
            </span>
          )}
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: "10px" }}>
          <input type="text" disabled={!canEdit} value={skillOrPluginId} onChange={(e) => setSkillOrPluginId(e.target.value)} data-testid="quarantine-skill-id-input" placeholder={t("superadmin.ops.quarantineSkillPlaceholder", "Mã skill hoặc plugin cần cách ly")} style={INPUT_STYLE} />
          <select disabled={!canEdit} value={severity} onChange={(e) => setSeverity(e.target.value as AdvisorySeverity)} data-testid="quarantine-severity-select" style={INPUT_STYLE}>
            <option value="CRITICAL">{t("superadmin.ops.severityCritical", "CRITICAL (Khẩn cấp)")}</option>
            <option value="HIGH">{t("superadmin.ops.severityHigh", "HIGH (Nghiêm trọng)")}</option>
            <option value="MEDIUM">{t("superadmin.ops.severityMedium", "MEDIUM (Trung bình)")}</option>
          </select>
          <select disabled={!canEdit} value={action} onChange={(e) => setAction(e.target.value as AdvisoryAction)} data-testid="quarantine-action-select" style={INPUT_STYLE}>
            <option value="QUARANTINE">{t("superadmin.ops.actionQuarantine", "QUARANTINE (Khóa toàn cầu)")}</option>
            <option value="WARN">{t("superadmin.ops.actionWarn", "WARN (Cảnh báo người dùng)")}</option>
            <option value="REVOKE_QUARANTINE">{t("superadmin.ops.actionRevokeQuarantine", "REVOKE_QUARANTINE (Gỡ cách ly)")}</option>
          </select>
          <input type="text" disabled={!canEdit} value={affectedSha256} onChange={(e) => setAffectedSha256(e.target.value)} data-testid="quarantine-sha-input" placeholder={t("superadmin.ops.quarantineShaPlaceholder", "SHA-256 bị ảnh hưởng (tùy chọn)")} style={INPUT_STYLE} />
        </div>
        <textarea
          rows={2}
          disabled={!canEdit}
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          data-testid="quarantine-reason-input"
          placeholder={t("superadmin.ops.quarantineReasonPlaceholder", "Lý do bắt buộc phát hành lệnh cách ly (>= 5 ký tự, ghi nhật ký kiểm toán)")}
          style={INPUT_STYLE}
        />
        <button
          type="submit"
          disabled={!canEdit || submitting || !valid}
          data-testid="quarantine-submit-btn"
          style={{
            alignSelf: "flex-start",
            height: "34px",
            padding: "0 14px",
            borderRadius: "8px",
            border: "none",
            backgroundColor: canEdit && valid ? "var(--state-danger)" : "var(--bg-subtle)",
            color: canEdit && valid ? "var(--action-primary-fg)" : "var(--text-disabled)",
            fontSize: "13px",
            fontWeight: FONT_WEIGHT_SEMIBOLD,
            cursor: canEdit && valid ? "pointer" : "not-allowed",
          }}
        >
          {t("superadmin.ops.quarantineSubmitBtn", "Phát hành Lệnh Cách ly Toàn cầu")}
        </button>
      </form>

      <div style={{ ...CARD_STYLE, overflowX: "auto" }}>
        <strong style={{ fontSize: "13px" }}>
          {t("superadmin.ops.quarantineListTitle", "Danh sách Bản tin Cách ly đã ký số")} ({formatNumber(advisories.length)})
        </strong>
        <table style={{ width: "100%", borderCollapse: "collapse", minWidth: "580px" }} data-testid="quarantine-advisories-table">
          <thead>
            <tr>
              <th style={TH_STYLE}>{t("superadmin.ops.colSkillOrPlugin", "Skill / Plugin")}</th>
              <th style={TH_STYLE}>{t("superadmin.ops.colSeverity", "Mức độ")}</th>
              <th style={TH_STYLE}>{t("superadmin.ops.colAction", "Hành động")}</th>
              <th style={TH_STYLE}>{t("superadmin.ops.colReason", "Lý do kiểm toán")}</th>
              <th style={TH_STYLE}>{t("superadmin.ops.colActor", "Người phát hành")}</th>
            </tr>
          </thead>
          <tbody>
            {advisories.length === 0 ? (
              <tr>
                <td colSpan={5} style={{ ...TD_STYLE, textAlign: "center", color: "var(--text-muted)" }}>
                  {t("superadmin.ops.quarantineEmpty", "Không có skill hay plugin nào đang bị cách ly.")}
                </td>
              </tr>
            ) : (
              advisories.map((adv) => {
                const pal = severityPalette(adv.severity);
                return (
                  <tr key={adv.id} data-testid={`advisory-row-${adv.skillOrPluginId}`}>
                    <td style={TD_STYLE}>
                      <strong>{adv.skillOrPluginId}</strong>
                      <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                        {t("superadmin.ops.seqNumberLabel", "chuỗi #{{seq}}", { seq: formatNumber(adv.advisorySeq) })}
                      </div>
                    </td>
                    <td style={TD_STYLE}>
                      <span style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: pal.bg, color: pal.fg, fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
                        {adv.severity}
                      </span>
                    </td>
                    <td style={TD_STYLE}>{adv.action}</td>
                    <td style={TD_STYLE}>{adv.reason}</td>
                    <td style={TD_STYLE}>{adv.issuedBy}</td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
};
