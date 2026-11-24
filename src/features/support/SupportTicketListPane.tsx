// @story 14.1
// @implements AC-01, AC-02 (Support Ticket List, Category Selector, Diagnostics Preview & Localized Zod Error Formatter)
import React, { useEffect, useState } from "react";
import { ZodError } from "zod";
import {
  apiSupport,
  getSupportTicketAttachmentUrl,
  SupportApiError,
} from "../../api/support";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  AdminReply,
  LocalSupportTicket,
  SupportCategory,
  SupportTicketStatus,
} from "../../lib/validations/support";

export const ICON_SIZE_XS = 11;
export const ICON_SIZE_SM = 13;
export const ICON_SIZE_MD = 16;
export const FONT_WEIGHT_MEDIUM = 500;
export const FONT_WEIGHT_SEMIBOLD = 600;
const JSON_PREVIEW_INDENT = 2;
const LIGHTBOX_Z_INDEX = 10020;

export interface SupportCategoryOption {
  id: SupportCategory;
  icon: string;
  labelKey: string;
  fallbackLabel: string;
  descKey: string;
  fallbackDesc: string;
}

export const SUPPORT_CATEGORY_OPTIONS: readonly SupportCategoryOption[] = [
  {
    id: "bug_report",
    icon: "alert",
    labelKey: "support.catBugReport",
    fallbackLabel: "Báo lỗi",
    descKey: "support.catBugReportDesc",
    fallbackDesc: "Ứng dụng chạy sai hoặc gặp sự cố",
  },
  {
    id: "feature_request",
    icon: "plus",
    labelKey: "support.catFeatureRequest",
    fallbackLabel: "Đề xuất tính năng",
    descKey: "support.catFeatureRequestDesc",
    fallbackDesc: "Ý tưởng cải tiến giao diện & luồng việc",
  },
  {
    id: "skill_template_request",
    icon: "plugin",
    labelKey: "support.catSkillRequest",
    fallbackLabel: "Yêu cầu Skill/Template",
    descKey: "support.catSkillRequestDesc",
    fallbackDesc: "Thêm mẫu thiết kế hoặc bộ công cụ mới",
  },
  {
    id: "question",
    icon: "help",
    labelKey: "support.catQuestion",
    fallbackLabel: "Hỏi đáp",
    descKey: "support.catQuestionDesc",
    fallbackDesc: "Cần hướng dẫn sử dụng nhanh",
  },
];

export const SUPPORT_FIELD_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "8px 10px",
  fontSize: "12px",
  fontFamily: "var(--font-sans)",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-default)",
  boxSizing: "border-box",
};

const TICKET_CARD_STYLE: React.CSSProperties = {
  padding: "12px",
  borderRadius: "var(--radius-sm)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-workspace)",
  display: "flex",
  flexDirection: "column",
  gap: "8px",
};

const REPLIES_BOX_STYLE: React.CSSProperties = {
  marginTop: "4px",
  padding: "8px 10px",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-subtle)",
  borderLeft: "2px solid var(--action-primary)",
  display: "flex",
  flexDirection: "column",
  gap: "6px",
};

const DIAG_BOX_STYLE: React.CSSProperties = {
  padding: "10px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-subtle)",
  display: "flex",
  flexDirection: "column",
  gap: "6px",
};

const DIAG_PRE_STYLE: React.CSSProperties = {
  margin: 0,
  padding: "8px",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)",
  fontFamily: "var(--font-mono)",
  fontSize: "10px",
  color: "var(--text-secondary)",
  overflowX: "auto",
};

export function formatSupportErrorMessage(
  err: unknown,
  t: (key: string, defaultVal?: string) => string
): string {
  if (err instanceof ZodError) {
    const firstIssue = err.issues[0];
    const pathKey = firstIssue?.path.join(".") ?? "";
    if (pathKey === "contactEmail") {
      return t("support.invalidEmailError", "Địa chỉ email không hợp lệ. Vui lòng kiểm tra lại.");
    }
    if (pathKey === "screenshotDataUrl") {
      return t(
        "support.invalidScreenshotError",
        "Ảnh chụp màn hình phải là tệp PNG, JPEG hoặc WebP hợp lệ."
      );
    }
    return t("support.invalidFormError", "Thông tin nhập chưa hợp lệ. Vui lòng kiểm tra lại các trường.");
  }
  if (err instanceof Error && err.message.trim().length > 0) {
    return err.message;
  }
  return t("support.submitFailed", "Không thể gửi yêu cầu lúc này.");
}

function statusToneStyle(status: SupportTicketStatus): { color: string; bg: string } {
  if (status === "resolved" || status === "closed") {
    return { color: "var(--state-success)", bg: "var(--state-success-soft)" };
  }
  if (status === "in_progress" || status === "triaged") {
    return { color: "var(--state-warning)", bg: "var(--state-warning-soft)" };
  }
  return { color: "var(--action-primary)", bg: "var(--bg-selected)" };
}

function statusLabel(
  status: SupportTicketStatus,
  t: (key: string, defaultVal?: string) => string
): string {
  switch (status) {
    case "submitted":
      return t("support.statusSubmitted", "Đã gửi");
    case "triaged":
      return t("support.statusTriaged", "Đã tiếp nhận");
    case "in_progress":
      return t("support.statusInProgress", "Đang xử lý");
    case "resolved":
      return t("support.statusResolved", "Đã giải quyết");
    case "closed":
      return t("support.statusClosed", "Đã đóng");
  }
}

function categoryLabel(
  category: SupportCategory,
  t: (key: string, defaultVal?: string) => string
): string {
  const found = SUPPORT_CATEGORY_OPTIONS.find((item) => item.id === category);
  return found ? t(found.labelKey, found.fallbackLabel) : category;
}

const AdminRepliesSection: React.FC<{
  ticketCode: string;
  replies: AdminReply[];
  locale: string;
}> = ({ ticketCode, replies, locale }) => {
  const { t } = useTranslation(locale);
  const fmt = useFormatter({ locale });
  if (replies.length === 0) return null;
  return (
    <div data-testid={`support-ticket-replies-${ticketCode}`} style={REPLIES_BOX_STYLE}>
      <div style={{ fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>
        {t("support.adminReplyHeading", "Phản hồi từ đội ngũ hỗ trợ")}
      </div>
      {replies.map((reply) => (
        <div key={reply.id} style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: "10.5px", color: "var(--text-muted)" }}>
            <span style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{reply.author}</span>
            <span>{`${fmt.formatDate(reply.createdAt)} ${fmt.formatTime(reply.createdAt)}`}</span>
          </div>
          <div style={{ fontSize: "11.5px", color: "var(--text-secondary)", lineHeight: "1.4" }}>
            {reply.message}
          </div>
        </div>
      ))}
    </div>
  );
};

const ATTACHMENT_CHIP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  alignSelf: "flex-start",
  padding: "4px 9px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-subtle)",
  color: "var(--text-primary)",
  fontSize: "11px",
  fontWeight: FONT_WEIGHT_MEDIUM,
  cursor: "pointer",
};

const LIGHTBOX_BACKDROP_STYLE: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: LIGHTBOX_Z_INDEX,
  backgroundColor: "var(--bg-overlay)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  padding: "20px",
};

const LIGHTBOX_SURFACE_STYLE: React.CSSProperties = {
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-subtle)",
  borderRadius: "var(--radius-md)",
  boxShadow: "var(--shadow-lg)",
  maxWidth: "min(92vw, 860px)",
  maxHeight: "90vh",
  width: "auto",
  minWidth: "300px",
  padding: "14px 16px",
  display: "flex",
  flexDirection: "column",
  gap: "12px",
  fontFamily: "var(--font-sans)",
};

const LIGHTBOX_CLOSE_BTN_STYLE: React.CSSProperties = {
  background: "transparent",
  border: "1px solid var(--border-subtle)",
  borderRadius: "var(--radius-xs)",
  width: "28px",
  height: "28px",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  color: "var(--text-secondary)",
  cursor: "pointer",
  flexShrink: 0,
};

const LIGHTBOX_DOWNLOAD_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  padding: "5px 10px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "11.5px",
  fontWeight: FONT_WEIGHT_MEDIUM,
  textDecoration: "none",
};

const MEDIA_EXT_BY_MIME: Readonly<Record<string, string>> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/webp": "webp",
  "video/mp4": "mp4",
  "video/webm": "webm",
};

export type AttachmentFetcher = (ticketId: string, signal?: AbortSignal) => Promise<Blob>;

const defaultFetchAttachment: AttachmentFetcher = (ticketId, signal) =>
  apiSupport.fetchSupportTicketAttachment(ticketId, signal);

function attachmentFilename(ticket: LocalSupportTicket): string {
  const ext = (ticket.attachmentMime && MEDIA_EXT_BY_MIME[ticket.attachmentMime]) || "png";
  return `${ticket.ticketCode}.${ext}`;
}

function useLightboxEscapeKey(onClose: () => void): void {
  useEffect(() => {
    const onKeyDown = (evt: KeyboardEvent) => {
      if (evt.key !== "Escape") return;
      evt.preventDefault();
      evt.stopImmediatePropagation();
      onClose();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [onClose]);
}

function useTicketAttachmentSrc(
  ticketId: string,
  fetchAttachment: AttachmentFetcher
): { src: string | null; loading: boolean; failed: boolean; fallbackUrl: string } {
  const fallbackUrl = getSupportTicketAttachmentUrl(ticketId);
  const [state, setState] = useState<{ src: string | null; loading: boolean; failed: boolean }>({
    src: null,
    loading: true,
    failed: false,
  });

  useEffect(() => {
    const controller = new AbortController();
    let objectUrl: string | null = null;
    setState({ src: null, loading: true, failed: false });
    void fetchAttachment(ticketId, controller.signal)
      .then((blob) => {
        if (controller.signal.aborted) return;
        const resolved = typeof URL.createObjectURL === "function" ? URL.createObjectURL(blob) : fallbackUrl;
        if (resolved !== fallbackUrl) objectUrl = resolved;
        setState({ src: resolved, loading: false, failed: false });
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        const isHttpError = err instanceof SupportApiError;
        setState({ src: isHttpError ? null : fallbackUrl, loading: false, failed: isHttpError });
      });
    return () => {
      controller.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [ticketId, fetchAttachment, fallbackUrl]);

  return { ...state, fallbackUrl };
}

const SupportMediaLightboxBody: React.FC<{
  ticket: LocalSupportTicket;
  locale: string;
  src: string | null;
  loading: boolean;
  failed: boolean;
}> = ({ ticket, locale, src, loading, failed }) => {
  const { t } = useTranslation(locale);
  const isVideo = Boolean(ticket.attachmentMime?.startsWith("video/"));
  if (loading) {
    return (
      <div style={{ padding: "32px 24px", textAlign: "center", fontSize: "12px", color: "var(--text-muted)" }}>
        {t("support.loadingAttachment", "Đang tải tệp đính kèm...")}
      </div>
    );
  }
  if (failed || !src) {
    return (
      <div role="alert" style={{ padding: "16px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-danger-soft)", color: "var(--state-danger)", fontSize: "12px" }}>
        {t("support.attachmentLoadFailed", "Không thể tải tệp đính kèm của yêu cầu này.")}
      </div>
    );
  }
  if (isVideo) {
    return (
      <video controls autoPlay playsInline style={{ maxWidth: "100%", maxHeight: "70vh" }} src={src} />
    );
  }
  return (
    <img alt={ticket.title} style={{ maxWidth: "100%", maxHeight: "70vh", objectFit: "contain" }} src={src} />
  );
};

const SupportMediaLightbox: React.FC<{
  ticket: LocalSupportTicket;
  locale: string;
  onClose: () => void;
  fetchAttachment: AttachmentFetcher;
}> = ({ ticket, locale, onClose, fetchAttachment }) => {
  const { t } = useTranslation(locale);
  const media = useTicketAttachmentSrc(ticket.id, fetchAttachment);
  useLightboxEscapeKey(onClose);
  const downloadHref = media.src || media.fallbackUrl;
  return (
    <div role="presentation" onClick={onClose} style={LIGHTBOX_BACKDROP_STYLE}>
      <div role="dialog" aria-modal="true" aria-label={ticket.title} data-testid="support-media-lightbox" onClick={(e) => e.stopPropagation()} style={LIGHTBOX_SURFACE_STYLE}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: "2px", minWidth: 0 }}>
            <span style={{ fontFamily: "var(--font-mono)", fontSize: "10.5px", color: "var(--text-secondary)" }}>
              {ticket.ticketCode}
            </span>
            <span style={{ fontSize: "13px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {ticket.title}
            </span>
          </div>
          <button type="button" data-testid="support-media-lightbox-close" aria-label={t("common.close", "Đóng")} onClick={onClose} style={LIGHTBOX_CLOSE_BTN_STYLE}>
            <BrandIcon name="close" size={ICON_SIZE_SM} color="currentColor" />
          </button>
        </div>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", backgroundColor: "var(--bg-workspace)", borderRadius: "var(--radius-xs)", padding: "8px", overflow: "auto" }}>
          <SupportMediaLightboxBody ticket={ticket} locale={locale} src={media.src} loading={media.loading} failed={media.failed} />
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end" }}>
          <a href={downloadHref} download={attachmentFilename(ticket)} data-testid="support-media-lightbox-download" style={LIGHTBOX_DOWNLOAD_STYLE}>
            <BrandIcon name="download" size={ICON_SIZE_XS} color="currentColor" />
            <span>{t("support.downloadAttachment", "Tải xuống")}</span>
          </a>
        </div>
      </div>
    </div>
  );
};

const SupportTicketAttachmentChip: React.FC<{
  ticket: LocalSupportTicket;
  locale: string;
  onPreview: (ticket: LocalSupportTicket) => void;
}> = ({ ticket, locale, onPreview }) => {
  const { t } = useTranslation(locale);
  if (!ticket.screenshotSha256) return null;
  const isVideo = Boolean(ticket.attachmentMime?.startsWith("video/"));
  const label = isVideo
    ? t("support.viewVideoAttachment", "Xem video đính kèm")
    : t("support.viewImageAttachment", "Xem ảnh đính kèm");
  return (
    <button
      type="button"
      data-testid={`support-ticket-attachment-${ticket.ticketCode}`}
      onClick={() => onPreview(ticket)}
      style={ATTACHMENT_CHIP_STYLE}
    >
      <BrandIcon name={isVideo ? "play" : "image-icon"} size={ICON_SIZE_XS} color="var(--action-primary)" />
      <span>{label}</span>
    </button>
  );
};

const SupportTicketCard: React.FC<{
  ticket: LocalSupportTicket;
  locale: string;
  onPreviewMedia: (ticket: LocalSupportTicket) => void;
}> = ({ ticket, locale, onPreviewMedia }) => {
  const { t } = useTranslation(locale);
  const fmt = useFormatter({ locale });
  const tone = statusToneStyle(ticket.status);
  const ts = `${fmt.formatDate(ticket.createdAt)} ${fmt.formatTime(ticket.createdAt)}`;
  return (
    <article data-testid={`support-ticket-card-${ticket.ticketCode}`} style={TICKET_CARD_STYLE}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <span style={{ fontFamily: "var(--font-mono)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>
          {ticket.ticketCode}
        </span>
        <span style={{ padding: "2px 8px", borderRadius: "var(--radius-pill)", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: tone.color, backgroundColor: tone.bg }}>
          {statusLabel(ticket.status, t)}
        </span>
      </div>
      <div style={{ fontSize: "12.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", lineHeight: "1.35" }}>
        {ticket.title}
      </div>
      <p style={{ margin: 0, fontSize: "11.5px", color: "var(--text-secondary)", lineHeight: "1.45", whiteSpace: "pre-wrap" }}>
        {ticket.description}
      </p>
      <SupportTicketAttachmentChip ticket={ticket} locale={locale} onPreview={onPreviewMedia} />
      <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "8px", fontSize: "10.5px", color: "var(--text-muted)" }}>
        <span>{categoryLabel(ticket.category, t)}</span>
        <span>•</span>
        <span>{ts}</span>
        <span>•</span>
        <span data-testid={`support-ticket-scrub-badge-${ticket.ticketCode}`} style={{ display: "inline-flex", alignItems: "center", gap: "4px", color: "var(--state-success)" }}>
          <BrandIcon name="check-circle" size={ICON_SIZE_XS} color="currentColor" />
          <span>{t("support.zeroPiiBadge", "Zero-PII đã làm sạch")}</span>
        </span>
      </div>
      <AdminRepliesSection ticketCode={ticket.ticketCode} replies={ticket.adminReplies} locale={locale} />
    </article>
  );
};

export const SupportCategoryGrid: React.FC<{
  category: SupportCategory;
  locale: string;
  onSelect: (cat: SupportCategory) => void;
}> = ({ category, locale, onSelect }) => {
  const { t } = useTranslation(locale);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
      <span style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>
        {t("support.categoryLabel", "Bạn cần hỗ trợ điều gì?")}
      </span>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "8px" }}>
        {SUPPORT_CATEGORY_OPTIONS.map((opt) => {
          const active = category === opt.id;
          return (
            <button
              key={opt.id}
              type="button"
              aria-pressed={active}
              data-testid={`support-category-${opt.id}`}
              onClick={() => onSelect(opt.id)}
              style={{
                display: "flex", flexDirection: "column", alignItems: "flex-start", gap: "3px",
                padding: "8px 10px", borderRadius: "var(--radius-xs)", textAlign: "left", cursor: "pointer",
                border: active ? "1.5px solid var(--action-primary)" : "1px solid var(--border-subtle)",
                backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
                color: "var(--text-primary)",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
                <BrandIcon name={opt.icon} size={ICON_SIZE_SM} color={active ? "var(--action-primary)" : "var(--text-secondary)"} />
                <span>{t(opt.labelKey, opt.fallbackLabel)}</span>
              </div>
              <span style={{ fontSize: "10.5px", color: "var(--text-muted)", lineHeight: "1.3" }}>
                {t(opt.descKey, opt.fallbackDesc)}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
};

function buildDiagnosticsPreviewJson(opts: {
  locale: string;
  includeDiagnostics: boolean;
  recentErrorCodes: string[];
  activeSkills: string[];
}): string {
  const { locale, includeDiagnostics, recentErrorCodes, activeSkills } = opts;
  return JSON.stringify(
    {
      channel: "desktop",
      locale,
      includeDiagnostics,
      recentErrorCodes: includeDiagnostics ? recentErrorCodes : [],
      activeSkills: includeDiagnostics ? activeSkills : [],
      sourceCodeAttached: false,
      chatPromptsAttached: false,
    },
    null,
    JSON_PREVIEW_INDENT
  );
}

export const SupportDiagnosticsPanel: React.FC<{
  locale: string;
  includeDiagnostics: boolean;
  showDiagPreview: boolean;
  recentErrorCodes: string[];
  activeSkills: string[];
  onToggleInclude: (next: boolean) => void;
  onTogglePreview: () => void;
}> = ({ locale, includeDiagnostics, showDiagPreview, recentErrorCodes, activeSkills, onToggleInclude, onTogglePreview }) => {
  const { t } = useTranslation(locale);
  const previewJson = buildDiagnosticsPreviewJson({ locale, includeDiagnostics, recentErrorCodes, activeSkills });
  return (
    <div style={DIAG_BOX_STYLE}>
      <label style={{ display: "flex", alignItems: "flex-start", gap: "8px", cursor: "pointer", fontSize: "11.5px", color: "var(--text-primary)" }}>
        <input type="checkbox" data-testid="support-include-diagnostics-toggle" checked={includeDiagnostics} onChange={(e) => onToggleInclude(e.target.checked)} style={{ marginTop: "2px" }} />
        <span>{t("support.includeDiagnosticsLabel", "Đính kèm gói chẩn đoán ẩn danh (phiên bản, hệ điều hành, mã lỗi)")}</span>
      </label>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", fontSize: "10.5px", color: "var(--state-success)" }}>
        <span style={{ display: "inline-flex", alignItems: "center", gap: "4px" }}>
          <BrandIcon name="check-circle" size={ICON_SIZE_XS} color="currentColor" />
          <span>{t("support.zeroPiiNotice", "Tự động xóa API key, mật khẩu & đường dẫn máy trước khi gửi")}</span>
        </span>
        <button type="button" data-testid="support-preview-diagnostics-btn" onClick={onTogglePreview} style={{ background: "transparent", border: "none", color: "var(--action-primary)", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer", padding: 0, flexShrink: 0 }}>
          {showDiagPreview ? t("support.hidePreview", "Ẩn") : t("support.showPreview", "Xem trước")}
        </button>
      </div>
      {showDiagPreview && <pre data-testid="support-diagnostics-preview" style={DIAG_PRE_STYLE}>{previewJson}</pre>}
    </div>
  );
};

export interface SupportTicketListPaneProps {
  locale: string;
  tickets: LocalSupportTicket[];
  loading: boolean;
  loadError: string | null;
  onRefresh: () => void;
  fetchAttachment?: AttachmentFetcher;
}

const SupportTicketListBody: React.FC<{
  locale: string;
  tickets: LocalSupportTicket[];
  loading: boolean;
  onPreviewMedia: (ticket: LocalSupportTicket) => void;
}> = ({ locale, tickets, loading, onPreviewMedia }) => {
  const { t } = useTranslation(locale);
  if (loading) {
    return (
      <div style={{ fontSize: "12px", color: "var(--text-muted)", padding: "16px 0", textAlign: "center" }}>
        {t("support.loadingTickets", "Đang tải danh sách yêu cầu...")}
      </div>
    );
  }
  if (tickets.length === 0) {
    return (
      <div data-testid="support-empty-tickets" style={{ padding: "20px 12px", borderRadius: "var(--radius-sm)", border: "1px dashed var(--border-default)", backgroundColor: "var(--bg-workspace)", textAlign: "center", fontSize: "12px", color: "var(--text-muted)" }}>
        {t("support.emptyTickets", "Bạn chưa gửi yêu cầu nào. Chuyển sang tab Gửi phản hồi để bắt đầu.")}
      </div>
    );
  }
  return (
    <>
      {tickets.map((ticket) => (
        <SupportTicketCard key={ticket.id} ticket={ticket} locale={locale} onPreviewMedia={onPreviewMedia} />
      ))}
    </>
  );
};

export const SupportTicketListPane: React.FC<SupportTicketListPaneProps> = ({
  locale,
  tickets,
  loading,
  loadError,
  onRefresh,
  fetchAttachment = defaultFetchAttachment,
}) => {
  const { t } = useTranslation(locale);
  const [previewTicket, setPreviewTicket] = useState<LocalSupportTicket | null>(null);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span
          data-testid="support-my-requests-heading"
          style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}
        >
          {t("support.tabMyRequests", "Yêu cầu của tôi")}
        </span>
        <button type="button" data-testid="support-refresh-tickets-btn" onClick={onRefresh} style={{ display: "inline-flex", alignItems: "center", gap: "4px", padding: "4px 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)", cursor: "pointer" }}>
          <BrandIcon name="refresh" size={ICON_SIZE_XS} color="currentColor" />
          <span>{t("support.refreshBtn", "Làm mới")}</span>
        </button>
      </div>
      {loadError && (
        <div role="alert" style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-danger-soft)", color: "var(--state-danger)", fontSize: "11.5px" }}>
          {loadError}
        </div>
      )}
      <SupportTicketListBody locale={locale} tickets={tickets} loading={loading} onPreviewMedia={setPreviewTicket} />
      {previewTicket && (
        <SupportMediaLightbox
          ticket={previewTicket}
          locale={locale}
          onClose={() => setPreviewTicket(null)}
          fetchAttachment={fetchAttachment}
        />
      )}
    </div>
  );
};
