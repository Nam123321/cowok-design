// @story 08.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { apiPluginCenter } from "../../api/plugin-center";
import type {
  DiscoverOrInstallPluginRequest,
  QuarantinePluginRequest,
  StagingDecisionRequest,
  TransitionPluginRequest,
} from "../../lib/validations/plugins";
import type {
  CompilePromptBundleRequest,
  LearnCardDecisionRequest,
  ProposeLearnCardRequest,
  TriggerSyncRequest,
  UpsertCronScheduleRequest,
} from "../../lib/validations/plugins-learn-cron";
import type {
  DiscoveryDescriptorRequest,
  RepackageBundleRequest,
  TriangleLintRequest,
  VideoIntakeRequest,
} from "../../lib/validations/plugins-repackager";

export const pluginCenterClient = {
  listPlugins: (opts?: { enabled?: boolean; projectId?: string }) => apiPluginCenter.listPlugins(opts),
  getPluginRegistry: (opts?: { projectId?: string }) => apiPluginCenter.getPluginRegistry(opts),
  discoverPlugin: (input: DiscoverOrInstallPluginRequest) => apiPluginCenter.discoverPlugin(input),
  installPlugin: (input: DiscoverOrInstallPluginRequest) => apiPluginCenter.installPlugin(input),
  transitionPlugin: (pluginId: string, input: TransitionPluginRequest) => apiPluginCenter.transitionPlugin(pluginId, input),
  quarantinePlugin: (pluginId: string, input: QuarantinePluginRequest) => apiPluginCenter.quarantinePlugin(pluginId, input),
  promotePluginStaging: (pluginId: string, input: StagingDecisionRequest) => apiPluginCenter.promotePluginStaging(pluginId, input),
  discardPluginStaging: (pluginId: string, input: StagingDecisionRequest) => apiPluginCenter.discardPluginStaging(pluginId, input),
  repackageBundle: (input: RepackageBundleRequest) => apiPluginCenter.repackageBundle(input),
  generateDiscoveryStub: (input: DiscoveryDescriptorRequest) => apiPluginCenter.generateDiscoveryStub(input),
  intakeVideoStoryboard: (input: VideoIntakeRequest) => apiPluginCenter.intakeVideoStoryboard(input),
  runTriangleLint: (input: TriangleLintRequest) => apiPluginCenter.runTriangleLint(input),
  compilePromptBundle: (projectId: string, input: CompilePromptBundleRequest) => apiPluginCenter.compilePromptBundle(projectId, input),
  listLearnCards: (projectId: string) => apiPluginCenter.listLearnCards(projectId),
  proposeLearnCard: (projectId: string, input: ProposeLearnCardRequest) => apiPluginCenter.proposeLearnCard(projectId, input),
  approveLearnCard: (projectId: string, cardId: string, input: LearnCardDecisionRequest) => apiPluginCenter.approveLearnCard(projectId, cardId, input),
  rejectLearnCard: (projectId: string, cardId: string, input: LearnCardDecisionRequest) => apiPluginCenter.rejectLearnCard(projectId, cardId, input),
  getPluginSchedule: (pluginId: string, projectId?: string) => apiPluginCenter.getPluginSchedule(pluginId, projectId),
  upsertPluginSchedule: (pluginId: string, input: UpsertCronScheduleRequest) => apiPluginCenter.upsertPluginSchedule(pluginId, input),
  triggerPluginSyncNow: (pluginId: string, input?: TriggerSyncRequest) => apiPluginCenter.triggerPluginSyncNow(pluginId, input),
  subscribePluginEvents: (projectId: string, opts: { signal?: AbortSignal; onRefresh: () => void; onConnected?: () => void }) =>
    apiPluginCenter.subscribePluginEvents(projectId, opts),
};
