// @story 08.5
// @implements AC-03 (Repo / User Skill Import UI with SSRF, License/ToS & <=100-tok JIT Header Preview)
import React, { useEffect, useState } from "react";
import {
  DeRuntimeApiError,
  importUserSkillRepo,
  listImportedUserSkills,
} from "../../api/de-runtime";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import {
  MAX_HEADER_TOKENS,
  type PackagedUserSkillReceiptDto,
} from "../../lib/validations/de-runtime";
import "../plugins/plugins.css";

const ICON_SIZE = 14;
const CHARS_PER_TOKEN = 4;
const FRONTMATTER_BASE_TOKENS = 18;
const LICENSE_OPTIONS = ["MIT", "Apache-2.0", "BSD-3-Clause", "ISC", "MPL-2.0", "GPL-3.0"] as const;

export interface RepoImportFormProps {
  projectId: string;
  onSkillImported?: (receipt: PackagedUserSkillReceiptDto) => void;
}

interface RepoFormState {
  slug: string;
  repoUrl: string;
  license: string;
  description: string;
  bodyMarkdown: string;
}

const DEFAULT_FORM: RepoFormState = {
  slug: "oklch-brand-guard",
  repoUrl: "https://github.com/cowok-ai/oklch-brand-guard",
  license: "MIT",
  description: "Enforce OKLCH WCAG AA contrast tokens.",
  bodyMarkdown: "# OKLCH Guard\nBind colors to semantic CSS variables.",
};

function estimateHeaderTokens(slug: string, description: string): number {
  const bytes = new TextEncoder().encode(`${slug} ${description}`).length;
  return Math.min(FRONTMATTER_BASE_TOKENS + Math.ceil(bytes / CHARS_PER_TOKEN), MAX_HEADER_TOKENS);
}

function verdictBadgeClass(verdict: PackagedUserSkillReceiptDto["securityScanVerdict"]): string {
  if (verdict === "PASS") return "plugin-eco-badge plugin-eco-badge-active";
  if (verdict === "QUARANTINED") return "plugin-eco-badge plugin-eco-badge-warning";
  return "plugin-eco-badge plugin-eco-badge-danger";
}

const ImportedSkillsList: React.FC<{ items: PackagedUserSkillReceiptDto[] }> = ({ items }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  if (items.length === 0) {
    return (
      <div className="plugin-eco-state-box" data-testid="user-skills-empty">
        {t("deRuntime.importEmpty", "No user skills packaged in V20 user_skill_registry yet.")}
      </div>
    );
  }
  return (
    <div className="plugin-eco-chip-grid" data-testid="user-skills-list">
      {items.map((item) => (
        <div key={item.id} className="plugin-eco-chip-card" data-testid={`user-skill-row-${item.skillSlug}`}>
          <div className="plugin-eco-card-row">
            <span className="plugin-eco-slug">{item.skillSlug}</span>
            <div className="plugin-eco-actions">
              <span className={verdictBadgeClass(item.securityScanVerdict)}>
                {item.securityScanVerdict}
              </span>
              <span className="plugin-eco-badge">{item.licenseClassification}</span>
              <span className="plugin-eco-badge">
                {formatNumber(item.headerTokens)}/{formatNumber(MAX_HEADER_TOKENS)} {t("deRuntime.tokenUnit", "tok")}
              </span>
            </div>
          </div>
          <div className="plugin-eco-meta">{item.skillMdPath} • {item.repoSourceUrl}</div>
        </div>
      ))}
    </div>
  );
};

const RepoImportMetaInputs: React.FC<{
  form: RepoFormState;
  onPatch: (patch: Partial<RepoFormState>) => void;
}> = ({ form, onPatch }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-meta-row">
      <input
        type="text"
        className="plugin-eco-input"
        data-testid="input-import-skill-slug"
        value={form.slug}
        onChange={(e) => onPatch({ slug: e.target.value })}
        placeholder={t("deRuntime.slugPlaceholder", "skill-slug (e.g. oklch-guard)")}
        required
      />
      <input
        type="url"
        className="plugin-eco-input"
        data-testid="input-import-repo-url"
        value={form.repoUrl}
        onChange={(e) => onPatch({ repoUrl: e.target.value })}
        placeholder={t("deRuntime.repoUrlPlaceholder", "https://github.com/org/skill-repo")}
        required
      />
      <select
        className="plugin-eco-input"
        data-testid="select-import-license"
        value={form.license}
        onChange={(e) => onPatch({ license: e.target.value })}
        aria-label={t("deRuntime.licenseLabel", "SPDX License")}
      >
        {LICENSE_OPTIONS.map((opt) => (
          <option key={opt} value={opt}>{opt}</option>
        ))}
      </select>
    </div>
  );
};

const RepoImportBodyInputs: React.FC<{
  form: RepoFormState;
  submitting: boolean;
  onPatch: (patch: Partial<RepoFormState>) => void;
}> = ({ form, submitting, onPatch }) => {
  const { t } = useTranslation();
  return (
    <>
      <input
        type="text"
        className="plugin-eco-input"
        data-testid="input-import-description"
        value={form.description}
        onChange={(e) => onPatch({ description: e.target.value })}
        placeholder={t("deRuntime.descPlaceholder", "Concise skill summary (auto-clamped <=100 tok)")}
        required
      />
      <textarea
        className="plugin-eco-textarea"
        data-testid="textarea-import-markdown"
        value={form.bodyMarkdown}
        onChange={(e) => onPatch({ bodyMarkdown: e.target.value })}
        placeholder={t("deRuntime.bodyPlaceholder", "# Skill Guidelines\nDescribe deterministic rules...")}
        required
      />
      <div className="plugin-eco-actions">
        <button
          type="submit"
          className="plugin-eco-btn plugin-eco-btn-primary"
          data-testid="btn-submit-import-repo"
          disabled={submitting}
        >
          {submitting
            ? t("deRuntime.importingBtn", "Scanning & Packaging...")
            : t("deRuntime.importBtn", "Scan & Package SKILL.md")}
        </button>
      </div>
    </>
  );
};

function useRepoImportController(
  projectId: string,
  onSkillImported?: (receipt: PackagedUserSkillReceiptDto) => void
) {
  const [form, setForm] = useState<RepoFormState>(DEFAULT_FORM);
  const [items, setItems] = useState<PackagedUserSkillReceiptDto[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastReceipt, setLastReceipt] = useState<PackagedUserSkillReceiptDto | null>(null);

  useEffect(() => {
    let active = true;
    listImportedUserSkills(projectId)
      .then((res) => { if (active) setItems(res.data.items); })
      .catch((err) => {
        if (active) {
          setError(err instanceof DeRuntimeApiError ? `[${err.code}] ${err.message}` : String(err));
        }
      });
    return () => { active = false; };
  }, [projectId]);

  const patchForm = (patch: Partial<RepoFormState>) => setForm((prev) => ({ ...prev, ...patch }));

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const res = await importUserSkillRepo({
        projectId,
        skillSlug: form.slug.trim(),
        repoSourceUrl: form.repoUrl.trim(),
        licenseSpdx: form.license,
        description: form.description.trim(),
        bodyMarkdown: form.bodyMarkdown.trim(),
      });
      setLastReceipt(res.data);
      setItems((prev) => [res.data, ...prev.filter((i) => i.skillSlug !== res.data.skillSlug)]);
      onSkillImported?.(res.data);
    } catch (err) {
      setError(err instanceof DeRuntimeApiError ? `[${err.code}] ${err.message}` : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return { form, items, submitting, error, lastReceipt, patchForm, submit };
}

export const RepoImportForm: React.FC<RepoImportFormProps> = ({ projectId, onSkillImported }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const ctrl = useRepoImportController(projectId, onSkillImported);
  const estimatedTokens = estimateHeaderTokens(ctrl.form.slug, ctrl.form.description);

  return (
    <section
      className="plugin-eco-section"
      data-ff-id="plugin-repo-import-form"
      data-testid="plugin-repo-import-form"
    >
      <div className="plugin-eco-header">
        <div className="plugin-eco-title-group">
          <BrandIcon name="skill" size={ICON_SIZE} color="var(--action-primary)" />
          <h3 className="plugin-eco-title">
            {t("deRuntime.repoImportTitle", "Repo & User Skill Auto-Packager")}
          </h3>
        </div>
        <span className="plugin-eco-badge" data-testid="jit-header-token-badge">
          {t("deRuntime.jitHeaderLabel", "Tiêu đề JIT")}: {formatNumber(estimatedTokens)}/
          {formatNumber(MAX_HEADER_TOKENS)} {t("deRuntime.tokenUnit", "tok")}
        </span>
      </div>
      <form className="plugin-eco-form-grid" onSubmit={(e) => void ctrl.submit(e)}>
        <RepoImportMetaInputs form={ctrl.form} onPatch={ctrl.patchForm} />
        <RepoImportBodyInputs
          form={ctrl.form}
          submitting={ctrl.submitting}
          onPatch={ctrl.patchForm}
        />
      </form>
      {ctrl.error && (
        <div className="plugin-eco-error-box" role="alert" data-testid="repo-import-error">
          <span>{ctrl.error}</span>
        </div>
      )}
      {ctrl.lastReceipt && (
        <div className="plugin-eco-markdown-preview" data-testid="repo-import-receipt">
          {ctrl.lastReceipt.skillMdPath} • {ctrl.lastReceipt.securityScanVerdict} •{" "}
          {formatNumber(ctrl.lastReceipt.headerTokens)} {t("deRuntime.tokenUnit", "tok")}
        </div>
      )}
      <ImportedSkillsList items={ctrl.items} />
    </section>
  );
};
