// @story 14.2
// @implements ADR-008 (SuperAdmin Portal 3-Group Navigation & RBAC Metadata)
import type { CloudAdminRole } from "../../lib/validations/cloud-admin";

export type SuperAdminLiveSectionId = "helpdesk" | "clusters" | "funnel" | "crashes";
export type SuperAdminPlannedSectionId =
  | "plugin-catalog"
  | "ab-experiments"
  | "distribution"
  | "quarantine"
  | "licensing"
  | "governance";
export type SuperAdminSectionId = SuperAdminLiveSectionId | SuperAdminPlannedSectionId;
export type SuperAdminNavGroupId = "operations" | "growth-distribution" | "system-governance";
export type SuperAdminNavStatus = "live" | "roadmap" | "policy";

export interface SuperAdminNavItem {
  id: SuperAdminSectionId;
  groupId: SuperAdminNavGroupId;
  icon: string;
  labelKey: string;
  labelFallback: string;
  subtitleKey: string;
  subtitleFallback: string;
  status: SuperAdminNavStatus;
  badge?: string;
  badgeKey?: string;
  allowedRoles: readonly CloudAdminRole[];
  writeRoles: readonly CloudAdminRole[];
}

export interface SuperAdminNavGroup {
  id: SuperAdminNavGroupId;
  titleKey: string;
  titleFallback: string;
  items: readonly SuperAdminNavItem[];
}

export const ALL_ADMIN_ROLES: readonly CloudAdminRole[] = [
  "SUPER_ADMIN",
  "SUPPORT_AGENT",
  "SKILL_CURATOR",
  "ANALYST_READONLY",
];

const OPS_ITEMS: readonly SuperAdminNavItem[] = [
  {
    id: "helpdesk",
    groupId: "operations",
    icon: "message",
    labelKey: "superadmin.tabs.helpdesk",
    labelFallback: "Hộp thư Hỗ trợ",
    subtitleKey: "superadmin.nav.helpdeskDesc",
    subtitleFallback: "Đồng bộ phản hồi 2 chiều và xử lý yêu cầu hỗ trợ người dùng",
    status: "live",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: ["SUPER_ADMIN", "SUPPORT_AGENT"],
  },
  {
    id: "clusters",
    groupId: "operations",
    icon: "layers",
    labelKey: "superadmin.tabs.clusters",
    labelFallback: "Cụm Yêu cầu Tính năng",
    subtitleKey: "superadmin.nav.clustersDesc",
    subtitleFallback: "Gom cụm đề xuất tính năng và liên kết nhanh GitHub Issue",
    status: "live",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: ["SUPER_ADMIN", "SUPPORT_AGENT", "SKILL_CURATOR"],
  },
  {
    id: "funnel",
    groupId: "operations",
    icon: "activity",
    labelKey: "superadmin.tabs.funnel",
    labelFallback: "Phễu Kích hoạt",
    subtitleKey: "superadmin.nav.funnelDesc",
    subtitleFallback: "Phân tích 5 bước kích hoạt, 4 kênh phát hành và 5 ứng dụng tích hợp",
    status: "live",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: [],
  },
  {
    id: "crashes",
    groupId: "operations",
    icon: "alert",
    labelKey: "superadmin.tabs.crashes",
    labelFallback: "Trung tâm Lỗi",
    subtitleKey: "superadmin.nav.crashesDesc",
    subtitleFallback: "Phân loại nhóm sự cố đã ẩn danh hóa và theo dõi mức độ ảnh hưởng",
    status: "live",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: ["SUPER_ADMIN", "SUPPORT_AGENT"],
  },
];

const GROWTH_ITEMS: readonly SuperAdminNavItem[] = [
  {
    id: "plugin-catalog",
    groupId: "growth-distribution",
    icon: "plugin",
    labelKey: "superadmin.tabs.pluginCatalog",
    labelFallback: "Danh mục Plugin GitHub",
    subtitleKey: "superadmin.nav.pluginCatalogDesc",
    subtitleFallback: "Quản lý vòng đời Plugin mặc định & đề xuất kèm commit SHA cố định",
    status: "roadmap",
    badgeKey: "superadmin.shell.badgeComingSoon",
    badge: "Sắp ra mắt",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: ["SUPER_ADMIN", "SKILL_CURATOR"],
  },
  {
    id: "ab-experiments",
    groupId: "growth-distribution",
    icon: "sliders",
    labelKey: "superadmin.tabs.abExperiments",
    labelFallback: "Thử nghiệm A/B",
    subtitleKey: "superadmin.nav.abExperimentsDesc",
    subtitleFallback: "Phân nhóm xác định không chứa PII, kết nối Phễu Kích hoạt & Ngắt khẩn cấp",
    status: "roadmap",
    badgeKey: "superadmin.shell.badgeComingSoon",
    badge: "Sắp ra mắt",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: ["SUPER_ADMIN", "SKILL_CURATOR"],
  },
  {
    id: "distribution",
    groupId: "growth-distribution",
    icon: "deploy",
    labelKey: "superadmin.tabs.distribution",
    labelFallback: "Phát hành Canary",
    subtitleKey: "superadmin.nav.distributionDesc",
    subtitleFallback: "Lộ trình phát hành 4 kênh (5% → 25% → 100%) ký số Ed25519",
    status: "roadmap",
    badgeKey: "superadmin.shell.badgeComingSoon",
    badge: "Sắp ra mắt",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: ["SUPER_ADMIN"],
  },
  {
    id: "quarantine",
    groupId: "growth-distribution",
    icon: "shield",
    labelKey: "superadmin.tabs.quarantine",
    labelFallback: "Cách ly Skill Toàn cầu",
    subtitleKey: "superadmin.nav.quarantineDesc",
    subtitleFallback: "Thu hồi khẩn cấp skill/plugin rủi ro qua bản tin advisories.json ký Ed25519",
    status: "roadmap",
    badgeKey: "superadmin.shell.badgeComingSoon",
    badge: "Sắp ra mắt",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: ["SUPER_ADMIN", "SKILL_CURATOR"],
  },
  {
    id: "licensing",
    groupId: "growth-distribution",
    icon: "lock",
    labelKey: "superadmin.tabs.licensing",
    labelFallback: "Giấy phép & Tổ chức",
    subtitleKey: "superadmin.nav.licensingDesc",
    subtitleFallback: "Giấy phép Ed25519 ngoại tuyến và liên kết tổ chức Cowok.ai",
    status: "roadmap",
    badgeKey: "superadmin.shell.badgeComingSoon",
    badge: "Sắp ra mắt",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: ["SUPER_ADMIN"],
  },
];

const GOVERNANCE_ITEMS: readonly SuperAdminNavItem[] = [
  {
    id: "governance",
    groupId: "system-governance",
    icon: "users",
    labelKey: "superadmin.tabs.governance",
    labelFallback: "Phân quyền & Nhật ký Kiểm toán",
    subtitleKey: "superadmin.nav.governanceDesc",
    subtitleFallback: "Ma trận 4 vai trò RBAC, chính sách admin_audit_logs bất biến và OIDC + MFA",
    status: "policy",
    badge: "RBAC",
    allowedRoles: ALL_ADMIN_ROLES,
    writeRoles: ["SUPER_ADMIN"],
  },
];

export const SUPERADMIN_NAV_GROUPS: readonly SuperAdminNavGroup[] = [
  {
    id: "operations",
    titleKey: "superadmin.nav.groupOperations",
    titleFallback: "Vận hành & Hỗ trợ",
    items: OPS_ITEMS,
  },
  {
    id: "growth-distribution",
    titleKey: "superadmin.nav.groupGrowth",
    titleFallback: "Danh mục, A/B & Phát hành",
    items: GROWTH_ITEMS,
  },
  {
    id: "system-governance",
    titleKey: "superadmin.nav.groupGovernance",
    titleFallback: "Quản trị Hệ thống & Bảo mật",
    items: GOVERNANCE_ITEMS,
  },
];

export const SUPERADMIN_NAV_ITEMS: readonly SuperAdminNavItem[] = SUPERADMIN_NAV_GROUPS.flatMap(
  (group) => group.items
);

const LIVE_SECTION_SET = new Set<string>(["helpdesk", "clusters", "funnel", "crashes"]);

export function isLiveSection(id: SuperAdminSectionId): id is SuperAdminLiveSectionId {
  return LIVE_SECTION_SET.has(id);
}

export function findNavSection(id: SuperAdminSectionId): SuperAdminNavItem {
  return SUPERADMIN_NAV_ITEMS.find((item) => item.id === id) ?? OPS_ITEMS[0];
}

export function canRoleEditSection(item: SuperAdminNavItem, role?: CloudAdminRole): boolean {
  return Boolean(role && item.writeRoles.includes(role));
}
