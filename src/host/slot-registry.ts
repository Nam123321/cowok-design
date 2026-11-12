/**
 * @file src/host/slot-registry.ts
 * @description Declarative Host Slot Composition Root enforcing the Five Public Nouns
 * architecture (Feature, Entrypoint, Card, Client, Host) and Slot Cardinality invariants
 * (EC-SPRINT3-REV6-02, R-NOUN5: zero imports from src/features/**).
 */

export type HostSlotId =
  | 'contextBannerSlot'
  | 'unifiedFilterBarSlot'
  | 'headerTabsSlot'
  | 'catalogSlot'
  | 'rightDrawerSlot';

export type SlotCardinality = 'single' | 'ordered-tabs' | 'contextual';

export interface FeatureEntrypointContract {
  id: string;
  featureSlug: string;
  hostShellId: string;
  slotId: HostSlotId;
  slotCardinality: SlotCardinality;
  order: number;
  routePath: string;
  labelKey?: string;
  queryParamTrigger?: string;
  designPatternRef: string;
}

export interface FeatureCardDescriptor<TSchema = unknown> {
  cardId: string;
  featureSlug: string;
  titleKey: string;
  designPatternRef: string;
  schemaVersion: string;
  payloadSchema: TSchema;
}

export interface HostShellSlotPolicy {
  hostShellId: string;
  routePrefix: string;
  slots: Record<HostSlotId, { cardinality: SlotCardinality; description: string }>;
}

export const HOST_SHELL_POLICIES: Record<string, HostShellSlotPolicy> = {
  'host-plugin-center': {
    hostShellId: 'host-plugin-center',
    routePrefix: '/plugins',
    slots: {
      contextBannerSlot: {
        cardinality: 'contextual',
        description: 'Contextual origin-return breadcrumb banner (?origin=workflow|agent|audit)',
      },
      headerTabsSlot: {
        cardinality: 'ordered-tabs',
        description: 'Primary view mode tabs (Workspace Inventory vs Curated Kits)',
      },
      unifiedFilterBarSlot: {
        cardinality: 'single',
        description: 'Single unified search & filter bar with Catalog Filter vs AI Semantic Discovery mode switch',
      },
      catalogSlot: {
        cardinality: 'single',
        description: 'Primary catalog table/grid or Tombstone Fallback Card when a tombstoned package is inspected',
      },
      rightDrawerSlot: {
        cardinality: 'contextual',
        description: 'Contextual inspection drawer for kit capabilities and audit history',
      },
    },
  },
  'host-agents-console': {
    hostShellId: 'host-agents-console',
    routePrefix: '/agents',
    slots: {
      contextBannerSlot: {
        cardinality: 'contextual',
        description: 'Optional cross-surface context banner when deep-linked from Plugin Center',
      },
      headerTabsSlot: {
        cardinality: 'ordered-tabs',
        description: 'Digital Workforce Setup top-level tabs: Agent Directory & Cron Schedulers',
      },
      unifiedFilterBarSlot: {
        cardinality: 'single',
        description: 'Single unified perspective and search bar for active tab',
      },
      catalogSlot: {
        cardinality: 'single',
        description: 'Active tab workspace canvas (AgentDirectoryDashboard or CronSchedulerView)',
      },
      rightDrawerSlot: {
        cardinality: 'contextual',
        description: 'Immutable version history & hot-swap comparison drawer',
      },
    },
  },
};

const REGISTERED_ENTRYPOINTS_MAP = new Map<string, FeatureEntrypointContract>([
  [
    'plugin-center-origin-return-banner',
    {
      id: 'plugin-center-origin-return-banner',
      featureSlug: 'plugin-center',
      hostShellId: 'host-plugin-center',
      slotId: 'contextBannerSlot',
      slotCardinality: 'contextual',
      order: 10,
      routePath: '/plugins',
      labelKey: 'navigation.backToOrigin',
      queryParamTrigger: 'origin',
      designPatternRef: 'Contextual Breadcrumb Banner [5.14]',
    },
  ],
  [
    'plugin-center-unified-filter-bar',
    {
      id: 'plugin-center-unified-filter-bar',
      featureSlug: 'plugin-center',
      hostShellId: 'host-plugin-center',
      slotId: 'unifiedFilterBarSlot',
      slotCardinality: 'single',
      order: 10,
      routePath: '/plugins',
      labelKey: 'pluginDiscovery.searchMode.label',
      designPatternRef: 'AdvancedFilterBar [5.22]',
    },
  ],
  [
    'plugin-center-curated-kit-catalog',
    {
      id: 'plugin-center-curated-kit-catalog',
      featureSlug: 'plugin-center',
      hostShellId: 'host-plugin-center',
      slotId: 'catalogSlot',
      slotCardinality: 'single',
      order: 10,
      routePath: '/plugins',
      labelKey: 'pluginDiscovery.tabs.curatedKits',
      designPatternRef: 'DataTableView [5.21]',
    },
  ],
  [
    'plugin-center-inspection-drawer',
    {
      id: 'plugin-center-inspection-drawer',
      featureSlug: 'plugin-center',
      hostShellId: 'host-plugin-center',
      slotId: 'rightDrawerSlot',
      slotCardinality: 'contextual',
      order: 10,
      routePath: '/plugins',
      labelKey: 'pluginDiscovery.drawer.title',
      queryParamTrigger: 'kitId',
      designPatternRef: 'RightDrawer [5.25]',
    },
  ],
  [
    'plugin-studio-operation-inspector-drawer',
    {
      id: 'plugin-studio-operation-inspector-drawer',
      featureSlug: 'plugin-studio',
      hostShellId: 'host-plugin-center',
      slotId: 'rightDrawerSlot',
      slotCardinality: 'contextual',
      order: 20,
      routePath: '/plugins',
      labelKey: 'pluginStudio.drawer.title',
      queryParamTrigger: 'studioPackageId',
      designPatternRef: 'MasterDetailChatDrawer [5.46]',
    },
  ],
  [
    'plugin-studio-bulk-resolution-drawer',
    {
      id: 'plugin-studio-bulk-resolution-drawer',
      featureSlug: 'plugin-studio',
      hostShellId: 'host-plugin-center',
      slotId: 'rightDrawerSlot',
      slotCardinality: 'contextual',
      order: 30,
      routePath: '/plugins',
      labelKey: 'pluginStudio.bulk.resolutionDrawerTitle',
      queryParamTrigger: 'bulkResolution',
      designPatternRef: 'RightDrawer [5.25]',
    },
  ],
  [
    'agent-directory-header-tab',
    {
      id: 'agent-directory-header-tab',
      featureSlug: 'agent-directory',
      hostShellId: 'host-agents-console',
      slotId: 'headerTabsSlot',
      slotCardinality: 'ordered-tabs',
      order: 10,
      routePath: '/agents',
      labelKey: 'agentHotSwap.tabs.agentDirectory',
      designPatternRef: 'Segmented Tab Navigation [5.14]',
    },
  ],
  [
    'agent-directory-kpi-and-filter-bar',
    {
      id: 'agent-directory-kpi-and-filter-bar',
      featureSlug: 'agent-directory',
      hostShellId: 'host-agents-console',
      slotId: 'unifiedFilterBarSlot',
      slotCardinality: 'single',
      order: 10,
      routePath: '/agents',
      labelKey: 'agentHotSwap.filter.perspectiveLabel',
      designPatternRef: 'Metric Line [5.10] + AdvancedFilterBar [5.22]',
    },
  ],
  [
    'agent-directory-catalog-table',
    {
      id: 'agent-directory-catalog-table',
      featureSlug: 'agent-directory',
      hostShellId: 'host-agents-console',
      slotId: 'catalogSlot',
      slotCardinality: 'single',
      order: 10,
      routePath: '/agents',
      labelKey: 'agentHotSwap.table.title',
      designPatternRef: 'DataTableView [5.21]',
    },
  ],
  [
    'agent-directory-hotswap-drawer',
    {
      id: 'agent-directory-hotswap-drawer',
      featureSlug: 'agent-directory',
      hostShellId: 'host-agents-console',
      slotId: 'rightDrawerSlot',
      slotCardinality: 'contextual',
      order: 10,
      routePath: '/agents',
      labelKey: 'agentHotSwap.drawer.title',
      queryParamTrigger: 'agentId',
      designPatternRef: 'RightDrawer [5.25]',
    },
  ],
  [
    'cron-schedulers-header-tab',
    {
      id: 'cron-schedulers-header-tab',
      featureSlug: 'cron-schedulers',
      hostShellId: 'host-agents-console',
      slotId: 'headerTabsSlot',
      slotCardinality: 'ordered-tabs',
      order: 20,
      routePath: '/agents?tab=cron',
      labelKey: 'agentHotSwap.tabs.cronSchedulers',
      designPatternRef: 'Segmented Tab Navigation [5.14]',
    },
  ],
]);

/**
 * Inversion-of-control registration entrypoint so features register into the Host contract
 * without src/host/** ever importing from src/features/** (R-NOUN5).
 */
export function registerFeatureEntrypoints(
  entrypoints: readonly FeatureEntrypointContract[],
): readonly FeatureEntrypointContract[] {
  for (const ep of entrypoints) {
    REGISTERED_ENTRYPOINTS_MAP.set(ep.id, ep);
  }
  return entrypoints;
}

/**
 * Resolve and sort entrypoints for a given host shell and slot, enforcing slot cardinality.
 */
export function resolveHostSlotEntrypoints(
  hostShellId: string,
  slotId: HostSlotId,
  activeQueryParams?: URLSearchParams,
): FeatureEntrypointContract[] {
  const policy = HOST_SHELL_POLICIES[hostShellId];
  if (!policy) {
    return [];
  }

  const slotPolicy = policy.slots[slotId];
  const matched = Array.from(REGISTERED_ENTRYPOINTS_MAP.values())
    .filter((ep) => ep.hostShellId === hostShellId && ep.slotId === slotId)
    .sort((a, b) => a.order - b.order);

  if (slotPolicy.cardinality === 'contextual' && activeQueryParams) {
    return matched.filter((ep) => {
      if (!ep.queryParamTrigger) return true;
      return activeQueryParams.has(ep.queryParamTrigger);
    });
  }

  if (slotPolicy.cardinality === 'single' && matched.length > 1) {
    return [matched[0]];
  }

  return matched;
}

export function getAllDiscoveredEntrypoints(): readonly FeatureEntrypointContract[] {
  return Array.from(REGISTERED_ENTRYPOINTS_MAP.values());
}
