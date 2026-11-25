// @story 14.3
// @implements AC-04, AC-05 (Offline-Verifiable Ed25519 License Issuance, Cowok.ai Organization Federation & 1-Click Offline Verification Inspector)
import React, { useState } from "react";
import {
  inspectOfflineLicenseToken,
  type OfflineLicenseInspectionResult,
} from "../../../api/distribution-ops";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import type {
  IssueLicenseRequest,
  OfflineLicenseRecord,
  OfflineLicenseTier,
} from "../../../lib/validations/distribution-ops";

export interface LicenseFederationTabProps {
  licenses: OfflineLicenseRecord[];
  keyId: string;
  publicKeyBase64: string;
  operatorRole?: CloudAdminRole;
  submitting: boolean;
  onIssueLicense: (input: IssueLicenseRequest) => Promise<boolean>;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 15;
const MIN_ORG_LEN = 2;
const DEFAULT_SEATS = 10;
const DEFAULT_VALID_DAYS = 365;

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

function LicenseVerificationCard({
  licenseId,
  result,
}: {
  licenseId: string;
  result: OfflineLicenseInspectionResult;
}) {
  const { t } = useTranslation();
  const isOk = result.validFormat && result.signatureVerified && !result.expired;
  return (
    <div
      style={{
        ...CARD_STYLE,
        borderColor: isOk ? "var(--state-success)" : "var(--state-danger)",
      }}
      data-testid="license-offline-verify-preview"
    >
      <strong style={{ fontSize: "13px", color: isOk ? "var(--state-success)" : "var(--state-danger)" }}>
        {isOk
          ? t("superadmin.ops.licenseVerifyPass", "Xác minh chữ ký Ed25519 ngoại tuyến hợp lệ (100% không cần mạng)")
          : t("superadmin.ops.licenseVerifyFail", "Khóa bản quyền không hợp lệ, bị chỉnh sửa hoặc đã hết hạn")}
      </strong>
      <div style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
        {t("superadmin.ops.licenseVerifyIdLabel", "ID")}: <code>{licenseId}</code> ·{" "}
        {t("superadmin.ops.keyLabel", "Khóa")}: <code>{result.keyId ?? "—"}</code> ·{" "}
        {t("superadmin.ops.sigLabel", "Chữ ký")}: <code>{result.signaturePreview ?? "—"}…</code>
        {result.reason && (
          <span>
            {" "}· {t("superadmin.ops.reasonLabel", "Lý do")}: <code>{result.reason}</code>
          </span>
        )}
        {result.payload?.cowokOrgId && (
          <span>
            {" "}· {t("superadmin.ops.cowokOrgLabel", "Cowok.ai Org")}: <strong>{result.payload.cowokOrgId}</strong>
          </span>
        )}
      </div>
    </div>
  );
}

export const LicenseFederationTab: React.FC<LicenseFederationTabProps> = ({
  licenses,
  keyId,
  publicKeyBase64,
  operatorRole,
  submitting,
  onIssueLicense,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const canEdit = operatorRole === "SUPER_ADMIN";
  const [tier, setTier] = useState<OfflineLicenseTier>("PRO");
  const [subjectOrg, setSubjectOrg] = useState("");
  const [cowokOrgId, setCowokOrgId] = useState("");
  const [maxSeats, setMaxSeats] = useState<number>(DEFAULT_SEATS);
  const [validDays, setValidDays] = useState<number>(DEFAULT_VALID_DAYS);
  const [inspection, setInspection] = useState<{
    licenseId: string;
    result: OfflineLicenseInspectionResult;
  } | null>(null);
  const [verifyingId, setVerifyingId] = useState<string | null>(null);

  const valid = subjectOrg.trim().length >= MIN_ORG_LEN && maxSeats >= 1 && validDays >= 1;

  const handleVerifyOffline = async (lic: OfflineLicenseRecord) => {
    setVerifyingId(lic.licenseId);
    try {
      const result = await inspectOfflineLicenseToken(lic.signedToken, publicKeyBase64);
      setInspection({ licenseId: lic.licenseId, result });
    } finally {
      setVerifyingId((curr) => (curr === lic.licenseId ? null : curr));
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canEdit || !valid) return;
    const ok = await onIssueLicense({
      tier,
      subjectOrg: subjectOrg.trim(),
      cowokOrgId: cowokOrgId.trim() || null,
      maxSeats,
      validDays,
    });
    if (ok) {
      setSubjectOrg("");
      setCowokOrgId("");
    }
  };

  return (
    <section
      data-ff-id="superadmin-licensing-panel"
      data-testid="superadmin-panel-licensing"
      style={{ display: "flex", flexDirection: "column", gap: "14px" }}
    >
      <div style={CARD_STYLE}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <div style={{ display: "inline-flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="lock" size={ICON_SIZE_SM} color="var(--action-primary)" />
            <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.ops.licenseTitle", "Giấy phép Ngoại tuyến Ed25519 & Liên kết Tổ chức Cowok.ai")}
            </h2>
          </div>
          <span style={{ padding: "3px 10px", borderRadius: "999px", backgroundColor: "var(--state-info-soft)", color: "var(--state-info)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
            Free · Pro · Team · Enterprise
          </span>
        </div>
        {publicKeyBase64 && (
          <code data-testid="license-public-key" style={{ fontSize: "11px", padding: "6px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)", overflowWrap: "anywhere" }}>
            {t("superadmin.ops.publicKeyLabel", "Khóa công khai ({{keyId}})", { keyId })}: {publicKeyBase64}
          </code>
        )}
      </div>

      <form onSubmit={handleSubmit} style={CARD_STYLE} data-testid="license-issue-form">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <strong style={{ fontSize: "13px" }}>
            {t("superadmin.ops.licenseFormTitle", "Cấp Khóa Bản quyền Xác thực Ngoại tuyến")}
          </strong>
          {!canEdit && (
            <span data-testid="license-readonly-lock" style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.ops.licenseSuperAdminOnly", "Chỉ SUPER_ADMIN được cấp khóa")}
            </span>
          )}
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(190px, 1fr))", gap: "10px" }}>
          <select disabled={!canEdit} value={tier} onChange={(e) => setTier(e.target.value as OfflineLicenseTier)} data-testid="license-tier-select" style={INPUT_STYLE}>
            <option value="FREE">{t("superadmin.ops.tierFree", "FREE")}</option>
            <option value="PRO">{t("superadmin.ops.tierPro", "PRO (Desktop Pro)")}</option>
            <option value="TEAM">{t("superadmin.ops.tierTeam", "TEAM (Nhóm)")}</option>
            <option value="ENTERPRISE">{t("superadmin.ops.tierEnterprise", "ENTERPRISE (Docker & Tổ chức)")}</option>
          </select>
          <input type="text" disabled={!canEdit} value={subjectOrg} onChange={(e) => setSubjectOrg(e.target.value)} data-testid="license-org-input" placeholder={t("superadmin.ops.licenseOrgPlaceholder", "Tên tổ chức / khách hàng")} style={INPUT_STYLE} />
          <input type="text" disabled={!canEdit} value={cowokOrgId} onChange={(e) => setCowokOrgId(e.target.value)} data-testid="license-cowok-org-input" placeholder={t("superadmin.ops.cowokOrgPlaceholder", "Mã liên kết cowok_org_id (tùy chọn)")} style={INPUT_STYLE} />
          <input type="number" min={1} disabled={!canEdit} value={maxSeats} onChange={(e) => setMaxSeats(Number(e.target.value) || 1)} data-testid="license-seats-input" placeholder={t("superadmin.ops.licenseSeatsPlaceholder", "Số ghế (seats)")} style={INPUT_STYLE} />
          <input type="number" min={1} max={3650} disabled={!canEdit} value={validDays} onChange={(e) => setValidDays(Number(e.target.value) || DEFAULT_VALID_DAYS)} data-testid="license-days-input" placeholder={t("superadmin.ops.licenseDaysPlaceholder", "Thời hạn (ngày)")} style={INPUT_STYLE} />
        </div>
        <button
          type="submit"
          disabled={!canEdit || submitting || !valid}
          data-testid="license-submit-btn"
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
          {t("superadmin.ops.licenseSubmitBtn", "Cấp & Ký số Ed25519")}
        </button>
      </form>

      {inspection && (
        <LicenseVerificationCard licenseId={inspection.licenseId} result={inspection.result} />
      )}

      <div style={{ ...CARD_STYLE, overflowX: "auto" }}>
        <strong style={{ fontSize: "13px" }}>
          {t("superadmin.ops.licenseListTitle", "Danh sách Giấy phép đã cấp")} ({formatNumber(licenses.length)})
        </strong>
        <table style={{ width: "100%", borderCollapse: "collapse", minWidth: "600px" }} data-testid="licenses-table">
          <thead>
            <tr>
              <th style={TH_STYLE}>{t("superadmin.ops.colOrg", "Tổ chức")}</th>
              <th style={TH_STYLE}>{t("superadmin.ops.colTier", "Gói")}</th>
              <th style={TH_STYLE}>cowok_org_id</th>
              <th style={TH_STYLE}>{t("superadmin.ops.colSeats", "Số ghế")}</th>
              <th style={TH_STYLE}>{t("superadmin.ops.colVerify", "Xác minh ngoại tuyến")}</th>
            </tr>
          </thead>
          <tbody>
            {licenses.length === 0 ? (
              <tr>
                <td colSpan={5} style={{ ...TD_STYLE, textAlign: "center", color: "var(--text-muted)" }}>
                  {t("superadmin.ops.licenseEmpty", "Chưa có giấy phép ngoại tuyến nào được cấp.")}
                </td>
              </tr>
            ) : (
              licenses.map((lic) => (
                <tr key={lic.licenseId} data-testid={`license-row-${lic.licenseId}`}>
                  <td style={TD_STYLE}>
                    <strong>{lic.subjectOrg}</strong>
                    <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>{lic.licenseId}</div>
                  </td>
                  <td style={TD_STYLE}>
                    <span style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: "var(--state-info-soft)", color: "var(--state-info)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
                      {lic.tier}
                    </span>
                  </td>
                  <td style={TD_STYLE}>{lic.cowokOrgId ?? "—"}</td>
                  <td style={TD_STYLE}>{formatNumber(lic.maxSeats)}</td>
                  <td style={TD_STYLE}>
                    <button
                      type="button"
                      disabled={verifyingId === lic.licenseId}
                      data-testid={`license-verify-btn-${lic.licenseId}`}
                      onClick={() => void handleVerifyOffline(lic)}
                      style={{
                        padding: "4px 10px",
                        borderRadius: "6px",
                        border: "1px solid var(--border-default)",
                        backgroundColor: "var(--state-success-soft)",
                        color: "var(--state-success)",
                        fontSize: "11px",
                        fontWeight: FONT_WEIGHT_BOLD,
                        cursor: verifyingId === lic.licenseId ? "wait" : "pointer",
                      }}
                    >
                      {t("superadmin.ops.verifyOfflineBtn", "Kiểm tra ngoại tuyến")}
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
};
