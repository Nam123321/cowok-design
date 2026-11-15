// @story 04.2, 04.3 (Local CLI Doctor -> Proxy to Rust Daemon /api/v1/doctor/cli-matrix)
// @implements AC-01, AC-02 (Honest CLI Presence & Config File Sanity Check)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";

export interface CliDiagnosticItem {
  id: string;
  name: string;
  command: string;
  category?: "cli" | "ide" | "ide_extension" | "local";
  scanMode?: "passive" | "active";
  installed: boolean;
  version?: string;
  authenticated: boolean;
  status: "ready" | "not_installed" | "auth_needed" | "manual_ide_setup" | "timeout" | "error";
  authReason?: string;
  latencyMs: number;
}

export const GET = guardedRoute(
  (req, ctx) =>
    forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/doctor/cli-matrix",
      rawQuery: new URL(req.url, "http://127.0.0.1").search.slice(1),
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    }),
  { authMode: "strict-loopback" }
);
