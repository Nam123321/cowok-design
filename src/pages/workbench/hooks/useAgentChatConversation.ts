// @story 04.1, 04.4
// @implements AC-02, AC-04 (Left-Open Chat Conversation & V7 Node Sub-Sessions Sync)
import { useEffect, useMemo, useRef, useState } from "react";
import { apiFlowExecution } from "@/api/flow-execution";
import { apiSdiEngine } from "@/api/sdi-engine";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import type { ChatMessage } from "@/components/workbench/ChatPrimitivesBridge";
import type { NodeSubSessionItem } from "../types";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

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
  sentPrompt: string;
  appendMessageToSession: (key: string, msg: ChatMessage) => void;
  formatTime: (d: Date | number) => string;
  formatNumber: (n: number) => string;
  t: (k: string, f: string) => string;
}) {
  void apiSdiEngine
    .compileIntent(params.projectId, { rawPrompt: params.sentPrompt })
    .then((intent) => params.appendMessageToSession(params.targetKey, { id: `msg-agent-${intent.id}`, sender: "agent", text: buildCompiledIntentReply(intent, params.formatNumber, params.t), timestamp: params.formatTime(new Date()) }))
    .catch((err) => {
      const errText = err instanceof Error ? err.message : params.t("chat.daemonConnectError", "Không thể kết nối Rust daemon");
      params.appendMessageToSession(params.targetKey, { id: `msg-err-${Date.now()}`, sender: "agent", text: `${params.t("chat.daemonSyncError", "Lỗi đồng bộ SDI daemon")}: ${errText}`, timestamp: params.formatTime(new Date()) });
    });
}

export function useAgentChatConversation(
  attachmentChips: string[], onSendMessage?: (text: string) => void, projectId?: string, activeSessionId?: string
) {
  const { t, locale } = useTranslation();
  const { formatTime, formatNumber } = useFormatter({ locale });
  const sessionKey = activeSessionId || (projectId ? `project:${projectId}` : "default");
  const initialGreeting: ChatMessage = useMemo(
    () => ({
      id: "msg-1", sender: "agent",
      text: t("chat.welcomeGreeting", "Chào bạn! Tôi là cowok Agent. Tôi đã sẵn sàng đồng hành cùng bạn xây dựng hệ thống thiết kế và luồng người dùng 2D DAG. Bạn có thể kéo thả đỉnh trên đồ thị, kết nối các màn hình hoặc bôi đen chữ để Quote nhanh vào khung nhập liệu."),
      timestamp: formatTime(new Date().setHours(10, 0, 0, 0)),
    }),
    [t, formatTime]
  );
  const [messagesBySession, setMessagesBySession] = useState<Record<string, ChatMessage[]>>({});
  const [draftBySession, setDraftBySession] = useState<Record<string, string>>({});
  const [showScrollBottom, setShowScrollBottom] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const messages = useMemo(() => messagesBySession[sessionKey] ?? [initialGreeting], [messagesBySession, sessionKey, initialGreeting]);
  const inputValue = draftBySession[sessionKey] ?? "";
  const setInputValue = (val: string) => setDraftBySession((prev) => ({ ...prev, [sessionKey]: val }));
  useEffect(() => { messagesEndRef.current?.scrollIntoView?.({ behavior: "smooth" }); }, [messages, sessionKey]);

  const appendMessageToSession = (targetKey: string, msg: ChatMessage) => {
    setMessagesBySession((prev) => ({ ...prev, [targetKey]: [...(prev[targetKey] ?? [initialGreeting]), msg] }));
  };
  const handleScrollMessages = (e: React.UIEvent<HTMLDivElement>) => {
    const { scrollTop, scrollHeight, clientHeight } = e.currentTarget;
    setShowScrollBottom(scrollHeight - scrollTop - clientHeight > 60);
  };
  const scrollToBottom = () => { messagesEndRef.current?.scrollIntoView?.({ behavior: "smooth" }); setShowScrollBottom(false); };
  const handleSend = () => {
    if (!inputValue.trim() && attachmentChips.length === 0) return;
    const sentPrompt = inputValue.trim() || attachmentChips.join(", ") || t("chat.attachedFallback", "Đính kèm");
    appendMessageToSession(sessionKey, { id: `msg-${Date.now()}`, sender: "human", text: inputValue || sentPrompt, timestamp: formatTime(new Date()) });
    onSendMessage?.(inputValue);
    setDraftBySession((prev) => ({ ...prev, [sessionKey]: "" }));
    if (projectId) sendSdiCompile({ projectId, targetKey: sessionKey, sentPrompt, appendMessageToSession, formatTime, formatNumber, t });
  };
  return { messages, inputValue, setInputValue, showScrollBottom, messagesEndRef, handleScrollMessages, scrollToBottom, handleSend };
}

export function useV7NodeSubSessions(projectId?: string, activeSessionId?: string) {
  const [bySession, setBySession] = useState<Record<string, NodeSubSessionItem[]>>({});

  useEffect(() => {
    if (!projectId || !UUID_RE.test(projectId)) return;
    void apiFlowExecution.listNodeSessions(projectId, activeSessionId).then((res) => {
      const jobMap = new Map(res.jobs.map((j) => [j.compositeScopeKey, j]));
      const grouped: Record<string, NodeSubSessionItem[]> = {};
      for (const s of res.sessions) {
        const job = jobMap.get(s.compositeScopeKey);
        const mappedStatus = job?.status === "RUNNING" ? "running" : job?.status === "PAUSED" ? "paused" : job?.status === "INTERRUPTED" ? "interrupted" : job?.status === "QUEUED" ? "queued" : job?.status === "COMPLETED" ? "completed" : "idle";
        const arr = grouped[s.chatSessionId] ?? [];
        arr.push({ nodeId: s.nodeId, nodeTitle: s.title, screenKey: s.nodeId, messageCount: s.unreadCount || undefined, status: mappedStatus, compositeScopeKey: s.compositeScopeKey });
        grouped[s.chatSessionId] = arr;
      }
      setBySession(grouped);
    }).catch(() => { /* ignore when offline */ });
  }, [projectId, activeSessionId]);

  return bySession;
}
