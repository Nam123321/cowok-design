// @story 05.1
// @implements Zero-Tailwind Semantic Pure CSS Clarification Card
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { ClarificationVariant } from "../../../lib/validations/sdi";

const FONT_WEIGHT_SEMIBOLD = 600;

interface ClarificationCardProps {
  variant: ClarificationVariant;
  isSelected: boolean;
  onSelect: (id: string) => void;
}

const CARD_BASE_STYLE: React.CSSProperties = {
  textAlign: "left",
  padding: "12px",
  borderRadius: "var(--radius-md)",
  transition: "all var(--duration-fast) var(--ease-standard)",
  display: "flex",
  flexDirection: "column",
  gap: "8px",
  cursor: "pointer",
  width: "100%",
  boxSizing: "border-box",
};

interface VariantBulletSectionProps {
  label: string;
  color: string;
  items: string[];
  prefix: string;
}

const VariantBulletSection: React.FC<VariantBulletSectionProps> = ({
  label,
  color,
  items,
  prefix,
}) => (
  <div style={{ fontSize: "11px", display: "flex", flexDirection: "column", gap: "4px" }}>
    <div style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, color }}>{label}</div>
    <ul
      style={{
        margin: 0,
        paddingLeft: "16px",
        color: "var(--text-muted)",
        display: "flex",
        flexDirection: "column",
        gap: "2px",
      }}
    >
      {items.map((item, idx) => (
        <li key={`${prefix}-${idx}`}>{item}</li>
      ))}
    </ul>
  </div>
);

export const ClarificationCard: React.FC<ClarificationCardProps> = ({
  variant,
  isSelected,
  onSelect,
}) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      data-testid={`clarification-card-${variant.id}`}
      onClick={() => onSelect(variant.id)}
      style={{
        ...CARD_BASE_STYLE,
        border: isSelected ? "2px solid var(--action-primary)" : "1px solid var(--border-default)",
        backgroundColor: isSelected ? "var(--bg-selected)" : "var(--bg-surface)",
        boxShadow: isSelected ? "var(--shadow-sm)" : "none",
      }}
    >
      <div style={{ fontSize: "13px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
        {variant.title}
      </div>
      <p style={{ margin: 0, fontSize: "11px", color: "var(--text-muted)", lineHeight: "1.4" }}>
        {variant.description}
      </p>
      <VariantBulletSection label={t("sdi.prosLabel", "Ưu điểm:")} color="var(--state-success)" items={variant.pros} prefix="pro" />
      <VariantBulletSection label={t("sdi.consLabel", "Đánh đổi:")} color="var(--text-primary)" items={variant.cons} prefix="con" />
    </button>
  );
};
