// @story 04.2 (Local CLI Doctor -> Proxy to Rust Daemon /api/v1/doctor/cli-matrix)
// @implements AC-02 (Honest CLI Presence & Config File Sanity Check)
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";

export interface CliDiagnosticItem {
  id: string;
  name: string;
  command: string;
  installed: boolean;
  version?: string;
  authenticated: boolean;
  status: "ready" | "not_installed" | "auth_needed" | "error";
  authReason?: string;
  latencyMs: number;
}

export const GET = guardedRoute(
  (req, ctx) => forwardFromGuardedContext(req, ctx, "/api/v1/doctor/cli-matrix"),
  { authMode: "strict-loopback" }
);
