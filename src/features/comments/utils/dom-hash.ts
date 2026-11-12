const FNV_OFFSET_BASIS = 2166136261;
const FNV_PRIME = 16777619;
const RADIX_HEX = 16;
const HEX_PAD_LENGTH = 8;
const PAD_CHAR = "0";

/**
 * Computes a deterministic 32-bit FNV-1a hex hash of an element's structural snapshot.
 */
export function computeDomHash(
  tagName: string,
  classNames: string,
  innerText: string
): string {
  const normTag = tagName.trim().toLowerCase();
  const normClasses = classNames
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .sort()
    .join(" ");
  const normText = innerText.trim().replace(/\s+/g, " ");

  const combined = `${normTag}|${normClasses}|${normText}`;
  let hash = FNV_OFFSET_BASIS;

  for (let i = 0; i < combined.length; i += 1) {
    hash ^= combined.charCodeAt(i);
    hash = Math.imul(hash, FNV_PRIME);
  }

  return (hash >>> 0).toString(RADIX_HEX).padStart(HEX_PAD_LENGTH, PAD_CHAR);
}
