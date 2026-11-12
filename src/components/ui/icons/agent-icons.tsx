// @story 04.1
import * as React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CowokIconProps } from "./core-icons";

const base = {
  fill: "none",
  stroke: "currentColor",
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export const useAgentIconTranslation = useTranslation;

export function BulkSelectIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><line x1="8" y1="6" x2="21" y2="6" /><line x1="8" y1="12" x2="21" y2="12" /><line x1="8" y1="18" x2="21" y2="18" /><line x1="3" y1="6" x2="3.01" y2="6" /><line x1="3" y1="12" x2="3.01" y2="12" /><line x1="3" y1="18" x2="3.01" y2="18" /></svg>;
}

export function LlmConfigIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M5 5h14v14H5zM8 9h8M8 13h4M15 13h1M8 17h8" /><circle cx="16" cy="13" r="1" /></svg>;
}

export function PluginIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M8 3v4M16 3v4M8 17v4M16 17v4" /><rect x="6" y="7" width="12" height="10" rx="2" /><path d="M3 10h3M18 10h3M3 14h3M18 14h3" /></svg>;
}

export function ToolIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M15.2 4.4a4 4 0 0 0-4.9 5.2l-5.8 5.8a2.7 2.7 0 0 0 3.8 3.8l5.8-5.8a4 4 0 0 0 5.2-4.9l-3.1 3.1-3.9-3.9z" /></svg>;
}

export function AiAgentIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="5.2" y="7.1" width="13.6" height="9.8" rx="4.2" /><path d="M12 7.1V4.7M10.4 4.7h3.2M9.1 12h.1M14.8 12h.1M9.7 14.4c1.4.9 3.2.9 4.6 0" /></svg>;
}

export function ProposalIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="4.5" y="4.5" width="15" height="15" rx="3" /><path d="M8 9h8M8 12h5m.5 4 1.5 1.5 3-3" /></svg>;
}

export function DraftIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 1 1 3 3L12 15l-4 1 1-4Z" /></svg>;
}

export function AgentMktBrandIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /><path d="m12 8-1.5 3H7.5l2.5 2-1 3.5 3-2.5 3 2.5-1-3.5 2.5-2H13.5z" /></svg>;
}

export function ShieldCheckIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" /><path d="m9 12 2 2 4-4" /></svg>;
}

export const AGENT_ICON_COMPONENTS = {
  sliders: LlmConfigIcon,
  cpu: LlmConfigIcon,
  "llm-config": LlmConfigIcon,
  models: LlmConfigIcon,
  model: LlmConfigIcon,
  plugin: PluginIcon,
  plugins: PluginIcon,
  mcp: PluginIcon,
  tool: ToolIcon,
  tools: ToolIcon,
  "ai-agent": AiAgentIcon,
  agent: AiAgentIcon,
  proposal: ProposalIcon,
  draft: DraftIcon,
  shield: ShieldCheckIcon,
  "shield-check": ShieldCheckIcon,
  "bulk-select": BulkSelectIcon,
  "brand-agent": AgentMktBrandIcon,
};
