// @story 04.1
import type * as React from "react";
import { useTranslation } from "../../hooks/useTranslation";
import { AGENT_ICON_COMPONENTS, AgentMktBrandIcon, AiAgentIcon, DraftIcon, LlmConfigIcon, PluginIcon, ProposalIcon, ToolIcon } from "./icons/agent-icons";
import {
  CORE_ICON_COMPONENTS,
  type CowokIconProps,
  DeployIcon,
  EditIcon,
  GlobeIcon,
  MapIcon,
  MessageSquareIcon,
  PinIcon,
  PlusIcon,
  SearchIcon,
  SendIcon,
  SettingsIcon,
  Trash2Icon,
} from "./icons/core-icons";
import {
  NAV_ICON_COMPONENTS,
  ArrowDownIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ExternalLinkIcon,
  EyeIcon,
  LayersIcon,
  MoonIcon,
  PanelLeftCloseIcon,
  PanelRightIcon,
  RefreshIcon,
  ReplyIcon,
  SunIcon,
  TerminalIcon,
  XIcon,
} from "./icons/navigation-icons";
import { WORKFLOW_ICON_COMPONENTS, WorkflowIcon, WorkspaceIcon } from "./icons/workflow-icons";
import { FLAGSHIP_DOMAIN_ICON_COMPONENTS } from "./icons/flagship-domain-icons";

export type { CowokIconProps };
export const useIconTranslation = useTranslation;

export const COWOK_ICON_REGISTRY: Record<string, React.ComponentType<CowokIconProps>> = {
  ...FLAGSHIP_DOMAIN_ICON_COMPONENTS,
  ...CORE_ICON_COMPONENTS,
  ...NAV_ICON_COMPONENTS,
  ...WORKFLOW_ICON_COMPONENTS,
  ...AGENT_ICON_COMPONENTS,
};

export { WorkflowIcon, WorkspaceIcon, SettingsIcon, PlusIcon, Trash2Icon, DeployIcon, MapIcon, SendIcon };
