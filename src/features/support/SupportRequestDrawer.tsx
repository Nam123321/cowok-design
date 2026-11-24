// @story 14.1
// @implements AC-01, AC-02, AC-04 (In-App Non-Tech Support/Request Drawer — RightDrawer [5.25] + CowokToast [5.12] + Zero-PII Preview)
import React, { useCallback, useEffect, useRef, useState } from "react";
import { apiSupport, type SupportApiClient } from "../../api/support";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { CowokToast, type CowokToastTone } from "../../components/ui/CowokToast";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import {
  DEFAULT_SUPPORT_TICKETS_LIMIT,
  MAX_TICKET_DESC_LEN,
  MAX_TICKET_TITLE_LEN,
  MIN_TICKET_DESC_LEN,
  MIN_TICKET_TITLE_LEN,
  SCREENSHOT_DATA_URL_RE,
  type LocalSupportTicket,
  type SupportCategory,
} from "../../lib/validations/support";
import {
  FONT_WEIGHT_MEDIUM,
  FONT_WEIGHT_SEMIBOLD,
  formatSupportErrorMessage,
  ICON_SIZE_MD,
  ICON_SIZE_SM,
  SUPPORT_FIELD_STYLE,
  SupportCategoryGrid,
  SupportDiagnosticsPanel,
  SupportTicketListPane,
} from "./SupportTicketListPane";

const MAX_UPLOAD_BYTES = 5_000_000;
const BYTES_PER_KB = 1_000;
const BYTES_PER_MB = 1_000_000;
const DRAWER_Z_INDEX = 9990;
const BACKDROP_Z_INDEX = DRAWER_Z_INDEX - 1;
const TEXTAREA_ROWS = 4;
const OPACITY_SUBMITTING = 0.7;
const OPACITY_IDLE = 1;
const MEDIA_ACCEPT_ATTR = "image/png,image/jpeg,image/webp,video/mp4,video/webm";
const ALLOWED_MEDIA_TYPES = new Set([
  "image/png",
  "image/jpeg",
  "image/webp",
  "video/mp4",
  "video/webm",
]);

interface SupportMediaAttachment {
  dataUrl: string;
  fileName: string;
  fileType: string;
  fileSize: number;
}

export interface SupportRequestDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  locale?: string;
  activeSkills?: string[];
  recentErrorCodes?: string[];
  client?: Pick<SupportApiClient, "createSupportTicket" | "listSupportTickets">;
}

interface SupportTicketDraft {
  category: SupportCategory;
  title: string;
  description: string;
  contactEmail: string;
  screenshotDataUrl?: string;
  includeDiagnostics: boolean;
}

const DRAWER_SHELL_STYLE: React.CSSProperties = {
  position: "fixed",
  top: 0,
  right: 0,
  bottom: 0,
  width: "380px",
  maxWidth: "100vw",
  backgroundColor: "var(--bg-surface)",
  borderLeft: "1px solid var(--border-subtle)",
  boxShadow: "var(--shadow-lg)",
  zIndex: DRAWER_Z_INDEX,
  display: "flex",
  flexDirection: "column",
  fontFamily: "var(--font-sans)",
};

const CLOSE_BTN_STYLE: React.CSSProperties = {
  background: "transparent",
  border: "1px solid var(--border-subtle)",
  borderRadius: "var(--radius-xs)",
  width: "26px",
  height: "26px",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  color: "var(--text-secondary)",
  cursor: "pointer",
};

const LABEL_STYLE: React.CSSProperties = {
  fontSize: "11.5px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  color: "var(--text-secondary)",
};

const SUB_LABEL_STYLE: React.CSSProperties = {
  fontSize: "11px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  color: "var(--text-secondary)",
};

function buildTabStyle(selected: boolean): React.CSSProperties {
  return {
    padding: "10px 8px",
    fontSize: "11.5px",
    fontWeight: selected ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
    color: selected ? "var(--text-primary)" : "var(--text-secondary)",
    backgroundColor: selected ? "var(--bg-selected)" : "transparent",
    border: "none",
    borderBottom: selected ? "2px solid var(--action-primary)" : "2px solid transparent",
    cursor: "pointer",
  };
}

function formatAttachmentSize(bytes: number): string {
  if (bytes >= BYTES_PER_MB) return `${(bytes / BYTES_PER_MB).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / BYTES_PER_KB))} KB`;
}

const UPLOAD_BTN_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "6px",
  width: "100%",
  padding: "8px 10px",
  borderRadius: "var(--radius-xs)",
  border: "1px dashed var(--action-primary)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "11.5px",
  fontWeight: FONT_WEIGHT_MEDIUM,
  cursor: "pointer",
};

const PREVIEW_CHIP_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "8px",
  padding: "6px 8px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
};

const MEDIA_THUMB_STYLE: React.CSSProperties = {
  width: "36px",
  height: "36px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  objectFit: "cover",
  flexShrink: 0,
};

const VIDEO_BADGE_STYLE: React.CSSProperties = {
  width: "36px",
  height: "36px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-surface)",
  color: "var(--action-primary)",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  flexShrink: 0,
};

const SupportMediaPreviewChip: React.FC<{
  attachment: SupportMediaAttachment;
  removeLabel: string;
  onRemove: () => void;
}> = ({ attachment, removeLabel, onRemove }) => {
  const isVideo = attachment.fileType.startsWith("video/");
  return (
    <div data-testid="support-media-preview-chip" style={PREVIEW_CHIP_STYLE}>
      {isVideo ? (
        <span data-testid="support-media-preview-video" style={VIDEO_BADGE_STYLE}>
          <BrandIcon name="play" size={ICON_SIZE_SM} color="currentColor" />
        </span>
      ) : (
        <img
          src={attachment.dataUrl}
          alt={attachment.fileName}
          data-testid="support-media-preview-img"
          style={MEDIA_THUMB_STYLE}
        />
      )}
      <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: "2px" }}>
        <span style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {attachment.fileName}
        </span>
        <span style={{ fontSize: "10px", color: "var(--text-secondary)", fontFamily: "var(--font-mono)" }}>
          {`${isVideo ? "VIDEO" : "IMG"} · ${formatAttachmentSize(attachment.fileSize)}`}
        </span>
      </div>
      <button type="button" onClick={onRemove} aria-label={removeLabel} data-testid="support-media-remove-btn" style={CLOSE_BTN_STYLE}>
        <BrandIcon name="close" size={ICON_SIZE_SM} color="currentColor" />
      </button>
    </div>
  );
};

interface MediaAndEmailFieldProps {
  locale: string;
  contactEmail: string;
  attachment: SupportMediaAttachment | null;
  onEmailChange: (val: string) => void;
  onFileChange: (evt: React.ChangeEvent<HTMLInputElement>) => void;
  onClearMedia: () => void;
}

const EmailAndScreenshotRow: React.FC<MediaAndEmailFieldProps> = ({
  locale, contactEmail, attachment, onEmailChange, onFileChange, onClearMedia,
}) => {
  const { t } = useTranslation(locale);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const handleRemove = () => {
    if (fileInputRef.current) fileInputRef.current.value = "";
    onClearMedia();
  };
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
        <label htmlFor="support-email-input" style={SUB_LABEL_STYLE}>
          {t("support.emailLabel", "Email nhận phản hồi (tùy chọn)")}
        </label>
        <input
          id="support-email-input" data-testid="support-email-input" type="email"
          value={contactEmail} onChange={(e) => onEmailChange(e.target.value)}
          placeholder={t("support.emailPlaceholder", "ten@congty.com")} style={SUPPORT_FIELD_STYLE}
        />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        <label htmlFor="support-screenshot-input" style={SUB_LABEL_STYLE}>
          {t("support.mediaLabel", "Ảnh / video minh họa (tùy chọn, ≤ 5 MB)")}
        </label>
        <input
          ref={fileInputRef} id="support-screenshot-input" data-testid="support-screenshot-input"
          type="file" accept={MEDIA_ACCEPT_ATTR} onChange={onFileChange} style={{ display: "none" }}
        />
        <button type="button" data-testid="support-media-upload-btn" onClick={() => fileInputRef.current?.click()} style={UPLOAD_BTN_STYLE}>
          <BrandIcon name="upload" size={ICON_SIZE_SM} color="var(--action-primary)" />
          <span>{t("support.mediaUploadBtn", "Đính kèm ảnh / video (≤ 5 MB)")}</span>
        </button>
        {attachment && (
          <SupportMediaPreviewChip attachment={attachment} removeLabel={t("support.removeMedia", "Xóa tệp đính kèm")} onRemove={handleRemove} />
        )}
      </div>
    </div>
  );
};

interface TextFieldsProps {
  locale: string;
  title: string;
  description: string;
  onTitleChange: (val: string) => void;
  onDescriptionChange: (val: string) => void;
}

const SupportTextInputs: React.FC<TextFieldsProps> = ({
  locale,
  title,
  description,
  onTitleChange,
  onDescriptionChange,
}) => {
  const { t } = useTranslation(locale);
  return (
    <>
      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
        <label htmlFor="support-title-input" style={LABEL_STYLE}>
          {t("support.titleLabel", "Tiêu đề ngắn gọn")}
        </label>
        <input
          id="support-title-input"
          data-testid="support-title-input"
          type="text"
          maxLength={MAX_TICKET_TITLE_LEN}
          value={title}
          onChange={(e) => onTitleChange(e.target.value)}
          placeholder={t("support.titlePlaceholder", "Ví dụ: Không xuất được trang xem trước")}
          style={SUPPORT_FIELD_STYLE}
        />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
        <label htmlFor="support-description-input" style={LABEL_STYLE}>
          {t("support.descriptionLabel", "Mô tả chi tiết")}
        </label>
        <textarea
          id="support-description-input"
          data-testid="support-description-input"
          rows={TEXTAREA_ROWS}
          maxLength={MAX_TICKET_DESC_LEN}
          value={description}
          onChange={(e) => onDescriptionChange(e.target.value)}
          placeholder={t("support.descriptionPlaceholder", "Kể lại bước bạn vừa làm hoặc tính năng bạn mong muốn...")}
          style={{ ...SUPPORT_FIELD_STYLE, resize: "vertical", minHeight: "88px" }}
        />
      </div>
    </>
  );
};

function readScreenshotUpload(opts: {
  evt: React.ChangeEvent<HTMLInputElement>;
  t: (key: string, defaultVal?: string) => string;
  onClear: () => void;
  onError: (msg: string | null) => void;
  onLoaded: (attachment: SupportMediaAttachment) => void;
}): void {
  const { evt, t, onClear, onError, onLoaded } = opts;
  const file = evt.target.files?.[0];
  if (!file) { onClear(); return; }
  const invalidMsg = t("support.mediaInvalidType", "Chỉ hỗ trợ ảnh (PNG, JPEG, WebP) hoặc video ngắn (MP4, WebM).");
  if (!ALLOWED_MEDIA_TYPES.has(file.type)) {
    onClear();
    evt.target.value = "";
    onError(invalidMsg);
    return;
  }
  if (file.size > MAX_UPLOAD_BYTES) {
    onClear();
    evt.target.value = "";
    onError(t("support.mediaTooLarge", "Tệp đính kèm vượt quá giới hạn 5 MB."));
    return;
  }
  onError(null);
  const reader = new FileReader();
  reader.onerror = () => {
    onClear();
    onError(t("support.mediaReadFailed", "Không thể đọc tệp đính kèm."));
  };
  reader.onload = () => {
    if (typeof reader.result === "string" && SCREENSHOT_DATA_URL_RE.test(reader.result)) {
      onLoaded({ dataUrl: reader.result, fileName: file.name, fileType: file.type, fileSize: file.size });
      return;
    }
    onClear();
    onError(invalidMsg);
  };
  reader.readAsDataURL(file);
}

interface SupportSubmitFormProps {
  locale: string;
  activeSkills: string[];
  recentErrorCodes: string[];
  isSubmitting: boolean;
  onSubmit: (payload: SupportTicketDraft) => Promise<boolean>;
}

function useSupportFormState(
  t: (key: string, defaultVal?: string) => string,
  onSubmit: (payload: SupportTicketDraft) => Promise<boolean>
) {
  const [category, setCategory] = useState<SupportCategory>("bug_report");
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [contactEmail, setContactEmail] = useState("");
  const [attachment, setAttachment] = useState<SupportMediaAttachment | null>(null);
  const [includeDiagnostics, setIncludeDiagnostics] = useState(true);
  const [showDiagPreview, setShowDiagPreview] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const clearAttachment = () => setAttachment(null);
  const handleFileChange = (evt: React.ChangeEvent<HTMLInputElement>) =>
    readScreenshotUpload({ evt, t, onClear: clearAttachment, onError: setFormError, onLoaded: setAttachment });

  const handleSubmit = async (evt: React.FormEvent) => {
    evt.preventDefault();
    const trimmedTitle = title.trim();
    const trimmedDesc = description.trim();
    if (trimmedTitle.length < MIN_TICKET_TITLE_LEN) {
      setFormError(t("support.titleTooShort", "Vui lòng nhập tiêu đề ít nhất 3 ký tự."));
      return;
    }
    if (trimmedDesc.length < MIN_TICKET_DESC_LEN) {
      setFormError(t("support.descriptionTooShort", "Vui lòng mô tả chi tiết ít nhất 5 ký tự."));
      return;
    }
    setFormError(null);
    const ok = await onSubmit({
      category, title: trimmedTitle, description: trimmedDesc,
      contactEmail: contactEmail.trim(), screenshotDataUrl: attachment?.dataUrl, includeDiagnostics,
    });
    if (ok) { setTitle(""); setDescription(""); clearAttachment(); }
  };

  return {
    category, setCategory, title, setTitle, description, setDescription,
    contactEmail, setContactEmail, attachment, clearAttachment, includeDiagnostics,
    setIncludeDiagnostics, showDiagPreview, setShowDiagPreview, formError,
    handleFileChange, handleSubmit,
  };
}

const SupportSubmitForm: React.FC<SupportSubmitFormProps> = ({
  locale, activeSkills, recentErrorCodes, isSubmitting, onSubmit,
}) => {
  const { t } = useTranslation(locale);
  const form = useSupportFormState(t, onSubmit);
  const submitBtnStyle: React.CSSProperties = {
    height: "36px", padding: "0 14px", borderRadius: "var(--radius-xs)", border: "none",
    backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)",
    fontSize: "12.5px", fontWeight: FONT_WEIGHT_SEMIBOLD,
    cursor: isSubmitting ? "not-allowed" : "pointer",
    opacity: isSubmitting ? OPACITY_SUBMITTING : OPACITY_IDLE,
    display: "inline-flex", alignItems: "center", justifyContent: "center", gap: "6px",
  };
  return (
    <form onSubmit={(e) => void form.handleSubmit(e)} style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <SupportCategoryGrid category={form.category} locale={locale} onSelect={form.setCategory} />
      <SupportTextInputs locale={locale} title={form.title} description={form.description} onTitleChange={form.setTitle} onDescriptionChange={form.setDescription} />
      <EmailAndScreenshotRow locale={locale} contactEmail={form.contactEmail} attachment={form.attachment} onEmailChange={form.setContactEmail} onFileChange={form.handleFileChange} onClearMedia={form.clearAttachment} />
      <SupportDiagnosticsPanel
        locale={locale} includeDiagnostics={form.includeDiagnostics} showDiagPreview={form.showDiagPreview}
        recentErrorCodes={recentErrorCodes} activeSkills={activeSkills}
        onToggleInclude={form.setIncludeDiagnostics} onTogglePreview={() => form.setShowDiagPreview((prev) => !prev)}
      />
      {form.formError && (
        <div role="alert" data-testid="support-form-error" style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-danger-soft)", color: "var(--state-danger)", fontSize: "11.5px" }}>
          {form.formError}
        </div>
      )}
      <button type="submit" data-testid="support-submit-btn" disabled={isSubmitting} style={submitBtnStyle}>
        <BrandIcon name="send" size={ICON_SIZE_SM} color="currentColor" />
        <span>{isSubmitting ? t("support.submittingBtn", "Đang gửi...") : t("support.submitBtn", "Gửi yêu cầu")}</span>
      </button>
    </form>
  );
};

const SupportDrawerHeader: React.FC<{ locale: string; onClose: () => void }> = ({ locale, onClose }) => {
  const { t } = useTranslation(locale);
  return (
    <header style={{ padding: "14px 16px", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: "10px" }}>
      <div style={{ display: "flex", flexDirection: "column", gap: "3px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "7px", fontSize: "14px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
          <BrandIcon name="help" size={ICON_SIZE_MD} color="var(--action-primary)" />
          <span>{t("support.drawerTitle", "Trợ giúp & Góp ý")}</span>
        </div>
        <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
          {t("support.noGithubSubtitle", "Gửi trực tiếp trong ứng dụng — không cần tài khoản GitHub")}
        </span>
      </div>
      <button type="button" onClick={onClose} aria-label={t("common.close", "Đóng")} data-testid="support-drawer-close-btn" style={CLOSE_BTN_STYLE}>
        <BrandIcon name="close" size={ICON_SIZE_SM} color="currentColor" />
      </button>
    </header>
  );
};

const SupportDrawerTabBar: React.FC<{
  locale: string;
  activeTab: "submit" | "my_requests";
  ticketCount: number;
  onSelectTab: (tab: "submit" | "my_requests") => void;
}> = ({ locale, activeTab, ticketCount, onSelectTab }) => {
  const { t } = useTranslation(locale);
  const fmt = useFormatter({ locale });
  return (
    <div role="tablist" style={{ display: "grid", gridTemplateColumns: "1fr 1fr", borderBottom: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-subtle)" }}>
      <button type="button" role="tab" aria-selected={activeTab === "submit"} data-testid="support-tab-submit" onClick={() => onSelectTab("submit")} style={buildTabStyle(activeTab === "submit")}>
        {t("support.tabSubmit", "Gửi phản hồi")}
      </button>
      <button type="button" role="tab" aria-selected={activeTab === "my_requests"} data-testid="support-tab-my-requests" onClick={() => onSelectTab("my_requests")} style={buildTabStyle(activeTab === "my_requests")}>
        {`${t("support.tabMyRequests", "Yêu cầu của tôi")} (${fmt.formatNumber(ticketCount)})`}
      </button>
    </div>
  );
};

function useDrawerEscapeKey(isOpen: boolean, onClose: () => void): void {
  useEffect(() => {
    if (!isOpen) return;
    const onKeyDown = (evt: KeyboardEvent) => { if (evt.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, onClose]);
}

interface DrawerControllerOpts {
  isOpen: boolean;
  onClose: () => void;
  resolvedLocale: string;
  activeSkills: string[];
  recentErrorCodes: string[];
  client: Pick<SupportApiClient, "createSupportTicket" | "listSupportTickets">;
  t: (key: string, defaultVal?: string) => string;
}

function useSupportDrawerController(opts: DrawerControllerOpts) {
  const { isOpen, onClose, resolvedLocale, activeSkills, recentErrorCodes, client, t } = opts;
  const [activeTab, setActiveTab] = useState<"submit" | "my_requests">("submit");
  const [tickets, setTickets] = useState<LocalSupportTicket[]>([]);
  const [loadingTickets, setLoadingTickets] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [toast, setToast] = useState<{ tone: CowokToastTone; message: string } | null>(null);
  const loadSeqRef = useRef(0);

  const loadTickets = useCallback(async () => {
    const seq = loadSeqRef.current + 1;
    loadSeqRef.current = seq;
    setLoadingTickets(true);
    setLoadError(null);
    try {
      const resp = await client.listSupportTickets(DEFAULT_SUPPORT_TICKETS_LIMIT);
      if (loadSeqRef.current === seq) setTickets(resp.data.tickets);
    } catch (err) {
      if (loadSeqRef.current === seq) setLoadError(formatSupportErrorMessage(err, t));
    } finally {
      if (loadSeqRef.current === seq) setLoadingTickets(false);
    }
  }, [client, t]);

  useEffect(() => { if (isOpen) void loadTickets(); }, [isOpen, loadTickets]);
  useDrawerEscapeKey(isOpen, onClose);

  const handleSubmitTicket = async (draft: SupportTicketDraft): Promise<boolean> => {
    setIsSubmitting(true);
    try {
      const resp = await client.createSupportTicket({ ...draft, channel: "desktop", locale: resolvedLocale, recentErrorCodes, activeSkills });
      setTickets((prev) => [resp.data, ...prev.filter((item) => item.id !== resp.data.id)]);
      setActiveTab("my_requests");
      setToast({ tone: "success", message: `${t("support.submitSuccessPrefix", "Đã gửi yêu cầu thành công")} (${resp.data.ticketCode})` });
      return true;
    } catch (err) {
      setToast({ tone: "danger", message: formatSupportErrorMessage(err, t) });
      return false;
    } finally {
      setIsSubmitting(false);
    }
  };

  return { activeTab, setActiveTab, tickets, loadingTickets, loadError, isSubmitting, toast, setToast, loadTickets, handleSubmitTicket };
}

export const SupportRequestDrawer: React.FC<SupportRequestDrawerProps> = ({
  isOpen, onClose, locale, activeSkills = ["design-craft"], recentErrorCodes = [], client = apiSupport,
}) => {
  const { t, locale: activeLocale } = useTranslation(locale);
  const resolvedLocale = locale || activeLocale;
  const ctrl = useSupportDrawerController({ isOpen, onClose, resolvedLocale, activeSkills, recentErrorCodes, client, t });
  if (!isOpen) return null;
  return (
    <>
      <div role="presentation" onClick={onClose} style={{ position: "fixed", inset: 0, backgroundColor: "var(--bg-overlay)", zIndex: BACKDROP_Z_INDEX }} />
      <aside role="dialog" aria-modal="true" aria-label={t("support.drawerTitle", "Trợ giúp & Góp ý")} data-testid="support-request-drawer" style={DRAWER_SHELL_STYLE}>
        <SupportDrawerHeader locale={resolvedLocale} onClose={onClose} />
        <SupportDrawerTabBar locale={resolvedLocale} activeTab={ctrl.activeTab} ticketCount={ctrl.tickets.length} onSelectTab={ctrl.setActiveTab} />
        <div style={{ flex: 1, overflowY: "auto", padding: "14px 16px" }}>
          {ctrl.activeTab === "submit" ? (
            <SupportSubmitForm locale={resolvedLocale} activeSkills={activeSkills} recentErrorCodes={recentErrorCodes} isSubmitting={ctrl.isSubmitting} onSubmit={ctrl.handleSubmitTicket} />
          ) : (
            <SupportTicketListPane locale={resolvedLocale} tickets={ctrl.tickets} loading={ctrl.loadingTickets} loadError={ctrl.loadError} onRefresh={() => void ctrl.loadTickets()} />
          )}
        </div>
      </aside>
      {ctrl.toast && <CowokToast tone={ctrl.toast.tone} message={ctrl.toast.message} testId="support-drawer-toast" onDismiss={() => ctrl.setToast(null)} />}
    </>
  );
};
