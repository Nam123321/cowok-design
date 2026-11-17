// @story 05.2
import React from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { VisualDiffResult } from "../../../lib/validations/change-proposal";
import { formatVisualDiffMetrics } from "../utils/diff-engine";

const PERCENT_MULTIPLIER = 100;
const SSIM_DECIMALS = 3;
const DELTA_DECIMALS = 2;
const SUMMARY_KEY_LEN = 20;

export interface CowokDesignDiffViewerProps {
  visualDiff: VisualDiffResult | null | undefined;
  beforeImageUrl?: string;
  afterImageUrl?: string;
  beforeSummary?: string;
  afterSummary?: string;
  onComputeDiff?: () => void;
  isComputing?: boolean;
}

function buildBboxStyle(visualDiff: VisualDiffResult): React.CSSProperties | undefined {
  const bbox = visualDiff.changedBbox;
  if (!bbox || visualDiff.width <= 0 || visualDiff.height <= 0) return undefined;
  return {
    left: `${(bbox.x / visualDiff.width) * PERCENT_MULTIPLIER}%`,
    top: `${(bbox.y / visualDiff.height) * PERCENT_MULTIPLIER}%`,
    width: `${(bbox.width / visualDiff.width) * PERCENT_MULTIPLIER}%`,
    height: `${(bbox.height / visualDiff.height) * PERCENT_MULTIPLIER}%`,
  };
}

const VisualDiffUnavailablePanel: React.FC<{
  visualDiff?: VisualDiffResult | null;
  onComputeDiff?: () => void;
  isComputing: boolean;
}> = ({ visualDiff, onComputeDiff, isComputing }) => {
  const { t } = useTranslation();
  return (
    <div className="cp-visual-diff-fallback" data-testid="visual-diff-unavailable">
      <span className="cp-gate-banner-code">{t("proposals.rasterizerUnavailableCode", "RASTERIZER_UNAVAILABLE")}</span>
      <span>{visualDiff?.reason || t("proposals.rasterizerUnavailableDesc", "Headless pixel rasterizer buffer not attached; showing structural diff summary.")}</span>
      {visualDiff?.structuralDiffSummary && visualDiff.structuralDiffSummary.length > 0 && (
        <ul className="cp-visual-diff-summary">
          {visualDiff.structuralDiffSummary.map((item, i) => (
            <li key={`${i}-${item.slice(0, SUMMARY_KEY_LEN)}`}>{item}</li>
          ))}
        </ul>
      )}
      {onComputeDiff && (
        <button type="button" className="cp-btn-rebase" data-testid="compute-visual-diff-btn" disabled={isComputing} onClick={onComputeDiff}>
          {isComputing ? t("proposals.computingDiff", "Computing...") : t("proposals.computeDiffBtn", "Run Pixel Diff")}
        </button>
      )}
    </div>
  );
};

const VisualDiffCompareGrid: React.FC<{
  beforeImageUrl?: string;
  afterImageUrl?: string;
  beforeSummary?: string;
  afterSummary?: string;
  bboxStyle?: React.CSSProperties;
}> = ({ beforeImageUrl, afterImageUrl, beforeSummary, afterSummary, bboxStyle }) => {
  const { t } = useTranslation();
  return (
    <div className="cp-visual-diff-stage">
      <div className="cp-visual-diff-pane" data-testid="visual-diff-before-pane">
        <span className="cp-visual-diff-pane-label">{t("proposals.beforeLabel", "BEFORE")}</span>
        <div className="cp-visual-diff-pane-body">
          {beforeImageUrl ? <img src={beforeImageUrl} alt={t("proposals.beforeLabel", "BEFORE")} className="cp-visual-diff-img" /> : <span>{beforeSummary || t("proposals.baselineSnapshot", "Baseline frame")}</span>}
        </div>
      </div>
      <div className="cp-visual-diff-pane" data-testid="visual-diff-after-pane">
        <span className="cp-visual-diff-pane-label">{t("proposals.afterLabel", "AFTER")}</span>
        <div className="cp-visual-diff-pane-body">
          {afterImageUrl ? <img src={afterImageUrl} alt={t("proposals.afterLabel", "AFTER")} className="cp-visual-diff-img" /> : <span>{afterSummary || t("proposals.proposedSnapshot", "Proposed frame")}</span>}
          {bboxStyle && <div className="cp-visual-diff-bbox" data-testid="visual-diff-changed-bbox" style={bboxStyle} aria-label={t("proposals.changedBboxAria", "Changed pixel region")} />}
        </div>
      </div>
    </div>
  );
};

export const CowokDesignDiffViewer: React.FC<CowokDesignDiffViewerProps> = ({
  visualDiff,
  beforeImageUrl,
  afterImageUrl,
  beforeSummary,
  afterSummary,
  onComputeDiff,
  isComputing = false,
}) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const metrics = formatVisualDiffMetrics(visualDiff);
  const bboxStyle = visualDiff ? buildBboxStyle(visualDiff) : undefined;
  const isUnavailable = !visualDiff || visualDiff.status === "RASTERIZER_UNAVAILABLE";

  return (
    <div className="cp-visual-diff" data-testid="cowok-design-diff-viewer">
      <div className="cp-visual-diff-header">
        <span className="cp-section-label">{t("proposals.visualDiffTitle", "cowok-design-diff (pixelmatch + SSIM)")}</span>
        <div className="cp-visual-diff-metrics">
          <span>{t("proposals.ssimLabel", "SSIM")}: {metrics.ssimValue !== null ? formatNumber(metrics.ssimValue, SSIM_DECIMALS) : "—"}</span>
          <span>{t("proposals.deltaLabel", "Delta")}: {metrics.deltaPercent !== null ? `${formatNumber(metrics.deltaPercent, DELTA_DECIMALS)}%` : "—"}</span>
          {visualDiff && metrics.isComputed && (
            <span>{formatNumber(visualDiff.mismatchedPixels)}/{formatNumber(visualDiff.totalPixels)} px</span>
          )}
        </div>
      </div>
      {isUnavailable ? (
        <VisualDiffUnavailablePanel visualDiff={visualDiff} onComputeDiff={onComputeDiff} isComputing={isComputing} />
      ) : (
        <VisualDiffCompareGrid beforeImageUrl={beforeImageUrl} afterImageUrl={afterImageUrl} beforeSummary={beforeSummary} afterSummary={afterSummary} bboxStyle={bboxStyle} />
      )}
    </div>
  );
};
