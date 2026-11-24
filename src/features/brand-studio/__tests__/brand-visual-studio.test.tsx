// @vitest-environment jsdom
// @story 05.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  BrandVisualStudioModal,
  HiddenSpecDrawer,
  useBrandStudioController,
} from "../index";
import type {
  BrandProposalV1,
  FeDataContractDraft,
  HeritageDesignPack,
  HiddenSpecDocument,
  PopularComponentAuditItem,
} from "../../../lib/validations/cognitive-studio";

const PROJECT_ID = "11111111-1111-4111-8111-111111111111";

const SAMPLE_PROPOSAL_WARM: BrandProposalV1 = {
  id: "brand-atelier-precision",
  brandName: "Precision Atelier",
  rationale: "Balanced hierarchy with 1px semantic borders.",
  inspirationRefs: ["Linear", "Stripe"],
  themeModes: [
    {
      modeId: "warm",
      label: "Warm",
      tokens: {
        "--bg-canvas": "#F6F2EA",
        "--bg-surface": "#FDFBF7",
        "--text-primary": "#23211D",
        "--border-default": "#DDD6C7",
        "--action-primary": "#1D4ED8",
        "--text-on-accent": "#FFFFFF",
      },
      wcagAaVerified: true,
    },
  ],
  fontPairing: { heading: "Inter, sans-serif", body: "Inter, sans-serif", mono: "monospace" },
  radiusScale: "balanced",
  density: "comfortable",
  componentStyleOverrides: {},
};

const SAMPLE_AUDIT: PopularComponentAuditItem[] = [
  { componentKind: "button", category: "input", status: "detected_in_sketch", reason: "Primary button", selected: true, occurrenceCount: 3 },
  { componentKind: "toast", category: "feedback", status: "suggested_contextual", reason: "Save feedback", selected: false, occurrenceCount: 0 },
  { componentKind: "drawer", category: "overlay", status: "not_needed", reason: "Optional detail drawer", selected: false, occurrenceCount: 0 },
];

const SAMPLE_SPECS: HiddenSpecDocument[] = [
  {
    specKey: "02_BRAND_IDENTITY",
    title: "02 · Brand Identity & Theme Modes",
    revision: 2,
    contentSha256: "a".repeat(64),
    contentMarkdown: "# 02 Brand Identity\nMode: warm",
    updatedAt: 1700000000000,
  },
];

const SAMPLE_FE_CONTRACT: FeDataContractDraft = {
  projectId: PROJECT_ID,
  screenId: "default",
  boundaryNotice: "FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY",
  viewModelFields: [
    { ffId: "ff-kpi-mrr", fieldName: "kpiMrr", inferredType: "currency", sampleValue: "$24,800" },
  ],
  actionInputs: [
    { actionId: "submitDeal", triggerFfId: "ff-submit-deal", requiredFields: ["dealTitle"] },
  ],
  subStateFixtures: {
    DEFAULT: { kpiMrr: "$24,800" },
    EMPTY: { isEmpty: true },
    LOADING: { isLoading: true },
    ERROR: { errorCode: "VIEW_DATA_UNAVAILABLE" },
  },
};

const SAMPLE_PACK: HeritageDesignPack = {
  id: "pack-056-1",
  tenantId: "local-default",
  name: "Atelier Warm Pack v1",
  sourceProjectId: PROJECT_ID,
  entryMode: "DECK_FROM_APP",
  brandIdentity: { proposalId: "brand-atelier-precision" },
  componentRegistry: { components: ["button", "toast"] },
  presentationTheme: { aspectRatio: "16:9" },
  themeModes: ["warm", "dim"],
  createdAt: 1700000000000,
};

function StudioHarness({
  initialModes,
  autoOpenStudio = false,
  autoOpenDrawer = false,
  onApplyBrandTokens,
}: {
  initialModes: string[];
  autoOpenStudio?: boolean;
  autoOpenDrawer?: boolean;
  onApplyBrandTokens?: (proposal: BrandProposalV1, activeModeId: string) => void;
}) {
  const controller = useBrandStudioController({
    projectId: PROJECT_ID,
    screenId: "default",
    locale: "vi",
    initialThemeModes: initialModes,
    onApplyBrandTokens,
  });

  React.useEffect(() => {
    if (autoOpenStudio) controller.openStudio();
    if (autoOpenDrawer) controller.openHiddenSpec();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <>
      <button type="button" data-testid="trigger-studio" onClick={controller.openStudio}>Open Studio</button>
      <button type="button" data-testid="trigger-drawer" onClick={controller.openHiddenSpec}>Open Drawer</button>
      <BrandVisualStudioModal controller={controller} />
      <HiddenSpecDrawer controller={controller} />
    </>
  );
}

function mockStudioFetch(opts: { conflictOnPut?: boolean } = {}) {
  let packs = [SAMPLE_PACK];
  return vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
    const method = init?.method ?? "GET";

    if (url.includes("/api/auth/local-session")) {
      return new Response(JSON.stringify({ sessionToken: "test-session-token" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }

    if (url.includes("/api/v1/cognitive/brand-proposals") && method === "POST") {
      const reqBody = JSON.parse(String(init?.body ?? "{}")) as { themeModes: string[]; activeModeId?: string };
      const modes = reqBody.themeModes ?? ["warm"];
      const activeModeId = reqBody.activeModeId ?? modes[0]!;
      const prop: BrandProposalV1 = {
        ...SAMPLE_PROPOSAL_WARM,
        themeModes: modes.map((m) => ({ ...SAMPLE_PROPOSAL_WARM.themeModes[0]!, modeId: m, label: m })),
      };
      return new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-bp",
          data: {
            projectId: PROJECT_ID,
            requestedModes: modes,
            activeModeId,
            selectedProposalId: prop.id,
            proposals: [prop],
            previewBundle: {
              proposalId: prop.id,
              activeModeId,
              requestedModes: modes,
              totalSketchPages: 3,
              cappedToRepresentative: false,
              pages: [
                {
                  screenId: "sketch-hero",
                  title: "Hero Overview",
                  pageRole: "hero",
                  modeId: activeModeId,
                  renderedHtml: `<html data-brand-mode="${activeModeId}"><body><h1>Hero</h1></body></html>`,
                  contentSha256: "b".repeat(64),
                  wcagAaVerified: true,
                  contrastReports: [],
                  renderTimeMicros: 380,
                },
              ],
              totalRenderTimeMicros: 920,
            },
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    }

    if (url.includes("/api/v1/cognitive/scan-components")) {
      const reqBody = JSON.parse(String(init?.body ?? "{}")) as { selectedKinds?: string[] };
      const items = reqBody.selectedKinds
        ? SAMPLE_AUDIT.map((it) => ({ ...it, selected: reqBody.selectedKinds!.includes(it.componentKind) }))
        : SAMPLE_AUDIT;
      return new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-scan",
          data: {
            projectId: PROJECT_ID,
            portalId: "SalesApp",
            extractedKinds: ["button"],
            auditItems: items,
            detectedCount: 1,
            suggestedCount: 1,
            notNeededCount: 1,
            syncedToRegistryCount: items.filter((i) => i.selected).length,
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    }

    if (url.includes("/api/v1/cognitive/fe-data-contract")) {
      return new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-fe",
          data: {
            draft: SAMPLE_FE_CONTRACT,
            draftTsSource: 'export const FE_CONTRACT_BOUNDARY = "FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY" as const;\nexport interface ScreenViewModel { kpiMrr: string; }',
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    }

    if (url.includes("/api/v1/cognitive/hidden-specs") && method === "GET") {
      return new Response(
        JSON.stringify({ ok: true, requestId: "req-hs-get", data: { projectId: PROJECT_ID, specs: SAMPLE_SPECS } }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    }

    if (url.includes("/api/v1/cognitive/hidden-specs") && method === "PUT") {
      if (opts.conflictOnPut) {
        return new Response(
          JSON.stringify({
            error: {
              code: "STALE_MAPPING",
              message: "Concurrent edit conflict on 02_BRAND_IDENTITY",
              requestId: "req-hs-409",
              details: { specKey: "02_BRAND_IDENTITY", expectedRevision: 2, currentRevision: 3 },
            },
          }),
          { status: 409, headers: { "Content-Type": "application/json" } }
        );
      }
      return new Response(
        JSON.stringify({ ok: true, requestId: "req-hs-put", data: { ...SAMPLE_SPECS[0]!, revision: 3 } }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    }

    if (url.includes("/api/v1/cognitive/heritage-packs") && method === "GET") {
      return new Response(
        JSON.stringify({ ok: true, requestId: "req-hp-list", data: { packs } }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    }

    if (url.includes("/api/v1/cognitive/heritage-packs") && method === "POST") {
      const req = JSON.parse(String(init?.body ?? "{}")) as { name: string; entryMode: HeritageDesignPack["entryMode"]; themeModes: string[] };
      const created: HeritageDesignPack = {
        ...SAMPLE_PACK,
        id: `pack-${Date.now()}`,
        name: req.name,
        entryMode: req.entryMode,
        themeModes: req.themeModes,
      };
      packs = [created, ...packs];
      return new Response(
        JSON.stringify({ ok: true, requestId: "req-hp-post", data: { pack: created, filePath: `/tmp/${created.id}.json` } }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    }

    return new Response(JSON.stringify({ ok: true }), { status: 200 });
  });
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("Story 05.6 — Brand Visual Studio, Dynamic themeModes, Gallery, Hidden Spec OCC & Heritage Pack", () => {
  it("hides dynamic Light/Dark switcher when user requests single mode ['warm'] and sandboxes Sketch iframes", async () => {
    mockStudioFetch();
    render(<StudioHarness initialModes={["warm"]} autoOpenStudio />);

    await waitFor(() => expect(screen.getByTestId("brand-visual-studio-modal")).toBeTruthy());
    expect(screen.queryByTestId("dynamic-theme-mode-switcher")).toBeNull();

    const iframe = await screen.findByTestId("sketch-preview-iframe-sketch-hero");
    expect(iframe.getAttribute("sandbox")).toBe("");
    expect(iframe.getAttribute("srcdoc")).toContain('data-brand-mode="warm"');
  });

  it("renders exact user multi-mode ['warm','dim'] pills, Shared Component Gallery, Popular Audit, and Heritage Pack save/apply", async () => {
    mockStudioFetch();
    render(<StudioHarness initialModes={["warm", "dim"]} autoOpenStudio />);

    const switcher = await screen.findByTestId("dynamic-theme-mode-switcher");
    expect(switcher).toBeTruthy();
    expect(screen.getByTestId("studio-mode-switch-warm")).toBeTruthy();
    expect(screen.getByTestId("studio-mode-switch-dim")).toBeTruthy();
    expect(screen.queryByTestId("studio-mode-switch-light")).toBeNull();
    expect(screen.queryByTestId("studio-mode-switch-dark")).toBeNull();

    // Switch to Shared Component Gallery
    fireEvent.click(screen.getByTestId("studio-tab-component-gallery"));
    await waitFor(() => expect(screen.getByTestId("gallery-card-button")).toBeTruthy());

    // Switch to Popular Component Audit and toggle Toast
    fireEvent.click(screen.getByTestId("studio-tab-popular-audit"));
    await waitFor(() => expect(screen.getByTestId("audit-row-toast")).toBeTruthy());
    fireEvent.click(screen.getByTestId("audit-filter-suggested_contextual"));
    expect(screen.queryByTestId("audit-row-button")).toBeNull();
    expect(screen.getByTestId("audit-row-toast")).toBeTruthy();

    // Open Heritage Pack Picker, save new pack with DECK_FROM_APP, and apply existing pack
    fireEvent.click(screen.getByTestId("toggle-heritage-picker-btn"));
    fireEvent.change(screen.getByTestId("heritage-entry-mode-select"), { target: { value: "DECK_FROM_APP" } });
    fireEvent.change(screen.getByTestId("heritage-pack-name-input"), { target: { value: "Exec Deck Pack" } });
    fireEvent.click(screen.getByTestId("save-heritage-pack-btn"));
    await waitFor(() => expect(screen.getByText("Exec Deck Pack")).toBeTruthy());
  });

  it("shows OCC 409 STALE_MAPPING conflict banner in Hidden Spec Drawer and displays FE Data Contract boundary + draft.ts", async () => {
    mockStudioFetch({ conflictOnPut: true });
    render(<StudioHarness initialModes={["warm"]} autoOpenDrawer />);

    await waitFor(() => expect(screen.getByTestId("hidden-spec-drawer")).toBeTruthy());
    await waitFor(() => expect(screen.getByTestId("hidden-spec-markdown-editor")).toBeTruthy());

    fireEvent.click(screen.getByTestId("save-hidden-spec-btn"));
    await waitFor(() => expect(screen.getByTestId("hidden-spec-occ-conflict-banner")).toBeTruthy());
    expect(screen.getByTestId("hidden-spec-occ-conflict-banner").textContent).toContain("STALE_MAPPING");

    // Reload clears conflict banner
    fireEvent.click(screen.getByTestId("reload-hidden-spec-btn"));
    await waitFor(() => expect(screen.queryByTestId("hidden-spec-occ-conflict-banner")).toBeNull());

    // Switch to FE Data Contract tab & verify Icon Buttons (Copy + Open Specs Folder)
    fireEvent.click(screen.getByTestId("drawer-tab-fe-contract"));
    await waitFor(() => expect(screen.getByTestId("fe-contract-boundary-banner")).toBeTruthy());
    expect(screen.getByTestId("fe-contract-boundary-banner").textContent).toContain(
      "FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY"
    );
    expect(screen.getByTestId("fe-contract-ts-preview").textContent).toContain("ScreenViewModel");
    expect(screen.getByTestId("open-fe-contract-folder-btn")).toBeTruthy();
    expect(screen.getByTestId("copy-fe-contract-btn")).toBeTruthy();
    fireEvent.click(screen.getByTestId("open-fe-contract-folder-btn"));
  });

  it("mounts StudioDrawerShell [5.26] with 5 viewports, AgentChatComposer, merged select/region mode & comment-to-chat sync", async () => {
    mockStudioFetch();
    const onSync = vi.fn();
    const SyncHarness: React.FC = () => {
      const controller = useBrandStudioController({
        projectId: PROJECT_ID,
        screenId: "scr-1",
        locale: "vi",
        initialThemeModes: ["warm"],
        onSyncToMainChat: onSync,
      });
      return (
        <div>
          <button type="button" data-testid="open-sync-studio" onClick={controller.openStudio}>Open</button>
          <BrandVisualStudioModal controller={controller} />
        </div>
      );
    };
    render(<SyncHarness />);
    fireEvent.click(screen.getByTestId("open-sync-studio"));

    await waitFor(() => expect(screen.getByTestId("studio-right-drawer")).toBeTruthy());
    expect(screen.getByTestId("studio-toolbar-tier1")).toBeTruthy();
    expect(screen.getByTestId("studio-toolbar-tier2")).toBeTruthy();
    expect(screen.getByTestId("studio-toolbar-tier3")).toBeTruthy();
    expect(screen.getByTestId("studio-viewport-fold-inner-712")).toBeTruthy();
    expect(screen.getByTestId("studio-viewport-fold-outer-344")).toBeTruthy();
    expect(screen.getByTestId("studio-pointer-mode-toggle")).toBeTruthy();
    expect(screen.getByTestId("studio-subchat-panel")).toBeTruthy();
    expect(screen.getByTestId("studio-subchat-sync-toggle")).toBeTruthy();
    expect(screen.getByTestId("studio-subchat-attach-btn")).toBeTruthy();

    // Sub-Chat command updates proposal style & syncs back to active Main Chat
    const input = screen.getByTestId("studio-subchat-input") as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: "Chuyển bo góc mềm và mật độ gọn" } });
    fireEvent.click(screen.getByTestId("studio-subchat-send-btn"));
    await waitFor(() => expect(onSync).toHaveBeenCalledWith(expect.stringContaining("[Brand Studio]")));

    // Right-click on preview page opens canonical StudioSpatialActionBar + inline comment renders in Sub-Chat & syncs Main Chat
    const firstCard = screen.getByTestId("sketch-pages-preview-grid").firstElementChild!;
    fireEvent.contextMenu(firstCard);
    await waitFor(() => expect(screen.getByTestId("studio-spatial-action-bar")).toBeTruthy());
    fireEvent.click(screen.getByTestId("studio-spatial-action-comment"));
    const commentInput = screen.getByTestId("studio-spatial-comment-input") as HTMLInputElement;
    fireEvent.change(commentInput, { target: { value: "Tăng độ tương phản tiêu đề chính" } });
    fireEvent.click(screen.getByTestId("studio-spatial-comment-submit"));
    await waitFor(() => expect(screen.getByTestId("studio-subchat-comment-item").textContent).toContain("Tăng độ tương phản tiêu đề chính"));
    expect(onSync).toHaveBeenCalledWith(expect.stringContaining("[Brand Studio · Ghim #1"));
  });

  it("hides unchecked detected component in SharedComponentGallery, wires referenceApps + toast + onApplyBrandTokens, and closes on Escape", async () => {
    const fetchSpy = mockStudioFetch();
    const onApply = vi.fn();
    render(<StudioHarness initialModes={["warm"]} autoOpenStudio onApplyBrandTokens={onApply} />);

    await waitFor(() => expect(screen.getByTestId("brand-visual-studio-modal")).toBeTruthy());

    // Reference apps input + Regenerate sends parsed referenceApps
    const refInput = screen.getByTestId("brand-studio-reference-apps-input") as HTMLInputElement;
    fireEvent.change(refInput, { target: { value: "Figma, Linear, Raycast" } });
    fireEvent.click(screen.getByTestId("brand-studio-regenerate-btn"));
    await waitFor(() => {
      const calls = fetchSpy.mock.calls.filter(([u]) => String(u).includes("/api/v1/cognitive/brand-proposals"));
      const lastBody = JSON.parse(String(calls[calls.length - 1]?.[1]?.body ?? "{}")) as { referenceApps?: string[] };
      expect(lastBody.referenceApps).toEqual(["Figma", "Linear", "Raycast"]);
    });

    // Unchecking detected 'button' in Popular Audit hides it in Shared Component Gallery
    fireEvent.click(screen.getByTestId("studio-tab-popular-audit"));
    await waitFor(() => expect(screen.getByTestId("audit-toggle-button")).toBeTruthy());
    fireEvent.click(screen.getByTestId("audit-toggle-button"));
    fireEvent.click(screen.getByTestId("studio-tab-component-gallery"));
    await waitFor(() => expect(screen.queryByTestId("gallery-card-button")).toBeNull());
    expect(screen.getByTestId("shared-component-gallery-empty")).toBeTruthy();

    // Apply Brand ID invokes onApplyBrandTokens(proposal, activeModeId) and displays Toast receipt
    fireEvent.click(screen.getByTestId("apply-brand-id-btn"));
    expect(onApply).toHaveBeenCalledTimes(1);
    expect(onApply.mock.calls[0]?.[1]).toBe("warm");
    await waitFor(() => expect(screen.getByTestId("brand-studio-toast")).toBeTruthy());
    expect(screen.getByTestId("brand-studio-toast").textContent).toContain("Precision Atelier");

    // Re-open studio and dismiss with Escape key
    fireEvent.click(screen.getByTestId("trigger-studio"));
    await waitFor(() => expect(screen.getByTestId("brand-visual-studio-modal")).toBeTruthy());
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByTestId("brand-visual-studio-modal")).toBeNull());

    // Open Hidden Spec Drawer and dismiss with Escape key
    fireEvent.click(screen.getByTestId("trigger-drawer"));
    await waitFor(() => expect(screen.getByTestId("hidden-spec-drawer")).toBeTruthy());
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByTestId("hidden-spec-drawer")).toBeNull());
  });
});
