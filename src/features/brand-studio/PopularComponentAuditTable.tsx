// @story 05.6
// @implements AC-03 (Popular FE/Deck Component Audit Table — DataTableView [5.21] + AdvancedFilterBar [5.22])
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { PopularComponentAuditItem } from "../../lib/validations/cognitive-studio";

type AuditFilter = "all" | "detected_in_sketch" | "suggested_contextual" | "not_needed";
const STROKE_MONO = 1.85;
const ICON_XS = 12;
const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;

export interface PopularComponentAuditTableProps {
  items: PopularComponentAuditItem[];
  onToggleItem: (componentKind: string) => void;
}

function statusLabel(
  status: PopularComponentAuditItem["status"],
  t: (k: string, fb: string) => string
): string {
  switch (status) {
    case "detected_in_sketch":
      return t("brandStudio.statusDetected", "Đã có trong Sketch");
    case "suggested_contextual":
      return t("brandStudio.statusSuggested", "Gợi ý theo ngữ cảnh");
    case "not_needed":
      return t("brandStudio.statusNotNeeded", "Không cần cho dự án này");
    default:
      return t("brandStudio.statusOptional", "Tùy chọn");
  }
}

function filterChipStyle(active: boolean): React.CSSProperties {
  return {
    height: "26px",
    padding: "0 10px",
    borderRadius: "var(--radius-pill)",
    border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
    backgroundColor: active ? "var(--human-soft, var(--bg-elevated))" : "var(--bg-surface)",
    color: active ? "var(--action-primary)" : "var(--text-secondary)",
    fontSize: "11.5px",
    fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
    cursor: "pointer",
  };
}

const AuditFilterToolbar: React.FC<{
  filter: AuditFilter;
  onChangeFilter: (f: AuditFilter) => void;
}> = ({ filter, onChangeFilter }) => {
  const { t } = useTranslation();
  const chips: readonly { id: AuditFilter; label: string }[] = [
    { id: "all", label: t("brandStudio.filterAll", "Tất cả") },
    { id: "detected_in_sketch", label: t("brandStudio.filterDetected", "Đã có") },
    { id: "suggested_contextual", label: t("brandStudio.filterSuggested", "Gợi ý") },
    { id: "not_needed", label: t("brandStudio.filterNotNeeded", "Không cần") },
  ];
  return (
    <div role="toolbar" aria-label={t("brandStudio.auditFilterLabel", "Lọc trạng thái component")} style={{ display: "flex", flexWrap: "wrap", gap: "6px", alignItems: "center" }}>
      {chips.map((chip) => (
        <button key={chip.id} type="button" data-testid={`audit-filter-${chip.id}`} aria-pressed={filter === chip.id} onClick={() => onChangeFilter(chip.id)} style={filterChipStyle(filter === chip.id)}>
          {chip.label}
        </button>
      ))}
    </div>
  );
};

const AuditTableRow: React.FC<{
  item: PopularComponentAuditItem;
  onToggleItem: (componentKind: string) => void;
}> = ({ item, onToggleItem }) => {
  const { t } = useTranslation();
  const isDetected = item.status === "detected_in_sketch";
  return (
    <tr data-testid={`audit-row-${item.componentKind}`} style={{ height: "36px", borderBottom: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)" }}>
      <td style={{ padding: "6px 10px" }}>
        <input type="checkbox" checked={item.selected} aria-label={item.componentKind} data-testid={`audit-toggle-${item.componentKind}`} onChange={() => onToggleItem(item.componentKind)} />
      </td>
      <td style={{ padding: "6px 10px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{item.componentKind}</td>
      <td style={{ padding: "6px 10px" }}>
        <span style={{ display: "inline-flex", alignItems: "center", gap: "4px", fontSize: "11px", color: isDetected ? "var(--action-primary)" : "var(--text-secondary)" }}>
          {isDetected && <BrandIcon name="check" size={ICON_XS} strokeWidth={STROKE_MONO} color="var(--action-primary)" />}
          {statusLabel(item.status, t)}
        </span>
      </td>
      <td style={{ padding: "6px 10px", color: "var(--text-secondary)", fontSize: "11.5px" }}>{item.reason ?? "—"}</td>
    </tr>
  );
};

export const PopularComponentAuditTable: React.FC<PopularComponentAuditTableProps> = ({
  items,
  onToggleItem,
}) => {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<AuditFilter>("all");
  const filtered = filter === "all" ? items : items.filter((i) => i.status === filter);

  return (
    <div data-testid="popular-component-audit" style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <AuditFilterToolbar filter={filter} onChangeFilter={setFilter} />
      <div style={{ border: "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", overflow: "hidden" }}>
        <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "12px" }}>
          <thead>
            <tr style={{ backgroundColor: "var(--bg-elevated)", borderBottom: "1px solid var(--border-default)", textAlign: "left" }}>
              <th style={{ padding: "8px 10px", width: "36px" }} />
              <th style={{ padding: "8px 10px", color: "var(--text-secondary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{t("brandStudio.colComponent", "Component")}</th>
              <th style={{ padding: "8px 10px", color: "var(--text-secondary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{t("brandStudio.colStatus", "Phân loại")}</th>
              <th style={{ padding: "8px 10px", color: "var(--text-secondary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{t("brandStudio.colReason", "Lý do")}</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((item) => (
              <AuditTableRow key={item.componentKind} item={item} onToggleItem={onToggleItem} />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
};
