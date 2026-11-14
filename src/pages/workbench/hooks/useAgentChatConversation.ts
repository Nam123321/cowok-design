// @story 04.1, 04.4
// @implements AC-02, AC-04 (Shared Left-Open Chat Conversation & Reactive V7 Node Sub-Sessions Sync)
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { apiFlowExecution } from "@/api/flow-execution";
import { apiSdiEngine } from "@/api/sdi-engine";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import type { ChatMessage } from "@/components/workbench/ChatPrimitivesBridge";
import { dispatchNodeExecutionToDaemon } from "@/features/flow/hooks/dispatchNodeExecution";
import { useKeyedParallelCoordinator } from "@/features/flow/hooks/useKeyedParallelCoordinator";
import { globalKeyedParallelCoordinator } from "@/features/flow/store/keyed-parallel-coordinator";
import { sharedChatStore, toSessionChatKey } from "@/features/flow/store/shared-chat-sync";
import type { WorkbenchNode } from "@/features/flow/store/flow-workbench-store";
import type { NodeSubSessionItem } from "../types";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function mapDaemonStatus(status?: string): NodeSubSessionItem["status"] {
  if (status === "RUNNING") return "running";
  if (status === "PAUSED") return "paused";
  if (status === "INTERRUPTED") return "interrupted";
  if (status === "QUEUED") return "queued";
  if (status === "COMPLETED") return "completed";
  return "idle";
}

function buildCompiledIntentReply(
  intent: {
    intentType: string;
    status: string;
    ambiguityScore: number;
    clarificationVariants?: Array<{ title: string }>;
    targetScope: { screenKeys: string[] };
  },
  formatNumber: (n: number) => string,
  t: (k: string, defVal?: string) => string
): string {
  const ambiguityPct = formatNumber(Math.round(intent.ambiguityScore * 100));
  const variants = intent.clarificationVariants?.map((v) => v.title).join(" / ") || t("chat.selectOptionPrompt", "Vui lòng chọn phương án");
  const scopeKeys = intent.targetScope.screenKeys.join(", ") || "project";
  if (intent.status === "clarifying") {
    return `${t("chat.clarifyPrefix", "Cần làm rõ yêu cầu")} (${intent.intentType}, ${t("chat.ambiguityLabel", "ambiguity")} ${ambiguityPct}%): ${variants}`;
  }
  return `${t("chat.compiledIntentPrefix", "Đã biên dịch intent")} ${intent.intentType} (${intent.status}) — ${t("chat.scopeLabel", "phạm vi")}: ${scopeKeys}.`;
}

function sendSdiCompile(params: {
  projectId: string;
  targetKey: string;
  primaryNodeId?: string;
  sentPrompt: string;
  formatTime: (d: Date | number) => string;
  formatNumber: (n: number) => string;
  t: (k: string, f: string) => string;
}) {
  void apiSdiEngine
    .compileIntent(params.projectId, { rawPrompt: params.sentPrompt, targetNodeId: params.primaryNodeId })
    .then((intent) => {
      const now = Date.now();
      sharedChatStore.appendMessage(params.targetKey, {
        id: `msg-agent-${intent.id}`, sender: "agent", createdAtMs: now, timestamp: params.formatTime(now), nodeId: params.primaryNodeId,
        text: buildCompiledIntentReply(intent, params.formatNumber, params.t),
      });
    })
    .catch((err) => {
      const now = Date.now();
      const errText = err instanceof Error ? err.message : params.t("chat.daemonConnectError", "Không thể kết nối Rust daemon");
      sharedChatStore.appendMessage(params.targetKey, {
        id: `msg-err-${now}`, sender: "agent", createdAtMs: now, timestamp: params.formatTime(now), nodeId: params.primaryNodeId,
        text: `${params.t("chat.daemonSyncError", "Lỗi đồng bộ SDI daemon")}: ${errText}`,
      });
    });
}

export function useAgentChatConversation(
  attachmentChips: string[],
  onSendMessage?: (text: string) => void,
  projectId?: string,
  activeSessionId?: string,
  selectedNodes?: Array<{ id: string; screenKey: string; title: string }>
) {
  const { t, locale } = useTranslation();
  const { formatTime, formatNumber } = useFormatter({ locale });
  const coordinator = useKeyedParallelCoordinator();
  const snap = useSyncExternalStore(sharedChatStore.subscribe, sharedChatStore.getSnapshot, sharedChatStore.getSnapshot);
  const sessionKey = toSessionChatKey(projectId, activeSessionId);
  const initialGreeting: ChatMessage = useMemo(
    () => ({
      id: "msg-1", sender: "agent",
      text: t("chat.welcomeGreeting", "Chào bạn! Tôi là cowok Agent. Tôi đã sẵn sàng đồng hành cùng bạn xây dựng hệ thống thiết kế và luồng người dùng 2D DAG. Bạn có thể kéo thả đỉnh trên đồ thị, kết nối các màn hình hoặc bôi đen chữ để Quote nhanh vào khung nhập liệu."),
      timestamp: formatTime(new Date().setHours(10, 0, 0, 0)),
    }),
    [t, formatTime]
  );
  const [draftBySession, setDraftBySession] = useState<Record<string, string>>({});
  const [showScrollBottom, setShowScrollBottom] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const shared = sharedChatStore.getMessages(sessionKey);
  const messages = useMemo(
    () => [initialGreeting, ...shared.map((m) => ({ id: m.id, sender: m.sender, text: m.text, timestamp: m.timestamp }))],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [initialGreeting, snap.version, sessionKey]
  );
  const inputValue = draftBySession[sessionKey] ?? "";
  const setInputValue = (val: string) => setDraftBySession((prev) => ({ ...prev, [sessionKey]: val }));
  useEffect(() => { messagesEndRef.current?.scrollIntoView?.({ behavior: "smooth" }); }, [messages, sessionKey]);

  const handleScrollMessages = (e: React.UIEvent<HTMLDivElement>) => {
    const { scrollTop, scrollHeight, clientHeight } = e.currentTarget;
    setShowScrollBottom(scrollHeight - scrollTop - clientHeight > 60);
  };
  const scrollToBottom = () => { messagesEndRef.current?.scrollIntoView?.({ behavior: "smooth" }); setShowScrollBottom(false); };

  const handleSend = () => {
    if (!inputValue.trim() && attachmentChips.length === 0) return;
    const sentPrompt = inputValue.trim() || attachmentChips.join(", ") || t("chat.attachedFallback", "Đính kèm");
    const now = Date.now();
    const primaryNode = selectedNodes?.[0] ?? null;
    sharedChatStore.appendMessage(sessionKey, { id: `msg-${now}`, sender: "human", text: inputValue || sentPrompt, timestamp: formatTime(now), createdAtMs: now, nodeId: primaryNode?.id });
    onSendMessage?.(inputValue);
    setDraftBySession((prev) => ({ ...prev, [sessionKey]: "" }));
    if (!projectId) return;
    if (primaryNode && selectedNodes && selectedNodes.length > 0 && UUID_RE.test(projectId)) {
      void dispatchNodeExecutionToDaemon({
        projectId, sessionId: activeSessionId || "session-1", primaryNode: primaryNode as unknown as WorkbenchNode,
        selectedNodeIds: selectedNodes.map((n) => n.id), prompt: sentPrompt, coordinator, t, formatNumber,
        onAppendMsg: (m) => sharedChatStore.appendMessage(sessionKey, {
          id: m.id, sender: m.role === "user" ? "human" : "agent", text: m.content, timestamp: formatTime(m.timestamp),
          createdAtMs: m.timestamp, nodeId: m.nodeId, projectSeq: m.projectSeq, fencingToken: m.fencingToken,
        }),
      });
      return;
    }
    sendSdiCompile({ projectId, targetKey: sessionKey, primaryNodeId: primaryNode?.id, sentPrompt, formatTime, formatNumber, t });
  };
  return { messages, inputValue, setInputValue, showScrollBottom, messagesEndRef, handleScrollMessages, scrollToBottom, handleSend };
}

export function useV7NodeSubSessions(projectId?: string, activeSessionId?: string) {
  const [bySession, setBySession] = useState<Record<string, NodeSubSessionItem[]>>({});
  const [coordTick, setCoordTick] = useState(0);
  const snap = useSyncExternalStore(sharedChatStore.subscribe, sharedChatStore.getSnapshot, sharedChatStore.getSnapshot);

  useEffect(() => globalKeyedParallelCoordinator.subscribe(() => setCoordTick((n) => n + 1)), []);

  useEffect(() => {
    if (!projectId || !UUID_RE.test(projectId)) return;
    let active = true;
    void apiFlowExecution.listNodeSessions(projectId, activeSessionId).then((res) => {
      if (!active) return;
      const jobMap = new Map(res.jobs.map((j) => [j.compositeScopeKey, j]));
      const grouped: Record<string, NodeSubSessionItem[]> = {};
      for (const s of res.sessions) {
        const job = jobMap.get(s.compositeScopeKey);
        const localStatus = globalKeyedParallelCoordinator.getNodeStatus(s.nodeId, { projectId, sessionId: s.chatSessionId })?.status;
        const safeLocal: NodeSubSessionItem["status"] =
          localStatus && localStatus !== "error" && localStatus !== "cancelled" ? localStatus : "idle";
        const mappedStatus: NodeSubSessionItem["status"] = job ? mapDaemonStatus(job.status) : safeLocal;
        const cleared = sharedChatStore.isScopeCleared(s.compositeScopeKey);
        const arr = grouped[s.chatSessionId] ?? [];
        arr.push({
          nodeId: s.nodeId, nodeTitle: s.title, screenKey: s.nodeId,
          messageCount: cleared ? undefined : (s.unreadCount || undefined),
          status: mappedStatus, compositeScopeKey: s.compositeScopeKey,
        });
        grouped[s.chatSessionId] = arr;
      }
      setBySession(grouped);
    }).catch(() => { /* ignore when offline */ });
    return () => { active = false; };
  }, [projectId, activeSessionId, snap.subSessionsVersion, coordTick]);

  return bySession;
}
