// @story 14.3
// @implements AC-05 (Live Governance Console — 4-Role RBAC Matrix & Immutable admin_audit_logs Stream)
import React, { useMemo, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import type { DistributionAuditLog } from "../../../lib/validations/distribution-ops";

export interface GovernanceAuditTabProps {
  logs: DistributionAuditLog[];
  operatorRole?: CloudAdminRole;
}

const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_MD = 18;

const CARD_STYLE: React.CSSProperties = {
  backgroundColor: "var(--bg-card, var(--bg-surface))",
  border: "1px solid var(--border-default)",
  borderRadius: "10px",
  padding: "16px",
  display: "flex",
  flexDirection: "column",
  gap: "12px",
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

interface RbacMatrixCell {
  key: string;
  fallback: string;
}

const RBAC_MATRIX_ROWS: ReadonlyArray<{
  role: CloudAdminRole;
  ops: RbacMatrixCell;
  catalog: RbacMatrixCell;
  release: RbacMatrixCell;
}> = [
  {
    role: "SUPER_ADMIN",
    ops: { key: "superadmin.governance.permFull", fallback: "Toàn quyền (Đọc/Ghi/Phân loại)" },
    catalog: { key: "superadmin.ops.permSuperCatalog", fallback: "Toàn quyền (Đọc/Ghi/Ký số Ed25519)" },
    release: { key: "superadmin.governance.permFullRelease", fallback: "Toàn quyền (Canary, Giấy phép, Nhật ký)" },
  },
  {
    role: "SUPPORT_AGENT",
    ops: { key: "superadmin.governance.permSupport", fallback: "Trả lời 2 chiều & Gom cụm tính năng" },
    catalog: { key: "superadmin.governance.permReadOnly", fallback: "Chỉ đọc" },
    release: { key: "superadmin.governance.permReadOnly", fallback: "Chỉ đọc" },
  },
  {
    role: "SKILL_CURATOR",
    ops: { key: "superadmin.governance.permCuratorHelp", fallback: "Gom cụm tính năng (Chỉ đọc Ticket)" },
    catalog: { key: "superadmin.governance.permCuratorCatalog", fallback: "Quản trị Danh mục Plugin, A/B & Cách ly" },
    release: { key: "superadmin.governance.permReadOnly", fallback: "Chỉ đọc" },
  },
  {
    role: "ANALYST_READONLY",
    ops: { key: "superadmin.ops.permAnalystOps", fallback: "Chỉ đọc (Khóa phản hồi)" },
    catalog: { key: "superadmin.governance.permReadOnly", fallback: "Chỉ đọc" },
    release: { key: "superadmin.ops.permAnalystRelease", fallback: "Chỉ đọc (Không thể tự nâng quyền)" },
  },
];

export const GovernanceAuditTab: React.FC<GovernanceAuditTabProps> = ({ logs, operatorRole }) => {
  const { t, locale } = useTranslation();
  const { formatDate, formatTime, formatNumber } = useFormatter({ locale });
  const [roleFilter, setRoleFilter] = useState<string>("ALL");

  const filteredLogs = useMemo(() => {
    if (roleFilter === "ALL") return logs;
    return logs.filter((entry) => entry.actorRole === roleFilter);
  }, [logs, roleFilter]);

  return (
    <section
      data-ff-id="superadmin-governance-audit-panel"
      data-testid="superadmin-panel-governance"
      style={{ display: "flex", flexDirection: "column", gap: "14px" }}
    >
      <div style={{ ...CARD_STYLE, overflowX: "auto" }} data-testid="superadmin-rbac-matrix">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <div style={{ display: "inline-flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="users" size={ICON_SIZE_MD} color="var(--action-primary)" />
            <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.governance.matrixTitle", "Ma trận Phân quyền 4 Vai trò (RBAC Enforcement)")}
            </h2>
          </div>
          <code style={{ padding: "3px 8px", borderRadius: "6px", backgroundColor: "var(--state-info-soft)", color: "var(--state-info)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
            {t("superadmin.ops.governanceBadge", "4-Role RBAC · Nhật ký kiểm toán bất biến · OIDC+MFA")}
          </code>
        </div>
        <table style={{ width: "100%", borderCollapse: "collapse", minWidth: "560px" }}>
          <thead>
            <tr>
              <th style={TH_STYLE}>{t("superadmin.governance.colRole", "Vai trò")}</th>
              <th style={TH_STYLE}>{t("superadmin.governance.colOps", "Hỗ trợ & Cụm tính năng")}</th>
              <th style={TH_STYLE}>{t("superadmin.governance.colCatalog", "Danh mục Plugin, A/B & Cách ly")}</th>
              <th style={TH_STYLE}>{t("superadmin.governance.colRelease", "Canary, Giấy phép & Quản trị")}</th>
            </tr>
          </thead>
          <tbody>
            {RBAC_MATRIX_ROWS.map((row) => {
              const isCurrent = row.role === operatorRole;
              return (
                <tr key={row.role} style={{ backgroundColor: isCurrent ? "var(--bg-selected)" : "transparent" }}>
                  <td style={{ ...TD_STYLE, fontWeight: FONT_WEIGHT_BOLD, color: isCurrent ? "var(--action-primary)" : "var(--text-primary)", whiteSpace: "nowrap" }}>
                    {row.role}
                  </td>
                  <td style={{ ...TD_STYLE, color: "var(--text-secondary)", fontWeight: FONT_WEIGHT_MEDIUM }}>
                    {t(row.ops.key, row.ops.fallback)}
                  </td>
                  <td style={{ ...TD_STYLE, color: "var(--text-secondary)", fontWeight: FONT_WEIGHT_MEDIUM }}>
                    {t(row.catalog.key, row.catalog.fallback)}
                  </td>
                  <td style={{ ...TD_STYLE, color: "var(--text-secondary)", fontWeight: FONT_WEIGHT_MEDIUM }}>
                    {t(row.release.key, row.release.fallback)}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <div style={{ ...CARD_STYLE, overflowX: "auto" }} data-testid="governance-audit-log-card">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <div>
            <strong style={{ fontSize: "14px", color: "var(--text-primary)" }}>
              {t("superadmin.ops.auditTitle", "Nhật ký kiểm toán bất biến")} ({formatNumber(filteredLogs.length)})
            </strong>
            <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
              {t("superadmin.ops.auditSubtitle", "Chỉ ghi nối tiếp — chặn hoàn toàn mọi thao tác sửa hoặc xóa lịch sử vận hành.")}
            </div>
          </div>
          <select
            aria-label={t("superadmin.ops.auditRoleFilter", "Lọc theo vai trò")}
            value={roleFilter}
            onChange={(e) => setRoleFilter(e.target.value)}
            data-testid="audit-role-filter"
            style={{
              height: "32px",
              padding: "0 10px",
              borderRadius: "8px",
              border: "1px solid var(--border-default)",
              backgroundColor: "var(--bg-workspace)",
              color: "var(--text-primary)",
              fontSize: "12px",
            }}
          >
            <option value="ALL">{t("superadmin.ops.allRolesOption", "Tất cả vai trò")}</option>
            <option value="SUPER_ADMIN">SUPER_ADMIN</option>
            <option value="SKILL_CURATOR">SKILL_CURATOR</option>
            <option value="SUPPORT_AGENT">SUPPORT_AGENT</option>
            <option value="ANALYST_READONLY">ANALYST_READONLY</option>
          </select>
        </div>

        <table style={{ width: "100%", borderCollapse: "collapse", minWidth: "600px" }} data-testid="governance-audit-table">
          <thead>
            <tr>
              <th style={TH_STYLE}>{t("superadmin.ops.colAction", "Hành động")}</th>
              <th style={TH_STYLE}>{t("superadmin.ops.colTarget", "Tài nguyên")}</th>
              <th style={TH_STYLE}>{t("superadmin.ops.colActor", "Quản trị viên")}</th>
              <th style={TH_STYLE}>MFA</th>
            </tr>
          </thead>
          <tbody>
            {filteredLogs.length === 0 ? (
              <tr>
                <td colSpan={4} style={{ ...TD_STYLE, textAlign: "center", color: "var(--text-muted)" }}>
                  {t("superadmin.ops.auditEmpty", "Chưa có bản ghi kiểm toán nào khớp bộ lọc.")}
                </td>
              </tr>
            ) : (
              filteredLogs.map((entry) => (
                <tr key={entry.id} data-testid={`audit-row-${entry.id}`}>
                  <td style={TD_STYLE}>
                    <strong>{entry.action}</strong>
                    <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                      {formatDate(entry.createdAt)} {formatTime(entry.createdAt)}
                    </div>
                  </td>
                  <td style={TD_STYLE}><code>{entry.targetResource}</code></td>
                  <td style={TD_STYLE}>
                    {entry.actorSubject} · <span style={{ color: "var(--text-muted)" }}>{entry.actorRole}</span>
                  </td>
                  <td style={TD_STYLE}>
                    <span style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: "var(--state-success-soft)", color: "var(--state-success)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
                      {entry.mfaVerified
                        ? t("superadmin.ops.mfaOkBadge", "MFA OK")
                        : t("superadmin.ops.mfaNoBadge", "NO")}
                    </span>
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
