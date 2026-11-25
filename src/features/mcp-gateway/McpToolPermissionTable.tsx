// @story 11.2
// @implements AC-01, AC-03, AC-04 (Chip-Grid 4-Tier MCP Tool Permission Matrix locking HUMAN_ONLY offline with single localized pills)
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { McpPermissionTier, McpToolPolicyDto } from "../../lib/validations/mcp-gateway";

const HEADER_ICON_SIZE = 14;
const PILL_ICON_SIZE = 11;
const FONT_WEIGHT_SEMIBOLD = 600;
const STATUS_DOT_SIZE = 6;

export interface McpToolPermissionTableProps {
  policies: McpToolPolicyDto[];
  busyToolName: string | null;
  onToggle: (policy: McpToolPolicyDto, nextEnabled: boolean) => void;
}

const BUILTIN_TOOL_FALLBACKS: Readonly<Record<string, { title: string; desc: string }>> = {
  get_app_state: {
    title: "Đọc trạng thái không gian thiết kế",
    desc: "Đọc danh sách thành phần trên canvas, phiên bản tài liệu và cảnh báo bố cục.",
  },
  resolve_run_bundle: {
    title: "Phân giải bộ công cụ theo tác vụ",
    desc: "Chọn sẵn tập công cụ được phép sử dụng cho từng loại tác vụ thiết kế.",
  },
  query_predicate: {
    title: "Truy vấn thành phần theo điều kiện",
    desc: "Tìm nhanh các thành phần theo bộ lọc như chữ bị tràn hoặc loại thẻ.",
  },
  banner_solver: {
    title: "Tối ưu bố cục lưới Banner",
    desc: "Tự động tính toán bố cục banner không chồng chéo theo 15 mẫu lưới chuẩn.",
  },
  grounded_diagram_elk: {
    title: "Sắp xếp sơ đồ theo tầng",
    desc: "Tự động bố trí sơ đồ kiến trúc nhiều tầng gọn gàng và nhất quán.",
  },
  apply_batch: {
    title: "Đề xuất chỉnh sửa hàng loạt",
    desc: "Áp dụng nhóm thay đổi canvas an toàn kèm xem trước và tự động hoàn tác khi lỗi.",
  },
  ephemeral_tool_forge: {
    title: "Tạo kịch bản trợ giúp tạm thời",
    desc: "Tạo kịch bản hỗ trợ trong vùng cách ly; yêu cầu xác nhận trực tiếp trên ứng dụng.",
  },
  learn_card_promote: {
    title: "Phê duyệt quy tắc thiết kế đã học",
    desc: "Duyệt và đưa các quy tắc thiết kế mới vào bộ quy chuẩn chính thức.",
  },
  staging_plugin_promote: {
    title: "Kích hoạt tiện ích từ vùng thử nghiệm",
    desc: "Đưa tiện ích đã kiểm duyệt từ vùng thử nghiệm vào không gian làm việc chính.",
  },
  workspace_destructive_reset: {
    title: "Đặt lại toàn bộ không gian làm việc",
    desc: "Xóa và khôi phục trạng thái mặc định của toàn bộ không gian thiết kế.",
  },
};

const TIER_META: Readonly<
  Record<Exclude<McpPermissionTier, "HUMAN_ONLY">, { key: string; fallback: string; style: React.CSSProperties }>
> = {
  READ_ONLY: {
    key: "mcpGateway.tierReadOnly",
    fallback: "Chỉ đọc",
    style: {
      backgroundColor: "var(--bg-subtle)",
      color: "var(--text-secondary)",
      border: "1px solid var(--border-subtle)",
    },
  },
  INSPECT: {
    key: "mcpGateway.tierInspect",
    fallback: "Kiểm tra",
    style: {
      backgroundColor: "var(--bg-subtle)",
      color: "var(--text-primary)",
      border: "1px solid var(--border-default)",
    },
  },
  REVISE_PROPOSE: {
    key: "mcpGateway.tierRevisePropose",
    fallback: "Đề xuất sửa",
    style: {
      backgroundColor: "var(--state-warning-soft)",
      color: "var(--state-warning)",
      border: "1px solid var(--state-warning)",
    },
  },
};

const HumanOnlyLockPill: React.FC<{ toolName: string }> = ({ toolName }) => {
  const { t } = useTranslation();
  return (
    <span
      data-testid={`mcp-human-only-lock-${toolName}`}
      title={t(
        "mcpGateway.humanOnlyLockedHint",
        "Công cụ này bị khóa an toàn và chỉ có thể thực hiện trực tiếp trong ứng dụng Cowok-Design."
      )}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "5px",
        padding: "3px 9px",
        borderRadius: "var(--radius-pill)",
        fontSize: "10px",
        fontWeight: FONT_WEIGHT_SEMIBOLD,
        backgroundColor: "var(--state-danger-soft)",
        color: "var(--state-danger)",
        border: "1px solid var(--state-danger)",
        whiteSpace: "nowrap",
      }}
    >
      <BrandIcon name="shield-alert" size={PILL_ICON_SIZE} color="var(--state-danger)" />
      <span>{t("mcpGateway.tierHumanOnly", "Chỉ người dùng thao tác")}</span>
    </span>
  );
};

const ToolToggleChip: React.FC<{
  policy: McpToolPolicyDto;
  busy: boolean;
  onToggle: (policy: McpToolPolicyDto, nextEnabled: boolean) => void;
}> = ({ policy, busy, onToggle }) => {
  const { t } = useTranslation();
  const active = policy.enabled;
  return (
    <button
      type="button"
      role="switch"
      aria-checked={active}
      data-testid={`mcp-tool-toggle-${policy.toolName}`}
      disabled={busy}
      onClick={() => onToggle(policy, !active)}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "5px",
        height: "24px",
        padding: "0 10px",
        borderRadius: "var(--radius-pill)",
        fontSize: "10px",
        fontWeight: FONT_WEIGHT_SEMIBOLD,
        border: active ? "1px solid var(--state-success)" : "1px solid var(--border-default)",
        backgroundColor: active ? "var(--state-success-soft)" : "var(--bg-subtle)",
        color: active ? "var(--state-success)" : "var(--text-secondary)",
        cursor: busy ? "wait" : "pointer",
        transition: "background-color 120ms ease, border-color 120ms ease, color 120ms ease",
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: `${STATUS_DOT_SIZE}px`,
          height: `${STATUS_DOT_SIZE}px`,
          borderRadius: "50%",
          backgroundColor: active ? "var(--state-success)" : "var(--text-muted)",
        }}
      />
      <span>{active ? t("mcpGateway.toolEnabled", "Đang bật") : t("mcpGateway.toolDisabled", "Đang tắt")}</span>
    </button>
  );
};

const CARD_BASE_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  justifyContent: "space-between",
  gap: "8px",
  padding: "10px 12px",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-surface)",
  transition: "border-color 120ms ease, background-color 120ms ease",
};

const PolicyCardFooter: React.FC<{
  policy: McpToolPolicyDto;
  isHumanOnly: boolean;
  busy: boolean;
  onToggle: (policy: McpToolPolicyDto, nextEnabled: boolean) => void;
}> = ({ policy, isHumanOnly, busy, onToggle }) => {
  const { t } = useTranslation();
  if (isHumanOnly) {
    return (
      <div style={{ display: "flex", alignItems: "center", paddingTop: "2px" }}>
        <HumanOnlyLockPill toolName={policy.toolName} />
      </div>
    );
  }
  const tierInfo = TIER_META[policy.permissionTier as Exclude<McpPermissionTier, "HUMAN_ONLY">];
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", flexWrap: "wrap", paddingTop: "2px" }}>
      <span
        style={{
          fontSize: "10px",
          fontWeight: FONT_WEIGHT_SEMIBOLD,
          padding: "2px 8px",
          borderRadius: "var(--radius-pill)",
          display: "inline-flex",
          alignItems: "center",
          ...tierInfo.style,
        }}
      >
        {t(tierInfo.key, tierInfo.fallback)}
      </span>
      <ToolToggleChip policy={policy} busy={busy} onToggle={onToggle} />
    </div>
  );
};

const PolicyChipCard: React.FC<{
  policy: McpToolPolicyDto;
  busy: boolean;
  onToggle: (policy: McpToolPolicyDto, nextEnabled: boolean) => void;
}> = ({ policy, busy, onToggle }) => {
  const { t } = useTranslation();
  const [hovered, setHovered] = useState(false);
  const isHumanOnly = policy.lockedHumanOnly || policy.permissionTier === "HUMAN_ONLY";
  const fallback = BUILTIN_TOOL_FALLBACKS[policy.toolName];
  const title = t(`mcpGateway.tools.${policy.toolName}.title`, fallback?.title ?? policy.toolName);
  const desc = t(`mcpGateway.tools.${policy.toolName}.desc`, fallback?.desc ?? policy.description);

  return (
    <div
      data-testid={`mcp-policy-row-${policy.toolName}`}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{ ...CARD_BASE_STYLE, border: `1px solid ${hovered ? "var(--border-strong)" : "var(--border-subtle)"}` }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: "3px" }}>
        <div style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", lineHeight: "1.35" }}>
          {title}
        </div>
        <div style={{ fontSize: "10px", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
          {policy.toolName}
        </div>
        <div style={{ fontSize: "11px", color: "var(--text-secondary)", lineHeight: "1.45", marginTop: "2px" }}>
          {desc}
        </div>
      </div>
      <PolicyCardFooter policy={policy} isHumanOnly={isHumanOnly} busy={busy} onToggle={onToggle} />
    </div>
  );
};

export const McpToolPermissionTable: React.FC<McpToolPermissionTableProps> = ({
  policies,
  busyToolName,
  onToggle,
}) => {
  const { t } = useTranslation();
  return (
    <section
      data-ff-id="mcp-tool-permission-matrix"
      aria-label={t("mcpGateway.permissionMatrixTitle", "Phân quyền Công cụ MCP")}
      style={{ display: "flex", flexDirection: "column", gap: "10px" }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="shield-check" size={HEADER_ICON_SIZE} color="var(--text-primary)" />
          <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
            {t("mcpGateway.permissionMatrixTitle", "Phân quyền Công cụ MCP")}
          </span>
        </div>
        <span style={{ fontSize: "10.5px", color: "var(--text-muted)" }}>
          {t("mcpGateway.permissionMatrixSubtitle", "Kiểm soát công cụ được phép gọi từ ứng dụng AI bên ngoài")}
        </span>
      </div>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(290px, 1fr))",
          gap: "10px",
        }}
      >
        {policies.map((policy) => (
          <PolicyChipCard
            key={policy.toolName}
            policy={policy}
            busy={busyToolName === policy.toolName}
            onToggle={onToggle}
          />
        ))}
      </div>
    </section>
  );
};
