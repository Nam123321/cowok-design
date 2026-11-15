// @story 04.4
// @implements AC-01, AC-02 (Contextual Node Floating Chat Controller & State Hook)
import { useState, useRef, useEffect, useCallback } from "react";
import { apiWorkbenchSettings } from "@/api/workbench-settings";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import type { WorkbenchNode } from "../store/flow-workbench-store";
import type { NodeJobStatus } from "../store/keyed-parallel-coordinator";
import { useKeyedParallelCoordinator } from "./useKeyedParallelCoordinator";
import { useFloatingWindowGeometry } from "./useFloatingWindowGeometry";
import { dispatchNodeJobToDaemon } from "./useFloatingChatDaemonSync";
import type { FloatingChatMessage } from "../components/FloatingChatStreamPanel";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export interface ContextualNodeFloatingChatProps {
  projectId: string;
  sessionId: string;
  selectedNodes: WorkbenchNode[];
  onDeselectNode: (nodeId: string) => void;
  onClearSelection: () => void;
  onOpenLeftChat?: () => void;
  onFocusNode?: (nodeId: string) => void;
  onAttachContext?: (content: string) => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
  defaultExpanded?: boolean;
  anchorX?: number;
  anchorY?: number;
}

interface SteerOptions {
  projectId: string;
  sessionId: string;
  jobId: string;
  nodeId: string;
  instruction: string;
  onAppendMsg: (msg: FloatingChatMessage) => void;
  t: (key: string, fallback: string) => string;
}

async function dispatchSteerToDaemon(opts: SteerOptions): Promise<void> {
  if (!UUID_RE.test(opts.projectId)) return;
  try {
    await apiWorkbenchSettings.postJobEvent(opts.projectId, {
      jobId: opts.jobId,
      nodeId: opts.nodeId,
      sessionId: opts.sessionId,
      eventKind: "steered",
      payloadText: JSON.stringify({ instruction: opts.instruction }),
      jobStatus: "RUNNING",
    });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : "steer_failed";
    opts.onAppendMsg({
      id: `steer-err-${Date.now()}`,
      nodeId: opts.nodeId,
      role: "assistant",
      content: `${opts.t("floatingChat.daemonSyncError", "Không thể đồng bộ với Rust daemon")}: ${msg}`,
      timestamp: Date.now(),
    });
  }
}

function useScrollAndCleanup(
  messages: FloatingChatMessage[],
  status: string | undefined,
  historyEndRef: React.RefObject<HTMLDivElement | null>,
  streamAbortRef: React.MutableRefObject<AbortController | null>
) {
  useEffect(() => {
    historyEndRef.current?.scrollIntoView?.({ behavior: "smooth" });
  }, [messages, status, historyEndRef]);

  useEffect(() => () => {
    streamAbortRef.current?.abort();
  }, [streamAbortRef]);
}

function useChatUIState(defaultExpanded?: boolean) {
  const [isCollapsed, setIsCollapsed] = useState(!defaultExpanded);
  const [showReviewSummary, setShowReviewSummary] = useState(false);
  const [messages, setMessages] = useState<FloatingChatMessage[]>([]);
  const [inputPrompt, setInputPrompt] = useState("");
  const [steerDraft, setSteerDraft] = useState("");
  const [isSteering, setIsSteering] = useState(false);
  const appendMsg = useCallback((m: FloatingChatMessage) => setMessages((prev) => [...prev, m]), []);

  return {
    isCollapsed,
    setIsCollapsed,
    showReviewSummary,
    setShowReviewSummary,
    messages,
    inputPrompt,
    setInputPrompt,
    steerDraft,
    setSteerDraft,
    isSteering,
    setIsSteering,
    appendMsg,
  };
}

interface SendMessageHandlerOptions {
  inputPrompt: string;
  setInputPrompt: (v: string) => void;
  selectedNodes: WorkbenchNode[];
  primaryNode: WorkbenchNode | null;
  projectId: string;
  sessionId: string;
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
  streamAbortRef: React.MutableRefObject<AbortController | null>;
  appendMsg: (m: FloatingChatMessage) => void;
  t: (key: string, fallback: string) => string;
  formatNumber: (value: number) => string;
}

function useSendMessageHandler(opts: SendMessageHandlerOptions) {
  return useCallback(() => {
    if (!opts.inputPrompt.trim() || opts.selectedNodes.length === 0) return;
    const prompt = opts.inputPrompt.trim();
    opts.setInputPrompt("");
    opts.appendMsg({
      id: `msg-${Date.now()}`,
      nodeId: opts.primaryNode?.id || "unknown",
      role: "user",
      content: prompt,
      timestamp: Date.now(),
    });
    const jobs = opts.selectedNodes.map((n) =>
      opts.coordinator.enqueueJob({ projectId: opts.projectId, sessionId: opts.sessionId, nodeId: n.id, prompt })
    );
    void dispatchNodeJobToDaemon({
      projectId: opts.projectId,
      sessionId: opts.sessionId,
      primaryNode: opts.primaryNode,
      prompt,
      jobs,
      coordinator: opts.coordinator,
      streamAbortRef: opts.streamAbortRef,
      onAppendMsg: opts.appendMsg,
      t: opts.t,
      formatNumber: opts.formatNumber,
    });
  }, [opts]);
}

interface SteerHandlerOptions {
  steerDraft: string;
  setSteerDraft: (v: string) => void;
  setIsSteering: (v: boolean) => void;
  primaryNode: WorkbenchNode | null;
  primaryStatus: { status: NodeJobStatus; queuePosition: number; jobId?: string } | null;
  projectId: string;
  sessionId: string;
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
  appendMsg: (m: FloatingChatMessage) => void;
  t: (key: string, fallback: string) => string;
}

function useSteerHandler(opts: SteerHandlerOptions) {
  return useCallback(() => {
    if (!opts.steerDraft.trim() || !opts.primaryStatus?.jobId || !opts.primaryNode) return;
    const instruction = opts.steerDraft.trim();
    opts.coordinator.steerJob(opts.primaryStatus.jobId, instruction);
    opts.appendMsg({
      id: `steer-${Date.now()}`,
      nodeId: opts.primaryNode.id,
      role: "user",
      content: `${opts.t("floatingChat.steerPrefix", "Chỉ dẫn thêm (Steer)")}: ${instruction}`,
      timestamp: Date.now(),
    });
    void dispatchSteerToDaemon({
      projectId: opts.projectId,
      sessionId: opts.sessionId,
      jobId: opts.primaryStatus.jobId,
      nodeId: opts.primaryNode.id,
      instruction,
      onAppendMsg: opts.appendMsg,
      t: opts.t,
    });
    opts.setSteerDraft("");
    opts.setIsSteering(false);
  }, [opts]);
}

function useChatRefs() {
  const containerRef = useRef<HTMLDivElement>(null);
  const historyEndRef = useRef<HTMLDivElement>(null);
  const streamAbortRef = useRef<AbortController | null>(null);
  return { containerRef, historyEndRef, streamAbortRef };
}

export function useFloatingChatController(props: ContextualNodeFloatingChatProps) {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const coordinator = useKeyedParallelCoordinator();
  const ui = useChatUIState(props.defaultExpanded);
  const geo = useFloatingWindowGeometry(ui.isCollapsed, props.anchorX, props.anchorY);
  const refs = useChatRefs();

  const primaryNode = props.selectedNodes[0] || null;
  const primaryStatus = primaryNode ? coordinator.getNodeStatus(primaryNode.id) : null;
  useScrollAndCleanup(ui.messages, primaryStatus?.status, refs.historyEndRef, refs.streamAbortRef);

  const handleSendMessage = useSendMessageHandler({
    inputPrompt: ui.inputPrompt,
    setInputPrompt: ui.setInputPrompt,
    selectedNodes: props.selectedNodes,
    primaryNode,
    projectId: props.projectId,
    sessionId: props.sessionId,
    coordinator,
    streamAbortRef: refs.streamAbortRef,
    appendMsg: ui.appendMsg,
    t,
    formatNumber,
  });

  const handleSteerSubmit = useSteerHandler({
    steerDraft: ui.steerDraft,
    setSteerDraft: ui.setSteerDraft,
    setIsSteering: ui.setIsSteering,
    primaryNode,
    primaryStatus,
    projectId: props.projectId,
    sessionId: props.sessionId,
    coordinator,
    appendMsg: ui.appendMsg,
    t,
  });

  return {
    coordinator,
    ...ui,
    geo,
    ...refs,
    primaryNode,
    primaryStatus,
    handleSendMessage,
    handleSteerSubmit,
  };
}
