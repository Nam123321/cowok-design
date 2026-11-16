// @story 04.5
// @implements AC-01, AC-03 (Persistent Node-Nested Comments CAS 409 STALE_MAPPING & Uppercase Fixture Sync Hook)
import { useCallback, useEffect, useState } from "react";
import { apiElementComment } from "../../../api/element-comment";
import { apiScreenFixture } from "../../../api/screen-fixture";
import { HarnessDaemonApiError } from "../../../api/harness-daemon-transport";
import type { ElementCommentResponse } from "../../../lib/validations/element-comment";
import { normalizeFixtureRef, type FixtureVariant, type ViewportPreset } from "../../../lib/validations/screen-fixture";
import type { ElementRectBox } from "./useSandboxedIframeBridge";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const DOM_HASH_LEN = 8;
const HTTP_CONFLICT = 409;

function buildLocalComment(p: { projectId?: string; nodeId?: string | null; elementFfId: string; content: string; sha256: string }): ElementCommentResponse {
  const now = new Date().toISOString();
  return {
    id: `local-${Date.now()}`, tenantId: "local-default", projectId: p.projectId || "local", nodeId: p.nodeId || "node-1",
    elementFfId: p.elementFfId, author: "Designer", content: p.content, domHash: p.sha256.slice(0, DOM_HASH_LEN),
    artifactSha256: p.sha256, anchorKind: "html_element", anchorStatus: "active", createdAt: now, updatedAt: now,
  };
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
  setStaleToastMessage: React.Dispatch<React.SetStateAction<string | null>>;
  onSelectSubState?: (variant: FixtureVariant) => void;
}) {
  const { canPersist, projectId, selectedNodeId, setComments, setFixturesByVariant, setStaleToastMessage, onSelectSubState } = params;
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
        const nextMap: Record<string, string> = {};
        let defaultVar: FixtureVariant | null = null;
        for (const f of fixtureList) {
          const v = normalizeFixtureRef(f.variant);
          nextMap[v] = f.payloadJson;
          if (f.isDefault) defaultVar = v;
        }
        setFixturesByVariant(nextMap);
        if (defaultVar) onSelectSubState?.(defaultVar);
      } catch (err) {
        if (!cancelled && isStaleMappingErr(err)) setStaleToastMessage(err.message);
      }
    };
    void hydrate();
    return () => { cancelled = true; };
  }, [canPersist, projectId, selectedNodeId, setComments, setFixturesByVariant, setStaleToastMessage, onSelectSubState]);
}

function useCreateCommentAction(args: {
  canPersist: boolean;
  projectId?: string;
  selectedNodeId?: string | null;
  artifactSha256: string;
  setComments: React.Dispatch<React.SetStateAction<ElementCommentResponse[]>>;
  setStaleToastMessage: React.Dispatch<React.SetStateAction<string | null>>;
}) {
  const { canPersist, projectId, selectedNodeId, artifactSha256, setComments, setStaleToastMessage } = args;
  return useCallback(async (content: string, elementFfId: string) => {
    if (!canPersist || !projectId || !selectedNodeId) {
      setComments((prev) => [...prev, buildLocalComment({ projectId, nodeId: selectedNodeId, elementFfId, content, sha256: artifactSha256 })]);
      return;
    }
    try {
      const created = await apiElementComment.createComment(projectId, selectedNodeId, {
        elementFfId, author: "Designer", content, domHash: artifactSha256.slice(0, DOM_HASH_LEN), artifactSha256, anchorKind: "html_element",
      });
      setComments((prev) => [...prev, created]);
      setStaleToastMessage(null);
    } catch (err) {
      if (isStaleMappingErr(err)) setStaleToastMessage(err.message || "409 STALE_MAPPING");
    }
  }, [artifactSha256, canPersist, projectId, selectedNodeId, setComments, setStaleToastMessage]);
}

function useMutateCommentActions(args: {
  canPersist: boolean;
  projectId?: string;
  selectedNodeId?: string | null;
  artifactSha256: string;
  prefillFfId: string;
  setComments: React.Dispatch<React.SetStateAction<ElementCommentResponse[]>>;
  setStaleToastMessage: React.Dispatch<React.SetStateAction<string | null>>;
}) {
  const { canPersist, projectId, selectedNodeId, artifactSha256, prefillFfId, setComments, setStaleToastMessage } = args;
  const handleResolveComment = useCallback(async (commentId: string) => {
    if (!canPersist || !projectId || !selectedNodeId) {
      setComments((prev) => prev.map((c) => (c.id === commentId ? { ...c, anchorStatus: "resolved" } : c)));
      return;
    }
    try {
      const updated = await apiElementComment.resolveComment(projectId, selectedNodeId, commentId);
      setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
    } catch (err) {
      if (isStaleMappingErr(err)) {
        setStaleToastMessage(err.message);
        setComments((prev) => prev.map((c) => (c.id === commentId ? { ...c, anchorStatus: "stale" } : c)));
      }
    }
  }, [canPersist, projectId, selectedNodeId, setComments, setStaleToastMessage]);

  const handleReanchorComment = useCallback(async (commentId: string) => {
    const targetFfId = prefillFfId || "sec-header";
    const nextDomHash = artifactSha256.slice(0, DOM_HASH_LEN);
    if (!canPersist || !projectId || !selectedNodeId) {
      setComments((prev) => prev.map((c) => (c.id === commentId ? { ...c, elementFfId: targetFfId, domHash: nextDomHash, artifactSha256, anchorStatus: "active" } : c)));
      setStaleToastMessage(null);
      return;
    }
    const updated = await apiElementComment.reanchorComment(projectId, selectedNodeId, commentId, { elementFfId: targetFfId, domHash: nextDomHash, artifactSha256 });
    setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
    setStaleToastMessage(null);
  }, [artifactSha256, canPersist, prefillFfId, projectId, selectedNodeId, setComments, setStaleToastMessage]);

  return { handleResolveComment, handleReanchorComment };
}

export function usePreviewPaneActions(params: {
  projectId?: string;
  selectedNodeId?: string | null;
  artifactSha256: string;
  elementRects: Record<string, ElementRectBox>;
  postFixtureHydrate: (variant: FixtureVariant, fixtureJson: string) => void;
  onSelectSubState?: (variant: FixtureVariant) => void;
  onSetViewportPreset: (vp: ViewportPreset) => void;
}) {
  const [comments, setComments] = useState<ElementCommentResponse[]>([]);
  const [fixturesByVariant, setFixturesByVariant] = useState<Record<string, string>>({});
  const [prefillFfId, setPrefillFfId] = useState<string>("sec-header");
  const [staleToastMessage, setStaleToastMessage] = useState<string | null>(null);
  const { projectId, selectedNodeId } = params;
  const canPersist = Boolean(projectId && UUID_RE.test(projectId) && selectedNodeId);

  useInitialNodePreviewSync({ canPersist, projectId, selectedNodeId, setComments, setFixturesByVariant, setStaleToastMessage, onSelectSubState: params.onSelectSubState });
  const handleAddComment = useCreateCommentAction({ canPersist, projectId, selectedNodeId, artifactSha256: params.artifactSha256, setComments, setStaleToastMessage });
  const mutate = useMutateCommentActions({ canPersist, projectId, selectedNodeId, artifactSha256: params.artifactSha256, prefillFfId, setComments, setStaleToastMessage });

  const handleSelectFixtureVariant = useCallback(async (variant: FixtureVariant) => {
    const normalized = normalizeFixtureRef(variant);
    params.onSelectSubState?.(normalized);
    const payloadJson = fixturesByVariant[normalized] || JSON.stringify({ substate: normalized });
    params.postFixtureHydrate(normalized, payloadJson);
    if (canPersist && projectId && selectedNodeId) {
      await apiScreenFixture.saveFixture(projectId, selectedNodeId, { variant: normalized, payloadJson, isDefault: normalized === "DEFAULT" }).catch(() => undefined);
    }
  }, [canPersist, fixturesByVariant, params, projectId, selectedNodeId]);

  const handlePersistViewport = useCallback(async (vp: ViewportPreset) => {
    params.onSetViewportPreset(vp);
    if (canPersist && projectId && selectedNodeId) {
      await apiScreenFixture.setViewportPreset(projectId, selectedNodeId, vp).catch(() => undefined);
    }
  }, [canPersist, params, projectId, selectedNodeId]);

  const collapsedCommentIds = comments.filter((c) => { const r = params.elementRects[c.elementFfId]; return !r || r.width <= 0 || r.height <= 0; }).map((c) => c.id);
  return { comments, setComments, prefillFfId, setPrefillFfId, staleToastMessage, setStaleToastMessage, collapsedCommentIds, handleSelectFixtureVariant, handlePersistViewport, handleAddComment, ...mutate };
}
