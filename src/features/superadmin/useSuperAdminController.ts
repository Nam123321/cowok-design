// @story 14.2
// @implements AC-02, AC-03, AC-04 (SuperAdmin Portal State Controller — OIDC+MFA Session, Debounced Filters, Race-Safe Refetch, 2-Way Helpdesk, Clusters, Funnel & Crash Triage)
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  apiCloudAdmin,
  CloudAdminApiError,
  type CloudAdminApiClient,
  type CloudAdminTicketFilters,
} from "../../api/cloud-admin";
import type { CowokToastTone } from "../../components/ui/CowokToast";
import type {
  AdminOperator,
  AnalyticsFunnelData,
  CloudAdminRole,
  CloudFeatureCluster,
  CloudSupportTicket,
  CloudTicketPriority,
  CloudTicketStatus,
  CrashGroup,
} from "../../lib/validations/cloud-admin";

const MFA_CODE_LENGTH = 6;
const SEARCH_DEBOUNCE_MS = 250;

export type SuperAdminTabKey = "helpdesk" | "clusters" | "funnel" | "crashes";

export type SuperAdminApiPort = Pick<
  CloudAdminApiClient,
  | "createAdminSession"
  | "listTickets"
  | "replyToTicket"
  | "listClusters"
  | "createCluster"
  | "getAnalyticsFunnel"
  | "listCrashes"
> &
  Partial<Pick<CloudAdminApiClient, "fetchTicketAttachment">>;

export interface SuperAdminToastState {
  tone: CowokToastTone;
  message: string;
}

export interface SuperAdminAuthForm {
  oidcProvider: string;
  email: string;
  role: CloudAdminRole;
  mfaCode: string;
}

export interface TicketReplySubmitInput {
  ticketId: string;
  body: string;
  priority: CloudTicketPriority;
  status: CloudTicketStatus;
}

export interface ClusterCreateInput {
  title: string;
  summary: string;
  ticketIds: string[];
  status: "open" | "planned" | "shipped" | "declined";
}

function formatControllerError(err: unknown, fallback: string): string {
  if (err instanceof CloudAdminApiError) return `${err.code}: ${err.message}`;
  if (err instanceof Error && err.message) return err.message;
  return fallback;
}

function defaultAuthForm(): SuperAdminAuthForm {
  return { oidcProvider: "google", email: "ops-admin@cowok.design", role: "SUPER_ADMIN", mfaCode: "" };
}

function defaultTicketFilters(): CloudAdminTicketFilters {
  return { priority: "ALL", status: "all", channel: "all", search: "" };
}

function buildAuthPayload(form: SuperAdminAuthForm) {
  const trimmedCode = form.mfaCode.trim();
  return {
    oidcProvider: form.oidcProvider,
    subject: `oidc:${form.email.trim().toLowerCase()}`,
    email: form.email.trim(),
    role: form.role,
    mfaVerified: trimmedCode.length === MFA_CODE_LENGTH,
    mfaCode: trimmedCode,
  };
}

export function canRoleReplyTickets(role?: CloudAdminRole): boolean {
  return role === "SUPER_ADMIN" || role === "SUPPORT_AGENT";
}

export function canRoleManageClusters(role?: CloudAdminRole): boolean {
  return role === "SUPER_ADMIN" || role === "SUPPORT_AGENT" || role === "SKILL_CURATOR";
}

function useDebouncedTicketFilters(filters: CloudAdminTicketFilters): CloudAdminTicketFilters {
  const [debounced, setDebounced] = useState<CloudAdminTicketFilters>(filters);
  const prevSearchRef = useRef<string>(filters.search ?? "");

  useEffect(() => {
    const nextSearch = filters.search ?? "";
    if (nextSearch === prevSearchRef.current) {
      setDebounced(filters);
      return;
    }
    prevSearchRef.current = nextSearch;
    const timer = setTimeout(() => setDebounced(filters), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [filters]);

  return debounced;
}

function useSuperAdminAuth(
  client: SuperAdminApiPort,
  onToast: (toast: SuperAdminToastState) => void
) {
  const [sessionToken, setSessionToken] = useState<string | null>(null);
  const [operator, setOperator] = useState<AdminOperator | null>(null);
  const [authForm, setAuthForm] = useState<SuperAdminAuthForm>(defaultAuthForm);
  const [authPending, setAuthPending] = useState(false);

  const signInOidcMfa = useCallback(
    async (override?: Partial<SuperAdminAuthForm>) => {
      const form = { ...authForm, ...override };
      setAuthPending(true);
      try {
        const res = await client.createAdminSession(buildAuthPayload(form), override?.role ? sessionToken : undefined);
        setAuthForm(form);
        setSessionToken(res.data.sessionToken);
        setOperator(res.data.operator);
        onToast({ tone: "success", message: `OIDC + MFA verified (${res.data.operator.role})` });
      } catch (err) {
        onToast({ tone: "danger", message: formatControllerError(err, "OIDC+MFA authentication failed") });
      } finally {
        setAuthPending(false);
      }
    },
    [authForm, client, onToast, sessionToken]
  );

  const previewRole = useCallback(
    (role: CloudAdminRole) => {
      if (operator?.role === "SUPER_ADMIN") void signInOidcMfa({ role });
    },
    [operator?.role, signInOidcMfa]
  );

  const signOut = useCallback(() => {
    setSessionToken(null);
    setOperator(null);
    setAuthForm((prev) => ({ ...prev, mfaCode: "" }));
  }, []);

  return { sessionToken, operator, authForm, setAuthForm, authPending, signInOidcMfa, previewRole, signOut };
}

function useTicketLoader(
  client: SuperAdminApiPort,
  sessionToken: string | null,
  debouncedFilters: CloudAdminTicketFilters,
  onToast: (toast: SuperAdminToastState) => void
) {
  const [tickets, setTickets] = useState<CloudSupportTicket[]>([]);
  const [loading, setLoading] = useState(false);
  const ticketReqSeqRef = useRef(0);

  const loadTickets = useCallback(
    async (tok: string | null, activeFilters: CloudAdminTicketFilters) => {
      if (!tok) return;
      const seq = ++ticketReqSeqRef.current;
      setLoading(true);
      try {
        const tRes = await client.listTickets(activeFilters, tok);
        if (seq === ticketReqSeqRef.current) setTickets(tRes.data.tickets);
      } catch (err) {
        if (seq === ticketReqSeqRef.current) {
          onToast({ tone: "danger", message: formatControllerError(err, "Failed to load tickets") });
        }
      } finally {
        if (seq === ticketReqSeqRef.current) setLoading(false);
      }
    },
    [client, onToast]
  );

  useEffect(() => {
    if (!sessionToken) {
      ticketReqSeqRef.current += 1;
      setTickets([]);
      return;
    }
    void loadTickets(sessionToken, debouncedFilters);
  }, [sessionToken, debouncedFilters, loadTickets]);

  return { tickets, setTickets, loading, loadTickets };
}

function fetchOverviewBundle(client: SuperAdminApiPort, tok: string) {
  return Promise.all([
    client.listClusters(tok),
    client.getAnalyticsFunnel(tok),
    client.listCrashes({}, tok),
  ]);
}

function useOverviewLoader(
  client: SuperAdminApiPort,
  sessionToken: string | null,
  onToast: (toast: SuperAdminToastState) => void
) {
  const [clusters, setClusters] = useState<CloudFeatureCluster[]>([]);
  const [funnel, setFunnel] = useState<AnalyticsFunnelData | null>(null);
  const [crashes, setCrashes] = useState<CrashGroup[]>([]);
  const overviewReqSeqRef = useRef(0);

  const loadOverview = useCallback(
    async (tok: string | null) => {
      if (!tok) return;
      const seq = ++overviewReqSeqRef.current;
      try {
        const [cRes, fRes, crRes] = await fetchOverviewBundle(client, tok);
        if (seq !== overviewReqSeqRef.current) return;
        setClusters(cRes.data.clusters);
        setFunnel(fRes.data);
        setCrashes(crRes.data.crashes);
      } catch (err) {
        if (seq === overviewReqSeqRef.current) {
          onToast({ tone: "danger", message: formatControllerError(err, "Failed to load overview metrics") });
        }
      }
    },
    [client, onToast]
  );

  const refreshFunnel = useCallback(async () => {
    if (!sessionToken) return;
    const seq = ++overviewReqSeqRef.current;
    try {
      const fRes = await client.getAnalyticsFunnel(sessionToken);
      if (seq === overviewReqSeqRef.current) setFunnel(fRes.data);
    } catch { /* non-fatal KPI refresh */ }
  }, [client, sessionToken]);

  useEffect(() => {
    if (!sessionToken) {
      overviewReqSeqRef.current += 1;
      setClusters([]); setFunnel(null); setCrashes([]);
      return;
    }
    void loadOverview(sessionToken);
  }, [sessionToken, loadOverview]);

  return { clusters, setClusters, funnel, crashes, loadOverview, refreshFunnel };
}

interface MutationBaseParams {
  client: SuperAdminApiPort;
  sessionToken: string | null;
  setSubmitting: React.Dispatch<React.SetStateAction<boolean>>;
  onToast: (toast: SuperAdminToastState) => void;
  refreshFunnel: () => Promise<void>;
}

function useReplyMutation(
  params: MutationBaseParams & {
    setTickets: React.Dispatch<React.SetStateAction<CloudSupportTicket[]>>;
  }
) {
  const { client, sessionToken, setTickets, setSubmitting, onToast, refreshFunnel } = params;
  return useCallback(
    async (input: TicketReplySubmitInput) => {
      if (!sessionToken) return false;
      setSubmitting(true);
      try {
        const res = await client.replyToTicket(
          input.ticketId,
          { body: input.body, priority: input.priority, status: input.status },
          sessionToken
        );
        setTickets((prev) => prev.map((it) => (it.id === res.data.ticket.id ? res.data.ticket : it)));
        const syncMsg = res.data.reply.syncedToClient
          ? "Reply synced 2-way to client inbox & audit logged"
          : "Reply saved to cloud ticket & audit logged";
        onToast({ tone: "success", message: syncMsg });
        void refreshFunnel();
        return true;
      } catch (err) {
        onToast({ tone: "danger", message: formatControllerError(err, "Reply failed") });
        return false;
      } finally {
        setSubmitting(false);
      }
    },
    [client, onToast, refreshFunnel, sessionToken, setSubmitting, setTickets]
  );
}

function useClusterMutation(
  params: MutationBaseParams & {
    setClusters: React.Dispatch<React.SetStateAction<CloudFeatureCluster[]>>;
    refreshTickets: () => Promise<void>;
  }
) {
  const { client, sessionToken, setClusters, setSubmitting, onToast, refreshFunnel, refreshTickets } = params;
  return useCallback(
    async (input: ClusterCreateInput) => {
      if (!sessionToken) return false;
      setSubmitting(true);
      try {
        const res = await client.createCluster(input, sessionToken);
        setClusters((prev) => [res.data.cluster, ...prev.filter((c) => c.id !== res.data.cluster.id)]);
        onToast({ tone: "success", message: "Feature cluster saved with 1-click GitHub link" });
        void Promise.all([refreshTickets(), refreshFunnel()]);
        return true;
      } catch (err) {
        onToast({ tone: "danger", message: formatControllerError(err, "Cluster save failed") });
        return false;
      } finally {
        setSubmitting(false);
      }
    },
    [client, onToast, refreshFunnel, refreshTickets, sessionToken, setClusters, setSubmitting]
  );
}

function useSuperAdminMutations(params: {
  client: SuperAdminApiPort;
  sessionToken: string | null;
  setTickets: React.Dispatch<React.SetStateAction<CloudSupportTicket[]>>;
  setClusters: React.Dispatch<React.SetStateAction<CloudFeatureCluster[]>>;
  onToast: (toast: SuperAdminToastState) => void;
  refreshFunnel: () => Promise<void>;
  refreshTickets: () => Promise<void>;
}) {
  const [submitting, setSubmitting] = useState(false);
  const base: MutationBaseParams = {
    client: params.client,
    sessionToken: params.sessionToken,
    setSubmitting,
    onToast: params.onToast,
    refreshFunnel: params.refreshFunnel,
  };
  const submitTicketReply = useReplyMutation({ ...base, setTickets: params.setTickets });
  const createFeatureCluster = useClusterMutation({
    ...base,
    setClusters: params.setClusters,
    refreshTickets: params.refreshTickets,
  });
  return { submitting, submitTicketReply, createFeatureCluster };
}

function useAttachmentFetcher(client: SuperAdminApiPort, sessionToken: string | null) {
  return useCallback(
    async (ticketId: string): Promise<Blob | null> => {
      if (!sessionToken || !client.fetchTicketAttachment) return null;
      try {
        return await client.fetchTicketAttachment(ticketId, sessionToken);
      } catch {
        return null;
      }
    },
    [client, sessionToken]
  );
}

export function useSuperAdminController(client: SuperAdminApiPort = apiCloudAdmin) {
  const [activeTab, setActiveTab] = useState<SuperAdminTabKey>("helpdesk");
  const [filters, setFilters] = useState<CloudAdminTicketFilters>(defaultTicketFilters);
  const [selectedTicketId, setSelectedTicketId] = useState<string | null>(null);
  const [toast, setToast] = useState<SuperAdminToastState | null>(null);

  const notify = useCallback((next: SuperAdminToastState) => setToast(next), []);
  const debouncedFilters = useDebouncedTicketFilters(filters);
  const auth = useSuperAdminAuth(client, notify);
  const ticketLoader = useTicketLoader(client, auth.sessionToken, debouncedFilters, notify);
  const overviewLoader = useOverviewLoader(client, auth.sessionToken, notify);

  const refreshTickets = useCallback(
    () => ticketLoader.loadTickets(auth.sessionToken, debouncedFilters),
    [auth.sessionToken, debouncedFilters, ticketLoader]
  );
  const mutations = useSuperAdminMutations({
    client,
    sessionToken: auth.sessionToken,
    setTickets: ticketLoader.setTickets,
    setClusters: overviewLoader.setClusters,
    onToast: notify,
    refreshFunnel: overviewLoader.refreshFunnel,
    refreshTickets,
  });
  const selectedTicket = useMemo(
    () => ticketLoader.tickets.find((t) => t.id === selectedTicketId) ?? null,
    [selectedTicketId, ticketLoader.tickets]
  );
  const refreshAll = useCallback(() => {
    void Promise.all([refreshTickets(), overviewLoader.loadOverview(auth.sessionToken)]);
  }, [auth.sessionToken, overviewLoader, refreshTickets]);
  const loadTicketAttachment = useAttachmentFetcher(client, auth.sessionToken);
  const handleSignOut = useCallback(() => {
    setSelectedTicketId(null);
    auth.signOut();
  }, [auth]);

  return {
    activeTab, setActiveTab, ...auth, signOut: handleSignOut, filters, setFilters,
    tickets: ticketLoader.tickets, clusters: overviewLoader.clusters, funnel: overviewLoader.funnel,
    crashes: overviewLoader.crashes, loading: ticketLoader.loading, ...mutations,
    selectedTicket, selectedTicketId, setSelectedTicketId, refreshAll, loadTicketAttachment,
    toast, dismissToast: () => setToast(null),
  };
}
