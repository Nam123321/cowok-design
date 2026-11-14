// @story 04.1
// @implements AC-04, AC-08
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { WorkbenchTransition, FlowEdgeType } from "../store/flow-workbench-store";

export interface EdgeConditionInspectorProps {
  transition: WorkbenchTransition | null;
  onUpdate: (
    transitionId: string,
    updates: Partial<Pick<WorkbenchTransition, "edgeType" | "conditionLabel" | "triggerEvent" | "guardCondition">>
  ) => void;
  onReverse: (transitionId: string) => void;
  onDelete: (transitionId: string) => void;
  onClose: () => void;
}

const INSPECTOR_CONTAINER_STYLE: React.CSSProperties = {
  position: "absolute", top: "16px", right: "60px", width: "280px", backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", boxShadow: "var(--shadow-md)",
  padding: "14px", zIndex: 25, display: "flex", flexDirection: "column", gap: "10px", fontFamily: "var(--font-sans)",
};

const FIELD_INPUT_STYLE: React.CSSProperties = {
  padding: "6px 8px", fontSize: "12px", border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", boxSizing: "border-box",
};

const FIELD_LABEL_STYLE: React.CSSProperties = { fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" };

const EdgeFormFields: React.FC<{
  transition: WorkbenchTransition;
  onUpdate: EdgeConditionInspectorProps["onUpdate"];
}> = ({ transition, onUpdate }) => {
  const { t } = useTranslation();
  return (
    <>
      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
        <label style={FIELD_LABEL_STYLE}>{t("flow.edgeTypeLabel", "Loại luồng")}</label>
        <select value={transition.edgeType} onChange={(e) => onUpdate(transition.id, { edgeType: e.target.value as FlowEdgeType })} style={FIELD_INPUT_STYLE}>
          <option value="main">{t("flow.edgeOptionMain", "Luồng chính (Main ───▶)")}</option>
          <option value="branch">{t("flow.edgeOptionBranch", "Luồng rẽ nhánh (IF / ELSE - - ▶)")}</option>
          <option value="fallback">{t("flow.edgeOptionFallback", "Luồng dự phòng (IF NOT ···▶)")}</option>
        </select>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
        <label style={FIELD_LABEL_STYLE}>{t("flow.edgeConditionLabel", "Nhãn điều kiện (Condition Label)")}</label>
        <input type="text" value={transition.conditionLabel || ""} placeholder={t("flow.edgeConditionPlaceholder", "e.g. status == 'VERIFIED' hoặc onClick")} onChange={(e) => onUpdate(transition.id, { conditionLabel: e.target.value, guardCondition: e.target.value || null })} style={FIELD_INPUT_STYLE} />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
        <label style={FIELD_LABEL_STYLE}>{t("flow.edgeTriggerLabel", "Sự kiện kích hoạt (Trigger)")}</label>
        <input type="text" value={transition.triggerEvent} onChange={(e) => onUpdate(transition.id, { triggerEvent: e.target.value })} style={FIELD_INPUT_STYLE} />
      </div>
    </>
  );
};

export const EdgeConditionInspector: React.FC<EdgeConditionInspectorProps> = ({
  transition,
  onUpdate,
  onReverse,
  onDelete,
  onClose,
}) => {
  const { t } = useTranslation();
  if (!transition) return null;

  return (
    <div className="edge-condition-inspector" data-testid="edge-condition-inspector" role="dialog" aria-label={t("flow.edgeInspectorTitle", "Thuộc tính liên kết luồng")} onClick={(e) => e.stopPropagation()} onMouseDown={(e) => e.stopPropagation()} style={INSPECTOR_CONTAINER_STYLE}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          <span style={{ fontSize: "13.5px", fontWeight: 600, color: "var(--text-primary)" }}>{t("flow.edgeTitle", "Liên kết luồng (Edge)")}</span>
          {transition.status === "draft" && (
            <span style={{ fontSize: "10px", fontFamily: "var(--font-mono)", fontWeight: 600, padding: "2px 6px", backgroundColor: "var(--bg-elevated)", color: "var(--text-primary)", border: "1px dashed var(--action-primary)", borderRadius: "var(--radius-xs)" }}>DRAFT</span>
          )}
        </div>
        <button type="button" onClick={onClose} aria-label={t("common.close", "Đóng")} style={{ border: "none", background: "transparent", cursor: "pointer", fontSize: "16px", color: "var(--text-muted)" }}>×</button>
      </div>
      <div style={{ fontSize: "11px", color: "var(--text-secondary)", backgroundColor: "var(--bg-subtle)", padding: "6px 8px", borderRadius: "var(--radius-xs)", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={{ fontFamily: "var(--font-mono)" }}>{transition.sourceNodeId.slice(0, 8)}</span>
        <span style={{ fontWeight: 600 }}>➔</span>
        <span style={{ fontFamily: "var(--font-mono)" }}>{transition.targetNodeId.slice(0, 8)}</span>
      </div>
      <EdgeFormFields transition={transition} onUpdate={onUpdate} />
      <div style={{ display: "flex", gap: "8px", marginTop: "2px" }}>
        <button type="button" data-testid="reverse-edge-btn" onClick={() => onReverse(transition.id)} style={{ flex: 1, padding: "6px 8px", fontSize: "12px", fontWeight: 600, border: "1px solid var(--border-default)", backgroundColor: "var(--bg-subtle)", color: "var(--text-primary)", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{t("flow.reverseEdge", "⇄ Đảo chiều")}</button>
        <button type="button" data-testid="delete-edge-btn" onClick={() => onDelete(transition.id)} style={{ padding: "6px 10px", fontSize: "12px", fontWeight: 600, border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--state-danger)", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{t("flow.deleteEdge", "Xóa")}</button>
      </div>
    </div>
  );
};
