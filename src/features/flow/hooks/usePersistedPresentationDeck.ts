// @story 06.2
// @implements AC-01, AC-02
import { useEffect, useRef, useState } from "react";
import { apiPresentation } from "../../../api/presentation";
import type {
  PresentationDeckDto,
  SlideLayoutKind,
  SlideSlotsSnapshotResponse,
} from "../../../lib/validations/presentation";
import type { SlidePhaseItem } from "../components/companion-canvas-data";

const SEED_LAYOUT_CYCLE: readonly SlideLayoutKind[] = [
  "hero_cover",
  "split_comparison",
  "kpi_metric_grid",
];

function extractSlotFieldValues(snap: SlideSlotsSnapshotResponse | null): { headline?: string; subtitle?: string } {
  if (!snap) return {};
  const headingSlot = snap.slots.find((sl) => sl.slotKind === "heading");
  const bodySlot = snap.slots.find(
    (sl) => sl.ffId.endsWith("-body") || sl.ffId === "ff-slide-subtitle" || sl.slotKind === "text"
  );
  return {
    headline: headingSlot ? headingSlot.textValue : undefined,
    subtitle: bodySlot ? bodySlot.textValue : undefined,
  };
}

function mapSingleSlidePhaseItem(args: {
  slide: PresentationDeckDto["slides"][number];
  idx: number;
  deckTitle: string;
  prevSlide?: SlidePhaseItem;
  fallback?: SlidePhaseItem;
  snapshot: SlideSlotsSnapshotResponse | null;
}): SlidePhaseItem {
  const { slide, idx, deckTitle, prevSlide, fallback, snapshot } = args;
  const slotVals = extractSlotFieldValues(snapshot);
  const codePrefix = `SLIDE ${String(idx + 1).padStart(2, "0")}`;
  const defaultTitle = `Slide ${idx + 1}`;
  return {
    id: slide.slideUid,
    code: slide.eyebrowTag || `${codePrefix} • ${slide.layoutKind.toUpperCase()}`,
    title: slotVals.headline || fallback?.title || prevSlide?.title || slide.eyebrowTag || defaultTitle,
    headline: slotVals.headline ?? (fallback?.headline || deckTitle || slide.eyebrowTag || defaultTitle),
    subtitle: slotVals.subtitle ?? (slide.narrativeArc || fallback?.subtitle || ""),
    speakerNotes: snapshot?.speakerNotes ?? slide.speakerNotes ?? prevSlide?.speakerNotes ?? "",
    kpi1: prevSlide?.kpi1 ?? 0,
    kpi2: prevSlide?.kpi2 ?? "OK",
    kpi3: prevSlide?.kpi3 ?? "<16ms",
  };
}

function mapDeckToSlidePhaseItems(
  deck: PresentationDeckDto,
  existing: SlidePhaseItem[],
  snapshots: Array<SlideSlotsSnapshotResponse | null>,
  preferPersistedOverFallback: boolean
): SlidePhaseItem[] {
  if (deck.slides.length === 0) return existing;
  return deck.slides.map((slide, idx) =>
    mapSingleSlidePhaseItem({
      slide,
      idx,
      deckTitle: deck.title,
      prevSlide: existing[idx],
      fallback: preferPersistedOverFallback ? undefined : existing[idx],
      snapshot: snapshots[idx] ?? null,
    })
  );
}

function buildInitialSlideSeeds(initialSlides: SlidePhaseItem[]) {
  if (initialSlides.length === 0) {
    return [
      {
        layoutKind: "hero_cover" as SlideLayoutKind,
        headline: "Executive Overview",
        eyebrowTag: "SLIDE 01",
        narrativeArc: "",
      },
    ];
  }
  return initialSlides.map((seed, idx) => ({
    layoutKind: SEED_LAYOUT_CYCLE[idx % SEED_LAYOUT_CYCLE.length] ?? "hero_cover",
    headline: seed.headline || seed.title || `Slide ${idx + 1}`,
    eyebrowTag: seed.code || seed.title || `SLIDE ${String(idx + 1).padStart(2, "0")}`,
    narrativeArc: seed.subtitle || "",
  }));
}

async function resolveOrCreateDeckWithSnapshots(
  projectId: string,
  initialSlides: SlidePhaseItem[]
): Promise<{ deck: PresentationDeckDto; snapshots: Array<SlideSlotsSnapshotResponse | null> } | null> {
  try {
    const existing = await apiPresentation.listDecks(projectId);
    let deck: PresentationDeckDto;
    if (existing.length > 0) {
      deck = existing[0];
    } else {
      const first = initialSlides[0];
      deck = await apiPresentation.createDeck(
        projectId,
        {
          title: first?.title || "Executive Pitch Deck",
          themePreset: "warm",
          aspectRatio: "16:9",
          deckMode: "multi-page",
          initialSlides: buildInitialSlideSeeds(initialSlides),
        },
        { idempotencyKey: `boot-deck-${projectId}` }
      );
    }
    const settled = await Promise.allSettled(
      deck.slides.map((s) => apiPresentation.listSlideSlots({ projectId, deckId: deck.id, slideUid: s.slideUid }))
    );
    const snapshots = settled.map((r) => (r.status === "fulfilled" ? r.value : null));
    return { deck, snapshots };
  } catch (err) {
    if (typeof console !== "undefined") {
      console.warn("[usePersistedPresentationDeck] Failed to hydrate persisted deck; falling back to local-deck", err);
    }
    return null;
  }
}

function useDeckInvalidationTick(projectId?: string): number {
  const [invalidateTick, setInvalidateTick] = useState(0);
  useEffect(() => {
    if (typeof window === "undefined") return;
    const onInvalidated = (ev: Event) => {
      const detail = (ev as CustomEvent<{ projectId?: string }>).detail;
      if (!projectId || (detail?.projectId && detail.projectId !== projectId)) return;
      setInvalidateTick((n) => n + 1);
    };
    window.addEventListener("cowok:project-state-invalidated", onInvalidated);
    return () => window.removeEventListener("cowok:project-state-invalidated", onInvalidated);
  }, [projectId]);
  return invalidateTick;
}

export function usePersistedPresentationDeck(opts: {
  projectId?: string;
  enabled: boolean;
  slides: SlidePhaseItem[];
  setSlides: React.Dispatch<React.SetStateAction<SlidePhaseItem[]>>;
  reloadToken?: number;
}): { deckId: string; isDeckResolving: boolean } {
  const { projectId, enabled, slides, setSlides, reloadToken = 0 } = opts;
  const slidesRef = useRef(slides);
  slidesRef.current = slides;
  const invalidateTick = useDeckInvalidationTick(projectId);
  const resolveKey = projectId && enabled ? `${projectId}::slide-cms::${reloadToken + invalidateTick}` : "";
  const [resolvedState, setResolvedState] = useState<{ projectId: string; deckId: string; resolvedKey: string }>({
    projectId: "",
    deckId: "local-deck",
    resolvedKey: "",
  });

  useEffect(() => {
    if (!resolveKey || !projectId) {
      setResolvedState({ projectId: projectId || "", deckId: "local-deck", resolvedKey: "" });
      return;
    }
    let cancelled = false;
    void resolveOrCreateDeckWithSnapshots(projectId, slidesRef.current).then((result) => {
      if (cancelled) return;
      if (result) {
        setResolvedState({ projectId, deckId: result.deck.id, resolvedKey: resolveKey });
        setSlides((prev) => mapDeckToSlidePhaseItems(result.deck, prev, result.snapshots, reloadToken > 0 || invalidateTick > 0));
      } else {
        setResolvedState({ projectId, deckId: "local-deck", resolvedKey: resolveKey });
      }
    });
    return () => { cancelled = true; };
  }, [invalidateTick, projectId, reloadToken, resolveKey, setSlides]);

  const isSameProject = resolvedState.projectId === (projectId || "");
  const isDeckResolving = Boolean(resolveKey && (!isSameProject || resolvedState.resolvedKey !== resolveKey));
  return {
    deckId: !enabled || !isSameProject || isDeckResolving ? "local-deck" : resolvedState.deckId,
    isDeckResolving,
  };
}
