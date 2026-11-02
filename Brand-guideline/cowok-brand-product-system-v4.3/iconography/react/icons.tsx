import * as React from 'react';

export type CowokIconProps = React.SVGProps<SVGSVGElement> & { size?: number; strokeWidth?: number };
const base = { fill:'none', stroke:'currentColor', strokeLinecap:'round' as const, strokeLinejoin:'round' as const };

export function ActivityIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M3.5 13h4l2-5 3.2 9 2.1-5H20.5"/>`}} />;
}

export function AiAgentIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="5.2" y="7.1" width="13.6" height="9.8" rx="4.2"/><path d="M12 7.1V4.7M10.4 4.7h3.2"/><path d="M9.1 12h.1M14.8 12h.1M9.7 14.4c1.4.9 3.2.9 4.6 0"/>`}} />;
}

export function AiEvolutionIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M5 17.8V13M10 17.8V9.5M15 17.8V6.2M20 17.8V3.8"/><path d="M4.5 20h16"/><path d="m16.8 5.8 2.9-2.9"/>`}} />;
}

export function ApprovalIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M12 3.3 5.3 6v4.8c0 4.3 2.4 7.3 6.7 9.8 4.3-2.5 6.7-5.5 6.7-9.8V6z"/><path d="m8.8 12.1 2.1 2.1 4.5-4.5"/>`}} />;
}

export function AutomationIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="5" y="5" width="14" height="14" rx="3.3"/><path d="m10.2 9.1 4.7 2.9-4.7 2.9z"/><path d="M8 3.7h8"/>`}} />;
}

export function ConnectorIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M8.5 8.4 6.8 6.7a2.8 2.8 0 0 0-4 4l2.4 2.4a2.8 2.8 0 0 0 4 0l1-1"/><path d="m15.5 15.6 1.7 1.7a2.8 2.8 0 0 0 4-4l-2.4-2.4a2.8 2.8 0 0 0-4 0l-1 1"/><path d="m8.5 15.5 7-7"/>`}} />;
}

export function DepartmentIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="4" y="4.5" width="16" height="15" rx="2.7"/><path d="M8 9h3M8 13h3M14 9h2M14 13h2M8 17h8"/>`}} />;
}

export function ExecutionPipelineIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="5" cy="12" r="1.8"/><rect x="9.1" y="9.2" width="5.8" height="5.6" rx="1.8"/><circle cx="19" cy="12" r="1.8"/><path d="M6.8 12h2.3M14.9 12h2.3"/>`}} />;
}

export function HandoffIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="6.8" cy="7" r="2.2"/><circle cx="17.2" cy="17" r="2.2"/><path d="M9.1 8.2c3 1 4.5 2.2 5.6 4.7M14.6 6.2H18v3.4M9.4 17.8H6v-3.4"/>`}} />;
}

export function HumanStaffIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="7" r="2.8"/><path d="M5.7 20c.6-4.1 2.7-6.2 6.3-6.2s5.7 2.1 6.3 6.2"/>`}} />;
}

export function IntegrationIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3.8" y="7" width="6.4" height="10" rx="2.2"/><rect x="13.8" y="7" width="6.4" height="10" rx="2.2"/><path d="M10.2 10h3.6M10.2 14h3.6"/>`}} />;
}

export function KnowledgeBaseIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M4.4 5.4c2.7-.8 5.1-.3 7.6 1.4v12c-2.5-1.7-4.9-2.2-7.6-1.4z"/><path d="M19.6 5.4c-2.7-.8-5.1-.3-7.6 1.4v12c2.5-1.7 4.9-2.2 7.6-1.4z"/><path d="M8.1 9.8h1.7M14.2 9.8h1.7"/>`}} />;
}

export function ProgressIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="12" r="8"/><path d="M12 4a8 8 0 0 1 8 8"/><path d="M12 8v4l2.8 1.8"/>`}} />;
}

export function ProjectIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M4 8V6.4A1.6 1.6 0 0 1 5.6 4.8h4l1.8 2h7A1.6 1.6 0 0 1 20 8.4v8.9a1.8 1.8 0 0 1-1.8 1.8H5.8A1.8 1.8 0 0 1 4 17.3z"/><path d="M7.5 11.4h9"/>`}} />;
}

export function ProposalIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="4.5" y="4.5" width="15" height="15" rx="3"/><path d="M8 9h8M8 12h5"/><path d="m13.5 16 1.5 1.5 3-3"/>`}} />;
}

export function ReviewRequiredIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="4.5" y="5" width="15" height="13" rx="3"/><path d="M8 9.5h8M8 12.5h5"/><path d="M12 18v2M9.8 20h4.4"/>`}} />;
}

export function RoleIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="8" cy="7.2" r="2.4"/><path d="M3.9 18.7c.6-3.3 2.1-4.9 4.3-4.9 2.1 0 3.4 1.1 4.1 3.2"/><path d="M16.5 10.4 20 12v2.7c0 2.2-1.2 4-3.5 5.2-2.3-1.2-3.5-3-3.5-5.2V12z"/>`}} />;
}

export function SecurityIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M12 3.2 5.2 6v4.9c0 4.3 2.5 7.3 6.8 9.8 4.3-2.5 6.8-5.5 6.8-9.8V6z"/><path d="M9.2 12.1h5.6M12 9.3v5.6"/>`}} />;
}

export function SharedCowokIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="8.2" cy="7.3" r="2.4"/><circle cx="16.2" cy="8.2" r="2.1"/><path d="M3.9 19.7c.6-3.4 2.1-5.1 4.6-5.1 2.1 0 3.6 1.1 4.4 3.3"/><path d="M12.3 19.5c.5-2.7 1.8-4.1 4.1-4.1 2.2 0 3.5 1.4 4 4.1"/><path d="M10.1 10.1c1.4 1.1 2.8 1.3 4.2.5"/>`}} />;
}

export function SkillIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M13 2.9 6.6 13h4.7l-.8 8.1 7-10.9H13z"/>`}} />;
}

export function StatusIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="12" r="8"/><path d="M8.5 12h7M12 8.5v7"/>`}} />;
}

export function SupervisorIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="8.3" cy="7.2" r="2.3"/><path d="M4.2 18.8c.6-3.2 2.1-4.8 4.3-4.8 1.5 0 2.7.6 3.5 1.8"/><path d="M16.5 9.5 20 11v2.8c0 2.4-1.2 4.2-3.5 5.5-2.3-1.3-3.5-3.1-3.5-5.5V11z"/><path d="m14.8 14 1.1 1.1 2.3-2.3"/>`}} />;
}

export function TaskIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="4.5" y="4.5" width="15" height="15" rx="3.4"/><path d="m8.2 12.1 2.3 2.3 5.3-5.3"/>`}} />;
}

export function TeamIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="8.3" cy="8" r="2.3"/><circle cx="15.6" cy="8.6" r="2"/><path d="M3.7 19.6c.6-3.4 2.3-5.1 4.9-5.1 2.7 0 4.3 1.7 5 5.1"/><path d="M13.7 15.1c2.8-.2 4.9 1.3 5.7 4.5"/>`}} />;
}

export function ToolIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M15.2 4.4a4 4 0 0 0-4.9 5.2l-5.8 5.8a2.7 2.7 0 0 0 3.8 3.8l5.8-5.8a4 4 0 0 0 5.2-4.9l-3.1 3.1-3.9-3.9z"/>`}} />;
}

export function WikiIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M4.5 5.5c2.6-.8 5-.3 7.5 1.4v12c-2.5-1.7-4.9-2.2-7.5-1.4z"/><path d="M19.5 5.5c-2.6-.8-5-.3-7.5 1.4v12c2.5-1.7 4.9-2.2 7.5-1.4z"/>`}} />;
}

export function WorkflowIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3.7" y="5.2" width="5.4" height="4.2" rx="1.7"/><rect x="14.9" y="5.2" width="5.4" height="4.2" rx="1.7"/><rect x="9.3" y="14.8" width="5.4" height="4.2" rx="1.7"/><path d="M6.4 9.4v1.6c0 1.1.9 2 2 2h7.2c1.1 0 2-.9 2-2V9.4M12 13v1.8"/>`}} />;
}

export function WorkspaceIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3.8" y="4.5" width="16.4" height="15" rx="3"/><path d="M3.8 9.2h16.4M8.4 9.2v10.3"/>`}} />;
}

export function AddMemberIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M18 8v6"/><path d="M15 11h6"/>`}} />;
}

export function AgentAccountantIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="4" y="3" width="16" height="18" rx="2" /><path d="M9 7h6M9 11h2M14 11h2M9 15h2M13 15h3M9 18h6" />`}} />;
}

export function AgentDevopsIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M12 12a5 5 0 1 1-3.5-3.5C10.5 8.5 12 12 12 12s1.5 3.5 3.5 3.5a5 5 0 1 0-3.5-3.5z" /><path d="M12 6V3M12 21v-3" />`}} />;
}

export function AgentLegalIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M14 3v5h5" /><path d="M5 21V3h10l5 5v13a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2z" /><path d="M9 11h6M9 15h4M9 19h2" />`}} />;
}

export function AgentMktBrandIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /><path d="m12 8-1.5 3H7.5l2.5 2-1 3.5 3-2.5 3 2.5-1-3.5 2.5-2H13.5z" />`}} />;
}

export function AgentMktContentIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="3" width="18" height="18" rx="2" /><path d="M9 17V7l7 5z" /><path d="M9 7h8" />`}} />;
}

export function AgentMktCopywriterIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" /><path d="M8 12h8M8 8h5" />`}} />;
}

export function AgentMktDesignerIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M12 22C17.5228 22 22 17.5228 22 12C22 6.47715 17.5228 2 12 2C6.47715 2 2 6.47715 2 12C2 17.5228 6.47715 22 12 22Z" /><circle cx="7.5" cy="10.5" r="1.5" /><circle cx="11.5" cy="7.5" r="1.5" /><circle cx="16.5" cy="9.5" r="1.5" /><circle cx="15.5" cy="14.5" r="1.5" />`}} />;
}

export function AgentMktEcomIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M6 2L3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4zM3 6h18M16 10a4 4 0 0 1-8 0" /><circle cx="12" cy="15" r="1" />`}} />;
}

export function AgentMktKolIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M11 5L6 9H2v6h4l5 4V5zM15.54 8.46a5 5 0 0 1 0 7.07M19.07 4.93a10 10 0 0 1 0 14.14" />`}} />;
}

export function AgentMktOpsIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="5" r="3" /><circle cx="5" cy="19" r="3" /><circle cx="19" cy="19" r="3" /><path d="M12 8v8M5 16l5-8M19 16l-5-8" />`}} />;
}

export function AgentMktTradeIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /><rect x="9" y="14" width="6" height="8" /><path d="M9 9h6M12 6v6" />`}} />;
}

export function AgentQaIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="6" y="7" width="12" height="11" rx="4" /><path d="M8 7V5c0-1.5 1.5-2 4-2s4 .5 4 2v2M6 11H3M18 11h3M6 15l-3 1M18 15l3 1M12 7v11M9 18v2M15 18v2" />`}} />;
}

export function AgentRecruiterIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="12" r="10" /><circle cx="12" cy="10" r="3" /><path d="M6 18c1-3 3-4 6-4s5 1 6 4" /><path d="M12 2v2M12 20v2M2 12h2M20 12h2" />`}} />;
}

export function AgentSalesIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M3 3v18h18" /><path d="M18.5 7.5L12 14l-4-4-5 5" /><path d="M14 7.5h4.5V12" />`}} />;
}

export function AgentSalesAdminIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="8" y="2" width="8" height="4" rx="1" /><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" /><path d="m9 14 2 2 4-4" />`}} />;
}

export function AgentSalesOpsIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M3 3h18l-7 9v6l-4 3v-9z" /><circle cx="18" cy="18" r="3" /><path d="M18 15v1M18 20v1M15 18h1M20 18h1" />`}} />;
}

export function AgentSalesReporterIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M14 3v5h5" /><path d="M5 21V3h10l5 5v13a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2z" /><path d="M9 17v-3M12 17v-5M15 17V9" />`}} />;
}

export function AiCardIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/><path d="M7 15h2"/><circle cx="17" cy="15" r="1"/>`}} />;
}

export function AnalyticsIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M4 18h16"/><path d="M6 16l4-5 4 2 5-7"/><path d="M6 16V8M14 13V7M19 6v10"/>`}} />;
}

export function AssetsIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M6 3h12L22 9L12 21L2 9Z" /><path d="M2 9h20" /><path d="M6 3L8 9L12 21" /><path d="M18 3L16 9L12 21" />`}} />;
}

export function AutoFixIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>
  <path d="M19 15l1.5 3 3 1.5-3 1.5L19 24l-1.5-3-3-1.5 3-1.5L19 15z" stroke="currentColor"/>`}} />;
}

export function BellIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M18 16H6c1.5-1.5 2-3.5 2-6a4 4 0 0 1 8 0c0 2.5.5 4.5 2 6z"/><path d="M10 19c.5 1 1 1.5 2 1.5s1.5-.5 2-1.5"/>`}} />;
}

export function BoxIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M12 2L2 7l10 5 10-5Z" /><path d="M2 7v10l10 5 10-5V7" /><path d="M12 12v10" />`}} />;
}

export function BulkSelectIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<line x1="8" y1="6" x2="21" y2="6"></line><line x1="8" y1="12" x2="21" y2="12"></line><line x1="8" y1="18" x2="21" y2="18"></line><line x1="3" y1="6" x2="3.01" y2="6"></line><line x1="3" y1="12" x2="3.01" y2="12"></line><line x1="3" y1="18" x2="3.01" y2="18"></line>`}} />;
}

export function CalendarIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="5" y="5" width="14" height="15" rx="2"/><path d="M8 3v4M16 3v4M5 9h14"/>`}} />;
}

export function ChatIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M4 5h16v10H9l-5 5z"/><path d="M8 9h8M8 12h5"/>`}} />;
}

export function CheckCircleIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M21.801 10A10 10 0 1 1 17 3.335" /><path d="m9 11 3 3L22 4" />`}} />;
}

export function ChevronDownIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="m6 9 6 6 6-6" />`}} />;
}

export function ChevronRightIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="m9 18 6-6-6-6" />`}} />;
}

export function CommissionsIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M20 6h-2V5a3 3 0 0 0-3-3H9a3 3 0 0 0-3 3v1H4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2zM9 5h6v1H9V5zm7 7h4v4h-4v-4z" />`}} />;
}

export function CopyTextIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>`}} />;
}

export function CornerUpLeftIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<polyline points="9 14 4 9 9 4"></polyline><path d="M20 20v-7a4 4 0 0 0-4-4H4"></path>`}} />;
}

export function DashboardIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M7 9h4M7 13h10"/>`}} />;
}

export function DataDeskIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<ellipse cx="12" cy="5" rx="8" ry="2.5"/>
  <path d="M4 5v6c0 1.38 3.58 2.5 8 2.5 1.2 0 2.34-.08 3.36-.24"/>
  <path d="M4 11v6c0 1.38 3.58 2.5 8 2.5 1.1 0 2.15-.07 3.1-.2"/>
  <circle cx="17.5" cy="15.5" r="3.5"/>
  <path d="M20 18l2 2"/>`}} />;
}

export function DataGridIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="3" width="18" height="18" rx="3"/>
  <path d="M3 8.5h18"/>
  <path d="M9 8.5v12.5"/>
  <path d="M6 5.5h2"/>
  <path d="M7 4.5v2"/>
  <path d="M14 13h4"/>
  <path d="M14 17h2.5"/>`}} />;
}

export function DatabaseIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>`}} />;
}

export function DeployIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M 12 3 C 13.5 6 14.5 10 14.5 16 H 9.5 C 9.5 10 10.5 6 12 3 Z" /><circle cx="12" cy="9.5" r="1.5" /><path d="M 9.5 12 L 6 15.5 V 18 H 9.5" stroke="currentColor" /><path d="M 14.5 12 L 18 15.5 V 18 H 14.5" stroke="currentColor" /><path d="M 10.5 16 C 10.5 18.5 12 20.5 12 20.5 C 12 20.5 13.5 18.5 13.5 16" stroke="currentColor" /><path d="M 12 16 V 19" stroke="currentColor" />`}} />;
}

export function DownloadIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M12 15V3" /><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><path d="m7 10 5 5 5-5" />`}} />;
}

export function DraftIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 1 1 3 3L12 15l-4 1 1-4Z" />`}} />;
}

export function EditIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>`}} />;
}

export function EyeIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"></path><circle cx="12" cy="12" r="3"></circle>`}} />;
}

export function EyeOffIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49" /><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242" /><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143" /><path d="m2 2 20 20" />`}} />;
}

export function FileIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v5h5"/>`}} />;
}

export function FileTextIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z" /><path d="M14 2v5a1 1 0 0 0 1 1h5" /><path d="M10 9H8" /><path d="M16 13H8" /><path d="M16 17H8" />`}} />;
}

export function FolderIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />`}} />;
}

export function ForwardIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<polyline points="15 17 20 12 15 7"></polyline><path d="M4 18v-2a4 4 0 0 1 4-4h12"></path>`}} />;
}

export function GithubIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M 5 15.5 C 5 18 8 20.5 12 20.5 C 16 20.5 19 18 19 15.5 C 19 12 18 9.5 18 8 L 15 9.5 L 12 8 L 9 9.5 L 6 8 C 6 9.5 5 12 5 15.5 Z" /><path d="M 8.5 15.5 C 10 14 12 12.5 15.5 11" stroke="currentColor" /><path d="M 8.5 11 C 10 12.5 12 14 15.5 15.5" stroke="currentColor" /><path d="M 10 14 L 10 12.5" stroke="currentColor" /><path d="M 12 13.5 L 12 13" stroke="currentColor" /><path d="M 14 12.5 L 14 14" stroke="currentColor" />`}} />;
}

export function GlobeIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="4" width="18" height="16" rx="2" /><path d="M3 9h18" /><path d="M6 6.5h.01M9 6.5h.01M12 6.5h.01" />`}} />;
}

export function HardDriveIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M22 12H2"/><path d="M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><line x1="6" y1="16" x2="6.01" y2="16"/><line x1="10" y1="16" x2="10.01" y2="16"/>`}} />;
}

export function HashIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<line x1="4" y1="9" x2="20" y2="9" /><line x1="4" y1="15" x2="20" y2="15" /><line x1="10" y1="3" x2="8" y2="21" /><line x1="16" y1="3" x2="14" y2="21" />`}} />;
}

export function HelpCircleIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/>`}} />;
}

export function ImageIconIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M6 3h12L22 9L12 21L2 9Z" /><path d="M2 9h20" /><path d="M6 3L8 9L12 21" /><path d="M18 3L16 9L12 21" />`}} />;
}

export function JumpMessageIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><polyline points="10 8 13 11 10 14"/><line x1="7" y1="11" x2="13" y2="11"/>`}} />;
}

export function KanbanIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="4" width="5" height="16" rx="1"/><rect x="10" y="4" width="5" height="16" rx="1"/><rect x="17" y="4" width="4" height="16" rx="1"/><path d="M5 8h1M12 8h1M19 8h.5"/>`}} />;
}

export function KeyIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4" /><path d="m21 2-9.6 9.6" /><circle cx="7.5" cy="15.5" r="5.5" />`}} />;
}

export function LarkIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M4 12c0-4.4 3.6-8 8-8s8 3.6 8 8-3.6 8-8 8H4v-8z" /><path d="M12 8l3 4h-6z" />`}} />;
}

export function LayersIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M12 2L2 7l10 5 10-5-10-5Z" /><path d="M2 17l10 5 10-5" /><path d="M2 12l10 5 10-5" />`}} />;
}

export function LightbulbIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-1 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5"/><path d="M9 18h6"/><path d="M10 22h4"/>`}} />;
}

export function LinkIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path>`}} />;
}

export function LlmConfigIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M5 5h14v14H5z"/><path d="M8 9h8M8 13h4M15 13h1M8 17h8"/><circle cx="16" cy="13" r="1"/>`}} />;
}

export function LockIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect width="18" height="11" x="3" y="11" rx="2" ry="2" /><path d="M7 11V7a5 5 0 0 1 10 0v4" />`}} />;
}

export function LogOutIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" /><path d="m16 17 5-5-5-5" /><line x1="21" y1="12" x2="9" y2="12" />`}} />;
}

export function MapIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M8 6L21 3 21 18 8 21 3 18 3 3 8 6z"/><path d="M8 6v15"/><path d="M15 3v15"/>`}} />;
}

export function MemberIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="7" r="4"/><path d="M5 21c1.2-5 3.7-8 7-8s5.8 3 7 8"/>`}} />;
}

export function MessageSquareIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />`}} />;
}

export function MonitorIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="2" y="3" width="20" height="14" rx="2" ry="2" /><line x1="8" y1="21" x2="16" y2="21" /><line x1="12" y1="17" x2="12" y2="21" />`}} />;
}

export function MoonIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401" />`}} />;
}

export function MoreHorizontalIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="12" r="1"></circle><circle cx="19" cy="12" r="1"></circle><circle cx="5" cy="12" r="1"></circle>`}} />;
}

export function NotificationIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M18 16H6c1.5-1.5 2-3.5 2-6a4 4 0 0 1 8 0c0 2.5.5 4.5 2 6z"/><path d="M10 19c.5 1 1 1.5 2 1.5s1.5-.5 2-1.5"/>`}} />;
}

export function OnboardingIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M12 3a9 9 0 1 0 9 9"/><path d="M12 7v5l4 2"/><path d="M17 3h4v4"/>`}} />;
}

export function PanelLeftCloseIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="3" width="18" height="18" rx="2" ry="2" /><line x1="9" y1="3" x2="9" y2="21" /><path d="m14 9-3 3 3 3" />`}} />;
}

export function PanelRightIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="3" width="18" height="18" rx="2" ry="2" /><line x1="15" y1="3" x2="15" y2="21" />`}} />;
}

export function PinIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<line x1="12" y1="17" x2="12" y2="22"></line><path d="M5 17h14v-1.5c0-1.5-1.5-3-1.5-3v-6a4.5 4.5 0 0 0-9 0v6c0 0-1.5 1.5-1.5 3V17z"></path><line x1="9" y1="5" x2="15" y2="5"></line>`}} />;
}

export function PinOffIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<line x1="2" y1="2" x2="22" y2="22"></line><line x1="12" y1="17" x2="12" y2="22"></line><path d="M9 9v3H5v1.5c0 1.5 1.5 3 1.5 3h6.5"></path><path d="M15 9.34V5h4h-3.34"></path><line x1="9" y1="5" x2="15" y2="5"></line>`}} />;
}

export function PlayIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z" />`}} />;
}

export function PluginIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M8 3v4M16 3v4M8 17v4M16 17v4"/><rect x="6" y="7" width="12" height="10" rx="2"/><path d="M3 10h3M18 10h3M3 14h3M18 14h3"/>`}} />;
}

export function PlusIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />`}} />;
}

export function ReconciliationIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<ellipse cx="12" cy="5" rx="9" ry="3"/>
  <path d="M3 5v6c0 1.66 4.03 3 9 3s9-1.34 9-3V5"/>
  <path d="M3 11v6c0 1.66 4.03 3 9 3 1.15 0 2.25-.07 3.28-.2"/>
  <polyline points="14 17 16 14 18 20 20 16 22 17"/>`}} />;
}

export function RefreshIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>`}} />;
}

export function ReplyIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<polyline points="9 17 4 12 9 7"></polyline><path d="M20 18v-2a4 4 0 0 0-4-4H4"></path>`}} />;
}

export function ReportIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M6 20V4h12v16z"/><path d="M9 15v-4M12 15V8M15 15v-6"/>`}} />;
}

export function SalesDeckIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M4 5h16v12H4z"/><path d="M8 21h8M12 17v4M8 12l3-3 3 2 3-5"/>`}} />;
}

export function SchemaTopologyIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="2" y="3" width="8" height="8" rx="1.8"/>
  <rect x="14" y="13" width="8" height="8" rx="1.8"/>
  <path d="M2 6h8"/>
  <path d="M14 16h8"/>
  <path d="M10 7h2.5a2 2 0 0 1 2 2v4"/>
  <circle cx="10" cy="7" r="1" fill="none"/>
  <path d="M14.5 13l-1.5 2-1.5-2"/>`}} />;
}

export function SearchIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="m21 21-4.34-4.34" /><circle cx="11" cy="11" r="8" />`}} />;
}

export function SendIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z" /><path d="m21.854 2.147-10.94 10.939" />`}} />;
}

export function ServerIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect width="20" height="8" x="2" y="2" rx="2" ry="2" /><rect width="20" height="8" x="2" y="14" rx="2" ry="2" /><line x1="6" x2="6.01" y1="6" y2="6" /><line x1="6" x2="6.01" y1="18" y2="18" />`}} />;
}

export function SettingsIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1"/>`}} />;
}

export function ShieldIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" />`}} />;
}

export function ShieldAlertIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" /><path d="M12 8v4" /><path d="M12 16h.01" />`}} />;
}

export function ShieldCheckIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" /><path d="m9 12 2 2 4-4" />`}} />;
}

export function SlackIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="8" width="6" height="6" rx="1" /><path d="M12 8a2 2 0 1 0-2-2v2h2zm0 0v6M12 14a2 2 0 1 0 2 2v-2h-2zm0 0H6M14.5 9.5a2 2 0 1 1 2 2h-2v-2z" />`}} />;
}

export function SmileIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="12" r="10"></circle><path d="M8 14s1.5 2 4 2 4-2 4-2"></path><line x1="9" y1="9" x2="9.01" y2="9"></line><line x1="15" y1="9" x2="15.01" y2="9"></line>`}} />;
}

export function SqlExplainIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="3" width="18" height="12" rx="2.5"/>
  <path d="M7 8l2.5 2L7 12"/>
  <path d="M12 12h5"/>
  <path d="M6 19h12"/>
  <path d="M12 15v4"/>
  <circle cx="18" cy="19" r="1.5"/>`}} />;
}

export function SubscriptionsIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M3 8h12a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V10a2 2 0 0 1 2-2zm4-4h12a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2h-1M3 13h10M7 17h4" />`}} />;
}

export function SunIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="12" r="4" /><path d="M12 2v2" /><path d="M12 20v2" /><path d="m4.93 4.93 1.41 1.41" /><path d="m17.66 17.66 1.41 1.41" /><path d="M2 12h2" /><path d="M20 12h2" /><path d="m6.34 17.66-1.41 1.41" /><path d="m19.07 4.93-1.41 1.41" />`}} />;
}

export function SwitchIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="m16 3 4 4-4 4"/><path d="M20 7H4"/><path d="m8 21-4-4 4-4"/><path d="M4 17h16"/>`}} />;
}

export function SwitchWorkspaceIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><path d="M3.27 6.96L12 12l8.73-5.04M12 22V12"/>`}} />;
}

export function TableProIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="3" y="3" width="18" height="18" rx="2.5"/><line x1="3" y1="9" x2="21" y2="9"/><line x1="3" y1="15" x2="21" y2="15"/><line x1="9" y1="9" x2="9" y2="21"/>`}} />;
}

export function TalentPoolIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="12" r="10" stroke-dasharray="3 3"/><circle cx="12" cy="8" r="2.5"/><path d="M8 15a4 4 0 0 1 8 0"/><circle cx="7" cy="13" r="2"/><path d="M4 18.5a3 3 0 0 1 6 0"/><circle cx="17" cy="13" r="2"/><path d="M14 18.5a3 3 0 0 1 6 0"/>`}} />;
}

export function TelegramIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="m22 2-21 8 8 3 10-10-7 11 1 5 3-4 6 5z" />`}} />;
}

export function TerminalIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="m4 17 6-6-6-6" /><line x1="12" y1="19" x2="20" y2="19" />`}} />;
}

export function TicketDiscrepancyIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M2 9a3 3 0 0 1 0 6v4a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-4a3 3 0 0 1 0-6V5a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v4z"/>
  <path d="M12 8v5"/>
  <circle cx="12" cy="16" r="1" fill="none"/>`}} />;
}

export function TokenChipIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<rect x="4" y="4" width="16" height="16" rx="2" ry="2"/><rect x="9" y="9" width="6" height="6"/><line x1="9" y1="1" x2="9" y2="4"/><line x1="15" y1="1" x2="15" y2="4"/><line x1="9" y1="20" x2="9" y2="23"/><line x1="15" y1="20" x2="15" y2="23"/><line x1="20" y1="9" x2="23" y2="9"/><line x1="20" y1="14" x2="23" y2="14"/><line x1="1" y1="9" x2="4" y2="9"/><line x1="1" y1="14" x2="4" y2="14"/>`}} />;
}

export function TopicIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M4 6h16M4 12h10M4 18h14M18 12l3 3-3 3" />`}} />;
}

export function Trash2Icon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M10 11v6" /><path d="M14 11v6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" /><path d="M3 6h18" /><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />`}} />;
}

export function TrendingUpIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<polyline points="23 6 13.5 15.5 8.5 10.5 1 18"/><polyline points="17 6 23 6 23 12"/>`}} />;
}

export function UploadIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M 4 14 V 18 C 4 19.1 4.9 20 6 20 H 18 C 19.1 20 20 19.1 20 18 V 14" /><path d="M 12 15 V 4 M 8 8 L 12 4 L 16 8" stroke="currentColor" />`}} />;
}

export function UserIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<circle cx="12" cy="8" r="4"/><path d="M5 20c0-4 4-6 7-6s7 2 7 6"/>`}} />;
}

export function UsersIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" /><circle cx="9" cy="7" r="4" /><path d="M22 21v-2a4 4 0 0 0-3-3.87" /><path d="M16 3.13a4 4 0 0 1 0 7.75" />`}} />;
}

export function VouchersIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M21 9V6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v3a2 2 0 0 1 0 4v3a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-3a2 2 0 0 1 0-4zM9 6v12M15 6v12" />`}} />;
}

export function WhatsappIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z" /><path d="M9 10a.5.5 0 0 0 1 0V9a.5.5 0 0 0-1 0v1zm6 4a.5.5 0 0 0 1 0v-1a.5.5 0 0 0-1 0v1z" />`}} />;
}

export function WikiSaveIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"></path><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"></path>`}} />;
}

export function XIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M18 6 6 18" /><path d="m6 6 12 12" />`}} />;
}

export function ZipIcon({size=24,strokeWidth=1.8,...props}:CowokIconProps){
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props} dangerouslySetInnerHTML={{__html:`<path d="M 14 3 H 6 C 4.9 3 4 3.9 4 5 V 19 C 4 20.1 4.9 21 6 21 H 18 C 19.1 21 20 20.1 20 19 V 9 L 14 3 Z" /><path d="M 14 3 V 9 H 20" /><rect x="6.5" y="12" width="11" height="6.2" rx="1.2" stroke="currentColor" /><path d="M 8.5 13.85 H 10.5 L 8.5 16.35 H 10.5" stroke="currentColor" /><path d="M 12 13.85 V 16.35" stroke="currentColor" /><path d="M 13.75 16.35 V 13.85 H 15.25 C 15.7 13.85 15.7 15.1 15.25 15.1 H 13.75" stroke="currentColor" />`}} />;
}

