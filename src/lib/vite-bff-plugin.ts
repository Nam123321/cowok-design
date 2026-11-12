// @story 04.1, 04.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import type { IncomingMessage, ServerResponse } from "node:http";
import type { Connect, Plugin } from "vite";
import { isAllowedLoopbackHost, issueBffLoopbackSession } from "./api-guard";
import { forwardToDaemonFlow } from "./daemon-client";
import * as snapshotRoute from "../app/api/projects/[id]/flow/snapshot/route";
import * as nodesRoute from "../app/api/projects/[id]/flow/nodes/route";
import * as positionsRoute from "../app/api/projects/[id]/flow/nodes/positions/route";
import * as singleNodeRoute from "../app/api/projects/[id]/flow/nodes/[nodeId]/route";
import * as transitionsRoute from "../app/api/projects/[id]/flow/transitions/route";
import * as singleTransitionRoute from "../app/api/projects/[id]/flow/transitions/[transitionId]/route";
import * as deployDraftRoute from "../app/api/projects/[id]/flow/deploy-draft/route";
import * as projectListRoute from "../app/api/projects/route";
import * as projectSessionsRoute from "../app/api/projects/[id]/sessions/route";
import * as singleSessionRoute from "../app/api/projects/[id]/sessions/[sessionId]/route";
import * as pathTokenRoute from "../app/api/projects/[id]/path-token/route";
import * as commentsRoute from "../app/api/projects/[id]/flow/nodes/[nodeId]/comments/route";
import * as fixturesRoute from "../app/api/projects/[id]/flow/nodes/[nodeId]/fixtures/route";
import * as doctorRoute from "../app/api/doctor/route";

const MAX_BODY_BYTES = 1_048_576;

function readRawBody(req: IncomingMessage): Promise<Buffer | "TOO_LARGE"> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let total = 0;
    let overflow = false;
    req.on("data", (chunk: Buffer) => {
      total += chunk.length;
      if (total > MAX_BODY_BYTES) overflow = true;
      else chunks.push(chunk);
    });
    req.on("end", () => resolve(overflow ? "TOO_LARGE" : Buffer.concat(chunks)));
    req.on("error", reject);
  });
}

function sendWebResponse(res: ServerResponse, webRes: Response): Promise<void> {
  res.statusCode = webRes.status;
  webRes.headers.forEach((value, key) => res.setHeader(key, value));
  return webRes.text().then((body) => {
    res.end(body);
  });
}

function verifyBootstrapOrigin(req: IncomingMessage): boolean {
  const host = String(req.headers.host || "127.0.0.1");
  if (!isAllowedLoopbackHost(host)) return false;
  const sfs = String(req.headers["sec-fetch-site"] || "");
  if (sfs && sfs !== "same-origin" && sfs !== "none") return false;
  const origin = req.headers.origin;
  if (!origin) return true;
  try {
    return isAllowedLoopbackHost(new URL(String(origin)).host);
  } catch {
    return false;
  }
}

type RouteFn = (req: Request, ctx: { params?: Record<string, string> }) => Promise<Response>;

function forwardDirect(daemonPath: string): RouteFn {
  return async (req: Request) => {
    const body = req.method !== "GET" && req.method !== "HEAD" ? await req.json().catch(() => undefined) : undefined;
    return forwardToDaemonFlow({
      method: req.method,
      daemonPath,
      body,
      inboundHeaders: req.headers,
      principal: {
        tenantId: req.headers.get("x-tenant-id") || "local-default",
        userId: req.headers.get("x-user-id") || "local-operator",
        role: "editor",
      },
    });
  };
}

function matchBffRoute(
  method: string,
  pathname: string
): { handler: RouteFn; params: Record<string, string> } | null {
  // Projects Root
  if (pathname === "/api/projects" && method === "GET") return { handler: projectListRoute.GET, params: {} };
  if (pathname === "/api/projects" && method === "POST") return { handler: projectListRoute.POST, params: {} };

  // CLI Doctor Probe
  if (pathname === "/api/doctor" && method === "GET") return { handler: doctorRoute.GET, params: {} };

  // Project Sessions
  const mSess = /^\/api\/projects\/([^/]+)\/sessions$/.exec(pathname);
  if (mSess && method === "GET") return { handler: projectSessionsRoute.GET, params: { id: mSess[1] } };
  if (mSess && method === "POST") return { handler: projectSessionsRoute.POST, params: { id: mSess[1] } };

  const mSingleSess = /^\/api\/projects\/([^/]+)\/sessions\/([^/]+)$/.exec(pathname);
  if (mSingleSess && method === "GET") return { handler: singleSessionRoute.GET, params: { id: mSingleSess[1], sessionId: mSingleSess[2] } };
  if (mSingleSess && method === "PUT") return { handler: singleSessionRoute.PUT, params: { id: mSingleSess[1], sessionId: mSingleSess[2] } };
  if (mSingleSess && method === "DELETE") return { handler: singleSessionRoute.DELETE, params: { id: mSingleSess[1], sessionId: mSingleSess[2] } };

  // Project Path Token
  const mPathToken = /^\/api\/projects\/([^/]+)\/path-token$/.exec(pathname);
  if (mPathToken && method === "POST") return { handler: pathTokenRoute.POST, params: { id: mPathToken[1] } };

  // Flow Graph Routes
  const mSnap = /^\/api\/projects\/([^/]+)\/flow\/snapshot$/.exec(pathname);
  if (mSnap && method === "GET") return { handler: snapshotRoute.GET, params: { id: mSnap[1] } };
  const mNodes = /^\/api\/projects\/([^/]+)\/flow\/nodes$/.exec(pathname);
  if (mNodes && method === "GET") return { handler: nodesRoute.GET, params: { id: mNodes[1] } };
  if (mNodes && method === "POST") return { handler: nodesRoute.POST, params: { id: mNodes[1] } };
  const mPos = /^\/api\/projects\/([^/]+)\/flow\/nodes\/positions$/.exec(pathname);
  if (mPos && method === "PATCH") return { handler: positionsRoute.PATCH, params: { id: mPos[1] } };
  const mNode = /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)$/.exec(pathname);
  if (mNode && method === "PATCH") return { handler: singleNodeRoute.PATCH, params: { id: mNode[1], nodeId: mNode[2] } };
  if (mNode && method === "DELETE") return { handler: singleNodeRoute.DELETE, params: { id: mNode[1], nodeId: mNode[2] } };
  const mTrans = /^\/api\/projects\/([^/]+)\/flow\/transitions$/.exec(pathname);
  if (mTrans && method === "GET") return { handler: transitionsRoute.GET, params: { id: mTrans[1] } };
  if (mTrans && method === "POST") return { handler: transitionsRoute.POST, params: { id: mTrans[1] } };
  const mSingleTrans = /^\/api\/projects\/([^/]+)\/flow\/transitions\/([^/]+)$/.exec(pathname);
  if (mSingleTrans && method === "PATCH") {
    return { handler: singleTransitionRoute.PATCH, params: { id: mSingleTrans[1], transitionId: mSingleTrans[2] } };
  }
  if (mSingleTrans && method === "DELETE") {
    return { handler: singleTransitionRoute.DELETE, params: { id: mSingleTrans[1], transitionId: mSingleTrans[2] } };
  }
  const mDeploy = /^\/api\/projects\/([^/]+)\/flow\/deploy-draft$/.exec(pathname);
  if (mDeploy && method === "POST") return { handler: deployDraftRoute.POST, params: { id: mDeploy[1] } };

  // Comments and Fixtures
  const mComments = /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/comments$/.exec(pathname);
  if (mComments && method === "GET") return { handler: commentsRoute.GET, params: { id: mComments[1], nodeId: mComments[2] } };
  if (mComments && method === "POST") return { handler: commentsRoute.POST, params: { id: mComments[1], nodeId: mComments[2] } };

  const mFixtures = /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/fixtures$/.exec(pathname);
  if (mFixtures && method === "GET") return { handler: fixturesRoute.GET, params: { id: mFixtures[1], nodeId: mFixtures[2] } };
  if (mFixtures && method === "POST") return { handler: fixturesRoute.POST, params: { id: mFixtures[1], nodeId: mFixtures[2] } };

  // Snapshots (Direct to Rust SQLite Daemon)
  const mSnapshots = /^\/api\/projects\/([^/]+)\/snapshots$/.exec(pathname);
  if (mSnapshots) return { handler: forwardDirect(`/api/v1/projects/${mSnapshots[1]}/snapshots`), params: { id: mSnapshots[1] } };

  const mRevertSnap = /^\/api\/projects\/([^/]+)\/snapshots\/([^/]+)\/revert$/.exec(pathname);
  if (mRevertSnap && method === "POST") return { handler: forwardDirect(`/api/v1/projects/${mRevertSnap[1]}/snapshots/${mRevertSnap[2]}/revert`), params: { id: mRevertSnap[1] } };

  const mExportSnap = /^\/api\/projects\/([^/]+)\/snapshots\/export$/.exec(pathname);
  if (mExportSnap && method === "POST") return { handler: forwardDirect(`/api/v1/projects/${mExportSnap[1]}/snapshots/export`), params: { id: mExportSnap[1] } };

  const mDownloadExport = /^\/api\/projects\/([^/]+)\/snapshots\/export\/download$/.exec(pathname);
  if (mDownloadExport && method === "GET") return { handler: forwardDirect(`/api/v1/projects/${mDownloadExport[1]}/snapshots/export/download`), params: { id: mDownloadExport[1] } };

  const mPrSnap = /^\/api\/projects\/([^/]+)\/snapshots\/pr$/.exec(pathname);
  if (mPrSnap && method === "POST") return { handler: forwardDirect(`/api/v1/projects/${mPrSnap[1]}/snapshots/pr`), params: { id: mPrSnap[1] } };

  // Proposals (Direct to Rust SQLite Daemon)
  const mProposals = /^\/api\/projects\/([^/]+)\/proposals$/.exec(pathname);
  if (mProposals) return { handler: forwardDirect(`/api/v1/projects/${mProposals[1]}/proposals`), params: { id: mProposals[1] } };

  const mSingleProposal = /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)$/.exec(pathname);
  if (mSingleProposal && method === "GET") return { handler: forwardDirect(`/api/v1/projects/${mSingleProposal[1]}/proposals/${mSingleProposal[2]}`), params: { id: mSingleProposal[1] } };

  const mValidateProposal = /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)\/validate$/.exec(pathname);
  if (mValidateProposal && method === "POST") return { handler: forwardDirect(`/api/v1/projects/${mValidateProposal[1]}/proposals/${mValidateProposal[2]}/validate`), params: { id: mValidateProposal[1] } };

  const mApplyProposal = /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)\/apply$/.exec(pathname);
  if (mApplyProposal && method === "POST") return { handler: forwardDirect(`/api/v1/projects/${mApplyProposal[1]}/proposals/${mApplyProposal[2]}/apply`), params: { id: mApplyProposal[1] } };

  const mRejectProposal = /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)\/reject$/.exec(pathname);
  if (mRejectProposal && method === "POST") return { handler: forwardDirect(`/api/v1/projects/${mRejectProposal[1]}/proposals/${mRejectProposal[2]}/reject`), params: { id: mRejectProposal[1] } };

  // SDI (Direct to Rust SQLite Daemon)
  const mSdiIntent = /^\/api\/projects\/([^/]+)\/sdi\/intent$/.exec(pathname);
  if (mSdiIntent && method === "POST") return { handler: forwardDirect(`/api/v1/projects/${mSdiIntent[1]}/sdi/intent`), params: { id: mSdiIntent[1] } };

  const mSdiClarify = /^\/api\/projects\/([^/]+)\/sdi\/clarification$/.exec(pathname);
  if (mSdiClarify && method === "POST") return { handler: forwardDirect(`/api/v1/projects/${mSdiClarify[1]}/sdi/clarification`), params: { id: mSdiClarify[1] } };

  return null;
}

async function handleBootstrapSession(req: IncomingMessage, res: ServerResponse): Promise<void> {
  if (!verifyBootstrapOrigin(req)) {
    res.statusCode = 403;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ code: "ORIGIN_REJECTED", error: "ORIGIN_REJECTED" }));
    return;
  }
  const rawBuf = await readRawBody(req);
  if (rawBuf === "TOO_LARGE") {
    res.statusCode = 413;
    res.end(JSON.stringify({ code: "PAYLOAD_TOO_LARGE" }));
    return;
  }
  const bodyJson = rawBuf.length > 0 ? (JSON.parse(rawBuf.toString("utf8")) as Record<string, string>) : {};
  const issued = issueBffLoopbackSession({
    tenantId: bodyJson.tenantId,
    userId: bodyJson.userId,
    role: (bodyJson.role as "viewer" | "editor" | "admin") || "editor",
  });
  res.statusCode = 200;
  res.setHeader("Content-Type", "application/json");
  res.end(JSON.stringify(issued));
}

async function dispatchMiddlewareRequest(
  req: IncomingMessage,
  res: ServerResponse,
  next: Connect.NextFunction
): Promise<void> {
  const host = String(req.headers.host || "127.0.0.1");
  const parsedUrl = new URL(req.url || "/", `http://${host}`);
  const method = (req.method || "GET").toUpperCase();
  if (parsedUrl.pathname === "/api/auth/local-session" && method === "POST") {
    await handleBootstrapSession(req, res);
    return;
  }
  const matched = matchBffRoute(method, parsedUrl.pathname);
  if (!matched) {
    next();
    return;
  }
  const rawBuf = await readRawBody(req);
  if (rawBuf === "TOO_LARGE") {
    res.statusCode = 413;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ code: "PAYLOAD_TOO_LARGE", error: "PAYLOAD_TOO_LARGE" }));
    return;
  }
  const headers = new Headers();
  for (const [k, v] of Object.entries(req.headers)) {
    if (Array.isArray(v)) headers.set(k, v.join(", "));
    else if (typeof v === "string") headers.set(k, v);
  }
  const webReq = new Request(parsedUrl.toString(), {
    method,
    headers,
    body: method === "GET" || method === "HEAD" || rawBuf.length === 0 ? undefined : new Uint8Array(rawBuf),
  });
  await sendWebResponse(res, await matched.handler(webReq, { params: matched.params }));
}

export function cowokLoopbackBffPlugin(): Plugin {
  return {
    name: "cowok-loopback-bff",
    configureServer(server) {
      server.middlewares.use((req, res, next) => void dispatchMiddlewareRequest(req, res, next));
    },
    configurePreviewServer(server) {
      server.middlewares.use((req, res, next) => void dispatchMiddlewareRequest(req, res, next));
    },
  };
}
