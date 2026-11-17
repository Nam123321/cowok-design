// @story 08.2
// @implements AC-02, AC-07
/**
 * Compile-time immutable bridge SHA-256 pin for production builds.
 * Populated automatically at release build/package time via `scripts/pin-bridge.mjs`.
 * Kept `null` in unbuilt source checkouts so `NODE_ENV=production` fails closed unless pinned.
 */
export const COMPILED_RELEASE_BRIDGE_SHA256: string | null = null;
