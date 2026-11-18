// @story 08.2
// @implements AC-03, AC-05, AC-07
import { scanExternalScriptSecurity } from "./repackager-security";

export type Bucket = "scripts" | "references" | "assets_text" | "assets_image";

const SCRIPT_EXTS = new Set(["py", "js", "mjs", "cjs", "ts", "tsx", "jsx", "sh", "bash", "zsh", "rb", "pl"]);
const REF_EXTS = new Set(["md", "markdown", "txt", "json", "yaml", "yml", "toml"]);
const ASSET_TEXT_EXTS = new Set(["svg", "html"]);
const ASSET_IMG_EXTS = new Set(["png", "jpg", "jpeg", "gif", "webp"]);
const BLOCKED_BIN_EXTS = new Set(["so", "dylib", "dll", "exe", "node", "wasm", "bin", "o", "a", "zip", "tar", "gz", "bz2", "xz", "7z"]);

function hasTagEventHandlerAttr(lower: string): boolean {
  let inTag = false;
  let quote: string | null = null;
  for (let i = 0; i < lower.length; i++) {
    const ch = lower[i];
    if (!inTag) {
      if (ch === "<" && /[a-z]/i.test(lower[i + 1] ?? "")) {
        inTag = true;
        quote = null;
      }
      continue;
    }
    if (quote !== null) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      continue;
    }
    if (ch === ">") {
      inTag = false;
      continue;
    }
    const prev = lower[i - 1] ?? "";
    if (/[\s/"']/.test(prev) && lower.slice(i, i + 2) === "on" && /^on[a-z]+\s*=/.test(lower.slice(i))) {
      return true;
    }
  }
  return false;
}

function decodeHtmlEntitiesLower(input: string): string {
  const named: Record<string, string> = {
    lt: "<",
    gt: ">",
    colon: ":",
    amp: "&",
    quot: "\"",
    apos: "'",
  };
  return input
    .replace(/&#x([0-9a-f]+);?/gi, (_m, hex: string) => {
      const code = Number.parseInt(hex, 16);
      return Number.isFinite(code) && code > 0 && code <= 0x10ffff ? String.fromCodePoint(code) : "";
    })
    .replace(/&#([0-9]+);?/g, (_m, dec: string) => {
      const code = Number.parseInt(dec, 10);
      return Number.isFinite(code) && code > 0 && code <= 0x10ffff ? String.fromCodePoint(code) : "";
    })
    .replace(/&([a-z]+);?/gi, (m, name: string) => named[name.toLowerCase()] ?? m)
    .toLowerCase();
}

export function scanSkillMarkdownSafety(label: string, content: string): void {
  if (/[\u00AD\u034F\u061C\u180E\u200B-\u200F\u2028-\u202E\u2060-\u2069\uFE00-\uFE0F\uFEFF]|\uDB40[\uDC01-\uDC7F]/u.test(content)) {
    throw new Error(`Disallowed zero-width or bidirectional control Unicode character in ${label}`);
  }
  const scanSlice = (text: string): void => {
    const norm = decodeHtmlEntitiesLower(text);
    const compact = norm.replace(/\s+/g, "");
    for (const tag of [
      "<important", "</important", "<system", "</system",
      "<learned_cards_data", "</learned_cards_data", "<skill_catalog_data", "</skill_catalog_data",
      "<learn_context", "</learn_context", "<assistant", "</assistant", "<tool_call", "</tool_call",
      "<!--", "-->", "<img", "<script", "<iframe", "<object", "<embed", "<image",
      "<style", "<link", "<meta", "<animate", "<set", "<use",
      "@import", "dangerously" + "setinnerhtml",
    ]) {
      if (norm.includes(tag) || compact.includes(tag)) {
        throw new Error(`Indirect prompt injection or active tag '${tag}' disallowed in ${label}`);
      }
    }
    if (
      compact.includes("javascript:") ||
      compact.includes("vbscript:") ||
      compact.includes("data:text/html") ||
      compact.includes("data:image/svg+xml") ||
      hasTagEventHandlerAttr(norm)
    ) {
      throw new Error(`Disallowed active script URI or event handler in ${label}`);
    }
    if (/!\[[^\]]*\]\([^)]*\)/.test(norm) || /!\[[^\]]*\][\s]*\[[^\]]*\]/.test(norm)) {
      throw new Error(`Disallowed Markdown image beacon in ${label}`);
    }
  };
  scanSlice(content);
  const shadow = content.normalize("NFKC").replace(/[\u2010-\u2015\u2212\uFE58\uFE63]/g, "-");
  if (shadow !== content) scanSlice(shadow);
}

function hasExecutableMagic(raw: Buffer): string | null {
  if (raw.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46]))) return "ELF executable header";
  if (raw.subarray(0, 4).equals(Buffer.from("\x00asm", "binary"))) return "WebAssembly binary header";
  if (raw.length >= 4) {
    const h = raw.subarray(0, 4).readUInt32BE(0);
    if (h === 0xfeedface || h === 0xfeedfacf || h === 0xcefaedfe || h === 0xcffaedfe || h === 0xcafebabe || h === 0xcafebabf) {
      return "Mach-O executable header";
    }
  }
  if (raw.subarray(0, 2).toString("latin1") === "MZ") return "PE/MZ executable header";
  return null;
}

function hasValidImageMagic(ext: string, raw: Buffer): boolean {
  if (ext === "png") return raw.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));
  if (ext === "jpg" || ext === "jpeg") return raw.length >= 3 && raw[0] === 0xff && raw[1] === 0xd8 && raw[2] === 0xff;
  if (ext === "gif") return raw.subarray(0, 6).toString("ascii") === "GIF87a" || raw.subarray(0, 6).toString("ascii") === "GIF89a";
  if (ext === "webp") return raw.length >= 12 && raw.subarray(0, 4).toString("ascii") === "RIFF" && raw.subarray(8, 12).toString("ascii") === "WEBP";
  return false;
}

export function classifyAndValidateSourceAsset(relPath: string, raw: Buffer): Bucket {
  const execMagic = hasExecutableMagic(raw);
  if (execMagic) throw new Error(`Security scan failed on ${relPath}: Disallowed executable payload (${execMagic})`);
  const base = relPath.split(/[/\\]/).pop() ?? relPath;
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot === base.length - 1) throw new Error(`UnsupportedOrBinaryAsset: missing extension or dotfile on ${relPath}`);
  const ext = base.slice(dot + 1).toLowerCase();
  if (BLOCKED_BIN_EXTS.has(ext)) throw new Error(`UnsupportedOrBinaryAsset: blocked extension '.${ext}' on ${relPath}`);
  if (ASSET_IMG_EXTS.has(ext)) {
    if (!hasValidImageMagic(ext, raw)) throw new Error(`UnsupportedOrBinaryAsset: spoofed raster header for ${relPath}`);
    return "assets_image";
  }
  const bucket: Bucket | null = SCRIPT_EXTS.has(ext) ? "scripts" : REF_EXTS.has(ext) ? "references" : ASSET_TEXT_EXTS.has(ext) ? "assets_text" : null;
  if (!bucket) throw new Error(`UnsupportedOrBinaryAsset: unsupported extension '.${ext}' on ${relPath}`);
  if (raw.includes(0)) throw new Error(`UnsupportedOrBinaryAsset: NUL byte in ${relPath}`);
  const text = new TextDecoder("utf-8", { fatal: true }).decode(raw);
  const sec = scanExternalScriptSecurity(relPath, text);
  if (!sec.valid) throw new Error(`Security scan failed on ${relPath}: ${sec.error}`);
  if (bucket === "references" || bucket === "assets_text") scanSkillMarkdownSafety(relPath, text);
  return bucket;
}
