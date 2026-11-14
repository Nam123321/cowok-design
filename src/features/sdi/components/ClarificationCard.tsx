import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { ClarificationVariant } from "../../../lib/validations/sdi";

interface ClarificationCardProps {
  variant: ClarificationVariant;
  isSelected: boolean;
  onSelect: (id: string) => void;
}

export const ClarificationCard: React.FC<ClarificationCardProps> = ({
  variant,
  isSelected,
  onSelect,
}) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      onClick={() => onSelect(variant.id)}
      className={`text-left p-3 rounded-lg border transition-all flex flex-col gap-2 ${
        isSelected
          ? "border-primary bg-primary/10 shadow-sm"
          : "border-border bg-card hover:border-muted-foreground"
      }`}
    >
      <div className="font-semibold text-sm text-foreground">{variant.title}</div>
      <p className="text-xs text-muted-foreground">{variant.description}</p>
      <div className="space-y-1 text-xs">
        <div className="font-medium text-emerald-600 dark:text-emerald-400">{t("sdi.prosLabel", "Ưu điểm:")}</div>
        <ul className="list-disc pl-4 space-y-0.5 text-muted-foreground">
          {variant.pros.map((p, idx) => (
            <li key={`pro-${idx}`}>{p}</li>
          ))}
        </ul>
      </div>
      <div className="space-y-1 text-xs">
        <div className="font-semibold text-foreground">{t("sdi.consLabel", "Đánh đổi:")}</div>
        <ul className="list-disc pl-4 space-y-0.5 text-muted-foreground">
          {variant.cons.map((c, idx) => (
            <li key={`con-${idx}`}>{c}</li>
          ))}
        </ul>
      </div>
    </button>
  );
};
