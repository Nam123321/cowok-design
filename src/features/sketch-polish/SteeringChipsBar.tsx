// @story 05.4
// @implements AC-03 (1-Click impeccable-polish Steering Chips wired to POST /api/v1/patch)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";

export type SteeringChipId =
  | "more-whitespace"
  | "denser-layout"
  | "softer-corners"
  | "sharper-corners"
  | "stronger-type";

interface ChipMeta {
  id: SteeringChipId;
  labelKey: string;
  fallback: string;
}

const STEERING_CHIPS: readonly ChipMeta[] = [
  { id: "more-whitespace", labelKey: "sketchPolish.chipMoreWhitespace", fallback: "Thoáng hơn" },
  { id: "denser-layout", labelKey: "sketchPolish.chipDenserLayout", fallback: "Gọn hơn" },
  { id: "softer-corners", labelKey: "sketchPolish.chipSofterCorners", fallback: "Bo góc mềm" },
  { id: "sharper-corners", labelKey: "sketchPolish.chipSharperCorners", fallback: "Góc sắc nét" },
  { id: "stronger-type", labelKey: "sketchPolish.chipStrongerType", fallback: "Chữ nổi bật" },
];

const ICON_SIZE_SM = 12;
const STROKE_WIDTH_MONO = 1.85;

export interface SteeringChipsBarProps {
  activeChipId?: SteeringChipId | null;
  isApplying?: boolean;
  canUndo?: boolean;
  onSelectChip: (chipId: SteeringChipId) => void;
  onUndo?: () => void;
}

export const SteeringChipsBar: React.FC<SteeringChipsBarProps> = ({
  activeChipId = null,
  isApplying = false,
  canUndo = false,
  onSelectChip,
  onUndo,
}) => {
  const { t } = useTranslation();

  return (
    <div
      className="sp-chip-bar"
      role="toolbar"
      aria-label={t("sketchPolish.steeringChipsLabel", "Tinh chỉnh nhanh")}
      data-ff-id="steering-chips-bar"
      data-testid="steering-chips-bar"
    >
      {STEERING_CHIPS.map((chip) => {
        const isActive = activeChipId === chip.id;
        return (
          <button
            key={chip.id}
            type="button"
            disabled={isApplying}
            aria-pressed={isActive}
            data-testid={`steering-chip-${chip.id}`}
            className={`sp-chip-btn${isActive ? " sp-chip-btn--active" : ""}`}
            onClick={() => onSelectChip(chip.id)}
          >
            <span>{t(chip.labelKey, chip.fallback)}</span>
          </button>
        );
      })}
      {canUndo && onUndo && (
        <button
          type="button"
          className="sp-chip-btn"
          data-testid="steering-undo-btn"
          onClick={onUndo}
        >
          <BrandIcon name="corner-up-left" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />
          <span>{t("sketchPolish.instantUndo", "Hoàn tác")}</span>
        </button>
      )}
    </div>
  );
};
