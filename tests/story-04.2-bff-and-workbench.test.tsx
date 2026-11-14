/** @vitest-environment jsdom */
// @story 04.2
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import fs from "node:fs";
import path from "node:path";
import { Readable } from "node:stream";
import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, cleanup, renderHook, act } from "@testing-library/react";
import { issueBffLoopbackSession, setBoundBffPort } from "../src/lib/api-guard";
import { ensureRunningDaemon } from "../src/lib/daemon-client";
import { matchBffRoute } from "../src/lib/vite-bff-routes";
import { HarnessDaemonApiError, HarnessDaemonClient } from "../src/api/harness-daemon";
import { PromoteFolderInputSchema, type ProjectDto } from "../src/lib/validations/harness-daemon";
import {
  ACTIVE_PROJECT_HINT_KEY,
  MANAGED_SLUG_REGEX,
  toManagedSlug,
} from "../src/pages/workbench/hooks/useProjectTrashAndBrowse";
import { useWorkbenchProjectsAndSessions } from "../src/pages/workbench/hooks/useWorkbenchSessionSettings";
import { NewConversationModal } from "../src/pages/workbench/components/NewConversationModal";
import { DaemonFolderPickerDialog } from "../src/pages/workbench/components/DaemonFolderPickerDialog";
import { SidebarTrashJournalDrawer } from "../src/pages/workbench/components/SidebarTrashJournalDrawer";

const HTTP_OK = 200;
const HTTP_BAD_REQUEST = 400;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const BFF_TEST_PORT = 5173;
const FAKE_DAEMON_PORT = 49999;
const SCHEMA_STALE_V6 = 6;
const SCHEMA_CURRENT_V7 = 7;
const SAMPLE_TS_MS = 1794654600000;

const SAMPLE_PROJECT_DTO: ProjectDto = {
  id: "11111111-1111-4111-8111-111111111111",
  slug: "checkout-flow",
  name: "Thanh Toán Đơn Hàng Đỏ 2026",
  product_brief: "Story 04.2 test project",
  workspace_root: "~/CowokProjects/checkout-flow",
  project_code: "P-2026-001",
  provenance: "folder-backed",
  trash_deleted_at: null,
  created_at: SAMPLE_TS_MS,
  updated_at: SAMPLE_TS_MS,
};

function makeWebRequest(method: string, urlPath: string, headers: Record<string, string>, body?: string): Request {
  return new Request(`http://127.0.0.1:${BFF_TEST_PORT}${urlPath}`, {
    method,
    headers: { host: `127.0.0.1:${BFF_TEST_PORT}`, ...headers },
    body,
  });
}

function listFilesRecursive(dir: string): string[] {
  if (!fs.existsSync(dir)) return [];
  const results: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) results.push(...listFilesRecursive(full));
    else results.push(full);
  }
  return results;
}

describe("Story 04.2: Source Security Invariants & BFF Routes", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    window.localStorage.clear();
  });

  it("enforces zero in-memory new Map() in src/app/api/** and zero browser FS pickers in workbench", () => {
    const apiFiles = listFilesRecursive(path.resolve("src/app/api"));
    expect(apiFiles.length).toBeGreaterThanOrEqual(4);
    for (const file of apiFiles) {
      expect(fs.readFileSync(file, "utf8"), `${file} must not keep in-memory Map stores`).not.toMatch(/\bnew\s+Map\b/);
    }

    const workbenchFiles = listFilesRecursive(path.resolve("src/pages/workbench"));
    expect(workbenchFiles.length).toBeGreaterThan(0);
    for (const file of workbenchFiles) {
      const src = fs.readFileSync(file, "utf8");
      expect(src, `${file} must not use showDirectoryPicker`).not.toContain("showDirectoryPicker");
      expect(src, `${file} must not use webkitdirectory`).not.toContain("webkitdirectory");
    }
  });

  it("preserves Readable.fromWeb SSE streaming and streams encoded chunks cleanly", async () => {
    const pluginSrc = fs.readFileSync(path.resolve("src/lib/vite-bff-plugin.ts"), "utf8");
    expect(pluginSrc).toContain("Readable.fromWeb");
    const webStream = new ReadableStream<Uint8Array>({
      start(ctrl) {
        ctrl.enqueue(new TextEncoder().encode("data: {\"ok\":true}\n\n"));
        ctrl.close();
      },
    });
    const nodeStream = Readable.fromWeb(webStream as unknown as import("node:stream/web").ReadableStream<Uint8Array>);
    const chunks: string[] = [];
    for await (const chunk of nodeStream) chunks.push(Buffer.from(chunk).toString("utf8"));
    expect(chunks.join("")).toBe("data: {\"ok\":true}\n\n");
  });

  it("rejects daemon lockfile and health responses with schema_version < 7", async () => {
    const origWs = process.env.UXUI_OS_WORKSPACE_ROOT;
    const tmpRoot = fs.mkdtempSync(path.join(process.cwd(), "node_modules/.story042-ws-"));
    const lockDir = path.join(tmpRoot, ".cowok-design");
    fs.mkdirSync(lockDir, { recursive: true });
    process.env.UXUI_OS_WORKSPACE_ROOT = tmpRoot;
    const baseLock = { host: "127.0.0.1", port: FAKE_DAEMON_PORT, pid: process.pid, daemon_nonce: "n1", hmac_key_hex: "00" };
    let spawnedPid: number | null = null;
    try {
      const healthSpy = vi.spyOn(globalThis, "fetch").mockResolvedValue(
        new Response(JSON.stringify({ service: "cowok-design-harness-daemon", schema_version: SCHEMA_STALE_V6 }), { status: HTTP_OK })
      );
      fs.writeFileSync(path.join(lockDir, "daemon.lock"), JSON.stringify({ ...baseLock, schema_version: SCHEMA_CURRENT_V7 }));
      await expect(ensureRunningDaemon()).rejects.toThrow("Unexpected daemon health identity or schema_version");

      healthSpy.mockRestore();
      fs.writeFileSync(path.join(lockDir, "daemon.lock"), JSON.stringify({ ...baseLock, schema_version: SCHEMA_STALE_V6 }));
      const upgraded = await ensureRunningDaemon();
      spawnedPid = upgraded.pid;
      expect(upgraded.schema_version).toBeGreaterThanOrEqual(SCHEMA_CURRENT_V7);
      expect(upgraded.port).not.toBe(FAKE_DAEMON_PORT);
    } finally {
      if (spawnedPid && spawnedPid !== process.pid) {
        try { process.kill(spawnedPid, "SIGTERM"); } catch { /* exited */ }
      }
      if (origWs === undefined) delete process.env.UXUI_OS_WORKSPACE_ROOT;
      else process.env.UXUI_OS_WORKSPACE_ROOT = origWs;
      fs.rmSync(tmpRoot, { recursive: true, force: true });
    }
  });

  it("fails closed on BFF routes for missing session, wrong role, cross-origin, and missing token", async () => {
    setBoundBffPort(BFF_TEST_PORT);
    try {
      const { sessionToken: viewerToken } = issueBffLoopbackSession({ tenantId: "local-default", userId: "v1", role: "viewer" });
      const { sessionToken: editorToken } = issueBffLoopbackSession({ tenantId: "local-default", userId: "e1", role: "editor" });

      const browseMatch = matchBffRoute("GET", "/api/fs/browse");
      expect(browseMatch).not.toBeNull();
      const unauthRes = await browseMatch!.handler(makeWebRequest("GET", "/api/fs/browse", {}), { params: browseMatch!.params });
      expect(unauthRes.status).toBe(HTTP_UNAUTHORIZED);

      const restoreMatch = matchBffRoute("POST", "/api/projects/p1/restore");
      expect(restoreMatch).not.toBeNull();
      const viewerForbidden = await restoreMatch!.handler(
        makeWebRequest("POST", "/api/projects/p1/restore", {
          "x-cowok-session-token": viewerToken,
          origin: `http://127.0.0.1:${BFF_TEST_PORT}`,
        }),
        { params: restoreMatch!.params }
      );
      expect(viewerForbidden.status).toBe(HTTP_FORBIDDEN);

      const crossOriginForbidden = await restoreMatch!.handler(
        makeWebRequest("POST", "/api/projects/p1/restore", {
          "x-cowok-session-token": editorToken,
          origin: "http://evil.example:9999",
        }),
        { params: restoreMatch!.params }
      );
      expect(crossOriginForbidden.status).toBe(HTTP_FORBIDDEN);

      const promoteMatch = matchBffRoute("POST", "/api/projects/p1/promote");
      expect(promoteMatch).not.toBeNull();
      const invalidPromote = await promoteMatch!.handler(
        makeWebRequest(
          "POST",
          "/api/projects/p1/promote",
          {
            "x-cowok-session-token": editorToken,
            origin: `http://127.0.0.1:${BFF_TEST_PORT}`,
            "content-type": "application/json",
          },
          JSON.stringify({ target_directory: "/tmp/app" })
        ),
        { params: promoteMatch!.params }
      );
      expect(invalidPromote.status).toBe(HTTP_BAD_REQUEST);
      expect(PromoteFolderInputSchema.safeParse({ target_directory: "/tmp/app", path_token: "tok-123" }).success).toBe(true);
      expect(PromoteFolderInputSchema.safeParse({ target_directory: "/tmp/app" }).success).toBe(false);
    } finally {
      setBoundBffPort(null);
    }
  });

  it("wires HarnessDaemonClient SDK for trash listing, restore, browse, token verify, and promote", async () => {
    const calls: string[] = [];
    const client = new HarnessDaemonClient({
      baseUrl: "http://127.0.0.1:43111",
      getSessionToken: () => "test-editor-token",
      fetchImpl: async (input) => {
        const url = String(input);
        calls.push(url);
        if (url.includes("/api/projects?include_trashed=true")) {
          return new Response(JSON.stringify([{ ...SAMPLE_PROJECT_DTO, trash_deleted_at: SAMPLE_TS_MS }]), { status: HTTP_OK });
        }
        if (url.endsWith("/restore")) {
          return new Response(JSON.stringify({ restored: true, project: SAMPLE_PROJECT_DTO }), { status: HTTP_OK });
        }
        if (url.includes("/api/fs/browse")) {
          return new Response(
            JSON.stringify({ current_path: "/tmp/CowokProjects", parent_path: "/tmp", managed_root: "/tmp/CowokProjects", entries: [] }),
            { status: HTTP_OK }
          );
        }
        if (url.endsWith("/verify-path-token")) {
          return new Response(
            JSON.stringify({ valid: true, normalized_relative_path: "app", canonical_path: "/tmp/app", project_id: SAMPLE_PROJECT_DTO.id, principal_id: "u1", daemon_nonce: "n1", ancestor_dev_ino: "1:2", target_exists: true }),
            { status: HTTP_OK }
          );
        }
        if (url.endsWith("/promote")) {
          return new Response(
            JSON.stringify({ project_id: SAMPLE_PROJECT_DTO.id, boundary_type: "folder-backed", workspace_path: "/tmp/app", config_dir: "/tmp/app/.cowok", is_git_repository: true, promoted_at: SAMPLE_TS_MS, created_at: SAMPLE_TS_MS, updated_at: SAMPLE_TS_MS }),
            { status: HTTP_OK }
          );
        }
        return new Response(JSON.stringify({ error: "FS_JAIL_VIOLATION", message: "Outside jail" }), { status: HTTP_FORBIDDEN });
      },
    });

    expect((await client.listProjects({ includeTrashed: true }))[0].trash_deleted_at).toBe(SAMPLE_TS_MS);
    expect((await client.restoreProject(SAMPLE_PROJECT_DTO.id)).restored).toBe(true);
    expect((await client.browseFs("/tmp/CowokProjects")).managed_root).toBe("/tmp/CowokProjects");
    const verified = await client.verifyPathToken(SAMPLE_PROJECT_DTO.id, { token: "signed-ptok", target_directory: "/tmp/app" });
    expect(verified.valid).toBe(true);
    const promoted = await client.promoteProject(SAMPLE_PROJECT_DTO.id, { target_directory: "/tmp/app", path_token: "signed-ptok" });
    expect(promoted.is_git_repository).toBe(true);
    expect(calls.some((u) => u.includes("include_trashed=true"))).toBe(true);
    expect(HarnessDaemonApiError).toBeTypeOf("function");
  });
});

describe("Story 04.2: Workbench Slug, Daemon Picker, Trash Drawer & F5 Hydration", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    window.localStorage.clear();
  });

  it("normalizes Vietnamese diacritics and symbols into valid ManagedSlugs", () => {
    const slug = toManagedSlug("Thanh Toán Đơn Hàng Đỏ 2026");
    expect(slug).toBe("thanh-toan-don-hang-do-2026");
    expect(MANAGED_SLUG_REGEX.test(slug)).toBe(true);
    expect(toManagedSlug("   !!! @@@   ")).toBe("cowok-project");
  });

  it("renders distinct slug inputs across NewConversationModal and DaemonFolderPickerDialog and surfaces 403 jail errors", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL) => {
        if (String(input).includes("/api/auth/local-session")) {
          return new Response(JSON.stringify({ sessionToken: "test-tok" }), { status: HTTP_OK });
        }
        return new Response(JSON.stringify({ error: "FS_JAIL_VIOLATION", message: "Path forbidden (/etc)" }), { status: HTTP_FORBIDDEN });
      })
    );

    render(
      <NewConversationModal
        isOpen
        onClose={vi.fn()}
        projects={[{ id: SAMPLE_PROJECT_DTO.id, name: SAMPLE_PROJECT_DTO.name, localPath: SAMPLE_PROJECT_DTO.workspace_root ?? undefined }]}
        activeProjectId={SAMPLE_PROJECT_DTO.id}
        onCreateInProject={vi.fn()}
        onCreateNewProject={vi.fn()}
        onCreateStandalone={vi.fn()}
      />
    );

    fireEvent.click(document.getElementById("tab-new-project-btn")!);
    const newConvoSlugInput = document.getElementById("newConvoManagedSlugInput") as HTMLInputElement;
    expect(newConvoSlugInput).toBeTruthy();

    fireEvent.click(screen.getByTestId("open-daemon-fs-picker-btn"));
    await waitFor(() => expect(document.getElementById("daemonFolderPickerDialog")).toBeTruthy());
    const pickerSlugInput = document.getElementById("pickerManagedSlugInput") as HTMLInputElement;
    expect(pickerSlugInput).toBeTruthy();
    expect(pickerSlugInput).not.toBe(newConvoSlugInput);

    await waitFor(() => expect(screen.getByTestId("daemon-picker-error")).toBeTruthy());
    expect(screen.getByTestId("daemon-picker-error").textContent).toContain("Path forbidden");
    cleanup();

    render(
      <DaemonFolderPickerDialog
        isOpen
        mode="promote"
        projectId={SAMPLE_PROJECT_DTO.id}
        initialSlug="my-app"
        onClose={vi.fn()}
        onConfirmPromote={vi.fn(async () => Promise.reject(new Error("FS_JAIL_VIOLATION")))}
      />
    );
    await waitFor(() => expect(document.getElementById("confirmFolderPromoteBtn")).toBeTruthy());
  });

  it("renders SidebarTrashJournalDrawer with project_code badge and triggers 1-click restore", () => {
    const onRestore = vi.fn();
    const trashed: ProjectDto = {
      ...SAMPLE_PROJECT_DTO,
      trash_deleted_at: SAMPLE_TS_MS,
    };
    render(<SidebarTrashJournalDrawer trashedProjects={[trashed]} onRestoreProject={onRestore} />);

    const drawer = document.getElementById("sidebarTrashJournal");
    expect(drawer).toBeTruthy();
    fireEvent.click(screen.getByTestId("sidebar-trash-journal-toggle"));
    expect(drawer!.textContent).toContain("P-2026-001");
    fireEvent.click(screen.getByTestId(`restore-project-btn-${trashed.id}`));
    expect(onRestore).toHaveBeenCalledWith(trashed.id);
  });

  it("hydrates F5 state from SQLite V7, clears project-bound defaults when empty, and rolls back failed creates", async () => {
    window.localStorage.setItem(ACTIVE_PROJECT_HINT_KEY, "deleted-or-stale-id");
    let mode: "empty" | "fail-create" = "empty";

    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = String(input);
        if (url.includes("/api/auth/local-session")) {
          return new Response(JSON.stringify({ sessionToken: "test-tok" }), { status: HTTP_OK });
        }
        if (url.includes("/api/projects?include_trashed=true")) {
          return new Response(JSON.stringify(mode === "empty" ? [] : [SAMPLE_PROJECT_DTO]), { status: HTTP_OK });
        }
        if (url.includes("/sessions")) return new Response(JSON.stringify([]), { status: HTTP_OK });
        if (url.endsWith("/api/projects") && init?.method === "POST") {
          return new Response(JSON.stringify({ error: "CONSTRAINT", message: "Slug exists" }), { status: HTTP_FORBIDDEN });
        }
        return new Response(JSON.stringify([]), { status: HTTP_OK });
      })
    );

    const { result, unmount } = renderHook(() => useWorkbenchProjectsAndSessions());
    await waitFor(() => expect(result.current.projects).toEqual([]));
    expect(result.current.sessions.filter((s) => Boolean(s.projectId))).toEqual([]);
    expect(window.localStorage.getItem(ACTIVE_PROJECT_HINT_KEY)).toBeNull();
    unmount();

    mode = "fail-create";
    window.localStorage.setItem(ACTIVE_PROJECT_HINT_KEY, "stale-hint");
    const hook2 = renderHook(() => useWorkbenchProjectsAndSessions());
    await waitFor(() => expect(hook2.result.current.projects).toHaveLength(1));
    expect(hook2.result.current.activeProjectId).toBe(SAMPLE_PROJECT_DTO.id);

    act(() => {
      hook2.result.current.handleCreateNewProject("Broken Project", "Init", "folder-backed", { managedSlug: "broken" });
    });
    await waitFor(() => expect(hook2.result.current.toastState?.kind).toBe("danger"));
    expect(hook2.result.current.projects).toHaveLength(1);
    expect(hook2.result.current.projects[0].id).toBe(SAMPLE_PROJECT_DTO.id);
    hook2.unmount();
  });

  it("omits managed_slug for scratch and custom browse paths while sending managedSlug for 1-click managed creation", async () => {
    const createBodies: Array<Record<string, unknown>> = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = String(input);
        if (url.includes("/api/auth/local-session")) {
          return new Response(JSON.stringify({ sessionToken: "test-tok" }), { status: HTTP_OK });
        }
        if (url.includes("/api/projects?include_trashed=true")) {
          return new Response(JSON.stringify([]), { status: HTTP_OK });
        }
        if (url.endsWith("/api/projects") && init?.method === "POST") {
          const parsed = JSON.parse(String(init.body || "{}")) as Record<string, unknown>;
          createBodies.push(parsed);
          return new Response(JSON.stringify({ ...SAMPLE_PROJECT_DTO, id: String(parsed.id), provenance: String(parsed.provenance) }), { status: 201 });
        }
        return new Response(JSON.stringify({}), { status: HTTP_OK });
      })
    );

    const { result, unmount } = renderHook(() => useWorkbenchProjectsAndSessions());
    await waitFor(() => expect(result.current.projects).toEqual([]));
    act(() => {
      result.current.handleCreateNewProject("Scratch App", "Init", "scratch", undefined);
      result.current.handleCreateNewProject("Custom App", "Init", "folder-backed", { localPath: "/tmp/custom-repo" });
      result.current.handleCreateNewProject("Managed App", "Init", "folder-backed", { managedSlug: "managed-app" });
    });
    await waitFor(() => expect(createBodies).toHaveLength(3));
    expect(createBodies[0].provenance).toBe("scratch");
    expect(createBodies[0].managed_slug).toBeUndefined();
    expect(createBodies[0].workspace_root).toBeNull();
    expect(createBodies[1].provenance).toBe("folder-backed");
    expect(createBodies[1].managed_slug).toBeUndefined();
    expect(createBodies[1].workspace_root).toBe("/tmp/custom-repo");
    expect(createBodies[2].provenance).toBe("folder-backed");
    expect(createBodies[2].managed_slug).toBe("managed-app");
    unmount();
  });

  it("blocks same-origin port fallback when an active bootCapToken exists and releases coordinator slots on failJob", async () => {
    const { PassThrough } = await import("node:stream");
    const {
      issueBootCapToken,
      revokeBootCapToken,
      handleBootstrapSession,
    } = await import("../src/lib/vite-bff-bootstrap");
    const { KeyedParallelCoordinator } = await import("../src/features/flow/store/keyed-parallel-coordinator");

    setBoundBffPort(BFF_TEST_PORT);
    const activeCap = issueBootCapToken();
    try {
      const req = new PassThrough() as unknown as import("node:http").IncomingMessage &
        import("node:stream").PassThrough;
      req.method = "POST";
      req.url = "/api/auth/local-session";
      req.headers = {
        host: `127.0.0.1:${BFF_TEST_PORT}`,
        origin: `http://127.0.0.1:${BFF_TEST_PORT}`,
        "sec-fetch-site": "same-origin",
        "content-type": "application/json",
      };
      req.end(JSON.stringify({ role: "editor" }));
      let rawOut = "";
      const res = {
        statusCode: 0,
        setHeader: vi.fn(),
        end: (chunk?: string) => {
          if (chunk) rawOut += chunk;
        },
      } as unknown as import("node:http").ServerResponse;
      await handleBootstrapSession(req, res);
      expect(res.statusCode).toBe(HTTP_FORBIDDEN);
      expect(rawOut).toContain("INSUFFICIENT_ROLE");
    } finally {
      revokeBootCapToken(activeCap);
    }

    const coord = new KeyedParallelCoordinator(4);
    const job = coord.enqueueJob({
      projectId: "non-uuid-project",
      sessionId: "sess-1",
      nodeId: "node-1",
      prompt: "test fail release",
    });
    expect(coord.getActiveJobs()).toHaveLength(1);
    coord.failJob(job.id, "NON_UUID_PROJECT");
    expect(coord.getActiveJobs()).toHaveLength(0);
    expect(coord.getSummary().runningCount).toBe(0);
    expect(coord.getSummary().totalActive).toBe(0);
  });
});
