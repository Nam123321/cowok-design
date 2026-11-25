// @story 14.2
// @implements ADR-008 (Honest Architecture Previews for Story 14.3 Modules & System Governance — Zero Fabricated Data)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import type { SuperAdminPlannedSectionId } from "../navigation";

export interface PlannedModulePanelProps {
  sectionId: SuperAdminPlannedSectionId;
  operatorRole?: CloudAdminRole;
}

interface SpecCardItem {
  id: string;
  icon: string;
  titleKey: string;
  titleFallback: string;
  bodyKey: string;
  bodyFallback: string;
  codeTag?: string;
}

interface PlannedModuleSpec {
  badgeKey: string;
  badgeFallback: string;
  bannerTone: "warning" | "info";
  titleKey: string;
  titleFallback: string;
  summaryKey: string;
  summaryFallback: string;
  contractTag: string;
  cards: readonly SpecCardItem[];
}

const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 15;
const ICON_SIZE_MD = 18;
const BODY_LINE_HEIGHT = 1.55;

const MODULE_SPECS: Record<SuperAdminPlannedSectionId, PlannedModuleSpec> = {
  "plugin-catalog": {
    badgeKey: "superadmin.planned.badgeRoadmap143",
    badgeFallback: "Tính năng sắp ra mắt",
    bannerTone: "warning",
    titleKey: "superadmin.planned.pluginCatalogTitle",
    titleFallback: "Vòng đời Danh mục Plugin Mặc định & Đề xuất",
    summaryKey: "superadmin.planned.pluginCatalogSummary",
    summaryFallback:
      "Quản trị tập trung các kho lưu trữ plugin GitHub mặc định và đề xuất, phân phối qua tệp catalog.json có chữ ký số Ed25519 mà không can thiệp ghi đè cấu hình cục bộ của người dùng.",
    contractTag: "Ed25519 · catalog.json · Local Override > Cloud Default",
    cards: [
      {
        id: "lifecycle",
        icon: "layers",
        titleKey: "superadmin.planned.pluginLifecycleTitle",
        titleFallback: "4 Trạng thái Vòng đời Plugin",
        bodyKey: "superadmin.planned.pluginLifecycleBody",
        bodyFallback: "Hỗ trợ chuyển đổi có kiểm soát giữa ACTIVE / INACTIVE / DEPRECATED / QUARANTINED kèm lý do bắt buộc và bản ghi audit log.",
        codeTag: "ACTIVE | INACTIVE | DEPRECATED | QUARANTINED",
      },
      {
        id: "github-sha",
        icon: "git-branch",
        titleKey: "superadmin.planned.pluginGithubShaTitle",
        titleFallback: "Thêm & Chỉnh sửa Repo GitHub với Commit SHA Cố định",
        bodyKey: "superadmin.planned.pluginGithubShaBody",
        bodyFallback: "Mọi mục plugin đều ghim cố định commit SHA 40 ký tự để ngăn tấn công supply-chain từ tag di động trên nhánh mặc định.",
        codeTag: "owner/repo@<40-char-commit-sha>",
      },
      {
        id: "signed-manifest",
        icon: "shield",
        titleKey: "superadmin.planned.pluginSignedTitle",
        titleFallback: "Phân phối catalog.json Ký số Ed25519",
        bodyKey: "superadmin.planned.pluginSignedBody",
        bodyFallback: "Daemon cục bộ xác minh chữ ký Ed25519 ngoại tuyến trước khi nạp danh sách plugin đề xuất cho người dùng cuối.",
        codeTag: "GET /v1/cloud/catalog.json + .sig",
      },
      {
        id: "precedence",
        icon: "lock",
        titleKey: "superadmin.planned.pluginPrecedenceTitle",
        titleFallback: "Quy tắc Ưu tiên: Local Override > Cloud Default",
        bodyKey: "superadmin.planned.pluginPrecedenceBody",
        bodyFallback: "Mọi plugin do người dùng tự cài đặt hoặc ghim phiên bản tại máy luôn có độ ưu tiên cao hơn cấu hình mặc định từ Cloud.",
        codeTag: "Local Override > Cloud Default",
      },
    ],
  },
  "ab-experiments": {
    badgeKey: "superadmin.planned.badgeRoadmap143",
    badgeFallback: "Tính năng sắp ra mắt",
    bannerTone: "warning",
    titleKey: "superadmin.planned.abTitle",
    titleFallback: "Thử nghiệm A/B Xác định & Bảo vệ Phễu Kích hoạt",
    summaryKey: "superadmin.planned.abSummary",
    summaryFallback:
      "Phân nhóm thử nghiệm hoàn toàn xác định ngay trên máy khách không cần truyền dữ liệu định danh cá nhân, gắn kết trực tiếp với Phễu Kích hoạt 5 bước và cơ chế ngắt khẩn cấp từ Trung tâm Lỗi.",
    contractTag: "hash(install_id + key) % 100 · 0% PII · experiments.json",
    cards: [
      {
        id: "deterministic-hash",
        icon: "sliders",
        titleKey: "superadmin.planned.abHashTitle",
        titleFallback: "Phân nhóm Xác định 0% PII",
        bodyKey: "superadmin.planned.abHashBody",
        bodyFallback: "Tính toán nhánh thử nghiệm cục bộ từ install_id ẩn danh, không thu thập email, IP hay mã định danh người dùng.",
        codeTag: "hash(install_id + key) % 100",
      },
      {
        id: "signed-experiments",
        icon: "shield",
        titleKey: "superadmin.planned.abManifestTitle",
        titleFallback: "Cấu hình experiments.json Ký số Ed25519",
        bodyKey: "superadmin.planned.abManifestBody",
        bodyFallback: "Tệp cấu hình thử nghiệm được ký số Ed25519 để đảm bảo tỷ lệ phân bổ và tham số biến thể không thể bị giả mạo trên đường truyền.",
        codeTag: "GET /v1/cloud/experiments.json + .sig",
      },
      {
        id: "funnel-link",
        icon: "activity",
        titleKey: "superadmin.planned.abFunnelTitle",
        titleFallback: "Liên kết Trực tiếp Phễu Kích hoạt 5 Bước",
        bodyKey: "superadmin.planned.abFunnelBody",
        bodyFallback: "Đo lường chuyển đổi theo từng biến thể qua 5 mốc Day-0 → AI Connected → Project → Preview → Export.",
        codeTag: "5-Step Activation Funnel",
      },
      {
        id: "kill-switch",
        icon: "alert",
        titleKey: "superadmin.planned.abKillSwitchTitle",
        titleFallback: "Ngắt Khẩn cấp (Kill-Switch) từ Trung tâm Lỗi",
        bodyKey: "superadmin.planned.abKillSwitchBody",
        bodyFallback: "Tự động cảnh báo và hỗ trợ ngắt nhanh biến thể khi tỷ lệ sự cố tại Crash Center vượt ngưỡng an toàn.",
        codeTag: "Crash Center Auto Kill-Switch",
      },
    ],
  },
  distribution: {
    badgeKey: "superadmin.planned.badgeRoadmap143",
    badgeFallback: "Tính năng sắp ra mắt",
    bannerTone: "warning",
    titleKey: "superadmin.planned.distTitle",
    titleFallback: "Phát hành Canary 4 Kênh & Trình cập nhật Ed25519",
    summaryKey: "superadmin.planned.distSummary",
    summaryFallback:
      "Điều phối phát hành tăng dần qua 4 kênh Desktop, CLI, Docker và Plugin với kiểm soát theo vòng Canary và xác thực chữ ký Ed25519.",
    contractTag: "5% → 25% → 100% · Ed25519 latest.json",
    cards: [
      {
        id: "canary-rings",
        icon: "deploy",
        titleKey: "superadmin.planned.distRingsTitle",
        titleFallback: "Vòng phát hành 5% → 25% → 100%",
        bodyKey: "superadmin.planned.distRingsBody",
        bodyFallback: "Mở rộng lưu lượng phát hành theo từng nấc an toàn và tạm dừng ngay lập tức khi phát hiện hồi quy.",
        codeTag: "Canary 5% -> 25% -> 100%",
      },
      {
        id: "updater-manifest",
        icon: "shield",
        titleKey: "superadmin.planned.distUpdaterTitle",
        titleFallback: "Tệp latest.json Ký số Ed25519",
        bodyKey: "superadmin.planned.distUpdaterBody",
        bodyFallback: "Mọi bản cập nhật nhị phân đều đính kèm checksum SHA-256 và chữ ký Ed25519 xác minh độc lập trên máy người dùng.",
        codeTag: "latest.json + Ed25519",
      },
    ],
  },
  quarantine: {
    badgeKey: "superadmin.planned.badgeRoadmap143",
    badgeFallback: "Tính năng sắp ra mắt",
    bannerTone: "warning",
    titleKey: "superadmin.planned.quarantineTitle",
    titleFallback: "Bản tin Cách ly Skill Toàn cầu Khẩn cấp",
    summaryKey: "superadmin.planned.quarantineSummary",
    summaryFallback:
      "Phát hành lệnh thu hồi khẩn cấp advisories.json ký Ed25519 để vô hiệu hóa tức thời các skill hoặc plugin bị phát hiện lỗ hổng bảo mật.",
    contractTag: "Emergency Ed25519 advisories.json",
    cards: [
      {
        id: "advisory-feed",
        icon: "shield",
        titleKey: "superadmin.planned.quarantineFeedTitle",
        titleFallback: "Nguồn dữ liệu Thu hồi advisories.json",
        bodyKey: "superadmin.planned.quarantineFeedBody",
        bodyFallback: "Gửi danh sách mã băm SHA-256 cần cách ly đến toàn bộ daemon đang hoạt động với độ trễ thấp.",
        codeTag: "advisories.json · SHA-256 Blocklist",
      },
      {
        id: "audit-gate",
        icon: "lock",
        titleKey: "superadmin.planned.quarantineAuditTitle",
        titleFallback: "Quyền Phát hành & Ghi nhật ký Bắt buộc",
        bodyKey: "superadmin.planned.quarantineAuditBody",
        bodyFallback: "Chỉ SUPER_ADMIN và SKILL_CURATOR được kích hoạt lệnh cách ly, mọi lệnh đều ghi nhận bất biến vào admin_audit_logs.",
        codeTag: "RBAC + admin_audit_logs",
      },
    ],
  },
  licensing: {
    badgeKey: "superadmin.planned.badgeRoadmap143",
    badgeFallback: "Tính năng sắp ra mắt",
    bannerTone: "warning",
    titleKey: "superadmin.planned.licenseTitle",
    titleFallback: "Giấy phép Ed25519 Ngoại tuyến & Liên kết Cowok.ai",
    summaryKey: "superadmin.planned.licenseSummary",
    summaryFallback:
      "Cấp phát giấy phép mật mã Ed25519 hỗ trợ hoạt động hoàn toàn ngoại tuyến và liên kết tổ chức doanh nghiệp qua cowok_org_id.",
    contractTag: "Free / Pro / Team / Enterprise · cowok_org_id",
    cards: [
      {
        id: "license-tiers",
        icon: "lock",
        titleKey: "superadmin.planned.licenseTiersTitle",
        titleFallback: "4 Cấp độ Giấy phép Ngoại tuyến",
        bodyKey: "superadmin.planned.licenseTiersBody",
        bodyFallback: "Phân bổ quyền lợi Free, Pro, Team và Enterprise xác minh cục bộ trong Rust daemon không gây gián đoạn khi mất mạng.",
        codeTag: "Free | Pro | Team | Enterprise",
      },
      {
        id: "org-federation",
        icon: "globe",
        titleKey: "superadmin.planned.licenseFederationTitle",
        titleFallback: "Liên kết Tổ chức Cowok.ai (cowok_org_id)",
        bodyKey: "superadmin.planned.licenseFederationBody",
        bodyFallback: "Gắn kết ghế bản quyền doanh nghiệp với định danh tổ chức cowok_org_id phục vụ đồng bộ chính sách bảo mật.",
        codeTag: "cowok_org_id Federation",
      },
    ],
  },
  governance: {
    badgeKey: "superadmin.planned.badgeGovernanceActive",
    badgeFallback: "Chính sách bảo mật hiện hành",
    bannerTone: "info",
    titleKey: "superadmin.planned.govTitle",
    titleFallback: "Ma trận 4 Vai trò RBAC & Chính sách Nhật ký Kiểm toán Bất biến",
    summaryKey: "superadmin.planned.govSummary",
    summaryFallback:
      "Quy định phân quyền chặt chẽ giữa 4 vai trò quản trị, cơ chế trigger chống sửa/xóa bảng admin_audit_logs và lộ trình triển khai OIDC + PKCE + MFA trong môi trường Production.",
    contractTag: "4-Role RBAC · Append-Only admin_audit_logs · OIDC+PKCE+MFA",
    cards: [
      {
        id: "immutable-audit",
        icon: "lock",
        titleKey: "superadmin.planned.govAuditTitle",
        titleFallback: "Trigger Bất biến trên bảng admin_audit_logs",
        bodyKey: "superadmin.planned.govAuditBody",
        bodyFallback: "Cơ sở dữ liệu chặn hoàn toàn thao tác UPDATE và DELETE trên admin_audit_logs; mọi phản hồi hỗ trợ, gom cụm hay thay đổi cấu hình đều lưu dấu vết vĩnh viễn.",
        codeTag: "BEFORE UPDATE/DELETE -> RAISE ABORT",
      },
      {
        id: "oidc-pkce-rollout",
        icon: "shield",
        titleKey: "superadmin.planned.govOidcTitle",
        titleFallback: "Lộ trình OIDC + PKCE + MFA cho Production",
        bodyKey: "superadmin.planned.govOidcBody",
        bodyFallback: "Chuyển tiếp mượt từ phiên kiểm thử MFA cục bộ sang Google/Okta OIDC + PKCE kết hợp khóa bảo mật WebAuthn/TOTP trên tên miền riêng admin.cowok.design.",
        codeTag: "OIDC + PKCE + WebAuthn/TOTP MFA",
      },
    ],
  },
};

const RBAC_MATRIX_ROWS: ReadonlyArray<{
  role: CloudAdminRole;
  helpdeskKey: string;
  helpdeskFallback: string;
  catalogKey: string;
  catalogFallback: string;
  releaseKey: string;
  releaseFallback: string;
}> = [
  {
    role: "SUPER_ADMIN",
    helpdeskKey: "superadmin.governance.permFull",
    helpdeskFallback: "Toàn quyền (Đọc/Ghi/Phân loại)",
    catalogKey: "superadmin.governance.permFull",
    catalogFallback: "Toàn quyền (Đọc/Ghi/Ký số)",
    releaseKey: "superadmin.governance.permFullRelease",
    releaseFallback: "Toàn quyền (Canary, License, Preview Role)",
  },
  {
    role: "SUPPORT_AGENT",
    helpdeskKey: "superadmin.governance.permSupport",
    helpdeskFallback: "Trả lời 2 chiều & Gom cụm",
    catalogKey: "superadmin.governance.permReadOnly",
    catalogFallback: "Chỉ đọc",
    releaseKey: "superadmin.governance.permReadOnly",
    releaseFallback: "Chỉ đọc",
  },
  {
    role: "SKILL_CURATOR",
    helpdeskKey: "superadmin.governance.permCuratorHelp",
    helpdeskFallback: "Gom cụm tính năng (Chỉ đọc Ticket)",
    catalogKey: "superadmin.governance.permCuratorCatalog",
    catalogFallback: "Quản trị Plugin Catalog, A/B & Cách ly",
    releaseKey: "superadmin.governance.permReadOnly",
    releaseFallback: "Chỉ đọc",
  },
  {
    role: "ANALYST_READONLY",
    helpdeskKey: "superadmin.governance.permReadOnly",
    helpdeskFallback: "Chỉ đọc (Khóa phản hồi)",
    catalogKey: "superadmin.governance.permReadOnly",
    catalogFallback: "Chỉ đọc",
    releaseKey: "superadmin.governance.permReadOnly",
    releaseFallback: "Chỉ đọc (Không thể tự nâng quyền)",
  },
];

function SpecCard({ card }: { card: SpecCardItem }) {
  const { t } = useTranslation();
  return (
    <article
      data-testid={`planned-spec-card-${card.id}`}
      style={{
        backgroundColor: "var(--bg-surface)",
        border: "1px solid var(--border-default)",
        borderRadius: "10px",
        padding: "16px",
        display: "flex",
        flexDirection: "column",
        gap: "8px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name={card.icon} size={ICON_SIZE_SM} color="var(--action-primary)" />
        <h3 style={{ margin: 0, fontSize: "14px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>
          {t(card.titleKey, card.titleFallback)}
        </h3>
      </div>
      <p style={{ margin: 0, fontSize: "13px", lineHeight: BODY_LINE_HEIGHT, color: "var(--text-secondary)" }}>
        {t(card.bodyKey, card.bodyFallback)}
      </p>
      {card.codeTag && (
        <code style={{ alignSelf: "flex-start", padding: "3px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", color: "var(--text-primary)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
          {card.codeTag}
        </code>
      )}
    </article>
  );
}

function GovernanceRbacMatrix({ operatorRole }: { operatorRole?: CloudAdminRole }) {
  const { t } = useTranslation();
  return (
    <section
      data-testid="superadmin-rbac-matrix"
      style={{ backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "10px", padding: "16px", display: "flex", flexDirection: "column", gap: "12px", overflowX: "auto" }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="users" size={ICON_SIZE_MD} color="var(--action-primary)" />
        <h3 style={{ margin: 0, fontSize: "14px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>
          {t("superadmin.governance.matrixTitle", "Ma trận Phân quyền 4 Vai trò (RBAC Enforcement)")}
        </h3>
      </div>
      <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "12px", minWidth: "560px" }}>
        <thead>
          <tr style={{ borderBottom: "1px solid var(--border-default)", textAlign: "left", color: "var(--text-muted)" }}>
            <th style={{ padding: "8px 10px" }}>{t("superadmin.governance.colRole", "Vai trò")}</th>
            <th style={{ padding: "8px 10px" }}>{t("superadmin.governance.colOps", "Hỗ trợ & Cụm tính năng")}</th>
            <th style={{ padding: "8px 10px" }}>{t("superadmin.governance.colCatalog", "Danh mục Plugin, A/B & Cách ly")}</th>
            <th style={{ padding: "8px 10px" }}>{t("superadmin.governance.colRelease", "Canary, Giấy phép & Quản trị")}</th>
          </tr>
        </thead>
        <tbody>
          {RBAC_MATRIX_ROWS.map((row) => {
            const isCurrent = row.role === operatorRole;
            return (
              <tr key={row.role} style={{ borderBottom: "1px solid var(--border-subtle)", backgroundColor: isCurrent ? "var(--bg-selected)" : "transparent" }}>
                <td style={{ padding: "10px", fontWeight: FONT_WEIGHT_BOLD, color: isCurrent ? "var(--action-primary)" : "var(--text-primary)", whiteSpace: "nowrap" }}>
                  {row.role}
                </td>
                <td style={{ padding: "10px", color: "var(--text-secondary)", fontWeight: FONT_WEIGHT_MEDIUM }}>{t(row.helpdeskKey, row.helpdeskFallback)}</td>
                <td style={{ padding: "10px", color: "var(--text-secondary)", fontWeight: FONT_WEIGHT_MEDIUM }}>{t(row.catalogKey, row.catalogFallback)}</td>
                <td style={{ padding: "10px", color: "var(--text-secondary)", fontWeight: FONT_WEIGHT_MEDIUM }}>{t(row.releaseKey, row.releaseFallback)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}

export const PlannedModulePanel: React.FC<PlannedModulePanelProps> = ({ sectionId, operatorRole }) => {
  const { t } = useTranslation();
  const spec = MODULE_SPECS[sectionId];
  const isWarning = spec.bannerTone === "warning";
  const bannerBg = isWarning ? "var(--state-warning-soft)" : "var(--state-info-soft)";
  const bannerFg = isWarning ? "var(--state-warning)" : "var(--state-info)";

  return (
    <section
      data-testid={`superadmin-panel-${sectionId}`}
      style={{ display: "flex", flexDirection: "column", gap: "16px" }}
    >
      <div style={{ backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "10px", padding: "18px", display: "flex", flexDirection: "column", gap: "10px" }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", flexWrap: "wrap" }}>
          <span style={{ padding: "4px 10px", borderRadius: "999px", backgroundColor: bannerBg, color: bannerFg, fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD, display: "inline-flex", alignItems: "center", gap: "6px" }}>
            <BrandIcon name={isWarning ? "info" : "shield"} size={ICON_SIZE_SM} color={bannerFg} />
            <span>{t(spec.badgeKey, spec.badgeFallback)}</span>
          </span>
          <code style={{ padding: "3px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
            {spec.contractTag}
          </code>
        </div>
        <h2 style={{ margin: 0, fontSize: "16px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>
          {t(spec.titleKey, spec.titleFallback)}
        </h2>
        <p style={{ margin: 0, fontSize: "13px", lineHeight: BODY_LINE_HEIGHT, color: "var(--text-secondary)" }}>
          {t(spec.summaryKey, spec.summaryFallback)}
        </p>
      </div>

      {sectionId === "governance" && <GovernanceRbacMatrix operatorRole={operatorRole} />}

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: "12px" }}>
        {spec.cards.map((card) => (
          <SpecCard key={card.id} card={card} />
        ))}
      </div>
    </section>
  );
};
