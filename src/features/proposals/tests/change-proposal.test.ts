/** @vitest-environment jsdom */
// @story 05.2
import React from "react";
import fs from "node:fs";
import path from "node:path";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CreateProposalSchema,
  ChangeProposalResponseSchema,
  ComputeVisualDiffSchema,
  DecideTradeoffSchema,
  ProposalGateReceiptSchema,
  ProposalTradeoffSchema,
  VisualDiffResultSchema,
  toRustChangeProposalV1,
  ValidateProposalSchema,
  RejectProposalSchema,
  type ChangeProposalResponse,
  type ProposalTradeoff,
  type VisualDiffResult,
} from "../../../lib/validations/change-proposal";
import { ChangeProposalClient } from "../../../api/change-proposal";
import { matchBffRoute } from "../../../lib/vite-bff-routes";
import {
  computeAutoFlipPopoverPosition,
  computeUnifiedDiff,
  parsePatchLines,
  resolveIframeElementStageRects,
  runValidationChecks,
  shouldSuppressProposalHotkey,
} from "../utils/diff-engine";
import { ChangeProposalDrawer } from "../components/ChangeProposalDrawer";
import { CowokDesignDiffViewer } from "../components/CowokDesignDiffViewer";
import { DesignerTalkOverlay } from "../components/DesignerTalkOverlay";
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

    it("should validate ValidateProposalSchema and RejectProposalSchema strictly via Zod", () => {
      const defaultValidate = ValidateProposalSchema.parse({});
      expect(defaultValidate.dryRun).toBe(false);

      const dryRunValidate = ValidateProposalSchema.parse({ dryRun: true });
      expect(dryRunValidate.dryRun).toBe(true);

      const defaultReject = RejectProposalSchema.parse({});
      expect(defaultReject.reason).toBeUndefined();

      const reasonedReject = RejectProposalSchema.parse({ reason: "Architecture mismatch" });
      expect(reasonedReject.reason).toBe("Architecture mismatch");
    });

    it("should fail-closed on spaced eval and Function dynamic execution patterns", () => {
      const spacedEval = [
        computeUnifiedDiff("src/SpacedEval.tsx", "", "export const x = () => eval  ('alert(1)');"),
      ];
      const resEval = runValidationChecks(spacedEval);
      expect(resEval.validationStatus).toBe("failed");
      expect(resEval.checks.find((c) => c.checkType === "ast_lint")?.status).toBe("failed");

      const funcEval = [
        computeUnifiedDiff("src/FuncEval.tsx", "", "export const run = () => Function('return 1')();"),
      ];
      const resFunc = runValidationChecks(funcEval);
      expect(resFunc.validationStatus).toBe("failed");
      expect(resFunc.checks.find((c) => c.checkType === "ast_lint")?.status).toBe("failed");
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

  describe("AC-04..AC-06 & EC-PROPOSAL-002: Story 05.2 Schemas, Pure CSS UI, Visual Diff & Designer Talk", () => {
    afterEach(() => {
      cleanup();
    });

    it("validates VisualDiffResultSchema, ProposalTradeoffSchema, ProposalGateReceiptSchema, DecideTradeoffSchema, and rebaseBaseHashes", () => {
      const computed = VisualDiffResultSchema.parse({
        status: "COMPUTED",
        ssim: 0.964,
        mismatchRatio: 0.036,
        mismatchedPixels: 36,
        totalPixels: 1000,
        width: 100,
        height: 10,
        changedBbox: { x: 10, y: 2, width: 20, height: 6 },
      });
      expect(computed.changedBbox?.width).toBe(20);
      expect(computed.structuralDiffSummary).toEqual([]);

      const unavailable = VisualDiffResultSchema.parse({
        status: "RASTERIZER_UNAVAILABLE",
        ssim: 1,
        mismatchRatio: 0,
        mismatchedPixels: 0,
        totalPixels: 0,
        width: 0,
        height: 0,
        reason: "No RGBA framebuffer supplied",
        structuralDiffSummary: ["src/NavBar.tsx: +5/-2"],
      });
      expect(unavailable.status).toBe("RASTERIZER_UNAVAILABLE");

      const tradeoff = ProposalTradeoffSchema.parse({
        id: "tr-1",
        proposalId: "550e8400-e29b-41d4-a716-446655440000",
        nodeId: "scr-02",
        elementFfId: "ff-cta-btn",
        badgeIndex: 1,
        designNote: "Keep monochrome CTA contrast high",
        insteadLabel: "Use solid border outline button",
        insteadPatch: { ffId: "ff-cta-btn", border: "1px solid var(--cp-text)" },
        decision: "pending",
        createdAt: "2026-11-16T12:00:00Z",
      });
      expect(tradeoff.badgeIndex).toBe(1);
      expect(() =>
        ProposalTradeoffSchema.parse({ ...tradeoff, badgeIndex: 13 })
      ).toThrow();

      expect(() =>
        CreateProposalSchema.parse({
          title: "Too many tradeoffs",
          files: [{ path: "src/A.tsx", status: "added", additions: 1, deletions: 0, patch: "+const a = 1;" }],
          tradeoffs: Array.from({ length: 13 }, (_, idx) => ({
            elementFfId: `ff-${idx}`,
            badgeIndex: ((idx % 12) + 1),
            designNote: "Note",
            insteadLabel: "Instead",
            insteadPatch: "+patch",
          })),
        })
      ).toThrow();

      const receipt = ProposalGateReceiptSchema.parse({
        id: "rcpt-1",
        proposalId: "550e8400-e29b-41d4-a716-446655440000",
        diffSha256: "abc123",
        constraintsSha256: "def456",
        workspaceHeadSha256: "789ghi",
        exitCode: 0,
        receiptHmac: "deadbeefcafebabe",
        createdAt: 1763294400000,
      });
      expect(receipt.exitCode).toBe(0);

      expect(DecideTradeoffSchema.parse({ decision: "asked", askPrompt: "Why?" }).askPrompt).toBe("Why?");
      expect(ValidateProposalSchema.parse({}).rebaseBaseHashes).toBe(false);
      expect(ValidateProposalSchema.parse({ rebaseBaseHashes: true }).rebaseBaseHashes).toBe(true);
      expect(ComputeVisualDiffSchema.parse({ width: 320, height: 240 }).width).toBe(320);
    });

    it("mounts diff & tradeoff BFF routes and forwards ?nodeId= in ChangeProposalClient.getProposals", async () => {
      const diffRoute = matchBffRoute("POST", "/api/projects/proj-1/proposals/prop-1/diff");
      expect(diffRoute).not.toBeNull();
      expect(diffRoute?.params).toEqual({ id: "proj-1", proposalId: "prop-1" });

      const decideRoute = matchBffRoute("POST", "/api/projects/proj-1/proposals/prop-1/tradeoffs/tr-1/decide");
      expect(decideRoute).not.toBeNull();
      expect(decideRoute?.params).toEqual({ id: "proj-1", proposalId: "prop-1", tradeoffId: "tr-1" });

      const calls: string[] = [];
      const mockFetch = (async (input: RequestInfo | URL) => {
        calls.push(String(input));
        return new Response("[]", { status: 200, headers: { "Content-Type": "application/json" } });
      }) as typeof fetch;

      const client = new ChangeProposalClient({
        getSessionToken: () => "tok-123",
        fetchImpl: mockFetch,
      });
      await client.getProposals("proj-1", "scr-02");
      expect(calls[0]).toBe("/api/projects/proj-1/proposals?nodeId=scr-02");
    });

    it("suppresses Designer Talk hotkeys inside inputs, contenteditable, modals, and Inline </> Editor", () => {
      const input = document.createElement("input");
      const textarea = document.createElement("textarea");
      const select = document.createElement("select");
      const editable = document.createElement("div");
      editable.setAttribute("contenteditable", "true");
      const inlineEditor = document.createElement("div");
      inlineEditor.setAttribute("data-inline-code-editor", "true");
      const childInEditor = document.createElement("span");
      inlineEditor.appendChild(childInEditor);
      const stageBtn = document.createElement("button");

      expect(shouldSuppressProposalHotkey({ target: input })).toBe(true);
      expect(shouldSuppressProposalHotkey({ target: textarea })).toBe(true);
      expect(shouldSuppressProposalHotkey({ target: select })).toBe(true);
      expect(shouldSuppressProposalHotkey({ target: editable })).toBe(true);
      expect(shouldSuppressProposalHotkey({ target: childInEditor })).toBe(true);
      expect(shouldSuppressProposalHotkey({ target: stageBtn }, { isModalOpen: true })).toBe(true);
      expect(shouldSuppressProposalHotkey({ target: stageBtn }, { isInlineEditorOpen: true })).toBe(true);
      expect(shouldSuppressProposalHotkey({ target: stageBtn, isComposing: true })).toBe(true);
      expect(shouldSuppressProposalHotkey({ target: stageBtn, metaKey: true })).toBe(true);
      expect(shouldSuppressProposalHotkey({ target: stageBtn })).toBe(false);
    });

    it("auto-flips Designer Talk popover away from RightDrawer (420px) and maps iframe rects", () => {
      const mapped = resolveIframeElementStageRects(
        { "ff-cta": { left: 100, top: 50, width: 80, height: 32 } },
        { left: 20, top: 40 },
        1.5
      );
      expect(mapped["ff-cta"]).toEqual({ left: 170, top: 115, width: 120, height: 48 });

      // Near bottom & near 420px right drawer edge -> flips upward ("top") and clamps away from drawer
      const nearBottomAndDrawer = computeAutoFlipPopoverPosition(
        { left: 780, top: 520, width: 40, height: 32 },
        { width: 1280, height: 600, drawerWidth: 420 },
        { width: 288, height: 220 }
      );
      expect(nearBottomAndDrawer.placement).toBe("top");
      expect(nearBottomAndDrawer.left + 288).toBeLessThanOrEqual(1280 - 420);
    });

    it("enforces Zero Tailwind, Zero Amber (#FF9A2F/#e89a35), and Zero .msg-avatar across src/features/proposals/**", () => {
      const proposalsDir = path.resolve(process.cwd(), "src/features/proposals/components");
      const files = fs.readdirSync(proposalsDir).filter((f) => f.endsWith(".tsx") || f.endsWith(".ts"));
      expect(files.length).toBeGreaterThanOrEqual(5);

      const tailwindClassRe = /\b(?:flex|grid|items-center|justify-between|gap-\d+|p-\d+|px-\d+|py-\d+|pt-\d+|pb-\d+|bg-emerald-\d+|bg-rose-\d+|bg-muted|bg-card|bg-background|bg-primary|text-xs|text-sm|text-foreground|text-muted-foreground|text-emerald-\d+|text-rose-\d+|border-border|rounded-md|rounded-lg|shadow-2xl|w-full|max-w-lg|z-50)\b/;
      for (const file of files) {
        const src = fs.readFileSync(path.join(proposalsDir, file), "utf8");
        const classMatches = [...src.matchAll(/className\s*=\s*(?:"([^"]*)"|'([^']*)'|\{`([^`]*)`\})/g)];
        for (const m of classMatches) {
          const val = m[1] ?? m[2] ?? m[3] ?? "";
          expect(val, `Tailwind class found in ${file}: ${val}`).not.toMatch(tailwindClassRe);
        }
        expect(src.toLowerCase(), `Amber hex found in ${file}`).not.toContain("#ff9a2f");
        expect(src.toLowerCase(), `Amber hex found in ${file}`).not.toContain("#e89a35");
        expect(src, `.msg-avatar found in ${file}`).not.toContain("msg-avatar");
      }
    });

    it("renders ChangeProposalDrawer, CowokDesignDiffViewer, and DesignerTalkOverlay with confirmation gate & hotkeys", () => {
      const sampleDiff: VisualDiffResult = {
        status: "COMPUTED",
        ssim: 0.942,
        mismatchRatio: 0.058,
        mismatchedPixels: 58,
        totalPixels: 1000,
        width: 100,
        height: 100,
        changedBbox: { x: 10, y: 10, width: 30, height: 20 },
        structuralDiffSummary: [],
      };
      const sampleTradeoffs: ProposalTradeoff[] = [
        {
          id: "tr-1",
          proposalId: "550e8400-e29b-41d4-a716-446655440000",
          nodeId: "scr-02",
          elementFfId: "ff-submit",
          badgeIndex: 1,
          designNote: "Monochrome solid button",
          insteadLabel: "Outlined ghost button",
          insteadPatch: "+border: 1px solid",
          decision: "pending",
          createdAt: "2026-11-16T12:00:00Z",
        },
        {
          id: "tr-2",
          proposalId: "550e8400-e29b-41d4-a716-446655440000",
          nodeId: "scr-02",
          elementFfId: "ff-title",
          badgeIndex: 2,
          designNote: "Tighter heading tracking",
          insteadLabel: "Default tracking",
          insteadPatch: "+letter-spacing: 0",
          decision: "agreed",
          createdAt: "2026-11-16T12:00:00Z",
        },
      ];
      const sampleProposal: ChangeProposalResponse = {
        id: "550e8400-e29b-41d4-a716-446655440000",
        tenantId: "tenant-alpha",
        projectId: "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
        nodeId: "scr-02",
        title: "Refine Checkout Button",
        description: "Pure CSS tokens",
        files: [{ path: "src/Checkout.tsx", status: "modified", additions: 2, deletions: 1, patch: "-old\n+new" }],
        validationStatus: "passed",
        validationChecks: [{ checkType: "ast_lint", status: "passed", message: "ok", timestamp: "2026-11-16T12:00:00Z" }],
        gateReceiptId: "rcpt-1",
        gateReceipt: {
          id: "rcpt-1",
          proposalId: "550e8400-e29b-41d4-a716-446655440000",
          diffSha256: "aaa",
          constraintsSha256: "bbb",
          workspaceHeadSha256: "ccc",
          exitCode: 0,
          receiptHmac: "deadbeef1234",
          createdAt: "2026-11-16T12:00:00Z",
        },
        visualDiff: sampleDiff,
        tradeoffs: sampleTradeoffs,
        status: "ready",
        createdAt: "2026-11-16T12:00:00Z",
        updatedAt: "2026-11-16T12:00:00Z",
      };

      // Closed by default
      const closedRender = render(
        React.createElement(ChangeProposalDrawer, {
          proposal: sampleProposal,
          isOpen: false,
          onClose: vi.fn(),
          onApply: vi.fn(),
          onReject: vi.fn(),
        })
      );
      expect(screen.queryByTestId("change-proposal-drawer")).toBeNull();
      closedRender.unmount();

      // Open drawer + 409 rebase + Visual Diff tab + explicit Apply confirm modal
      const onApply = vi.fn();
      const onRebase = vi.fn();
      render(
        React.createElement(ChangeProposalDrawer, {
          proposal: sampleProposal,
          isOpen: true,
          onClose: vi.fn(),
          onApply,
          onReject: vi.fn(),
          onRebaseAndValidate: onRebase,
          errorBanner: { status: 409, code: "PROPOSAL_STALE_BASE_HASH", message: "Stale", staleFiles: ["src/styles/tokens.css"] },
        })
      );
      expect(screen.getByTestId("proposal-scope-pill").textContent).toContain("scr-02");
      expect(screen.getByTestId("gate-error-409")).toBeTruthy();
      fireEvent.click(screen.getByTestId("rebase-revalidate-btn"));
      expect(onRebase).toHaveBeenCalledWith(sampleProposal.id);
      expect(screen.getByTestId("proposal-apply-btn").hasAttribute("disabled")).toBe(true);

      fireEvent.click(screen.getByTestId("proposal-tab-visual"));
      expect(screen.getByTestId("visual-diff-changed-bbox")).toBeTruthy();

      cleanup();

      // Explicit confirmation modal before Apply when no error banner
      render(
        React.createElement(ChangeProposalDrawer, {
          proposal: sampleProposal,
          isOpen: true,
          onClose: vi.fn(),
          onApply,
          onReject: vi.fn(),
        })
      );
      fireEvent.click(screen.getByTestId("proposal-apply-btn"));
      expect(onApply).not.toHaveBeenCalled();
      expect(screen.getByTestId("proposal-apply-confirm-modal")).toBeTruthy();
      fireEvent.click(screen.getByTestId("proposal-confirm-apply-btn"));
      expect(onApply).toHaveBeenCalledWith(sampleProposal.id);

      cleanup();

      // Unavailable visual diff fallback
      render(
        React.createElement(CowokDesignDiffViewer, {
          visualDiff: {
            status: "RASTERIZER_UNAVAILABLE",
            ssim: 1,
            mismatchRatio: 0,
            mismatchedPixels: 0,
            totalPixels: 0,
            width: 0,
            height: 0,
            reason: "Headless rasterizer absent",
            structuralDiffSummary: ["src/Checkout.tsx: +2/-1"],
          },
        })
      );
      expect(screen.getByTestId("visual-diff-unavailable")).toBeTruthy();

      cleanup();

      // DesignerTalkOverlay badges, Ask input suppression, and Agree hotkey
      const onDecide = vi.fn();
      render(
        React.createElement(DesignerTalkOverlay, {
          isOpen: true,
          tradeoffs: sampleTradeoffs,
          elementRects: { "ff-submit": { left: 40, top: 60, width: 100, height: 32 } },
          onDecide,
        })
      );
      expect(screen.getByTestId("designer-talk-badge-1")).toBeTruthy();
      expect(screen.getByTestId("designer-talk-badge-2")).toBeTruthy();

      // Open Ask input -> typing 'a' inside input must NOT trigger Agree hotkey
      fireEvent.click(screen.getByTestId("dt-ask-btn"));
      const askInput = screen.getByTestId("dt-ask-input");
      fireEvent.keyDown(askInput, { key: "a" });
      expect(onDecide).not.toHaveBeenCalled();

      // Pressing 'a' on stage triggers Agree on active badge #1
      fireEvent.keyDown(window, { key: "a" });
      expect(onDecide).toHaveBeenCalledWith("tr-1", { decision: "agreed" });
    });
  });
});
