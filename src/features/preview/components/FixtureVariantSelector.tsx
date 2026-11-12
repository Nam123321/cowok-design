import React from "react";
import { FixtureVariant } from "../../../lib/validations/screen-fixture";

export const FIXTURE_VARIANTS: { variant: FixtureVariant; label: string; badgeClass: string }[] = [
  { variant: "DEFAULT", label: "Default", badgeClass: "bg-blue-500/10 text-blue-600 border-blue-500/20" },
  { variant: "EMPTY", label: "Empty", badgeClass: "bg-secondary text-secondary-foreground border-border" },
  { variant: "LOADING", label: "Loading", badgeClass: "bg-purple-500/10 text-purple-600 border-purple-500/20" },
  { variant: "ERROR", label: "Error", badgeClass: "bg-red-500/10 text-red-600 border-red-500/20" },
];

interface FixtureVariantSelectorProps {
  currentVariant: FixtureVariant;
  onVariantChange: (variant: FixtureVariant) => void;
}

export const FixtureVariantSelector: React.FC<FixtureVariantSelectorProps> = ({
  currentVariant,
  onVariantChange,
}) => {
  return (
    <div className="flex items-center gap-1 bg-muted/60 p-1 rounded-lg border border-border">
      {FIXTURE_VARIANTS.map(({ variant, label, badgeClass }) => {
        const isActive = currentVariant === variant;
        return (
          <button
            key={variant}
            type="button"
            onClick={() => onVariantChange(variant)}
            className={`px-2.5 py-1 text-xs font-medium rounded-md transition-all border ${
              isActive
                ? `${badgeClass} shadow-sm font-semibold`
                : "border-transparent text-muted-foreground hover:text-foreground"
            }`}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
};
