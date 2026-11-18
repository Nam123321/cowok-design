// @story 08.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10
import * as pluginsRoute from "../app/api/plugins/route";
import * as pluginsRegistryRoute from "../app/api/plugins/registry/route";
import * as pluginsDiscoveryRoute from "../app/api/plugins/discovery/route";
import * as pluginsInstallsRoute from "../app/api/plugins/installs/route";
import * as pluginTransitionsRoute from "../app/api/plugins/[id]/transitions/route";
import * as pluginQuarantineRoute from "../app/api/plugins/[id]/quarantine/route";
import * as pluginStagingPromotionsRoute from "../app/api/plugins/[id]/staging/promotions/route";
import * as pluginStagingDiscardsRoute from "../app/api/plugins/[id]/staging/discards/route";
import * as repackageBundlesRoute from "../app/api/plugins/repackager/bundles/route";
import * as repackageStubsRoute from "../app/api/plugins/repackager/discovery-stubs/route";
import * as repackageVideoRoute from "../app/api/plugins/repackager/video-intakes/route";
import * as linterChecksRoute from "../app/api/plugins/linter/checks/route";
import * as pluginScheduleRoute from "../app/api/plugins/[id]/schedule/route";
import * as pluginSyncJobsRoute from "../app/api/plugins/[id]/sync-jobs/route";
import * as skillPromptBundlesRoute from "../app/api/projects/[id]/skills/prompt-bundles/route";
import * as learnCardsRoute from "../app/api/projects/[id]/learn/cards/route";
import * as learnCardApprovalsRoute from "../app/api/projects/[id]/learn/cards/[cardId]/approvals/route";
import * as learnCardRejectionsRoute from "../app/api/projects/[id]/learn/cards/[cardId]/rejections/route";
import * as pluginStreamRoute from "../app/api/projects/[id]/plugins/stream/route";
import { safeDecodeSegment, type RouteEntry } from "./vite-bff-flow-routes";

const noParams = (): Record<string, string> => ({});
const idOnly = (m: RegExpExecArray): Record<string, string> => ({ id: safeDecodeSegment(m[1]) });
const projectAndCard = (m: RegExpExecArray): Record<string, string> => ({
  id: safeDecodeSegment(m[1]),
  cardId: safeDecodeSegment(m[2]),
});

export const PLUGIN_ECOSYSTEM_ROUTES: RouteEntry[] = [
  { method: "GET", pattern: /^\/api\/plugins$/, extractParams: noParams, resolveHandler: () => pluginsRoute.GET },
  { method: "GET", pattern: /^\/api\/plugins\/registry$/, extractParams: noParams, resolveHandler: () => pluginsRegistryRoute.GET },
  { method: "POST", pattern: /^\/api\/plugins\/discovery$/, extractParams: noParams, resolveHandler: () => pluginsDiscoveryRoute.POST },
  { method: "POST", pattern: /^\/api\/plugins\/installs$/, extractParams: noParams, resolveHandler: () => pluginsInstallsRoute.POST },
  { method: "POST", pattern: /^\/api\/plugins\/([^/]+)\/transitions$/, extractParams: idOnly, resolveHandler: () => pluginTransitionsRoute.POST },
  { method: "POST", pattern: /^\/api\/plugins\/([^/]+)\/quarantine$/, extractParams: idOnly, resolveHandler: () => pluginQuarantineRoute.POST },
  { method: "POST", pattern: /^\/api\/plugins\/([^/]+)\/staging\/promotions$/, extractParams: idOnly, resolveHandler: () => pluginStagingPromotionsRoute.POST },
  { method: "POST", pattern: /^\/api\/plugins\/([^/]+)\/staging\/discards$/, extractParams: idOnly, resolveHandler: () => pluginStagingDiscardsRoute.POST },
  { method: "POST", pattern: /^\/api\/plugins\/repackager\/bundles$/, extractParams: noParams, resolveHandler: () => repackageBundlesRoute.POST },
  { method: "POST", pattern: /^\/api\/plugins\/repackager\/discovery-stubs$/, extractParams: noParams, resolveHandler: () => repackageStubsRoute.POST },
  { method: "POST", pattern: /^\/api\/plugins\/repackager\/video-intakes$/, extractParams: noParams, resolveHandler: () => repackageVideoRoute.POST },
  { method: "POST", pattern: /^\/api\/plugins\/linter\/checks$/, extractParams: noParams, resolveHandler: () => linterChecksRoute.POST },
  { method: "GET", pattern: /^\/api\/plugins\/([^/]+)\/schedule$/, extractParams: idOnly, resolveHandler: () => pluginScheduleRoute.GET },
  { method: "PUT", pattern: /^\/api\/plugins\/([^/]+)\/schedule$/, extractParams: idOnly, resolveHandler: () => pluginScheduleRoute.PUT },
  { method: "POST", pattern: /^\/api\/plugins\/([^/]+)\/sync-jobs$/, extractParams: idOnly, resolveHandler: () => pluginSyncJobsRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/skills\/prompt-bundles$/, extractParams: idOnly, resolveHandler: () => skillPromptBundlesRoute.POST },
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/learn\/cards$/, extractParams: idOnly, resolveHandler: () => learnCardsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/learn\/cards$/, extractParams: idOnly, resolveHandler: () => learnCardsRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/learn\/cards\/([^/]+)\/approvals$/, extractParams: projectAndCard, resolveHandler: () => learnCardApprovalsRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/learn\/cards\/([^/]+)\/rejections$/, extractParams: projectAndCard, resolveHandler: () => learnCardRejectionsRoute.POST },
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/plugins\/stream$/, extractParams: idOnly, resolveHandler: () => pluginStreamRoute.GET },
];
