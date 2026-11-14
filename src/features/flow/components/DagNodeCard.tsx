// @story 04.1
// @implements AC-04, AC-07, AC-08
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";
import { WorkbenchNode, NodeSubState } from "../store/flow-workbench-store";
import { FIXTURE_VARIANTS, normalizeFixtureRef } from "../../../lib/validations/screen-fixture";
import { DagNodeHeader, DagNodePorts, PortMouseDownArgs } from "./DagNodeHeaderAndPorts";

export type { PortMouseDownArgs };

export interface DagNodeCardProps {
  node: WorkbenchNode;
  isSelected: boolean;
  isOrphan?: boolean;
  onNodeMouseDown: (e: React.MouseEvent, node: WorkbenchNode) => void;
  onPortMouseDown: (args: PortMouseDownArgs) => void;
  onUpdateNote: (nodeId: string, note: string) => void;
  onSelectSubState?: (nodeId: string, subState: NodeSubState) => void;
  onDuplicateNode?: (nodeId: string) => void;
  onRenameNode?: (nodeId: string, title: string) => void;
  onDeleteNode?: (nodeId: string) => void;
  onCommentOnNode?: (screenKey: string) => void;
  nodeWidth: number;
  nodeHeight: number;
  executionStatus?: {
    status: "running" | "queued" | "paused" | "completed" | "error" | "cancelled";
    queuePosition?: number;
  };
}

const getNodeBorder = (isSelected: boolean, isDraft: boolean): string => {
  if (isSelected) return "2px solid var(--action-primary)";
  if (isDraft) return "1.5px dashed var(--action-primary)";
  return "1px solid var(--border-subtle)";
};

const DagNodeSubStatePills: React.FC<{
  nodeId: string;
  activeSubState: NodeSubState;
  onSelectSubState?: (nodeId: string, subState: NodeSubState) => void;
}> = ({ nodeId, activeSubState, onSelectSubState }) => {
  const normalizedActive = normalizeFixtureRef(activeSubState);
  return (
    <div className="dag-node-substate-switcher" style={{ display: "flex", gap: "3px", marginBottom: "6px", flexWrap: "wrap" }}>
      {FIXTURE_VARIANTS.map(({ variant, label }) => {
        const isActive = variant === normalizedActive;
        return (
          <button key={variant} type="button" className={`dag-substate-pill ${isActive ? "active" : ""}`} onClick={(e) => { e.stopPropagation(); onSelectSubState?.(nodeId, variant); }} style={{ fontSize: "9px", padding: "1px 5px", borderRadius: "var(--radius-xs)", border: isActive ? "1px solid var(--action-primary)" : "1px solid var(--border-subtle)", backgroundColor: isActive ? "var(--bg-selected)" : "transparent", color: isActive ? "var(--text-primary)" : "var(--text-muted)", cursor: "pointer" }}>
            {label}
          </button>
        );
      })}
    </div>
  );
};

const DagNodeTitleEditor: React.FC<{
  node: WorkbenchNode;
  isRenaming: boolean;
  renameDraft: string;
  onChangeRenameDraft: (val: string) => void;
  onCommitRename: () => void;
  onCancelRename: () => void;
  onStartRename: () => void;
}> = (p) =>
  p.isRenaming ? (
    <input
      type="text" data-testid={`rename-node-input-${p.node.id}`} autoFocus value={p.renameDraft}
      onClick={(e) => e.stopPropagation()} onMouseDown={(e) => e.stopPropagation()}
      onChange={(e) => p.onChangeRenameDraft(e.target.value)} onBlur={p.onCommitRename}
      onKeyDown={(e) => { if (e.key === "Enter") p.onCommitRename(); if (e.key === "Escape") p.onCancelRename(); }}
      style={{ width: "100%", fontSize: "12px", fontWeight: 600, padding: "2px 6px", marginBottom: "4px", borderRadius: "var(--radius-xs)", border: "1px solid var(--action-primary)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", boxSizing: "border-box" }}
    />
  ) : (
    <div onDoubleClick={(e) => { e.stopPropagation(); p.onStartRename(); }} style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", marginBottom: "4px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
      {p.node.title}
    </div>
  );

const DagNodeBody: React.FC<{
  node: WorkbenchNode;
  isOrphan?: boolean;
  isRenaming: boolean;
  renameDraft: string;
  onChangeRenameDraft: (val: string) => void;
  onCommitRename: () => void;
  onCancelRename: () => void;
  onStartRename: () => void;
  onUpdateNote: (nodeId: string, note: string) => void;
  onSelectSubState?: (nodeId: string, subState: NodeSubState) => void;
}> = (p) => {
  const { t } = useTranslation();
  const activeSubState: NodeSubState = normalizeFixtureRef(p.node.activeSubState);
  const fixtureRef = normalizeFixtureRef(p.node.fixtureRef || activeSubState);
  return (
    <div style={{ padding: "8px 10px", flex: 1 }}>
      <DagNodeTitleEditor node={p.node} isRenaming={p.isRenaming} renameDraft={p.renameDraft} onChangeRenameDraft={p.onChangeRenameDraft} onCommitRename={p.onCommitRename} onCancelRename={p.onCancelRename} onStartRename={p.onStartRename} />
      {p.isOrphan && (
        <div
          data-testid={`dag-node-orphan-badge-${p.node.id}`}
          title={t("flow.orphanNodeTooltip", "ScreenState chưa được nối vào luồng (Orphan Node)")}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "4px",
            fontSize: "9px",
            fontWeight: 600,
            padding: "1px 6px",
            marginBottom: "4px",
            borderRadius: "var(--radius-xs)",
            backgroundColor: "var(--state-warning-soft)",
            color: "var(--state-warning)",
            border: "1px solid var(--state-warning)",
          }}
        >
          <BrandIcon name="warning" size={10} color="currentColor" />
          <span>{t("flow.orphanNodeLabel", "Cô lập")}</span>
        </div>
      )}
      {p.node.routePath && <div style={{ fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--text-muted)", marginBottom: "6px" }}>{p.node.routePath}</div>}
      <div className="dag-node-wireframe-preview" data-substate={activeSubState} style={{ border: "1px dashed var(--border-subtle)", borderRadius: "var(--radius-xs)", padding: "5px 6px", marginBottom: "6px", backgroundColor: "var(--bg-canvas)", display: "flex", alignItems: "center", justifyContent: "space-between", fontSize: "9px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
        <span>{t("flow.wireframePrefix", "Wireframe")}: {activeSubState}</span>
        <span className="dag-node-contract-badge" style={{ color: "var(--human-accent)" }}>{fixtureRef}</span>
      </div>
      <DagNodeSubStatePills nodeId={p.node.id} activeSubState={activeSubState} onSelectSubState={p.onSelectSubState} />
      <input type="text" placeholder={t("flow.addNotePlaceholder", "Thêm ghi chú...")} value={p.node.note || ""} onChange={(e) => p.onUpdateNote(p.node.id, e.target.value)} onClick={(e) => e.stopPropagation()} onMouseDown={(e) => e.stopPropagation()} style={{ width: "100%", marginTop: "4px", fontSize: "10px", border: "1px solid var(--border-subtle)", borderRadius: "var(--radius-xs)", padding: "2px 6px", backgroundColor: "var(--bg-canvas)", color: "var(--text-secondary)", boxSizing: "border-box" }} />
    </div>
  );
};

export const DagNodeCard: React.FC<DagNodeCardProps> = (props) => {
  const { t } = useTranslation();
  const [isRenaming, setIsRenaming] = useState(false);
  const [renameDraft, setRenameDraft] = useState(props.node.title);
  const isDraft = props.node.status === "draft";
  const startRename = () => { setRenameDraft(props.node.title); setIsRenaming(true); };
  const commitRename = () => {
    setIsRenaming(false);
    const trimmed = renameDraft.trim();
    if (trimmed && trimmed !== props.node.title) props.onRenameNode?.(props.node.id, trimmed);
  };
  const cardStyle: React.CSSProperties = {
    position: "absolute", left: `${props.node.posX}px`, top: `${props.node.posY}px`,
    width: `${props.nodeWidth}px`, minHeight: `${props.nodeHeight}px`, backgroundColor: "var(--bg-surface)",
    border: getNodeBorder(props.isSelected, isDraft), borderRadius: "var(--radius-md)",
    boxShadow: props.isSelected ? "var(--shadow-md)" : "var(--shadow-sm)", pointerEvents: "auto",
    cursor: "move", display: "flex", flexDirection: "column", justifyContent: "space-between", boxSizing: "border-box",
  };

  return (
    <div className="dag-node-card" data-testid={`dag-node-${props.node.id}`} aria-label={t("flow.nodeCard", `Node ${props.node.screenKey}`)} onMouseDown={(e) => props.onNodeMouseDown(e, props.node)} style={cardStyle}>
      <DagNodeHeader
        node={props.node}
        isDraft={isDraft}
        onStartRename={startRename}
        onCommentOnNode={props.onCommentOnNode}
        onDuplicateNode={props.onDuplicateNode}
        onDeleteNode={props.onDeleteNode}
        executionStatus={props.executionStatus}
      />
      <DagNodeBody node={props.node} isOrphan={props.isOrphan} isRenaming={isRenaming} renameDraft={renameDraft} onChangeRenameDraft={setRenameDraft} onCommitRename={commitRename} onCancelRename={() => setIsRenaming(false)} onStartRename={startRename} onUpdateNote={props.onUpdateNote} onSelectSubState={props.onSelectSubState} />
      <DagNodePorts node={props.node} nodeWidth={props.nodeWidth} nodeHeight={props.nodeHeight} onPortMouseDown={props.onPortMouseDown} />
    </div>
  );
};
