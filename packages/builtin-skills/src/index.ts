/**
 * @cowok-design/builtin-skills public contract entrypoint.
 * Per D2 Single Core Port (Story 08.1 AC-02), `crates/plugin-host` is the sole
 * authoritative runtime; this package exposes asset schemas, Zod contracts, and typed errors.
 */
export * from "./types";
export * from "./lifecycle-types";
export * from "./lifecycle-errors";
