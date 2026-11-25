// @story 14.2, 14.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, ADR-008 (SuperAdmin Portal Root Application — Strict 2-Plane Isolation with Live Distribution Ops Modules)
import React, { useCallback, useState } from "react";
import type { DistributionOpsApiPort } from "../../api/distribution-ops";
import { CowokToast } from "../../components/ui/CowokToast";
import { ActivationFunnelTab } from "../../features/superadmin/ActivationFunnelTab";
import { CrashCenterTab } from "../../features/superadmin/CrashCenterTab";
import { AbExperimentsTab } from "../../features/superadmin/distribution/AbExperimentsTab";
import { CanaryRolloutTab } from "../../features/superadmin/distribution/CanaryRolloutTab";
import { GovernanceAuditTab } from "../../features/superadmin/distribution/GovernanceAuditTab";
import { LicenseFederationTab } from "../../features/superadmin/distribution/LicenseFederationTab";
import { PluginCatalogManagerTab } from "../../features/superadmin/distribution/PluginCatalogManagerTab";
import { SkillQuarantineTab } from "../../features/superadmin/distribution/SkillQuarantineTab";
import { useDistributionOps } from "../../features/superadmin/distribution/useDistributionOps";
import { FeatureClustersTab } from "../../features/superadmin/FeatureClustersTab";
import { HelpdeskInboxTab } from "../../features/superadmin/HelpdeskInboxTab";
import { SuperAdminKpiStrip } from "../../features/superadmin/SuperAdminKpiStrip";
import { TicketDetailDrawer } from "../../features/superadmin/TicketDetailDrawer";
import {
  type SuperAdminApiPort,
  type SuperAdminToastState,
  useSuperAdminController,
} from "../../features/superadmin/useSuperAdminController";
import { AdminLoginGate } from "./auth/AdminLoginGate";
import { SuperAdminShell } from "./layout/SuperAdminShell";
import {
  isLiveSection,
  type SuperAdminLiveSectionId,
  type SuperAdminPlannedSectionId,
  type SuperAdminSectionId,
} from "./navigation";

export interface SuperAdminPortalPageProps {
  client?: SuperAdminApiPort;
  distributionClient?: DistributionOpsApiPort;
}

const ROOT_STYLE: React.CSSProperties = {
  minHeight: "100vh",
  width: "100%",
  backgroundColor: "var(--bg-canvas)",
  color: "var(--text-primary)",
  boxSizing: "border-box",
};

type SuperAdminCtrl = ReturnType<typeof useSuperAdminController>;
type DistributionOpsCtrl = ReturnType<typeof useDistributionOps>;

function LiveActiveTabView({ activeTab, ctrl }: { activeTab: SuperAdminLiveSectionId; ctrl: SuperAdminCtrl }) {
  if (activeTab === "helpdesk") {
    return (
      <HelpdeskInboxTab
        tickets={ctrl.tickets}
        filters={ctrl.filters}
        loading={ctrl.loading}
        selectedTicketId={ctrl.selectedTicketId}
        onFiltersChange={ctrl.setFilters}
        onSelectTicket={ctrl.setSelectedTicketId}
      />
    );
  }
  if (activeTab === "clusters") {
    return (
      <FeatureClustersTab
        clusters={ctrl.clusters}
        tickets={ctrl.tickets}
        operatorRole={ctrl.operator?.role}
        submitting={ctrl.submitting}
        onCreateCluster={ctrl.createFeatureCluster}
      />
    );
  }
  if (activeTab === "funnel") return <ActivationFunnelTab funnel={ctrl.funnel} />;
  return <CrashCenterTab crashes={ctrl.crashes} loading={ctrl.loading} />;
}

function LiveOperationsWorkspace({
  activeTab,
  ctrl,
  openTicketCount,
  p0TicketCount,
}: {
  activeTab: SuperAdminLiveSectionId;
  ctrl: SuperAdminCtrl;
  openTicketCount: number;
  p0TicketCount: number;
}) {
  return (
    <>
      <SuperAdminKpiStrip
        funnel={ctrl.funnel}
        openTicketFallback={openTicketCount}
        p0Fallback={p0TicketCount}
        crashFallback={ctrl.crashes.length}
      />
      <LiveActiveTabView activeTab={activeTab} ctrl={ctrl} />
      <TicketDetailDrawer
        ticket={ctrl.selectedTicket}
        operatorRole={ctrl.operator?.role}
        submitting={ctrl.submitting}
        loadAttachment={ctrl.loadTicketAttachment}
        onClose={() => ctrl.setSelectedTicketId(null)}
        onSubmitReply={ctrl.submitTicketReply}
      />
    </>
  );
}

function DistributionModuleView({
  sectionId,
  operatorRole,
  distOps,
}: {
  sectionId: SuperAdminPlannedSectionId;
  operatorRole: NonNullable<SuperAdminCtrl["operator"]>["role"];
  distOps: DistributionOpsCtrl;
}) {
  if (sectionId === "plugin-catalog") {
    return (
      <PluginCatalogManagerTab
        plugins={distOps.plugins}
        signedManifest={distOps.catalogManifest}
        operatorRole={operatorRole}
        submitting={distOps.submitting}
        onUpsertPlugin={distOps.upsertCatalogPlugin}
        onBulkUpdateCatalog={distOps.bulkUpdateCatalog}
      />
    );
  }
  if (sectionId === "ab-experiments") {
    return (
      <AbExperimentsTab
        experiments={distOps.experiments}
        signedManifest={distOps.experimentsManifest}
        operatorRole={operatorRole}
        submitting={distOps.submitting}
        onUpsertExperiment={distOps.upsertExperiment}
      />
    );
  }
  if (sectionId === "distribution") {
    return (
      <CanaryRolloutTab
        rollouts={distOps.rollouts}
        latestManifest={distOps.latestManifest}
        operatorRole={operatorRole}
        submitting={distOps.submitting}
        onPublishRollout={distOps.publishRollout}
      />
    );
  }
  if (sectionId === "quarantine") {
    return (
      <SkillQuarantineTab
        advisories={distOps.advisories}
        signedManifest={distOps.advisoriesManifest}
        operatorRole={operatorRole}
        submitting={distOps.submitting}
        onIssueQuarantine={distOps.issueQuarantine}
      />
    );
  }
  if (sectionId === "licensing") {
    return (
      <LicenseFederationTab
        licenses={distOps.licenses}
        keyId={distOps.licenseKeyId}
        publicKeyBase64={distOps.licensePublicKey}
        operatorRole={operatorRole}
        submitting={distOps.submitting}
        onIssueLicense={distOps.issueLicense}
      />
    );
  }
  return <GovernanceAuditTab logs={distOps.auditLogs} operatorRole={operatorRole} />;
}

function PortalAuthenticatedBody({
  ctrl,
  distOps,
  activeSection,
  onSelectSection,
  onRefreshAll,
  onSignOut,
}: {
  ctrl: SuperAdminCtrl;
  distOps: DistributionOpsCtrl;
  activeSection: SuperAdminSectionId;
  onSelectSection: (section: SuperAdminSectionId) => void;
  onRefreshAll: () => void;
  onSignOut: () => void;
}) {
  const operator = ctrl.operator;
  if (!operator) return null;
  const openTicketCount = ctrl.tickets.filter((tk) => tk.status !== "resolved" && tk.status !== "closed").length;
  const p0TicketCount = ctrl.tickets.filter((tk) => tk.priority === "P0" && tk.status !== "closed").length;
  const activeCrashCount = ctrl.funnel?.activeCrashGroups ?? ctrl.crashes.length;

  return (
    <SuperAdminShell
      operator={operator}
      activeSection={activeSection}
      loading={ctrl.loading || distOps.loading}
      openTicketCount={ctrl.funnel?.openTickets ?? openTicketCount}
      activeCrashCount={activeCrashCount}
      onSelectSection={onSelectSection}
      onRefresh={onRefreshAll}
      onPreviewRole={ctrl.previewRole}
      onSignOut={onSignOut}
    >
      {isLiveSection(activeSection) ? (
        <LiveOperationsWorkspace
          activeTab={activeSection}
          ctrl={ctrl}
          openTicketCount={openTicketCount}
          p0TicketCount={p0TicketCount}
        />
      ) : (
        <DistributionModuleView sectionId={activeSection} operatorRole={operator.role} distOps={distOps} />
      )}
    </SuperAdminShell>
  );
}

export const SuperAdminPortalApp: React.FC<SuperAdminPortalPageProps> = ({
  client,
  distributionClient,
}) => {
  const ctrl = useSuperAdminController(client);
  const [activeSection, setActiveSection] = useState<SuperAdminSectionId>("helpdesk");
  const [distToast, setDistToast] = useState<SuperAdminToastState | null>(null);

  const distOps = useDistributionOps({
    sessionToken: ctrl.sessionToken,
    operatorRole: ctrl.operator?.role,
    client: distributionClient,
    onNotify: setDistToast,
  });

  const handleSelectSection = useCallback(
    (section: SuperAdminSectionId) => {
      setActiveSection(section);
      if (isLiveSection(section)) ctrl.setActiveTab(section);
    },
    [ctrl]
  );

  const handleRefreshAll = useCallback(() => {
    ctrl.refreshAll();
    void distOps.refreshDistributionOps();
  }, [ctrl, distOps]);

  const handleSignOut = useCallback(() => {
    setActiveSection("helpdesk");
    setDistToast(null);
    ctrl.signOut();
  }, [ctrl]);

  const activeToast = distToast ?? ctrl.toast;
  const dismissActiveToast = useCallback(() => {
    if (distToast) setDistToast(null);
    else ctrl.dismissToast();
  }, [ctrl, distToast]);

  return (
    <main style={ROOT_STYLE} data-ff-id="superadmin-portal-root" data-testid="superadmin-portal-root">
      {!ctrl.operator ? (
        <AdminLoginGate
          form={ctrl.authForm}
          pending={ctrl.authPending}
          onChange={ctrl.setAuthForm}
          onSignIn={() => void ctrl.signInOidcMfa()}
        />
      ) : (
        <PortalAuthenticatedBody
          ctrl={ctrl}
          distOps={distOps}
          activeSection={activeSection}
          onSelectSection={handleSelectSection}
          onRefreshAll={handleRefreshAll}
          onSignOut={handleSignOut}
        />
      )}
      {activeToast && (
        <CowokToast
          tone={activeToast.tone}
          message={activeToast.message}
          onDismiss={dismissActiveToast}
          testId="superadmin-toast"
        />
      )}
    </main>
  );
};

export default SuperAdminPortalApp;
