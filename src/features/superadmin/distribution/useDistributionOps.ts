// @story 14.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (SuperAdmin Distribution Ops State Hook — Catalog, A/B, Canary, Quarantine, Offline Licenses & Audit Logs)
import { useCallback, useEffect, useRef, useState } from "react";
import {
  apiDistributionOps,
  DistributionOpsApiError,
  type DistributionOpsApiPort,
} from "../../../api/distribution-ops";
import type {
  AbExperiment,
  BulkUpdateCatalogRequest,
  CatalogPlugin,
  DistributionAuditLog,
  IssueLicenseRequest,
  IssueQuarantineRequest,
  OfflineLicenseRecord,
  PluginLifecycleStatus,
  PublishRolloutRequest,
  ReleaseRollout,
  SignedAdvisoriesManifest,
  SignedCatalogManifest,
  SignedExperimentsManifest,
  SignedLatestReleaseManifest,
  SkillAdvisory,
  UpsertCatalogPluginRequest,
  UpsertExperimentRequest,
} from "../../../lib/validations/distribution-ops";
import { getGlobalLocale, translateGlobal } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import type { SuperAdminToastState } from "../useSuperAdminController";

type UpsertCatalogHandler = (input: UpsertCatalogPluginRequest) => Promise<boolean>;
type BulkCatalogHandler = (input: BulkUpdateCatalogRequest) => Promise<boolean>;

const BULK_HANDLER_REGISTRY = new WeakMap<UpsertCatalogHandler, BulkCatalogHandler>();

export function resolveBulkCatalogHandler(
  upsertFn: UpsertCatalogHandler
): BulkCatalogHandler | undefined {
  return BULK_HANDLER_REGISTRY.get(upsertFn);
}

const STATUS_I18N_MAP: Record<PluginLifecycleStatus, { key: string; fallback: string }> = {
  ACTIVE: { key: "superadmin.ops.statusActive", fallback: "Đang hoạt động" },
  PROCESSING: { key: "superadmin.ops.statusProcessing", fallback: "Đang xử lý" },
  PENDING_APPROVAL: { key: "superadmin.ops.statusPendingApproval", fallback: "Chờ duyệt" },
  INACTIVE: { key: "superadmin.ops.statusInactive", fallback: "Tạm dừng" },
  DEPRECATED: { key: "superadmin.ops.statusDeprecated", fallback: "Ngừng khuyến nghị" },
  QUARANTINED: { key: "superadmin.ops.statusQuarantined", fallback: "Đang cách ly" },
};

function translateStatusLabel(status: PluginLifecycleStatus): string {
  const spec = STATUS_I18N_MAP[status];
  return spec ? translateGlobal(spec.key, spec.fallback) : status;
}

function formatLocalizedCount(val: number): string {
  try {
    return new Intl.NumberFormat(getGlobalLocale() || "vi-VN").format(val);
  } catch {
    return String(val);
  }
}

function formatDistError(err: unknown, fallback: string): string {
  if (err instanceof DistributionOpsApiError) return `${err.code}: ${err.message}`;
  if (err instanceof Error && err.message) return err.message;
  return fallback;
}

export interface UseDistributionOpsParams {
  sessionToken: string | null;
  operatorRole?: CloudAdminRole;
  client?: DistributionOpsApiPort;
  onNotify?: (toast: SuperAdminToastState) => void;
}

export function useDistributionOps({
  sessionToken,
  operatorRole,
  client = apiDistributionOps,
  onNotify,
}: UseDistributionOpsParams) {
  const [rollouts, setRollouts] = useState<ReleaseRollout[]>([]);
  const [latestManifest, setLatestManifest] = useState<SignedLatestReleaseManifest | null>(null);
  const [plugins, setPlugins] = useState<CatalogPlugin[]>([]);
  const [catalogManifest, setCatalogManifest] = useState<SignedCatalogManifest | null>(null);
  const [experiments, setExperiments] = useState<AbExperiment[]>([]);
  const [experimentsManifest, setExperimentsManifest] = useState<SignedExperimentsManifest | null>(
    null
  );
  const [advisories, setAdvisories] = useState<SkillAdvisory[]>([]);
  const [advisoriesManifest, setAdvisoriesManifest] = useState<SignedAdvisoriesManifest | null>(
    null
  );
  const [licenses, setLicenses] = useState<OfflineLicenseRecord[]>([]);
  const [licenseKeyId, setLicenseKeyId] = useState<string>("cowok-ed25519-v1");
  const [licensePublicKey, setLicensePublicKey] = useState<string>("");
  const [auditLogs, setAuditLogs] = useState<DistributionAuditLog[]>([]);
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const loadSeqRef = useRef(0);

  const canReadSuperAdminOnly = operatorRole === undefined || operatorRole === "SUPER_ADMIN";

  const refreshDistributionOps = useCallback(async () => {
    if (!sessionToken) return;
    const seq = ++loadSeqRef.current;
    setLoading(true);
    try {
      const results = await Promise.allSettled([
        client.listRollouts(sessionToken),
        client.listCatalog(sessionToken),
        client.listExperiments(sessionToken),
        client.listQuarantines(sessionToken),
        canReadSuperAdminOnly ? client.listLicenses(sessionToken) : Promise.resolve(null),
        canReadSuperAdminOnly ? client.listAuditLogs(sessionToken) : Promise.resolve(null),
      ]);
      if (seq !== loadSeqRef.current) return;

      const [rRes, cRes, eRes, qRes, lRes, aRes] = results;
      if (rRes.status === "fulfilled") {
        setRollouts(rRes.value.data.rollouts);
        setLatestManifest(rRes.value.data.latestSignedManifest);
      } else {
        setRollouts([]);
        setLatestManifest(null);
      }
      if (cRes.status === "fulfilled") {
        setPlugins(cRes.value.data.plugins);
        setCatalogManifest(cRes.value.data.signedManifest);
      } else {
        setPlugins([]);
        setCatalogManifest(null);
      }
      if (eRes.status === "fulfilled") {
        setExperiments(eRes.value.data.experiments);
        setExperimentsManifest(eRes.value.data.signedManifest);
      } else {
        setExperiments([]);
        setExperimentsManifest(null);
      }
      if (qRes.status === "fulfilled") {
        setAdvisories(qRes.value.data.advisories);
        setAdvisoriesManifest(qRes.value.data.signedManifest);
      } else {
        setAdvisories([]);
        setAdvisoriesManifest(null);
      }
      if (lRes.status === "fulfilled" && lRes.value) {
        setLicenses(lRes.value.data.licenses);
        setLicenseKeyId(lRes.value.data.keyId);
        setLicensePublicKey(lRes.value.data.publicKeyBase64);
      } else {
        setLicenses([]);
        setLicensePublicKey("");
      }
      if (aRes.status === "fulfilled" && aRes.value) {
        setAuditLogs(aRes.value.data.logs);
      } else {
        setAuditLogs([]);
      }
    } finally {
      if (seq === loadSeqRef.current) setLoading(false);
    }
  }, [canReadSuperAdminOnly, client, sessionToken]);

  useEffect(() => {
    if (!sessionToken) {
      loadSeqRef.current += 1;
      setRollouts([]);
      setLatestManifest(null);
      setPlugins([]);
      setCatalogManifest(null);
      setExperiments([]);
      setExperimentsManifest(null);
      setAdvisories([]);
      setAdvisoriesManifest(null);
      setLicenses([]);
      setLicensePublicKey("");
      setAuditLogs([]);
      return;
    }
    void refreshDistributionOps();
  }, [sessionToken, refreshDistributionOps]);

  const refreshAuditQuietly = useCallback(async () => {
    if (!sessionToken) return;
    try {
      const res = await client.listAuditLogs(sessionToken);
      setAuditLogs(res.data.logs);
    } catch {
      setAuditLogs([]);
    }
  }, [client, sessionToken]);

  const publishRollout = useCallback(
    async (input: PublishRolloutRequest): Promise<boolean> => {
      if (!sessionToken) return false;
      setSubmitting(true);
      try {
        const res = await client.publishRollout(input, sessionToken);
        setRollouts((prev) => [res.data.rollout, ...prev]);
        setLatestManifest(res.data.signedManifest);
        onNotify?.({
          tone: "success",
          message: translateGlobal(
            "superadmin.ops.toastRolloutPublished",
            "Đã xuất bản bản phát hành {{channel}} v{{version}} ({{percent}}%)",
            {
              channel: res.data.rollout.channel,
              version: res.data.rollout.version,
              percent: formatLocalizedCount(res.data.rollout.rolloutPercent),
            }
          ),
        });
        void refreshAuditQuietly();
        return true;
      } catch (err) {
        onNotify?.({
          tone: "danger",
          message: formatDistError(
            err,
            translateGlobal("superadmin.ops.errRolloutFailed", "Xuất bản Canary thất bại")
          ),
        });
        return false;
      } finally {
        setSubmitting(false);
      }
    },
    [client, onNotify, refreshAuditQuietly, sessionToken]
  );

  const upsertCatalogPlugin = useCallback(
    async (input: UpsertCatalogPluginRequest): Promise<boolean> => {
      if (!sessionToken) return false;
      setSubmitting(true);
      try {
        const res = await client.upsertCatalogPlugin(input, sessionToken);
        setPlugins((prev) => [
          res.data.plugin,
          ...prev.filter((p) => p.pluginId !== res.data.plugin.pluginId),
        ]);
        setCatalogManifest(res.data.signedManifest);
        const statusLabel = translateStatusLabel(res.data.plugin.lifecycleStatus);
        onNotify?.({
          tone: "success",
          message: translateGlobal(
            "superadmin.ops.toastPluginUpdated",
            "Đã cập nhật plugin {{pluginId}} ({{status}})",
            { pluginId: res.data.plugin.pluginId, status: statusLabel }
          ),
        });
        void refreshAuditQuietly();
        return true;
      } catch (err) {
        onNotify?.({
          tone: "danger",
          message: formatDistError(
            err,
            translateGlobal("superadmin.ops.errCatalogFailed", "Cập nhật danh mục Plugin thất bại")
          ),
        });
        return false;
      } finally {
        setSubmitting(false);
      }
    },
    [client, onNotify, refreshAuditQuietly, sessionToken]
  );

  const bulkUpdateCatalog = useCallback(
    async (input: BulkUpdateCatalogRequest): Promise<boolean> => {
      if (!sessionToken) return false;
      setSubmitting(true);
      try {
        const res = await client.bulkUpdateCatalog(input, sessionToken);
        setPlugins(res.data.plugins);
        setCatalogManifest(res.data.signedManifest);
        const statusLabel = translateStatusLabel(res.data.lifecycleStatus);
        onNotify?.({
          tone: "success",
          message: translateGlobal(
            "superadmin.ops.toastBulkUpdated",
            "Đã cập nhật hàng loạt {{count}} plugin sang trạng thái \"{{status}}\"",
            { count: formatLocalizedCount(res.data.updatedCount), status: statusLabel }
          ),
        });
        void refreshAuditQuietly();
        return true;
      } catch (err) {
        onNotify?.({
          tone: "danger",
          message: formatDistError(
            err,
            translateGlobal("superadmin.ops.errBulkFailed", "Cập nhật hàng loạt Plugin thất bại")
          ),
        });
        return false;
      } finally {
        setSubmitting(false);
      }
    },
    [client, onNotify, refreshAuditQuietly, sessionToken]
  );

  BULK_HANDLER_REGISTRY.set(upsertCatalogPlugin, bulkUpdateCatalog);

  const upsertExperiment = useCallback(
    async (input: UpsertExperimentRequest): Promise<boolean> => {
      if (!sessionToken) return false;
      setSubmitting(true);
      try {
        const res = await client.upsertExperiment(input, sessionToken);
        setExperiments((prev) => [
          res.data.experiment,
          ...prev.filter((e) => e.experimentKey !== res.data.experiment.experimentKey),
        ]);
        setExperimentsManifest(res.data.signedManifest);
        onNotify?.({
          tone: res.data.experiment.killSwitch ? "warning" : "success",
          message: res.data.experiment.killSwitch
            ? translateGlobal(
                "superadmin.ops.toastExpKilled",
                "Đã kích hoạt ngắt khẩn cấp thử nghiệm {{key}}",
                { key: res.data.experiment.experimentKey }
              )
            : translateGlobal(
                "superadmin.ops.toastExpSaved",
                "Đã lưu thử nghiệm A/B {{key}}",
                { key: res.data.experiment.experimentKey }
              ),
        });
        void refreshAuditQuietly();
        return true;
      } catch (err) {
        onNotify?.({
          tone: "danger",
          message: formatDistError(
            err,
            translateGlobal("superadmin.ops.errExpFailed", "Lưu thử nghiệm A/B thất bại")
          ),
        });
        return false;
      } finally {
        setSubmitting(false);
      }
    },
    [client, onNotify, refreshAuditQuietly, sessionToken]
  );

  const issueQuarantine = useCallback(
    async (input: IssueQuarantineRequest): Promise<boolean> => {
      if (!sessionToken) return false;
      setSubmitting(true);
      try {
        const res = await client.issueQuarantine(input, sessionToken);
        setAdvisories((prev) => [res.data.advisory, ...prev]);
        setAdvisoriesManifest(res.data.signedManifest);
        if (res.data.advisory.action === "QUARANTINE") {
          setPlugins((prev) =>
            prev.map((p) =>
              p.pluginId === res.data.advisory.skillOrPluginId
                ? { ...p, lifecycleStatus: "QUARANTINED" }
                : p
            )
          );
        } else if (res.data.advisory.action === "REVOKE_QUARANTINE") {
          setPlugins((prev) =>
            prev.map((p) =>
              p.pluginId === res.data.advisory.skillOrPluginId &&
              p.lifecycleStatus === "QUARANTINED"
                ? { ...p, lifecycleStatus: "ACTIVE" }
                : p
            )
          );
        }
        onNotify?.({
          tone: "warning",
          message: translateGlobal(
            "superadmin.ops.toastQuarantineIssued",
            "Đã phát hành bản tin cách ly cho {{id}}",
            { id: res.data.advisory.skillOrPluginId }
          ),
        });
        void refreshAuditQuietly();
        return true;
      } catch (err) {
        onNotify?.({
          tone: "danger",
          message: formatDistError(
            err,
            translateGlobal("superadmin.ops.errQuarantineFailed", "Phát hành lệnh cách ly thất bại")
          ),
        });
        return false;
      } finally {
        setSubmitting(false);
      }
    },
    [client, onNotify, refreshAuditQuietly, sessionToken]
  );

  const issueLicense = useCallback(
    async (input: IssueLicenseRequest): Promise<boolean> => {
      if (!sessionToken) return false;
      setSubmitting(true);
      try {
        const res = await client.issueLicense(input, sessionToken);
        setLicenses((prev) => [res.data.license, ...prev]);
        onNotify?.({
          tone: "success",
          message: translateGlobal(
            "superadmin.ops.toastLicenseIssued",
            "Đã cấp khóa bản quyền {{tier}} cho {{org}}",
            { tier: res.data.license.tier, org: res.data.license.subjectOrg }
          ),
        });
        void refreshAuditQuietly();
        return true;
      } catch (err) {
        onNotify?.({
          tone: "danger",
          message: formatDistError(
            err,
            translateGlobal("superadmin.ops.errLicenseFailed", "Cấp khóa bản quyền thất bại")
          ),
        });
        return false;
      } finally {
        setSubmitting(false);
      }
    },
    [client, onNotify, refreshAuditQuietly, sessionToken]
  );

  return {
    rollouts,
    latestManifest,
    plugins,
    catalogManifest,
    experiments,
    experimentsManifest,
    advisories,
    advisoriesManifest,
    licenses,
    licenseKeyId,
    licensePublicKey,
    auditLogs,
    loading,
    submitting,
    refreshDistributionOps,
    publishRollout,
    upsertCatalogPlugin,
    bulkUpdateCatalog,
    upsertExperiment,
    issueQuarantine,
    issueLicense,
  };
}
