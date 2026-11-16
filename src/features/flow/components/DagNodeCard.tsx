// @story 04.1, 05.1
// @implements AC-04, AC-07, AC-08, Option 5 Vertical Storyboard
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";
import { WorkbenchNode, NodeSubState } from "../store/flow-workbench-store";
import type { NodeJobStatus } from "../store/keyed-parallel-coordinator";
import { FIXTURE_VARIANTS, normalizeFixtureRef } from "../../../lib/validations/screen-fixture";
import { DagNodeHeader, DagNodePorts, PortMouseDownArgs } from "./DagNodeHeaderAndPorts";

export type { PortMouseDownArgs };

export interface ExtendedWorkbenchNode extends WorkbenchNode {
  ctaFfId?: string;
  sourceCtaFfId?: string;
  ctaLabel?: string;
  metadata?: Record<string, unknown>;
}

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
  onHoverCta?: (ffId: string, hovered: boolean) => void;
  nodeWidth: number;
  nodeHeight: number;
  executionStatus?: {
    status: NodeJobStatus;
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
    <div
      className="dag-node-substate-switcher"
      data-testid={`dag-node-substates-${nodeId}`}
      style={{ display: "flex", gap: "3px", marginBottom: "6px", flexWrap: "wrap" }}
    >
      {FIXTURE_VARIANTS.map(({ variant, label }) => {
        const isActive = variant === normalizedActive;
        return (
          <button
            key={variant}
            type="button"
            className={`dag-substate-pill ${isActive ? "active" : ""}`}
            data-testid={`dag-substate-${variant.toLowerCase()}-${nodeId}`}
            onClick={(e) => {
              e.stopPropagation();
              onSelectSubState?.(nodeId, variant);
            }}
            style={{
              fontSize: "9px",
              padding: "1px 5px",
              borderRadius: "var(--radius-xs)",
              border: isActive ? "1px solid var(--action-primary)" : "1px solid var(--border-subtle)",
              backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
              color: isActive ? "var(--text-primary)" : "var(--text-muted)",
              cursor: "pointer",
              fontWeight: isActive ? 600 : 400,
            }}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
};

const MiniSkeletonWireframe: React.FC<{ subState: NodeSubState; fixtureRef: string }> = ({
  subState,
  fixtureRef,
}) => (
  <div
    className="dag-node-mini-skeleton"
    data-testid="dag-node-mini-skeleton"
    data-substate={subState}
    style={{
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-xs)",
      padding: "5px 6px",
      marginBottom: "6px",
      backgroundColor: "var(--bg-canvas)",
      display: "flex",
      flexDirection: "column",
      gap: "4px",
    }}
  >
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <div style={{ width: "38%", height: "5px", borderRadius: "2px", backgroundColor: "var(--border-strong)" }} />
      <span
        className="dag-node-contract-badge"
        style={{ fontSize: "8.5px", fontFamily: "var(--font-mono)", color: "var(--human-accent)" }}
      >
        {fixtureRef}
      </span>
    </div>
    <div style={{ display: "flex", gap: "4px" }}>
      <div style={{ flex: 1, height: "12px", borderRadius: "2px", backgroundColor: "var(--bg-subtle)" }} />
      <div style={{ flex: 1, height: "12px", borderRadius: "2px", backgroundColor: "var(--bg-subtle)" }} />
    </div>
  </div>
);

const DagNodeCtaPills: React.FC<{
  node: WorkbenchNode;
  onHoverCta?: (ffId: string, hovered: boolean) => void;
}> = ({ node, onHoverCta }) => {
  const extNode = node as ExtendedWorkbenchNode;
  const outboundCtaFfId = extNode.ctaFfId || `cta-${node.screenKey || node.id}`;
  const loopbackFfId = extNode.sourceCtaFfId || (extNode.metadata as Record<string, unknown> | undefined)?.sourceCtaFfId;
  const ctaLabel = extNode.ctaLabel || "Next Step ──▶";

  return (
    <div style={{ display: "flex", gap: "4px", flexWrap: "wrap", marginBottom: "6px" }}>
      <span
        data-testid={`outbound-cta-pill-${node.id}`}
        data-ff-id={outboundCtaFfId}
        onMouseEnter={() => onHoverCta?.(outboundCtaFfId, true)}
        onMouseLeave={() => onHoverCta?.(outboundCtaFfId, false)}
        style={{
          fontSize: "9px",
          fontWeight: 600,
          padding: "2px 6px",
          borderRadius: "var(--radius-xs)",
          backgroundColor: "var(--action-primary)",
          color: "var(--action-primary-fg)",
          border: "1.85px solid var(--action-primary)",
          display: "inline-flex",
          alignItems: "center",
          gap: "3px",
          cursor: "pointer",
          userSelect: "none",
        }}
        title={`Outbound CTA: ${outboundCtaFfId}`}
      >
        <span>{ctaLabel}</span>
      </span>

      {typeof loopbackFfId === "string" && loopbackFfId && (
        <span
          data-testid={`loopback-cta-pill-${node.id}`}
          data-ff-id={loopbackFfId}
          onMouseEnter={() => onHoverCta?.(loopbackFfId, true)}
          onMouseLeave={() => onHoverCta?.(loopbackFfId, false)}
          style={{
            fontSize: "9px",
            fontWeight: 500,
            padding: "2px 6px",
            borderRadius: "var(--radius-xs)",
            backgroundColor: "transparent",
            color: "var(--agent-accent)",
            border: "1.85px dashed var(--agent-accent)",
            display: "inline-flex",
            alignItems: "center",
            gap: "3px",
            cursor: "pointer",
            userSelect: "none",
          }}
          title={`Loopback CTA: ${loopbackFfId}`}
        >
          <span>↺ {loopbackFfId}</span>
        </span>
      )}
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
      type="text"
      data-testid={`rename-node-input-${p.node.id}`}
      autoFocus
      value={p.renameDraft}
      onClick={(e) => e.stopPropagation()}
      onMouseDown={(e) => e.stopPropagation()}
      onChange={(e) => p.onChangeRenameDraft(e.target.value)}
      onBlur={p.onCommitRename}
      onKeyDown={(e) => {
        if (e.key === "Enter") p.onCommitRename();
        if (e.key === "Escape") p.onCancelRename();
      }}
      style={{
        width: "100%",
        fontSize: "12px",
        fontWeight: 600,
        padding: "2px 6px",
        marginBottom: "4px",
        borderRadius: "var(--radius-xs)",
        border: "1px solid var(--action-primary)",
        backgroundColor: "var(--bg-workspace)",
        color: "var(--text-primary)",
        boxSizing: "border-box",
      }}
    />
  ) : (
    <div
      onDoubleClick={(e) => {
        e.stopPropagation();
        p.onStartRename();
      }}
      style={{
        fontSize: "12px",
        fontWeight: 600,
        color: "var(--text-primary)",
        marginBottom: "4px",
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
      }}
    >
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
  onHoverCta?: (ffId: string, hovered: boolean) => void;
}> = (p) => {
  const { t } = useTranslation();
  const activeSubState: NodeSubState = normalizeFixtureRef(p.node.activeSubState);
  const fixtureRef = normalizeFixtureRef(p.node.fixtureRef || activeSubState);
  const extNode = p.node as ExtendedWorkbenchNode;
  const appShellTag = extNode.clusterId || (extNode.metadata as Record<string, unknown> | undefined)?.appShell;
  const resolvedAppShell = typeof appShellTag === "string" ? appShellTag : undefined;

  return (
    <div style={{ padding: "8px 10px", flex: 1, display: "flex", flexDirection: "column" }}>
      {resolvedAppShell && (
        <div
          data-testid={`appshell-badge-${p.node.id}`}
          style={{
            fontSize: "8.5px",
            fontFamily: "var(--font-mono)",
            fontWeight: 600,
            padding: "1px 5px",
            borderRadius: "var(--radius-xs)",
            backgroundColor: "var(--bg-subtle)",
            color: "var(--text-secondary)",
            border: "1px solid var(--border-default)",
            display: "inline-flex",
            alignItems: "center",
            gap: "3px",
            alignSelf: "flex-start",
            marginBottom: "4px",
          }}
        >
          <BrandIcon name="layout" size={9} strokeWidth={1.85} color="currentColor" />
          <span>{t("flow.appShellPrefix", "Shell:")} {resolvedAppShell}</span>
        </div>
      )}
      <DagNodeTitleEditor
        node={p.node}
        isRenaming={p.isRenaming}
        renameDraft={p.renameDraft}
        onChangeRenameDraft={p.onChangeRenameDraft}
        onCommitRename={p.onCommitRename}
        onCancelRename={p.onCancelRename}
        onStartRename={p.onStartRename}
      />
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
          <BrandIcon name="warning" size={10} color="currentColor" strokeWidth={1.85} />
          <span>{t("flow.orphanNodeLabel", "Cô lập")}</span>
        </div>
      )}
      {p.node.routePath && (
        <div
          style={{
            fontSize: "10px",
            fontFamily: "var(--font-mono)",
            color: "var(--text-muted)",
            marginBottom: "6px",
          }}
        >
          {p.node.routePath}
        </div>
      )}

      {/* Option 5 Vertical Storyboard: Compact mini-skeleton preview <= 4KB */}
      <MiniSkeletonWireframe subState={activeSubState} fixtureRef={fixtureRef} />

      {/* 4 Interactive Sub-State pills (Default, Loading, Empty, Error) */}
      <DagNodeSubStatePills
        nodeId={p.node.id}
        activeSubState={activeSubState}
        onSelectSubState={p.onSelectSubState}
      />

      {/* Solid high-contrast Outbound CTA vs dashed Loopback pills with <16ms hover sync */}
      <DagNodeCtaPills node={p.node} onHoverCta={p.onHoverCta} />

      <input
        type="text"
        placeholder={t("flow.addNotePlaceholder", "Thêm ghi chú...")}
        value={p.node.note || ""}
        onChange={(e) => p.onUpdateNote(p.node.id, e.target.value)}
        onClick={(e) => e.stopPropagation()}
        onMouseDown={(e) => e.stopPropagation()}
        style={{
          width: "100%",
          marginTop: "auto",
          fontSize: "10px",
          border: "1px solid var(--border-subtle)",
          borderRadius: "var(--radius-xs)",
          padding: "2px 6px",
          backgroundColor: "var(--bg-canvas)",
          color: "var(--text-secondary)",
          boxSizing: "border-box",
        }}
      />
    </div>
  );
};

export const DagNodeCard: React.FC<DagNodeCardProps> = (props) => {
  const { t } = useTranslation();
  const [isRenaming, setIsRenaming] = useState(false);
  const [renameDraft, setRenameDraft] = useState(props.node.title);
  const isDraft = props.node.status === "draft";
  const startRename = () => {
    setRenameDraft(props.node.title);
    setIsRenaming(true);
  };
  const commitRename = () => {
    setIsRenaming(false);
    const trimmed = renameDraft.trim();
    if (trimmed && trimmed !== props.node.title) props.onRenameNode?.(props.node.id, trimmed);
  };
  const cardStyle: React.CSSProperties = {
    position: "absolute",
    left: `${props.node.posX}px`,
    top: `${props.node.posY}px`,
    width: `${props.nodeWidth}px`,
    minHeight: `${props.nodeHeight}px`,
    backgroundColor: "var(--bg-surface)",
    border: getNodeBorder(props.isSelected, isDraft),
    borderRadius: "var(--radius-md)",
    boxShadow: props.isSelected ? "var(--shadow-md)" : "var(--shadow-sm)",
    pointerEvents: "auto",
    cursor: "move",
    display: "flex",
    flexDirection: "column",
    justifyContent: "space-between",
    boxSizing: "border-box",
  };

  return (
    <div
      className="dag-node-card"
      data-testid={`dag-node-${props.node.id}`}
      aria-label={t("flow.nodeCard", `Node ${props.node.screenKey}`)}
      onMouseDown={(e) => props.onNodeMouseDown(e, props.node)}
      style={cardStyle}
    >
      <DagNodeHeader
        node={props.node}
        isDraft={isDraft}
        onStartRename={startRename}
        onCommentOnNode={props.onCommentOnNode}
        onDuplicateNode={props.onDuplicateNode}
        onDeleteNode={props.onDeleteNode}
        executionStatus={props.executionStatus}
      />
      <DagNodeBody
        node={props.node}
        isOrphan={props.isOrphan}
        isRenaming={isRenaming}
        renameDraft={renameDraft}
        onChangeRenameDraft={setRenameDraft}
        onCommitRename={commitRename}
        onCancelRename={() => setIsRenaming(false)}
        onStartRename={startRename}
        onUpdateNote={props.onUpdateNote}
        onSelectSubState={props.onSelectSubState}
        onHoverCta={props.onHoverCta}
      />
      <DagNodePorts
        node={props.node}
        nodeWidth={props.nodeWidth}
        nodeHeight={props.nodeHeight}
        onPortMouseDown={props.onPortMouseDown}
      />
    </div>
  );
};
