// @vitest-environment jsdom
// @story 05.3
import fs from "node:fs";
import path from "node:path";
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { apiVersionSnapshot } from "../../../api/version-snapshot";
import {
  ForkTurnSnapshotSchema,
  GitHubPRSchema,
  OpenIdeHandoffSchema,
  RevertSnapshotSchema,
  type ChatArtifactSnapshotResponse,
  type ExportZipResponse,
  type SnapshotFileItem,
  type VersionSnapshotResponse,
} from "../../../lib/validations/version-snapshot";
import {
  buildExportZipMetadata,
  checkExportCompleteness,
} from "../utils/export-bundle";
import {
  isHistoricalPreviewReadOnly,
  parseTurnManifestFiles,
} from "../utils/snapshot-engine";
import { DrawerSidebar } from "../components/DrawerSidebar";
import { ExportBundleModal } from "../components/ExportBundleModal";
import { GitHubAccountSection } from "../components/GitHubAccountSection";
import { HandoffManifestPanel } from "../components/HandoffManifestPanel";
import { RevertConfirmModal } from "../components/RevertConfirmModal";
import { SnapshotDetailInspector } from "../components/SnapshotDetailInspector";
import { VersionPublishMenuButton } from "../components/VersionPublishMenuButton";
import { TurnSnapshotReceiptCard } from "../components/TurnSnapshotReceiptCard";

const REPO_ROOT = path.resolve(__dirname, "../../../..");

const sampleTurnSnapshot: ChatArtifactSnapshotResponse = {
  id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  tenantId: "local-default",
  projectId: "11111111-1111-4111-8111-111111111111",
  sessionId: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
  messageId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
  nodeId: "node-checkout",
  proposalId: null,
  parentSnapshotId: null,
  artifactType: "turn_workspace_checkpoint",
  contentHash: "a1b2c3d4e5f60718293a4b5c6d7e8f9011223344556677889900aabbccddeeff",
  manifestJson: JSON.stringify({
    files: [{ path: "index.html", content: "<html><body>Historical</body></html>" }],
  }),
  nodeSnapshotJson: JSON.stringify({ id: "node-checkout", label: "Checkout" }),
  label: "Refine checkout card",
  createdAt: "2026-11-16T10:00:00.000Z",
};

const sampleSnapshot: VersionSnapshotResponse = {
  id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
  tenantId: "local-default",
  projectId: "11111111-1111-4111-8111-111111111111",
  versionTag: "v1.1.0",
  label: "Checkpoint 1",
  description: "Baseline",
  trigger: "manual",
  treeSha256: "0".repeat(64),
  files: [{ path: "src/App.tsx", content: "export default function App() { return null; }" }],
  createdAt: "2026-11-16T10:00:00.000Z",
};

afterEach(() => {
  cleanup();
});

describe("Story 05.3: Purged Mocks & Deterministic Contracts", () => {
  it("confirms prSequenceCounter and liveWorkspaceStore are completely purged from source files", () => {
    const exportBundleSrc = fs.readFileSync(
      path.join(REPO_ROOT, "src/features/snapshots/utils/export-bundle.ts"),
      "utf8"
    );
    const engineSrc = fs.readFileSync(
      path.join(REPO_ROOT, "src/features/snapshots/utils/snapshot-engine.ts"),
      "utf8"
    );

    expect(exportBundleSrc).not.toMatch(/prSequenceCounter|buildGitHubPRPayload/);
    expect(engineSrc).not.toMatch(
      /liveWorkspaceStore|getLiveWorkspaceFile|setLiveWorkspaceFile|clearLiveWorkspace|collectWorkspaceFiles/
    );
  });

  it("enforces strict git ref regex on GitHubPRSchema (TC-05.3-08)", () => {
    const valid = GitHubPRSchema.safeParse({
      branchName: "feature/story-05.3_handoff",
      prTitle: "Add handoff manifest",
      targetBranch: "main",
    });
    expect(valid.success).toBe(true);

    const maliciousRefs = [
      "--upload-pack=evil",
      "-leading-dash",
      "foo..bar",
      "feat@{0}",
      "branch.lock",
      "feat//double",
      "branch/",
    ];
    for (const badRef of maliciousRefs) {
      const res = GitHubPRSchema.safeParse({
        branchName: badRef,
        prTitle: "Evil PR",
        targetBranch: "main",
      });
      expect(res.success, `Expected ref '${badRef}' to be rejected`).toBe(false);
    }
  });

  it("enforces jailed relativeEntryPath & allowlisted IDE schemes in OpenIdeHandoffSchema (TC-05.3-11)", () => {
    expect(
      OpenIdeHandoffSchema.safeParse({
        ide: "cursor",
        relativeEntryPath: "src/App.tsx",
      }).success
    ).toBe(true);

    expect(
      OpenIdeHandoffSchema.safeParse({
        ide: "cursor",
        relativeEntryPath: "../../etc/passwd",
      }).success
    ).toBe(false);

    expect(
      OpenIdeHandoffSchema.safeParse({
        ide: "malicious-shell",
      }).success
    ).toBe(false);

    expect(RevertSnapshotSchema.parse({}).restoreMissingNodes).toBe(false);
    expect(ForkTurnSnapshotSchema.parse({}).includeNodeScope).toBe(true);
  });

  it("checks cowok-handoff-manifest.json completeness in checkExportCompleteness", () => {
    const baseFiles: SnapshotFileItem[] = [
      { path: "package.json", content: '{"name":"app","scripts":{"build":"vite build"}}' },
      { path: "src/App.tsx", content: "export default function App() { return null; }" },
      { path: "src/styles/tokens.css", content: ":root { --cw-canvas-bg: #0b0e14; }" },
      { path: "src/assets/icon-manifest.json", content: '{"icons":[]}' },
      { path: "src/fixtures/screens.json", content: '{"screens":[]}' },
    ];

    const missingManifest = checkExportCompleteness(baseFiles);
    expect(missingManifest.hasHandoffManifest).toBe(false);
    expect(missingManifest.readyToRun).toBe(false);
    expect(missingManifest.missingItems).toContain("cowok-handoff-manifest.json");

    const completeFiles = [
      ...baseFiles,
      { path: "cowok-handoff-manifest.json", content: '{"schemaVersion":"1.0"}' },
    ];
    const complete = checkExportCompleteness(completeFiles);
    expect(complete.hasHandoffManifest).toBe(true);
    expect(complete.readyToRun).toBe(true);
    expect(complete.missingItems).toHaveLength(0);

    const meta = buildExportZipMetadata("proj-1", completeFiles);
    expect(meta.handoffManifestIncluded).toBe(true);
    expect(meta.readyToRun).toBe(true);
  });

  it("evaluates read-only historical preview helpers deterministically", () => {
    expect(isHistoricalPreviewReadOnly(null)).toBe(false);
    expect(isHistoricalPreviewReadOnly(sampleTurnSnapshot.id)).toBe(true);
    const parsedFiles = parseTurnManifestFiles(sampleTurnSnapshot.manifestJson);
    expect(parsedFiles).toHaveLength(1);
    expect(parsedFiles[0].path).toBe("index.html");
  });
});

describe("Story 05.3: Pure CSS Snapshot, Turn Receipt & IDE Handoff Components", () => {
  it("renders TurnSnapshotReceiptCard with Historical Preview, Undo Turn & Fork Session actions", () => {
    const onPreview = vi.fn();
    const onUndoTurn = vi.fn();
    const onForkSession = vi.fn();

    render(
      React.createElement(TurnSnapshotReceiptCard, {
        turnSnapshot: sampleTurnSnapshot,
        isPreviewActive: true,
        forkedSessionId: "ffffffff-ffff-4fff-8fff-ffffffffffff",
        detachedNodeIds: ["node-removed-1"],
        onPreview,
        onUndoTurn,
        onForkSession,
      })
    );

    expect(screen.getByTestId("turn-historical-readonly-badge")).toBeTruthy();
    expect(screen.getByTestId("turn-fork-session-indicator")).toBeTruthy();
    expect(screen.getByTestId("turn-detached-nodes-badge")).toBeTruthy();

    fireEvent.click(screen.getByTestId(`turn-preview-btn-${sampleTurnSnapshot.id}`));
    fireEvent.click(screen.getByTestId(`turn-undo-btn-${sampleTurnSnapshot.id}`));
    fireEvent.click(screen.getByTestId(`turn-fork-btn-${sampleTurnSnapshot.id}`));

    expect(onPreview).toHaveBeenCalledWith(sampleTurnSnapshot);
    expect(onUndoTurn).toHaveBeenCalledWith(sampleTurnSnapshot);
    expect(onForkSession).toHaveBeenCalledWith(sampleTurnSnapshot);
  });

  it("renders HandoffManifestPanel with 4 IDE buttons and CLI fallback output", () => {
    const onOpenIde = vi.fn();
    render(
      React.createElement(HandoffManifestPanel, {
        handoffResult: {
          ide: "cursor",
          launchUri: "cursor://file/tmp/project/src/App.tsx",
          cliFallbackCommand: "cursor /tmp/project/src/App.tsx",
          manifestPath: "cowok-handoff-manifest.json",
          manifestSha256: "abc123def456",
        },
        onOpenIde,
      })
    );

    fireEvent.click(screen.getByTestId("ide-handoff-btn-antigravity"));
    expect(onOpenIde).toHaveBeenCalledWith("antigravity");
    expect(screen.getByTestId("ide-handoff-launch-uri").getAttribute("href")).toBe(
      "cursor://file/tmp/project/src/App.tsx"
    );
    expect(screen.getByTestId("ide-handoff-cli-fallback").textContent).toContain("cursor");
  });

  it("renders ExportBundleModal with 5-item checklist and 501 offline ZIP fallback", () => {
    const onDownloadZip = vi.fn();
    const onCreateGitHubPR = vi.fn();
    const metadata: ExportZipResponse = {
      fileName: "proj-bundle.zip",
      byteLength: 4096,
      sha256: "deadbeef".repeat(8),
      filesIncluded: [
        "src/styles/tokens.css",
        "src/assets/icon-manifest.json",
        "src/fixtures/screens.json",
        "package.json",
        "cowok-handoff-manifest.json",
      ],
      readyToRun: true,
      handoffManifestIncluded: true,
    };

    render(
      React.createElement(ExportBundleModal, {
        isOpen: true,
        metadata,
        isExporting: false,
        prError: {
          status: 501,
          code: "GH_CLI_MISSING",
          message: "gh CLI not authenticated",
        },
        onDownloadZip,
        onCreateGitHubPR,
        onOpenIde: vi.fn(),
        onClose: vi.fn(),
      })
    );

    expect(screen.getByTestId("checklist-item-handoff").className).toContain(
      "snap-checklist-item--ok"
    );
    expect(screen.getByTestId("pr-501-fallback-card")).toBeTruthy();

    fireEvent.click(screen.getByTestId("pr-501-zip-fallback-btn"));
    expect(onDownloadZip).toHaveBeenCalledTimes(1);

    // Invalid ref blocked client-side
    fireEvent.change(screen.getByTestId("pr-branch-input"), {
      target: { value: "bad..branch" },
    });
    fireEvent.click(screen.getByTestId("export-create-pr-btn"));
    expect(screen.getByTestId("pr-validation-error")).toBeTruthy();
    expect(onCreateGitHubPR).not.toHaveBeenCalled();
  });

  it("renders RevertConfirmModal 409 deleted-node graph restore guard and DrawerSidebar tabs", () => {
    const onToggleRestore = vi.fn();
    const onConfirm = vi.fn();

    render(
      React.createElement(RevertConfirmModal, {
        isOpen: true,
        targetSnapshot: sampleSnapshot,
        isReverting: false,
        requiresGraphRestore: true,
        restoreMissingNodes: false,
        onToggleRestoreMissingNodes: onToggleRestore,
        onConfirm,
        onClose: vi.fn(),
      })
    );

    expect(screen.getByTestId("node-deleted-restore-banner")).toBeTruthy();
    const confirmBtn = screen.getByTestId("revert-confirm-btn") as HTMLButtonElement;
    expect(confirmBtn.disabled).toBe(true);

    fireEvent.click(screen.getByTestId("restore-missing-nodes-checkbox"));
    expect(onToggleRestore).toHaveBeenCalledWith(true);

    cleanup();
    render(
      React.createElement(DrawerSidebar, {
        snapshots: [sampleSnapshot],
        onClose: vi.fn(),
        onSelect: vi.fn(),
        onOpenExport: vi.fn(),
      })
    );
    expect(screen.getByTestId("snapshot-filter-tab-all")).toBeTruthy();
    expect(screen.getByTestId("snapshot-filter-tab-manual")).toBeTruthy();
    expect(screen.getByTestId("snapshot-filter-tab-auto_proposal_apply")).toBeTruthy();
    expect(screen.getByTestId("snapshot-filter-tab-pre_revert_safety")).toBeTruthy();
    fireEvent.click(screen.getByTestId("snapshot-filter-tab-pre_revert_safety"));
    expect(screen.getByTestId("snapshots-empty-state")).toBeTruthy();
  });

  it("renders VersionPublishMenuButton dropdown + hover submenu and SnapshotDetailInspector FE vs Slide modes", () => {
    const onOpenHistory = vi.fn();
    const onOpenExportSection = vi.fn();
    render(
      React.createElement(VersionPublishMenuButton, {
        snapshotsCount: 3,
        onOpenHistory,
        onOpenExportSection,
      })
    );

    fireEvent.click(screen.getByTestId("version-publish-menu-btn"));
    expect(screen.getByTestId("version-publish-dropdown")).toBeTruthy();
    fireEvent.click(screen.getByTestId("open-export-handoff-btn"));
    expect(screen.getByTestId("export-publish-submenu")).toBeTruthy();
    fireEvent.click(screen.getByTestId("submenu-export-github"));
    expect(onOpenExportSection).toHaveBeenCalledWith("github-pr");

    cleanup();
    const { rerender } = render(
      React.createElement(SnapshotDetailInspector, {
        selected: { kind: "snapshot", item: sampleSnapshot },
        artifactFormat: "dag",
        onCloseDetail: vi.fn(),
        onRequestRevertSnapshot: vi.fn(),
      })
    );
    expect(screen.getByTestId("detail-mode-html")).toBeTruthy();
    expect(screen.getByTestId("detail-mode-userflow")).toBeTruthy();
    expect(screen.getByTestId("detail-mode-files")).toBeTruthy();

    rerender(
      React.createElement(SnapshotDetailInspector, {
        selected: { kind: "snapshot", item: sampleSnapshot },
        artifactFormat: "slide-cms",
        onCloseDetail: vi.fn(),
        onRequestRevertSnapshot: vi.fn(),
      })
    );
    expect(screen.getByTestId("detail-mode-html")).toBeTruthy();
    expect(screen.queryByTestId("detail-mode-userflow")).toBeNull();
  });

  it("renders GitHubAccountSection inline setup when unconfigured and account confirmation when connected", () => {
    const onConnectGitHub = vi.fn().mockResolvedValue(undefined);
    const onSelectGitHubAccount = vi.fn().mockResolvedValue(undefined);
    const onBindGitRemote = vi.fn().mockResolvedValue(undefined);

    const { rerender } = render(
      React.createElement(GitHubAccountSection, {
        status: {
          github: {
            connected: false,
            activeLogin: null,
            activeSource: null,
            accounts: [],
            ghCliInstalled: true,
            ghCliAuthenticated: false,
            remoteUrl: null,
            readyForPr: false,
          },
          ides: [],
          preferredIde: "antigravity",
        },
        onConnectGitHub,
        onSelectGitHubAccount,
        onBindGitRemote,
      })
    );
    expect(screen.getByTestId("github-inline-setup-card")).toBeTruthy();
    expect(screen.getByTestId("github-add-account-form")).toBeTruthy();

    rerender(
      React.createElement(GitHubAccountSection, {
        status: {
          github: {
            connected: true,
            activeLogin: "hatrang-dev",
            activeSource: "vault_pat",
            accounts: [
              {
                login: "hatrang-dev",
                displayName: "Ha Trang",
                avatarUrl: null,
                source: "vault_pat",
                maskedHint: "ghp_...abcd",
                active: true,
              },
            ],
            ghCliInstalled: true,
            ghCliAuthenticated: true,
            remoteUrl: "https://github.com/org/cowok-repo.git",
            readyForPr: true,
          },
          ides: [],
          preferredIde: "cursor",
        },
        onConnectGitHub,
        onSelectGitHubAccount,
        onBindGitRemote,
      })
    );
    expect(screen.getByTestId("github-account-confirm-card").textContent).toContain("@hatrang-dev");
  });
});

describe("Story 05.3: Positive Contract Gate & EC-EXPORT-002 Zero-Tailwind / Night Zero-Amber", () => {
  it("enforces ZERO Tailwind classes and ZERO amber hex across src/features/snapshots/**", () => {
    const snapshotRoot = path.join(REPO_ROOT, "src/features/snapshots");
    const files: string[] = [];
    const walk = (dir: string) => {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          if (entry.name !== "tests") walk(full);
        } else if (entry.name.endsWith(".tsx") || entry.name.endsWith(".ts")) {
          files.push(full);
        }
      }
    };
    walk(snapshotRoot);

    const forbiddenTailwindPattern =
      /(?:^|[\s"'`])(?:flex|flex-col|flex-1|items-center|justify-between|justify-end|fixed|inset-0|inset-y-0|w-full|max-w-md|max-w-lg|z-40|z-50|rounded|rounded-lg|rounded-md|rounded-full|border-l|border-b|border-t|shadow-xl|shadow-2xl|overflow-y-auto|(?:mt|mb|px|py|p|space-y|gap)-[a-z0-9./-]+|(?:bg|text|border)-(?:black|white|background|foreground|primary|secondary|muted|card|emerald|cyan|purple)[a-z0-9./-]*)(?=$|[\s"'`])/;

    for (const file of files) {
      const content = fs.readFileSync(file, "utf8");
      if (file.endsWith(".tsx")) {
        const classAttrMatches = content.match(/className\s*=\s*(?:"[^"]*"|\{`[^`]*`\})/g) || [];
        for (const attr of classAttrMatches) {
          expect(attr, `Tailwind class found in ${path.relative(REPO_ROOT, file)}: ${attr}`).not.toMatch(
            forbiddenTailwindPattern
          );
        }
      }
      expect(content.toLowerCase()).not.toContain("#ff9a2f");
      expect(content.toLowerCase()).not.toContain("#e89a35");
      expect(content).not.toMatch(/\bfetch\s*\(/);
      expect(content).not.toMatch(/\bBuffer\./);
    }

    const tokensCss = fs.readFileSync(path.join(REPO_ROOT, "src/styles/tokens.css"), "utf8");
    const snapCssSection = tokensCss.slice(tokensCss.indexOf("Story 05.3"));
    expect(snapCssSection).toContain("var(--bg-surface)");
    expect(snapCssSection).toContain("var(--action-primary)");
    expect(snapCssSection).toContain("var(--state-success)");
    expect(snapCssSection).toContain("var(--human-accent)");
    expect(snapCssSection.toLowerCase()).not.toContain("#0b0e14");
    expect(snapCssSection.toLowerCase()).not.toContain("#6366f1");
    expect(snapCssSection.toLowerCase()).not.toContain("#10b981");
    expect(snapCssSection.toLowerCase()).not.toContain("#ff9a2f");
    expect(snapCssSection.toLowerCase()).not.toContain("#e89a35");
  });

  it("verifies Positive Contract Gate: 8 guarded BFF routes, Vite BFF wiring, and SDK parity (TC-05.3-18)", () => {
    const bffRoutes = [
      "src/app/api/projects/[id]/snapshots/route.ts",
      "src/app/api/projects/[id]/snapshots/[snapshotId]/route.ts",
      "src/app/api/projects/[id]/snapshots/[snapshotId]/revert/route.ts",
      "src/app/api/projects/[id]/snapshots/[turnSnapshotId]/fork/route.ts",
      "src/app/api/projects/[id]/snapshots/export/route.ts",
      "src/app/api/projects/[id]/snapshots/export/download/route.ts",
      "src/app/api/projects/[id]/snapshots/pr/route.ts",
      "src/app/api/projects/[id]/handoff/open-ide/route.ts",
    ];
    for (const rel of bffRoutes) {
      const content = fs.readFileSync(path.join(REPO_ROOT, rel), "utf8");
      expect(content, `${rel} must use guardedRoute`).toContain("guardedRoute");
    }

    const snapshotsRoute = fs.readFileSync(
      path.join(REPO_ROOT, "src/app/api/projects/[id]/snapshots/route.ts"),
      "utf8"
    );
    expect(snapshotsRoute).toContain("kind");

    const viteRoutes = fs.readFileSync(
      path.join(REPO_ROOT, "src/lib/vite-bff-routes.ts"),
      "utf8"
    );
    expect(viteRoutes).toContain("forkTurnSnapshotRoute");
    expect(viteRoutes).toContain("openIdeHandoffRoute");
    expect(viteRoutes).toContain("singleSnapshotRoute");
    expect(viteRoutes).toContain("/integrations/status");
    expect(viteRoutes).toContain("/integrations/github/connect");

    expect(typeof apiVersionSnapshot.getSnapshots).toBe("function");
    expect(typeof apiVersionSnapshot.listTurnSnapshots).toBe("function");
    expect(typeof apiVersionSnapshot.getSnapshot).toBe("function");
    expect(typeof apiVersionSnapshot.createSnapshot).toBe("function");
    expect(typeof apiVersionSnapshot.revertSnapshot).toBe("function");
    expect(typeof apiVersionSnapshot.forkSessionFromTurn).toBe("function");
    expect(typeof apiVersionSnapshot.exportBundle).toBe("function");
    expect(typeof apiVersionSnapshot.downloadBundle).toBe("function");
    expect(typeof apiVersionSnapshot.createGitHubPR).toBe("function");
    expect(typeof apiVersionSnapshot.openIdeHandoff).toBe("function");
    expect(typeof apiVersionSnapshot.getIntegrationsStatus).toBe("function");
    expect(typeof apiVersionSnapshot.connectGitHubAccount).toBe("function");
    expect(typeof apiVersionSnapshot.selectGitHubAccount).toBe("function");
    expect(typeof apiVersionSnapshot.bindGitRemote).toBe("function");
    expect(typeof apiVersionSnapshot.setDefaultIde).toBe("function");

    const companionSrc = fs.readFileSync(
      path.join(REPO_ROOT, "src/features/flow/components/CompanionCanvasTab.tsx"),
      "utf8"
    );
    expect(companionSrc).toContain("VersionPublishMenuButton");
    expect(companionSrc).toContain("CanvasSnapshotDrawerAndModalHost");
    expect(companionSrc).not.toMatch(/\bfetch\s*\(/);
  });
});
