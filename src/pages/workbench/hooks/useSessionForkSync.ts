// @story 05.3, 06.4
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import {
  type SessionForkedEventDetail,
  sharedChatStore,
  toSessionChatKey,
} from "../../../features/flow/store/shared-chat-sync";
import type { ChatSession } from "../types";
import { readPinnedSessionIds, writeStandaloneSessions } from "./usePinnedSessionsStorage";
import type { WorkbenchToastState } from "./useProjectTrashAndBrowse";

export interface SessionForkSyncDeps {
  justNowText: string;
  sessionsRef: React.MutableRefObject<ChatSession[]>;
  setSessions: React.Dispatch<React.SetStateAction<ChatSession[]>>;
  setActiveProjectId: React.Dispatch<React.SetStateAction<string>>;
  setActiveSessionIdState: React.Dispatch<React.SetStateAction<string>>;
  showToast: (toast: WorkbenchToastState) => void;
  t: (k: string, fallback: string) => string;
}

function refreshDaemonProjectSessions(deps: SessionForkSyncDeps, projId: string): void {
  if (!/^[0-9a-f-]{36}$/i.test(projId)) return;
  void apiHarnessDaemon.listSessions(projId).then((rows) => {
    const pinnedIds = readPinnedSessionIds();
    deps.setSessions((prev) => {
      const prevById = new Map(prev.map((s) => [s.id, s]));
      const ids = new Set(rows.map((r) => r.id));
      const mapped: ChatSession[] = rows.map((r) => ({
        id: r.id,
        title: r.title,
        projectId: r.project_id,
        entryKind: r.entry_kind ?? "fe-dag",
        isPinned: pinnedIds.has(r.id),
        updatedAt: prevById.get(r.id)?.updatedAt || deps.justNowText,
      }));
      return [...mapped, ...prev.filter((s) => s.projectId !== projId || !ids.has(s.id))];
    });
  }).catch((err) => {
    const msg = err instanceof Error && err.message ? err.message : deps.t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7");
    deps.showToast({ kind: "warning", message: msg });
  });
}

export function handleSessionForkedEvent(ev: Event, deps: SessionForkSyncDeps): void {
  const detail = (ev as CustomEvent<SessionForkedEventDetail>).detail;
  if (!detail?.forkedSessionId || !detail.sourceSessionId) return;
  const source = deps.sessionsRef.current.find((s) => s.id === detail.sourceSessionId);
  const projId = detail.projectId || source?.projectId;
  sharedChatStore.copyBranchMessages(toSessionChatKey(projId, detail.sourceSessionId), toSessionChatKey(projId, detail.forkedSessionId), detail.cutoffMessageId, detail.cutoffCreatedAtMs);
  const branchTitle = detail.forkedTitle?.trim() || `${source?.title || deps.t("workbench.defaultNewSessionTitle", "Phiên thiết kế mới")} (Fork)`;
  const forkedItem: ChatSession = { id: detail.forkedSessionId, title: branchTitle, projectId: projId, entryKind: detail.entryKind ?? source?.entryKind ?? "fe-dag", updatedAt: deps.justNowText };
  deps.setSessions((prev) => {
    const next = prev.some((s) => s.id === forkedItem.id) ? prev : [forkedItem, ...prev];
    if (!projId) writeStandaloneSessions(next);
    return next;
  });
  if (projId) {
    deps.setActiveProjectId(projId);
    refreshDaemonProjectSessions(deps, projId);
    if (typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent("cowok:project-state-invalidated", {
        detail: { projectId: projId, reason: "fork", detachedNodeIds: detail.detachedNodeIds ?? [] },
      }));
    }
  }
  deps.setActiveSessionIdState(detail.forkedSessionId);
  const detached = detail.detachedNodeIds?.length ?? 0;
  deps.showToast({
    kind: detached > 0 ? "warning" : "success",
    message: detached > 0
      ? deps.t("snapshots.forkDetachedNodesToast", "Đã chuyển sang nhánh mới (một số màn hình cũ không còn tồn tại nên đã tách khỏi phạm vi).")
      : `${deps.t("snapshots.forkSwitchedToast", "Đã rẽ nhánh và chuyển sang phiên")}: ${branchTitle}`,
  });
}
