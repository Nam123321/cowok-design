import { describe, expect, it } from "vitest";
import {
  DEFAULT_SHELL_TYPOGRAPHY_CONTRACT,
  SPEC_KEY_TITLES,
  listOrderedSpecKeys,
  resolvePortalShellDescriptor,
} from "./index.js";

describe("@cowok-design/ui-shell contracts", () => {
  it("resolves all canonical portals and rejects unknown portals", () => {
    expect(resolvePortalShellDescriptor("developer_portal").label).toBe("Developer Portal");
    expect(() => resolvePortalShellDescriptor("unknown_portal")).toThrow();
  });

  it("maps all 9 canonical spec keys cleanly", () => {
    const keys = listOrderedSpecKeys();
    expect(keys).toHaveLength(9);
    for (const k of keys) {
      expect(SPEC_KEY_TITLES[k].length).toBeGreaterThan(0);
    }
    expect(DEFAULT_SHELL_TYPOGRAPHY_CONTRACT.min_wcag_contrast).toBe(4.5);
  });
});
