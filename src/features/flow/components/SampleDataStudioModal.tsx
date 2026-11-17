// @story 05.1
// @implements Sample Data Studio (Collapsible Tray/Modal with DataTableView & AdvancedFilterBar for 4 Presets)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

export type SampleDataPreset = "HAPPY" | "EMPTY" | "LONG_TEXT" | "VALIDATION_ERROR";

export interface SampleDataStudioModalProps {
  isOpen: boolean;
  onClose: () => void;
  activePreset?: SampleDataPreset;
  onApplyFixture?: (preset: SampleDataPreset, fixtureJson: string) => void;
}

interface DataRow {
  key: string;
  type: string;
  value: string;
}

const MODAL_Z_INDEX = 50;
const ICON_HEADER_SIZE = 16;
const ICON_SEARCH_SIZE = 12;
const ICON_PLAY_SIZE = 11;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const PRESET_KEYS: readonly SampleDataPreset[] = ["HAPPY", "EMPTY", "LONG_TEXT", "VALIDATION_ERROR"];

const PRESET_DATA: Record<SampleDataPreset, { label: string; badge: string; data: DataRow[] }> = {
  HAPPY: { label: "Happy Path", badge: "200 OK", data: [{ key: "user.id", type: "UUID", value: "usr_9981a2f" }, { key: "user.name", type: "String", value: "Alexander Nguyen" }, { key: "account.balance", type: "Currency", value: "$124,500.00 USD" }, { key: "auth.webauthnPasskey", type: "Boolean", value: "true (Enrolled)" }, { key: "transaction.status", type: "Enum", value: "SETTLED" }] },
  EMPTY: { label: "Empty State", badge: "0 Records", data: [{ key: "user.id", type: "UUID", value: "usr_new_001" }, { key: "account.balance", type: "Currency", value: "$0.00 USD" }, { key: "transactions.items", type: "Array", value: "[] (Empty)" }, { key: "notifications.count", type: "Number", value: "0" }] },
  LONG_TEXT: { label: "Long Text / Edge Case", badge: "Overflow Test", data: [{ key: "organization.legalName", type: "String (256ch)", value: "International Trans-Continental Consolidated Asset Management Corporation Limited Vietnam Branch" }, { key: "memo.content", type: "String (1024ch)", value: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua." }, { key: "beneficiary.address", type: "String", value: "Apartment 48B, Floor 19, Landmark 81 Tower, Binh Thanh District, Ho Chi Minh City, 700000, Vietnam" }] },
  VALIDATION_ERROR: { label: "Validation Error", badge: "422 Unprocessable", data: [{ key: "error.code", type: "String", value: "ERR_INSUFFICIENT_MULTI_SIG_QUORUM" }, { key: "error.field", type: "String", value: "signatureProof" }, { key: "error.message", type: "String", value: "At least 2 out of 3 authorized treasury signers are required to release funds." }, { key: "error.retryAllowed", type: "Boolean", value: "false" }] },
};

const TRAY_CONTAINER_STYLE: React.CSSProperties = {
  position: "fixed", bottom: 0, left: 0, right: 0, height: "360px", zIndex: MODAL_Z_INDEX, backgroundColor: "var(--bg-surface)", borderTop: "2px solid var(--border-default)", boxShadow: "var(--shadow-lg)", display: "flex", flexDirection: "column", transition: "height 0.2s ease",
};

const OVERLAY_CONTAINER_STYLE: React.CSSProperties = {
  position: "fixed", inset: 0, zIndex: MODAL_Z_INDEX, backgroundColor: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", padding: "16px",
};

const DIALOG_BOX_STYLE: React.CSSProperties = {
  width: "100%", maxWidth: "760px", maxHeight: "85vh", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-lg)", boxShadow: "var(--shadow-lg)", display: "flex", flexDirection: "column", overflow: "hidden",
};

const SampleDataHeader: React.FC<{ badge: string; isTrayMode: boolean; onToggleTray: () => void; onClose: () => void }> = ({ badge, isTrayMode, onToggleTray, onClose }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "14px 20px", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", backgroundColor: "var(--bg-workspace)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="database" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="var(--action-primary)" />
        <h2 style={{ margin: 0, fontSize: "14px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{t("sampleData.title", "Sample Data Studio (DataTableView & Fixtures)")}</h2>
        <span style={{ fontSize: "9px", fontFamily: "var(--font-mono)", padding: "2px 6px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-selected)", color: "var(--action-primary)", border: "1px solid var(--border-subtle)" }}>{badge}</span>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
        <button type="button" data-testid="toggle-tray-mode-btn" onClick={onToggleTray} title={isTrayMode ? "Mở dạng Modal" : "Thu thành Bottom Tray"} style={{ background: "transparent", border: "1px solid var(--border-default)", color: "var(--text-muted)", cursor: "pointer", padding: "4px 8px", borderRadius: "var(--radius-xs)", fontSize: "11px" }}>{isTrayMode ? "Modal Mode" : "Tray Mode"}</button>
        <button type="button" data-testid="sample-data-close-btn" onClick={onClose} style={{ background: "transparent", border: "none", color: "var(--text-muted)", cursor: "pointer", padding: "4px" }}>
          <BrandIcon name="close" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
        </button>
      </div>
    </div>
  );
};

const SampleDataFilterBar: React.FC<{ currentPreset: SampleDataPreset; filterQuery: string; onSelectPreset: (p: SampleDataPreset) => void; onChangeQuery: (q: string) => void }> = ({ currentPreset, filterQuery, onSelectPreset, onChangeQuery }) => (
  <div data-testid="advanced-filter-bar" style={{ padding: "8px 16px", borderBottom: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "10px", flexWrap: "wrap" }}>
    <div style={{ display: "flex", gap: "6px" }}>
      {PRESET_KEYS.map((presetKey) => {
        const isSelected = currentPreset === presetKey;
        return (
          <button key={presetKey} type="button" data-testid={`preset-filter-btn-${presetKey.toLowerCase()}`} onClick={() => onSelectPreset(presetKey)} style={{ padding: "4px 10px", fontSize: "11px", fontWeight: isSelected ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: isSelected ? "1px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: isSelected ? "var(--bg-selected)" : "transparent", color: isSelected ? "var(--action-primary)" : "var(--text-secondary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "4px" }}>
            <span>{PRESET_DATA[presetKey].label}</span>
          </button>
        );
      })}
    </div>
    <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
      <BrandIcon name="search" size={ICON_SEARCH_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="var(--text-muted)" />
      <input type="text" data-testid="sample-data-search-input" placeholder="Lọc thuộc tính..." value={filterQuery} onChange={(e) => onChangeQuery(e.target.value)} style={{ padding: "3px 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", width: "140px" }} />
    </div>
  </div>
);

const SampleDataTable: React.FC<{ rows: DataRow[] }> = ({ rows }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="data-table-view" style={{ flex: 1, overflowY: "auto", padding: "12px 16px", backgroundColor: "var(--bg-canvas)" }}>
      <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "11px", fontFamily: "var(--font-mono)", backgroundColor: "var(--bg-surface)", borderRadius: "var(--radius-sm)", overflow: "hidden", border: "1px solid var(--border-default)" }}>
        <thead>
          <tr style={{ backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)" }}>
            <th style={{ textAlign: "left", padding: "8px 12px", color: "var(--text-muted)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{t("sampleData.colKey", "Attribute Key")}</th>
            <th style={{ textAlign: "left", padding: "8px 12px", color: "var(--text-muted)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{t("sampleData.colType", "Data Type")}</th>
            <th style={{ textAlign: "left", padding: "8px 12px", color: "var(--text-muted)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{t("sampleData.colValue", "Fixture Value")}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row, idx) => (
            <tr key={row.key} data-testid={`data-row-${idx}`} style={{ borderBottom: "1px solid var(--border-subtle)", backgroundColor: idx % 2 === 0 ? "transparent" : "var(--bg-subtle)" }}>
              <td style={{ padding: "8px 12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--human-accent)" }}>{row.key}</td>
              <td style={{ padding: "8px 12px", color: "var(--text-muted)" }}>{row.type}</td>
              <td style={{ padding: "8px 12px", color: "var(--text-primary)", wordBreak: "break-word" }}>{row.value}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};

const SampleDataFooter: React.FC<{ rowCount: number; preset: SampleDataPreset; onClose: () => void; onApply: () => void }> = ({ rowCount, preset, onClose, onApply }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "10px 16px", borderTop: "1px solid var(--border-default)", display: "flex", justifyContent: "space-between", alignItems: "center", backgroundColor: "var(--bg-workspace)" }}>
      <span style={{ fontSize: "10.5px", color: "var(--text-muted)" }}>{t("sampleData.footerSummary", "{{count}} attributes loaded • Preset: {{preset}}").replace("{{count}}", String(rowCount)).replace("{{preset}}", preset)}</span>
      <div style={{ display: "flex", gap: "8px" }}>
        <button type="button" data-testid="sample-data-cancel-btn" onClick={onClose} style={{ padding: "5px 12px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", cursor: "pointer" }}>{t("common.cancel", "Đóng")}</button>
        <button type="button" data-testid="sample-data-apply-btn" onClick={onApply} style={{ padding: "5px 14px", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "5px" }}>
          <BrandIcon name="play" size={ICON_PLAY_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
          <span>{t("sampleData.applyBtn", "Nạp Fixture vào Live Preview")}</span>
        </button>
      </div>
    </div>
  );
};

export const SampleDataStudioModal: React.FC<SampleDataStudioModalProps> = ({ isOpen, onClose, activePreset = "HAPPY", onApplyFixture }) => {
  const [currentPreset, setCurrentPreset] = useState<SampleDataPreset>(activePreset);
  const [filterQuery, setFilterQuery] = useState("");
  const [isTrayMode, setIsTrayMode] = useState(false);

  if (!isOpen) return null;
  const currentRows = PRESET_DATA[currentPreset].data;
  const filteredRows = currentRows.filter((r) => r.key.toLowerCase().includes(filterQuery.toLowerCase()) || r.value.toLowerCase().includes(filterQuery.toLowerCase()));
  const fixtureJson = JSON.stringify(Object.fromEntries(currentRows.map((r) => [r.key, r.value])), null, 2);

  return (
    <div role="dialog" aria-modal="true" data-testid="sample-data-studio-modal" style={isTrayMode ? TRAY_CONTAINER_STYLE : OVERLAY_CONTAINER_STYLE}>
      <div style={isTrayMode ? { width: "100%", height: "100%", display: "flex", flexDirection: "column" } : DIALOG_BOX_STYLE}>
        <SampleDataHeader badge={PRESET_DATA[currentPreset].badge} isTrayMode={isTrayMode} onToggleTray={() => setIsTrayMode((v) => !v)} onClose={onClose} />
        <SampleDataFilterBar currentPreset={currentPreset} filterQuery={filterQuery} onSelectPreset={setCurrentPreset} onChangeQuery={setFilterQuery} />
        <SampleDataTable rows={filteredRows} />
        <SampleDataFooter rowCount={filteredRows.length} preset={currentPreset} onClose={onClose} onApply={() => { onApplyFixture?.(currentPreset, fixtureJson); onClose(); }} />
      </div>
    </div>
  );
};
