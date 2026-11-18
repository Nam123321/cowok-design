// @story 08.4
// @implements AC-01, AC-04, AC-05
import type { FeatureEntrypointContract } from "../../host/slot-registry";

const HEADER_TAB_ORDER_LEARN = 10;
const HEADER_TAB_ORDER_CRON = 20;
const CATALOG_ORDER = 5;

export const PLUGIN_CENTER_ENTRYPOINTS: readonly FeatureEntrypointContract[] = [
  {
    id: "plugin-center-registry-catalog",
    featureSlug: "plugin-center",
    hostShellId: "host-plugin-center",
    slotId: "catalogSlot",
    slotCardinality: "single",
    order: CATALOG_ORDER,
    routePath: "/plugins",
    labelKey: "plugins.registryTitle",
    designPatternRef: "DataTableView [5.21]",
  },
  {
    id: "plugin-center-learn-tab",
    featureSlug: "plugin-center",
    hostShellId: "host-plugin-center",
    slotId: "headerTabsSlot",
    slotCardinality: "ordered-tabs",
    order: HEADER_TAB_ORDER_LEARN,
    routePath: "/plugins?tab=learn",
    labelKey: "learn.queueTitle",
    designPatternRef: "Segmented Tab Navigation [5.14]",
  },
  {
    id: "plugin-center-cron-tab",
    featureSlug: "plugin-center",
    hostShellId: "host-plugin-center",
    slotId: "headerTabsSlot",
    slotCardinality: "ordered-tabs",
    order: HEADER_TAB_ORDER_CRON,
    routePath: "/plugins?tab=cron",
    labelKey: "cron.statusTitle",
    designPatternRef: "Segmented Tab Navigation [5.14]",
  },
];
