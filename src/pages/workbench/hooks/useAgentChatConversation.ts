// @story 04.1, 04.4
// @implements AC-02, AC-04 (Shared Left-Open Chat Conversation & Reactive V7 Node Sub-Sessions Sync)
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { apiChatVault } from "@/api/chat-vault";
import { apiFlowExecution } from "@/api/flow-execution";
import type { VaultReceiptDto } from "@/lib/validations/chat-vault";
import { inspectChatSecretCandidate } from "@/features/chat-vault/secret-interceptor";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import type { ChatMessage } from "@/components/workbench/ChatPrimitivesBridge";
import { dispatchNodeExecutionToDaemon } from "@/features/flow/hooks/dispatchNodeExecution";
import { useKeyedParallelCoordinator } from "@/features/flow/hooks/useKeyedParallelCoordinator";
import { globalKeyedParallelCoordinator } from "@/features/flow/store/keyed-parallel-coordinator";
import { sharedChatStore, toSessionChatKey } from "@/features/flow/store/shared-chat-sync";
import type { WorkbenchNode } from "@/features/flow/store/flow-workbench-store";
import {
  composePromptWithAttachments,
  extractSourceCommentId,
  forkConversationFromMessage,
  selectSdiClarificationVariant,
  sendSdiCompile,
  type SendSdiCompileParams,
} from "@/features/flow/utils/agent-chat-actions";
import type { NodeSubSessionItem } from "../types";

export { composePromptWithAttachments, forkConversationFromMessage, sendSdiCompile, type SendSdiCompileParams };

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function mapDaemonStatus(status?: string): NodeSubSessionItem["status"] {
  if (status === "RUNNING") return "running";
  if (status === "PAUSED") return "paused";
  if (status === "INTERRUPTED") return "interrupted";
  if (status === "QUEUED") return "queued";
  if (status === "COMPLETED") return "completed";
  return "idle";
}

function useChatReceiptSubscription(
  projectId: string | undefined,
  sessionKey: string,
  formatTime: (v: number) => string
) {
  useEffect(() => {
    const onReceipt = (ev: Event) => {
      const detail = (ev as CustomEvent<{ projectId?: string; text?: string }>).detail;
      if (!detail?.text || (detail.projectId && projectId && detail.projectId !== projectId)) return;
      const ts = Date.now();
      sharedChatStore.appendMessage(sessionKey, {
        id: `receipt-${ts}-${Math.random().toString(36).slice(2, 6)}`,
        sender: "agent",
        text: detail.text,
        timestamp: formatTime(ts),
        createdAtMs: ts,
      });
    };
    window.addEventListener("cowok:chat-receipt", onReceipt);
    return () => window.removeEventListener("cowok:chat-receipt", onReceipt);
  }, [formatTime, projectId, sessionKey]);
}

export interface UseAgentChatConversationOpts {
  attachmentChips: string[];
  onSendMessage?: (text: string) => void;
  projectId?: string;
  activeSessionId?: string;
  selectedNodes?: Array<{ id: string; screenKey: string; title: string }>;
  attachmentSnippets?: Record<string, string>;
  onClearAttachments?: () => void;
}

const CHAT_SEND_DEDUP_MS = 350;
const SCROLL_BOTTOM_THRESHOLD_PX = 60;
const RANDOM_ID_RADIX = 36;
const GREETING_HOUR = 10;

async function ingestDetectedTurnSecrets(params: {
  sessionKey: string;
  activeSid: string;
  projectId?: string;
  nodeId?: string;
  displayMessage: string;
  now: number;
  msgSuffix: string;
  localOnly: boolean;
  formatTime: (v: number) => string;
  compiledCheck: ReturnType<typeof inspectChatSecretCandidate>;
}): Promise<void> {
  const displayCheck = inspectChatSecretCandidate(params.displayMessage);
  const receipts: VaultReceiptDto[] = [];
  for (const entry of params.compiledCheck.detectedSecrets) {
    const rcpt = await apiChatVault.ingestVaultSecret({
      sessionId: params.activeSid,
      projectId: params.projectId ?? null,
      provider: entry.provider,
      rawSecretEnvelope: entry.rawSecretEnvelope,
      enterpriseLocalOnlyLock: params.localOnly,
    });
    receipts.push(rcpt);
  }
  await sharedChatStore.persistAndAppend(
    params.sessionKey,
    params.activeSid,
    { id: `msg-${params.now}-${params.msgSuffix}`, sender: "human", text: displayCheck.redactedPreview, timestamp: params.formatTime(params.now), createdAtMs: params.now, nodeId: params.nodeId, vaultReceipt: receipts[0] },
    params.projectId
  );
  for (const [idx, extra] of receipts.slice(1).entries()) {
    await sharedChatStore.persistAndAppend(
      params.sessionKey,
      params.activeSid,
      { id: `msg-${params.now}-${params.msgSuffix}-r${idx + 1}`, sender: "human", text: `[VAULTED:${extra.provider}] ${extra.presenterMask}`, timestamp: params.formatTime(params.now), createdAtMs: params.now, nodeId: params.nodeId, vaultReceipt: extra },
      params.projectId
    );
  }
}

function handleSecretTurnDispatch(params: {
  sessionKey: string;
  activeSid: string;
  conv: UseAgentChatConversationOpts;
  displayMessage: string;
  compiledCheck: ReturnType<typeof inspectChatSecretCandidate>;
  nodeId?: string;
  now: number;
  msgSuffix: string;
  localOnly: boolean;
  clearInput: () => void;
  setChatVaultError: (msg: string | null) => void;
  formatTime: (v: number) => string;
  t: (k: string, f: string) => string;
}) {
  params.setChatVaultError(null);
  void ingestDetectedTurnSecrets({
    sessionKey: params.sessionKey, activeSid: params.activeSid, projectId: params.conv.projectId, nodeId: params.nodeId,
    displayMessage: params.displayMessage, now: params.now, msgSuffix: params.msgSuffix, localOnly: params.localOnly,
    formatTime: params.formatTime, compiledCheck: params.compiledCheck,
  })
    .then(() => { params.conv.onClearAttachments?.(); params.clearInput(); })
    .catch((err) => { params.setChatVaultError(err instanceof Error ? err.message : params.t("vault.ingestFailed", "Vault ingest failed")); });
}

function dispatchConversationTurn(opts: {
  sessionKey: string;
  inputValue: string;
  conv: UseAgentChatConversationOpts;
  lastSendRef: React.MutableRefObject<{ at: number; displayMessage: string } | null>;
  clearInput: () => void;
  enterpriseLocalOnlyLock: boolean;
  setChatVaultError: (msg: string | null) => void;
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
  t: (k: string, f: string) => string;
  formatTime: (v: number) => string;
  formatNumber: (n: number) => string;
}) {
  const { sessionKey, inputValue, conv, lastSendRef, clearInput, enterpriseLocalOnlyLock, setChatVaultError, coordinator, t, formatTime, formatNumber } = opts;
  if (!inputValue.trim() && conv.attachmentChips.length === 0) return;
  const { displayMessage, compiledPrompt } = composePromptWithAttachments(inputValue, conv.attachmentChips, conv.attachmentSnippets, t("chat.attachedFallback", "Đính kèm"));
  const sourceCommentId = extractSourceCommentId(conv.attachmentChips, conv.attachmentSnippets);
  const now = Date.now();
  if (lastSendRef.current && now - lastSendRef.current.at < CHAT_SEND_DEDUP_MS && lastSendRef.current.displayMessage === displayMessage) return;
  lastSendRef.current = { at: now, displayMessage };
  const msgSuffix = Math.random().toString(RANDOM_ID_RADIX).slice(2, 6);
  const primaryNode = conv.selectedNodes?.[0] ?? null;
  const activeSid = conv.activeSessionId || "session-1";
  const secretCheck = inspectChatSecretCandidate(compiledPrompt);
  if (secretCheck.detected && secretCheck.detectedSecrets.length > 0) {
    handleSecretTurnDispatch({ sessionKey, activeSid, conv, displayMessage, compiledCheck: secretCheck, nodeId: primaryNode?.id, now, msgSuffix, localOnly: enterpriseLocalOnlyLock, clearInput, setChatVaultError, formatTime, t });
    return;
  }
  const humanMsg = { id: `msg-${now}-${msgSuffix}`, sender: "human" as const, text: displayMessage, timestamp: formatTime(now), createdAtMs: now, nodeId: primaryNode?.id };
  void sharedChatStore.persistAndAppend(sessionKey, activeSid, humanMsg, conv.projectId).catch(() => { /* ignore when daemon is offline */ });
  conv.onSendMessage?.(compiledPrompt);
  conv.onClearAttachments?.();
  clearInput();
  if (!conv.projectId) {
    const ackMsg = { id: `msg-standalone-${now}`, sender: "agent" as const, text: `${t("workbench.standaloneChatAck", "Đã ghi nhận trao đổi trong phiên độc lập")}: ${displayMessage}`, timestamp: formatTime(now), createdAtMs: now };
    void sharedChatStore.persistAndAppend(sessionKey, activeSid, ackMsg, undefined).catch(() => { /* ignore when daemon is offline */ });
    return;
  }
  if (primaryNode && conv.selectedNodes && conv.selectedNodes.length > 0 && UUID_RE.test(conv.projectId)) {
    void dispatchNodeExecutionToDaemon({
      projectId: conv.projectId, sessionId: conv.activeSessionId || "session-1", primaryNode: primaryNode as unknown as WorkbenchNode,
      selectedNodeIds: conv.selectedNodes.map((n) => n.id), prompt: compiledPrompt, sourceCommentId, coordinator, t, formatNumber,
      onAppendMsg: (m) => void sharedChatStore.persistAndAppend(sessionKey, activeSid, { id: m.id, sender: m.role === "user" ? "human" : "agent", text: m.content, timestamp: formatTime(m.timestamp), createdAtMs: m.timestamp, nodeId: m.nodeId, projectSeq: m.projectSeq, fencingToken: m.fencingToken, sdiClarification: m.sdiClarification }, conv.projectId).catch(() => {}),
    });
    return;
  }
  sendSdiCompile({ projectId: conv.projectId, targetKey: sessionKey, primaryNodeId: primaryNode?.id, sourceCommentId, sentPrompt: compiledPrompt, formatTime, formatNumber, t });
}

export function useAgentChatConversation(opts: UseAgentChatConversationOpts) {
  const { t, locale } = useTranslation();
  const { formatTime, formatNumber } = useFormatter({ locale });
  const coordinator = useKeyedParallelCoordinator();
  const snap = useSyncExternalStore(sharedChatStore.subscribe, sharedChatStore.getSnapshot, sharedChatStore.getSnapshot);
  const sessionKey = toSessionChatKey(opts.projectId, opts.activeSessionId);
  const initialGreeting = useMemo<ChatMessage>(() => ({
    id: "msg-1", sender: "agent",
    text: t("chat.welcomeGreeting", "Chào bạn! Tôi là cowok Agent. Tôi đã sẵn sàng đồng hành cùng bạn xây dựng hệ thống thiết kế và luồng người dùng 2D DAG. Bạn có thể kéo thả đỉnh trên đồ thị, kết nối các màn hình hoặc bôi đen chữ để Quote nhanh vào khung nhập liệu."),
    timestamp: formatTime(new Date().setHours(GREETING_HOUR, 0, 0, 0)),
  }), [t, formatTime]);
  const [showScrollBottom, setShowScrollBottom] = useState(false);
  const [enterpriseLocalOnlyLock, setEnterpriseLocalOnlyLock] = useState(false);
  const [chatVaultError, setChatVaultError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const lastSendRef = useRef<{ at: number; displayMessage: string } | null>(null);
  const shared = sharedChatStore.getMessages(sessionKey);
  const messages = useMemo(() => [initialGreeting, ...shared.map((m) => ({ id: m.id, sender: m.sender, text: m.text, timestamp: m.timestamp, vaultReceipt: m.vaultReceipt ?? null, sdiClarification: m.sdiClarification }))], [initialGreeting, snap.version, sessionKey]);
  const inputValue = sharedChatStore.getDraft(sessionKey);
  const setInputValue = (val: string) => sharedChatStore.setDraft(sessionKey, val);
  useEffect(() => { messagesEndRef.current?.scrollIntoView?.({ behavior: "smooth" }); }, [messages, sessionKey]);
  useEffect(() => {
    const sid = opts.activeSessionId;
    if (!sid) return;
    let active = true;
    void apiChatVault
      .getMessages(sid)
      .then((res) => {
        if (!active) return;
        for (const m of res.messages) {
          sharedChatStore.appendMessage(sessionKey, {
            id: m.id,
            sender: m.role === "user" ? "human" : "agent",
            text: m.proseMarkdown,
            timestamp: formatTime(m.createdAt),
            createdAtMs: m.createdAt,
            projectSeq: m.seqNo,
            vaultReceipt: m.vaultReceipt ?? null,
          });
        }
      })
      .catch(() => { /* ignore when offline or unprovisioned */ });
    return () => { active = false; };
  }, [opts.activeSessionId, sessionKey, formatTime]);
  useChatReceiptSubscription(opts.projectId, sessionKey, formatTime);

  const handleSelectSdiVariant = (intentId: string, pid: string, variantId: string) =>
    selectSdiClarificationVariant({ sessionKey, intentId, pid, variantId, formatTime, t });

  const handleScrollMessages = (e: React.UIEvent<HTMLDivElement>) => setShowScrollBottom(e.currentTarget.scrollHeight - e.currentTarget.scrollTop - e.currentTarget.clientHeight > SCROLL_BOTTOM_THRESHOLD_PX);
  const scrollToBottom = () => { messagesEndRef.current?.scrollIntoView?.({ behavior: "smooth" }); setShowScrollBottom(false); };
  const handleSend = () => dispatchConversationTurn({ sessionKey, inputValue, conv: opts, lastSendRef, clearInput: () => setInputValue(""), enterpriseLocalOnlyLock, setChatVaultError, coordinator, t, formatTime, formatNumber });
  const handleForkFromMessage = (msg: ChatMessage) => forkConversationFromMessage({ projectId: opts.projectId, sourceSessionId: opts.activeSessionId, messageId: msg.id });
  const handleReplaceVaultReceipt = async (receiptId: string, nextSecret: string) => {
    const existing = sharedChatStore.getMessages(sessionKey).find((m) => m.vaultReceipt?.receiptId === receiptId)?.vaultReceipt;
    const inferred = inspectChatSecretCandidate(nextSecret).provider;
    const provider = (inferred || existing?.provider || "custom") as "google_ai_studio" | "typesafe_jev" | "anthropic" | "openai" | "custom";
    const updated = await apiChatVault.replaceVaultReceipt(receiptId, { provider, rawSecretEnvelope: nextSecret, enterpriseLocalOnlyLock });
    sharedChatStore.updateVaultReceipt(sessionKey, receiptId, updated);
  };
  const handleRevokeVaultReceipt = async (receiptId: string) => {
    await apiChatVault.revokeVaultReceipt(receiptId);
    sharedChatStore.updateVaultReceipt(sessionKey, receiptId, null);
  };

  return { messages, inputValue, setInputValue, showScrollBottom, messagesEndRef, handleScrollMessages, scrollToBottom, handleSend, handleSelectSdiVariant, handleForkFromMessage, handleReplaceVaultReceipt, handleRevokeVaultReceipt, enterpriseLocalOnlyLock, setEnterpriseLocalOnlyLock, chatVaultError, setChatVaultError };
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
