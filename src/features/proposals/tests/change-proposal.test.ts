import { describe, expect, it } from "vitest";
import {
  CreateProposalSchema,
  ChangeProposalResponseSchema,
  toRustChangeProposalV1,
} from "../../../lib/validations/change-proposal";
import {
  computeUnifiedDiff,
  parsePatchLines,
  runValidationChecks,
} from "../utils/diff-engine";
import { issueBffLoopbackSession } from "../../../lib/api-guard";
import { buildCanonicalHmacHeaders, ensureRunningDaemon } from "../../../lib/daemon-client";
import {
  POST as createProposalRoute,
  GET as listProposalsRoute,
} from "../../../app/api/projects/[id]/proposals/route";
import { POST as validateProposalRoute } from "../../../app/api/projects/[id]/proposals/[proposalId]/validate/route";
import { GET as getProposalRoute } from "../../../app/api/projects/[id]/proposals/[proposalId]/route";
import { POST as applyProposalRoute } from "../../../app/api/projects/[id]/proposals/[proposalId]/apply/route";
import { POST as rejectProposalRoute } from "../../../app/api/projects/[id]/proposals/[proposalId]/reject/route";

const HTTP_OK = 200;
const HTTP_CREATED = 201;
const HTTP_BAD_REQUEST = 400;
const HTTP_FORBIDDEN = 403;
const HTTP_NOT_FOUND = 404;

async function seedProposalTestProject(tenantId: string): Promise<{
  projectId: string;
  sessionToken: string;
}> {
  const lock = await ensureRunningDaemon();
  const projectId = crypto.randomUUID();
  const rawBody = JSON.stringify({
    id: projectId,
    slug: `prop-${projectId.slice(0, 8)}`,
    name: "Proposal Integration Project",
    product_brief: "SQLite WAL test project",
    workspace_root: null,
    inherit_brand_from_project_id: null,
  });
  const headers = buildCanonicalHmacHeaders({
    daemonNonce: lock.daemon_nonce,
    hmacKeyHex: lock.hmac_key_hex,
    method: "POST",
    canonicalPath: "/api/v1/projects",
    rawQuery: "",
    principal: { tenantId, userId: "proposal-tester", role: "editor" },
    rawBody,
    correlationId: crypto.randomUUID(),
  });
  const seedRes = await fetch(`http://${lock.host}:${lock.port}/api/v1/projects`, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...headers },
    body: rawBody,
  });
  if (!seedRes.ok) {
    throw new Error(`Project seed failed: ${seedRes.status} ${await seedRes.text()}`);
  }
  const { sessionToken } = issueBffLoopbackSession({ tenantId, role: "editor" });
  return { projectId, sessionToken };
}

function bffHeaders(sessionToken: string, hasBody: boolean = false): Record<string, string> {
  return {
    Host: "127.0.0.1:5173",
    Origin: "http://127.0.0.1:5173",
    "X-Cowok-Session-Token": sessionToken,
    ...(hasBody ? { "Content-Type": "application/json" } : {}),
  };
}

describe("Story 05.2: Sandboxed ChangeProposal Diff, Validation Gate & One-Click Apply / Reject", () => {
  describe("AC-01: Sandboxed Diff & Validation Engine", () => {
    it("should compute accurate additions, deletions, and unified patch", () => {
      const orig = "const a = 1;\nconst b = 2;\n";
      const updated = "const a = 1;\nconst b = 3;\nconst c = 4;\n";

      const diff = computeUnifiedDiff("src/index.ts", orig, updated);
      expect(diff.path).toBe("src/index.ts");
      expect(diff.status).toBe("modified");
      expect(diff.additions).toBe(2);
      expect(diff.deletions).toBe(1);
      expect(diff.patch).toContain("-const b = 2;");
      expect(diff.patch).toContain("+const b = 3;");
      expect(diff.patch).toContain("+const c = 4;");
    });

    it("should parse patch into structured added, deleted, and context lines", () => {
      const patch = " const a = 1;\n-const b = 2;\n+const b = 3;";
      const lines = parsePatchLines(patch);

      expect(lines).toHaveLength(3);
      expect(lines[0]).toEqual({ type: "context", content: "const a = 1;" });
      expect(lines[1]).toEqual({ type: "deleted", content: "const b = 2;" });
      expect(lines[2]).toEqual({ type: "added", content: "const b = 3;" });
    });

    it("should pass validation checks for clean files and fail on forbidden AST patterns", () => {
      const cleanFiles = [
        computeUnifiedDiff("src/Button.tsx", "", "export const Button = () => <button>OK</button>;"),
      ];
      const cleanResult = runValidationChecks(cleanFiles);
      expect(cleanResult.validationStatus).toBe("passed");
      expect(cleanResult.checks.every((c) => c.status === "passed")).toBe(true);

      const dirtyFiles = [
        computeUnifiedDiff("src/Bad.tsx", "", "export const Bad = () => eval('alert(1)');"),
      ];
      const dirtyResult = runValidationChecks(dirtyFiles);
      expect(dirtyResult.validationStatus).toBe("failed");
      const astCheck = dirtyResult.checks.find((c) => c.checkType === "ast_lint");
      expect(astCheck?.status).toBe("failed");

      const typeMismatchFiles = [
        computeUnifiedDiff("src/TypeMismatch.ts", "", "const value: string = getNumericValue();"),
      ];
      const typeResult = runValidationChecks(typeMismatchFiles);
      expect(typeResult.validationStatus).toBe("failed");
      const typeCheck = typeResult.checks.find((c) => c.checkType === "typecheck");
      expect(typeCheck?.status).toBe("failed");

      const missingImportFiles = [
        {
          path: "src/MissingImport.ts",
          status: "added" as const,
          additions: 2,
          deletions: 0,
          patch: "+import { missing } from './does-not-exist';\n+export const x = missing;",
        },
      ];
      const missingImportResult = runValidationChecks(missingImportFiles);
      expect(missingImportResult.validationStatus).toBe("failed");
      const buildCheck = missingImportResult.checks.find((c) => c.checkType === "build");
      expect(buildCheck?.status).toBe("failed");

      const unknownVarFiles = [
        {
          path: "src/UnknownVar.ts",
          status: "added" as const,
          additions: 1,
          deletions: 0,
          patch: "+const value: string = unknownValue;",
        },
      ];
      const unknownVarResult = runValidationChecks(unknownVarFiles);
      expect(unknownVarResult.validationStatus).toBe("failed");
      const unknownCheck = unknownVarResult.checks.find((c) => c.checkType === "typecheck");
      expect(unknownCheck?.status).toBe("failed");

      const syntaxErrFiles = [
        {
          path: "src/SyntaxErr.ts",
          status: "added" as const,
          additions: 1,
          deletions: 0,
          patch: "+const = ;",
        },
      ];
      const syntaxResult = runValidationChecks(syntaxErrFiles);
      expect(syntaxResult.validationStatus).toBe("failed");
    });

    it("should transform ChangeProposal to Rust domain contract ChangeProposalV1", () => {
      const tsProposal = {
        id: "550e8400-e29b-41d4-a716-446655440000",
        tenantId: "tenant-alpha",
        projectId: "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
        title: "Test Proposal",
        description: "Testing Rust contract mapping",
        files: [],
        validationStatus: "passed" as const,
        validationChecks: [],
        status: "ready" as const,
        createdAt: "2026-11-07T00:00:00Z",
        updatedAt: "2026-11-07T00:00:00Z",
      };
      const rustDto = toRustChangeProposalV1(tsProposal);
      expect(rustDto.id).toBe(tsProposal.id);
      expect(rustDto.tenant_id).toBe(tsProposal.tenantId);
      expect(rustDto.project_id).toBe(tsProposal.projectId);
      expect(rustDto.validation_status).toBe("passed");
      expect(rustDto.status).toBe("ready");
    });

    it("should validate CreateProposalSchema strictly via Zod", () => {
      const valid = CreateProposalSchema.parse({
        title: "Tối ưu hóa UI Cart",
        description: "Chỉnh màu sắc nút checkout",
        files: [
          {
            path: "src/Cart.tsx",
            status: "modified",
            additions: 1,
            deletions: 0,
            patch: "+<CheckoutButton />",
          },
        ],
      });
      expect(valid.title).toBe("Tối ưu hóa UI Cart");
      expect(() => CreateProposalSchema.parse({ title: "", files: [] })).toThrow();
    });
  });

  describe("AC-02: Fastify REST API Routes & Tenant Isolation", () => {
    it("should create proposal with initial validationStatus and checks via POST /proposals", async () => {
      const { projectId, sessionToken } = await seedProposalTestProject("tenant-alpha");
      const req = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          title: "Refactor Navigation Bar",
          description: "Chuẩn hóa theo DESIGN.md [5.25]",
          files: [
            {
              path: "src/NavBar.tsx",
              status: "modified",
              additions: 5,
              deletions: 2,
              patch: "+<RightDrawer />",
            },
          ],
        }),
      });

      const res = await createProposalRoute(req, { params: { id: projectId } });
      expect(res.status).toBe(HTTP_CREATED);
      const data = await res.json();
      expect(data.id).toBeDefined();
      expect(data.tenantId).toBe("tenant-alpha");
      expect(data.validationStatus).toBe("passed");
      expect(data.status).toBe("ready");
    });

    it("should fetch single proposal via GET /proposals/:proposalId", async () => {
      const { projectId, sessionToken } = await seedProposalTestProject("tenant-alpha");
      const createReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          title: "Proposal for Single Fetch",
          files: [{ path: "src/Single.ts", status: "added", additions: 1, deletions: 0, patch: "+const s = 1;" }],
        }),
      });
      const createRes = await createProposalRoute(createReq, { params: { id: projectId } });
      const proposal = await createRes.json();

      const getReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals/${proposal.id}`, {
        method: "GET",
        headers: bffHeaders(sessionToken),
      });
      const getRes = await getProposalRoute(getReq, { params: { id: projectId, proposalId: proposal.id } });
      expect(getRes.status).toBe(HTTP_OK);
      const fetched = await getRes.json();
      expect(fetched.id).toBe(proposal.id);
      expect(fetched.title).toBe("Proposal for Single Fetch");
    });

    it("should enforce tenant isolation in GET /proposals", async () => {
      const { projectId, sessionToken } = await seedProposalTestProject("tenant-alpha");
      await createProposalRoute(
        new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
          method: "POST",
          headers: bffHeaders(sessionToken, true),
          body: JSON.stringify({
            title: "Tenant Alpha Proposal",
            files: [{ path: "src/T.ts", status: "added", additions: 1, deletions: 0, patch: "+const t = 1;" }],
          }),
        }),
        { params: { id: projectId } }
      );

      const { sessionToken: strangerToken } = issueBffLoopbackSession({
        tenantId: "tenant-stranger",
        role: "editor",
      });
      const reqOther = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "GET",
        headers: bffHeaders(strangerToken),
      });
      const resOther = await listProposalsRoute(reqOther, { params: { id: projectId } });
      expect(resOther.status).toBe(HTTP_FORBIDDEN);

      const reqSame = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "GET",
        headers: bffHeaders(sessionToken),
      });
      const resSame = await listProposalsRoute(reqSame, { params: { id: projectId } });
      expect(resSame.status).toBe(HTTP_OK);
      const dataSame = await resSame.json();
      expect(dataSame.length).toBeGreaterThan(0);
    });

    it("should prevent cross-project IDOR when proposal belongs to different project", async () => {
      const { projectId: projectA, sessionToken } = await seedProposalTestProject("tenant-alpha");
      const { projectId: projectB } = await seedProposalTestProject("tenant-alpha");

      const createReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectA}/proposals`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          title: "Proposal in Project A",
          files: [{ path: "src/A.ts", status: "added", additions: 1, deletions: 0, patch: "+const a = 1;" }],
        }),
      });
      const createRes = await createProposalRoute(createReq, { params: { id: projectA } });
      const proposal = await createRes.json();

      const applyReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectB}/proposals/${proposal.id}/apply`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({ force: false }),
      });
      const applyRes = await applyProposalRoute(applyReq, { params: { id: projectB, proposalId: proposal.id } });
      expect(applyRes.status).toBe(HTTP_NOT_FOUND);
      const err = await applyRes.json();
      expect(err.error).toBe("PROPOSAL_NOT_FOUND");
    });

    it("should re-validate proposal via POST /proposals/:proposalId/validate", async () => {
      const { projectId, sessionToken } = await seedProposalTestProject("tenant-alpha");
      const createReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          title: "Test Validation Run",
          files: [{ path: "src/A.ts", status: "added", additions: 1, deletions: 0, patch: "+const ok = 1;" }],
        }),
      });
      const createRes = await createProposalRoute(createReq, { params: { id: projectId } });
      const proposal = await createRes.json();

      const valReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals/${proposal.id}/validate`, {
        method: "POST",
        headers: bffHeaders(sessionToken),
      });
      const valRes = await validateProposalRoute(valReq, { params: { id: projectId, proposalId: proposal.id } });
      expect(valRes.status).toBe(HTTP_OK);
      const valData = await valRes.json();
      expect(valData.validationStatus).toBe("passed");
    });

    it("should strictly reject validate on rejected proposal with HTTP 400 INVALID_STATE", async () => {
      const { projectId, sessionToken } = await seedProposalTestProject("tenant-alpha");
      const createReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          title: "Proposal To Reject Then Validate",
          files: [{ path: "src/A.ts", status: "added", additions: 1, deletions: 0, patch: "+const ok = 1;" }],
        }),
      });
      const createRes = await createProposalRoute(createReq, { params: { id: projectId } });
      const proposal = await createRes.json();

      const rejReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals/${proposal.id}/reject`, {
        method: "POST",
        headers: bffHeaders(sessionToken),
      });
      await rejectProposalRoute(rejReq, { params: { id: projectId, proposalId: proposal.id } });

      const valReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals/${proposal.id}/validate`, {
        method: "POST",
        headers: bffHeaders(sessionToken),
      });
      const valRes = await validateProposalRoute(valReq, { params: { id: projectId, proposalId: proposal.id } });
      expect(valRes.status).toBe(HTTP_BAD_REQUEST);
      const valErr = await valRes.json();
      expect(valErr.error).toBe("INVALID_STATE");
    });

    it("should mark proposal as rejected via POST /proposals/:proposalId/reject", async () => {
      const { projectId, sessionToken } = await seedProposalTestProject("tenant-alpha");
      const createReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          title: "Proposal to Reject",
          files: [{ path: "src/B.ts", status: "added", additions: 1, deletions: 0, patch: "+const b = 1;" }],
        }),
      });
      const createRes = await createProposalRoute(createReq, { params: { id: projectId } });
      const proposal = await createRes.json();

      const rejReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals/${proposal.id}/reject`, {
        method: "POST",
        headers: bffHeaders(sessionToken),
      });
      const rejRes = await rejectProposalRoute(rejReq, { params: { id: projectId, proposalId: proposal.id } });
      expect(rejRes.status).toBe(HTTP_OK);
      const rejData = await rejRes.json();
      expect(rejData.status).toBe("rejected");
    });
  });

  describe("AC-03: [EDGE-CASE] Resilience & Boundary Guard for Apply Action", () => {
    it("should strictly reject apply with HTTP 400 when validation checks fail", async () => {
      const { projectId, sessionToken } = await seedProposalTestProject("tenant-alpha");
      const createReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          title: "Malicious Dangerous Proposal",
          files: [
            {
              path: "src/Evil.tsx",
              status: "modified",
              additions: 1,
              deletions: 0,
              patch: "+dangerouslySetInnerHTML({ __html: 'xss' })",
            },
          ],
        }),
      });

      const createRes = await createProposalRoute(createReq, { params: { id: projectId } });
      const proposal = await createRes.json();
      expect(proposal.validationStatus).toBe("failed");

      const applyReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals/${proposal.id}/apply`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({ force: false }),
      });

      const applyRes = await applyProposalRoute(applyReq, { params: { id: projectId, proposalId: proposal.id } });
      expect(applyRes.status).toBe(HTTP_BAD_REQUEST);
      const err = await applyRes.json();
      expect(err.error).toBe("PRECONDITION_FAILED");
    });

    it("should strictly reject apply with HTTP 400 when proposal is not in ready status (e.g. rejected)", async () => {
      const { projectId, sessionToken } = await seedProposalTestProject("tenant-alpha");
      const createReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          title: "Proposal To Reject Then Apply",
          files: [{ path: "src/C.ts", status: "added", additions: 1, deletions: 0, patch: "+const c = 1;" }],
        }),
      });
      const createRes = await createProposalRoute(createReq, { params: { id: projectId } });
      const proposal = await createRes.json();
      expect(proposal.status).toBe("ready");

      const rejReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals/${proposal.id}/reject`, {
        method: "POST",
        headers: bffHeaders(sessionToken),
      });
      await rejectProposalRoute(rejReq, { params: { id: projectId, proposalId: proposal.id } });

      const applyReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals/${proposal.id}/apply`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({ force: false }),
      });
      const applyRes = await applyProposalRoute(applyReq, { params: { id: projectId, proposalId: proposal.id } });
      expect(applyRes.status).toBe(HTTP_BAD_REQUEST);
      const err = await applyRes.json();
      expect(err.error).toBe("INVALID_STATE");
    });

    it("should successfully apply proposal when all checks pass", async () => {
      const { projectId, sessionToken } = await seedProposalTestProject("tenant-alpha");
      const createReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          title: "Safe Valid Proposal",
          files: [
            {
              path: "src/Clean.tsx",
              status: "added",
              additions: 3,
              deletions: 0,
              patch: "export const Clean = () => <div>Safe</div>;",
            },
          ],
        }),
      });

      const createRes = await createProposalRoute(createReq, { params: { id: projectId } });
      const proposal = await createRes.json();
      expect(proposal.validationStatus).toBe("passed");

      const applyReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/proposals/${proposal.id}/apply`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({ force: false }),
      });

      const applyRes = await applyProposalRoute(applyReq, { params: { id: projectId, proposalId: proposal.id } });
      expect(applyRes.status).toBe(HTTP_OK);
      const applied = await applyRes.json();
      expect(applied.status).toBe("applied");
      expect(applied.appliedManifest).toBeUndefined();
    });
  });
});
