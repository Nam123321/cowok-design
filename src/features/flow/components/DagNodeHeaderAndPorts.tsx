// @story 04.1
// @implements AC-04, AC-07, AC-08
import React, { useState } from "react";
import { useTranslation } from "@/hooks/useTranslation";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { WorkbenchNode, SemanticNodeType } from "../store/flow-workbench-store";
import { PortType } from "../utils/dag-algorithms";

export interface PortMouseDownArgs {
  e: React.MouseEvent;
  nodeId: string;
  portType: PortType;
  startX: number;
  startY: number;
}

const PORT_SIZE = 12;
const PORT_HALF_SIZE = 6;
const ICON_COMMENT_SIZE = 13;

const NODE_TYPE_LABELS: Record<SemanticNodeType, string> = {
  screen: "Screen",
  modal: "Modal",
  condition: "Condition",
  decision: "Condition",
  action: "Action",
};

const HEADER_STYLE: React.CSSProperties = {
  padding: "8px 10px", borderBottom: "1px solid var(--border-subtle)", display: "flex",
  alignItems: "center", justifyContent: "space-between", backgroundColor: "var(--bg-subtle)",
  borderTopLeftRadius: "var(--radius-md)", borderTopRightRadius: "var(--radius-md)",
};

const MENU_CONTAINER_STYLE: React.CSSProperties = {
  position: "absolute", top: "30px", right: "6px", zIndex: 30, backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", boxShadow: "var(--shadow-md)",
  padding: "4px", display: "flex", flexDirection: "column", gap: "2px", minWidth: "132px",
};

const MENU_ITEM_STYLE: React.CSSProperties = {
  border: "none", background: "transparent", textAlign: "left", fontSize: "10.5px",
  padding: "4px 6px", cursor: "pointer", color: "var(--text-primary)", borderRadius: "var(--radius-xs)",
};

const BASE_PORT_STYLE: React.CSSProperties = {
  position: "absolute", width: `${PORT_SIZE}px`, height: `${PORT_SIZE}px`, borderRadius: "50%",
  border: "2px solid var(--bg-surface)", boxShadow: "0 0 4px rgba(0,0,0,0.2)", pointerEvents: "auto",
};

const NodeContextMenu: React.FC<{
  nodeId: string;
  onStartRename: () => void;
  onDuplicateNode?: (id: string) => void;
  onDeleteNode?: (id: string) => void;
  onClose: () => void;
}> = ({ nodeId, onStartRename, onDuplicateNode, onDeleteNode, onClose }) => {
  const { t } = useTranslation();
  return (
    <div className="dag-node-context-menu" role="menu" onClick={(e) => e.stopPropagation()} style={MENU_CONTAINER_STYLE}>
      <button type="button" role="menuitem" style={MENU_ITEM_STYLE} onClick={() => { onClose(); onStartRename(); }}>
        {t("flow.renameNode", "Đổi tên Node")}
      </button>
      <button type="button" role="menuitem" style={MENU_ITEM_STYLE} onClick={() => { onDuplicateNode?.(nodeId); onClose(); }}>
        {t("flow.duplicateNode", "Nhân bản (⌘D)")}
      </button>
      <button type="button" role="menuitem" style={{ ...MENU_ITEM_STYLE, color: "var(--state-danger)" }} onClick={() => { onDeleteNode?.(nodeId); onClose(); }}>
        {t("flow.deleteNode", "Xóa Node (Delete)")}
      </button>
    </div>
  );
};

export const DagNodeHeader: React.FC<{
  node: WorkbenchNode;
  isDraft: boolean;
  onStartRename: () => void;
  onCommentOnNode?: (screenKey: string) => void;
  onDuplicateNode?: (nodeId: string) => void;
  onDeleteNode?: (nodeId: string) => void;
  executionStatus?: { status: "running" | "queued" | "paused" | "completed" | "error" | "cancelled"; queuePosition?: number };
}> = ({ node, isDraft, onStartRename, onCommentOnNode, onDuplicateNode, onDeleteNode, executionStatus }) => {
  const { t } = useTranslation();
  const [menuOpen, setMenuOpen] = useState(false);
  const nodeType: SemanticNodeType = node.nodeType || "screen";

  return (
    <div style={HEADER_STYLE}>
      <div style={{ display: "flex", alignItems: "center", gap: "5px" }}>
        <span style={{ fontFamily: "var(--font-mono)", fontSize: "11px", fontWeight: 600, color: "var(--text-primary)" }}>{node.screenKey}</span>
        <span className="dag-node-type-badge" style={{ fontSize: "9px", fontWeight: 600, backgroundColor: "var(--bg-canvas)", color: "var(--text-muted)", border: "1px solid var(--border-subtle)", padding: "1px 5px", borderRadius: "var(--radius-xs)", textTransform: "uppercase" }}>
          {NODE_TYPE_LABELS[nodeType]}
        </span>
        {isDraft && (
          <span className="dag-node-draft-badge" style={{ fontFamily: "var(--font-mono)", fontSize: "9px", fontWeight: 700, backgroundColor: "var(--bg-elevated)", color: "var(--text-primary)", border: "1px dashed var(--action-primary)", padding: "1px 5px", borderRadius: "var(--radius-xs)" }}>DRAFT</span>
        )}
        {executionStatus?.status === "running" && (
          <span
            className="dag-node-running-badge"
            style={{
              fontSize: "9px",
              fontWeight: 700,
              backgroundColor: "rgba(59, 130, 246, 0.18)",
              color: "var(--action-primary)",
              border: "1px solid var(--action-primary)",
              padding: "1px 5px",
              borderRadius: "var(--radius-xs)",
              display: "flex",
              alignItems: "center",
              gap: "3px",
            }}
          >
            <span style={{ display: "inline-block", width: "5px", height: "5px", borderRadius: "50%", backgroundColor: "currentColor" }} />
            RUNNING
          </span>
        )}
        {executionStatus?.status === "queued" && (
          <span
            className="dag-node-queued-badge"
            style={{
              fontSize: "9px",
              fontWeight: 600,
              backgroundColor: "rgba(245, 158, 11, 0.15)",
              color: "#f59e0b",
              border: "1px solid rgba(245, 158, 11, 0.4)",
              padding: "1px 5px",
              borderRadius: "var(--radius-xs)",
            }}
          >
            QUEUED #{executionStatus.queuePosition ?? 1}
          </span>
        )}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "2px", position: "relative" }}>
        {onCommentOnNode && (
          <button type="button" title={t("flow.commentNode", "Bình luận trên Node này vào Chat")} onClick={(e) => { e.stopPropagation(); onCommentOnNode(node.screenKey); }} style={{ border: "none", background: "transparent", cursor: "pointer", padding: "2px", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--text-muted)" }}>
            <BrandIcon name="message-square" size={ICON_COMMENT_SIZE} color="currentColor" />
          </button>
        )}
        <button type="button" className="dag-node-menu-trigger" aria-label={`Menu ngữ cảnh ${node.screenKey}`} onClick={(e) => { e.stopPropagation(); setMenuOpen((prev) => !prev); }} style={{ border: "none", background: "transparent", cursor: "pointer", padding: "2px 4px", fontSize: "11px", fontWeight: 700, color: "var(--text-muted)" }}>
          ...
        </button>
        {menuOpen && <NodeContextMenu nodeId={node.id} onStartRename={onStartRename} onDuplicateNode={onDuplicateNode} onDeleteNode={onDeleteNode} onClose={() => setMenuOpen(false)} />}
      </div>
    </div>
  );
};

export const DagNodePorts: React.FC<{
  node: WorkbenchNode;
  nodeWidth: number;
  nodeHeight: number;
  onPortMouseDown: (args: PortMouseDownArgs) => void;
}> = ({ node, nodeWidth, nodeHeight, onPortMouseDown }) => (
  <>
    <div className="port-in" title="Cổng vào (Input)" style={{ ...BASE_PORT_STYLE, left: `-${PORT_HALF_SIZE}px`, top: "50%", transform: "translateY(-50%)", backgroundColor: "var(--human-accent)" }} />
    <div className="port-out-main" title="Cổng ra nhánh chính (Kéo để nối dây)" onMouseDown={(e) => onPortMouseDown({ e, nodeId: node.id, portType: "out-main", startX: node.posX + nodeWidth, startY: node.posY + nodeHeight / 2 })} style={{ ...BASE_PORT_STYLE, right: `-${PORT_HALF_SIZE}px`, top: "50%", transform: "translateY(-50%)", backgroundColor: "var(--action-primary)", cursor: "crosshair" }} />
    <div className="port-out-branch" title="Cổng ra rẽ nhánh / Fallback (Kéo để nối dây)" onMouseDown={(e) => onPortMouseDown({ e, nodeId: node.id, portType: "out-branch", startX: node.posX + nodeWidth / 2, startY: node.posY + nodeHeight })} style={{ ...BASE_PORT_STYLE, left: "50%", bottom: `-${PORT_HALF_SIZE}px`, transform: "translateX(-50%)", backgroundColor: "var(--agent-accent)", cursor: "crosshair" }} />
  </>
);
