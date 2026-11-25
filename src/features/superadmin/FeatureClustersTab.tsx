// @story 14.2
// @implements AC-03 (Feature Request Clustering & 1-Click Internal GitHub Issue Generator)
import React, { useMemo, useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  CloudAdminRole,
  CloudFeatureCluster,
  CloudSupportTicket,
} from "../../lib/validations/cloud-admin";
import { canRoleManageClusters, type ClusterCreateInput } from "./useSuperAdminController";

export interface FeatureClustersTabProps {
  clusters: CloudFeatureCluster[];
  tickets: CloudSupportTicket[];
  operatorRole?: CloudAdminRole;
  submitting: boolean;
  onCreateCluster: (input: ClusterCreateInput) => Promise<boolean>;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const MIN_CLUSTER_TITLE_LEN = 3;
const ICON_SIZE_SM = 14;

const CARD_STYLE: React.CSSProperties = {
  padding: "14px",
  borderRadius: "10px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  display: "flex",
  flexDirection: "column",
  gap: "8px",
};

const FIELD_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "8px 10px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "13px",
  boxSizing: "border-box",
};

function ClusterCard({ cluster }: { cluster: CloudFeatureCluster }) {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <article style={CARD_STYLE} data-testid={`cluster-card-${cluster.id}`}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px" }}>
        <strong style={{ fontSize: "14px", color: "var(--text-primary)" }}>{cluster.title}</strong>
        <span
          style={{
            padding: "2px 8px",
            borderRadius: "999px",
            fontSize: "11px",
            fontWeight: FONT_WEIGHT_BOLD,
            backgroundColor: "var(--state-info-soft)",
            color: "var(--state-info)",
          }}
        >
          {formatNumber(cluster.ticketCount)} {t("superadmin.clusters.requests", "requests")} · {cluster.status}
        </span>
      </div>
      <p style={{ margin: 0, fontSize: "12px", color: "var(--text-secondary)" }}>
        {cluster.summary || t("superadmin.clusters.noSummary", "Merged from non-tech feature requests.")}
      </p>
      {cluster.githubIssueUrl && (
        <a
          href={cluster.githubIssueUrl}
          target="_blank"
          rel="noopener noreferrer"
          data-testid={`cluster-github-link-${cluster.id}`}
          style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--action-link)" }}
        >
          {t("superadmin.clusters.openGithub", "1-Click Open GitHub Issue ->")}
        </a>
      )}
    </article>
  );
}

function TicketCheckList({
  featureTickets,
  selectedIds,
  disabled,
  onToggle,
}: {
  featureTickets: CloudSupportTicket[];
  selectedIds: string[];
  disabled: boolean;
  onToggle: (id: string) => void;
}) {
  return (
    <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
      {featureTickets.map((ticket) => {
        const checked = selectedIds.includes(ticket.id);
        return (
          <label
            key={ticket.id}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "6px",
              padding: "4px 8px",
              borderRadius: "6px",
              border: "1px solid var(--border-subtle)",
              backgroundColor: checked ? "var(--bg-selected)" : "var(--bg-subtle)",
              fontSize: "12px",
              color: "var(--text-primary)",
            }}
          >
            <input
              type="checkbox"
              disabled={disabled}
              checked={checked}
              onChange={() => onToggle(ticket.id)}
              data-testid={`cluster-ticket-check-${ticket.id}`}
            />
            <span>{ticket.title}</span>
          </label>
        );
      })}
    </div>
  );
}

function ClusterSubmitButton({ canManage, disabled }: { canManage: boolean; disabled: boolean }) {
  const { t } = useTranslation();
  return (
    <button
      type="submit"
      disabled={disabled}
      data-testid="cluster-submit-btn"
      style={{
        alignSelf: "flex-start",
        height: "34px",
        padding: "0 14px",
        borderRadius: "8px",
        border: "none",
        backgroundColor: canManage ? "var(--action-primary)" : "var(--bg-subtle)",
        color: canManage ? "var(--action-primary-fg)" : "var(--text-disabled)",
        fontSize: "13px",
        fontWeight: FONT_WEIGHT_SEMIBOLD,
        cursor: canManage ? "pointer" : "not-allowed",
        display: "inline-flex",
        alignItems: "center",
        gap: "6px",
      }}
    >
      <BrandIcon name="plus" size={ICON_SIZE_SM} color="currentColor" />
      <span>{t("superadmin.clusters.submitBtn", "Create Cluster & GitHub Link")}</span>
    </button>
  );
}

function ClusterTextInputs({
  title,
  summary,
  disabled,
  onTitle,
  onSummary,
}: {
  title: string;
  summary: string;
  disabled: boolean;
  onTitle: (v: string) => void;
  onSummary: (v: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <>
      <input type="text" disabled={disabled} data-testid="cluster-title-input" value={title} onChange={(e) => onTitle(e.target.value)} placeholder={t("superadmin.clusters.titlePlaceholder", "Cluster title (e.g., One-Click Figma Export)")} style={FIELD_STYLE} />
      <input type="text" disabled={disabled} data-testid="cluster-summary-input" value={summary} onChange={(e) => onSummary(e.target.value)} placeholder={t("superadmin.clusters.summaryPlaceholder", "Short problem summary for GitHub issue")} style={FIELD_STYLE} />
    </>
  );
}

function CreateClusterForm({
  featureTickets,
  canManage,
  submitting,
  onCreateCluster,
}: {
  featureTickets: CloudSupportTicket[];
  canManage: boolean;
  submitting: boolean;
  onCreateCluster: (input: ClusterCreateInput) => Promise<boolean>;
}) {
  const { t } = useTranslation();
  const [title, setTitle] = useState("");
  const [summary, setSummary] = useState("");
  const [selectedIds, setSelectedIds] = useState<string[]>([]);

  const toggleId = (id: string) =>
    setSelectedIds((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canManage || title.trim().length < MIN_CLUSTER_TITLE_LEN) return;
    if (await onCreateCluster({ title: title.trim(), summary: summary.trim(), ticketIds: selectedIds, status: "planned" })) {
      setTitle("");
      setSummary("");
      setSelectedIds([]);
    }
  };

  return (
    <form onSubmit={handleSubmit} style={CARD_STYLE} data-testid="create-cluster-form">
      <div style={{ fontWeight: FONT_WEIGHT_BOLD, fontSize: "13px", color: "var(--text-primary)" }}>
        {t("superadmin.clusters.createTitle", "Merge Feature Requests into Cluster")}
      </div>
      <ClusterTextInputs title={title} summary={summary} disabled={!canManage} onTitle={setTitle} onSummary={setSummary} />
      <TicketCheckList featureTickets={featureTickets} selectedIds={selectedIds} disabled={!canManage} onToggle={toggleId} />
      <ClusterSubmitButton canManage={canManage} disabled={!canManage || submitting || title.trim().length < MIN_CLUSTER_TITLE_LEN} />
    </form>
  );
}

export const FeatureClustersTab: React.FC<FeatureClustersTabProps> = ({
  clusters,
  tickets,
  operatorRole,
  submitting,
  onCreateCluster,
}) => {
  const { t } = useTranslation();
  const canManage = canRoleManageClusters(operatorRole);
  const featureTickets = useMemo(
    () => tickets.filter((tk) => tk.category === "feature_request" || tk.category === "ux_friction"),
    [tickets]
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "14px" }} data-testid="superadmin-clusters-tab">
      <CreateClusterForm featureTickets={featureTickets} canManage={canManage} submitting={submitting} onCreateCluster={onCreateCluster} />
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))", gap: "12px" }}>
        {clusters.length === 0 ? (
          <div style={{ ...CARD_STYLE, color: "var(--text-muted)", fontSize: "13px" }}>
            {t("superadmin.clusters.empty", "No feature request clusters yet.")}
          </div>
        ) : (
          clusters.map((cluster) => <ClusterCard key={cluster.id} cluster={cluster} />)
        )}
      </div>
    </div>
  );
};
