import { describe, it, expect } from "vitest";
import en from "./locales/en.json";
import es from "./locales/es.json";
import fr from "./locales/fr.json";
import itDict from "./locales/it.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import pt from "./locales/pt.json";
import vi from "./locales/vi.json";
import zh from "./locales/zh.json";

describe("i18n 9-locale parity and completeness", () => {
  const locales = { en, es, fr, it: itDict, ja, ko, pt, vi, zh };
  const nonEnglishLocales = ["es", "fr", "it", "ja", "ko", "pt", "vi", "zh"] as const;

  it("all 9 locales have all top-level sections from en.json", () => {
    const enSections = Object.keys(en);
    for (const [code, dict] of Object.entries(locales)) {
      expect(Object.keys(dict)).toEqual(enSections);
    }
  });

  it("all 9 locales have identical keys in each section", () => {
    for (const section of Object.keys(en) as (keyof typeof en)[]) {
      const enKeys = Object.keys(en[section]).sort();
      for (const [code, dict] of Object.entries(locales)) {
        const locKeys = Object.keys((dict as any)[section]).sort();
        expect(locKeys, `Section ${section} in locale ${code}`).toEqual(enKeys);
      }
    }
  });

  it("non-English locales do not leave values untranslated", () => {
    const universalTerms = new Set([
      "id", "sdk", "api", "mcp", "slm", "llm", "pbac", "rbac", "cas", "json",
      "yaml", "url", "uri", "cowok", "slack", "github", "gitlab", "jira",
      "notion", "hubspot", "lark", "google", "onedrive", "s3", "gcs", "pdf",
      "csv", "sso", "oauth", "jwt", "tls", "mtls", "sql", "ui", "ux", "ai",
      "diff", "manifest", "semver", "v{{ver}}", "rev {{rev}}", "git"
    ]);

    for (const section of Object.keys(en) as (keyof typeof en)[]) {
      for (const [key, enVal] of Object.entries(en[section])) {
        if (typeof enVal !== "string") continue;
        const trimmedEn = enVal.trim();
        const stripped = trimmedEn.replace(/\{\{[^}]+\}\}/g, "").replace(/[ ():•↑↓◀▶✓●-]/g, "").trim();
        if (!stripped || universalTerms.has(stripped.toLowerCase())) continue;
        if (!/[A-Za-z]{4,}/.test(stripped)) continue;

        for (const locCode of nonEnglishLocales) {
          const locVal = (locales[locCode] as any)[section][key];
          expect(locVal, `Key ${section}.${key} in ${locCode} should be translated and not identical to en`).not.toBe(enVal);
        }
      }
    }
  });
});
