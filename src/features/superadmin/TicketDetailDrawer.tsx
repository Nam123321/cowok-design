// @story 14.2
// @implements AC-02, AC-03 (DESIGN.md [5.25] RightDrawer — P0-P3 Triage, Sanitized Diagnostics & 2-Way Reply Composer with RBAC Lock)
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  CloudAdminRole,
  CloudSupportTicket,
  CloudTicketPriority,
  CloudTicketStatus,
} from "../../lib/validations/cloud-admin";
import { canRoleReplyTickets, type TicketReplySubmitInput } from "./useSuperAdminController";

export interface TicketDetailDrawerProps {
  ticket: CloudSupportTicket | null;
  operatorRole?: CloudAdminRole;
  submitting: boolean;
  loadAttachment?: (ticketId: string) => Promise<Blob | null>;
  onClose: () => void;
  onSubmitReply: (input: TicketReplySubmitInput) => Promise<boolean>;
}

const BACKDROP_Z_INDEX = 9980;
const DRAWER_Z_INDEX = 9981;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 14;
const REPLY_ROWS = 3;
const SHA_PREVIEW_LEN = 12;

const BACKDROP_STYLE: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  backgroundColor: "var(--bg-overlay)",
  zIndex: BACKDROP_Z_INDEX,
};

const DRAWER_STYLE: React.CSSProperties = {
  position: "fixed",
  top: 0,
  right: 0,
  bottom: 0,
  width: "460px",
  maxWidth: "100vw",
  backgroundColor: "var(--bg-surface)",
  borderLeft: "1px solid var(--border-default)",
  zIndex: DRAWER_Z_INDEX,
  display: "flex",
  flexDirection: "column",
};

const SECTION_BOX_STYLE: React.CSSProperties = {
  padding: "10px 12px",
  borderRadius: "8px",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-subtle)",
  fontSize: "12px",
  color: "var(--text-secondary)",
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

function buildTicketGitHubUrl(ticket: CloudSupportTicket): string {
  const title = encodeURIComponent(`[Support ${ticket.priority}] ${ticket.title}`);
  const body = encodeURIComponent(
    `Ticket ID: ${ticket.id}\nChannel: ${ticket.channel} (${ticket.hostClient})\nVersion: ${ticket.appVersion}\n\n${ticket.descriptionSanitized}`
  );
  return `https://github.com/cowok-design/cowok-design/issues/new?title=${title}&body=${body}`;
}

function TicketRepliesThread({ ticket }: { ticket: CloudSupportTicket }) {
  const { t } = useTranslation();
  const { formatTime } = useFormatter();
  if (ticket.replies.length === 0) {
    return (
      <div style={SECTION_BOX_STYLE}>
        {t("superadmin.drawer.noReplies", "No replies yet. Send a 2-way reply to reach the user's in-app inbox.")}
      </div>
    );
  }
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      {ticket.replies.map((rep) => (
        <div key={rep.id} style={SECTION_BOX_STYLE} data-testid={`ticket-reply-${rep.id}`}>
          <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "4px" }}>
            <strong style={{ color: "var(--text-primary)" }}>{rep.authorActor} ({rep.authorRole})</strong>
            <span style={{ color: rep.syncedToClient ? "var(--state-success)" : "var(--text-muted)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
              {rep.syncedToClient
                ? t("superadmin.drawer.synced", "Synced to App")
                : t("superadmin.drawer.cloudSaved", "Cloud Saved")}{" "}
              · {formatTime(rep.createdAt)}
            </span>
          </div>
          <div style={{ whiteSpace: "pre-wrap", color: "var(--text-primary)" }}>{rep.bodySanitized}</div>
        </div>
      ))}
    </div>
  );
}

function ReadOnlyLockNotice() {
  const { t } = useTranslation();
  return (
    <div
      role="note"
      data-testid="drawer-rbac-readonly-lock"
      style={{
        padding: "8px 10px",
        borderRadius: "8px",
        backgroundColor: "var(--state-warning-soft)",
        color: "var(--state-warning)",
        fontSize: "12px",
        display: "flex",
        alignItems: "center",
        gap: "6px",
      }}
    >
      <BrandIcon name="lock" size={ICON_SIZE_SM} color="var(--state-warning)" />
      <span>{t("superadmin.drawer.readOnlyLock", "Read-only RBAC mode: reply composer is locked for this role.")}</span>
    </div>
  );
}

function TriageSelectRow({
  priority,
  status,
  disabled,
  onPriority,
  onStatus,
}: {
  priority: CloudTicketPriority;
  status: CloudTicketStatus;
  disabled: boolean;
  onPriority: (val: CloudTicketPriority) => void;
  onStatus: (val: CloudTicketStatus) => void;
}) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
      <select
        aria-label={t("superadmin.drawer.priorityLabel", "Priority")}
        data-testid="drawer-priority-select"
        disabled={disabled}
        value={priority}
        onChange={(e) => onPriority(e.target.value as CloudTicketPriority)}
        style={FIELD_STYLE}
      >
        <option value="P0">P0 — Critical</option>
        <option value="P1">P1 — High</option>
        <option value="P2">P2 — Normal</option>
        <option value="P3">P3 — Low</option>
      </select>
      <select
        aria-label={t("superadmin.drawer.statusLabel", "Status")}
        data-testid="drawer-status-select"
        disabled={disabled}
        value={status}
        onChange={(e) => onStatus(e.target.value as CloudTicketStatus)}
        style={FIELD_STYLE}
      >
        <option value="triaged">triaged</option>
        <option value="replied">replied</option>
        <option value="resolved">resolved</option>
        <option value="closed">closed</option>
      </select>
    </div>
  );
}

function ReplySubmitButton({ canReply, disabled }: { canReply: boolean; disabled: boolean }) {
  const { t } = useTranslation();
  return (
    <button
      type="submit"
      disabled={disabled}
      data-testid="drawer-reply-submit"
      style={{
        height: "36px",
        borderRadius: "8px",
        border: "none",
        backgroundColor: canReply ? "var(--action-primary)" : "var(--bg-subtle)",
        color: canReply ? "var(--action-primary-fg)" : "var(--text-disabled)",
        fontSize: "13px",
        fontWeight: FONT_WEIGHT_SEMIBOLD,
        cursor: canReply ? "pointer" : "not-allowed",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        gap: "6px",
      }}
    >
      <BrandIcon name="send" size={ICON_SIZE_SM} color="currentColor" />
      <span>{t("superadmin.drawer.sendReply", "Send 2-Way Reply")}</span>
    </button>
  );
}

function TicketReplyForm({
  ticket,
  canReply,
  submitting,
  onSubmit,
}: {
  ticket: CloudSupportTicket;
  canReply: boolean;
  submitting: boolean;
  onSubmit: (input: TicketReplySubmitInput) => Promise<boolean>;
}) {
  const { t } = useTranslation();
  const [priority, setPriority] = useState<CloudTicketPriority>(ticket.priority);
  const [status, setStatus] = useState<CloudTicketStatus>("replied");
  const [body, setBody] = useState("");

  useEffect(() => {
    setPriority(ticket.priority);
    setStatus(ticket.status === "open" ? "replied" : ticket.status);
    setBody("");
  }, [ticket.id, ticket.priority, ticket.status]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canReply || !body.trim()) return;
    if (await onSubmit({ ticketId: ticket.id, body: body.trim(), priority, status })) setBody("");
  };

  return (
    <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      {!canReply && <ReadOnlyLockNotice />}
      <TriageSelectRow priority={priority} status={status} disabled={!canReply} onPriority={setPriority} onStatus={setStatus} />
      <textarea
        rows={REPLY_ROWS}
        disabled={!canReply}
        data-testid="drawer-reply-input"
        value={body}
        onChange={(e) => setBody(e.target.value)}
        placeholder={t("superadmin.drawer.replyPlaceholder", "Write a friendly 2-way reply to the user...")}
        style={{ ...FIELD_STYLE, resize: "vertical" }}
      />
      <ReplySubmitButton canReply={canReply} disabled={!canReply || submitting || !body.trim()} />
    </form>
  );
}

function DrawerHeader({ ticket, onClose }: { ticket: CloudSupportTicket; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "14px 16px", borderBottom: "1px solid var(--border-default)" }}>
      <div>
        <div style={{ fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>{ticket.title}</div>
        <div style={{ fontSize: "12px", color: "var(--text-muted)" }}>{ticket.id} · {ticket.channel} / {ticket.hostClient} · v{ticket.appVersion}</div>
      </div>
      <button
        type="button"
        onClick={onClose}
        aria-label={t("common.close", "Close")}
        data-testid="drawer-close-btn"
        style={{ background: "transparent", border: "1px solid var(--border-default)", borderRadius: "6px", padding: "6px", cursor: "pointer", color: "var(--text-secondary)" }}
      >
        <BrandIcon name="close" size={ICON_SIZE_SM} />
      </button>
    </header>
  );
}

function useAttachmentPreviewUrl(
  ticketId: string,
  attachmentMime: string | null | undefined,
  loadAttachment?: (ticketId: string) => Promise<Blob | null>
): string | null {
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let objectUrl: string | null = null;
    if (!attachmentMime || !loadAttachment) {
      setPreviewUrl(null);
      return;
    }
    void loadAttachment(ticketId).then((blob) => {
      if (!active || !blob) return;
      objectUrl = URL.createObjectURL(blob);
      setPreviewUrl(objectUrl);
    });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [attachmentMime, loadAttachment, ticketId]);

  return previewUrl;
}

function AttachmentPreviewCard({
  ticket,
  loadAttachment,
}: {
  ticket: CloudSupportTicket;
  loadAttachment?: (ticketId: string) => Promise<Blob | null>;
}) {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const previewUrl = useAttachmentPreviewUrl(ticket.id, ticket.attachmentMime, loadAttachment);

  if (!ticket.attachmentMime) return null;
  return (
    <div style={SECTION_BOX_STYLE} data-testid="drawer-attachment-meta">
      <div>
        <strong>{t("superadmin.drawer.attachment", "Re-encoded Attachment")}:</strong>{" "}
        {ticket.attachmentMime} ({formatNumber(ticket.attachmentBytes ?? 0)} B) · sha256:
        {ticket.attachmentSha256?.slice(0, SHA_PREVIEW_LEN)}
      </div>
      {previewUrl && (
        <img
          src={previewUrl}
          alt={t("superadmin.drawer.attachmentPreviewAlt", "Sanitized ticket screenshot preview")}
          data-testid="drawer-attachment-preview"
          style={{
            marginTop: "8px",
            maxWidth: "100%",
            maxHeight: "180px",
            borderRadius: "6px",
            border: "1px solid var(--border-default)",
            objectFit: "contain",
            display: "block",
          }}
        />
      )}
    </div>
  );
}

function TicketDiagnosticsBlock({
  ticket,
  loadAttachment,
}: {
  ticket: CloudSupportTicket;
  loadAttachment?: (ticketId: string) => Promise<Blob | null>;
}) {
  const { t } = useTranslation();
  return (
    <>
      <div style={SECTION_BOX_STYLE}>
        <div style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, marginBottom: "4px", color: "var(--text-primary)" }}>
          {t("superadmin.drawer.description", "Sanitized Description")}
        </div>
        <div style={{ whiteSpace: "pre-wrap" }}>{ticket.descriptionSanitized}</div>
      </div>
      <AttachmentPreviewCard ticket={ticket} loadAttachment={loadAttachment} />
      <div style={SECTION_BOX_STYLE}>
        <div style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, marginBottom: "4px", color: "var(--text-primary)" }}>
          {t("superadmin.drawer.diagnostics", "Sanitized Diagnostic Bundle")}
        </div>
        <pre style={{ margin: 0, overflowX: "auto", fontSize: "11px" }}>{JSON.stringify(ticket.diagnosticBundle, null, 2)}</pre>
      </div>
    </>
  );
}

export const TicketDetailDrawer: React.FC<TicketDetailDrawerProps> = ({
  ticket,
  operatorRole,
  submitting,
  loadAttachment,
  onClose,
  onSubmitReply,
}) => {
  const { t } = useTranslation();
  if (!ticket) return null;

  return (
    <>
      <div style={BACKDROP_STYLE} onClick={onClose} aria-hidden="true" />
      <aside role="dialog" aria-label={ticket.title} data-ff-id="superadmin-ticket-drawer" data-testid="superadmin-ticket-drawer" style={DRAWER_STYLE}>
        <DrawerHeader ticket={ticket} onClose={onClose} />
        <div style={{ flex: 1, overflowY: "auto", padding: "16px", display: "flex", flexDirection: "column", gap: "14px" }}>
          <TicketDiagnosticsBlock ticket={ticket} loadAttachment={loadAttachment} />
          <a href={buildTicketGitHubUrl(ticket)} target="_blank" rel="noopener noreferrer" data-testid="drawer-github-issue-link" style={{ color: "var(--action-link)", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
            {t("superadmin.drawer.createGithubIssue", "1-Click Create Internal GitHub Issue ->")}
          </a>
          <TicketRepliesThread ticket={ticket} />
          <TicketReplyForm ticket={ticket} canReply={canRoleReplyTickets(operatorRole)} submitting={submitting} onSubmit={onSubmitReply} />
        </div>
      </aside>
    </>
  );
};
