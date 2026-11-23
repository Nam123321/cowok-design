// @story 09.1
// @implements AC-01, AC-02, AC-03, AC-04 (Chat-at-Center Launchpad Live Two-Store /rea Search, Auto-Ingest & Advanced Proof UI Tests)
// @vitest-environment jsdom
import React from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as reaApi from "@/api/rea";
import { ProjectLaunchpadModal } from "@/features/flow/components/ProjectLaunchpadModal";
import {
  ReaCatalogResponseSchema,
  ReaExportProofResponseSchema,
  ReaIngestResponseSchema,
  ReaSearchResponseSchema,
} from "@/lib/validations/rea";

const SAMPLE_SHA = "a".repeat(64);
const SAMPLE_TS = 1_730_600_000_000;
const HTTP_UNPROCESSABLE = 422;

const SAMPLE_CATALOG_APPS = [
  {
    slug: "photocraft",
    name: "PhotoCraft",
    repoUrl: "https://github.com/photopea/photopea",
    category: "Design",
    subcategory: "Image Editor",
    stars: 79131,
    licenseSpdx: "MIT",
    firewallVerdict: "ADOPT_PRIOR_ART" as const,
    lang: "TypeScript",
    description: "Browser-based raster & vector canvas studio with layers and inspector.",
    topics: ["design", "canvas"],
    layoutArchetype: "canvas-editor" as const,
  },
  {
    slug: "figcraft",
    name: "FigCraft",
    repoUrl: "https://github.com/example/figcraft",
    category: "Design",
    subcategory: "Vector Design",
    stars: 4906,
    licenseSpdx: "MIT",
    firewallVerdict: "ADOPT_PRIOR_ART" as const,
    lang: "TypeScript",
    description: "Collaborative vector UI design tool inspired by Figma.",
    topics: ["figma", "design", "canvas"],
    layoutArchetype: "canvas-editor" as const,
  },
];

const SAMPLE_HIT = {
  id: "ref-091-1",
  tenantId: "tenant-091",
  slug: "fintech-design-kit",
  sourceUrl: "https://github.com/open-design/fintech-kit",
  sourceFormat: "github" as const,
  licenseSpdx: "MIT",
  firewallVerdict: "ADOPT_PRIOR_ART" as const,
  firewallReason: "Permissive open-source license cleared for ADOPT_PRIOR_ART",
  tier1aSummary: "Clean multi-sig fintech layout with semantic CSS tokens.",
  tier1aTokens: 9,
  proofStatus: "awaiting-verification" as const,
  sanitizedSecretsCount: 2,
  createdAt: SAMPLE_TS,
};

describe("Story 09.1 Chat-at-Center Launchpad UI & Two-Store /rea Contracts", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("UT-09.1-01: supports Local Folder/File quick-start & live Top-5 Awesome-RE visual gallery in Templates tab", async () => {
    const catalogSpy = vi.spyOn(reaApi, "searchAwesomeReCatalog").mockResolvedValue(
      ReaCatalogResponseSchema.parse({
        ok: true,
        requestId: "req-catalog-1",
        data: {
          upstreamRepo: "alexgaoth/awesome-reverse-engineered",
          upstreamCommit: "80f551625282",
          totalCatalogEntries: 250,
          query: "",
          sanitizedFtsQuery: "",
          total: SAMPLE_CATALOG_APPS.length,
          items: SAMPLE_CATALOG_APPS,
        },
      })
    );
    const searchSpy = vi.spyOn(reaApi, "searchReaReferences").mockResolvedValue(
      ReaSearchResponseSchema.parse({
        ok: true,
        requestId: "req-search-1",
        data: {
          tier: "1a",
          query: "",
          sanitizedFtsQuery: "",
          maxTier1aTokens: 250,
          total: 1,
          items: [SAMPLE_HIT],
        },
      })
    );
    const onLaunchPath = vi.fn();

    render(
      <ProjectLaunchpadModal isOpen={true} onClose={vi.fn()} onLaunchPath={onLaunchPath} />
    );

    // Default tab is Local Disk (folder + file)
    expect(screen.getByTestId("launchpad-local-folder-input")).toBeTruthy();
    expect(screen.getByTestId("launchpad-pick-folder-btn")).toBeTruthy();
    expect(screen.getByTestId("launchpad-prd-dropzone")).toBeTruthy();

    fireEvent.click(screen.getByTestId("launchpad-tab-templates"));
    await waitFor(() => {
      expect(screen.getByTestId("launchpad-awesome-re-figcraft")).toBeTruthy();
      expect(screen.getByTestId("launchpad-rea-hit-fintech-design-kit")).toBeTruthy();
    });
    expect(catalogSpy).toHaveBeenCalledWith({ q: "", limit: 5 });
    expect(searchSpy).toHaveBeenCalledWith({ q: "", limit: 4, tier: "1a" });
    expect(screen.getAllByTestId("awesome-re-visual-canvas-editor").length).toBeGreaterThanOrEqual(1);

    fireEvent.click(screen.getByTestId("launchpad-awesome-re-figcraft"));
    fireEvent.click(screen.getByTestId("launchpad-submit-btn"));
    expect(onLaunchPath).toHaveBeenCalledWith(
      "templates",
      expect.objectContaining({
        templateId: "awesome-re:figcraft",
        catalogApp: expect.objectContaining({ slug: "figcraft", layoutArchetype: "canvas-editor" }),
      })
    );
  });

  it("UT-09.1-02: hides technical controls by default, auto-ingests in 1 click, and verifies Honest Proof under Advanced", async () => {
    const ingestSpy = vi.spyOn(reaApi, "ingestReaReference").mockResolvedValue(
      ReaIngestResponseSchema.parse({
        ok: true,
        requestId: "req-ingest-1",
        data: {
          record: {
            ...SAMPLE_HIT,
            designDnaJson: JSON.stringify({ kind: "web-clone-l1-l6", layers: { l1Palette: ["#0f766e"] } }),
            rawFieldsJson: null,
          },
          firewall: {
            verdict: "ADOPT_PRIOR_ART",
            normalizedLicense: "MIT",
            reason: "Permissive open-source license cleared for ADOPT_PRIOR_ART",
            policyRule: "LICENSE_PERMISSIVE_ADOPT",
          },
          strippedCategories: ["api_key", "sql_statement"],
          extractedFiles: ["web-clone-dna.json"],
        },
      })
    );

    const proofSpy = vi
      .spyOn(reaApi, "exportReaFigmaProof")
      .mockResolvedValueOnce(
        ReaExportProofResponseSchema.parse({
          ok: true,
          requestId: "req-proof-1",
          data: {
            referenceId: "ref-091-1",
            slug: "fintech-design-kit",
            proofStatus: "awaiting-verification",
            verified: false,
            checksumMatched: false,
            computedSha256: SAMPLE_SHA,
            proofPayloadJson: "{}",
            verificationNote: "Awaiting roundtrip checksum",
          },
        })
      )
      .mockResolvedValueOnce(
        ReaExportProofResponseSchema.parse({
          ok: true,
          requestId: "req-proof-2",
          data: {
            referenceId: "ref-091-1",
            slug: "fintech-design-kit",
            proofStatus: "proof-verified",
            verified: true,
            checksumMatched: true,
            computedSha256: SAMPLE_SHA,
            proofPayloadJson: "{}",
            verificationNote: "Verified",
          },
        })
      );

    render(<ProjectLaunchpadModal isOpen={true} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTestId("launchpad-tab-design"));

    // Clean Chat-at-Center default: advanced jargon panel is collapsed by default
    expect(screen.queryByTestId("launchpad-rea-advanced-panel")).toBeNull();
    expect(screen.getByTestId("launchpad-design-tool-figma")).toBeTruthy();
    expect(screen.getByTestId("launchpad-design-tool-framer")).toBeTruthy();

    fireEvent.change(screen.getByTestId("launchpad-figma-url-input"), {
      target: { value: "https://github.com/open-design/fintech-kit" },
    });
    fireEvent.click(screen.getByTestId("launchpad-rea-ingest-btn"));

    await waitFor(() => {
      expect(screen.getByTestId("launchpad-rea-ingest-result")).toBeTruthy();
    });
    expect(ingestSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        slug: "fintech-kit",
        sourceUrl: "https://github.com/open-design/fintech-kit",
      })
    );
    // Expand Advanced options for Honest Figma Proof roundtrip checksum verification
    fireEvent.click(screen.getByTestId("launchpad-extract-tokens-btn"));
    expect(screen.getByTestId("launchpad-extracted-tokens-preview").textContent).toContain("web-clone-l1-l6");
    fireEvent.click(screen.getByTestId("launchpad-rea-advanced-toggle"));
    expect(screen.getByTestId("rea-proof-badge-awaiting-verification")).toBeTruthy();
    fireEvent.click(screen.getByTestId("launchpad-rea-export-proof-btn"));
    await waitFor(() => {
      expect(screen.getByTestId("launchpad-rea-verify-proof-btn")).toBeTruthy();
    });

    const shaInput = screen.getByTestId("launchpad-rea-roundtrip-sha-input");
    const verifyBtn = screen.getByTestId("launchpad-rea-verify-proof-btn") as HTMLButtonElement;
    expect(verifyBtn.disabled).toBe(true);
    fireEvent.change(shaInput, { target: { value: SAMPLE_SHA } });
    expect(verifyBtn.disabled).toBe(false);

    fireEvent.click(verifyBtn);
    await waitFor(() => {
      expect(screen.getByTestId("rea-proof-badge-proof-verified")).toBeTruthy();
    });
    expect(proofSpy).toHaveBeenLastCalledWith({
      referenceId: "ref-091-1",
      roundtripChecksum: SAMPLE_SHA,
    });
  });

  it("UT-09.1-03: renders localized REJECT firewall alert without closing modal", async () => {
    vi.spyOn(reaApi, "ingestReaReference").mockRejectedValue(
      new reaApi.ReaApiError(
        HTTP_UNPROCESSABLE,
        "FIREWALL_REJECTED",
        "Strong copyleft GPL license is rejected by Design-Only Firewall",
        "licenseSpdx"
      )
    );
    const onClose = vi.fn();
    render(<ProjectLaunchpadModal isOpen={true} onClose={onClose} />);

    fireEvent.click(screen.getByTestId("launchpad-tab-design"));
    fireEvent.click(screen.getByTestId("launchpad-rea-advanced-toggle"));
    fireEvent.change(screen.getByTestId("launchpad-rea-license-select"), {
      target: { value: "GPL-3.0" },
    });
    fireEvent.click(screen.getByTestId("launchpad-rea-ingest-btn"));

    await waitFor(() => {
      expect(screen.getByTestId("launchpad-rea-reject-alert")).toBeTruthy();
    });
    expect(screen.getByTestId("launchpad-rea-reject-alert").textContent).toContain("FIREWALL_REJECTED");
    expect(onClose).not.toHaveBeenCalled();
  });

  it("UT-09.1-04: rejects >1.4 MB local uploads on Tab 1 and keeps Tab 2 free of duplicate upload button", async () => {
    render(<ProjectLaunchpadModal isOpen={true} onClose={vi.fn()} />);

    // Tab 1 (Local Disk) handles local file upload + size cap without idea textarea clutter
    expect(screen.queryByTestId("launchpad-prd-textarea")).toBeNull();
    expect(screen.queryByTestId("launchpad-load-sample-prd-btn")).toBeNull();
    const localInput = screen.getByTestId("launchpad-prd-file-input");

    const hugeFile = new File(["x"], "huge-kit.zip", { type: "application/zip" });
    Object.defineProperty(hugeFile, "size", { value: 2_000_000 });
    fireEvent.change(localInput, { target: { files: [hugeFile] } });
    expect(screen.getByTestId("launchpad-local-upload-error")).toBeTruthy();

    // Tab 2 (Online Design Tools) has NO duplicate local file upload input
    fireEvent.click(screen.getByTestId("launchpad-tab-design"));
    expect(screen.queryByTestId("launchpad-design-file-input")).toBeNull();

    // Tab 4 (Reference Library) exposes live Awesome-RE Top-5 visual search & topic chips
    vi.spyOn(reaApi, "searchAwesomeReCatalog").mockResolvedValue(
      ReaCatalogResponseSchema.parse({
        ok: true,
        requestId: "req-catalog-4",
        data: {
          upstreamRepo: "alexgaoth/awesome-reverse-engineered",
          upstreamCommit: "80f551625282",
          totalCatalogEntries: 250,
          query: "",
          sanitizedFtsQuery: "",
          total: SAMPLE_CATALOG_APPS.length,
          items: SAMPLE_CATALOG_APPS,
        },
      })
    );
    fireEvent.click(screen.getByTestId("launchpad-tab-templates"));
    await waitFor(() => {
      expect(screen.getByTestId("launchpad-awesome-re-photocraft")).toBeTruthy();
      expect(screen.getByTestId("launchpad-awesome-re-figcraft")).toBeTruthy();
    });
    expect(screen.getByTestId("launchpad-rea-topic-design")).toBeTruthy();
  });
});
