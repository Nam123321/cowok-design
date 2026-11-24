// @story 06.5
// @implements AC-03, AC-04 (Direct Editable Data Table, Styled Upload Dropzone, Divergence Banner & Raw CSV Paste)
import React, { useState } from "react";
import { BrandIcon } from "../../../../components/ui/BrandIcon";
import { useFormatter } from "../../../../hooks/useFormatter";
import { useTranslation } from "../../../../hooks/useTranslation";
import { formatMetricValue } from "./studioDataUtils";
import {
  FONT_WEIGHT_SEMIBOLD,
  ICON_SM,
  SECTION_TITLE_STYLE,
  STUDIO_COMPACT_INPUT,
  STUDIO_ICON_BTN,
  STUDIO_INPUT_STYLE,
  STUDIO_SECONDARY_BTN,
} from "./studioStyles";
import type {
  SourceDivergenceInfo,
  StudioDataRow,
  StudioUnit,
  UploadedFileState,
} from "./types";

export interface StudioDataTableProps {
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  uploadedFile: UploadedFileState | null;
  pastedData: string;
  rows: readonly StudioDataRow[];
  unit: StudioUnit;
  divergence: SourceDivergenceInfo;
  highlightedRowId: string | null;
  onHighlightRow: (rowId: string | null) => void;
  onFileChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onClearFile: () => void;
  onRawCsvChange: (val: string) => void;
  onUpdateRow: (rowId: string, patch: Partial<StudioDataRow>) => void;
  onAddRow: () => void;
  onDeleteRow: (rowId: string) => void;
  onRevertToSource: () => void;
}

const PERCENT_SCALE = 100;
const RAW_CSV_ROWS = 2;

const FileUploadDropzone: React.FC<{
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  uploadedFile: UploadedFileState | null;
  onFileChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onClearFile: () => void;
}> = ({ fileInputRef, uploadedFile, onFileChange, onClearFile }) => {
  const { locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
      <input ref={fileInputRef} type="file" accept=".csv,.tsv,.pdf,text/csv,text/tab-separated-values,application/pdf" onChange={onFileChange} data-testid="deck-file-input" style={{ display: "none" }} />
      <button type="button" data-testid="studio-file-upload-btn" onClick={() => fileInputRef.current?.click()} style={{ ...STUDIO_SECONDARY_BTN, justifyContent: "center", borderStyle: "dashed", padding: "8px 10px", backgroundColor: "var(--bg-subtle)" }}>
        <BrandIcon name="file-text" size={ICON_SM} color="var(--action-primary)" />
        <span>Tải tệp dữ liệu (.csv, .tsv, .pdf ≤ 5 MB)</span>
      </button>
      {uploadedFile && (
        <div data-testid="deck-gen-uploaded-chip" style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "5px 8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-selected)", fontSize: "11.5px" }}>
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{uploadedFile.name} ({formatNumber(uploadedFile.byteSize)} B)</span>
          <button type="button" data-testid="deck-gen-clear-file-btn" onClick={onClearFile} style={{ border: "none", background: "transparent", color: "var(--text-secondary)", cursor: "pointer", fontSize: "11px" }}>Gỡ tệp</button>
        </div>
      )}
    </div>
  );
};

const SourceDivergenceBanner: React.FC<{ divergence: SourceDivergenceInfo; unit: StudioUnit; onRevert: () => void }> = ({ divergence, unit, onRevert }) => {
  if (!divergence.isDivergent) return null;
  const sign = divergence.totalDelta >= 0 ? "+" : "-";
  const absDelta = formatMetricValue(Math.abs(divergence.totalDelta), unit);
  return (
    <div data-testid="studio-data-divergence-banner" role="status" style={{ display: "flex", flexDirection: "column", gap: "6px", padding: "8px 10px", borderRadius: "var(--radius-sm)", border: "1px solid var(--state-warning)", backgroundColor: "var(--state-warning-soft)", color: "var(--text-primary)", fontSize: "11.5px" }}>
      <div style={{ display: "flex", alignItems: "flex-start", gap: "6px" }}>
        <BrandIcon name="warning" size={ICON_SM} color="var(--state-warning)" />
        <span><strong>Dữ liệu đã chỉnh sửa khác với tệp/nguồn gốc:</strong> {divergence.changedRowCount} dòng thay đổi (chênh tổng {sign}{absDelta}).</span>
      </div>
      <div>
        <button type="button" data-testid="studio-revert-source-btn" onClick={onRevert} style={{ ...STUDIO_SECONDARY_BTN, padding: "4px 8px", fontSize: "11px" }}>
          <BrandIcon name="refresh" size={ICON_SM} color="currentColor" />
          <span>Khôi phục dữ liệu gốc</span>
        </button>
      </div>
    </div>
  );
};

const EditableRowLine: React.FC<{
  row: StudioDataRow;
  sharePct: number;
  highlighted: boolean;
  onHighlight: () => void;
  onUpdate: (patch: Partial<StudioDataRow>) => void;
  onDelete: () => void;
}> = ({ row, sharePct, highlighted, onHighlight, onUpdate, onDelete }) => (
  <tr data-testid={`studio-data-row-${row.id}`} onClick={onHighlight} style={{ backgroundColor: highlighted ? "var(--bg-selected)" : "transparent" }}>
    <td style={{ padding: "3px" }}><input type="text" aria-label={`Nhãn ${row.label}`} value={row.label} onChange={(e) => onUpdate({ label: e.target.value })} style={STUDIO_COMPACT_INPUT} /></td>
    <td style={{ padding: "3px", width: "68px" }}><input type="number" aria-label={`Giá trị Q4 ${row.label}`} value={row.current} onChange={(e) => onUpdate({ current: Number(e.target.value) || 0 })} style={STUDIO_COMPACT_INPUT} /></td>
    <td style={{ padding: "3px", width: "64px" }}><input type="number" aria-label={`Giá trị Q3 ${row.label}`} value={row.previous} onChange={(e) => onUpdate({ previous: Number(e.target.value) || 0 })} style={STUDIO_COMPACT_INPUT} /></td>
    <td style={{ padding: "3px", textAlign: "right", fontSize: "11px", color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{sharePct}%</td>
    <td style={{ padding: "3px", width: "26px", textAlign: "center" }}>
      <button type="button" aria-label={`Xóa ${row.label}`} onClick={(e) => { e.stopPropagation(); onDelete(); }} style={{ ...STUDIO_ICON_BTN, padding: "3px", border: "none", backgroundColor: "transparent" }}>
        <BrandIcon name="trash" size={ICON_SM} color="var(--text-muted)" />
      </button>
    </td>
  </tr>
);

const EditableDataGrid: React.FC<{
  rows: readonly StudioDataRow[];
  highlightedRowId: string | null;
  onHighlightRow: (rowId: string | null) => void;
  onUpdateRow: (rowId: string, patch: Partial<StudioDataRow>) => void;
  onDeleteRow: (rowId: string) => void;
}> = ({ rows, highlightedRowId, onHighlightRow, onUpdateRow, onDeleteRow }) => {
  const total = rows.reduce((acc, r) => acc + Math.max(0, r.current), 0);
  return (
    <div style={{ border: "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", overflowX: "auto", backgroundColor: "var(--bg-workspace)" }}>
      <table data-testid="studio-data-table" style={{ width: "100%", borderCollapse: "collapse", fontSize: "11.5px" }}>
        <thead>
          <tr style={{ borderBottom: "1px solid var(--border-subtle)", color: "var(--text-muted)", textAlign: "left" }}>
            <th style={{ padding: "5px 6px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>Nhãn</th>
            <th style={{ padding: "5px 4px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>Q4</th>
            <th style={{ padding: "5px 4px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>Q3</th>
            <th style={{ padding: "5px 4px", fontWeight: FONT_WEIGHT_SEMIBOLD, textAlign: "right" }}>%</th>
            <th style={{ padding: "5px 2px" }} />
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <EditableRowLine
              key={row.id} row={row}
              sharePct={total > 0 ? Math.round((Math.max(0, row.current) / total) * PERCENT_SCALE) : 0}
              highlighted={highlightedRowId === row.id}
              onHighlight={() => onHighlightRow(highlightedRowId === row.id ? null : row.id)}
              onUpdate={(patch) => onUpdateRow(row.id, patch)}
              onDelete={() => onDeleteRow(row.id)}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
};

export const StudioDataTable: React.FC<StudioDataTableProps> = (props) => {
  const [rawOpen, setRawOpen] = useState(false);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      <FileUploadDropzone fileInputRef={props.fileInputRef} uploadedFile={props.uploadedFile} onFileChange={props.onFileChange} onClearFile={props.onClearFile} />
      <SourceDivergenceBanner divergence={props.divergence} unit={props.unit} onRevert={props.onRevertToSource} />
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={SECTION_TITLE_STYLE}>Bảng dữ liệu trực tiếp</span>
        <button type="button" data-testid="studio-add-row-btn" onClick={props.onAddRow} style={{ ...STUDIO_ICON_BTN, padding: "3px 7px", fontSize: "11px" }}>
          <BrandIcon name="plus" size={ICON_SM} color="currentColor" />
          <span>Thêm dòng</span>
        </button>
      </div>
      <EditableDataGrid rows={props.rows} highlightedRowId={props.highlightedRowId} onHighlightRow={props.onHighlightRow} onUpdateRow={props.onUpdateRow} onDeleteRow={props.onDeleteRow} />
      <button type="button" data-testid="studio-raw-csv-toggle" onClick={() => setRawOpen((v) => !v)} style={{ ...STUDIO_ICON_BTN, justifyContent: "space-between", width: "100%" }}>
        <span>Dán CSV / TSV thô</span>
        <BrandIcon name={rawOpen ? "chevron-down" : "chevron-right"} size={ICON_SM} color="currentColor" />
      </button>
      {rawOpen && (
        <textarea rows={RAW_CSV_ROWS} value={props.pastedData} onChange={(e) => props.onRawCsvChange(e.target.value)} placeholder={"Channel,Revenue\nDirect,$1,234.56\nPartner,42.5M VND"} data-testid="deck-gen-paste-input" style={{ ...STUDIO_INPUT_STYLE, fontFamily: "var(--font-mono, monospace)", resize: "vertical" }} />
      )}
    </div>
  );
};
