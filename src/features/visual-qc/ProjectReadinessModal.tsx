// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04 (Project Readiness Dashboard Modal)
import React, { useEffect } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  FlowCompletenessReport,
  QcIssue,
  QcRunResponseData,
  QcVerdict,
} from "../../lib/validations/visual-qc";

const ICON_SM = 14;
const ICON_MD = 16;
const STROKE_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const PERCENT_MULTIPLIER = 100;
const MODAL_Z_INDEX = 9999;
const COMPACT_LINE_HEIGHT = 1.25;
const LONG_VALUE_THRESHOLD = 10;

type TranslateFn = (key: string, defaultVal?: string) => string;
type ReadinessVerdictKey = QcVerdict | "PENDING";

interface VerdictPresentation {
  label: string;
  accent?: string;
}

interface IssueRuleCopy {
  title: string;
  hint: string;
}

interface SeverityPillStyle {
  label: string;
  color: string;
  backgroundColor: string;
  borderColor: string;
}

const RULE_COPY_MAP: ReadonlyArray<{
  prefixes: readonly string[];
  titleKey: string;
  titleDefault: string;
  hintKey: string;
  hintDefault: string;
}> = [
  {
    prefixes: ["R26_"],
    titleKey: "visualQc.ruleR26Title",
    titleDefault: "Chưa có trình duyệt kiểm tra ảnh chụp",
    hintKey: "visualQc.ruleR26Hint",
    hintDefault: "Hãy cài Chrome hoặc Chromium cục bộ để bật kiểm tra hiển thị thực tế đa màn hình.",
  },
  {
    prefixes: ["R01_"],
    titleKey: "visualQc.ruleR01Title",
    titleDefault: "Các thành phần bị chồng lấn",
    hintKey: "visualQc.ruleR01Hint",
    hintDefault: "Một số khối nội dung đang đè lên nhau — hãy tăng khoảng cách hoặc điều chỉnh bố cục.",
  },
  {
    prefixes: ["R02_", "R03_"],
    titleKey: "visualQc.ruleOverflowTitle",
    titleDefault: "Nội dung bị tràn khung hình",
    hintKey: "visualQc.ruleOverflowHint",
    hintDefault: "Một phần nội dung vượt quá kích thước màn hình — hãy điều chỉnh lại khoảng cách hoặc kích thước.",
  },
  {
    prefixes: ["R16_", "R07_", "R08_"],
    titleKey: "visualQc.ruleR16Title",
    titleDefault: "Độ tương phản chữ chưa đủ rõ",
    hintKey: "visualQc.ruleR16Hint",
    hintDefault: "Màu chữ và màu nền quá gần nhau — hãy tăng độ tương phản để người xem dễ đọc hơn.",
  },
  {
    prefixes: ["R17_", "R11_"],
    titleKey: "visualQc.ruleR17Title",
    titleDefault: "Khung nhúng chưa được bảo vệ an toàn",
    hintKey: "visualQc.ruleR17Hint",
    hintDefault: "Khung nhúng bên ngoài cần bật chế độ hộp cát để bảo vệ trang khỏi mã không mong muốn.",
  },
];

function resolveVerdictPresentation(verdict: QcVerdict | undefined, t: TranslateFn): VerdictPresentation {
  if (verdict === "PASS") {
    return { label: t("visualQc.verdictPassFriendly", "Đạt chuẩn hiển thị"), accent: "var(--state-success)" };
  }
  if (verdict === "FAIL") {
    return { label: t("visualQc.verdictFailFriendly", "Cần sửa trước khi xuất"), accent: "var(--state-danger)" };
  }
  if (verdict === "UNVERIFIED_NO_BROWSER") {
    return { label: t("visualQc.verdictUnverifiedFriendly", "Chưa kiểm tra trình duyệt"), accent: "var(--state-warning)" };
  }
  return { label: t("visualQc.verdictPendingFriendly", "Chưa kiểm tra") };
}

function resolveSeverityStyle(severity: QcIssue["severity"], t: TranslateFn): SeverityPillStyle {
  if (severity === "P0") {
    return {
      label: t("visualQc.severityP0Label", "Lỗi cần sửa"),
      color: "var(--state-danger)",
      backgroundColor: "var(--state-danger-soft)",
      borderColor: "var(--state-danger)",
    };
  }
  if (severity === "P1") {
    return {
      label: t("visualQc.severityP1Label", "Gợi ý cải thiện"),
      color: "var(--state-warning)",
      backgroundColor: "var(--state-warning-soft)",
      borderColor: "var(--state-warning)",
    };
  }
  return {
    label: t("visualQc.severityP2Label", "Lưu ý nhỏ"),
    color: "var(--state-info)",
    backgroundColor: "var(--state-info-soft)",
    borderColor: "var(--state-info)",
  };
}

function resolveIssueCopy(ruleId: string, t: TranslateFn): IssueRuleCopy {
  const matched = RULE_COPY_MAP.find((entry) => entry.prefixes.some((prefix) => ruleId.startsWith(prefix)));
  if (matched) {
    return { title: t(matched.titleKey, matched.titleDefault), hint: t(matched.hintKey, matched.hintDefault) };
  }
  return {
    title: t("visualQc.ruleGenericTitle", "Cần điều chỉnh hiển thị"),
    hint: t(
      "visualQc.ruleGenericHint",
      "Phát hiện điểm chưa tối ưu về bố cục hoặc trải nghiệm — mở chi tiết kỹ thuật bên dưới để kiểm tra."
    ),
  };
}

function useEscapeClose(open: boolean, onClose: () => void): void {
  useEffect(() => {
    if (!open || typeof window === "undefined") return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, onClose]);
}

export interface ProjectReadinessModalProps {
  open: boolean;
  runResult: QcRunResponseData | null;
  isRunning: boolean;
  onClose: () => void;
  onRunQc: () => void;
  onDeterministicFix: () => void;
}

const ReadinessMetricTile: React.FC<{
  label: string;
  value: string;
  testId: string;
  accent?: string;
  dataVerdict?: ReadinessVerdictKey;
}> = ({ label, value, testId, accent, dataVerdict }) => (
  <div
    data-testid={testId}
    data-verdict={dataVerdict}
    style={{
      display: "flex", flexDirection: "column", gap: "4px", padding: "10px",
      borderRadius: "var(--radius-sm)", border: "1px solid var(--border-default)",
      backgroundColor: "var(--bg-surface)", minWidth: 0, overflow: "hidden",
    }}
  >
    <span style={{ fontSize: "11px", color: "var(--text-secondary)" }}>{label}</span>
    <strong
      style={{
        fontSize: value.length > LONG_VALUE_THRESHOLD ? "12px" : "16px",
        lineHeight: COMPACT_LINE_HEIGHT,
        overflowWrap: "anywhere",
        color: accent ?? "var(--text-primary)",
      }}
    >
      {value}
    </strong>
  </div>
);

const ReadinessHeader: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="layers" size={ICON_MD} strokeWidth={STROKE_MONO} />
        <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD }}>
          {t("visualQc.readinessTitle", "Kiểm tra sẵn sàng dự án")}
        </h2>
      </div>
      <button
        type="button"
        data-testid="close-readiness-modal-button"
        onClick={onClose}
        aria-label={t("visualQc.closeReadiness", "Đóng")}
        style={{
          display: "inline-flex", alignItems: "center", justifyContent: "center",
          width: "28px", height: "28px", borderRadius: "var(--radius-xs)",
          border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)",
          color: "var(--text-primary)", cursor: "pointer",
        }}
      >
        <BrandIcon name="close" size={ICON_SM} strokeWidth={STROKE_MONO} />
      </button>
    </div>
  );
};

const UnverifiedBrowserBanner: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div
      data-testid="qc-unverified-no-browser-badge"
      style={{
        display: "flex", alignItems: "flex-start", gap: "8px", padding: "10px 12px",
        borderRadius: "var(--radius-sm)", border: "1px solid var(--state-warning)",
        backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)",
        fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, lineHeight: COMPACT_LINE_HEIGHT,
      }}
    >
      <BrandIcon name="warning" size={ICON_SM} strokeWidth={STROKE_MONO} />
      <span>
        {t(
          "visualQc.unverifiedBanner",
          "Chưa tìm thấy trình duyệt kiểm tra cục bộ — hệ thống đã kiểm tra nhanh bố cục HTML nhưng chưa đo được độ khớp ảnh chụp thực tế."
        )}
      </span>
    </div>
  );
};

const FlowCompletenessSection: React.FC<{ report: FlowCompletenessReport }> = ({ report }) => {
  const { t } = useTranslation();
  const missingCount = Object.keys(report.missingSubstates).length;
  return (
    <div
      data-testid="qc-flow-completeness-section"
      style={{
        display: "flex", flexDirection: "column", gap: "6px", padding: "10px",
        borderRadius: "var(--radius-sm)", border: "1px solid var(--border-default)",
        backgroundColor: "var(--bg-subtle)", fontSize: "12px",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
        <span>{t("visualQc.flowCompletenessTitle", "Độ đầy đủ luồng thao tác")}</span>
        <span style={{ color: report.isComplete ? "var(--state-success)" : "var(--state-danger)" }}>
          {report.isComplete ? t("visualQc.flowComplete", "Đầy đủ") : t("visualQc.flowIncomplete", "Còn thiếu sót")}
        </span>
      </div>
      <div style={{ display: "flex", gap: "12px", flexWrap: "wrap", color: "var(--text-secondary)" }}>
        <span>{t("visualQc.deadEndCtas", "Nút bấm chưa nối đích")}: <strong>{report.deadEndCtas.length}</strong></span>
        <span>{t("visualQc.orphanScreens", "Màn hình tách rời")}: <strong>{report.orphanScreens.length}</strong></span>
        <span>{t("visualQc.missingSubstates", "Thiếu trạng thái dữ liệu")}: <strong>{missingCount}</strong></span>
      </div>
    </div>
  );
};

const TechnicalDetailsBlock: React.FC<{ issue: QcIssue }> = ({ issue }) => {
  const { t } = useTranslation();
  return (
    <details
      data-testid={`readiness-issue-details-${issue.id}`}
      style={{ fontSize: "11px", color: "var(--text-muted)" }}
    >
      <summary style={{ cursor: "pointer", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>
        {t("visualQc.technicalDetailsToggle", "Xem chi tiết kỹ thuật")}
      </summary>
      <div
        style={{
          marginTop: "6px", padding: "8px", borderRadius: "var(--radius-xs)",
          backgroundColor: "var(--bg-subtle)", fontFamily: "var(--font-mono)",
          display: "flex", flexDirection: "column", gap: "4px", color: "var(--text-secondary)",
        }}
      >
        <span>{t("visualQc.technicalRuleLabel", "Mã quy tắc")}: {issue.ruleId}</span>
        <span>{t("visualQc.technicalViewportLabel", "Khung hình")}: {issue.viewportWidth}px</span>
        <span>{t("visualQc.technicalDetailLabel", "Chi tiết")}: {issue.message}</span>
      </div>
    </details>
  );
};

const ReadinessIssueCard: React.FC<{ issue: QcIssue }> = ({ issue }) => {
  const { t } = useTranslation();
  const pill = resolveSeverityStyle(issue.severity, t);
  const copy = resolveIssueCopy(issue.ruleId, t);
  return (
    <div
      data-testid={`readiness-issue-${issue.id}`}
      style={{
        display: "flex", flexDirection: "column", gap: "6px", padding: "10px",
        borderRadius: "var(--radius-sm)", border: "1px solid var(--border-default)",
        backgroundColor: "var(--bg-surface)", fontSize: "12px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
        <span
          data-testid={`readiness-issue-severity-${issue.id}`}
          style={{
            display: "inline-flex", alignItems: "center", padding: "2px 8px",
            borderRadius: "var(--radius-pill)", border: `1px solid ${pill.borderColor}`,
            backgroundColor: pill.backgroundColor, color: pill.color,
            fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, whiteSpace: "nowrap",
          }}
        >
          {pill.label}
        </span>
        <strong style={{ color: "var(--text-primary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{copy.title}</strong>
      </div>
      <p style={{ margin: 0, color: "var(--text-secondary)", lineHeight: COMPACT_LINE_HEIGHT }}>{copy.hint}</p>
      <TechnicalDetailsBlock issue={issue} />
    </div>
  );
};

const ReadinessIssueList: React.FC<{ issues: QcIssue[] }> = ({ issues }) => (
  <div style={{ display: "flex", flexDirection: "column", gap: "8px", maxHeight: "240px", overflowY: "auto" }}>
    {issues.map((issue) => (
      <ReadinessIssueCard key={issue.id} issue={issue} />
    ))}
  </div>
);

const ReadinessFooterActions: React.FC<{
  isRunning: boolean;
  deterministicCount: number;
  onRunQc: () => void;
  onDeterministicFix: () => void;
}> = ({ isRunning, deterministicCount, onRunQc, onDeterministicFix }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px" }}>
      <button
        type="button"
        data-testid="readiness-rerun-button"
        disabled={isRunning}
        onClick={onRunQc}
        style={{
          height: "30px", padding: "0 12px", borderRadius: "var(--radius-xs)",
          border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)",
          color: "var(--text-primary)", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD,
          cursor: isRunning ? "wait" : "pointer",
        }}
      >
        {t("visualQc.rerunAudit", "Chạy lại kiểm tra")}
      </button>
      {deterministicCount > 0 && (
        <button
          type="button"
          data-testid="readiness-autofix-button"
          onClick={onDeterministicFix}
          style={{
            height: "30px", padding: "0 12px", borderRadius: "var(--radius-xs)",
            border: "1px solid var(--action-primary)", backgroundColor: "var(--action-primary)",
            color: "var(--action-primary-fg)", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer",
          }}
        >
          {t("visualQc.fixDeterministicAll", "Tự động sửa nhanh")} ({deterministicCount})
        </button>
      )}
    </div>
  );
};

const ReadinessMetricsGrid: React.FC<{ runResult: QcRunResponseData | null }> = ({ runResult }) => {
  const { t } = useTranslation();
  const verdict = resolveVerdictPresentation(runResult?.verdict, t);
  const p0Count = runResult?.p0Count ?? 0;
  const p1Count = runResult?.p1Count ?? 0;
  const ssimLabel =
    runResult?.ssimScore !== null && runResult?.ssimScore !== undefined
      ? `${(runResult.ssimScore * PERCENT_MULTIPLIER).toFixed(1)}%`
      : t("visualQc.ssimUnavailable", "Chưa có");
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(125px, 1fr))", gap: "8px" }}>
      <ReadinessMetricTile
        label={t("visualQc.verdictLabel", "Kết quả chung")}
        value={verdict.label}
        testId="readiness-metric-verdict"
        dataVerdict={runResult?.verdict ?? "PENDING"}
        accent={verdict.accent}
      />
      <ReadinessMetricTile
        label={t("visualQc.p0CountLabel", "Lỗi cần sửa ngay")}
        value={String(p0Count)}
        testId="readiness-metric-p0"
        accent={p0Count > 0 ? "var(--state-danger)" : undefined}
      />
      <ReadinessMetricTile
        label={t("visualQc.p1CountLabel", "Gợi ý cải thiện")}
        value={String(p1Count)}
        testId="readiness-metric-p1"
        accent={p1Count > 0 ? "var(--state-warning)" : undefined}
      />
      <ReadinessMetricTile
        label={t("visualQc.ssimLabel", "Độ khớp hiển thị")}
        value={ssimLabel}
        testId="readiness-metric-ssim"
      />
    </div>
  );
};

export const ProjectReadinessModal: React.FC<ProjectReadinessModalProps> = (props) => {
  const { t } = useTranslation();
  useEscapeClose(props.open, props.onClose);
  if (!props.open) return null;
  const runResult = props.runResult;
  const isUnverified = runResult?.verdict === "UNVERIFIED_NO_BROWSER";
  const deterministicCount = (runResult?.issues ?? []).filter((i) => i.deterministicFixable).length;

  return (
    <div
      data-testid="project-readiness-backdrop"
      onClick={(e) => {
        if (e.target === e.currentTarget) props.onClose();
      }}
      style={{
        position: "fixed", inset: 0, zIndex: MODAL_Z_INDEX,
        backgroundColor: "var(--bg-overlay)", display: "flex",
        alignItems: "center", justifyContent: "center", padding: "16px",
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("visualQc.readinessTitle", "Kiểm tra sẵn sàng dự án")}
        data-ff-id="project-readiness-panel"
        data-testid="project-readiness-panel"
        style={{
          display: "flex", flexDirection: "column", gap: "12px", padding: "16px",
          borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
          backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", maxWidth: "640px", width: "100%",
        }}
      >
        <ReadinessHeader onClose={props.onClose} />
        {isUnverified && <UnverifiedBrowserBanner />}
        <ReadinessMetricsGrid runResult={runResult} />
        {runResult?.flowCompleteness && <FlowCompletenessSection report={runResult.flowCompleteness} />}
        <ReadinessIssueList issues={runResult?.issues ?? []} />
        <ReadinessFooterActions
          isRunning={props.isRunning}
          deterministicCount={deterministicCount}
          onRunQc={props.onRunQc}
          onDeterministicFix={props.onDeterministicFix}
        />
      </div>
    </div>
  );
};
