// @stateless-cache-bounded: ephemeral in-memory development store
import { guardedRoute } from "@/lib/api-guard";
import { collectProjectFiles } from "../route";
import {
  GitHubPRInput,
  GitHubPRSchema,
} from "../../../../../../lib/validations/version-snapshot";
import { buildGitHubPRPayload } from "../../../../../../features/snapshots/utils/export-bundle";
import { syncGitHubPRWithDaemon } from "../../../../../../api/version-snapshot";

const HTTP_STATUS_BAD_GATEWAY = 502;

export const POST = guardedRoute({ body: GitHubPRSchema }, async (_req, ctx, { body }) => {
  const projectId = ctx.projectId || ctx.params?.id || "default-project";
  const tenantId = ctx.tenantId;

  if (!body) {
    return new Response(JSON.stringify({ error: "INVALID_PR_PAYLOAD" }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    });
  }

  const files = collectProjectFiles(projectId, tenantId);
  let prResult = buildGitHubPRPayload(projectId, body, files.length);

  try {
    const remotePR = await syncGitHubPRWithDaemon(projectId, body, tenantId);
    if (remotePR) {
      prResult = remotePR;
    }
  } catch (err: unknown) {
    if (process.env.NODE_ENV !== "test") {
      const msg = err instanceof Error ? err.message : "DAEMON_PR_SYNC_FAILED";
      return new Response(JSON.stringify({ error: "DAEMON_PR_SYNC_FAILED", message: msg }), {
        status: HTTP_STATUS_BAD_GATEWAY,
        headers: { "Content-Type": "application/json" },
      });
    }
  }

  return new Response(JSON.stringify(prResult), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
