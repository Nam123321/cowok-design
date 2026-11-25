// @story 14.2
// @implements AC-02, AC-03, AC-04, ADR-008 (SuperAdmin Portal Root Application — Strict 2-Plane Isolation)
import React, { useCallback, useState } from "react";
import { CowokToast } from "../../components/ui/CowokToast";
import { ActivationFunnelTab } from "../../features/superadmin/ActivationFunnelTab";
import { CrashCenterTab } from "../../features/superadmin/CrashCenterTab";
import { FeatureClustersTab } from "../../features/superadmin/FeatureClustersTab";
import { HelpdeskInboxTab } from "../../features/superadmin/HelpdeskInboxTab";
import { SuperAdminKpiStrip } from "../../features/superadmin/SuperAdminKpiStrip";
import { TicketDetailDrawer } from "../../features/superadmin/TicketDetailDrawer";
import { type SuperAdminApiPort, useSuperAdminController } from "../../features/superadmin/useSuperAdminController";
import { AdminLoginGate } from "./auth/AdminLoginGate";
import { SuperAdminShell } from "./layout/SuperAdminShell";
import { PlannedModulePanel } from "./modules/PlannedModulePanel";
import { isLiveSection, type SuperAdminLiveSectionId, type SuperAdminSectionId } from "./navigation";

export interface SuperAdminPortalPageProps {
  client?: SuperAdminApiPort;
}

const ROOT_STYLE: React.CSSProperties = {
  minHeight: "100vh",
  width: "100%",
  backgroundColor: "var(--bg-canvas)",
  color: "var(--text-primary)",
  boxSizing: "border-box",
};

type SuperAdminCtrl = ReturnType<typeof useSuperAdminController>;

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
      <SuperAdminKpiStrip funnel={ctrl.funnel} openTicketFallback={openTicketCount} p0Fallback={p0TicketCount} crashFallback={ctrl.crashes.length} />
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

function PortalAuthenticatedBody({
  ctrl,
  activeSection,
  onSelectSection,
  onSignOut,
}: {
  ctrl: SuperAdminCtrl;
  activeSection: SuperAdminSectionId;
  onSelectSection: (section: SuperAdminSectionId) => void;
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
      loading={ctrl.loading}
      openTicketCount={ctrl.funnel?.openTickets ?? openTicketCount}
      activeCrashCount={activeCrashCount}
      onSelectSection={onSelectSection}
      onRefresh={ctrl.refreshAll}
      onPreviewRole={ctrl.previewRole}
      onSignOut={onSignOut}
    >
      {isLiveSection(activeSection) ? (
        <LiveOperationsWorkspace activeTab={activeSection} ctrl={ctrl} openTicketCount={openTicketCount} p0TicketCount={p0TicketCount} />
      ) : (
        <PlannedModulePanel sectionId={activeSection} operatorRole={operator.role} />
      )}
    </SuperAdminShell>
  );
}

export const SuperAdminPortalApp: React.FC<SuperAdminPortalPageProps> = ({ client }) => {
  const ctrl = useSuperAdminController(client);
  const [activeSection, setActiveSection] = useState<SuperAdminSectionId>("helpdesk");

  const handleSelectSection = useCallback(
    (section: SuperAdminSectionId) => {
      setActiveSection(section);
      if (isLiveSection(section)) ctrl.setActiveTab(section);
    },
    [ctrl]
  );

  const handleSignOut = useCallback(() => {
    setActiveSection("helpdesk");
    ctrl.signOut();
  }, [ctrl]);

  return (
    <main style={ROOT_STYLE} data-ff-id="superadmin-portal-root" data-testid="superadmin-portal-root">
      {!ctrl.operator ? (
        <AdminLoginGate form={ctrl.authForm} pending={ctrl.authPending} onChange={ctrl.setAuthForm} onSignIn={() => void ctrl.signInOidcMfa()} />
      ) : (
        <PortalAuthenticatedBody ctrl={ctrl} activeSection={activeSection} onSelectSection={handleSelectSection} onSignOut={handleSignOut} />
      )}
      {ctrl.toast && <CowokToast tone={ctrl.toast.tone} message={ctrl.toast.message} onDismiss={ctrl.dismissToast} testId="superadmin-toast" />}
    </main>
  );
};

export default SuperAdminPortalApp;
