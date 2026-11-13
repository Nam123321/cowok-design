// @story 04.1
// @implements AC-06
const SENSITIVE_KEY_PATTERN = /(hmac|secret|token|password|apikey|api_key|authorization|html|srcdoc)/i;

function redactValue(key: string, val: unknown): unknown {
  if (SENSITIVE_KEY_PATTERN.test(key)) {
    return "[REDACTED]";
  }
  if (Array.isArray(val)) {
    return val.map((item) => redactValue("", item));
  }
  if (val && typeof val === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(val as Record<string, unknown>)) {
      out[k] = redactValue(k, v);
    }
    return out;
  }
  return val;
}

function emit(level: "info" | "warn" | "error", msg: string, meta?: Record<string, unknown>): void {
  const safeMeta = meta ? (redactValue("root", meta) as Record<string, unknown>) : {};
  const payload = JSON.stringify({
    ts: new Date().toISOString(),
    level,
    msg,
    ...safeMeta,
  });
  if (typeof process === "undefined") {
    return;
  }
  if ((level === "error" || level === "warn") && process.stderr?.write) {
    process.stderr.write(`${payload}\n`);
  } else if (level === "info" && process.stdout?.write) {
    process.stdout.write(`${payload}\n`);
  }
}

export const logger = {
  info: (msg: string, meta?: Record<string, unknown>) => emit("info", msg, meta),
  warn: (msg: string, meta?: Record<string, unknown>) => emit("warn", msg, meta),
  error: (msg: string, meta?: Record<string, unknown>) => emit("error", msg, meta),
};
