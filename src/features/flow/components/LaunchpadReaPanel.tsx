// @story 09.1
// @implements AC-02, AC-03, AC-04 (Silent Design-Only Firewall Ingest & Collapsible Advanced Proof Options)
import React, { useEffect, useState } from "react";
import {
  exportReaFigmaProof,
  ingestReaReference,
  ReaApiError,
} from "@/api/rea";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";
import {
  type ReaExportProofDataDto,
  type ReaIngestOutcomeDto,
  type ReaProofStatus,
  type ReaSourceFormat,
} from "@/lib/validations/rea";
import {
  FirewallVerdictBadge,
  ProofStatusBadge,
  REA_INPUT_STYLE,
} from "./LaunchpadReaLibrary";

export {
  FirewallVerdictBadge,
  LaunchpadReaSearchSection,
  ProofStatusBadge,
} from "./LaunchpadReaLibrary";

const ICON_XS = 12;
const STROKE_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const SHORT_SHA_LEN = 16;
const MAX_AUTO_SLUG_LEN = 48;
const DISABLED_OPACITY = 0.55;
const SUBTLE_OPACITY = 0.85;
const SHA256_HEX_RE = /^[a-fA-F0-9]{64}$/;

const SOURCE_FORMAT_OPTIONS: readonly ReaSourceFormat[] = [
  "url",
  "github",
  "fig",
  "clipboard",
  "zip",
  "pen",
  "wire",
];

const LICENSE_OPTIONS: readonly string[] = [
  "MIT",
  "Apache-2.0",
  "BSD-3-Clause",
  "ISC",
  "CC0-1.0",
  "LGPL-2.1",
  "MPL-2.0",
  "CC-BY-4.0",
  "UNKNOWN_PUBLIC",
  "GPL-3.0",
  "PROPRIETARY",
];

const ReaProofVerifyRow: React.FC<{
  roundtripSha: string;
  proofBusy: boolean;
  onChangeSha: (value: string) => void;
  onVerify: () => void;
}> = ({ roundtripSha, proofBusy, onChangeSha, onVerify }) => {
  const { t } = useTranslation();
  const shaReady = SHA256_HEX_RE.test(roundtripSha.trim());
  return (
    <div style={{ display: "flex", gap: "6px", flexWrap: "wrap", alignItems: "center" }}>
      <input data-testid="launchpad-rea-roundtrip-sha-input" type="text" value={roundtripSha} onChange={(e) => onChangeSha(e.target.value)} placeholder={t("rea.roundtripShaPlaceholder", "Nhập mã SHA-256 đối chiếu vòng lặp (64 ký tự hex)...")} style={{ ...REA_INPUT_STYLE, flex: "1 1 200px", fontFamily: "var(--font-mono)", fontSize: "10.5px" }} />
      <button type="button" disabled={proofBusy || !shaReady} data-testid="launchpad-rea-verify-proof-btn" onClick={onVerify} style={{ padding: "5px 10px", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", opacity: proofBusy || !shaReady ? DISABLED_OPACITY : 1, cursor: proofBusy || !shaReady ? "not-allowed" : "pointer" }}>
        {t("rea.verifyProofBtn", "Xác minh đối chiếu")}
      </button>
    </div>
  );
};

const ReaAdvancedProofPanel: React.FC<{
  outcome: ReaIngestOutcomeDto;
  proof: ReaExportProofDataDto | null;
  proofBusy: boolean;
  roundtripSha: string;
  onChangeRoundtripSha: (v: string) => void;
  onExportProof: () => void;
  onVerifyRoundtrip: () => void;
}> = ({ outcome, proof, proofBusy, roundtripSha, onChangeRoundtripSha, onExportProof, onVerifyRoundtrip }) => {
  const { t } = useTranslation();
  const activeProofStatus: ReaProofStatus = proof ? proof.proofStatus : outcome.record.proofStatus;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "6px", paddingTop: "6px", borderTop: "1px dashed var(--border-subtle)" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", flexWrap: "wrap" }}>
        <ProofStatusBadge status={activeProofStatus} />
        <button type="button" disabled={proofBusy} data-testid="launchpad-rea-export-proof-btn" onClick={onExportProof} style={{ padding: "4px 9px", fontSize: "10.5px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer" }}>
          {t("rea.exportProofBtn", "Xuất bản đối chiếu Figma")}
        </button>
      </div>
      {proof && (
        <div data-testid="launchpad-rea-proof-sha" style={{ fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
          {t("rea.checksumLabel", "Mã kiểm tra đối chiếu")}: {proof.computedSha256.slice(0, SHORT_SHA_LEN)}...
        </div>
      )}
      {proof && !proof.verified && (
        <ReaProofVerifyRow roundtripSha={roundtripSha} proofBusy={proofBusy} onChangeSha={onChangeRoundtripSha} onVerify={onVerifyRoundtrip} />
      )}
    </div>
  );
};

export interface LaunchpadReaIngestProps {
  designUrl: string;
  clipboardSnippet: string;
  payloadBase64?: string;
  detectedFormat?: ReaSourceFormat;
  selectedFileName?: string;
  externalRejectError?: { code: string; message: string } | null;
  onChangeClipboard?: (v: string) => void;
  onIngested?: (outcome: ReaIngestOutcomeDto) => void;
  onLicenseChange?: (spdx: string) => void;
}

function deriveAutoSlug(url: string, fileName?: string): string {
  const raw = fileName
    ? fileName.replace(/\.[^.]+$/, "")
    : url.replace(/^https?:\/\//i, "").split(/[/?#]/).filter(Boolean).pop() || "design-ref";
  const cleaned = raw.toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^[^a-z0-9]+|-+$/g, "").replace(/\.\.+/g, "-").slice(0, MAX_AUTO_SLUG_LEN);
  return cleaned || "design-ref";
}

function inferAutoFormat(url: string, clipboard: string, detected?: ReaSourceFormat): ReaSourceFormat {
  if (detected) return detected;
  const trimmed = url.trim().toLowerCase();
  if (trimmed.includes("github.com")) return "github";
  if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) return "url";
  if (clipboard.trim().length > 0) return "clipboard";
  return "url";
}

function mapReaIngestError(err: unknown, fallbackMessage: string): { code: string; message: string } {
  if (err instanceof ReaApiError) return { code: err.code, message: err.message };
  return { code: "REQUEST_FAILED", message: err instanceof Error ? err.message : fallbackMessage };
}

function buildIngestPayload(
  props: LaunchpadReaIngestProps,
  manualSlug: string,
  sourceFormat: ReaSourceFormat,
  licenseSpdx: string
) {
  const safeSlug = manualSlug.trim() || deriveAutoSlug(props.designUrl, props.selectedFileName);
  return {
    slug: safeSlug,
    sourceUrl: props.designUrl.trim() || `local://${safeSlug}.${sourceFormat}`,
    sourceFormat,
    licenseSpdx,
    payloadText: props.payloadBase64 ? undefined : props.clipboardSnippet || undefined,
    payloadBase64: props.payloadBase64 || undefined,
  };
}

function useReaProofController(outcome: ReaIngestOutcomeDto | null, fallbackErrMsg: string, onErr: (e: { code: string; message: string }) => void) {
  const [proofBusy, setProofBusy] = useState(false);
  const [proof, setProof] = useState<ReaExportProofDataDto | null>(null);
  const [roundtripSha, setRoundtripSha] = useState("");
  const resetProof = () => { setProof(null); setRoundtripSha(""); };
  const handleExportProof = async (submittedChecksum?: string) => {
    if (!outcome) return;
    setProofBusy(true);
    try {
      const trimmed = submittedChecksum?.trim();
      const resp = await exportReaFigmaProof({
        referenceId: outcome.record.id,
        roundtripChecksum: trimmed && trimmed.length > 0 ? trimmed : undefined,
      });
      setProof(resp.data);
    } catch (err) {
      onErr(mapReaIngestError(err, fallbackErrMsg));
    } finally {
      setProofBusy(false);
    }
  };
  return { proofBusy, proof, roundtripSha, setRoundtripSha, resetProof, handleExportProof };
}

export async function runSilentReaIngest(params: {
  designUrl: string;
  clipboardSnippet: string;
  detectedFormat?: ReaSourceFormat;
  licenseSpdx?: string;
}): Promise<ReaIngestOutcomeDto> {
  const format = inferAutoFormat(params.designUrl, params.clipboardSnippet, params.detectedFormat);
  const safeSlug = deriveAutoSlug(params.designUrl);
  const resp = await ingestReaReference({
    slug: safeSlug,
    sourceUrl: params.designUrl.trim() || `local://${safeSlug}.${format}`,
    sourceFormat: format,
    licenseSpdx: params.licenseSpdx ?? "MIT",
    payloadText: params.clipboardSnippet || undefined,
  });
  return resp.data;
}

export function formatReaIngestError(err: unknown, fallbackMessage: string): { code: string; message: string } {
  return mapReaIngestError(err, fallbackMessage);
}

function formatRealDesignDnaPreview(rawJson: string): string {
  try {
    return JSON.stringify(JSON.parse(rawJson), null, 2);
  } catch {
    return rawJson;
  }
}

function useReaIngestController(props: LaunchpadReaIngestProps, fallbackErrMsg: string) {
  const [slug, setSlug] = useState("");
  const [sourceFormat, setSourceFormatState] = useState<ReaSourceFormat>(props.detectedFormat ?? "url");
  const [formatOverridden, setFormatOverridden] = useState(false);
  const [licenseSpdx, setLicenseSpdxState] = useState("MIT");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showDnaPreview, setShowDnaPreview] = useState(false);
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<ReaIngestOutcomeDto | null>(null);
  const [rejectError, setRejectError] = useState<{ code: string; message: string } | null>(null);
  const p = useReaProofController(outcome, fallbackErrMsg, setRejectError);

  useEffect(() => {
    if (!props.detectedFormat) return;
    setSourceFormatState(props.detectedFormat);
    setFormatOverridden(false);
  }, [props.detectedFormat]);

  const effectiveFormat = formatOverridden ? sourceFormat : inferAutoFormat(props.designUrl, props.clipboardSnippet, props.detectedFormat);
  const handleIngest = async (revealDna = false) => {
    setBusy(true);
    setRejectError(null);
    p.resetProof();
    try {
      const resp = await ingestReaReference(buildIngestPayload(props, slug, effectiveFormat, licenseSpdx));
      setOutcome(resp.data);
      if (revealDna) setShowDnaPreview(true);
      props.onIngested?.(resp.data);
    } catch (err) {
      setOutcome(null);
      setShowDnaPreview(false);
      setRejectError(mapReaIngestError(err, fallbackErrMsg));
    } finally {
      setBusy(false);
    }
  };

  return {
    slug, setSlug, sourceFormat: effectiveFormat,
    setSourceFormat: (fmt: ReaSourceFormat) => { setSourceFormatState(fmt); setFormatOverridden(true); },
    licenseSpdx, setLicenseSpdx: (next: string) => { setLicenseSpdxState(next); props.onLicenseChange?.(next); },
    showAdvanced, setShowAdvanced, showDnaPreview, setShowDnaPreview, busy, outcome,
    rejectError: rejectError ?? props.externalRejectError ?? null, handleIngest, ...p,
  };
}

const ReaAdvancedOptionsDrawer: React.FC<{
  props: LaunchpadReaIngestProps;
  c: ReturnType<typeof useReaIngestController>;
}> = ({ props, c }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="launchpad-rea-advanced-panel" style={{ padding: "10px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", flexDirection: "column", gap: "8px" }}>
      {props.onChangeClipboard && (
        <textarea data-testid="launchpad-clipboard-textarea" rows={2} placeholder={t("launchpad.clipboardPlaceholder", "Dán nhanh mã SVG / HTML từ clipboard (tùy chọn)...")} value={props.clipboardSnippet} onChange={(e) => props.onChangeClipboard?.(e.target.value)} style={{ ...REA_INPUT_STYLE, fontFamily: "var(--font-mono)", fontSize: "10.5px", resize: "vertical" }} />
      )}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(110px, 1fr))", gap: "6px" }}>
        <input data-testid="launchpad-rea-slug-input" type="text" aria-label={t("rea.slugLabel", "Mã định danh mẫu")} value={c.slug} onChange={(e) => c.setSlug(e.target.value)} placeholder={t("rea.slugPlaceholder", "Tự động tạo từ tên nguồn")} style={REA_INPUT_STYLE} />
        <select data-testid="launchpad-rea-format-select" aria-label={t("rea.formatLabel", "Định dạng nguồn")} value={c.sourceFormat} onChange={(e) => c.setSourceFormat(e.target.value as ReaSourceFormat)} style={REA_INPUT_STYLE}>
          {SOURCE_FORMAT_OPTIONS.map((fmt) => (<option key={fmt} value={fmt}>{fmt}</option>))}
        </select>
        <select data-testid="launchpad-rea-license-select" aria-label={t("rea.licenseLabel", "Giấy phép (SPDX)")} value={c.licenseSpdx} onChange={(e) => c.setLicenseSpdx(e.target.value)} style={REA_INPUT_STYLE}>
          {LICENSE_OPTIONS.map((lic) => (<option key={lic} value={lic}>{lic}</option>))}
        </select>
      </div>
      {c.outcome && (
        <ReaAdvancedProofPanel outcome={c.outcome} proof={c.proof} proofBusy={c.proofBusy} roundtripSha={c.roundtripSha} onChangeRoundtripSha={c.setRoundtripSha} onExportProof={() => void c.handleExportProof()} onVerifyRoundtrip={() => void c.handleExportProof(c.roundtripSha)} />
      )}
    </div>
  );
};

const ReaIngestStatusBanners: React.FC<{
  rejectError: { code: string; message: string } | null;
  outcome: ReaIngestOutcomeDto | null;
  showDnaPreview: boolean;
}> = ({ rejectError, outcome, showDnaPreview }) => {
  const { t } = useTranslation();
  return (
    <>
      {rejectError && (
        <div role="alert" data-testid="launchpad-rea-reject-alert" style={{ padding: "8px 10px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--state-danger-soft)", border: "1px solid var(--state-danger)", color: "var(--state-danger)", fontSize: "11px", display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <FirewallVerdictBadge verdict="REJECT" />
          <span>{rejectError.message}</span>
          <span style={{ fontFamily: "var(--font-mono)", fontSize: "10px", opacity: SUBTLE_OPACITY }}>[{rejectError.code}]</span>
        </div>
      )}
      {outcome && (
        <div data-testid="launchpad-rea-ingest-result" style={{ padding: "8px 10px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-canvas)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", flexWrap: "wrap" }}>
          <div style={{ display: "inline-flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
            <FirewallVerdictBadge verdict={outcome.firewall.verdict} />
            <span style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_MEDIUM, color: "var(--text-primary)" }}>
              {outcome.record.slug} · {t("rea.readyForChat", "Sẵn sàng mở vào Chat")}
            </span>
          </div>
        </div>
      )}
      {showDnaPreview && outcome && (
        <pre data-testid="launchpad-extracted-tokens-preview" style={{ margin: 0, padding: "8px", backgroundColor: "var(--bg-canvas)", border: "1px solid var(--border-subtle)", borderRadius: "var(--radius-xs)", fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--state-success)", maxHeight: "96px", overflow: "auto" }}>
          {formatRealDesignDnaPreview(outcome.record.designDnaJson)}
        </pre>
      )}
    </>
  );
};

export const LaunchpadReaIngestSection: React.FC<LaunchpadReaIngestProps> = (props) => {
  const { t } = useTranslation();
  const c = useReaIngestController(props, t("rea.ingestError", "Không thể nhập nguồn thiết kế."));
  const handleInspectRealDna = () => {
    if (c.outcome) c.setShowDnaPreview(!c.showDnaPreview);
    else void c.handleIngest(true);
  };

  return (
    <div data-testid="launchpad-rea-ingest-section" data-ff-id="rea-firewall-ingest" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", flexWrap: "wrap" }}>
        <button type="button" disabled={c.busy} data-testid="launchpad-rea-ingest-btn" onClick={() => void c.handleIngest(false)} style={{ padding: "5px 11px", fontSize: "11px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-sm)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="shield" size={ICON_XS} strokeWidth={STROKE_MONO} color="var(--action-primary)" />
          <span>{c.busy ? t("rea.ingestingBtn", "Đang kiểm tra...") : t("rea.precheckFirewallBtn", "Kiểm tra trước qua tường lửa")}</span>
        </button>
        <div style={{ display: "inline-flex", alignItems: "center", gap: "10px" }}>
          <button type="button" disabled={c.busy} data-testid="launchpad-extract-tokens-btn" onClick={handleInspectRealDna} style={{ background: "transparent", border: "none", padding: "4px 2px", fontSize: "11px", fontWeight: FONT_WEIGHT_MEDIUM, color: "var(--text-secondary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "4px" }}>
            <BrandIcon name="cpu" size={ICON_XS} strokeWidth={STROKE_MONO} color="currentColor" />
            <span>{t("launchpad.extractTokensBtn", "Xem Design DNA")}</span>
          </button>
          <button type="button" data-testid="launchpad-rea-advanced-toggle" onClick={() => c.setShowAdvanced(!c.showAdvanced)} style={{ background: "transparent", border: "none", padding: "4px 2px", fontSize: "11px", fontWeight: FONT_WEIGHT_MEDIUM, color: "var(--text-muted)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "4px" }}>
            <span>{t("rea.advancedOptions", "Tùy chọn nâng cao")}</span>
            <BrandIcon name={c.showAdvanced ? "chevron-up" : "chevron-down"} size={ICON_XS} strokeWidth={STROKE_MONO} color="currentColor" />
          </button>
        </div>
      </div>
      <ReaIngestStatusBanners rejectError={c.rejectError} outcome={c.outcome} showDnaPreview={c.showDnaPreview} />
      {c.showAdvanced && <ReaAdvancedOptionsDrawer props={props} c={c} />}
    </div>
  );
};
