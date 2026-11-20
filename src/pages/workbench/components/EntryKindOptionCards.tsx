// @story 04.1, 06.1
// @implements AC-06 (Unified Intent Selector Cards: Create App vs Create Presentation)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

const ICON_CARD_SIZE = 13;
const WEIGHT_SEMIBOLD = 600;
const WEIGHT_MEDIUM = 500;

export interface EntryKindOptionCardsProps {
  selectedKind?: "fe-dag" | "presentation";
  onSelect: (kind: "fe-dag" | "presentation") => void;
  testIdPrefix?: string;
  label?: string;
}

const buildIntentCardStyle = (isSelected: boolean, accentToken: string): React.CSSProperties => ({
  padding: "10px 12px",
  borderRadius: "var(--radius-sm)",
  textAlign: "left",
  cursor: "pointer",
  border: isSelected ? `1.5px solid ${accentToken}` : "1px solid var(--border-default)",
  backgroundColor: isSelected ? "var(--bg-selected)" : "var(--bg-workspace)",
  color: "var(--text-primary)",
  display: "flex",
  flexDirection: "column",
  gap: "4px",
  transition: "all var(--duration-fast) var(--ease-standard)",
});

export const EntryKindOptionCards: React.FC<EntryKindOptionCardsProps> = ({
  selectedKind,
  onSelect,
  testIdPrefix = "new-convo-entry",
  label,
}) => {
  const { t } = useTranslation();
  const options: ReadonlyArray<{
    kind: "fe-dag" | "presentation";
    icon: "workflow" | "layers";
    accent: string;
    title: string;
    desc: string;
  }> = [
    {
      kind: "fe-dag",
      icon: "workflow",
      accent: "var(--human-accent)",
      title: t("workbench.entryKindFeDag", "Tạo Ứng dụng"),
      desc: t(
        "workbench.entryKindFeDagDesc",
        "Thiết kế giao diện màn hình, luồng tương tác và chuyển hướng."
      ),
    },
    {
      kind: "presentation",
      icon: "layers",
      accent: "var(--agent-accent)",
      title: t("workbench.entryKindPresentation", "Tạo Presentation"),
      desc: t(
        "workbench.entryKindPresentationDesc",
        "Tạo bộ slide 16:9 đồng bộ thương hiệu và trình chiếu trực tiếp."
      ),
    },
  ];

  return (
    <div>
      {label && (
        <div
          style={{
            fontSize: "11.5px",
            fontWeight: WEIGHT_SEMIBOLD,
            color: "var(--text-secondary)",
            marginBottom: "6px",
          }}
        >
          {label}
        </div>
      )}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
        {options.map((opt) => {
          const isSelected = selectedKind === opt.kind;
          return (
            <button
              key={opt.kind}
              type="button"
              data-testid={`${testIdPrefix}-${opt.kind}`}
              aria-pressed={isSelected}
              onClick={() => onSelect(opt.kind)}
              style={buildIntentCardStyle(isSelected, opt.accent)}
            >
              <div
                style={{
                  fontSize: "12px",
                  fontWeight: WEIGHT_SEMIBOLD,
                  display: "flex",
                  alignItems: "center",
                  gap: "6px",
                  color: "var(--text-primary)",
                }}
              >
                <BrandIcon name={opt.icon} size={ICON_CARD_SIZE} color={opt.accent} />
                <span>{opt.title}</span>
              </div>
              <div
                style={{
                  fontSize: "10.5px",
                  fontWeight: WEIGHT_MEDIUM,
                  color: "var(--text-muted)",
                  lineHeight: "1.4",
                }}
              >
                {opt.desc}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
};
