// @story 14.2
// @implements AC-03 (DESIGN.md [5.22] AdvancedFilterBar + [5.21] DataTableView — 2-Way Non-Tech Helpdesk Inbox)
import React from "react";
import type { CloudAdminTicketFilters } from "../../api/cloud-admin";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { CloudSupportTicket, CloudTicketPriority } from "../../lib/validations/cloud-admin";

export interface HelpdeskInboxTabProps {
  tickets: CloudSupportTicket[];
  filters: CloudAdminTicketFilters;
  loading: boolean;
  selectedTicketId: string | null;
  onFiltersChange: (next: CloudAdminTicketFilters) => void;
  onSelectTicket: (ticketId: string) => void;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 14;
const TABLE_COL_SPAN = 7;
const PRIORITY_OPTIONS: ReadonlyArray<"ALL" | CloudTicketPriority> = ["ALL", "P0", "P1", "P2", "P3"];

const FILTER_BAR_STYLE: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  alignItems: "center",
  gap: "10px",
  padding: "10px 14px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "10px",
};

const INPUT_STYLE: React.CSSProperties = {
  flex: "1 1 220px",
  height: "34px",
  padding: "0 10px 0 32px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "13px",
};

const SELECT_STYLE: React.CSSProperties = {
  height: "34px",
  padding: "0 10px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "13px",
};

const TABLE_WRAP_STYLE: React.CSSProperties = {
  overflowX: "auto",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "10px",
};

const TH_STYLE: React.CSSProperties = {
  textAlign: "left",
  padding: "10px 12px",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  color: "var(--text-muted)",
  borderBottom: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-subtle)",
  whiteSpace: "nowrap",
};

const TD_STYLE: React.CSSProperties = {
  padding: "9px 12px",
  fontSize: "13px",
  color: "var(--text-primary)",
  borderBottom: "1px solid var(--border-subtle)",
  height: "40px",
};

function priorityColors(priority: string): { fg: string; bg: string } {
  if (priority === "P0") return { fg: "var(--state-danger)", bg: "var(--state-danger-soft)" };
  if (priority === "P1") return { fg: "var(--state-warning)", bg: "var(--state-warning-soft)" };
  if (priority === "P2") return { fg: "var(--state-info)", bg: "var(--state-info-soft)" };
  return { fg: "var(--text-secondary)", bg: "var(--bg-subtle)" };
}

function PriorityFilterPills({ active, onSelect }: { active: string; onSelect: (priority: string) => void }) {
  return (
    <div style={{ display: "flex", gap: "6px" }} role="group" aria-label="Priority filter">
      {PRIORITY_OPTIONS.map((p) => {
        const isSelected = active === p;
        return (
          <button
            key={p}
            type="button"
            data-testid={`priority-pill-${p}`}
            aria-pressed={isSelected}
            onClick={() => onSelect(p)}
            style={{
              height: "30px",
              padding: "0 10px",
              borderRadius: "999px",
              border: `1px solid ${isSelected ? "var(--action-primary)" : "var(--border-default)"}`,
              backgroundColor: isSelected ? "var(--bg-selected)" : "var(--bg-workspace)",
              color: isSelected ? "var(--action-primary)" : "var(--text-secondary)",
              fontSize: "12px",
              fontWeight: FONT_WEIGHT_SEMIBOLD,
              cursor: "pointer",
            }}
          >
            {p}
          </button>
        );
      })}
    </div>
  );
}

function HelpdeskSelectControls({
  filters,
  onChange,
}: {
  filters: CloudAdminTicketFilters;
  onChange: (next: CloudAdminTicketFilters) => void;
}) {
  const { t } = useTranslation();
  return (
    <>
      <select
        aria-label={t("superadmin.helpdesk.statusFilter", "Status")}
        data-testid="helpdesk-status-filter"
        value={filters.status ?? "all"}
        onChange={(e) => onChange({ ...filters, status: e.target.value })}
        style={SELECT_STYLE}
      >
        <option value="all">{t("superadmin.helpdesk.allStatuses", "All statuses")}</option>
        <option value="open">open</option>
        <option value="triaged">triaged</option>
        <option value="replied">replied</option>
        <option value="resolved">resolved</option>
        <option value="closed">closed</option>
      </select>
      <select
        aria-label={t("superadmin.helpdesk.channelFilter", "Channel")}
        data-testid="helpdesk-channel-filter"
        value={filters.channel ?? "all"}
        onChange={(e) => onChange({ ...filters, channel: e.target.value })}
        style={SELECT_STYLE}
      >
        <option value="all">{t("superadmin.helpdesk.allChannels", "All channels")}</option>
        <option value="cli">cli</option>
        <option value="desktop">desktop</option>
        <option value="docker">docker</option>
        <option value="plugin">plugin</option>
      </select>
    </>
  );
}

function HelpdeskFilterBar({
  filters,
  onChange,
}: {
  filters: CloudAdminTicketFilters;
  onChange: (next: CloudAdminTicketFilters) => void;
}) {
  const { t } = useTranslation();
  return (
    <div style={FILTER_BAR_STYLE} data-testid="superadmin-helpdesk-filter-bar">
      <div style={{ position: "relative", flex: "1 1 220px", display: "flex", alignItems: "center" }}>
        <span style={{ position: "absolute", left: "10px", pointerEvents: "none" }}>
          <BrandIcon name="search" size={ICON_SIZE_SM} color="var(--text-muted)" />
        </span>
        <input
          type="search"
          data-testid="helpdesk-search-input"
          value={filters.search ?? ""}
          onChange={(e) => onChange({ ...filters, search: e.target.value })}
          placeholder={t("superadmin.helpdesk.searchPlaceholder", "Search tickets, install ID...")}
          style={INPUT_STYLE}
        />
      </div>
      <PriorityFilterPills active={filters.priority ?? "ALL"} onSelect={(priority) => onChange({ ...filters, priority })} />
      <HelpdeskSelectControls filters={filters} onChange={onChange} />
    </div>
  );
}

function PriorityBadge({ priority }: { priority: string }) {
  const badge = priorityColors(priority);
  return (
    <span
      style={{
        display: "inline-block",
        padding: "2px 8px",
        borderRadius: "999px",
        fontSize: "11px",
        fontWeight: FONT_WEIGHT_BOLD,
        color: badge.fg,
        backgroundColor: badge.bg,
      }}
    >
      {priority}
    </span>
  );
}

function TicketTableRow({
  ticket,
  isSelected,
  onSelect,
}: {
  ticket: CloudSupportTicket;
  isSelected: boolean;
  onSelect: (id: string) => void;
}) {
  const { formatDate, formatNumber } = useFormatter();
  return (
    <tr
      data-testid={`ticket-row-${ticket.id}`}
      onClick={() => onSelect(ticket.id)}
      style={{ backgroundColor: isSelected ? "var(--bg-selected)" : "transparent", cursor: "pointer" }}
    >
      <td style={TD_STYLE}><PriorityBadge priority={ticket.priority} /></td>
      <td style={TD_STYLE}>
        <div style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{ticket.title}</div>
        <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>{ticket.installId} · {ticket.appVersion}</div>
      </td>
      <td style={TD_STYLE}>{ticket.category}</td>
      <td style={TD_STYLE}>{ticket.channel} / {ticket.hostClient}</td>
      <td style={TD_STYLE}>{ticket.status}</td>
      <td style={TD_STYLE}>{formatNumber(ticket.replies.length)}</td>
      <td style={TD_STYLE}>{formatDate(ticket.updatedAt)}</td>
    </tr>
  );
}

export const HelpdeskInboxTab: React.FC<HelpdeskInboxTabProps> = ({
  tickets,
  filters,
  loading,
  selectedTicketId,
  onFiltersChange,
  onSelectTicket,
}) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <HelpdeskFilterBar filters={filters} onChange={onFiltersChange} />
      <div style={TABLE_WRAP_STYLE} data-ff-id="superadmin-helpdesk-table" data-testid="superadmin-helpdesk-table">
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr>
              <th style={TH_STYLE}>{t("superadmin.helpdesk.colPriority", "Priority")}</th>
              <th style={TH_STYLE}>{t("superadmin.helpdesk.colTicket", "Ticket")}</th>
              <th style={TH_STYLE}>{t("superadmin.helpdesk.colCategory", "Category")}</th>
              <th style={TH_STYLE}>{t("superadmin.helpdesk.colChannel", "Channel / Host")}</th>
              <th style={TH_STYLE}>{t("superadmin.helpdesk.colStatus", "Status")}</th>
              <th style={TH_STYLE}>{t("superadmin.helpdesk.colReplies", "Replies")}</th>
              <th style={TH_STYLE}>{t("superadmin.helpdesk.colUpdated", "Updated")}</th>
            </tr>
          </thead>
          <tbody>
            {tickets.length === 0 ? (
              <tr>
                <td colSpan={TABLE_COL_SPAN} style={{ ...TD_STYLE, textAlign: "center", color: "var(--text-muted)" }}>
                  {loading ? t("superadmin.common.loading", "Loading...") : t("superadmin.helpdesk.empty", "No support tickets match the active filters.")}
                </td>
              </tr>
            ) : (
              tickets.map((ticket) => (
                <TicketTableRow key={ticket.id} ticket={ticket} isSelected={ticket.id === selectedTicketId} onSelect={onSelectTicket} />
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
};
