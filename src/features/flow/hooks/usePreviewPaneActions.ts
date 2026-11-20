// @story 04.5
// @implements AC-01, AC-03 (Persistent Node-Nested Comments CAS 409 STALE_MAPPING & Uppercase Fixture Sync Hook)
import { useCallback, useEffect, useRef, useState } from "react";
import { apiElementComment } from "../../../api/element-comment";
import { apiScreenFixture } from "../../../api/screen-fixture";
import { HarnessDaemonApiError } from "../../../api/harness-daemon-transport";
import type { AnchorKind, CommentRegionRect, ElementCommentResponse } from "../../../lib/validations/element-comment";
import { normalizeFixtureRef, type FixtureVariant, type ViewportPreset } from "../../../lib/validations/screen-fixture";
import type { ElementRectBox } from "./useSandboxedIframeBridge";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const DOM_HASH_LEN = 8;
const HTTP_CONFLICT = 409;

function buildLocalComment(p: {
  projectId?: string;
  nodeId?: string | null;
  elementFfId: string;
  content: string;
  sha256: string;
  domHash?: string;
  anchorKind?: AnchorKind;
}): ElementCommentResponse {
  const now = new Date().toISOString();
  return {
    id: `local-${Date.now()}`, tenantId: "local-default", projectId: p.projectId || "local", nodeId: p.nodeId || "node-1",
    elementFfId: p.elementFfId, author: "Designer", content: p.content, domHash: p.domHash || p.sha256.slice(0, DOM_HASH_LEN),
    artifactSha256: p.sha256, anchorKind: p.anchorKind ?? "html_element", anchorStatus: "active", createdAt: now, updatedAt: now,
  };
}

export interface CommentToastState {
  message: string;
  tone: "warning" | "danger";
}

function isStaleMappingErr(err: unknown): err is HarnessDaemonApiError {
  return err instanceof HarnessDaemonApiError && (err.status === HTTP_CONFLICT || err.code === "STALE_MAPPING");
}

function useInitialNodePreviewSync(params: {
  canPersist: boolean;
  projectId?: string;
  selectedNodeId?: string | null;
  setComments: React.Dispatch<React.SetStateAction<ElementCommentResponse[]>>;
  setFixturesByVariant: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  setToastState: React.Dispatch<React.SetStateAction<CommentToastState | null>>;
  onSelectSubState?: (variant: FixtureVariant) => void;
}) {
  const { canPersist, projectId, selectedNodeId, setComments, setFixturesByVariant, setToastState, onSelectSubState } = params;
  const onSelectSubStateRef = useRef(onSelectSubState);
  onSelectSubStateRef.current = onSelectSubState;

  useEffect(() => {
    if (!canPersist || !projectId || !selectedNodeId) return;
    let cancelled = false;
    const hydrate = async () => {
      try {
        const [commentList, fixtureList] = await Promise.all([
          apiElementComment.getComments(projectId, selectedNodeId),
          apiScreenFixture.getFixtures(projectId, selectedNodeId),
        ]);
        if (cancelled) return;
        setComments(commentList);
        setToastState(null);
        const nextMap: Record<string, string> = {};
        let defaultVar: FixtureVariant | null = null;
        for (const f of fixtureList) {
          const v = normalizeFixtureRef(f.variant);
          nextMap[v] = f.payloadJson;
          if (f.isDefault) defaultVar = v;
        }
        setFixturesByVariant(nextMap);
        if (defaultVar) onSelectSubStateRef.current?.(defaultVar);
      } catch (err) {
        if (!cancelled && err instanceof Error) setToastState({ message: err.message, tone: "danger" });
      }
    };
    void hydrate();
    return () => { cancelled = true; };
  }, [canPersist, projectId, selectedNodeId, setComments, setFixturesByVariant, setToastState]);
}

export interface CreateSpatialCommentOptions {
  anchorKind?: AnchorKind;
  domHash?: string;
  regionRect?: CommentRegionRect;
  coveredFfIds?: string[];
}

function useCreateCommentAction(args: {
  canPersist: boolean;
  isReadOnly?: boolean;
  projectId?: string;
  selectedNodeId?: string | null;
  artifactSha256: string;
  setComments: React.Dispatch<React.SetStateAction<ElementCommentResponse[]>>;
  setToastState: React.Dispatch<React.SetStateAction<CommentToastState | null>>;
}) {
  const { canPersist, isReadOnly, projectId, selectedNodeId, artifactSha256, setComments, setToastState } = args;
  const createWithMeta = useCallback(
    async (
      content: string,
      elementFfId: string,
      meta?: CreateSpatialCommentOptions
    ): Promise<ElementCommentResponse | null> => {
      if (isReadOnly) return null;
      const anchorKind: AnchorKind = meta?.anchorKind ?? "html_element";
      const resolvedDomHash = meta?.domHash || artifactSha256.slice(0, DOM_HASH_LEN);
      if (!canPersist || !projectId || !selectedNodeId) {
        const local = buildLocalComment({
          projectId,
          nodeId: selectedNodeId,
          elementFfId,
          content,
          sha256: artifactSha256,
          domHash: resolvedDomHash,
          anchorKind,
        });
        setComments((prev) => [...prev, local]);
        setToastState(null);
        return local;
      }
      try {
        const created = await apiElementComment.createComment(projectId, selectedNodeId, {
          elementFfId,
          author: "Designer",
          content,
          domHash: resolvedDomHash,
          artifactSha256,
          anchorKind,
          regionRect: meta?.regionRect,
          coveredFfIds: meta?.coveredFfIds,
        });
        setComments((prev) => [...prev, created]);
        setToastState(null);
        return created;
      } catch (err) {
        if (isStaleMappingErr(err)) setToastState({ message: err.message || "409 STALE_MAPPING", tone: "warning" });
        else if (err instanceof Error) setToastState({ message: err.message, tone: "danger" });
        return null;
      }
    },
    [artifactSha256, canPersist, isReadOnly, projectId, selectedNodeId, setComments, setToastState]
  );

  const handleAddComment = useCallback(
    async (content: string, elementFfId: string): Promise<boolean> => {
      const created = await createWithMeta(content, elementFfId);
      return Boolean(created);
    },
    [createWithMeta]
  );

  return { handleAddComment, handleCreateSpatialComment: createWithMeta };
}

function useResolveCommentAction(args: {
  canPersist: boolean;
  isReadOnly?: boolean;
  projectId?: string;
  selectedNodeId?: string | null;
  setComments: React.Dispatch<React.SetStateAction<ElementCommentResponse[]>>;
  setToastState: React.Dispatch<React.SetStateAction<CommentToastState | null>>;
}) {
  const { canPersist, isReadOnly, projectId, selectedNodeId, setComments, setToastState } = args;
  return useCallback(async (commentId: string) => {
    if (isReadOnly) return;
    if (!canPersist || !projectId || !selectedNodeId) {
      setComments((prev) => prev.map((c) => (c.id === commentId ? { ...c, anchorStatus: "resolved" } : c)));
      return;
    }
    try {
      const updated = await apiElementComment.resolveComment(projectId, selectedNodeId, commentId);
      setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
      setToastState(null);
    } catch (err) {
      if (isStaleMappingErr(err)) {
        setToastState({ message: err.message, tone: "warning" });
        setComments((prev) => prev.map((c) => (c.id === commentId ? { ...c, anchorStatus: "stale" } : c)));
      } else if (err instanceof Error) {
        setToastState({ message: err.message, tone: "danger" });
      }
    }
  }, [canPersist, isReadOnly, projectId, selectedNodeId, setComments, setToastState]);
}

function useReanchorCommentAction(args: {
  canPersist: boolean;
  isReadOnly?: boolean;
  projectId?: string;
  selectedNodeId?: string | null;
  artifactSha256: string;
  prefillFfId: string;
  setComments: React.Dispatch<React.SetStateAction<ElementCommentResponse[]>>;
  setToastState: React.Dispatch<React.SetStateAction<CommentToastState | null>>;
}) {
  const { canPersist, isReadOnly, projectId, selectedNodeId, artifactSha256, prefillFfId, setComments, setToastState } = args;
  return useCallback(async (commentId: string) => {
    if (isReadOnly) return;
    const targetFfId = prefillFfId || "sec-header";
    const nextDomHash = artifactSha256.slice(0, DOM_HASH_LEN);
    if (!canPersist || !projectId || !selectedNodeId) {
      setComments((prev) => prev.map((c) => (c.id === commentId ? { ...c, elementFfId: targetFfId, domHash: nextDomHash, artifactSha256, anchorStatus: "active" } : c)));
      setToastState(null);
      return;
    }
    try {
      const updated = await apiElementComment.reanchorComment(projectId, selectedNodeId, commentId, { elementFfId: targetFfId, domHash: nextDomHash, artifactSha256 });
      setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
      setToastState(null);
    } catch (err) {
      if (err instanceof Error) setToastState({ message: err.message, tone: "danger" });
    }
  }, [artifactSha256, canPersist, isReadOnly, prefillFfId, projectId, selectedNodeId, setComments, setToastState]);
}

export function usePreviewPaneActions(params: {
  projectId?: string;
  selectedNodeId?: string | null;
  artifactSha256: string;
  elementRects: Record<string, ElementRectBox>;
  postFixtureHydrate: (variant: FixtureVariant, fixtureJson: string) => void;
  onSelectSubState?: (variant: FixtureVariant) => void;
  onSetViewportPreset: (vp: ViewportPreset) => void;
  isReadOnly?: boolean;
}) {
  const [comments, setComments] = useState<ElementCommentResponse[]>([]);
  const [fixturesByVariant, setFixturesByVariant] = useState<Record<string, string>>({});
  const [prefillFfId, setPrefillFfId] = useState<string>("sec-header");
  const [toastState, setToastState] = useState<CommentToastState | null>(null);
  const { projectId, selectedNodeId, postFixtureHydrate, onSelectSubState, onSetViewportPreset, isReadOnly } = params;
  const canPersist = Boolean(projectId && UUID_RE.test(projectId) && selectedNodeId);

  useInitialNodePreviewSync({ canPersist, projectId, selectedNodeId, setComments, setFixturesByVariant, setToastState, onSelectSubState });
  useEffect(() => {
    const onResolved = (ev: Event) => {
      const detail = (ev as CustomEvent<{ commentId?: string }>).detail;
      if (!detail?.commentId) return;
      setComments((prev) =>
        prev.map((c) => (c.id === detail.commentId ? { ...c, anchorStatus: "resolved" } : c))
      );
    };
    const onReopened = (ev: Event) => {
      const detail = (ev as CustomEvent<{ commentId?: string }>).detail;
      if (!detail?.commentId) return;
      setComments((prev) =>
        prev.map((c) => (c.id === detail.commentId ? { ...c, anchorStatus: "active" } : c))
      );
    };
    window.addEventListener("cowok:comment-resolved", onResolved);
    window.addEventListener("cowok:comment-reopened", onReopened);
    return () => {
      window.removeEventListener("cowok:comment-resolved", onResolved);
      window.removeEventListener("cowok:comment-reopened", onReopened);
    };
  }, []);
  const { handleAddComment, handleCreateSpatialComment } = useCreateCommentAction({
    canPersist,
    isReadOnly,
    projectId,
    selectedNodeId,
    artifactSha256: params.artifactSha256,
    setComments,
    setToastState,
  });
  const handleResolveComment = useResolveCommentAction({ canPersist, isReadOnly, projectId, selectedNodeId, setComments, setToastState });
  const handleReanchorComment = useReanchorCommentAction({ canPersist, isReadOnly, projectId, selectedNodeId, artifactSha256: params.artifactSha256, prefillFfId, setComments, setToastState });

  const handleSelectFixtureVariant = useCallback(async (variant: FixtureVariant) => {
    if (isReadOnly) return;
    const normalized = normalizeFixtureRef(variant);
    onSelectSubState?.(normalized);
    const payloadJson = fixturesByVariant[normalized] || JSON.stringify({ substate: normalized });
    postFixtureHydrate(normalized, payloadJson);
    if (canPersist && projectId && selectedNodeId) {
      try {
        await apiScreenFixture.saveFixture(projectId, selectedNodeId, { variant: normalized, payloadJson, isDefault: normalized === "DEFAULT" });
      } catch (err) {
        if (err instanceof Error) setToastState({ message: err.message, tone: "danger" });
      }
    }
  }, [canPersist, fixturesByVariant, isReadOnly, onSelectSubState, postFixtureHydrate, projectId, selectedNodeId]);

  const handlePersistViewport = useCallback(async (vp: ViewportPreset) => {
    if (isReadOnly) return;
    onSetViewportPreset(vp);
    if (canPersist && projectId && selectedNodeId) {
      try {
        await apiScreenFixture.setViewportPreset(projectId, selectedNodeId, vp);
      } catch (err) {
        if (err instanceof Error) setToastState({ message: err.message, tone: "danger" });
      }
    }
  }, [canPersist, isReadOnly, onSetViewportPreset, projectId, selectedNodeId]);

  const collapsedCommentIds = comments.filter((c) => { const r = params.elementRects[c.elementFfId]; return !r || r.width <= 0 || r.height <= 0; }).map((c) => c.id);
  const staleToastMessage = toastState?.message ?? null;
  const setStaleToastMessage = useCallback((msg: string | null) => {
    setToastState(msg ? { message: msg, tone: "warning" } : null);
  }, []);
  return { comments, setComments, prefillFfId, setPrefillFfId, staleToastMessage, toastTone: toastState?.tone ?? "warning", setStaleToastMessage, collapsedCommentIds, handleSelectFixtureVariant, handlePersistViewport, handleAddComment, handleCreateSpatialComment, handleResolveComment, handleReanchorComment };
}
