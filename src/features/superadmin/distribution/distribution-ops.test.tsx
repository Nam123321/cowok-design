// @vitest-environment jsdom
// @story 14.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (SuperAdmin Distribution Ops UI & SDK Tests — Plugin Catalog, A/B Experiments, Canary Rollout, Global Quarantine, Offline Licenses, Governance Audit & RBAC Locks)
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  inspectOfflineLicenseToken,
  type DistributionOpsApiPort,
} from "../../../api/distribution-ops";
import {
  BulkUpdateCatalogRequestSchema,
  PublishRolloutRequestSchema,
  UpsertCatalogPluginRequestSchema,
  UpsertExperimentRequestSchema,
} from "../../../lib/validations/distribution-ops";
import { SuperAdminPortalApp } from "../../../portals/superadmin/SuperAdminPortalApp";
import { findNavSection } from "../../../portals/superadmin/navigation";
import { setGlobalLocale } from "../../../hooks/useTranslation";
import {
  resolveLocalizedSummary,
  resolveLocalizedTitle,
} from "./PluginCatalogDetailDrawer";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import type { SuperAdminApiPort } from "../useSuperAdminController";

function toBase64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 1) {
    bin += String.fromCharCode(bytes[i]);
  }
  return btoa(bin);
}

function toBase64Url(bytes: Uint8Array): string {
  return toBase64(bytes).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(",")}]`;
  }
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, v]) => v !== undefined)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${canonicalJson(v)}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

async function buildSignedOfflineToken(
  overrides: { expiresOffsetSec?: number } = {}
): Promise<{ token: string; publicKeyBase64: string; now: number }> {
  const keyPair = (await globalThis.crypto.subtle.generateKey(
    { name: "Ed25519" },
    true,
    ["sign", "verify"]
  )) as CryptoKeyPair;
  const rawPk = new Uint8Array(await globalThis.crypto.subtle.exportKey("raw", keyPair.publicKey));
  const publicKeyBase64 = toBase64(rawPk);
  const now = Math.floor(Date.now() / 1000);
  const expiresAt = now + (overrides.expiresOffsetSec ?? 86_400);
  const payload = {
    licenseId: "lic_ent_01",
    tier: "ENTERPRISE" as const,
    subjectOrg: "Cowok Studio VN",
    cowokOrgId: "org_cowok_vn_01",
    maxSeats: 25,
    features: ["desktop-pro", "offline-airgap"],
    issuedAt: now,
    expiresAt,
  };
  const payloadCanonicalJson = canonicalJson(payload);
  const signingInput = `v1:cowok-ed25519-v1:license:1:${now}:${expiresAt}:${payloadCanonicalJson}`;
  const sigBuf = await globalThis.crypto.subtle.sign(
    "Ed25519",
    keyPair.privateKey,
    new TextEncoder().encode(signingInput)
  );
  const envelope = {
    schemaVersion: 1,
    manifestKind: "license",
    manifestSeq: 1,
    issuedAt: now,
    expiresAt,
    keyId: "cowok-ed25519-v1",
    publicKeyBase64,
    payloadCanonicalJson,
    signatureBase64: toBase64(new Uint8Array(sigBuf)),
    payload,
  };
  const token = `cwk_lic_v1.${toBase64Url(new TextEncoder().encode(JSON.stringify(envelope)))}`;
  return { token, publicKeyBase64, now };
}

function makeSignedEnvelope<T>(kind: string, seq: number, payload: T) {
  const now = Math.floor(Date.now() / 1000);
  return {
    schemaVersion: 1,
    manifestKind: kind,
    manifestSeq: seq,
    issuedAt: now,
    expiresAt: now + 86_400,
    keyId: "cowok-ed25519-v1",
    publicKeyBase64: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
    payloadCanonicalJson: JSON.stringify(payload),
    signatureBase64: "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC==",
    payload,
  };
}

function createMockAdminClient(): SuperAdminApiPort {
  return {
    createAdminSession: vi.fn(async (payload) => ({
      ok: true as const,
      requestId: "req-auth-143",
      data: {
        sessionToken: `cwk_adm_v1.mock.${payload.role}`,
        expiresAt: Math.floor(Date.now() / 1000) + 3600,
        operator: {
          subject: payload.subject,
          email: payload.email,
          role: payload.role as CloudAdminRole,
          mfaVerified: payload.mfaVerified,
          oidcProvider: payload.oidcProvider,
        },
      },
    })),
    listTickets: vi.fn(async () => ({
      ok: true as const,
      requestId: "req-tkt",
      data: { tickets: [], totalCount: 0 },
    })),
    replyToTicket: vi.fn(),
    listClusters: vi.fn(async () => ({
      ok: true as const,
      requestId: "req-clu",
      data: { clusters: [] },
    })),
    createCluster: vi.fn(),
    getAnalyticsFunnel: vi.fn(async () => ({
      ok: true as const,
      requestId: "req-fun",
      data: {
        dau: 42,
        wau: 180,
        mau: 600,
        openTickets: 0,
        p0Tickets: 0,
        activeCrashGroups: 0,
        channelBreakdown: [],
        hostBreakdown: [],
        funnelSteps: [],
        featureAdoption: [],
      },
    })),
    listCrashes: vi.fn(async () => ({
      ok: true as const,
      requestId: "req-cr",
      data: { crashes: [] },
    })),
  };
}

function createMockDistributionClient(
  signedLicenseFixture: { token: string; publicKeyBase64: string } = {
    token: ["demo", "license", "stub"].join("-"),
    publicKeyBase64: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
  }
): DistributionOpsApiPort {
  const { token, publicKeyBase64 } = signedLicenseFixture;
  const seedPlugin = {
    pluginId: "html-forge-core",
    sourceKind: "DEFAULT_BUILTIN" as const,
    lifecycleStatus: "ACTIVE" as const,
    repoUrl: null,
    pinnedCommitSha: null,
    minAppVersion: "0.1.0",
    defaultEnabled: true,
    defaultConfig: {},
    titleI18n: { vi: "Lõi Dựng HTML Forge", en: "HTML Forge Core" },
    summaryI18n: {},
    manifestSeq: 1,
    updatedBy: "system:seed",
    updatedAt: 1_732_400_000_000,
  };
  const seedExp = {
    experimentKey: "onboarding_pack_v1",
    title: "Onboarding Starter Pack",
    targetSurface: "ONBOARDING_DEFAULTS" as const,
    status: "RUNNING" as const,
    variants: [
      { key: "control", weight: 50, config: {} },
      { key: "treatment", weight: 50, config: {} },
    ],
    killSwitch: false,
    manifestSeq: 1,
    updatedBy: "ops-admin@cowok.design",
    createdAt: 1_732_400_000_000,
    updatedAt: 1_732_400_000_000,
    attribution: [
      { variantKey: "control", installs: 100, funnelDay0: 100, funnelExport: 32, conversionRatePct: 32, crashEvents: 1, crashRatePct: 1 },
      { variantKey: "treatment", installs: 100, funnelDay0: 100, funnelExport: 45, conversionRatePct: 45, crashEvents: 0, crashRatePct: 0 },
    ],
  };
  const seedRollout = {
    id: "rel_01",
    channel: "desktop" as const,
    version: "1.4.0",
    versionSeq: 1,
    rolloutPercent: 25,
    killSwitch: false,
    announcement: {},
    downloadUrl: "https://releases.cowok.design/desktop/1.4.0",
    artifactSha256: "a".repeat(64),
    notes: "Wave 1",
    publishedBy: "ops-admin@cowok.design",
    createdAt: 1_732_400_000_000,
    updatedAt: 1_732_400_000_000,
  };
  const latestPayload = {
    channel: "desktop" as const,
    version: "1.4.0",
    versionSeq: 1,
    rolloutPercent: 25,
    killSwitch: false,
    canaryEligible: true,
    downloadUrl: seedRollout.downloadUrl,
    artifactSha256: seedRollout.artifactSha256,
    announcement: {},
    notes: "Wave 1",
    channels: [seedRollout],
  };

  return {
    listRollouts: vi.fn(async () => ({
      ok: true as const,
      requestId: "r-rel",
      data: {
        rollouts: [seedRollout],
        latestSignedManifest: makeSignedEnvelope("latest", 1, latestPayload),
      },
    })),
    publishRollout: vi.fn(async (input) => {
      const rollout = {
        ...seedRollout,
        id: "rel_02",
        channel: input.channel,
        version: input.version,
        versionSeq: 2,
        rolloutPercent: input.rolloutPercent,
        killSwitch: Boolean(input.killSwitch),
      };
      return {
        ok: true as const,
        requestId: "r-rel-post",
        data: {
          rollout,
          signedManifest: makeSignedEnvelope("latest", 2, { ...latestPayload, version: input.version, versionSeq: 2 }),
          auditLogId: "aud_rel_2",
        },
      };
    }),
    listCatalog: vi.fn(async () => ({
      ok: true as const,
      requestId: "r-cat",
      data: {
        plugins: [seedPlugin],
        signedManifest: makeSignedEnvelope("catalog", 1, { plugins: [seedPlugin] }),
      },
    })),
    upsertCatalogPlugin: vi.fn(async (input) => {
      const plugin = {
        ...seedPlugin,
        pluginId: input.pluginId,
        sourceKind: input.sourceKind,
        lifecycleStatus: input.lifecycleStatus,
        repoUrl: input.repoUrl ?? null,
        pinnedCommitSha: input.pinnedCommitSha ?? null,
        titleI18n: input.titleI18n ?? seedPlugin.titleI18n,
        summaryI18n: input.summaryI18n ?? seedPlugin.summaryI18n,
        manifestSeq: 2,
      };
      return {
        ok: true as const,
        requestId: "r-cat-post",
        data: {
          plugin,
          signedManifest: makeSignedEnvelope("catalog", 2, { plugins: [seedPlugin, plugin] }),
          auditLogId: "aud_cat_2",
        },
      };
    }),
    bulkUpdateCatalog: vi.fn(async (input) => {
      const updated = {
        ...seedPlugin,
        lifecycleStatus: input.lifecycleStatus,
        manifestSeq: 3,
      };
      return {
        ok: true as const,
        requestId: "r-cat-bulk",
        data: {
          updatedIds: input.pluginIds,
          updatedCount: input.pluginIds.length,
          lifecycleStatus: input.lifecycleStatus,
          plugins: [updated],
          signedManifest: makeSignedEnvelope("catalog", 3, { plugins: [updated] }),
          auditLogId: "aud_cat_bulk_3",
        },
      };
    }),
    listExperiments: vi.fn(async () => ({
      ok: true as const,
      requestId: "r-exp",
      data: {
        experiments: [seedExp],
        signedManifest: makeSignedEnvelope("experiments", 1, { experiments: [seedExp] }),
      },
    })),
    upsertExperiment: vi.fn(async (input) => {
      const exp = {
        ...seedExp,
        experimentKey: input.experimentKey,
        title: input.title,
        targetSurface: input.targetSurface,
        status: input.killSwitch ? ("KILLED" as const) : input.status,
        killSwitch: Boolean(input.killSwitch),
        manifestSeq: 2,
      };
      return {
        ok: true as const,
        requestId: "r-exp-post",
        data: {
          experiment: exp,
          signedManifest: makeSignedEnvelope("experiments", 2, { experiments: [exp] }),
          auditLogId: "aud_exp_2",
        },
      };
    }),
    listQuarantines: vi.fn(async () => ({
      ok: true as const,
      requestId: "r-q",
      data: {
        advisories: [],
        signedManifest: makeSignedEnvelope("advisories", 1, { advisories: [] }),
      },
    })),
    issueQuarantine: vi.fn(async (input) => {
      const advisory = {
        id: "adv_01",
        skillOrPluginId: input.skillOrPluginId,
        severity: input.severity,
        action: input.action ?? ("QUARANTINE" as const),
        reason: input.reason,
        affectedSha256: input.affectedSha256 ?? null,
        advisorySeq: 2,
        issuedBy: "ops-admin@cowok.design",
        createdAt: 1_732_400_000_000,
      };
      return {
        ok: true as const,
        requestId: "r-q-post",
        data: {
          advisory,
          signedManifest: makeSignedEnvelope("advisories", 2, { advisories: [advisory] }),
          auditLogId: "aud_q_2",
        },
      };
    }),
    listLicenses: vi.fn(async () => ({
      ok: true as const,
      requestId: "r-lic",
      data: {
        licenses: [
          {
            licenseId: "lic_ent_01",
            tier: "ENTERPRISE" as const,
            subjectOrg: "Cowok Studio VN",
            cowokOrgId: "org_cowok_vn_01",
            maxSeats: 25,
            issuedAt: Math.floor(Date.now() / 1000),
            expiresAt: Math.floor(Date.now() / 1000) + 86_400,
            signedToken: token,
            revoked: false,
            issuedBy: "ops-admin@cowok.design",
            offlineVerified: true,
          },
        ],
        keyId: "cowok-ed25519-v1",
        publicKeyBase64,
      },
    })),
    issueLicense: vi.fn(async (input) => {
      const now = Math.floor(Date.now() / 1000);
      const license = {
        licenseId: "lic_pro_02",
        tier: input.tier,
        subjectOrg: input.subjectOrg,
        cowokOrgId: input.cowokOrgId ?? null,
        maxSeats: input.maxSeats ?? 1,
        issuedAt: now,
        expiresAt: now + 86_400,
        signedToken: token,
        revoked: false,
        issuedBy: "ops-admin@cowok.design",
        offlineVerified: true,
      };
      return {
        ok: true as const,
        requestId: "r-lic-post",
        data: {
          license,
          verification: {
            payload: {
              licenseId: license.licenseId,
              tier: license.tier,
              subjectOrg: license.subjectOrg,
              cowokOrgId: license.cowokOrgId,
              maxSeats: license.maxSeats,
              features: ["desktop-pro"],
              issuedAt: now,
              expiresAt: now + 86_400,
            },
            keyId: "cowok-ed25519-v1",
            publicKeyBase64: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            offlineVerified: true,
          },
          auditLogId: "aud_lic_2",
        },
      };
    }),
    listAuditLogs: vi.fn(async () => ({
      ok: true as const,
      requestId: "r-aud",
      data: {
        logs: [
          {
            id: "aud_01",
            actorSubject: "oidc:ops-admin@cowok.design",
            actorRole: "SUPER_ADMIN",
            mfaVerified: true,
            action: "RELEASE_ROLLOUT_PUBLISHED",
            targetResource: "release:desktop:1.4.0",
            details: { rolloutPercent: 25 },
            createdAt: 1_732_400_000_000,
          },
        ],
      },
    })),
  };
}

function signInAs(role: CloudAdminRole = "SUPER_ADMIN") {
  if (role !== "SUPER_ADMIN") {
    fireEvent.change(screen.getByTestId("oidc-role-select"), { target: { value: role } });
  }
  fireEvent.change(screen.getByTestId("oidc-mfa-input"), { target: { value: "314159" } });
  fireEvent.click(screen.getByTestId("oidc-signin-btn"));
}

describe("Story 14.3 Distribution Ops SuperAdmin Tabs", () => {
  afterEach(() => {
    setGlobalLocale("vi-VN");
    cleanup();
    vi.clearAllMocks();
  });

  it("operates all 6 live Distribution Ops tabs for SUPER_ADMIN", async () => {
    const fixture = await buildSignedOfflineToken();
    const adminClient = createMockAdminClient();
    const distClient = createMockDistributionClient(fixture);
    render(<SuperAdminPortalApp client={adminClient} distributionClient={distClient} />);

    signInAs("SUPER_ADMIN");
    await waitFor(() => expect(screen.getByTestId("superadmin-tab-plugin-catalog")).toBeTruthy());

    // 1. Plugin Catalog Tab: DataTableView + AdvancedFilterBar + RightDrawer Create/Detail + Bulk Action Floating Card
    fireEvent.click(screen.getByTestId("superadmin-tab-plugin-catalog"));
    expect(screen.getByTestId("superadmin-panel-plugin-catalog")).toBeTruthy();
    expect(screen.getByTestId("catalog-filter-bar")).toBeTruthy();
    expect(screen.queryByTestId("catalog-right-drawer")).toBeNull();
    await waitFor(() => expect(screen.getByTestId("catalog-row-html-forge-core")).toBeTruthy());

    // Friendly Vietnamese status badge in table (no raw ALL_CAPS visible in row)
    expect(screen.getByTestId("catalog-status-badge-html-forge-core").textContent).toBe("Đang hoạt động");

    // Polish assertions: nav fallback removes GitHub, search placeholder uses "kho nguồn", Add button has no duplicate "+"
    const catalogNav = findNavSection("plugin-catalog");
    expect(catalogNav.labelFallback).toBe("Danh mục Plugin");
    expect(catalogNav.subtitleFallback).toBe(
      "Quản lý vòng đời, đánh giá bảo mật và cấu hình mặc định cho toàn bộ hệ sinh thái plugin"
    );
    expect((screen.getByTestId("catalog-search-input") as HTMLInputElement).placeholder).toBe(
      "Tìm theo mã plugin, tên hiển thị, kho nguồn hoặc Commit SHA..."
    );
    expect(screen.getByTestId("catalog-open-create-drawer-btn").textContent?.trim()).toBe("Thêm Plugin");

    // Open Create RightDrawer (Thêm Plugin) — defaults RECOMMENDED_GITHUB to PROCESSING
    fireEvent.click(screen.getByTestId("catalog-open-create-drawer-btn"));
    expect(screen.getByTestId("catalog-right-drawer")).toBeTruthy();
    const sourceSelect = screen.getByTestId("catalog-source-kind-select") as HTMLSelectElement;
    const statusSelect = screen.getByTestId("catalog-status-select") as HTMLSelectElement;
    expect(statusSelect.value).toBe("PROCESSING");
    expect(sourceSelect.style.padding).toBe("0px 32px 0px 10px");
    expect(statusSelect.style.padding).toBe("0px 32px 0px 10px");
    expect(sourceSelect.parentElement?.querySelector(".icon-chevron-down")).toBeTruthy();
    expect(statusSelect.parentElement?.querySelector(".icon-chevron-down")).toBeTruthy();

    fireEvent.change(screen.getByTestId("catalog-plugin-id-input"), { target: { value: "figma-sync" } });
    fireEvent.change(screen.getByTestId("catalog-repo-url-input"), {
      target: { value: "https://github.com/cowok-design/figma-sync" },
    });
    fireEvent.change(screen.getByTestId("catalog-commit-sha-input"), { target: { value: "main" } });
    fireEvent.change(screen.getByTestId("catalog-reason-input"), {
      target: { value: "Verify pinned release commit" },
    });
    expect((screen.getByTestId("catalog-submit-btn") as HTMLButtonElement).disabled).toBe(true);

    fireEvent.change(screen.getByTestId("catalog-commit-sha-input"), { target: { value: "c".repeat(40) } });
    expect((screen.getByTestId("catalog-submit-btn") as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByTestId("catalog-submit-btn"));
    await waitFor(() => expect(distClient.upsertCatalogPlugin).toHaveBeenCalledTimes(1));
    expect(distClient.upsertCatalogPlugin).toHaveBeenCalledWith(
      expect.objectContaining({
        pluginId: "figma-sync",
        lifecycleStatus: "PROCESSING",
        asyncSecurityScan: true,
      }),
      expect.any(String)
    );

    // Search & filter chips + sortable header
    fireEvent.change(screen.getByTestId("catalog-search-input"), { target: { value: "figma-sync" } });
    expect(screen.queryByTestId("catalog-row-html-forge-core")).toBeNull();
    expect(screen.getByTestId("catalog-row-figma-sync")).toBeTruthy();
    fireEvent.change(screen.getByTestId("catalog-search-input"), { target: { value: "" } });
    fireEvent.click(screen.getByTestId("catalog-sort-plugin"));

    // Clicking a row opens Detail & Edit RightDrawer (not selecting checkbox)
    fireEvent.click(screen.getByTestId("catalog-row-figma-sync"));
    expect(screen.getByTestId("catalog-right-drawer")).toBeTruthy();
    expect(screen.getByTestId("catalog-async-security-pill")).toBeTruthy();
    fireEvent.click(screen.getByTestId("catalog-drawer-close-btn"));
    expect(screen.queryByTestId("catalog-right-drawer")).toBeNull();

    // Selecting row checkbox reveals Bulk Action Floating Card without opening Drawer
    fireEvent.click(screen.getByTestId("catalog-row-select-html-forge-core"));
    expect(screen.queryByTestId("catalog-right-drawer")).toBeNull();
    expect(screen.getByTestId("catalog-bulk-floating-card")).toBeTruthy();
    expect(screen.getByTestId("catalog-bulk-count").textContent).toContain("1");
    fireEvent.change(screen.getByTestId("catalog-bulk-reason-input"), {
      target: { value: "Approve selected batch after review" },
    });
    fireEvent.click(screen.getByTestId("catalog-bulk-active-btn"));
    await waitFor(() => expect(distClient.bulkUpdateCatalog).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByTestId("catalog-bulk-floating-card")).toBeNull());

    // 2. A/B Experiments Tab + Friendly Vietnamese surface label + Kill-Switch
    fireEvent.click(screen.getByTestId("superadmin-tab-ab-experiments"));
    expect(screen.getByTestId("superadmin-panel-ab-experiments")).toBeTruthy();
    const expCard = screen.getByTestId("ab-exp-card-onboarding_pack_v1");
    expect(expCard.textContent).toContain("Mặc định Khởi tạo (Onboarding)");
    fireEvent.click(screen.getByTestId("ab-kill-btn-onboarding_pack_v1"));
    await waitFor(() => expect(distClient.upsertExperiment).toHaveBeenCalledTimes(1));

    // 3. Canary Rollout Tab (artifactSha256 starts empty, requires valid 64-char hex)
    fireEvent.click(screen.getByTestId("superadmin-tab-distribution"));
    expect(screen.getByTestId("superadmin-panel-distribution")).toBeTruthy();
    expect((screen.getByTestId("canary-sha256-input") as HTMLInputElement).value).toBe("");
    expect((screen.getByTestId("canary-submit-btn") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByTestId("canary-sha256-input"), { target: { value: "d".repeat(64) } });
    fireEvent.click(screen.getByTestId("canary-ring-100"));
    fireEvent.click(screen.getByTestId("canary-submit-btn"));
    await waitFor(() => expect(distClient.publishRollout).toHaveBeenCalledTimes(1));

    // Per-row Canary Kill-Switch button triggers signed killSwitch: true rollout
    fireEvent.click(screen.getByTestId("canary-kill-btn-desktop-1.4.0"));
    await waitFor(() => expect(distClient.publishRollout).toHaveBeenCalledTimes(2));
    expect(distClient.publishRollout).toHaveBeenLastCalledWith(
      expect.objectContaining({
        channel: "desktop",
        version: "1.4.0",
        killSwitch: true,
      }),
      expect.any(String)
    );

    // 4. Global Skill Quarantine Tab
    fireEvent.click(screen.getByTestId("superadmin-tab-quarantine"));
    expect(screen.getByTestId("superadmin-panel-quarantine")).toBeTruthy();
    fireEvent.change(screen.getByTestId("quarantine-skill-id-input"), { target: { value: "bad-skill" } });
    fireEvent.change(screen.getByTestId("quarantine-reason-input"), {
      target: { value: "Detected unauthorized file access in upstream tag" },
    });
    fireEvent.click(screen.getByTestId("quarantine-submit-btn"));
    await waitFor(() => expect(distClient.issueQuarantine).toHaveBeenCalledTimes(1));

    // 5. Offline Licenses & Cowok.ai Federation Tab + real WebCrypto Ed25519 verify
    fireEvent.click(screen.getByTestId("superadmin-tab-licensing"));
    expect(screen.getByTestId("superadmin-panel-licensing")).toBeTruthy();
    fireEvent.click(screen.getByTestId("license-verify-btn-lic_ent_01"));
    await waitFor(() => {
      const preview = screen.getByTestId("license-offline-verify-preview");
      expect(preview.textContent).toContain("org_cowok_vn_01");
      expect(preview.textContent).not.toContain("TAMPERED");
    });

    // 6. Governance & Audit Logs Tab
    fireEvent.click(screen.getByTestId("superadmin-tab-governance"));
    expect(screen.getByTestId("superadmin-panel-governance")).toBeTruthy();
    expect(screen.getByTestId("superadmin-rbac-matrix")).toBeTruthy();
    expect(screen.getByTestId("audit-row-aud_01")).toBeTruthy();
  });

  it("clears rejected sensitive slices when previewRole downgrades from SUPER_ADMIN to ANALYST_READONLY", async () => {
    const fixture = await buildSignedOfflineToken();
    const adminClient = createMockAdminClient();
    const distClient = createMockDistributionClient(fixture);
    const origListLicenses = distClient.listLicenses;
    const origListAuditLogs = distClient.listAuditLogs;
    distClient.listLicenses = vi.fn(async (tok) => {
      if (!tok.includes("SUPER_ADMIN")) throw new Error("FORBIDDEN");
      return origListLicenses(tok);
    });
    distClient.listAuditLogs = vi.fn(async (tok) => {
      if (!tok.includes("SUPER_ADMIN")) throw new Error("FORBIDDEN");
      return origListAuditLogs(tok);
    });

    render(<SuperAdminPortalApp client={adminClient} distributionClient={distClient} />);
    signInAs("SUPER_ADMIN");
    await waitFor(() => expect(screen.getByTestId("superadmin-tab-governance")).toBeTruthy());

    fireEvent.click(screen.getByTestId("superadmin-tab-governance"));
    await waitFor(() => expect(screen.getByTestId("audit-row-aud_01")).toBeTruthy());

    fireEvent.change(screen.getByTestId("superadmin-role-switcher"), {
      target: { value: "ANALYST_READONLY" },
    });
    await waitFor(() => expect(screen.queryByTestId("audit-row-aud_01")).toBeNull());

    fireEvent.click(screen.getByTestId("superadmin-tab-licensing"));
    expect(screen.queryByTestId("license-row-lic_ent_01")).toBeNull();
  });

  it("enforces read-only RBAC locks for ANALYST_READONLY across distribution tabs", async () => {
    const adminClient = createMockAdminClient();
    const distClient = createMockDistributionClient();
    render(<SuperAdminPortalApp client={adminClient} distributionClient={distClient} />);

    signInAs("ANALYST_READONLY");
    await waitFor(() => expect(screen.getByTestId("superadmin-tab-distribution")).toBeTruthy());

    fireEvent.click(screen.getByTestId("superadmin-tab-plugin-catalog"));
    expect(screen.getByTestId("catalog-readonly-lock")).toBeTruthy();
    expect((screen.getByTestId("catalog-open-create-drawer-btn") as HTMLButtonElement).disabled).toBe(true);
    await waitFor(() => expect(screen.getByTestId("catalog-row-html-forge-core")).toBeTruthy());
    fireEvent.click(screen.getByTestId("catalog-row-html-forge-core"));
    expect(screen.getByTestId("catalog-drawer-readonly-lock")).toBeTruthy();
    expect((screen.getByTestId("catalog-submit-btn") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId("catalog-drawer-close-btn"));

    fireEvent.click(screen.getByTestId("superadmin-tab-distribution"));
    expect(screen.getByTestId("canary-readonly-lock")).toBeTruthy();
    expect((screen.getByTestId("canary-submit-btn") as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(screen.getByTestId("superadmin-tab-quarantine"));
    expect(screen.getByTestId("quarantine-readonly-lock")).toBeTruthy();
    expect((screen.getByTestId("quarantine-submit-btn") as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(screen.getByTestId("superadmin-tab-licensing"));
    expect(screen.getByTestId("license-readonly-lock")).toBeTruthy();
    expect((screen.getByTestId("license-submit-btn") as HTMLButtonElement).disabled).toBe(true);
  });

  it("verifies real WebCrypto Ed25519 offline licenses and fails closed on tamper, untrusted key, or expiry", async () => {
    const validFixture = await buildSignedOfflineToken();
    const valid = await inspectOfflineLicenseToken(
      validFixture.token,
      validFixture.publicKeyBase64,
      validFixture.now
    );
    expect(valid.validFormat).toBe(true);
    expect(valid.signatureVerified).toBe(true);
    expect(valid.expired).toBe(false);
    expect(valid.payload?.cowokOrgId).toBe("org_cowok_vn_01");

    // Untrusted public key fails closed
    const otherFixture = await buildSignedOfflineToken();
    const wrongKey = await inspectOfflineLicenseToken(
      validFixture.token,
      otherFixture.publicKeyBase64,
      validFixture.now
    );
    expect(wrongKey.signatureVerified).toBe(false);
    expect(wrongKey.reason).toBe("UNTRUSTED_PUBLIC_KEY");

    // 1-byte tampered token fails closed
    const chars = validFixture.token.split("");
    const idx = chars.length - 6;
    chars[idx] = chars[idx] === "A" ? "B" : "A";
    const tampered = await inspectOfflineLicenseToken(
      chars.join(""),
      validFixture.publicKeyBase64,
      validFixture.now
    );
    expect(tampered.signatureVerified).toBe(false);

    // Strict expiry boundary (expiresAt === now)
    const atBoundary = await inspectOfflineLicenseToken(
      validFixture.token,
      validFixture.publicKeyBase64,
      validFixture.now + 86_400
    );
    expect(atBoundary.expired).toBe(true);

    const invalid = await inspectOfflineLicenseToken("invalid_prefix.abc", validFixture.publicKeyBase64);
    expect(invalid.validFormat).toBe(false);
    expect(invalid.signatureVerified).toBe(false);
  });

  it("enforces Zod supply-chain validation rules for GitHub refs, traversal URLs, SHA-256, and A/B keys", () => {
    expect(
      UpsertCatalogPluginRequestSchema.safeParse({
        pluginId: "bad-branch",
        sourceKind: "RECOMMENDED_GITHUB",
        lifecycleStatus: "ACTIVE",
        repoUrl: "https://github.com/cowok-design/plugin",
        pinnedCommitSha: "main",
        reason: "Testing mutable branch block",
      }).success
    ).toBe(false);

    expect(
      UpsertCatalogPluginRequestSchema.safeParse({
        pluginId: "bad-trav",
        sourceKind: "RECOMMENDED_GITHUB",
        lifecycleStatus: "ACTIVE",
        repoUrl: "https://github.com/../etc",
        pinnedCommitSha: "v1.2.3",
        reason: "Testing traversal block",
      }).success
    ).toBe(false);

    expect(
      UpsertExperimentRequestSchema.safeParse({
        experimentKey: "bad:colon",
        title: "Colon key test",
        targetSurface: "ONBOARDING_DEFAULTS",
        status: "RUNNING",
        reason: "Testing colon block",
        variants: [
          { key: "control", weight: 50 },
          { key: "treatment", weight: 50 },
        ],
      }).success
    ).toBe(false);

    expect(
      PublishRolloutRequestSchema.safeParse({
        channel: "desktop",
        version: "1.5.0",
        rolloutPercent: 25,
        downloadUrl: "https://releases.cowok.design/desktop/1.5.0",
        artifactSha256: "z".repeat(64),
      }).success
    ).toBe(false);

    expect(
      UpsertCatalogPluginRequestSchema.safeParse({
        pluginId: "async-scan-plugin",
        sourceKind: "RECOMMENDED_GITHUB",
        lifecycleStatus: "PROCESSING",
        repoUrl: "https://github.com/cowok-design/async-scan-plugin",
        pinnedCommitSha: "e".repeat(40),
        reason: "Submit for async security evaluation",
      }).success
    ).toBe(true);

    expect(
      BulkUpdateCatalogRequestSchema.safeParse({
        pluginIds: ["async-scan-plugin"],
        lifecycleStatus: "PENDING_APPROVAL",
        reason: "Move scanned batch to pending approval",
      }).success
    ).toBe(true);

    expect(
      BulkUpdateCatalogRequestSchema.safeParse({
        pluginIds: [],
        lifecycleStatus: "ACTIVE",
        reason: "Empty batch must fail",
      }).success
    ).toBe(false);
  });

  it("resolves plugin titleI18n and summaryI18n dynamically by active locale with en -> vi -> pluginId fallback", () => {
    const samplePlugin = {
      pluginId: "html-forge-core",
      sourceKind: "DEFAULT_BUILTIN" as const,
      lifecycleStatus: "ACTIVE" as const,
      repoUrl: null,
      pinnedCommitSha: null,
      minAppVersion: "0.1.0",
      defaultEnabled: true,
      defaultConfig: {},
      titleI18n: {
        vi: "Lõi Dựng HTML Forge",
        en: "HTML Forge Core",
        ja: "HTML Forge コア",
      },
      summaryI18n: {
        vi: "Mô tả tiếng Việt",
        en: "English summary",
      },
      manifestSeq: 1,
      updatedBy: "system:seed",
      updatedAt: 1_732_400_000_000,
    };

    expect(resolveLocalizedTitle(samplePlugin, "vi-VN")).toBe("Lõi Dựng HTML Forge");
    expect(resolveLocalizedTitle(samplePlugin, "en-US")).toBe("HTML Forge Core");
    expect(resolveLocalizedTitle(samplePlugin, "ja-JP")).toBe("HTML Forge コア");
    // Missing 'fr' falls back to 'en', never 'vi'
    expect(resolveLocalizedTitle(samplePlugin, "fr-FR")).toBe("HTML Forge Core");
    // Missing 'ja' in summary falls back to 'en' before 'vi'
    expect(resolveLocalizedSummary(samplePlugin, "ja-JP")).toBe("English summary");
    expect(resolveLocalizedSummary(samplePlugin, "vi-VN")).toBe("Mô tả tiếng Việt");
    // Empty i18n map falls back to pluginId
    expect(resolveLocalizedTitle({ ...samplePlugin, titleI18n: {} }, "ja-JP")).toBe("html-forge-core");
  });
});
