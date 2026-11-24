// @story 05.6
// @implements AC-05 (Cross-Project Heritage Pack Picker & 5 Adaptive Entry Modes)
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { AdaptiveEntryMode, HeritageDesignPack } from "../../lib/validations/cognitive-studio";

const STROKE_MONO = 1.85;
const ICON_SM = 13;
const FONT_WEIGHT_SEMIBOLD = 600;

const ENTRY_MODES: readonly AdaptiveEntryMode[] = [
  "GREENFIELD",
  "ADD_FEATURE",
  "REVAMP",
  "MICRO_POLISH",
  "DECK_FROM_APP",
];

export interface HeritagePackPickerProps {
  packs: HeritageDesignPack[];
  entryMode: AdaptiveEntryMode;
  onChangeEntryMode: (mode: AdaptiveEntryMode) => void;
  onSavePack: (name: string, entryMode: AdaptiveEntryMode) => Promise<void>;
  onApplyPack: (pack: HeritageDesignPack) => void;
}

const HeritagePackList: React.FC<{
  packs: HeritageDesignPack[];
  onApplyPack: (pack: HeritageDesignPack) => void;
}> = ({ packs, onApplyPack }) => {
  if (packs.length === 0) return null;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "4px", maxHeight: "120px", overflowY: "auto" }}>
      {packs.map((pack) => (
        <button
          key={pack.id}
          type="button"
          data-testid={`heritage-pack-item-${pack.id}`}
          onClick={() => onApplyPack(pack)}
          style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "6px 8px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", fontSize: "11.5px", cursor: "pointer", textAlign: "left" }}
        >
          <span>{pack.name}</span>
          <span style={{ fontSize: "10px", color: "var(--text-secondary)" }}>{pack.entryMode}</span>
        </button>
      ))}
    </div>
  );
};

const HeritageSaveForm: React.FC<{
  packName: string;
  isSaving: boolean;
  onChangeName: (v: string) => void;
  onSubmit: () => void;
}> = ({ packName, isSaving, onChangeName, onSubmit }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", gap: "6px" }}>
      <input
        type="text"
        value={packName}
        onChange={(e) => onChangeName(e.target.value)}
        placeholder={t("brandStudio.heritageNamePlaceholder", "Tên Heritage Pack...")}
        data-testid="heritage-pack-name-input"
        style={{ flex: 1, height: "28px", padding: "0 8px", fontSize: "11.5px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)" }}
      />
      <button
        type="button"
        data-testid="save-heritage-pack-btn"
        disabled={!packName.trim() || isSaving}
        onClick={onSubmit}
        style={{ height: "28px", padding: "0 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--action-primary)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}
      >
        {t("brandStudio.saveHeritageBtn", "Lưu Pack")}
      </button>
    </div>
  );
};

export const HeritagePackPicker: React.FC<HeritagePackPickerProps> = ({
  packs,
  entryMode,
  onChangeEntryMode,
  onSavePack,
  onApplyPack,
}) => {
  const { t } = useTranslation();
  const [packName, setPackName] = useState("");
  const [isSaving, setIsSaving] = useState(false);

  const handleSave = async () => {
    const trimmed = packName.trim();
    if (!trimmed || isSaving) return;
    setIsSaving(true);
    try {
      await onSavePack(trimmed, entryMode);
      setPackName("");
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div data-testid="heritage-pack-picker" style={{ border: "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", padding: "10px", backgroundColor: "var(--bg-elevated)", display: "flex", flexDirection: "column", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
        <BrandIcon name="layers" size={ICON_SM} strokeWidth={STROKE_MONO} color="var(--action-primary)" />
        <span>{t("brandStudio.heritageTitle", "Heritage Pack xuyên dự án")}</span>
      </div>
      <select aria-label={t("brandStudio.entryModeLabel", "Chế độ kế thừa")} data-testid="heritage-entry-mode-select" value={entryMode} onChange={(e) => onChangeEntryMode(e.target.value as AdaptiveEntryMode)} style={{ height: "28px", padding: "0 8px", fontSize: "11.5px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)" }}>
        {ENTRY_MODES.map((m) => <option key={m} value={m}>{m}</option>)}
      </select>
      <HeritageSaveForm packName={packName} isSaving={isSaving} onChangeName={setPackName} onSubmit={() => { void handleSave(); }} />
      <HeritagePackList packs={packs} onApplyPack={onApplyPack} />
    </div>
  );
};
