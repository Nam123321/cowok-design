// @story 14.3
// @implements AC-01, AC-05 (4-Channel Canary Rollout 5% -> 25% -> 100%, Kill-Switch, In-App Announcement & Signed Ed25519 latest.json)
import React, { useMemo, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { translateGlobal, useTranslation } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import {
  type DistributionChannel,
  isValidHttpsDownloadUrl,
  type PublishRolloutRequest,
  type ReleaseRollout,
  SHA256_HEX_REGEX,
  type SignedLatestReleaseManifest,
} from "../../../lib/validations/distribution-ops";

export interface CanaryRolloutTabProps {
  rollouts: ReleaseRollout[];
  latestManifest: SignedLatestReleaseManifest | null;
  operatorRole?: CloudAdminRole;
  submitting: boolean;
  onPublishRollout: (input: PublishRolloutRequest) => Promise<boolean>;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 15;
const MIN_KILL_REASON_LEN = 5;
const SIG_PREVIEW_LEN = 32;
const DEFAULT_ROLLOUT_PCT = 25;
const HISTORY_COL_SPAN = 5;
const RING_PRESETS = [5, 25, 100] as const;

const CARD_STYLE: React.CSSProperties = {
  backgroundColor: "var(--bg-card, var(--bg-surface))",
  border: "1px solid var(--border-default)",
  borderRadius: "10px",
  padding: "16px",
  display: "flex",
  flexDirection: "column",
  gap: "12px",
};

const INPUT_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "8px 10px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "13px",
  boxSizing: "border-box",
};

const TH_STYLE: React.CSSProperties = {
  textAlign: "left",
  padding: "8px 10px",
  fontSize: "12px",
  color: "var(--text-muted)",
  borderBottom: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-subtle)",
};

const TD_STYLE: React.CSSProperties = {
  padding: "9px 10px",
  fontSize: "12px",
  color: "var(--text-primary)",
  borderBottom: "1px solid var(--border-subtle)",
};

function RolloutHistoryRow({ rel, showKillBtn, submitting, onKillRollout }: { rel: ReleaseRollout; showKillBtn: boolean; submitting: boolean; onKillRollout: (r: ReleaseRollout) => void }) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <tr data-testid={`canary-row-${rel.channel}-${rel.version}`}>
      <td style={TD_STYLE}><strong>{rel.channel}</strong></td>
      <td style={TD_STYLE}>v{rel.version} (#{formatNumber(rel.versionSeq)})</td>
      <td style={TD_STYLE}>{formatNumber(rel.rolloutPercent)}%</td>
      <td style={TD_STYLE}>
        <div style={{ display: "inline-flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <span style={{ padding: "2px 8px", borderRadius: "999px", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD, backgroundColor: rel.killSwitch ? "var(--state-danger-soft)" : "var(--state-success-soft)", color: rel.killSwitch ? "var(--state-danger)" : "var(--state-success)" }}>
            {rel.killSwitch ? t("superadmin.ops.rolloutStatusKilled", "KILLED") : t("superadmin.ops.rolloutStatusActive", "ACTIVE")}
          </span>
          {showKillBtn && (
            <button type="button" disabled={submitting} data-testid={`canary-kill-btn-${rel.channel}-${rel.version}`} onClick={() => onKillRollout(rel)} style={{ padding: "3px 8px", borderRadius: "6px", border: "1px solid var(--state-danger)", backgroundColor: "var(--state-danger-soft)", color: "var(--state-danger)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD, cursor: submitting ? "not-allowed" : "pointer" }}>
              {t("superadmin.ops.killSwitchBtn", "Ngắt khẩn cấp (Kill-Switch)")}
            </button>
          )}
        </div>
      </td>
      <td style={TD_STYLE}>{rel.publishedBy}</td>
    </tr>
  );
}

function RolloutsHistoryTable({ rollouts, canEdit, submitting, onKillRollout }: { rollouts: ReleaseRollout[]; canEdit: boolean; submitting: boolean; onKillRollout: (rel: ReleaseRollout) => void }) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const killedKeys = useMemo(() => new Set(rollouts.filter((r) => r.killSwitch).map((r) => `${r.channel}:${r.version}`)), [rollouts]);

  return (
    <div style={{ ...CARD_STYLE, overflowX: "auto" }}>
      <strong style={{ fontSize: "13px" }}>{t("superadmin.ops.canaryHistoryTitle", "Lịch sử Phát hành 4 Kênh")} ({formatNumber(rollouts.length)})</strong>
      <table style={{ width: "100%", borderCollapse: "collapse", minWidth: "560px" }} data-testid="canary-rollouts-table">
        <thead>
          <tr>
            <th style={TH_STYLE}>{t("superadmin.ops.colChannel", "Kênh")}</th>
            <th style={TH_STYLE}>{t("superadmin.ops.colVersion", "Phiên bản")}</th>
            <th style={TH_STYLE}>{t("superadmin.ops.colPercent", "Tỷ lệ")}</th>
            <th style={TH_STYLE}>{t("superadmin.ops.colKillSwitch", "Trạng thái")}</th>
            <th style={TH_STYLE}>{t("superadmin.ops.colActor", "Người phát hành")}</th>
          </tr>
        </thead>
        <tbody>
          {rollouts.length === 0 ? (
            <tr><td colSpan={HISTORY_COL_SPAN} style={{ ...TD_STYLE, textAlign: "center", color: "var(--text-muted)" }}>{t("superadmin.ops.canaryEmpty", "Chưa có đợt phát hành Canary nào.")}</td></tr>
          ) : (
            rollouts.map((rel) => (
              <RolloutHistoryRow key={rel.id} rel={rel} showKillBtn={canEdit && !rel.killSwitch && !killedKeys.has(`${rel.channel}:${rel.version}`)} submitting={submitting} onKillRollout={onKillRollout} />
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}

export const CanaryRolloutTab: React.FC<CanaryRolloutTabProps> = ({
  rollouts,
  latestManifest,
  operatorRole,
  submitting,
  onPublishRollout,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const canEdit = operatorRole === "SUPER_ADMIN";
  const [channel, setChannel] = useState<DistributionChannel>("desktop");
  const [version, setVersion] = useState("1.5.0");
  const [rolloutPercent, setRolloutPercent] = useState<number>(DEFAULT_ROLLOUT_PCT);
  const [downloadUrl, setDownloadUrl] = useState("https://releases.cowok.design/desktop/1.5.0");
  const [artifactSha256, setArtifactSha256] = useState("");
  const [announcementText, setAnnouncementText] = useState("");
  const [killSwitch, setKillSwitch] = useState(false);
  const [reason, setReason] = useState("");

  const valid =
    version.trim().length > 0 &&
    isValidHttpsDownloadUrl(downloadUrl) &&
    SHA256_HEX_REGEX.test(artifactSha256.trim()) &&
    (!killSwitch || reason.trim().length >= MIN_KILL_REASON_LEN);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canEdit || !valid) return;
    const ok = await onPublishRollout({
      channel,
      version: version.trim(),
      rolloutPercent,
      killSwitch,
      downloadUrl: downloadUrl.trim(),
      artifactSha256: artifactSha256.trim(),
      announcement: announcementText.trim() ? { vi: announcementText.trim(), en: announcementText.trim() } : {},
      notes: announcementText.trim(),
      reason: reason.trim() || undefined,
    });
    if (ok) {
      setAnnouncementText("");
      setReason("");
      setKillSwitch(false);
    }
  };

  const handleKillRollout = (rel: ReleaseRollout) => {
    void onPublishRollout({
      channel: rel.channel,
      version: rel.version,
      rolloutPercent: rel.rolloutPercent,
      killSwitch: true,
      downloadUrl: rel.downloadUrl,
      artifactSha256: rel.artifactSha256,
      announcement: rel.announcement,
      notes: rel.notes,
      reason: translateGlobal(
        "superadmin.ops.canaryKillDefaultReason",
        "Triggered instant Canary Kill-Switch from SuperAdmin console"
      ),
    });
  };

  return (
    <section
      data-ff-id="superadmin-canary-rollout-panel"
      data-testid="superadmin-panel-distribution"
      style={{ display: "flex", flexDirection: "column", gap: "14px" }}
    >
      <div style={CARD_STYLE}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <div style={{ display: "inline-flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="deploy" size={ICON_SIZE_SM} color="var(--action-primary)" />
            <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.ops.canaryTitle", "Điều phối Phát hành Canary 4 Kênh & Ký số Ed25519")}
            </h2>
          </div>
          <span style={{ padding: "3px 10px", borderRadius: "999px", backgroundColor: "var(--state-success-soft)", color: "var(--state-success)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
            {latestManifest
              ? t("superadmin.ops.canaryManifestSeqBadge", "latest.json · v{{version}} (chuỗi #{{seq}})", {
                  version: latestManifest.payload.version,
                  seq: formatNumber(latestManifest.manifestSeq),
                })
              : "latest.json · Ed25519"}
          </span>
        </div>
        {latestManifest && (
          <code data-testid="canary-signed-fingerprint" style={{ fontSize: "11px", padding: "6px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)", overflowWrap: "anywhere" }}>
            {t("superadmin.ops.keyLabel", "Khóa")}: {latestManifest.keyId} · {t("superadmin.ops.sigLabel", "Chữ ký")}: {latestManifest.signatureBase64.slice(0, SIG_PREVIEW_LEN)}…
          </code>
        )}
      </div>

      <form onSubmit={handleSubmit} style={CARD_STYLE} data-testid="canary-publish-form">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <strong style={{ fontSize: "13px" }}>
            {t("superadmin.ops.canaryFormTitle", "Xuất bản Phiên bản & Tỷ lệ Canary (5% → 25% → 100%)")}
          </strong>
          {!canEdit && (
            <span data-testid="canary-readonly-lock" style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.ops.superAdminOnly", "Chỉ SUPER_ADMIN được phát hành")}
            </span>
          )}
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: "10px" }}>
          <select disabled={!canEdit} value={channel} onChange={(e) => setChannel(e.target.value as DistributionChannel)} data-testid="canary-channel-select" style={INPUT_STYLE}>
            <option value="desktop">{t("superadmin.ops.channelDesktop", "Desktop App (Tauri)")}</option>
            <option value="cli">{t("superadmin.ops.channelCli", "CLI Core")}</option>
            <option value="plugin">{t("superadmin.ops.channelPlugin", "IDE & Host Plugins")}</option>
            <option value="docker">{t("superadmin.ops.channelDocker", "Docker Enterprise")}</option>
          </select>
          <input type="text" disabled={!canEdit} value={version} onChange={(e) => setVersion(e.target.value)} data-testid="canary-version-input" placeholder="1.5.0" style={INPUT_STYLE} />
          <input type="url" disabled={!canEdit} value={downloadUrl} onChange={(e) => setDownloadUrl(e.target.value)} data-testid="canary-url-input" placeholder="https://releases.cowok.design/..." style={INPUT_STYLE} />
          <input type="text" disabled={!canEdit} value={artifactSha256} onChange={(e) => setArtifactSha256(e.target.value)} data-testid="canary-sha256-input" placeholder={t("superadmin.ops.canarySha256Placeholder", "SHA-256 (64 ký tự hex)")} style={INPUT_STYLE} />
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <span style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
            {t("superadmin.ops.canaryRingLabel", "Vòng triển khai")}: <strong>{formatNumber(rolloutPercent)}%</strong>
          </span>
          {RING_PRESETS.map((pct) => (
            <button
              key={pct}
              type="button"
              disabled={!canEdit}
              data-testid={`canary-ring-${pct}`}
              onClick={() => setRolloutPercent(pct)}
              style={{
                padding: "4px 10px",
                borderRadius: "999px",
                border: `1px solid ${rolloutPercent === pct ? "var(--action-primary)" : "var(--border-default)"}`,
                backgroundColor: rolloutPercent === pct ? "var(--bg-selected)" : "var(--bg-subtle)",
                color: rolloutPercent === pct ? "var(--action-primary)" : "var(--text-secondary)",
                fontSize: "12px",
                fontWeight: FONT_WEIGHT_SEMIBOLD,
                cursor: canEdit ? "pointer" : "not-allowed",
              }}
            >
              {formatNumber(pct)}%
            </button>
          ))}
        </div>
        <input type="text" disabled={!canEdit} value={announcementText} onChange={(e) => setAnnouncementText(e.target.value)} data-testid="canary-announcement-input" placeholder={t("superadmin.ops.canaryAnnouncementPlaceholder", "Thông báo hiển thị trong ứng dụng (tùy chọn)")} style={INPUT_STYLE} />
        <label style={{ display: "inline-flex", alignItems: "center", gap: "8px", fontSize: "12px", color: killSwitch ? "var(--state-danger)" : "var(--text-secondary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
          <input type="checkbox" disabled={!canEdit} checked={killSwitch} onChange={(e) => setKillSwitch(e.target.checked)} data-testid="canary-killswitch-check" />
          <span>{t("superadmin.ops.canaryKillSwitchLabel", "Bật Kill-Switch (chặn cập nhật ngay lập tức cho phiên bản này)")}</span>
        </label>
        {killSwitch && (
          <input type="text" disabled={!canEdit} value={reason} onChange={(e) => setReason(e.target.value)} data-testid="canary-reason-input" placeholder={t("superadmin.ops.canaryReasonPlaceholder", "Lý do bắt buộc kích hoạt Kill-Switch (>= 5 ký tự)")} style={INPUT_STYLE} />
        )}
        <button
          type="submit"
          disabled={!canEdit || submitting || !valid}
          data-testid="canary-submit-btn"
          style={{
            alignSelf: "flex-start",
            height: "34px",
            padding: "0 14px",
            borderRadius: "8px",
            border: "none",
            backgroundColor: canEdit && valid ? "var(--action-primary)" : "var(--bg-subtle)",
            color: canEdit && valid ? "var(--action-primary-fg)" : "var(--text-disabled)",
            fontSize: "13px",
            fontWeight: FONT_WEIGHT_SEMIBOLD,
            cursor: canEdit && valid ? "pointer" : "not-allowed",
          }}
        >
          {t("superadmin.ops.canarySubmitBtn", "Phát hành & Ký Ed25519 latest.json")}
        </button>
      </form>

      <RolloutsHistoryTable
        rollouts={rollouts}
        canEdit={canEdit}
        submitting={submitting}
        onKillRollout={handleKillRollout}
      />
    </section>
  );
};
