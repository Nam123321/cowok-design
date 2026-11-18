// @story 04.1, 04.2
// @implements AC-01, AC-02, AC-06
import type { IncomingMessage, ServerResponse } from "node:http";
import { issueBffLoopbackSession } from "./api-guard.ts";
import { LocalSessionBootstrapInputSchema } from "./validations/harness-daemon.ts";
import {
  HTTP_BAD_REQUEST,
  HTTP_FORBIDDEN,
  HTTP_OK,
  HTTP_PAYLOAD_TOO_LARGE,
  hasEditorBootstrapProof,
  isAuthorizedAdminBearer,
  isValidBootCapHeader,
  isValidWorkbenchCookie,
  readRawBody,
  singleHeader,
  timingSafeMatch,
  verifyBootstrapOrigin,
  writeJson,
} from "./vite-bff-proof.ts";

export {
  HTTP_FORBIDDEN,
  HTTP_OK,
  HTTP_PAYLOAD_TOO_LARGE,
  buildWorkbenchProofCookieHeader,
  injectBootCapMeta,
  issueBootCapToken,
  readRawBody,
  revokeBootCapToken,
  writeJson,
} from "./vite-bff-proof.ts";

interface BootstrapInput {
  tenantId?: string;
  userId?: string;
  role: "viewer" | "editor" | "admin";
}

async function parseBootstrapInput(
  req: IncomingMessage,
  res: ServerResponse
): Promise<BootstrapInput | null> {
  const rawBuf = await readRawBody(req);
  if (rawBuf === "TOO_LARGE") {
    writeJson(res, HTTP_PAYLOAD_TOO_LARGE, { code: "PAYLOAD_TOO_LARGE", error: "PAYLOAD_TOO_LARGE" });
    return null;
  }
  let rawJson: unknown = {};
  if (rawBuf.length > 0) {
    try {
      rawJson = JSON.parse(rawBuf.toString("utf8"));
    } catch {
      writeJson(res, HTTP_BAD_REQUEST, { code: "INVALID_JSON", error: "INVALID_JSON", message: "Malformed JSON request body" });
      return null;
    }
  }
  const parsed = LocalSessionBootstrapInputSchema.safeParse(rawJson);
  if (!parsed.success) {
    writeJson(res, HTTP_BAD_REQUEST, { code: "VALIDATION_ERROR", error: "VALIDATION_ERROR", details: parsed.error.issues });
    return null;
  }
  return parsed.data;
}

function authorizeBootstrapAccess(
  req: IncomingMessage,
  res: ServerResponse,
  data: BootstrapInput
): { effectiveRole: "viewer" | "editor" | "admin"; requestedTenant: string } | null {
  const headerRole = singleHeader(req.headers["x-cowok-role"]);
  const effectiveRole =
    headerRole === "viewer" || headerRole === "editor" || headerRole === "admin"
      ? headerRole
      : data.role;
  const isAdmin = isAuthorizedAdminBearer(req.headers.authorization);
  if (effectiveRole === "admin" && !isAdmin) {
    writeJson(res, HTTP_FORBIDDEN, { code: "INSUFFICIENT_ROLE", error: "INSUFFICIENT_ROLE", message: "Unauthenticated local-session bootstrap cannot issue admin role" });
    return null;
  }
  const requestedTenant = data.tenantId?.trim() || "local-default";
  if (requestedTenant !== "local-default" && !isAdmin) {
    writeJson(res, HTTP_FORBIDDEN, { code: "TENANT_ISOLATION_BREACH", error: "TENANT_ISOLATION_BREACH", message: "Unauthenticated local-session bootstrap cannot switch away from local-default tenant" });
    return null;
  }
  if (effectiveRole === "editor" && !hasEditorBootstrapProof(req)) {
    writeJson(res, HTTP_FORBIDDEN, { code: "INSUFFICIENT_ROLE", error: "INSUFFICIENT_ROLE", message: "Editor local-session bootstrap requires daemon nonce, boot capability token, or strict same-origin proof" });
    return null;
  }
  if (headerRole) res.setHeader("x-cowok-role", headerRole);
  return { effectiveRole, requestedTenant };
}

function checkHumanWorkbenchProof(req: IncomingMessage, effectiveRole: "viewer" | "editor" | "admin"): boolean {
  const hasDaemonNonceProof = req.headers["x-cowok-daemon-nonce"] !== undefined;
  if (hasDaemonNonceProof || isAuthorizedAdminBearer(req.headers.authorization) || effectiveRole === "viewer") {
    return false;
  }
  const suppliedCap = singleHeader(req.headers["x-cowok-boot-cap"]);
  const sfs = singleHeader(req.headers["sec-fetch-site"]);
  const qaNonceAllowed = process.env.NODE_ENV === "test" || process.env.VITEST === "true";
  const qaNonce = qaNonceAllowed ? process.env.COWOK_QA_NONCE?.trim() || "" : "";
  const suppliedQaNonce = singleHeader(req.headers["x-iwish-qa-nonce"]);
  const hasCookieOrQa =
    isValidWorkbenchCookie(req, suppliedCap) || (qaNonce.length > 0 && timingSafeMatch(suppliedQaNonce, qaNonce));
  return isValidBootCapHeader(suppliedCap) && sfs === "same-origin" && hasCookieOrQa;
}

/** Handles POST /api/auth/local-session with capability proof, tenant isolation, and role checks. */
export async function handleBootstrapSession(req: IncomingMessage, res: ServerResponse): Promise<void> {
  if (!verifyBootstrapOrigin(req)) {
    return writeJson(res, HTTP_FORBIDDEN, { code: "ORIGIN_REJECTED", error: "ORIGIN_REJECTED" });
  }
  const data = await parseBootstrapInput(req, res);
  if (!data) return;

  const authorized = authorizeBootstrapAccess(req, res, data);
  if (!authorized) return;

  const isHumanWorkbench = checkHumanWorkbenchProof(req, authorized.effectiveRole);
  return writeJson(
    res,
    HTTP_OK,
    issueBffLoopbackSession({
      tenantId: authorized.requestedTenant,
      userId: data.userId,
      role: authorized.effectiveRole,
      ...(isHumanWorkbench ? { isHumanWorkbench: true } : {}),
    })
  );
}
