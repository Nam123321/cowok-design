// @story 09.1
// @implements AC-01, AC-02 (Real GitHub OpenGraph App Preview with Honest Offline Repo Metadata Fallback — Zero Mock Wireframes)
import React, { useEffect, useState } from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";

const ICON_SM = 13;
const STROKE_MONO = 1.85;
const MAX_FALLBACK_TOPICS = 3;
const FONT_WEIGHT_SEMIBOLD = 600;

export function extractGitHubOwnerRepo(repoUrl: string): string | null {
  try {
    const parsed = new URL(repoUrl);
    if (parsed.hostname !== "github.com" && parsed.hostname !== "www.github.com") return null;
    const parts = parsed.pathname.split("/").filter(Boolean);
    if (parts.length < 2) return null;
    return `${parts[0]}/${parts[1]}`;
  } catch {
    return null;
  }
}

export function buildGitHubOpenGraphUrl(repoUrl: string): string | null {
  const ownerRepo = extractGitHubOwnerRepo(repoUrl);
  return ownerRepo ? `https://opengraph.githubassets.com/1/${ownerRepo}` : null;
}

const HonestOfflineRepoCard: React.FC<{
  ownerRepo: string;
  lang: string;
  category: string;
  topics: string[];
}> = ({ ownerRepo, lang, category, topics }) => {
  const { t } = useTranslation();
  return (
    <div
      data-testid="awesome-re-repo-fallback"
      style={{
        width: "100%",
        height: "100%",
        padding: "8px 10px",
        boxSizing: "border-box",
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
        backgroundColor: "var(--bg-surface)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
        <span style={{ fontSize: "10.5px", fontFamily: "var(--font-mono)", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", display: "inline-flex", alignItems: "center", gap: "4px" }}>
          <BrandIcon name="git-branch" size={ICON_SM} strokeWidth={STROKE_MONO} color="var(--action-primary)" />
          <span>{ownerRepo}</span>
        </span>
        {lang && (
          <span style={{ fontSize: "9.5px", fontFamily: "var(--font-mono)", color: "var(--text-secondary)", padding: "1px 6px", borderRadius: "var(--radius-pill)", border: "1px solid var(--border-subtle)" }}>
            {lang}
          </span>
        )}
      </div>
      <div style={{ fontSize: "10px", color: "var(--text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {category || t("rea.topicAll", "Awesome RE")}
      </div>
      {topics.length > 0 && (
        <div style={{ display: "flex", gap: "4px", flexWrap: "wrap", overflow: "hidden", maxHeight: "20px" }}>
          {topics.slice(0, MAX_FALLBACK_TOPICS).map((topic) => (
            <span key={topic} style={{ fontSize: "9px", padding: "1px 5px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)", border: "1px solid var(--border-subtle)" }}>
              #{topic}
            </span>
          ))}
        </div>
      )}
    </div>
  );
};

export const AppLayoutVisualPreview: React.FC<{
  repoUrl: string;
  name: string;
  archetype: string;
  lang?: string;
  category?: string;
  topics?: string[];
}> = ({ repoUrl, name, archetype, lang = "", category = "", topics = [] }) => {
  const { t } = useTranslation();
  const ogUrl = buildGitHubOpenGraphUrl(repoUrl);
  const ownerRepo = extractGitHubOwnerRepo(repoUrl) ?? name;
  const [imgFailed, setImgFailed] = useState(false);

  useEffect(() => {
    setImgFailed(false);
  }, [ogUrl]);

  return (
    <div
      data-testid={`awesome-re-visual-${archetype}`}
      style={{
        width: "100%",
        height: "110px",
        borderRadius: "var(--radius-sm)",
        backgroundColor: "var(--bg-workspace)",
        border: "1px solid var(--border-default)",
        overflow: "hidden",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {ogUrl && !imgFailed ? (
        <img
          src={ogUrl}
          alt={`${name} — ${t("rea.realGithubPreviewAlt", "Xem trước kho GitHub thực tế")}`}
          loading="lazy"
          referrerPolicy="no-referrer"
          onError={() => setImgFailed(true)}
          style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
        />
      ) : (
        <HonestOfflineRepoCard ownerRepo={ownerRepo} lang={lang} category={category} topics={topics} />
      )}
    </div>
  );
};
