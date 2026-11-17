// @story 05.3
// @implements Unconditional Live E2E Acceptance for Story 05.3 (VersionSnapshot Pre-Revert Safety, Secret-Filtered PKZIP + cowok-handoff-manifest.json, Real PR 400/501 Fallback, 1-Click IDE Handoff, Zero-Tailwind Pure CSS Night Mode)
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

const EVIDENCE_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-05/Story-05.3/qa/evidence"
);

async function mintEditorHeaders(page: Page, request: APIRequestContext, baseUrl: string) {
  const origin = new URL(baseUrl).origin;
  const bootCap = (await page.locator('meta[name="cowok-boot-cap"]').first().getAttribute("content")) || "";
  const sessRes = await request.post(`${baseUrl}api/auth/local-session`, {
    headers: { Origin: origin, "X-Cowok-Boot-Cap": bootCap },
    data: { role: "editor" },
  });
  expect(sessRes.ok()).toBeTruthy();
  const { sessionToken } = (await sessRes.json()) as { sessionToken: string };
  return { Origin: origin, "X-Cowok-Session-Token": sessionToken };
}

async function verifyLiveSnapshotAndHandoffApi(
  request: APIRequestContext,
  baseUrl: string,
  headers: { Origin: string; "X-Cowok-Session-Token": string }
) {
  const projRes = await request.post(`${baseUrl}api/projects`, {
    headers,
    data: {
      slug: `snap-e2e-${crypto.randomUUID().slice(0, 8)}`,
      name: "Story 05.3 Snapshots & Handoff Project",
      product_brief: "Live SQLite V11 snapshot/export/handoff verification",
      provenance: "scratch",
      entryPath: "prompt_prd",
    },
  });
  expect(projRes.status()).toBe(201);
  const project = (await projRes.json()) as { id: string };

  // Create initial runnable Snapshot S1 (including a secret .env file that MUST be stripped on ZIP export)
  const snap1Res = await request.post(`${baseUrl}api/projects/${project.id}/snapshots`, {
    headers,
    data: {
      label: "Initial Night Release",
      description: "Runnable baseline snapshot",
      trigger: "manual",
      files: [
        { path: "package.json", content: JSON.stringify({ name: "export-app", scripts: { build: "vite build" } }) },
        { path: "src/theme/tokens.css", content: ":root { --action-primary: #6366f1; }" },
        { path: "src/assets/icon-manifest.json", content: JSON.stringify({ icons: ["handoff"] }) },
        { path: "src/fixtures/home.json", content: JSON.stringify({ heading: "Checkout" }) },
        { path: "src/App.tsx", content: "export default function App() { return <main>Clean v1</main>; }" },
        { path: ".env", content: "TOP_SECRET_KEY=must-be-redacted" },
      ],
    },
  });
  expect(snap1Res.status()).toBe(201);
  const snap1 = (await snap1Res.json()) as { id: string; versionTag: string };

  // Create Snapshot S2 and revert back to S1 -> verify atomic pre_revert_safety checkpoint
  const snap2Res = await request.post(`${baseUrl}api/projects/${project.id}/snapshots`, {
    headers,
    data: {
      label: "v2 Mutation",
      trigger: "auto_proposal_apply",
      parentSnapshotId: snap1.id,
      files: [
        { path: "package.json", content: JSON.stringify({ name: "export-app", scripts: { build: "vite build" } }) },
        { path: "src/theme/tokens.css", content: ":root { --action-primary: #6366f1; }" },
        { path: "src/assets/icon-manifest.json", content: JSON.stringify({ icons: ["handoff"] }) },
        { path: "src/fixtures/home.json", content: JSON.stringify({ heading: "Checkout V2" }) },
        { path: "src/App.tsx", content: "export default function App() { return <main>V2</main>; }" },
      ],
    },
  });
  expect(snap2Res.status()).toBe(201);

  const revertRes = await request.post(`${baseUrl}api/projects/${project.id}/snapshots/${snap1.id}/revert`, {
    headers,
    data: { safetyLabel: "E2E Pre-Revert Safety" },
  });
  expect(revertRes.status()).toBe(200);
  const revertBody = (await revertRes.json()) as { success: boolean; safetySnapshotId: string };
  expect(revertBody.success).toBe(true);
  expect(revertBody.safetySnapshotId).toBeTruthy();

  // Verify ZIP metadata + binary download includes cowok-handoff-manifest.json and excludes .env
  const expMetaRes = await request.post(`${baseUrl}api/projects/${project.id}/snapshots/export`, {
    headers,
    data: { includeFixtures: true, includeTokens: true, includeIcons: true, format: "zip" },
  });
  expect(expMetaRes.status()).toBe(200);
  const expMeta = (await expMetaRes.json()) as {
    readyToRun: boolean;
    handoffManifestIncluded?: boolean;
    filesIncluded: string[];
  };
  expect(expMeta.readyToRun).toBe(true);
  expect(expMeta.filesIncluded).toContain("cowok-handoff-manifest.json");
  expect(expMeta.filesIncluded).not.toContain(".env");

  const dlRes = await request.get(`${baseUrl}api/projects/${project.id}/snapshots/export/download`, { headers });
  expect(dlRes.status()).toBe(200);
  const zipBytes = await dlRes.body();
  expect(zipBytes[0]).toBe(0x50);
  expect(zipBytes[1]).toBe(0x4b);

  // Verify PR validation rejects dangerous git refs and fails closed with 501 offline when no remote exists
  const badRefRes = await request.post(`${baseUrl}api/projects/${project.id}/snapshots/pr`, {
    headers,
    data: { branchName: "--upload-pack=evil", prTitle: "Bad ref", targetBranch: "main" },
  });
  expect(badRefRes.status()).toBe(400);

  const offlinePrRes = await request.post(`${baseUrl}api/projects/${project.id}/snapshots/pr`, {
    headers,
    data: { branchName: "feat/story-05-3-e2e", prTitle: "Sync executable design", targetBranch: "main" },
  });
  expect(offlinePrRes.status()).toBe(501);

  // Verify 1-Click IDE Handoff and fs_jail traversal rejection
  const ideRes = await request.post(`${baseUrl}api/projects/${project.id}/handoff/open-ide`, {
    headers,
    data: { ide: "cursor", relativeEntryPath: "src/App.tsx" },
  });
  expect(ideRes.status()).toBe(200);
  const ideBody = (await ideRes.json()) as { launchUri: string; cliFallbackCommand: string; manifestSha256: string };
  expect(ideBody.launchUri.startsWith("cursor://file/")).toBe(true);
  expect(ideBody.cliFallbackCommand).toContain("cursor");
  expect(ideBody.manifestSha256.length).toBe(64);

  const traversalRes = await request.post(`${baseUrl}api/projects/${project.id}/handoff/open-ide`, {
    headers,
    data: { ide: "vscode", relativeEntryPath: "../../etc/passwd" },
  });
  expect(traversalRes.status()).toBe(400);
}

test.describe("Story 05.3 Live E2E Acceptance Suite", () => {
  test("verifies VersionHistoryDrawer, ExportBundleModal 501 fallback, 1-Click IDE Handoff, Pure CSS Night tokens, and V11 APIs", async ({
    browser,
    request,
  }) => {
    test.setTimeout(90_000);
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    const qaNonce = crypto.randomBytes(16).toString("hex");
    fs.writeFileSync(
      path.join(EVIDENCE_DIR, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, generatedAt: new Date().toISOString() }, null, 2)
    );

    const videoDir = path.join(EVIDENCE_DIR, "videos");
    fs.mkdirSync(videoDir, { recursive: true });
    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordVideo: { dir: videoDir, size: { width: 1440, height: 900 } },
      recordHar: { path: path.join(EVIDENCE_DIR, "network-trace.har") },
    });
    await context.route(/^http:\/\/(?:127\.0\.0\.1|localhost)(?::\d+)?\//, async (route) => {
      const headers = { ...route.request().headers(), "x-iwish-qa-nonce": qaNonce, "x-tenant-id": "local-default" };
      await route.continue({ headers });
    });
    const page = await context.newPage();

    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];
    const unexpected500s: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error" && !msg.text().includes("501 (Not Implemented)")) {
        consoleErrors.push(msg.text());
      }
    });
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("response", (res) => {
      if (res.status() >= 500 && !(res.status() === 501 && res.url().includes("/snapshots/pr"))) {
        unexpected500s.push(`${res.status()} ${res.url()}`);
      }
    });

    await page.addInitScript(() => {
      if (window.self === window.top) window.localStorage.setItem("cowok_first_run_completed", "true");
    });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;

    try {
      await page.goto(baseUrl, { waitUntil: "networkidle" });
      const headers = await mintEditorHeaders(page, request, baseUrl);
      const onboardRes = await request.put(`${baseUrl}api/agent-settings`, {
        headers,
        data: {
          providerId: "cowok-swarm",
          modelId: "cowok-swarm",
          vaultMode: "vault_0600",
          permissionMode: "auto_sandbox",
          onboardingDone: true,
        },
      });
      expect(onboardRes.ok()).toBeTruthy();

      await verifyLiveSnapshotAndHandoffApi(request, baseUrl, headers);
      await page.reload({ waitUntil: "networkidle" });

      const saveShot = async (name: string) => {
        await page.screenshot({ path: path.join(EVIDENCE_DIR, name), fullPage: true });
      };

      await page.evaluate(() => {
        document.documentElement.setAttribute("data-theme", "night");
        const shell = document.querySelector(".workbench-shell");
        if (shell) {
          shell.setAttribute("data-theme", "night");
          shell.classList.remove("theme-warm", "theme-dim");
          shell.classList.add("theme-night");
        }
      });
      // Verify Option 3 layout: redundant strips removed & review controls in Chat header + Publish icon in HtmlPreviewToolbar
      await expect(page.getByTestId("canvas-snapshot-strip")).toBeHidden();
      await expect(page.getByTestId("proposal-canvas-toolbar")).toBeHidden();
      await expect(page.getByTestId("toggle-designer-talk-btn")).toBeVisible({ timeout: 15_000 });
      await expect(page.getByTestId("toggle-change-proposal-drawer-btn")).toBeVisible();
      await expect(page.getByTestId("version-publish-menu-btn")).toBeVisible();
      await expect(page.getByTestId("version-history-drawer")).toBeHidden();
      await saveShot("01-story-05-3-canvas-snapshot-controls-closed.png");

      // Test collapsed/shortened Chat rail keeps Designer Talk + Proposals icon controls accessible
      await page.locator(".btn-shorten-chat").first().click();
      await expect(page.getByTestId("chat-collapsed-rail")).toBeVisible();
      await expect(page.getByTestId("chat-collapsed-rail").getByTestId("toggle-designer-talk-btn")).toBeVisible();
      await expect(page.getByTestId("chat-collapsed-rail").getByTestId("toggle-change-proposal-drawer-btn")).toBeVisible();
      await page.locator(".btn-shorten-chat").first().click();

      // Open Version & Publish dropdown -> Version History Dual Master-Detail & check Night semantic tokens
      await page.getByTestId("version-publish-menu-btn").click();
      await expect(page.getByTestId("version-publish-dropdown")).toBeVisible();
      await expect(page.getByTestId("toggle-version-history-btn")).toBeVisible();
      await expect(page.getByTestId("open-export-handoff-btn")).toBeVisible();
      await page.getByTestId("toggle-version-history-btn").click();
      await expect(page.getByTestId("version-history-drawer")).toBeVisible();
      const nightCheck = await page.evaluate(() => {
        const cs = getComputedStyle(document.documentElement);
        return {
          actionPrimary: cs.getPropertyValue("--action-primary").trim().toLowerCase(),
          humanAccent: cs.getPropertyValue("--human-accent").trim().toLowerCase(),
        };
      });
      expect(nightCheck.actionPrimary).not.toBe("#ff9a2f");
      expect(nightCheck.humanAccent).not.toBe("#ff9a2f");
      expect(nightCheck.humanAccent).not.toBe("#e89a35");
      await saveShot("02-story-05-3-version-history-drawer-night.png");

      // Open ExportBundleModal from the drawer footer -> inline GitHub Setup, 1-Click IDE Handoff, & real PR 501 ZIP fallback
      await page.getByTestId("open-export-bundle-modal-btn").click();
      await expect(page.getByTestId("export-bundle-modal")).toBeVisible();
      await expect(page.getByTestId("github-inline-setup-card")).toBeVisible();
      await expect(page.getByTestId("handoff-manifest-panel")).toBeVisible();
      await page.getByTestId("ide-handoff-btn-cursor").click();
      await expect(page.getByTestId("ide-handoff-result")).toBeVisible({ timeout: 10_000 });
      await expect(page.getByTestId("ide-handoff-launch-uri")).toContainText("cursor://file/");
      await page.getByTestId("export-create-pr-btn").click();
      await expect(page.getByTestId("pr-501-fallback-card")).toBeVisible({ timeout: 10_000 });
      await saveShot("03-story-05-3-export-pr-ide-handoff-modal.png");

      fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), await page.content(), "utf8");
      expect(consoleErrors).toEqual([]);
      expect(pageErrors).toEqual([]);
      expect(unexpected500s).toEqual([]);
    } finally {
      const vid = page.video();
      await context.close();
      if (vid) {
        try {
          await vid.saveAs(path.join(EVIDENCE_DIR, "acceptance-story-05.3.webm"));
        } catch {
          /* ignore video flush failure */
        }
      }
    }
  });
});
