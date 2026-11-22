// @story 04.1, 04.5, 05.1
// @implements AC-02 (Option CSP-B: DOMParser-Only Zero-Network default-src 'none' + CSPRNG Script Nonce & Real Async SHA-256)
import {
  HostToIframeMessageSchema,
  IframeToHostMessageSchema,
  type HostToIframeMessage,
  type IframeToHostMessage,
} from "../../../lib/validations/flow-workbench";
import {
  buildByteZeroBridgeScript,
  computeArtifactSha256,
  computeArtifactSha256Sync,
  generateBootSecret,
  generateEpochNonce,
  type SandboxSubStateLabels,
} from "./html-sandbox-bridge";
import {
  escapeHtmlAttr,
  prepareHtmlForApplyBridge,
  sanitizeCssContent,
  sanitizeViaDomParser,
  validateUserHtmlPayload,
} from "./html-sandbox-sanitizer";
import { buildPreviewDocumentHtml, type PreviewDocumentLabels } from "./html-preview-document";
import { getStartupDesignCdnCspAllowlist, parseDesignCdnCspAllowlist } from "./html-sandbox-svg-policy";

export {
  HostToIframeMessageSchema,
  IframeToHostMessageSchema,
  buildPreviewDocumentHtml,
  computeArtifactSha256,
  computeArtifactSha256Sync,
  generateBootSecret,
  generateEpochNonce,
  prepareHtmlForApplyBridge,
  sanitizeCssContent,
  validateUserHtmlPayload,
};
export type { HostToIframeMessage, IframeToHostMessage, PreviewDocumentLabels, SandboxSubStateLabels };

export const IFRAME_SANDBOX_PERMISSIONS = "allow-scripts allow-forms" as const;
const DOM_ANCHOR_HASH_LEN = 16;
const SAFE_CSP_NONCE_RE = /^[a-zA-Z0-9_-]{1,128}$/;

export function buildStrictOfflineCspWithAllowlist(scriptNonce: string, allowlistRaw?: string): string {
  if (!SAFE_CSP_NONCE_RE.test(scriptNonce)) {
    throw new Error("Invalid CSP script nonce");
  }
  const origins =
    allowlistRaw !== undefined ? parseDesignCdnCspAllowlist(allowlistRaw) : getStartupDesignCdnCspAllowlist();
  const extra = origins.length > 0 ? ` ${origins.join(" ")}` : "";
  return `default-src 'none'; base-uri 'none'; connect-src 'none'; form-action 'none'; style-src 'nonce-${scriptNonce}'${extra}; img-src data: blob:${extra}; font-src data:${extra}; script-src 'nonce-${scriptNonce}';`;
}

export function buildStrictOfflineCsp(scriptNonce: string): string {
  return buildStrictOfflineCspWithAllowlist(scriptNonce);
}

export const STRICT_OFFLINE_CSP =
  "default-src 'none'; base-uri 'none'; connect-src 'none'; form-action 'none'; style-src 'none'; img-src data: blob:; font-src data:; script-src 'none';";

export interface WrapUserHtmlOptions {
  epoch?: number;
  bootSecret?: string;
  scriptNonce?: string;
  artifactSha256?: string;
  domAnchorHash?: string;
  parentOrigin?: string;
  subStateLabels?: SandboxSubStateLabels;
}

const SANDBOX_PROPAGATED_COLOR_TOKENS = [
  "--human-accent",
  "--agent-accent",
  "--state-success",
  "--state-info",
  "--state-warning",
  "--state-danger",
  "--text-secondary",
] as const;

const SAFE_CSS_COLOR_RE = /^(#[0-9a-fA-F]{3,8}|rgba?\(\s*[\d.\s,%]+\s*\)|hsla?\(\s*[\d.\s,%deg]+\s*\))$/;

function buildHostThemeTokenStyle(scriptNonce: string): string {
  if (typeof document === "undefined" || typeof getComputedStyle !== "function" || !document.documentElement) {
    return "";
  }
  const style = getComputedStyle(document.documentElement);
  const decls: string[] = [];
  for (const token of SANDBOX_PROPAGATED_COLOR_TOKENS) {
    const val = style.getPropertyValue(token).trim();
    if (val && SAFE_CSS_COLOR_RE.test(val) && sanitizeCssContent(val)) {
      decls.push(`${token}:${val}`);
    }
  }
  if (decls.length === 0) return "";
  return `<style nonce="${escapeHtmlAttr(scriptNonce)}">:root{${decls.join(";")}}</style>`;
}

export function wrapUserHtmlSrcDoc(rawHtml: string, opts?: WrapUserHtmlOptions): string {
  const epoch = opts?.epoch ?? generateEpochNonce();
  const bootSecret = opts?.bootSecret ?? generateBootSecret();
  const scriptNonce = opts?.scriptNonce ?? generateBootSecret();
  const artifactSha256 = opts?.artifactSha256 ?? computeArtifactSha256Sync(rawHtml);
  const domAnchorHash = opts?.domAnchorHash ?? artifactSha256.slice(0, DOM_ANCHOR_HASH_LEN);
  const parentOrigin =
    opts?.parentOrigin ??
    (typeof window !== "undefined" && window.location?.origin && window.location.origin !== "null"
      ? window.location.origin
      : undefined);

  const bridgeScript = buildByteZeroBridgeScript({
    epoch,
    bootSecret,
    scriptNonce,
    artifactSha256,
    domAnchorHash,
    parentOrigin,
    subStateLabels: opts?.subStateLabels,
  });
  const cspDirective = buildStrictOfflineCsp(scriptNonce);
  const themeStyle = buildHostThemeTokenStyle(scriptNonce);
  const headPrefix = `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${escapeHtmlAttr(cspDirective)}">${themeStyle}${bridgeScript}`;
  return sanitizeViaDomParser(rawHtml, headPrefix, scriptNonce);
}
