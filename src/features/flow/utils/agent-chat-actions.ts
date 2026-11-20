// @story 04.1, 04.4, 06.4
// @implements AC-02, AC-04 (Shared Conversation Actions, Prompt Composition & Branch Fork without FSD upward deps)
import { apiChangeProposal } from "@/api/change-proposal";
import { apiHarnessDaemon } from "@/api/harness-daemon";
import { apiSdiEngine } from "@/api/sdi-engine";
import { apiVersionSnapshot } from "@/api/version-snapshot";
import { buildUiChangeProposalInput, notifyProposalsChanged } from "@/features/proposals/utils/proposal-events";
import { emitSessionForked, sharedChatStore } from "../store/shared-chat-sync";

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

export interface SendSdiCompileParams {
  projectId: string;
  targetKey: string;
  primaryNodeId?: string;
  sourceCommentId?: string;
  sentPrompt: string;
  formatTime: (d: Date | number) => string;
  formatNumber: (n: number) => string;
  t: (k: string, f: string) => string;
}

async function createUiProposalIfUuid(params: SendSdiCompileParams): Promise<void> {
  if (!UUID_RE.test(params.projectId)) return;
  try {
    await apiChangeProposal.createProposal(
      params.projectId,
      buildUiChangeProposalInput({
        prompt: params.sentPrompt,
        nodeId: params.primaryNodeId,
        sourceCommentId: params.sourceCommentId,
        screenKey: params.primaryNodeId ? undefined : "BATCH",
      })
    );
    notifyProposalsChanged(params.projectId);
  } catch (err) {
    if (typeof console !== "undefined") {
      console.warn("[agent-chat-actions] Unable to sync proposal:", err);
    }
  }
}

export function sendSdiCompile(params: SendSdiCompileParams) {
  void apiSdiEngine
    .compileIntent(params.projectId, { rawPrompt: params.sentPrompt, targetNodeId: params.primaryNodeId })
    .then(async (intent) => {
      const now = Date.now();
      const sdiClarification =
        intent.status === "clarifying" && intent.clarificationVariants?.length
          ? { intentId: intent.id, projectId: params.projectId, status: intent.status, ambiguityScore: intent.ambiguityScore, variants: intent.clarificationVariants, selectedVariantId: intent.selectedVariantId ?? undefined }
          : undefined;
      sharedChatStore.appendMessage(params.targetKey, {
        id: `msg-agent-${intent.id}`, sender: "agent", createdAtMs: now, timestamp: params.formatTime(now), nodeId: params.primaryNodeId,
        text: buildCompiledIntentReply(intent, params.formatNumber, params.t), sdiClarification,
      });
      await createUiProposalIfUuid(params);
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

function joinPromptParts(primary: string, secondary: string, separator: string, fallback: string): string {
  if (primary && secondary) return `${primary}${separator}${secondary}`;
  return primary || secondary || fallback;
}

export function extractSourceCommentId(
  attachmentChips: string[],
  attachmentSnippets?: Record<string, string>
): string | undefined {
  for (const chip of attachmentChips) {
    const raw = attachmentSnippets?.[chip];
    if (!raw) continue;
    try {
      const parsed = JSON.parse(raw) as { kind?: string; commentId?: string };
      if (parsed.kind === "STRUCTURED_COMMENT_ENVELOPE" && typeof parsed.commentId === "string" && parsed.commentId.trim()) {
        return parsed.commentId.trim();
      }
    } catch {
      /* ignore non-JSON attachment snippets */
    }
  }
  return undefined;
}

export function composePromptWithAttachments(
  inputValue: string,
  attachmentChips: string[],
  attachmentSnippets?: Record<string, string>,
  fallbackText?: string
): { displayMessage: string; compiledPrompt: string } {
  const trimmed = inputValue.trim();
  const chipsSummary = attachmentChips.join(", ");
  const snippetBlocks = attachmentChips
    .map((chip) => (attachmentSnippets?.[chip] ? `${chip}\n${attachmentSnippets[chip]}` : ""))
    .filter(Boolean);
  const displayMessage = joinPromptParts(trimmed, chipsSummary, "\n", fallbackText || "");
  const attachmentPayload = snippetBlocks.length > 0 ? snippetBlocks.join("\n\n") : chipsSummary;
  const compiledPrompt = joinPromptParts(trimmed, attachmentPayload, "\n\n", fallbackText || "");
  return { displayMessage, compiledPrompt };
}

function notifyForkFailure(err: unknown): void {
  if (typeof window === "undefined") return;
  const detailMsg = err instanceof Error && err.message ? err.message : "Unable to fork conversation in daemon SQLite.";
  window.dispatchEvent(new CustomEvent("cowok:session-fork-error", { detail: { message: detailMsg } }));
}

async function forkRemoteProjectConversation(opts: {
  pid: string;
  sourceSessionId: string;
  sourceTitle?: string;
  entryKind?: "fe-dag" | "presentation";
  messageId: string;
}): Promise<void> {
  const turns = await apiVersionSnapshot.listTurnSnapshots(opts.pid);
  const matched = turns.find((tr) => tr.sessionId === opts.sourceSessionId && tr.messageId === opts.messageId);
  if (matched) {
    const res = await apiVersionSnapshot.forkSessionFromTurn(opts.pid, matched.id, { includeNodeScope: true });
    emitSessionForked({
      projectId: opts.pid, sourceSessionId: opts.sourceSessionId, forkedSessionId: res.forkedSessionId,
      entryKind: opts.entryKind, cutoffMessageId: opts.messageId, detachedNodeIds: res.detachedNodeIds, copiedMessagesCount: res.copiedMessagesCount,
    });
    return;
  }
  const fallbackTitle = opts.sourceTitle?.trim() ? `${opts.sourceTitle.trim()} (Fork)` : "Branched Session (Fork)";
  const created = await apiHarnessDaemon.createSession(opts.pid, { title: fallbackTitle, entry_kind: opts.entryKind ?? "fe-dag" });
  emitSessionForked({
    projectId: opts.pid, sourceSessionId: opts.sourceSessionId, forkedSessionId: created.id,
    forkedTitle: created.title, entryKind: created.entry_kind ?? opts.entryKind, cutoffMessageId: opts.messageId,
  });
}

export async function forkConversationFromMessage(params: {
  projectId?: string;
  sourceSessionId?: string;
  sourceTitle?: string;
  entryKind?: "fe-dag" | "presentation";
  messageId: string;
}): Promise<void> {
  const sourceSessionId = params.sourceSessionId || "session-1";
  const pid = params.projectId;
  if (pid && UUID_RE.test(pid)) {
    try {
      await forkRemoteProjectConversation({ pid, sourceSessionId, sourceTitle: params.sourceTitle, entryKind: params.entryKind, messageId: params.messageId });
    } catch (err) {
      notifyForkFailure(err);
    }
    return;
  }
  const localForkId = typeof crypto?.randomUUID === "function" ? crypto.randomUUID() : `session-fork-${Date.now()}`;
  emitSessionForked({ projectId: pid, sourceSessionId, forkedSessionId: localForkId, entryKind: params.entryKind, cutoffMessageId: params.messageId });
}

export async function selectSdiClarificationVariant(opts: {
  sessionKey: string;
  intentId: string;
  pid: string;
  variantId: string;
  formatTime: (ts: number) => string;
  t: (k: string, defVal?: string) => string;
}): Promise<void> {
  const { sessionKey, intentId, pid, variantId, formatTime, t } = opts;
  try {
    const updated = await apiSdiEngine.selectClarification(pid, { intentId, selectedVariantId: variantId });
    sharedChatStore.resolveSdiClarification(sessionKey, intentId, variantId, updated.status === "executed" ? "executed" : "ready");
    const picked = updated.clarificationVariants?.find((v) => v.id === variantId)?.title || variantId;
    const ts = Date.now();
    sharedChatStore.appendMessage(sessionKey, {
      id: `clarified-${intentId}-${variantId}`,
      sender: "agent",
      text: `${t("sdi.variantConfirmedPrefix", "Đã chọn phương án")} "${picked}" — ${t("sdi.variantUnlockedSuffix", "intent đã sẵn sàng (ready) và mở khóa thực thi.")}`,
      timestamp: formatTime(ts),
      createdAtMs: ts,
    });
    notifyProposalsChanged(pid);
  } catch (err) {
    const ts = Date.now();
    const errText = err instanceof Error ? err.message : t("chat.daemonConnectError", "Không thể kết nối Rust daemon");
    sharedChatStore.appendMessage(sessionKey, {
      id: `clarify-err-${ts}`,
      sender: "agent",
      text: `${t("chat.daemonSyncError", "Lỗi đồng bộ SDI daemon")}: ${errText}`,
      timestamp: formatTime(ts),
      createdAtMs: ts,
    });
  }
}
