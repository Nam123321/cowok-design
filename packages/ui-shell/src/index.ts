import {
  CANONICAL_SPEC_KEYS,
  PortalSchema,
  type CanonicalSpecKey,
  type Portal,
  type TypographyContract,
} from "@cowok-design/shared-types";

export * from "@cowok-design/shared-types";

export const SHELL_VERSION = "0.1.0";

export interface PortalShellDescriptor {
  portal: Portal;
  label: string;
  requiresDarkChrome: boolean;
}

export const PORTAL_SHELL_DESCRIPTORS: Record<Portal, PortalShellDescriptor> = {
  super_admin: { portal: "super_admin", label: "Super Admin", requiresDarkChrome: true },
  tenant_admin: { portal: "tenant_admin", label: "Tenant Admin", requiresDarkChrome: true },
  sales_app: { portal: "sales_app", label: "Sales App", requiresDarkChrome: false },
  marketing_web: { portal: "marketing_web", label: "Marketing Web", requiresDarkChrome: false },
  developer_portal: { portal: "developer_portal", label: "Developer Portal", requiresDarkChrome: true },
  presentation_studio: { portal: "presentation_studio", label: "Presentation Studio", requiresDarkChrome: true },
};

export const SPEC_KEY_TITLES: Record<CanonicalSpecKey, string> = {
  "01-product-overview.md": "Product Overview",
  "02-database-spec.md": "Database Spec",
  "03-ui-ux-spec.md": "UI/UX Spec",
  "04-architecture.md": "Architecture",
  "05-epics-and-stories.md": "Epics & Stories",
  "06-cross-dependencies.md": "Cross-Dependencies",
  "07-api-guidelines.md": "API Guidelines",
  "08-security-tenancy.md": "Security & Tenancy",
  "09-data-and-copy.md": "Data & Copy",
};

export const DEFAULT_SHELL_TYPOGRAPHY_CONTRACT: TypographyContract = {
  allowed_font_families: ["Inter", "JetBrains Mono"],
  allowed_font_weights: [400, 500, 600, 700],
  allowed_font_sizes: ["12px", "14px", "16px", "20px", "24px"],
  min_wcag_contrast: 4.5,
};

export function resolvePortalShellDescriptor(portal: unknown): PortalShellDescriptor {
  const parsed = PortalSchema.parse(portal);
  return PORTAL_SHELL_DESCRIPTORS[parsed];
}

export function listOrderedSpecKeys(): readonly CanonicalSpecKey[] {
  return CANONICAL_SPEC_KEYS;
}
