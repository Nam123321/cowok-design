import { describe, expect, it } from "vitest";
import {
  CreateSnapshotSchema,
  ExportZipSchema,
  SnapshotFileItem,
  VersionSnapshotDtoSchema,
} from "../../../lib/validations/version-snapshot";
import {
  computeSha256Hex,
  computeTreeSha256,
  createSnapshotFromInput,
  performTwoPhaseRevert,
  setLiveWorkspaceFile,
  clearLiveWorkspace,
  collectWorkspaceFiles,
} from "../utils/snapshot-engine";
import {
  buildExportZipMetadata,
  buildGitHubPRPayload,
  checkExportCompleteness,
} from "../utils/export-bundle";
import { issueBffLoopbackSession } from "../../../lib/api-guard";
import { buildCanonicalHmacHeaders, ensureRunningDaemon } from "../../../lib/daemon-client";
import {
  GET as getSnapshotsRoute,
  POST as createSnapshotRoute,
} from "../../../app/api/projects/[id]/snapshots/route";
import { POST as revertRoute } from "../../../app/api/projects/[id]/snapshots/[snapshotId]/revert/route";
import { POST as exportRoute } from "../../../app/api/projects/[id]/snapshots/export/route";
import { GET as downloadExportRoute } from "../../../app/api/projects/[id]/snapshots/export/download/route";
import { POST as createPRRoute } from "../../../app/api/projects/[id]/snapshots/pr/route";

const sampleFiles: SnapshotFileItem[] = [
  { path: "package.json", content: '{"name":"project","version":"1.0.0","scripts":{"build":"echo ok"}}' },
  { path: "src/App.tsx", content: "export default function App() {}" },
  { path: "src/theme/tokens.css", content: ":root { --color-primary: #0070f3; }" },
  { path: "src/assets/icon-manifest.json", content: '{"icons":["home","settings"]}' },
  { path: "src/fixtures/screens.json", content: '{"screens":[]}' },
];

describe("Story 05.3: VersionSnapshot Schema & Engine", () => {
  it("validates correct snapshot creation input", () => {
    const input = {
      label: "Initial Stable Release",
      description: "Baseline executable design intent",
      files: sampleFiles,
    };
    const parsed = CreateSnapshotSchema.parse(input);
    expect(parsed.label).toBe("Initial Stable Release");
    expect(parsed.files).toHaveLength(5);
  });

  it("computes authentic standard cryptographic SHA-256 (F-07)", () => {
    expect(computeSha256Hex("")).toBe("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    expect(computeSha256Hex("hello world")).toBe("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
  });

  it("computes deterministic 64-char tree sha256 hash", () => {
    const hash1 = computeTreeSha256(sampleFiles);
    const hash2 = computeTreeSha256([...sampleFiles].reverse());
    expect(hash1).toHaveLength(64);
    expect(hash1).toBe(hash2);
  });

  it("creates sequential version tags on successive snapshots", () => {
    const s1 = createSnapshotFromInput("proj-1", "tenant-1", {
      label: "v1",
      files: sampleFiles,
    }, 0);
    expect(s1.versionTag).toBe("v1.1.0");

    const s2 = createSnapshotFromInput("proj-1", "tenant-1", {
      label: "v2",
      files: sampleFiles,
    }, 1);
    expect(s2.versionTag).toBe("v1.2.0");
  });

  it("performs atomic two-phase revert with auto pre_revert_safety backup (AC-01)", () => {
    const original = createSnapshotFromInput("proj-1", "tenant-1", {
      label: "Initial",
      files: [sampleFiles[0]],
    });

    const modifiedFiles = [
      ...sampleFiles,
      { path: "src/Extra.tsx", content: "export default function Extra() {}" },
    ];

    const { safetySnapshot, restoredFiles, result } = performTwoPhaseRevert(
      modifiedFiles,
      original,
      "proj-1",
      "tenant-1"
    );

    expect(result.success).toBe(true);
    expect(result.revertedToSnapshotId).toBe(original.id);
    expect(safetySnapshot.trigger).toBe("pre_revert_safety");
    expect(safetySnapshot.files).toHaveLength(modifiedFiles.length);
    expect(restoredFiles).toHaveLength(1);
    expect(restoredFiles[0].path).toBe("package.json");
  });

  it("captures uncommitted workspace modifications into pre-revert safety backup (F-01)", () => {
    clearLiveWorkspace();
    setLiveWorkspaceFile("proj-test-workspace", "tenant-test", "src/features/UnsavedComponent.tsx", "export const Unsaved = () => null;");
    const gathered = collectWorkspaceFiles("proj-test-workspace", "tenant-test", []);
    expect(gathered.some((f) => f.path === "src/features/UnsavedComponent.tsx")).toBe(true);

    const otherTenantFiles = collectWorkspaceFiles("proj-test-workspace", "tenant-other", []);
    expect(otherTenantFiles.some((f) => f.path === "src/features/UnsavedComponent.tsx")).toBe(false);
    clearLiveWorkspace();
  });
});

describe("Story 05.3: Export Bundle & Runnable Verification (AC-03)", () => {
  it("verifies export completeness when tokens, icons, package.json, and fixtures exist", () => {
    const check = checkExportCompleteness(sampleFiles);
    expect(check.readyToRun).toBe(true);
    expect(check.missingItems).toHaveLength(0);
  });

  it("detects missing tokens or icons and flags readyToRun as false", () => {
    const incompleteFiles = [sampleFiles[0]];
    const check = checkExportCompleteness(incompleteFiles);
    expect(check.readyToRun).toBe(false);
    expect(check.missingItems).toContain("tokens.css");
    expect(check.missingItems).toContain("icon-manifest.json");
  });

  it("strictly flags incomplete package.json or missing source files as not readyToRun (F-03)", () => {
    const incompleteFiles = [{ path: "package.json", content: '{"name":"incomplete"}' }];
    const check = checkExportCompleteness(incompleteFiles);
    expect(check.readyToRun).toBe(false);
    expect(check.missingItems).toContain("package.json scripts.build");
    expect(check.missingItems).toContain("source code (src/*.tsx)");
  });

  it("builds export zip metadata and formats GitHub PR payload (F-05)", () => {
    const meta = buildExportZipMetadata("my-project", sampleFiles, {
      includeFixtures: true,
      includeTokens: true,
      includeIcons: true,
      format: "zip",
    });

    expect(meta.fileName).toContain("my-project");
    expect(meta.fileName).toContain("bundle-");
    expect(meta.readyToRun).toBe(true);
    expect(meta.filesIncluded).toHaveLength(5);

    const pr = buildGitHubPRPayload("my-project", {
      branchName: "feat/new design",
      prTitle: "Update design intent",
      prBody: "Automated PR",
      targetBranch: "main",
    }, meta.filesIncluded.length);

    expect(pr.branchName).toBe("feat-new-design");
    expect(pr.prUrl).toContain("https://github.com/cowok-ai/my-project/pull/");
    expect(pr.prNumber).toBeGreaterThan(100);
  });
});

describe("Story 05.3: REST API Routes & Tenant Jailing", () => {
  const tenantId = "tenant-gamma";
  let projectId = "";
  let sessionToken = "";
  let firstSnapshotId = "";

  async function ensureSeededProject(): Promise<void> {
    if (projectId && sessionToken) return;
    const lock = await ensureRunningDaemon();
    projectId = crypto.randomUUID();
    const rawBody = JSON.stringify({
      id: projectId,
      slug: `snap-${projectId.slice(0, 8)}`,
      name: "Snapshot Integration Project",
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
      principal: { tenantId, userId: "snapshot-tester", role: "editor" },
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
    sessionToken = issueBffLoopbackSession({ tenantId, role: "editor" }).sessionToken;
  }

  function bffHeaders(token: string, hasBody: boolean = false): Record<string, string> {
    return {
      Host: "127.0.0.1:5173",
      Origin: "http://127.0.0.1:5173",
      "X-Cowok-Session-Token": token,
      ...(hasBody ? { "Content-Type": "application/json" } : {}),
    };
  }

  it("handles GET /snapshots and POST /snapshots with tenant isolation", async () => {
    await ensureSeededProject();

    const postReq = new Request(`http://127.0.0.1:5173/api/projects/${projectId}/snapshots`, {
      method: "POST",
      headers: bffHeaders(sessionToken, true),
      body: JSON.stringify({
        label: "Stable Milestone 1",
        description: "Milestone snapshot",
        files: sampleFiles,
      }),
    });

    const createRes = await createSnapshotRoute(postReq, {
      params: { id: projectId },
    });
    expect(createRes.status).toBe(201);
    const created = await createRes.json();
    firstSnapshotId = created.id;
    expect(created.tenantId).toBe(tenantId);
    expect(created.versionTag).toBe("v1.1.0");

    const getReq = new Request(`http://127.0.0.1:5173/api/projects/${projectId}/snapshots`, {
      method: "GET",
      headers: bffHeaders(sessionToken),
    });

    const listRes = await getSnapshotsRoute(getReq, {
      params: { id: projectId },
    });
    expect(listRes.status).toBe(200);
    const list = await listRes.json();
    expect(list).toHaveLength(1);

    const { sessionToken: otherToken } = issueBffLoopbackSession({
      tenantId: "tenant-other",
      role: "editor",
    });
    const crossReq = new Request(`http://127.0.0.1:5173/api/projects/${projectId}/snapshots`, {
      method: "GET",
      headers: bffHeaders(otherToken),
    });
    const crossTenantGet = await getSnapshotsRoute(crossReq, {
      params: { id: projectId },
    });
    expect(crossTenantGet.status).toBe(403);
  });

  it("handles POST /snapshots/:id/revert and records safety backup", async () => {
    await ensureSeededProject();
    expect(firstSnapshotId).not.toBe("");

    const revertReq = new Request(
      `http://127.0.0.1:5173/api/projects/${projectId}/snapshots/${firstSnapshotId}/revert`,
      {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({ safetyLabel: "Before testing rollback" }),
      }
    );

    const res = await revertRoute(revertReq, {
      params: { id: projectId, snapshotId: firstSnapshotId },
    });
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.success).toBe(true);
    expect(body.revertedToSnapshotId).toBe(firstSnapshotId);
    expect(body.safetySnapshotId).toBeDefined();

    const listRes = await getSnapshotsRoute(
      new Request(`http://127.0.0.1:5173/api/projects/${projectId}/snapshots`, {
        method: "GET",
        headers: bffHeaders(sessionToken),
      }),
      { params: { id: projectId } }
    );
    const list = (await listRes.json()) as Array<{ id: string; trigger: string }>;
    const safety = list.find((s) => s.id === body.safetySnapshotId);
    expect(safety?.trigger).toBe("pre_revert_safety");
  });

  it("rejects invalid export exclusions violating AC-03 (F-02)", async () => {
    await ensureSeededProject();
    const invalidReq = new Request(`http://127.0.0.1:5173/api/projects/${projectId}/snapshots/export`, {
      method: "POST",
      headers: bffHeaders(sessionToken, true),
      body: JSON.stringify({ includeTokens: false }),
    });

    const res = await exportRoute(invalidReq, {
      params: { id: projectId },
    });
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toBe("AC03_RUNNABLE_BUNDLE_VIOLATION");
  });

  it("handles POST /snapshots/export returning AC-03 verified bundle metadata", async () => {
    await ensureSeededProject();
    const exportReq = new Request(`http://127.0.0.1:5173/api/projects/${projectId}/snapshots/export`, {
      method: "POST",
      headers: bffHeaders(sessionToken, true),
      body: JSON.stringify({ includeFixtures: true, includeTokens: true, includeIcons: true, format: "zip" }),
    });

    const res = await exportRoute(exportReq, {
      params: { id: projectId },
    });
    expect(res.status).toBe(200);
    const meta = await res.json();
    expect(meta.readyToRun).toBe(true);
    expect(meta.fileName).toContain(`${projectId}-${tenantId}-bundle-`);
  });

  it("streams binary zip on GET /snapshots/export/download and verifies PKZIP magic (F-03, F-04)", async () => {
    await ensureSeededProject();
    const downloadReq = new Request(`http://127.0.0.1:5173/api/projects/${projectId}/snapshots/export/download`, {
      method: "GET",
      headers: bffHeaders(sessionToken),
    });

    const res = await downloadExportRoute(downloadReq, {
      params: { id: projectId },
    });
    expect(res.status).toBe(200);
    expect(res.headers.get("Content-Type")).toBe("application/zip");
    expect(res.headers.get("Content-Disposition")).toContain("attachment; filename=");
    const buf = new Uint8Array(await res.arrayBuffer());
    expect(buf.length).toBeGreaterThan(100);
    expect(buf[0]).toBe(0x50);
    expect(buf[1]).toBe(0x4b);
    expect(buf[2]).toBe(0x03);
    expect(buf[3]).toBe(0x04);
  });

  it("rejects mock GitHub PR fabrication with 501 GITHUB_INTEGRATION_NOT_CONFIGURED on POST /snapshots/pr (F-05)", async () => {
    await ensureSeededProject();
    const prReq = new Request(`http://127.0.0.1:5173/api/projects/${projectId}/snapshots/pr`, {
      method: "POST",
      headers: bffHeaders(sessionToken, true),
      body: JSON.stringify({
        branchName: "feature/export-sdi",
        prTitle: "Export design intent v1",
        prBody: "Automated export PR",
      }),
    });

    const res = await createPRRoute(prReq, {
      params: { id: projectId },
    });
    expect(res.status).toBe(501);
    const body = await res.json();
    expect(body.error).toBe("GITHUB_INTEGRATION_NOT_CONFIGURED");
  });
});
