// @story 14.2
// @implements AC-02, ADR-008 (Dedicated OIDC SSO + 6-Digit MFA SuperAdmin Login Screen)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import type { SuperAdminAuthForm } from "../../../features/superadmin/useSuperAdminController";

export interface AdminLoginGateProps {
  form: SuperAdminAuthForm;
  pending: boolean;
  onChange: (next: SuperAdminAuthForm) => void;
  onSignIn: () => void;
}

const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_XS = 13;
const ICON_SIZE_SM = 15;
const ICON_SIZE_LG = 24;
const MFA_MAX_LEN = 6;
const BODY_LINE_HEIGHT = 1.5;

const WRAPPER_STYLE: React.CSSProperties = {
  minHeight: "100vh",
  width: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  padding: "24px 16px",
  backgroundColor: "var(--bg-canvas)",
  boxSizing: "border-box",
};

const CARD_STYLE: React.CSSProperties = {
  width: "100%",
  maxWidth: "520px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "14px",
  padding: "24px",
  display: "flex",
  flexDirection: "column",
  gap: "18px",
  boxSizing: "border-box",
};

const FIELD_LABEL_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "6px",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  color: "var(--text-secondary)",
};

const INPUT_STYLE: React.CSSProperties = {
  height: "38px",
  padding: "0 12px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "13px",
  width: "100%",
  boxSizing: "border-box",
};

const SUBMIT_BTN_STYLE: React.CSSProperties = {
  height: "40px",
  borderRadius: "8px",
  border: "none",
  backgroundColor: "var(--action-primary)",
  color: "var(--action-primary-fg)",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  fontSize: "13px",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "8px",
};

const ROLE_SUMMARIES: ReadonlyArray<{ role: CloudAdminRole; key: string; fallback: string }> = [
  { role: "SUPER_ADMIN", key: "superadmin.auth.roleSuperAdmin", fallback: "Toàn quyền điều phối, chuyển vai trò xem trước & phát hành" },
  { role: "SUPPORT_AGENT", key: "superadmin.auth.roleSupportAgent", fallback: "Phản hồi hỗ trợ 2 chiều & phân loại sự cố" },
  { role: "SKILL_CURATOR", key: "superadmin.auth.roleSkillCurator", fallback: "Quản trị cụm tính năng, danh mục plugin & thử nghiệm A/B" },
  { role: "ANALYST_READONLY", key: "superadmin.auth.roleAnalystReadonly", fallback: "Chỉ đọc KPI, phễu kích hoạt & nhật ký (khóa thao tác ghi)" },
];

function LoginHeader() {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", flexWrap: "wrap" }}>
        <div style={{ display: "inline-flex", alignItems: "center", gap: "10px" }}>
          <span style={{ width: "40px", height: "40px", borderRadius: "10px", backgroundColor: "var(--bg-selected)", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
            <BrandIcon name="shield" size={ICON_SIZE_LG} color="var(--action-primary)" />
          </span>
          <div>
            <h1 style={{ margin: 0, fontSize: "18px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>
              {t("superadmin.header.title", "Cổng Quản trị Cloud")}
            </h1>
            <div style={{ fontSize: "12px", color: "var(--text-muted)" }}>
              {t("superadmin.auth.planeTag", "admin.cowok.design · Cổng quản trị độc lập")}
            </div>
          </div>
        </div>
        <span style={{ padding: "4px 10px", borderRadius: "999px", backgroundColor: "var(--state-info-soft)", color: "var(--state-info)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
          {t("superadmin.auth.ssoBadge", "OIDC + MFA")}
        </span>
      </div>
      <p style={{ margin: 0, fontSize: "13px", lineHeight: BODY_LINE_HEIGHT, color: "var(--text-secondary)" }}>
        {t("superadmin.auth.gateSubtitle", "Cổng quản trị tách biệt hoàn toàn với không gian làm việc người dùng. Yêu cầu xác thực OIDC SSO kèm mã MFA 6 chữ số cho mọi phiên truy cập.")}
      </p>
    </div>
  );
}

function RoleMfaFields({ form, onChange }: { form: SuperAdminAuthForm; onChange: (next: SuperAdminAuthForm) => void }) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(190px, 1fr))", gap: "12px" }}>
      <label style={FIELD_LABEL_STYLE}>
        <span>{t("superadmin.auth.roleLabel", "Vai trò RBAC")}</span>
        <select aria-label={t("superadmin.auth.roleLabel", "Vai trò RBAC")} data-testid="oidc-role-select" value={form.role} onChange={(e) => onChange({ ...form, role: e.target.value as CloudAdminRole })} style={INPUT_STYLE}>
          <option value="SUPER_ADMIN">SUPER_ADMIN</option>
          <option value="SUPPORT_AGENT">SUPPORT_AGENT</option>
          <option value="SKILL_CURATOR">SKILL_CURATOR</option>
          <option value="ANALYST_READONLY">ANALYST_READONLY</option>
        </select>
      </label>
      <label style={FIELD_LABEL_STYLE}>
        <span>{t("superadmin.auth.mfaLabel", "Mã xác thực MFA (6 chữ số)")}</span>
        <input type="text" required inputMode="numeric" maxLength={MFA_MAX_LEN} data-testid="oidc-mfa-input" value={form.mfaCode} onChange={(e) => onChange({ ...form, mfaCode: e.target.value })} placeholder={t("superadmin.auth.mfaPlaceholder", "Nhập mã MFA 6 chữ số")} style={INPUT_STYLE} />
      </label>
    </div>
  );
}

function LoginFormFields({ form, onChange }: { form: SuperAdminAuthForm; onChange: (next: SuperAdminAuthForm) => void }) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <label style={FIELD_LABEL_STYLE}>
        <span>{t("superadmin.auth.emailLabel", "Email quản trị viên (OIDC SSO)")}</span>
        <input type="email" required data-testid="oidc-email-input" value={form.email} onChange={(e) => onChange({ ...form, email: e.target.value })} placeholder="ops-admin@cowok.design" style={INPUT_STYLE} />
      </label>
      <RoleMfaFields form={form} onChange={onChange} />
    </div>
  );
}

function SecurityPolicyFooter() {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "10px", paddingTop: "12px", borderTop: "1px solid var(--border-subtle)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
        <BrandIcon name="lock" size={ICON_SIZE_XS} color="var(--state-success)" />
        <span>{t("superadmin.auth.rbacAuditTitle", "Bảo vệ bởi 4 vai trò RBAC & Nhật ký kiểm toán bất biến")}</span>
      </div>
      <div style={{ display: "grid", gap: "6px" }}>
        {ROLE_SUMMARIES.map((item) => (
          <div key={item.role} style={{ display: "flex", alignItems: "baseline", gap: "8px", fontSize: "11px", color: "var(--text-secondary)", lineHeight: BODY_LINE_HEIGHT }}>
            <code style={{ padding: "1px 6px", borderRadius: "5px", backgroundColor: "var(--bg-subtle)", color: "var(--text-primary)", fontWeight: FONT_WEIGHT_SEMIBOLD, whiteSpace: "nowrap" }}>
              {item.role}
            </code>
            <span>{t(item.key, item.fallback)}</span>
          </div>
        ))}
      </div>
      <div style={{ padding: "8px 10px", borderRadius: "8px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", fontSize: "11px", color: "var(--text-muted)", fontWeight: FONT_WEIGHT_MEDIUM, lineHeight: BODY_LINE_HEIGHT }}>
        {t("superadmin.auth.auditNotice", "Mọi thao tác ghi đều được lưu vào bảng admin_audit_logs chỉ-thêm (không thể sửa/xóa) kèm danh tính OIDC và trạng thái MFA.")}
      </div>
    </div>
  );
}

export const AdminLoginGate: React.FC<AdminLoginGateProps> = ({ form, pending, onChange, onSignIn }) => {
  const { t } = useTranslation();
  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    onSignIn();
  };

  return (
    <section style={WRAPPER_STYLE} aria-label={t("superadmin.auth.gateTitle", "Đăng nhập Quản trị OIDC + MFA")}>
      <form onSubmit={handleSubmit} style={CARD_STYLE} data-testid="superadmin-oidc-gate">
        <LoginHeader />
        <LoginFormFields form={form} onChange={onChange} />
        <button type="submit" disabled={pending} data-testid="oidc-signin-btn" style={{ ...SUBMIT_BTN_STYLE, cursor: pending ? "wait" : "pointer" }}>
          <BrandIcon name="shield" size={ICON_SIZE_SM} color="currentColor" />
          <span>{t("superadmin.auth.signInBtn", "Xác thực MFA và Đăng nhập")}</span>
        </button>
        <SecurityPolicyFooter />
      </form>
    </section>
  );
};
