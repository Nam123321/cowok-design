#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { BUILDER_TOOL_REGISTRY } from "../engine/builder-tools.mjs";
import { QC_TOOL_REGISTRY } from "../engine/qc-tools.mjs";
import { PathJailError } from "../engine/path-jail.mjs";

const PROTOCOL_VERSION = "2024-11-05";
const SERVER_INFO = { name: "design-craft-mcp", version: "1.0.0" };
export const MAX_LINE_BYTES = 1_048_576;

const ALL_TOOLS = {
  ...BUILDER_TOOL_REGISTRY,
  ...QC_TOOL_REGISTRY,
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
};

export function listToolDefinitions() {
  return Object.keys(ALL_TOOLS).map((name) => ({
    name,
    description: TOOL_DESCRIPTIONS[name] ?? name,
    inputSchema: {
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
