// @story 13.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (RightDrawer [5.25] M1..M5 Release Certification Slide-Over)
import React, { useCallback, useEffect, useState } from "react";
import type { EvalCertificationApiClient } from "../../api/eval-certification";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { CowokToast } from "../../components/ui/CowokToast";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { EvalBenchmarkRunDto, EvalTier } from "../../lib/validations/eval-certification";
import { CertificationMetricStrip } from "./CertificationMetricStrip";
import { EvalAblationCalibrationTab } from "./EvalAblationCalibrationTab";
import {
  formatShaPreview,
  getPinSourceLabel,
  getSecurityGateLabel,
  getTamperReasonMessage,
  getTierLabel,
  getTierSubtitle,
  getVerdictLabel,
  type TranslateFn,
} from "./eval-copy";
import { EvalHistoryTimeline } from "./EvalHistoryTimeline";
import { SimCasesChecklistTable } from "./SimCasesChecklistTable";
import { MilestoneStatusPills, SimCasesFilterBar } from "./SimCasesFilterBar";
import { type EvalDrawerTab, useEvalCertification } from "./useEvalCertification";

export interface CertificationDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  projectId?: string;
  locale?: string;
  client?: Pick<EvalCertificationApiClient, "runEval" | "getEvalHistory">;
}

const DRAWER_Z_INDEX = 9992;
const BACKDROP_Z_INDEX = DRAWER_Z_INDEX - 1;
const DEFAULT_DRAWER_WIDTH = 520;
const MIN_DRAWER_WIDTH = 420;
const MAX_DRAWER_WIDTH = 960;
const DRAWER_VIEWPORT_MARGIN = 48;
const DRAWER_KEY_STEP_PX = 32;
const ICON_SIZE_MD = 15;
const ICON_SIZE_SM = 12;
const ICON_STROKE = 1.85;
const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;
const TIERS: readonly EvalTier[] = ["pr", "stage", "release"];

const DRAWER_SHELL_STYLE: React.CSSProperties = {
  position: "fixed",
  top: 0,
  right: 0,
  bottom: 0,
  maxWidth: "100vw",
  backgroundColor: "var(--bg-surface)",
  borderLeft: "1px solid var(--border-subtle)",
  boxShadow: "var(--shadow-lg)",
  zIndex: DRAWER_Z_INDEX,
  display: "flex",
  flexDirection: "column",
  fontFamily: "var(--font-sans)",
  color: "var(--text-primary)",
};

function clampDrawerWidth(rawWidth: number): number {
  const viewportMax =
    typeof window !== "undefined"
      ? Math.max(MIN_DRAWER_WIDTH, window.innerWidth - DRAWER_VIEWPORT_MARGIN)
      : MAX_DRAWER_WIDTH;
  const upper = Math.min(MAX_DRAWER_WIDTH, viewportMax);
  return Math.round(Math.min(upper, Math.max(MIN_DRAWER_WIDTH, rawWidth)));
}

function useResizableDrawerWidth() {
  const [width, setWidth] = useState<number>(DEFAULT_DRAWER_WIDTH);
  const [isResizing, setIsResizing] = useState(false);

  const onPointerDown = useCallback((evt: React.PointerEvent<HTMLDivElement>) => {
    evt.preventDefault();
    setIsResizing(true);
    const onMove = (moveEvt: PointerEvent) => {
      const next = clampDrawerWidth(window.innerWidth - moveEvt.clientX);
      setWidth(next);
    };
    const onUp = () => {
      setIsResizing(false);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }, []);

  const onKeyDown = useCallback((evt: React.KeyboardEvent<HTMLDivElement>) => {
    if (evt.key === "ArrowLeft") {
      evt.preventDefault();
      setWidth((prev) => clampDrawerWidth(prev + DRAWER_KEY_STEP_PX));
    } else if (evt.key === "ArrowRight") {
      evt.preventDefault();
      setWidth((prev) => clampDrawerWidth(prev - DRAWER_KEY_STEP_PX));
    } else if (evt.key === "Home") {
      evt.preventDefault();
      setWidth(DEFAULT_DRAWER_WIDTH);
    }
  }, []);

  const onDoubleClick = useCallback(() => {
    setWidth(DEFAULT_DRAWER_WIDTH);
  }, []);

  return { width, isResizing, onPointerDown, onKeyDown, onDoubleClick };
}

const ICON_BTN_STYLE: React.CSSProperties = {
  width: "28px",
  height: "28px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "transparent",
  color: "var(--text-secondary)",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  cursor: "pointer",
};

function buildTabBtnStyle(active: boolean): React.CSSProperties {
  return {
    padding: "9px 8px",
    fontSize: "11.5px",
    fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
    color: active ? "var(--text-primary)" : "var(--text-secondary)",
    backgroundColor: active ? "var(--bg-selected)" : "transparent",
    border: "none",
    borderBottom: active ? "2px solid var(--action-primary)" : "2px solid transparent",
    cursor: "pointer",
    whiteSpace: "nowrap",
  };
}

function useEscapeClose(isOpen: boolean, onClose: () => void): void {
  useEffect(() => {
    if (!isOpen) return;
    const onKeyDown = (evt: KeyboardEvent) => {
      if (evt.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, onClose]);
}

const DrawerHeaderBar: React.FC<{
  tamperActive: boolean;
  manifestSha256?: string;
  pinSourceLabel?: string;
  isLoading: boolean;
  onRefresh: () => void;
  onClose: () => void;
  t: TranslateFn;
}> = ({ tamperActive, manifestSha256, pinSourceLabel, isLoading, onRefresh, onClose, t }) => (
  <header
    style={{
      padding: "12px 16px",
      backgroundColor: "var(--bg-elevated)",
      borderBottom: "1px solid var(--border-subtle)",
      display: "flex",
      alignItems: "center",
      justifyContent: "space-between",
      gap: "10px",
    }}
  >
    <div style={{ display: "flex", flexDirection: "column", gap: "4px", minWidth: 0 }}>
      <div style={{ display: "flex", alignItems: "center", gap: "7px", flexWrap: "wrap" }}>
        <BrandIcon name="shield-check" size={ICON_SIZE_MD} strokeWidth={ICON_STROKE} color="var(--action-primary)" />
        <h2 style={{ margin: 0, fontSize: "14px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
          {t("evalCertification.drawerTitle", "Chứng nhận Phát hành M1–M5")}
        </h2>
        <span
          data-testid="eval-sealed-integrity-pill"
          title={pinSourceLabel}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "4px",
            padding: "2px 7px",
            borderRadius: "var(--radius-pill)",
            backgroundColor: tamperActive ? "var(--state-danger-soft)" : "var(--state-success-soft)",
            color: tamperActive ? "var(--state-danger)" : "var(--state-success)",
            fontSize: "10.5px",
            fontWeight: FONT_WEIGHT_SEMIBOLD,
          }}
        >
          <BrandIcon name="lock" size={10} strokeWidth={ICON_STROKE} color="currentColor" />
          <span>
            {tamperActive
              ? t("evalCertification.sealedBadgeTampered", "Niêm phong bị cảnh báo")
              : t("evalCertification.sealedBadgeVerified", "Tiêu chuẩn đã niêm phong")}
          </span>
        </span>
      </div>
      {manifestSha256 && (
        <span style={{ fontSize: "10.5px", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
          {`${pinSourceLabel ? `${pinSourceLabel} · ` : ""}${formatShaPreview(manifestSha256)}`}
        </span>
      )}
    </div>

    <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", flexShrink: 0 }}>
      <button
        type="button"
        data-testid="eval-refresh-btn"
        aria-label={t("evalCertification.refreshBtn", "Tải lại trạng thái")}
        title={t("evalCertification.refreshBtn", "Tải lại trạng thái")}
        disabled={isLoading}
        onClick={onRefresh}
        style={ICON_BTN_STYLE}
      >
        <BrandIcon name="refresh" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
      </button>
      <button
        type="button"
        data-testid="eval-drawer-close-btn"
        aria-label={t("common.close", "Đóng")}
        title={t("common.close", "Đóng")}
        onClick={onClose}
        style={ICON_BTN_STYLE}
      >
        <BrandIcon name="close" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
      </button>
    </div>
  </header>
);

const DrawerAlertBanners: React.FC<{
  tamperActive: boolean;
  tamperReason: string | null;
  conflictActive: boolean;
  loadError: string | null;
  selectedRun: EvalBenchmarkRunDto | null;
  locale: string;
  t: TranslateFn;
}> = ({ tamperActive, tamperReason, conflictActive, loadError, selectedRun, locale, t }) => {
  const fmt = useFormatter({ locale });
  const showSecurityNonMasking = Boolean(selectedRun && !selectedRun.securityGatesPassed);

  return (
    <>
      {tamperActive && (
        <div
          role="alert"
          data-testid="eval-tamper-banner"
          style={{
            padding: "10px 12px",
            borderRadius: "var(--radius-sm)",
            border: "1px solid var(--state-danger)",
            backgroundColor: "var(--state-danger-soft)",
            color: "var(--state-danger)",
            display: "flex",
            flexDirection: "column",
            gap: "4px",
            fontSize: "11.5px",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
            <BrandIcon name="shield-alert" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
            <span>{t("evalCertification.tamperBannerTitle", "Cảnh báo toàn vẹn bộ tiêu chuẩn niêm phong")}</span>
          </div>
          <span style={{ color: "var(--text-primary)", lineHeight: 1.4 }}>
            {getTamperReasonMessage(tamperReason, t)}
          </span>
        </div>
      )}

      {conflictActive && (
        <div
          role="alert"
          data-testid="eval-conflict-banner"
          style={{
            padding: "9px 12px",
            borderRadius: "var(--radius-sm)",
            border: "1px solid var(--state-warning)",
            backgroundColor: "var(--state-warning-soft)",
            color: "var(--state-warning)",
            fontSize: "11.5px",
            fontWeight: FONT_WEIGHT_MEDIUM,
          }}
        >
          {t(
            "evalCertification.errAlreadyRunning",
            "Đang có một phiên kiểm định chạy song song. Vui lòng đợi hoàn tất."
          )}
        </div>
      )}

      {showSecurityNonMasking && selectedRun && (
        <div
          role="alert"
          data-testid="eval-security-nonmasking-banner"
          style={{
            padding: "10px 12px",
            borderRadius: "var(--radius-sm)",
            border: "1px solid var(--state-danger)",
            backgroundColor: "var(--state-danger-soft)",
            display: "flex",
            flexDirection: "column",
            gap: "5px",
            fontSize: "11.5px",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--state-danger)" }}>
            <BrandIcon name="shield-alert" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
            <span>
              {t(
                "evalCertification.securityNonMaskingTitle",
                "Chứng nhận bị chặn bởi cổng bảo mật bắt buộc"
              )}
            </span>
          </div>
          <span style={{ color: "var(--text-primary)", lineHeight: 1.4 }}>
            {t(
              "evalCertification.securityNonMaskingDesc",
              "Dù tổng số hành trình đạt {{passed}}/{{total}}, hệ thống không cho phép bỏ qua các cổng bảo mật bắt buộc:",
              {
                passed: fmt.formatNumber(selectedRun.simCasesPassed),
                total: fmt.formatNumber(selectedRun.simCasesTotal),
              }
            )}
          </span>
          {selectedRun.securityBlockers.length > 0 && (
            <ul style={{ margin: 0, paddingLeft: "18px", color: "var(--state-danger)", fontSize: "11px" }}>
              {selectedRun.securityBlockers.map((blocker) => (
                <li key={blocker}>{getSecurityGateLabel(blocker, t)}</li>
              ))}
            </ul>
          )}
        </div>
      )}

      {loadError && (
        <div
          role="alert"
          data-testid="eval-load-error-banner"
          style={{
            padding: "9px 12px",
            borderRadius: "var(--radius-sm)",
            border: "1px solid var(--state-danger)",
            backgroundColor: "var(--state-danger-soft)",
            color: "var(--state-danger)",
            fontSize: "11.5px",
          }}
        >
          {loadError}
        </div>
      )}
    </>
  );
};

const TierSelectorRunBar: React.FC<{
  selectedTier: EvalTier;
  isRunning: boolean;
  latestVerdict?: EvalBenchmarkRunDto["verdict"];
  onSelectTier: (tier: EvalTier) => void;
  onRun: () => void;
  t: TranslateFn;
}> = ({ selectedTier, isRunning, latestVerdict, onSelectTier, onRun, t }) => (
  <section
    data-testid="eval-tier-selector"
    aria-label={t("evalCertification.tierSelectorAria", "Chọn cấp độ kiểm định")}
    style={{ display: "flex", flexDirection: "column", gap: "8px" }}
  >
    <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: "6px" }}>
      {TIERS.map((tier) => {
        const active = selectedTier === tier;
        return (
          <button
            key={tier}
            type="button"
            data-testid={`eval-tier-${tier}`}
            aria-pressed={active}
            onClick={() => onSelectTier(tier)}
            style={{
              textAlign: "left",
              padding: "8px 9px",
              borderRadius: "var(--radius-sm)",
              border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-subtle)",
              backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
              cursor: "pointer",
              display: "flex",
              flexDirection: "column",
              gap: "3px",
            }}
          >
            <span
              style={{
                fontSize: "11.5px",
                fontWeight: FONT_WEIGHT_SEMIBOLD,
                color: active ? "var(--action-primary)" : "var(--text-primary)",
              }}
            >
              {getTierLabel(tier, t)}
            </span>
            <span style={{ fontSize: "10px", color: "var(--text-muted)", lineHeight: 1.3 }}>
              {getTierSubtitle(tier, t)}
            </span>
          </button>
        );
      })}
    </div>

    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
      <button
        type="button"
        data-testid="eval-run-trigger-btn"
        aria-label={t("evalCertification.runBtnAria", "{{action}} — {{tier}}", {
          action: isRunning
            ? t("evalCertification.runningBtn", "Đang kiểm định...")
            : t("evalCertification.runBtn", "Chạy kiểm định"),
          tier: getTierLabel(selectedTier, t),
        })}
        disabled={isRunning}
        onClick={onRun}
        style={{
          flex: 1,
          height: "34px",
          padding: "0 14px",
          borderRadius: "var(--radius-xs)",
          border: "none",
          backgroundColor: "var(--action-primary)",
          color: "var(--action-primary-fg)",
          fontSize: "12px",
          fontWeight: FONT_WEIGHT_SEMIBOLD,
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          gap: "6px",
          whiteSpace: "nowrap",
          cursor: isRunning ? "not-allowed" : "pointer",
          opacity: isRunning ? 0.7 : 1,
        }}
      >
        <BrandIcon name="play" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
        <span>
          {isRunning
            ? t("evalCertification.runningBtn", "Đang kiểm định...")
            : t("evalCertification.runBtn", "Chạy kiểm định")}
        </span>
      </button>

      {latestVerdict && (
        <span
          data-testid="eval-verdict-badge"
          style={{
            padding: "6px 10px",
            borderRadius: "var(--radius-pill)",
            backgroundColor:
              latestVerdict === "PASS" ? "var(--state-success-soft)" : "var(--state-danger-soft)",
            color: latestVerdict === "PASS" ? "var(--state-success)" : "var(--state-danger)",
            fontSize: "11px",
            fontWeight: FONT_WEIGHT_SEMIBOLD,
            whiteSpace: "nowrap",
          }}
        >
          {getVerdictLabel(latestVerdict, t)}
        </span>
      )}
    </div>
  </section>
);

const DrawerSectionTabs: React.FC<{
  activeTab: EvalDrawerTab;
  casesCount: number;
  historyCount: number;
  locale: string;
  onSelectTab: (tab: EvalDrawerTab) => void;
  t: TranslateFn;
}> = ({ activeTab, casesCount, historyCount, locale, onSelectTab, t }) => {
  const fmt = useFormatter({ locale });
  return (
    <div
      role="tablist"
      aria-label={t("evalCertification.tabsAria", "Các mục chứng nhận")}
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
        borderBottom: "1px solid var(--border-subtle)",
        backgroundColor: "var(--bg-subtle)",
        flexShrink: 0,
      }}
    >
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === "cases"}
        data-testid="eval-tab-cases"
        onClick={() => onSelectTab("cases")}
        style={buildTabBtnStyle(activeTab === "cases")}
      >
        {t("evalCertification.tabCases", "Checklist ({{count}})", {
          count: fmt.formatNumber(casesCount),
        })}
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === "ablation"}
        data-testid="eval-tab-ablation"
        onClick={() => onSelectTab("ablation")}
        style={buildTabBtnStyle(activeTab === "ablation")}
      >
        {t("evalCertification.tabAblation", "Đối chứng & Hiệu chuẩn")}
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === "history"}
        data-testid="eval-tab-history"
        onClick={() => onSelectTab("history")}
        style={buildTabBtnStyle(activeTab === "history")}
      >
        {t("evalCertification.tabHistory", "Lịch sử ({{count}})", {
          count: fmt.formatNumber(historyCount),
        })}
      </button>
    </div>
  );
};

export const CertificationDrawer: React.FC<CertificationDrawerProps> = ({
  isOpen,
  onClose,
  projectId,
  locale,
  client,
}) => {
  const { t, locale: activeLocale } = useTranslation(locale);
  const resolvedLocale = locale || activeLocale;
  const ctrl = useEvalCertification({ isOpen, projectId, t, client });
  const resize = useResizableDrawerWidth();

  useEscapeClose(isOpen, onClose);
  if (!isOpen) return null;

  const pinSourceLabel = ctrl.history?.manifestPinSource
    ? getPinSourceLabel(ctrl.history.manifestPinSource, t)
    : undefined;
  const milestones = ctrl.selectedRun?.milestones ?? [];
  const totalCases = ctrl.selectedRun?.simCases.length ?? 0;
  const displayedVerdict: EvalBenchmarkRunDto["verdict"] | undefined = ctrl.liveTamperActive
    ? "SEALED_EVAL_TAMPER_ERROR"
    : ctrl.selectedRun?.verdict;

  return (
    <>
      <div
        role="presentation"
        data-testid="certification-drawer-backdrop"
        onClick={onClose}
        style={{
          position: "fixed",
          inset: 0,
          backgroundColor: "var(--bg-overlay)",
          zIndex: BACKDROP_Z_INDEX,
        }}
      />
      <aside
        role="dialog"
        aria-modal="true"
        aria-label={t("evalCertification.drawerTitle", "Chứng nhận Phát hành M1–M5")}
        data-testid="certification-drawer"
        style={{
          ...DRAWER_SHELL_STYLE,
          width: `${resize.width}px`,
          userSelect: resize.isResizing ? "none" : undefined,
        }}
      >
        <div
          role="separator"
          tabIndex={0}
          aria-orientation="vertical"
          aria-label={t("evalCertification.resizeDrawerAria", "Kéo để thay đổi chiều rộng ngăn")}
          aria-valuemin={MIN_DRAWER_WIDTH}
          aria-valuemax={MAX_DRAWER_WIDTH}
          aria-valuenow={resize.width}
          title={t("evalCertification.resizeDrawerAria", "Kéo để thay đổi chiều rộng ngăn")}
          data-testid="certification-drawer-resize-handle"
          onPointerDown={resize.onPointerDown}
          onKeyDown={resize.onKeyDown}
          onDoubleClick={resize.onDoubleClick}
          style={{
            position: "absolute",
            top: 0,
            left: "-3px",
            bottom: 0,
            width: "7px",
            cursor: "col-resize",
            zIndex: 2,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <span
            aria-hidden="true"
            style={{
              width: "2px",
              height: resize.isResizing ? "100%" : "36px",
              borderRadius: "var(--radius-pill)",
              backgroundColor: resize.isResizing
                ? "var(--action-primary)"
                : "var(--border-default)",
              transition: "height 120ms ease, background-color 120ms ease",
            }}
          />
        </div>
        <DrawerHeaderBar
          tamperActive={ctrl.liveTamperActive}
          manifestSha256={ctrl.history?.sealedManifestSha256}
          pinSourceLabel={pinSourceLabel}
          isLoading={ctrl.isLoadingHistory}
          onRefresh={() => void ctrl.loadHistory()}
          onClose={onClose}
          t={t}
        />

        <div
          style={{
            padding: "12px 16px",
            display: "flex",
            flexDirection: "column",
            gap: "10px",
            borderBottom: "1px solid var(--border-subtle)",
            flexShrink: 0,
          }}
        >
          <DrawerAlertBanners
            tamperActive={ctrl.tamperActive}
            tamperReason={ctrl.tamperReason}
            conflictActive={ctrl.conflictActive}
            loadError={ctrl.loadError}
            selectedRun={ctrl.selectedRun}
            locale={resolvedLocale}
            t={t}
          />
          <TierSelectorRunBar
            selectedTier={ctrl.selectedTier}
            isRunning={ctrl.isRunning}
            latestVerdict={displayedVerdict}
            onSelectTier={ctrl.setSelectedTier}
            onRun={() => void ctrl.handleRunEval()}
            t={t}
          />
          <CertificationMetricStrip run={ctrl.selectedRun} locale={resolvedLocale} />
          {milestones.length > 0 && (
            <MilestoneStatusPills
              milestones={milestones}
              activeMilestone={ctrl.milestoneFilter}
              onSelectMilestone={ctrl.selectMilestonePill}
              locale={resolvedLocale}
            />
          )}
        </div>

        <DrawerSectionTabs
          activeTab={ctrl.activeTab}
          casesCount={totalCases}
          historyCount={ctrl.history?.runs.length ?? 0}
          locale={resolvedLocale}
          onSelectTab={ctrl.setActiveTab}
          t={t}
        />

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            padding: "12px 16px",
            display: "flex",
            flexDirection: "column",
            gap: "10px",
          }}
        >
          {ctrl.activeTab === "cases" && (
            <>
              <SimCasesFilterBar
                searchQuery={ctrl.searchQuery}
                statusFilter={ctrl.statusFilter}
                securityOnly={ctrl.securityOnly}
                totalMatching={ctrl.filteredSimCases.length}
                totalCases={totalCases}
                onSearchChange={ctrl.setSearchQuery}
                onStatusChange={ctrl.setStatusFilter}
                onToggleSecurityOnly={() => ctrl.setSecurityOnly((prev) => !prev)}
                locale={resolvedLocale}
              />
              <SimCasesChecklistTable cases={ctrl.filteredSimCases} locale={resolvedLocale} />
            </>
          )}
          {ctrl.activeTab === "ablation" && (
            <EvalAblationCalibrationTab run={ctrl.selectedRun} locale={resolvedLocale} />
          )}
          {ctrl.activeTab === "history" && (
            <EvalHistoryTimeline
              runs={ctrl.history?.runs ?? []}
              selectedRunId={ctrl.selectedRun?.runId ?? null}
              onSelectRun={ctrl.selectHistoryRun}
              locale={resolvedLocale}
            />
          )}
        </div>
      </aside>
      {ctrl.toast && (
        <CowokToast
          tone={ctrl.toast.tone}
          message={ctrl.toast.message}
          testId="eval-certification-toast"
          onDismiss={ctrl.dismissToast}
        />
      )}
    </>
  );
};
