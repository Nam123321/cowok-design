// @story 09.1
// @implements AC-01, AC-02 (Live Top-5 Awesome-RE Visual App Gallery & Two-Store /rea Search)
import React, { useCallback, useEffect, useState } from "react";
import { searchAwesomeReCatalog, searchReaReferences } from "@/api/rea";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import {
  type ReaCatalogItemDto,
  type ReaFirewallVerdict,
  type ReaProofStatus,
  type ReaSearchHitDto,
} from "@/lib/validations/rea";
import { AppLayoutVisualPreview } from "./LaunchpadVisualPreview";

const ICON_XS = 12;
const STROKE_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const TOP_CATALOG_LIMIT = 5;
const SAVED_REFS_LIMIT = 4;

const QUICK_TOPIC_FILTERS: ReadonlyArray<{ id: string; query: string; labelKey: string; fallback: string }> = [
  { id: "all", query: "", labelKey: "rea.topicAll", fallback: "Top 5 nổi bật" },
  { id: "web", query: "webapp", labelKey: "rea.topicWeb", fallback: "Web Apps" },
  { id: "design", query: "canvas", labelKey: "rea.topicDesign", fallback: "Thiết kế" },
  { id: "music", query: "music", labelKey: "rea.topicMusic", fallback: "Âm nhạc" },
  { id: "video", query: "youtube", labelKey: "rea.topicVideo", fallback: "Video" },
  { id: "chat", query: "chat", labelKey: "rea.topicChat", fallback: "Chat" },
  { id: "mail", query: "mail", labelKey: "rea.topicMail", fallback: "Mail & Lịch" },
];

export const REA_INPUT_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "7px 10px",
  fontSize: "11.5px",
  fontFamily: "var(--font-sans)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-sm)",
  boxSizing: "border-box",
};

const PILL_BASE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  fontSize: "9.5px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  padding: "2px 7px",
  borderRadius: "var(--radius-pill)",
};

const VERDICT_PALETTES: Record<
  ReaFirewallVerdict,
  { bg: string; fg: string; labelKey: string; fallback: string; icon: string }
> = {
  ADOPT_PRIOR_ART: {
    bg: "var(--state-success-soft)",
    fg: "var(--state-success)",
    labelKey: "rea.verdictAdopt",
    fallback: "Dùng làm mẫu gốc",
    icon: "shield",
  },
  REFERENCE_ONLY: {
    bg: "var(--state-warning-soft)",
    fg: "var(--state-warning)",
    labelKey: "rea.verdictReference",
    fallback: "Chỉ tham khảo",
    icon: "layers",
  },
  REJECT: {
    bg: "var(--state-danger-soft)",
    fg: "var(--state-danger)",
    labelKey: "rea.verdictReject",
    fallback: "Bị chặn",
    icon: "shield-alert",
  },
};

export const FirewallVerdictBadge: React.FC<{ verdict: ReaFirewallVerdict }> = ({ verdict }) => {
  const { t } = useTranslation();
  const palette = VERDICT_PALETTES[verdict];
  return (
    <span data-testid={`rea-verdict-badge-${verdict}`} style={{ ...PILL_BASE, backgroundColor: palette.bg, color: palette.fg }}>
      <BrandIcon name={palette.icon} size={ICON_XS} strokeWidth={STROKE_MONO} color="currentColor" />
      <span>{t(palette.labelKey, palette.fallback)}</span>
    </span>
  );
};

export const ProofStatusBadge: React.FC<{ status: ReaProofStatus }> = ({ status }) => {
  const { t } = useTranslation();
  const verified = status === "proof-verified";
  return (
    <span data-testid={`rea-proof-badge-${status}`} style={{ ...PILL_BASE, backgroundColor: verified ? "var(--state-success-soft)" : "var(--state-warning-soft)", color: verified ? "var(--state-success)" : "var(--state-warning)" }}>
      <BrandIcon name={verified ? "check-circle" : "shield"} size={ICON_XS} strokeWidth={STROKE_MONO} color="currentColor" />
      <span>{verified ? t("rea.proofVerified", "Đã xác minh đối chiếu") : t("rea.proofAwaiting", "Chờ xác minh đối chiếu")}</span>
    </span>
  );
};

function extractSourceHost(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "web";
  }
}

const AwesomeReCardFooter: React.FC<{ item: ReaCatalogItemDto }> = ({ item }) => {
  const { t } = useTranslation();
  const isGithub = extractSourceHost(item.repoUrl) === "github.com";
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "4px", flexWrap: "wrap" }}>
      <FirewallVerdictBadge verdict={item.firewallVerdict} />
      <div style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
        <span style={{ fontSize: "9.5px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>{item.licenseSpdx}</span>
        <a href={item.repoUrl} target="_blank" rel="noopener noreferrer" data-testid={`launchpad-awesome-re-link-${item.slug}`} onClick={(e) => e.stopPropagation()} style={{ fontSize: "9.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--action-primary)", textDecoration: "none" }}>
          {isGithub ? t("rea.openGithubRepo", "Xem GitHub ↗") : t("rea.openWebSource", "Xem nguồn ↗")}
        </a>
      </div>
    </div>
  );
};

const AwesomeReVisualCard: React.FC<{
  item: ReaCatalogItemDto;
  isSelected: boolean;
  onSelect: (app: ReaCatalogItemDto) => void;
}> = ({ item, isSelected, onSelect }) => {
  const { formatNumber } = useFormatter();
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onSelect(item);
    }
  };
  return (
    <div role="button" tabIndex={0} data-testid={`launchpad-awesome-re-${item.slug}`} onClick={() => onSelect(item)} onKeyDown={handleKeyDown} style={{ textAlign: "left", padding: "9px", borderRadius: "var(--radius-md)", border: isSelected ? "2px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: isSelected ? "var(--bg-selected)" : "var(--bg-canvas)", cursor: "pointer", display: "flex", flexDirection: "column", gap: "7px" }}>
      <AppLayoutVisualPreview repoUrl={item.repoUrl} name={item.name} archetype={item.layoutArchetype} lang={item.lang} category={item.category} topics={item.topics} />
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "6px" }}>
        <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{item.name}</span>
        <span style={{ fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
          {item.stars > 0 ? `★ ${formatNumber(item.stars)}` : extractSourceHost(item.repoUrl)}
        </span>
      </div>
      <div style={{ fontSize: "10.5px", color: "var(--text-muted)", lineHeight: "1.35", display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical", overflow: "hidden" }}>
        {item.description}
      </div>
      <AwesomeReCardFooter item={item} />
    </div>
  );
};

const SavedReaChip: React.FC<{
  item: ReaSearchHitDto;
  isSelected: boolean;
  onSelect: (hit: ReaSearchHitDto) => void;
}> = ({ item, isSelected, onSelect }) => (
  <button
    type="button"
    data-testid={`launchpad-rea-hit-${item.slug}`}
    onClick={() => onSelect(item)}
    style={{
      padding: "5px 10px",
      borderRadius: "var(--radius-pill)",
      border: isSelected ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)",
      backgroundColor: isSelected ? "var(--bg-selected)" : "var(--bg-workspace)",
      color: "var(--text-primary)",
      cursor: "pointer",
      display: "inline-flex",
      alignItems: "center",
      gap: "6px",
      fontSize: "11px",
      fontWeight: FONT_WEIGHT_MEDIUM,
    }}
  >
    <span>{item.slug}</span>
    <FirewallVerdictBadge verdict={item.firewallVerdict} />
  </button>
);

function useAwesomeReGalleryController() {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [activeTopic, setActiveTopic] = useState("all");
  const [catalogItems, setCatalogItems] = useState<ReaCatalogItemDto[]>([]);
  const [totalIndexed, setTotalIndexed] = useState(0);
  const [savedHits, setSavedHits] = useState<ReaSearchHitDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const executeLiveSearch = useCallback(async (q: string) => {
    setLoading(true);
    setErrorMsg(null);
    try {
      const [catResp, savedResp] = await Promise.all([
        searchAwesomeReCatalog({ q, limit: TOP_CATALOG_LIMIT }),
        searchReaReferences({ q, limit: SAVED_REFS_LIMIT, tier: "1a" }).catch(() => null),
      ]);
      setCatalogItems(catResp.data.items);
      setTotalIndexed(catResp.data.totalCatalogEntries);
      setSavedHits(savedResp ? savedResp.data.items : []);
    } catch {
      setCatalogItems([]);
      setErrorMsg(t("rea.searchError", "Không thể kết nối bộ nhớ đệm mẫu cục bộ."));
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void executeLiveSearch("");
  }, [executeLiveSearch]);

  const selectTopic = (topicId: string, topicQuery: string) => {
    setActiveTopic(topicId);
    setQuery(topicQuery);
    void executeLiveSearch(topicQuery);
  };

  const submitQuery = () => {
    setActiveTopic("custom");
    void executeLiveSearch(query);
  };

  return { query, setQuery, activeTopic, catalogItems, totalIndexed, savedHits, loading, errorMsg, selectTopic, submitQuery };
}

const TopicFilterStrip: React.FC<{
  activeTopic: string;
  onSelectTopic: (id: string, query: string) => void;
}> = ({ activeTopic, onSelectTopic }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", gap: "5px", flexWrap: "wrap" }}>
      {QUICK_TOPIC_FILTERS.map((f) => {
        const active = activeTopic === f.id;
        return (
          <button key={f.id} type="button" data-testid={`launchpad-rea-topic-${f.id}`} onClick={() => onSelectTopic(f.id, f.query)} style={{ padding: "3px 9px", fontSize: "10.5px", fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-pill)", border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)", color: active ? "var(--action-primary)" : "var(--text-secondary)", cursor: "pointer" }}>
            {t(f.labelKey, f.fallback)}
          </button>
        );
      })}
    </div>
  );
};

const SavedReaStrip: React.FC<{
  hits: ReaSearchHitDto[];
  selectedTemplateId: string;
  onSelectReference: (hit: ReaSearchHitDto) => void;
}> = ({ hits, selectedTemplateId, onSelectReference }) => {
  const { t } = useTranslation();
  if (hits.length === 0) return null;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "6px", paddingTop: "6px", borderTop: "1px dashed var(--border-subtle)" }}>
      <span style={{ fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-muted)" }}>{t("rea.savedLibraryTitle", "Mẫu bạn đã lưu cục bộ")}</span>
      <div style={{ display: "flex", gap: "6px", flexWrap: "wrap" }}>
        {hits.map((hit) => (
          <SavedReaChip key={hit.id} item={hit} isSelected={selectedTemplateId === `rea:${hit.slug}`} onSelect={onSelectReference} />
        ))}
      </div>
    </div>
  );
};

export const LaunchpadReaSearchSection: React.FC<{
  selectedTemplateId: string;
  onSelectReference: (hit: ReaSearchHitDto) => void;
  onSelectCatalogApp?: (app: ReaCatalogItemDto) => void;
}> = ({ selectedTemplateId, onSelectReference, onSelectCatalogApp }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const g = useAwesomeReGalleryController();

  return (
    <div data-testid="launchpad-rea-search-section" data-ff-id="rea-two-store-library" style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
        <span style={{ fontSize: "11px", color: "var(--text-secondary)", display: "inline-flex", alignItems: "center", gap: "5px" }}>
          <BrandIcon name="layers" size={ICON_XS} strokeWidth={STROKE_MONO} color="var(--action-primary)" />
          <span>{t("rea.specSeedHint", "Tìm trong Awesome RE (Top 5 trực quan) — tự nạp vào 03-ui-ux-spec.md cho Chat.")}</span>
        </span>
        {g.totalIndexed > 0 && (
          <span style={{ fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
            {formatNumber(g.catalogItems.length)} / {formatNumber(g.totalIndexed)}
          </span>
        )}
      </div>

      <div style={{ display: "flex", gap: "6px" }}>
        <input data-testid="launchpad-rea-search-input" type="text" value={g.query} onChange={(e) => g.setQuery(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") g.submitQuery(); }} placeholder={t("rea.searchPlaceholder", "Tìm app/website trong Awesome RE (vd: figma, music, youtube, mail, chat)...")} style={REA_INPUT_STYLE} />
        <button type="button" data-testid="launchpad-rea-search-btn" onClick={g.submitQuery} style={{ padding: "6px 10px", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "5px", whiteSpace: "nowrap" }}>
          <BrandIcon name="search" size={ICON_XS} strokeWidth={STROKE_MONO} color="currentColor" />
          <span>{t("rea.searchBtn", "Tìm")}</span>
        </button>
      </div>

      <TopicFilterStrip activeTopic={g.activeTopic} onSelectTopic={g.selectTopic} />

      {g.loading && <div data-testid="launchpad-rea-loading" style={{ fontSize: "10.5px", color: "var(--text-muted)" }}>{t("rea.loadingLibrary", "Đang tải...")}</div>}
      {g.errorMsg && <div data-testid="launchpad-rea-search-error" style={{ fontSize: "10.5px", color: "var(--state-warning)" }}>{g.errorMsg}</div>}
      {!g.loading && !g.errorMsg && g.catalogItems.length === 0 && (
        <div data-testid="launchpad-rea-empty" style={{ fontSize: "11px", color: "var(--text-muted)" }}>{t("rea.emptyLibrary", "Không tìm thấy ứng dụng khớp từ khóa — bạn có thể bắt đầu ngay trong Chat.")}</div>
      )}
      {g.catalogItems.length > 0 && (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "8px" }}>
          {g.catalogItems.map((app) => (
            <AwesomeReVisualCard key={app.slug} item={app} isSelected={selectedTemplateId === `awesome-re:${app.slug}`} onSelect={(a) => onSelectCatalogApp?.(a)} />
          ))}
        </div>
      )}

      <SavedReaStrip hits={g.savedHits} selectedTemplateId={selectedTemplateId} onSelectReference={onSelectReference} />
    </div>
  );
};
