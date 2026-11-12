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
} from "../utils/snapshot-engine";
import {
  buildExportZipMetadata,
  buildGitHubPRPayload,
  checkExportCompleteness,
} from "../utils/export-bundle";
import {
  GET as getSnapshotsRoute,
  POST as createSnapshotRoute,
  snapshotsStore,
  collectProjectFiles,
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
    const gathered = collectProjectFiles("proj-test-workspace", "tenant-test");
    expect(gathered.some((f) => f.path === "src/features/UnsavedComponent.tsx")).toBe(true);

    const otherTenantFiles = collectProjectFiles("proj-test-workspace", "tenant-other");
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
  const projectId = "project-alpha";
  const tenantId = "tenant-gamma";

  it("handles GET /snapshots and POST /snapshots with tenant isolation", async () => {
    snapshotsStore.clear();

    const postReq = new Request("http://localhost/api/projects/project-alpha/snapshots", {
      method: "POST",
      headers: { "Content-Type": "application/json", "x-tenant-id": tenantId },
      body: JSON.stringify({
        label: "Stable Milestone 1",
        description: "Milestone snapshot",
        files: sampleFiles,
      }),
    });

    const createRes = await createSnapshotRoute(postReq, {
      params: { id: projectId },
      projectId,
      tenantId,
    });
    expect(createRes.status).toBe(201);
    const created = await createRes.json();
    expect(created.tenantId).toBe(tenantId);
    expect(created.versionTag).toBe("v1.1.0");

    const getReq = new Request("http://localhost/api/projects/project-alpha/snapshots", {
      method: "GET",
      headers: { "x-tenant-id": tenantId },
    });

    const listRes = await getSnapshotsRoute(getReq, {
      params: { id: projectId },
      projectId,
      tenantId,
    });
    expect(listRes.status).toBe(200);
    const list = await listRes.json();
    expect(list).toHaveLength(1);

    const crossReq = new Request("http://localhost/api/projects/project-alpha/snapshots", {
      method: "GET",
      headers: { "x-tenant-id": "tenant-other" },
    });
    const crossTenantGet = await getSnapshotsRoute(crossReq, {
      params: { id: projectId },
      projectId,
      tenantId: "tenant-other",
    });
    const crossList = await crossTenantGet.json();
    expect(crossList).toHaveLength(0);
  });

  it("handles POST /snapshots/:id/revert and records safety backup", async () => {
    const list = Array.from(snapshotsStore.values());
    const target = list[0];

    const revertReq = new Request(`http://localhost/api/projects/project-alpha/snapshots/${target.id}/revert`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "x-tenant-id": tenantId },
      body: JSON.stringify({ safetyLabel: "Before testing rollback" }),
    });

    const res = await revertRoute(revertReq, {
      params: { id: projectId, snapshotId: target.id },
      projectId,
      tenantId,
    });
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.success).toBe(true);
    expect(body.revertedToSnapshotId).toBe(target.id);
    expect(body.safetySnapshotId).toBeDefined();

    const safety = snapshotsStore.get(body.safetySnapshotId);
    expect(safety?.trigger).toBe("pre_revert_safety");
  });

  it("rejects invalid export exclusions violating AC-03 (F-02)", async () => {
    const invalidReq = new Request("http://localhost/api/projects/project-alpha/snapshots/export", {
      method: "POST",
      headers: { "Content-Type": "application/json", "x-tenant-id": tenantId },
      body: JSON.stringify({ includeTokens: false }),
    });

    const res = await exportRoute(invalidReq, {
      params: { id: projectId },
      projectId,
      tenantId,
    });
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toBe("AC03_RUNNABLE_BUNDLE_VIOLATION");
  });

  it("handles POST /snapshots/export returning AC-03 verified bundle metadata", async () => {
    const exportReq = new Request("http://localhost/api/projects/project-alpha/snapshots/export", {
      method: "POST",
      headers: { "Content-Type": "application/json", "x-tenant-id": tenantId },
      body: JSON.stringify({ includeFixtures: true, includeTokens: true, includeIcons: true, format: "zip" }),
    });

    const res = await exportRoute(exportReq, {
      params: { id: projectId },
      projectId,
      tenantId,
    });
    expect(res.status).toBe(200);
    const meta = await res.json();
    expect(meta.readyToRun).toBe(true);
    expect(meta.fileName).toContain("project-alpha-tenant-gamma-bundle-");
  });

  it("streams binary zip on GET /snapshots/export/download and verifies PKZIP magic (F-03, F-04)", async () => {
    const downloadReq = new Request("http://localhost/api/projects/project-alpha/snapshots/export/download", {
      method: "GET",
      headers: { "x-tenant-id": tenantId },
    });

    const res = await downloadExportRoute(downloadReq, {
      params: { id: projectId },
      projectId,
      tenantId,
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

  it("creates GitHub PR via POST /snapshots/pr route (F-05)", async () => {
    const prReq = new Request("http://localhost/api/projects/project-alpha/snapshots/pr", {
      method: "POST",
      headers: { "Content-Type": "application/json", "x-tenant-id": tenantId },
      body: JSON.stringify({
        branchName: "feature/export-sdi",
        prTitle: "Export design intent v1",
        prBody: "Automated export PR",
      }),
    });

    const res = await createPRRoute(prReq, {
      params: { id: projectId },
      projectId,
      tenantId,
    });
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.status).toBe("created");
    expect(body.prNumber).toBeGreaterThan(100);
    expect(body.prUrl).toContain("https://github.com/cowok-ai/project-alpha/pull/");
  });
});
