// @story 05.2
import { apiChangeProposal } from "../../../api/change-proposal";
import type { CreateProposalInput } from "../../../lib/validations/change-proposal";

type ProposalChangeListener = (projectId: string) => void;
const listeners = new Set<ProposalChangeListener>();

/** Notifies mounted proposal controllers that a project's ChangeProposals changed. */
export function notifyProposalsChanged(projectId: string): void {
  for (const fn of listeners) {
    fn(projectId);
  }
}

/** Subscribes to ChangeProposal mutation notifications; returns unsubscribe handler. */
export function subscribeProposalsChanged(fn: ProposalChangeListener): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

const MAX_TITLE_LEN = 72;

function resolveProposalNodeIds(nodeId?: string | null, nodeIds?: string[]): string[] | undefined {
  if (nodeIds && nodeIds.length > 0) return nodeIds;
  if (nodeId) return [nodeId];
  return undefined;
}

export function buildUiChangeProposalInput(opts: {
  prompt: string;
  targetPath?: string;
  nodeId?: string | null;
  nodeIds?: string[];
  screenKey?: string | null;
  sourceCommentId?: string;
  updatedHtml?: string;
  previousHtml?: string;
}): CreateProposalInput {
  const scopeLabel = opts.screenKey?.trim() || opts.nodeId?.trim() || "Batch";
  const cleanPrompt = opts.prompt.trim().replace(/\s+/g, " ");
  const titleSuffix = cleanPrompt.length > MAX_TITLE_LEN ? `${cleanPrompt.slice(0, MAX_TITLE_LEN)}...` : cleanPrompt;
  const targetPath = opts.targetPath || "src/pages/checkout-app.html";
  const nextHtml =
    opts.updatedHtml?.trim() ||
    [
      `<section data-ff-id="proposal-preview-card" class="cowok-card">`,
      `  <h2 data-ff-id="proposal-heading">${scopeLabel} — ${titleSuffix}</h2>`,
      `  <button type="button" data-ff-id="primary-cta-btn" class="cowok-btn cowok-btn-primary">${titleSuffix}</button>`,
      `</section>`,
    ].join("\n");

  return {
    title: `${scopeLabel}: ${titleSuffix}`,
    description: `Synthesized UI proposal for ${scopeLabel}: ${cleanPrompt}`,
    sourceCommentId: opts.sourceCommentId || undefined,
    nodeId: opts.nodeId || undefined,
    nodeIds: resolveProposalNodeIds(opts.nodeId, opts.nodeIds),
    contentMode: "full_file",
    files: [
      {
        path: targetPath,
        status: "modified",
        additions: nextHtml.split("\n").length,
        deletions: opts.previousHtml ? 1 : 0,
        patch: nextHtml,
      },
    ],
    tradeoffs: [
      {
        nodeId: opts.nodeId || undefined,
        elementFfId: "primary-cta-btn",
        badgeIndex: 1,
        designNote: `Trade-off (${scopeLabel}): Apply "${titleSuffix}" using high-contrast primary action hierarchy while keeping secondary metadata calm.`,
        insteadLabel: "Use compact ghost CTA with inline helper badge",
        insteadPatch: `<button type="button" data-ff-id="primary-cta-btn" class="cowok-btn cowok-btn-ghost">${titleSuffix}</button>`,
      },
    ],
  };
}

/** Creates and broadcasts a ChangeProposal when inline HTML is applied in the editor. */
export function submitInlineHtmlProposal(opts: {
  projectId?: string;
  tabTitle?: string;
  nodeId?: string | null;
  screenKey?: string | null;
  updatedHtml: string;
  previousHtml: string;
}): void {
  if (!opts.projectId || opts.updatedHtml === opts.previousHtml) return;
  const pid = opts.projectId;
  const tabTitle = opts.tabTitle || "checkout-app.html";
  void apiChangeProposal
    .createProposal(
      pid,
      buildUiChangeProposalInput({
        prompt: `Inline HTML edit (${tabTitle})`,
        targetPath: `src/pages/${tabTitle}`,
        nodeId: opts.nodeId,
        screenKey: opts.screenKey || null,
        updatedHtml: opts.updatedHtml,
        previousHtml: opts.previousHtml,
      })
    )
    .then(() => notifyProposalsChanged(pid))
    .catch(() => {
      /* ignore offline inline apply */
    });
}
