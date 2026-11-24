#!/usr/bin/env node
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { BUILDER_TOOL_REGISTRY } from "../engine/builder-tools.mjs";
import {
  generateFeedbackTicketCode,
  scrubFeedbackFields,
  VALID_FEEDBACK_CATEGORIES,
  VALID_FEEDBACK_CHANNELS,
  validateOptionalFeedbackEmail,
  writeFeedbackOutboxFile,
} from "../engine/feedback-scrubber.mjs";
import { QC_TOOL_REGISTRY } from "../engine/qc-tools.mjs";
import { PathJailError } from "../engine/path-jail.mjs";

const PROTOCOL_VERSION = "2024-11-05";
const SERVER_INFO = { name: "design-craft-mcp", version: "1.0.0" };
export const MAX_LINE_BYTES = 1_048_576;

const FEEDBACK_CATEGORIES = VALID_FEEDBACK_CATEGORIES;
const FEEDBACK_CHANNELS = VALID_FEEDBACK_CHANNELS;

export function cowokSubmitFeedback(args = {}, ctx = {}) {
  const category = String(args.category ?? "bug_report").trim();
  if (!FEEDBACK_CATEGORIES.includes(category)) {
    throw new PathJailError(
      "INVALID_PARAMS",
      `Unsupported feedback category '${category}'. Valid: ${FEEDBACK_CATEGORIES.join(", ")}`
    );
  }
  const envHost = String(ctx.env?.COWOK_PLUGIN_HOST ?? "").trim();
  const channel = String(args.channel ?? (FEEDBACK_CHANNELS.includes(envHost) ? envHost : "claude_code")).trim();
  if (!FEEDBACK_CHANNELS.includes(channel)) {
    throw new PathJailError(
      "INVALID_PARAMS",
      `Unsupported feedback channel '${channel}'. Valid: ${FEEDBACK_CHANNELS.join(", ")}`
    );
  }
  const rawTitle = String(args.title ?? "").trim();
  const rawDesc = String(args.description ?? "").trim();
  if (rawTitle.length < 3 || rawTitle.length > 160) {
    throw new PathJailError("INVALID_PARAMS", "title must be 3..=160 characters");
  }
  if (rawDesc.length < 5 || rawDesc.length > 4000) {
    throw new PathJailError("INVALID_PARAMS", "description must be 5..=4000 characters");
  }

  const emailCheck = validateOptionalFeedbackEmail(args.email);
  if (!emailCheck.ok) {
    throw new PathJailError("INVALID_PARAMS", emailCheck.error);
  }

  const scrubbed = scrubFeedbackFields({
    title: rawTitle,
    description: rawDesc,
    locale: args.locale ?? "en-US",
    recentErrorCodes: Array.isArray(args.recentErrorCodes) ? args.recentErrorCodes : [],
    activeSkills: Array.isArray(args.activeSkills) ? args.activeSkills : [],
  });

  const ticketCode = generateFeedbackTicketCode();
  const airgapped = ["1", "true", "yes"].includes(String(ctx.env?.COWOK_AIRGAPPED ?? "").trim().toLowerCase());
  const record = {
    ticketCode,
    category,
    channel,
    title: scrubbed.title,
    description: scrubbed.description,
    contactEmail: emailCheck.value,
    status: "submitted",
    syncStatus: airgapped ? "local_only" : "queued",
    scrubbedRedactions: scrubbed.redactionCount,
    diagnostics: {
      appVersion: SERVER_INFO.version,
      channel,
      os: os.platform(),
      locale: scrubbed.locale,
      recentErrorCodes: scrubbed.recentErrorCodes,
      activeSkills: scrubbed.activeSkills,
      redactionCount: scrubbed.redactionCount,
    },
    createdAt: Date.now(),
  };

  if (ctx.rootDir && fs.existsSync(ctx.rootDir)) {
    try {
      writeFeedbackOutboxFile(ctx.rootDir, ticketCode, record);
    } catch (err) {
      if (err instanceof PathJailError) {
        throw err;
      }
      if (!err || (err.code !== "EACCES" && err.code !== "EROFS")) {
        throw err;
      }
      // Non-fatal if workspace is read-only (EACCES/EROFS); scrubbed receipt is still returned.
    }
  }

  return { ok: true, ...record };
}

const ALL_TOOLS = {
  ...BUILDER_TOOL_REGISTRY,
  ...QC_TOOL_REGISTRY,
  cowok_submit_feedback: cowokSubmitFeedback,
};

function isPlainObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

const TOOL_DESCRIPTIONS = {
  "de-phase-router": "Routes entry mode (GREENFIELD/ADD_FEATURE/REVAMP/MICRO_POLISH/DECK_FROM_APP) with HITL gates and MICRO_POLISH single-agent fast-path.",
  "de-intake": "Normalizes project brief, outputTrack, custom themeModes, and jailed files into cognitive-state.json.",
  "de-scan-sketch": "Scans wireframe HTML for semantic landmarks, data-ff-id anchors, and interactive state coverage.",
  "de-tokens-oklch": "Generates perceptual OKLCH semantic tokens strictly for user-requested themeModes.",
  "de-brand-preview": "Renders an interactive HTML brand showcase strictly for requested themeModes.",
  "de-extract-contract": "Builds Design-Only Keep/Change/Do-Not-Copy reference contracts with offline archetype fallback.",
  "de-patch-dom": "Surgically patches HTML elements by data-ff-id inside the Zero-Trust Path Jail.",
  "qc-a11y-contrast": "Audits OKLCH and hex color pairs against WCAG 2.1 AA/AAA contrast ratios.",
  "qc-anti-slop-linter": "Detects internal jargon (BYOS, CRDT, SSOT), placeholders, Tailwind dark: overrides, and raw hex leaks.",
  "qc-layout-overflow": "Detects horizontal/vertical viewport overflow and element overlap across Desktop, Mobile, and 16:9 Slides.",
  "qc-chart-and-data": "Validates chart titles, units, finite data points, accessible labels, and pie/donut slice caps.",
  "qc-spec-parity": "Measures coverage parity between declared spec requirements and actual screens/states/themeModes.",
  "qc-popular-coverage": "Evaluates reuse of canonical popular FE or 16:9 Deck catalog components.",
  cowok_submit_feedback:
    "Submits a Zero-PII scrubbed non-tech bug report, feature request, skill/template request, or question attributed to the host channel (claude_code, codex, antigravity, cursor, cowok_ai, cli, desktop, docker).",
};

const TOOL_INPUT_SCHEMAS = {
  cowok_submit_feedback: {
    type: "object",
    required: ["category", "title", "description"],
    properties: {
      category: { type: "string", enum: FEEDBACK_CATEGORIES },
      title: { type: "string", minLength: 3, maxLength: 160 },
      description: { type: "string", minLength: 5, maxLength: 4000 },
      email: { type: "string" },
      channel: { type: "string", enum: FEEDBACK_CHANNELS },
      locale: { type: "string" },
      recentErrorCodes: { type: "array", items: { type: "string" } },
      activeSkills: { type: "array", items: { type: "string" } },
    },
    additionalProperties: false,
  },
};

export function listToolDefinitions() {
  return Object.keys(ALL_TOOLS).map((name) => ({
    name,
    description: TOOL_DESCRIPTIONS[name] ?? name,
    inputSchema: TOOL_INPUT_SCHEMAS[name] ?? {
      type: "object",
      additionalProperties: true,
    },
  }));
}

export function executeMcpTool(toolName, args = {}, ctx = {}) {
  if (typeof toolName !== "string" || !Object.hasOwn(ALL_TOOLS, toolName)) {
    throw new PathJailError("UNKNOWN_TOOL", `Unknown tool '${String(toolName)}'`);
  }
  if (!isPlainObject(args)) {
    throw new PathJailError("INVALID_PARAMS", "Tool arguments must be a plain JSON object");
  }
  const handler = ALL_TOOLS[toolName];
  const rootDir = ctx.rootDir ?? process.env.DESIGN_CRAFT_ROOT ?? process.cwd();
  return handler(args, { rootDir, env: ctx.env ?? process.env });
}

export function handleJsonRpcRequest(request, ctx = {}) {
  if (!isPlainObject(request)) {
    return {
      jsonrpc: "2.0",
      id: null,
      error: { code: -32600, message: "Invalid Request: expected a JSON object" },
    };
  }
  const id = Object.hasOwn(request, "id") ? request.id : null;
  const { method } = request;
  if (typeof method !== "string" || method.length === 0) {
    return {
      jsonrpc: "2.0",
      id,
      error: { code: -32600, message: "Invalid Request: method must be a non-empty string" },
    };
  }
  if (request.params !== undefined && !isPlainObject(request.params)) {
    return {
      jsonrpc: "2.0",
      id,
      error: { code: -32602, message: "Invalid params: expected a non-null JSON object" },
    };
  }
  const params = request.params ?? {};
  if (method === "notifications/initialized") {
    return null;
  }
  if (method === "initialize") {
    return {
      jsonrpc: "2.0",
      id,
      result: {
        protocolVersion: PROTOCOL_VERSION,
        capabilities: { tools: {} },
        serverInfo: SERVER_INFO,
      },
    };
  }
  if (method === "ping") {
    return { jsonrpc: "2.0", id, result: {} };
  }
  if (method === "tools/list") {
    return { jsonrpc: "2.0", id, result: { tools: listToolDefinitions() } };
  }
  if (method === "tools/call") {
    if (typeof params.name !== "string" || params.name.trim().length === 0) {
      return {
        jsonrpc: "2.0",
        id,
        error: { code: -32602, message: "Invalid params: params.name must be a non-empty string" },
      };
    }
    if (params.arguments !== undefined && !isPlainObject(params.arguments)) {
      return {
        jsonrpc: "2.0",
        id,
        error: { code: -32602, message: "Invalid params: params.arguments must be a JSON object" },
      };
    }
    try {
      const result = executeMcpTool(params.name, params.arguments ?? {}, ctx);
      return {
        jsonrpc: "2.0",
        id,
        result: {
          content: [{ type: "text", text: JSON.stringify(result) }],
          isError: false,
        },
      };
    } catch (err) {
      const code = err instanceof PathJailError ? err.code : "TOOL_EXECUTION_ERROR";
      return {
        jsonrpc: "2.0",
        id,
        result: {
          content: [{ type: "text", text: JSON.stringify({ error: code, message: err.message }) }],
          isError: true,
        },
      };
    }
  }
  return {
    jsonrpc: "2.0",
    id,
    error: { code: -32601, message: `Method not found: ${method}` },
  };
}

function dispatchLine(line, ctx, outStream) {
  const trimmed = line.trim();
  if (!trimmed) return;
  let response;
  try {
    const parsed = JSON.parse(trimmed);
    response = handleJsonRpcRequest(parsed, ctx);
  } catch (err) {
    response = {
      jsonrpc: "2.0",
      id: null,
      error: { code: -32700, message: `Parse error: ${err.message}` },
    };
  }
  if (response !== null) {
    outStream.write(`${JSON.stringify(response)}\n`);
  }
}

export function startStdioServer(ctx = {}) {
  const inStream = ctx.stdin ?? process.stdin;
  const outStream = ctx.stdout ?? process.stdout;
  const maxBytes = ctx.maxLineBytes ?? MAX_LINE_BYTES;
  let chunks = [];
  let bufferedBytes = 0;
  let discarding = false;

  const emitOverflow = () => {
    const errResp = {
      jsonrpc: "2.0",
      id: null,
      error: {
        code: -32600,
        message: `Invalid Request: line exceeds maximum size of ${maxBytes} bytes`,
      },
    };
    outStream.write(`${JSON.stringify(errResp)}\n`);
  };

  inStream.on("data", (rawChunk) => {
    const chunk = Buffer.isBuffer(rawChunk) ? rawChunk : Buffer.from(String(rawChunk), "utf8");
    let offset = 0;
    while (offset < chunk.length) {
      const nlIdx = chunk.indexOf(0x0a, offset);
      if (nlIdx !== -1) {
        const segment = chunk.subarray(offset, nlIdx);
        offset = nlIdx + 1;
        if (discarding) {
          discarding = false;
          chunks = [];
          bufferedBytes = 0;
          continue;
        }
        if (bufferedBytes + segment.length > maxBytes) {
          chunks = [];
          bufferedBytes = 0;
          emitOverflow();
          continue;
        }
        chunks.push(segment);
        const line = Buffer.concat(chunks).toString("utf8");
        chunks = [];
        bufferedBytes = 0;
        dispatchLine(line, ctx, outStream);
      } else {
        const segment = chunk.subarray(offset);
        offset = chunk.length;
        if (discarding) continue;
        if (bufferedBytes + segment.length > maxBytes) {
          chunks = [];
          bufferedBytes = 0;
          discarding = true;
          emitOverflow();
          continue;
        }
        chunks.push(segment);
        bufferedBytes += segment.length;
      }
    }
  });

  inStream.on("end", () => {
    if (!discarding && bufferedBytes > 0) {
      const line = Buffer.concat(chunks).toString("utf8");
      dispatchLine(line, ctx, outStream);
    }
    chunks = [];
    bufferedBytes = 0;
    discarding = false;
  });
}

function isDirectInvocation() {
  if (!process.argv[1]) return false;
  try {
    const argvUrl = pathToFileURL(fs.realpathSync(path.resolve(process.argv[1]))).href;
    const selfUrl = pathToFileURL(fs.realpathSync(fileURLToPath(import.meta.url))).href;
    return argvUrl === selfUrl;
  } catch {
    return false;
  }
}

if (isDirectInvocation()) {
  startStdioServer();
}
