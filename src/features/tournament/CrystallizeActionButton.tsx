// @story 10.3
// @implements AC-03 (1-Click /crystallize Action Packaging into Starter Template / Brand Kit / Skill Plugin)
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { CrystallizePackageKind } from "../../lib/validations/tournament";

const FONT_SEMIBOLD = 600;
const DISABLED_OPACITY = 0.5;
const LAYERS_ICON_SIZE = 13;

export interface CrystallizeActionButtonProps {
  defaultName: string;
  disabled?: boolean;
  isCrystallizing: boolean;
  onCrystallize: (kind: CrystallizePackageKind, name: string) => void;
}

const CONTAINER_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "8px",
  flexWrap: "wrap",
  padding: "12px 14px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-md)",
};

const INPUT_STYLE: React.CSSProperties = {
  padding: "7px 10px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-card)",
  color: "var(--text-primary)",
  fontSize: "12px",
};

const SUBMIT_BTN_BASE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "6px",
  padding: "8px 14px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-selected)",
  color: "var(--action-primary)",
  fontSize: "12.5px",
  fontWeight: FONT_SEMIBOLD,
  whiteSpace: "nowrap",
};

const CrystallizeKindSelect: React.FC<{
  value: CrystallizePackageKind;
  onChange: (next: CrystallizePackageKind) => void;
}> = ({ value, onChange }) => {
  const { t } = useTranslation();
  return (
    <select
      aria-label={t("tournament.crystallizeKindAria", "Loại gói kết tinh")}
      data-testid="crystallize-kind-select"
      value={value}
      onChange={(e) => onChange(e.target.value as CrystallizePackageKind)}
      style={INPUT_STYLE}
    >
      <option value="STARTER_TEMPLATE">{t("tournament.kindStarter", "Mẫu khởi đầu")}</option>
      <option value="BRAND_KIT">{t("tournament.kindBrandKit", "Bộ nhận diện thương hiệu")}</option>
      <option value="SKILL_PLUGIN">{t("tournament.kindSkillPlugin", "Gói kỹ năng tái sử dụng")}</option>
    </select>
  );
};

export const CrystallizeActionButton: React.FC<CrystallizeActionButtonProps> = ({
  defaultName,
  disabled = false,
  isCrystallizing,
  onCrystallize,
}) => {
  const { t } = useTranslation();
  const [packageKind, setPackageKind] = useState<CrystallizePackageKind>("STARTER_TEMPLATE");
  const [name, setName] = useState<string>(
    defaultName || t("tournament.defaultPackageName", "Mẫu giao diện mới")
  );
  const submitDisabled = disabled || isCrystallizing || !name.trim();

  return (
    <div style={CONTAINER_STYLE}>
      <CrystallizeKindSelect value={packageKind} onChange={setPackageKind} />
      <input
        type="text"
        aria-label={t("tournament.crystallizeNameAria", "Tên gói mẫu")}
        data-testid="crystallize-name-input"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder={t("tournament.crystallizeNamePlaceholder", "Nhập tên gói tái sử dụng")}
        style={{ ...INPUT_STYLE, flex: 1, minWidth: "160px" }}
      />
      <button
        type="button"
        disabled={submitDisabled}
        data-ff-id="crystallize-action-button"
        data-testid="crystallize-action-button"
        onClick={() => onCrystallize(packageKind, name)}
        style={{
          ...SUBMIT_BTN_BASE,
          cursor: disabled || isCrystallizing ? "not-allowed" : "pointer",
          opacity: disabled ? DISABLED_OPACITY : 1,
        }}
      >
        <BrandIcon name="layers" size={LAYERS_ICON_SIZE} color="currentColor" />
        <span>
          {isCrystallizing
            ? t("tournament.crystallizingBtn", "Đang đóng gói...")
            : t("tournament.crystallizeBtn", "Kết tinh thành mẫu")}
        </span>
      </button>
    </div>
  );
};
