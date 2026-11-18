// @story 08.2
// @implements AC-03, AC-05, AC-07
export type { Bucket } from "./repackager-assets";
export { classifyAndValidateSourceAsset, scanSkillMarkdownSafety } from "./repackager-assets";

function normalizeScriptForScanDual(content: string): Array<{ cased: string; lower: string }> {
  const noHidden = content
    .normalize("NFKC")
    .replace(/[\u00AD\u034F\u061C\u180E\u200B-\u200F\u2028-\u202E\u2060-\u2069\uFE00-\uFE0F\uFEFF]|\uDB40[\uDC01-\uDC7F]/gu, "");
  const joined = noHidden.replace(/\r\n/g, "\n").replace(/\r/g, "\n").replace(/\\\n/g, "");
  const multiFolded = joined.replace(/(['"`])[ \t\n]*\+[ \t\n]*(['"`])/g, "");
  return multiFolded.split("\n").map((line) => {
    const collapsed = line.replace(/[ \t]+/g, " ");
    let folded = collapsed
      .replace(/\$\{['"]/g, "")
      .replace(/['"]\}/g, "")
      .replace(/['"],[ \t]*['"]/g, "")
      .replace(/(['"]) \+ (['"])/g, "");
    for (let pass = 0; pass < 8; pass++) {
      const next = folded
        .replace(/([a-zA-Z0-9_])\\([a-zA-Z0-9_])/g, "$1$2")
        .replace(/([a-zA-Z0-9_])(?:""|''|" "|' ')([a-zA-Z0-9_])/g, "$1$2");
      if (next === folded) break;
      folded = next;
    }
    const cased = folded.replace(/[ \t]*\|[ \t]*/g, "|");
    return { cased, lower: cased.toLowerCase() };
  });
}

function hasCallOrIndirect(haystack: string, ident: string, caseInsensitive = true): boolean {
  const esc = ident.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const flags = caseInsensitive ? "i" : "";
  return new RegExp(`(?<![a-zA-Z0-9_])${esc}(?![a-zA-Z0-9_])[ \\t]*(?:\\(|\\?\\.\\(|\\)|;|,|:|\\}|\\])`, flags).test(haystack)
    || new RegExp(`(?:,|(?<![=!<>])=|:|\\{|\\[)[ \\t]*${esc}(?![a-zA-Z0-9_])`, flags).test(haystack)
    || new RegExp(`\\[[ \\t]*['"\`]${esc}['"\`][ \\t]*\\]`, flags).test(haystack);
}

/** Scans external scripts for command injection, obfuscated eval, egress, and path traversal (AC-03, AC-05). */
export function scanExternalScriptSecurity(filename: string, content: string): { valid: boolean; error?: string } {
  const shellTells = [
    "curl|bash", "curl|sh", "wget -o-|bash", "wget -o-|sh", "wget|bash", "wget|sh",
    "child_process.exec", "child_process.spawn", "node:child_process", "require(\"child_process\")", "require('child_process')",
    '["exec"]', "['exec']", "[`exec`]", '["execsync"]', "['execsync']", "[`execsync`]",
    "execsync(", "execfilesync(", "spawnsync(", "execfile(", "system(", "popen(",
    "subprocess.call", "subprocess.popen", "subprocess.run", "subprocess.check_output",
    "os.exec", "std::process::command", "deno.command", "bun.spawn", "bun.$", "os.system",
    "rm -rf /", "env -s", "bash -c", "sh -c",
  ];
  const evalTells = [
    "window.eval", "globalthis.eval", 'self["eval"]', "self['eval']", 'global["eval"]', "global['eval']",
    '["eval"]', "['eval']", "[`eval`]", "base64 -d", "base64 --decode", "buffer.from(",
    "fromcharcode(", "[\"fromcharcode\"]", "['fromcharcode']", "[`fromcharcode`]",
    "fromcodepoint(", "[\"fromcodepoint\"]", "['fromcodepoint']", "[`fromcodepoint`]",
    'require("fs")', "require('fs')", 'require("node:fs")', "require('node:fs')",
    'require("http")', "require('http')", 'require("https")', "require('https')",
    'require("net")', "require('net')", 'import("child_process"', "import('child_process'",
    'import("node:', "import('node:", "deno.run",
    "dangerously" + "setinnerhtml", "reflect.construct(function", ".constructor(", "__import__(",
    "node:vm", "vm.runinnewcontext", "vm.runinthiscontext", "vm.compilefunction",
  ];
  const netTells = [
    "nc -e", "/bin/sh", "/bin/bash", "socket.connect", "telemetry-exfil.com", "requests.", "urllib.",
    "http.request", "https.request", "axios.", '["fetch"]', "['fetch']", "[`fetch`]",
    "curl ", "curl|", "wget ", "wget|", "websocket(", "navigator.sendbeacon", "xmlhttprequest",
  ];
  const traversalTells = ["../", "..\\", "~/.ssh", ".ssh/", "/etc/passwd", "/etc/shadow", "/root/", "/proc/self"];
  const base = filename.split(/[/\\]/).pop() ?? filename;
  const ext = base.includes(".") ? base.split(".").pop()?.toLowerCase() ?? "" : "";
  const execExts = new Set(["py", "js", "mjs", "cjs", "ts", "tsx", "jsx"]);

  const lines = normalizeScriptForScanDual(content);
  for (let i = 0; i < lines.length; i++) {
    const { cased, lower } = lines[i];
    const ln = i + 1;
    const cmd = shellTells.find((t) => lower.includes(t));
    if (cmd || hasCallOrIndirect(lower, "spawn") || ((ext === "rb" || ext === "pl") && (lower.includes("%x{") || lower.includes("qx/")))) {
      return { valid: false, error: `Detected dangerous command execution pattern: ${cmd ?? "spawn"} at line ${ln}` };
    }
    const bareShellEval = (ext === "sh" || ext === "bash" || ext === "zsh") && new RegExp("(?<![a-z0-9_])" + "ev" + "al(?![a-z0-9_])").test(lower);
    if (bareShellEval || hasCallOrIndirect(lower, "eval") || hasCallOrIndirect(lower, "atob") || hasCallOrIndirect(cased, "Function", false) || evalTells.some((t) => lower.includes(t)) || (execExts.has(ext) && hasCallOrIndirect(lower, "exec"))) {
      return { valid: false, error: `Detected arbitrary code evaluation (eval / Function) at line ${ln}` };
    }
    const net = netTells.find((t) => lower.includes(t));
    if (net || hasCallOrIndirect(lower, "fetch")) return { valid: false, error: `Detected suspicious network call (${net ?? "fetch"}) at line ${ln}` };
    if (traversalTells.some((t) => lower.includes(t))) return { valid: false, error: `Detected path traversal pattern at line ${ln}` };
  }
  return { valid: true };
}
