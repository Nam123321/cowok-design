// @story 08.4
// @implements AC-01, AC-04, AC-05
import React from "react";
import { PLUGIN_CENTER_CARDS } from "../../features/plugin-center/cards";
import { PLUGIN_CENTER_ENTRYPOINTS } from "../../features/plugin-center/entrypoints";
import { PluginCenterWorkspace } from "../../features/plugins/PluginCenterWorkspace";
import { registerFeatureEntrypoints, resolveHostSlotEntrypoints } from "../../host/slot-registry";
import { DEFAULT_FLOW_PROJECT_ID } from "../workbench/constants";

registerFeatureEntrypoints(PLUGIN_CENTER_ENTRYPOINTS);

export interface PluginCenterPageProps {
  projectId?: string;
}

function resolveRouteContext(explicitProjectId?: string): {
  projectId: string;
  initialScope: "mcp" | "automations";
  initialLearnFilter: "ALL" | "PROPOSED" | "ACCEPTED" | "REJECTED";
} {
  if (typeof window === "undefined") {
    return { projectId: explicitProjectId || DEFAULT_FLOW_PROJECT_ID, initialScope: "mcp", initialLearnFilter: "ALL" };
  }
  const params = new URLSearchParams(window.location.search);
  let storedProj = "";
  try {
    storedProj = window.localStorage.getItem("cowok.activeProjectId")?.trim() || "";
  } catch {
    storedProj = "";
  }
  const projectId = explicitProjectId || params.get("projectId")?.trim() || storedProj || DEFAULT_FLOW_PROJECT_ID;
  const tab = (params.get("tab") || "").trim().toLowerCase();
  return {
    projectId,
    initialScope: tab === "cron" || tab === "automations" ? "automations" : "mcp",
    initialLearnFilter: tab === "learn" ? "PROPOSED" : "ALL",
  };
}

export const PluginCenterPage: React.FC<PluginCenterPageProps> = ({ projectId }) => {
  const ctx = resolveRouteContext(projectId);
  const slotEntrypoints = resolveHostSlotEntrypoints("host-plugin-center", "catalogSlot");
  const headerTabIds = resolveHostSlotEntrypoints("host-plugin-center", "headerTabsSlot").map((ep) => ep.id);

  return (
    <PluginCenterWorkspace
      projectId={ctx.projectId}
      hostSlotId={slotEntrypoints[0]?.id ?? "plugin-center-registry-catalog"}
      cardSchemaVersion={PLUGIN_CENTER_CARDS.pluginSummaryCard.schemaVersion}
      initialScope={ctx.initialScope}
      initialLearnFilter={ctx.initialLearnFilter}
      headerTabIds={headerTabIds}
    />
  );
};
