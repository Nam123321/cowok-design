// @story 01.1
// @cover AC-01, AC-02, AC-03, AC-04, AC-05
import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";
import {
  ActiveConstraintSchema,
  AstInjectionRequestSchema,
  AstInjectionResponseSchema,
  BotJobSchema,
  CanonicalSpecKeySchema,
  ConstraintProfileSchema,
  ConstraintSeveritySchema,
  ConstraintViolationSchema,
  DesignIntentV1Schema,
  FlowGraphSchema,
  GateReportSchema,
  HarnessErrorSchema,
  HttpErrorBodySchema,
  IconManifestEntrySchema,
  IconManifestSchema,
  IconValidationReportSchema,
  MAX_SAFE_INTEGER,
  MIN_SAFE_INTEGER,
  PatchKindSchema,
  PresentationDeckSchema,
  ProjectSchema,
  ProjectSpecSchema,
  ProvenanceStatusSchema,
  SafeJsIntegerSchema,
  ScreenStateSchema,
  SkillPluginSchema,
  SpecProvenanceSummarySchema,
  SpecRevisionSchema,
  SurgicalPatchRequestSchema,
  SurgicalPatchResponseSchema,
  TransitionSchema,
  TypographyContractSchema,
} from "./index.js";

describe("SafeJsInteger Invariants (KU-01.1 & AC-04)", () => {
  it("UT-INT-01: accepts MAX_SAFE_INTEGER", () => {
    expect(SafeJsIntegerSchema.parse(MAX_SAFE_INTEGER)).toBe(9_007_199_254_740_991);
  });

  it("UT-INT-02: accepts MIN_SAFE_INTEGER", () => {
    expect(SafeJsIntegerSchema.parse(MIN_SAFE_INTEGER)).toBe(-9_007_199_254_740_991);
  });

  it("UT-INT-03: rejects MAX_SAFE_INTEGER + 1", () => {
    expect(() => SafeJsIntegerSchema.parse(MAX_SAFE_INTEGER + 1)).toThrow();
  });

  it("UT-INT-04: rejects MIN_SAFE_INTEGER - 1", () => {
    expect(() => SafeJsIntegerSchema.parse(MIN_SAFE_INTEGER - 1)).toThrow();
  });

  it("rejects non-integers (floats)", () => {
    expect(() => SafeJsIntegerSchema.parse(123.456)).toThrow();
  });
});

describe("CanonicalSpecKey Strict Whitelist (AC-05 & EC-E01-03)", () => {
  const validKeys = [
    "01-product-overview.md",
    "02-database-spec.md",
    "03-ui-ux-spec.md",
    "04-architecture.md",
    "05-epics-and-stories.md",
    "06-cross-dependencies.md",
    "07-api-guidelines.md",
    "08-security-tenancy.md",
    "09-data-and-copy.md",
  ];

  it("UT-KEY-01..09: accepts all 9 canonical spec keys", () => {
    for (const key of validKeys) {
      expect(CanonicalSpecKeySchema.parse(key)).toBe(key);
    }
  });

  it("UT-KEY-10: rejects arbitrary unknown filenames", () => {
    expect(() => CanonicalSpecKeySchema.parse("custom-spec.md")).toThrow();
  });

  it("UT-KEY-11: rejects path traversal attempts with ../", () => {
    expect(() => CanonicalSpecKeySchema.parse("../01-product-overview.md")).toThrow();
  });

  it("UT-KEY-12: rejects relative subpath", () => {
    expect(() => CanonicalSpecKeySchema.parse("docs/01-product-overview.md")).toThrow();
  });

  it("UT-KEY-13: rejects Windows backslash traversal", () => {
    expect(() => CanonicalSpecKeySchema.parse("..\\01-product-overview.md")).toThrow();
  });

  it("UT-KEY-14: rejects null byte injection", () => {
    expect(() => CanonicalSpecKeySchema.parse("01-product-overview.md\0.txt")).toThrow();
  });

  it("UT-KEY-15: rejects known name without .md suffix", () => {
    expect(() => CanonicalSpecKeySchema.parse("01-product-overview")).toThrow();
  });
});

describe("ProvenanceStatus Wire Enum (KU-01.6)", () => {
  it("UT-PROV-01: accepts exact SCREAMING_SNAKE_CASE wire values", () => {
    expect(ProvenanceStatusSchema.parse("CONFIRMED")).toBe("CONFIRMED");
    expect(ProvenanceStatusSchema.parse("INFERRED")).toBe("INFERRED");
    expect(ProvenanceStatusSchema.parse("MISSING")).toBe("MISSING");
  });

  it("UT-PROV-02: strictly rejects legacy PascalCase values", () => {
    expect(() => ProvenanceStatusSchema.parse("Confirmed")).toThrow();
    expect(() => ProvenanceStatusSchema.parse("Inferred")).toThrow();
    expect(() => ProvenanceStatusSchema.parse("Missing")).toThrow();
  });
});

describe("HarnessError Discriminated Union (P7/P10)", () => {
  it("UT-ERR-01: parses INVALID_SPEC_KEY", () => {
    const raw = { kind: "INVALID_SPEC_KEY", payload: { key: "bad.md" } };
    expect(HarnessErrorSchema.parse(raw)).toEqual(raw);
  });

  it("UT-ERR-02: parses STALE_FILE", () => {
    const raw = {
      kind: "STALE_FILE",
      payload: { path: "spec.md", expected_hash: "a", actual_hash: "b" },
    };
    expect(HarnessErrorSchema.parse(raw)).toEqual(raw);
  });

  it("UT-ERR-03: parses PROJECT_NOT_FOUND", () => {
    const raw = { kind: "PROJECT_NOT_FOUND", payload: { project_id: "p-1" } };
    expect(HarnessErrorSchema.parse(raw)).toEqual(raw);
  });

  it("UT-ERR-04: parses PATH_TRAVERSAL_DENIED", () => {
    const raw = { kind: "PATH_TRAVERSAL_DENIED", payload: { path: "../../etc/passwd" } };
    expect(HarnessErrorSchema.parse(raw)).toEqual(raw);
  });

  it("UT-ERR-05: parses SQLITE_BUSY", () => {
    const raw = { kind: "SQLITE_BUSY", payload: { details: "database is locked" } };
    expect(HarnessErrorSchema.parse(raw)).toEqual(raw);
  });

  it("UT-ERR-06: parses VALIDATION_FAILED", () => {
    const raw = { kind: "VALIDATION_FAILED", payload: { reason: "field missing" } };
    expect(HarnessErrorSchema.parse(raw)).toEqual(raw);
  });

  it("UT-ERR-07: parses lossless UNSAFE_INTEGER i64 string and rejects in-range or malformed values", () => {
    const fromRustJson = JSON.parse('{"kind":"UNSAFE_INTEGER","payload":{"value":"9007199254740993"}}');
    expect(HarnessErrorSchema.parse(fromRustJson)).toEqual(fromRustJson);

    const negativeOverflow = { kind: "UNSAFE_INTEGER", payload: { value: "-9007199254740993" } };
    expect(HarnessErrorSchema.parse(negativeOverflow)).toEqual(negativeOverflow);

    // Numeric JSON value must be rejected to enforce lossless string wire contract (F-04)
    expect(() =>
      HarnessErrorSchema.parse(JSON.parse('{"kind":"UNSAFE_INTEGER","payload":{"value":9007199254740993}}'))
    ).toThrow();

    // Safe integer must not be accepted as UNSAFE_INTEGER
    expect(() =>
      HarnessErrorSchema.parse({ kind: "UNSAFE_INTEGER", payload: { value: "42" } })
    ).toThrow();

    // Non-numeric string must fail
    expect(() =>
      HarnessErrorSchema.parse({ kind: "UNSAFE_INTEGER", payload: { value: "not-an-int" } })
    ).toThrow();

    // Unknown top-level field on HarnessErrorSchema branch must be rejected (F-06 parity)
    expect(() =>
      HarnessErrorSchema.parse({
        kind: "INVALID_SPEC_KEY",
        payload: { key: "bad.md" },
        injected: true,
      })
    ).toThrow();
  });
});

describe("Domain Aggregates & P1 Persistence Models (AC-02 & P1)", () => {
  const sampleUuid = "123e4567-e89b-12d3-a456-426614174000";

  it("UT-VAL-01: validates Project with UUID and safe timestamps", () => {
    const project = {
      id: sampleUuid,
      name: "Cowok-Design Platform",
      slug: "cowok-design",
      created_at: 1730500000,
      updated_at: 1730500000,
    };
    expect(ProjectSchema.parse(project)).toEqual(project);
  });

  it("UT-VAL-02: validates ProjectSpec and unique spec_key binding", () => {
    const spec = {
      id: sampleUuid,
      project_id: sampleUuid,
      spec_key: "01-product-overview.md",
      status: "CONFIRMED",
      updated_at: 1730500000,
    };
    expect(ProjectSpecSchema.parse(spec)).toEqual(spec);
  });

  it("UT-VAL-03: validates SpecRevision with sequence number", () => {
    const rev = {
      id: sampleUuid,
      spec_id: sampleUuid,
      revision_number: 1,
      content_hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      created_at: 1730500000,
    };
    expect(SpecRevisionSchema.parse(rev)).toEqual(rev);
  });

  it("UT-VAL-03b: rejects SpecRevision with revision_number <= 0, > u32::MAX, or non-SHA256 content_hash", () => {
    expect(() =>
      SpecRevisionSchema.parse({
        id: sampleUuid,
        spec_id: sampleUuid,
        revision_number: 0,
        content_hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        created_at: 1730500000,
      })
    ).toThrow();

    expect(() =>
      SpecRevisionSchema.parse({
        id: sampleUuid,
        spec_id: sampleUuid,
        revision_number: 4_294_967_296,
        content_hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        created_at: 1730500000,
      })
    ).toThrow();

    expect(() =>
      SpecRevisionSchema.parse({
        id: sampleUuid,
        spec_id: sampleUuid,
        revision_number: 1,
        content_hash: "not-a-valid-64-char-sha256-hex-digest",
        created_at: 1730500000,
      })
    ).toThrow();
  });

  it("UT-VAL-04: rejects invalid UUID", () => {
    expect(() =>
      ProjectSchema.parse({
        id: "not-a-uuid",
        name: "Test",
        slug: "test",
        created_at: 0,
        updated_at: 0,
      })
    ).toThrow();
  });

  it("UT-VAL-06: rejects unknown object fields (P12 Consumer Safety / strict mode)", () => {
    expect(() =>
      ProjectSchema.parse({
        id: sampleUuid,
        name: "Test",
        slug: "test",
        created_at: 0,
        updated_at: 0,
        unknown_injected_field: "malicious",
      })
    ).toThrow();
  });

  it("validates remaining canonical aggregates", () => {
    expect(
      SpecProvenanceSummarySchema.parse({
        project_id: sampleUuid,
        confirmed_count: 9,
        inferred_count: 0,
        missing_count: 0,
        overall_status: "CONFIRMED",
      })
    ).toBeDefined();

    expect(
      DesignIntentV1Schema.parse({
        id: sampleUuid,
        project_id: sampleUuid,
        visual_style: "Industrial Brutalism",
        color_palette: ["#000000", "#FFFFFF"],
        typography: ["Geist Mono"],
        key_principles: ["Deterministic", "Zero-Trust"],
      })
    ).toBeDefined();

    expect(
      ConstraintProfileSchema.parse({
        id: sampleUuid,
        project_id: sampleUuid,
        max_complexity_score: 7,
        enforce_clean_code: true,
        forbidden_patterns: ["any", "eval"],
        target_framework: "react-19",
      })
    ).toBeDefined();

    expect(
      ScreenStateSchema.parse({
        id: "screen-1",
        name: "Dashboard",
        route_path: "/dashboard",
        components: ["Sidebar", "MetricCard"],
      })
    ).toBeDefined();

    expect(
      TransitionSchema.parse({
        id: "trans-1",
        from_state: "screen-1",
        to_state: "screen-2",
        trigger_action: "CLICK_SETTINGS",
      })
    ).toBeDefined();

    expect(
      FlowGraphSchema.parse({
        id: sampleUuid,
        project_id: sampleUuid,
        states: [],
        transitions: [],
      })
    ).toBeDefined();

    expect(
      PresentationDeckSchema.parse({
        id: sampleUuid,
        project_id: sampleUuid,
        title: "Sprint Review",
        slides_count: 5,
        last_exported_at: 1730500000,
      })
    ).toBeDefined();

    expect(
      SkillPluginSchema.parse({
        id: "plugin-1",
        name: "Figma Sync",
        version: "1.0.0",
        entry_point: "index.js",
        permissions: ["read:specs"],
      })
    ).toBeDefined();

    expect(
      BotJobSchema.parse({
        id: sampleUuid,
        task_type: "CODEGEN",
        payload_hash: "abc",
        status: "PENDING",
        enqueued_at: 1730500000,
      })
    ).toBeDefined();

    expect(
      GateReportSchema.parse({
        gate_name: "stage_3a_plan_approval",
        passed: true,
        checker_id: "validate-plan-proven-safe.py",
        timestamp: 1730500000,
        evidence_hash: "hash123",
      })
    ).toBeDefined();
  });
});

describe("Contract-V1 Fixture Parity (E2E-PARITY-01 & 02)", () => {
  const fixturesDir = path.resolve(
    __dirname,
    "../../../crates/domain/tests/fixtures/contract-v1"
  );

  it("parses valid-project.json fixture", () => {
    const raw = JSON.parse(
      fs.readFileSync(path.join(fixturesDir, "valid-project.json"), "utf-8")
    );
    expect(ProjectSchema.parse(raw.data)).toBeDefined();
  });

  it("parses valid-project-spec.json fixture", () => {
    const raw = JSON.parse(
      fs.readFileSync(path.join(fixturesDir, "valid-project-spec.json"), "utf-8")
    );
    expect(ProjectSpecSchema.parse(raw.data)).toBeDefined();
  });

  it("parses valid-spec-revision.json fixture", () => {
    const raw = JSON.parse(
      fs.readFileSync(path.join(fixturesDir, "valid-spec-revision.json"), "utf-8")
    );
    expect(SpecRevisionSchema.parse(raw.data)).toBeDefined();
  });

  it("strictly rejects legacy-pascal-case-rejected.json fixture", () => {
    const raw = JSON.parse(
      fs.readFileSync(
        path.join(fixturesDir, "legacy-pascal-case-rejected.json"),
        "utf-8"
      )
    );
    expect(() => ProjectSpecSchema.parse(raw.invalid_data)).toThrow();
  });

  it("parses error-invalid-spec-key.json fixture", () => {
    const raw = JSON.parse(
      fs.readFileSync(
        path.join(fixturesDir, "error-invalid-spec-key.json"),
        "utf-8"
      )
    );
    expect(HarnessErrorSchema.parse(raw.data)).toBeDefined();
  });

  it("parses error-stale-file.json fixture", () => {
    const raw = JSON.parse(
      fs.readFileSync(path.join(fixturesDir, "error-stale-file.json"), "utf-8")
    );
    expect(HarnessErrorSchema.parse(raw.data)).toBeDefined();
  });

  it("UT-ERR-EXT: parses extended error codes correctly", () => {
    const boundaryErr = {
      kind: "BOUNDARY_VIOLATION" as const,
      payload: { reason: "Subagent escaped boundary" },
    };
    expect(HarnessErrorSchema.parse(boundaryErr)).toEqual(boundaryErr);

    const budgetErr = {
      kind: "PROMPT_BUDGET_EXCEEDED" as const,
      payload: { tokens: 4000, ceiling: 3000 },
    };
    expect(HarnessErrorSchema.parse(budgetErr)).toEqual(budgetErr);

    const payloadErr = {
      kind: "INVALID_PROMPT_PAYLOAD" as const,
      payload: { details: "Malformed prompt tokens" },
    };
    expect(HarnessErrorSchema.parse(payloadErr)).toEqual(payloadErr);

    const adaptErr = {
      kind: "MODEL_ADAPTATION_FAILED" as const,
      payload: { reason: "Failed to resolve binding" },
    };
    expect(HarnessErrorSchema.parse(adaptErr)).toEqual(adaptErr);
  });
});

describe("Epic-03: Constraint Engine & AST Linter Schemas", () => {
  it("ConstraintSeveritySchema: accepts Info, Warn, Blocker (and uppercase) and rejects others", () => {
    expect(ConstraintSeveritySchema.parse("Info")).toBe("Info");
    expect(ConstraintSeveritySchema.parse("Warn")).toBe("Warn");
    expect(ConstraintSeveritySchema.parse("Blocker")).toBe("Blocker");
    expect(ConstraintSeveritySchema.parse("INFO")).toBe("INFO");
    expect(ConstraintSeveritySchema.parse("WARN")).toBe("WARN");
    expect(ConstraintSeveritySchema.parse("BLOCKER")).toBe("BLOCKER");
    expect(() => ConstraintSeveritySchema.parse("Fatal")).toThrow();
  });

  it("HttpErrorBodySchema: parses valid Axum error envelope and rejects malformed payloads", () => {
    const valid = {
      code: "SQLITE_BUSY",
      kind: "DATABASE_LOCKED",
      message: "Database is currently busy",
      details: { timeout_ms: 250 },
      request_id: "req_123456",
    };
    expect(HttpErrorBodySchema.parse(valid)).toEqual(valid);

    // Optional kind
    const withoutKind = {
      code: "INTERNAL_ERROR",
      message: "An unexpected error occurred",
      details: null,
      request_id: "req_999",
    };
    expect(HttpErrorBodySchema.parse(withoutKind)).toEqual(withoutKind);

    // Missing code or request_id fails
    expect(() => HttpErrorBodySchema.parse({ message: "failed" })).toThrow();
  });

  it("ActiveConstraintSchema: parses valid payload and rejects extra fields", () => {
    const valid = {
      rule_code: "BAN_INLINE_HEX",
      severity: "Blocker",
      config_json: '{"allowed": []}',
    };
    expect(ActiveConstraintSchema.parse(valid)).toEqual(valid);

    expect(() =>
      ActiveConstraintSchema.parse({
        ...valid,
        unknown_key: true,
      })
    ).toThrow();
  });

  it("ConstraintViolationSchema: handles line and column nullable/optional", () => {
    const violation = {
      rule_code: "BAN_TAILWIND",
      severity: "Warn",
      message: "Tailwind utility not allowed",
      line: 42,
      column: 15,
    };
    expect(ConstraintViolationSchema.parse(violation)).toEqual(violation);

    const withoutCoords = {
      rule_code: "BAN_TAILWIND",
      severity: "Warn",
      message: "Tailwind utility not allowed",
    };
    expect(ConstraintViolationSchema.parse(withoutCoords).line).toBeUndefined();
  });

  it("IconManifestSchema & IconManifestEntrySchema: parses entries and manifest", () => {
    const entry = {
      name: "ChevronRight",
      tier: "Primary",
      file: "chevron-right.svg",
      source: "lucide",
      tags: ["arrow", "navigation"],
    };
    expect(IconManifestEntrySchema.parse(entry)).toEqual(entry);

    const manifest = {
      name: "CoreIcons",
      status: "ACTIVE",
      count: 1,
      icons: [entry],
    };
    expect(IconManifestSchema.parse(manifest).count).toBe(1);

    expect(() =>
      IconManifestSchema.parse({
        name: "Invalid",
        count: -5,
        icons: [],
      })
    ).toThrow();
  });

  it("IconValidationReportSchema: validates valid and invalid icon reports", () => {
    const report = {
      requested_icon: "search-icon",
      is_valid: false,
      suggestions: ["search", "zoom-in"],
    };
    expect(IconValidationReportSchema.parse(report).is_valid).toBe(false);
  });

  it("TypographyContractSchema: parses valid typography rules and enforces positive contrast", () => {
    const contract = {
      allowed_font_families: ["Inter", "Roboto"],
      allowed_font_weights: [400, 500, 700],
      allowed_font_sizes: ["12px", "14px", "16px"],
      min_wcag_contrast: 4.5,
    };
    expect(TypographyContractSchema.parse(contract).min_wcag_contrast).toBe(4.5);

    expect(() =>
      TypographyContractSchema.parse({
        ...contract,
        min_wcag_contrast: -1.0,
      })
    ).toThrow();
  });

  it("PatchKindSchema: accepts exact 4 kinds and rejects invalid", () => {
    expect(PatchKindSchema.parse("text_content")).toBe("text_content");
    expect(PatchKindSchema.parse("metric_value")).toBe("metric_value");
    expect(PatchKindSchema.parse("css_variable")).toBe("css_variable");
    expect(PatchKindSchema.parse("attribute_value")).toBe("attribute_value");
    expect(() => PatchKindSchema.parse("inner_html")).toThrow();
  });

  it("SurgicalPatchRequestSchema & SurgicalPatchResponseSchema: parses request/response with target_attribute", () => {
    const req = {
      ff_id: "elem-123",
      patch_kind: "attribute_value" as const,
      new_value: "avatar.png",
      source: '<img data-ff-id="elem-123" src="old.png" />',
      target_attribute: "src",
    };
    expect(SurgicalPatchRequestSchema.parse(req).target_attribute).toBe("src");

    const res = {
      ff_id: "elem-123",
      success: true,
      original_text: "old.png",
      patched_source: '<img data-ff-id="elem-123" src="avatar.png" />',
      execution_time_micros: 350,
    };
    expect(SurgicalPatchResponseSchema.parse(res).execution_time_micros).toBe(350);
  });

  it("AstInjectionRequestSchema & AstInjectionResponseSchema: validates AST tagging contracts", () => {
    const req = {
      source: "<div><span>Hello</span></div>",
      component_name: "MyWidget",
    };
    expect(AstInjectionRequestSchema.parse(req).component_name).toBe("MyWidget");

    const res = {
      total_elements_tagged: 2,
      tagged_source: '<div data-ff-id="ff-1"><span data-ff-id="ff-2">Hello</span></div>',
      generated_ids: ["ff-1", "ff-2"],
    };
    expect(AstInjectionResponseSchema.parse(res).total_elements_tagged).toBe(2);
  });
});
