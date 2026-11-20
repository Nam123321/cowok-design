# Edge Case Guardian & Adversarial Review Report — Story 06.4

- **Story:** Story 06.4 — Dual-Screen Presenter Mode, Out-of-Band `BroadcastChannel` Security, Pace Timer & Fallback Modes
- **Epic:** Epic 06 (Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench)
- **Reviewer:** Review Agent (Edge Case Guardian Skill — Stage 3A Plan Proven Safe)
- **Risk Tier:** `3`
- **Verdict:** APPROVED (PASS)

## 1. 13-Pillar FMEA & Boundary Evaluation

| # | Pillar | Failure Mode | S | O | D | RPN | Mitigation & Verification Oracle |
|---|---|---|---|---|---|---|---|
| 1 | Out-of-Band Secret Leakage | `sessionSecret` leaked via `?secret=` URL query string, `document.title`, or `localStorage` | 10 | 3 | 1 | 30 | Synchronous `window.open("/presenter/" + encodeURIComponent(deckId))` with zero `?secret=`; `sessionSecret` (`>= 32` random hex bytes via `crypto.getRandomValues`) transferred strictly via origin-checked `window.opener` / `MessageChannel` or Tauri IPC (`UT-01, E2E-01`) |
| 2 | Same-Origin Forged Broadcast | Malicious same-origin tab or opaque-origin iframe spoofing `BroadcastChannel` nav/blackout commands | 10 | 3 | 1 | 30 | Dual-end checks of `senderOrigin === window.location.origin`, `deckId`, `sessionId`, Zod `.strict()` schema, and constant-time `timingSafeEqualString(sessionSecret)`; Audience iframe stays `0 allow-same-origin`; rejected frames emit `presenter.channel_rejected.count` (`UT-02, E2E-01`) |
| 3 | Popup-Blocker Gesture & Failure | Async `await` before `window.open` triggers OS popup blocker or `window.open === null` crashes stage | 8 | 5 | 1 | 40 | `window.open(...)` executes synchronously inside `onClick` BEFORE `apiPresentationPresenter.createPresenterSession`; `null` or immediate `.closed` transitions cleanly to `POPUP_BLOCKED_INLINE_FALLBACK`, shows `CowokToast` `.ff-toast-warning` (**"Mở lại Presenter Window"**), and mounts Night Inline Split Fallback (`UT-03, E2E-02`) |
| 4 | Reload Rehydration & Delayed Sync Rollback | `F5` reload in secondary Presenter window loses `sessionSecret` / restarts `seq` at `1`, or delayed `SYNC_FULL_STATE` / duplicate command rolls back slide/timer | 9 | 4 | 1 | 36 | Fresh `senderInstanceId` + Host atomic rebind (`nextSeqFloor = 1`, `lastAcceptedSeqByInstance[id] = 0`), `stateRevision` + `(senderInstanceId, commandId)` `128`-cache; Presenter applies `SYNC_FULL_STATE` atomically only when `stateRevision > localStateRevision` (`-1` on F5) (`UT-02, UT-02B, E2E-03, E2E-03B`) |
| 5 | Double-`Esc` Trap & Stage Drop | Pressing `Esc` in Audience fullscreen needs 2 presses OR `Esc` in Presenter window kills live Audience stage | 9 | 4 | 1 | 36 | Host `fullscreenchange` listener guarantees 1-press `Esc` exit; `Esc` inside `/presenter/:deckId` closes open popover or closes/unlinks ONLY Presenter window via `PRESENTER_WINDOW_CLOSING` + `channel.close()` + `window.close()` (`UT-04, E2E-04`) |
| 6 | Keyboard IME & Input Hijack | `B`, `W`, `F`, `P`, `Esc` firing mid-Telex composition or inside inputs/textareas | 7 | 4 | 2 | 56 | Shared `shouldIgnorePresentationKeyEvent` checks `ev.isComposing || ev.keyCode === 229 || ev.metaKey || ev.ctrlKey || ev.altKey` and input/textarea/select/contentEditable targets (`UT-04`) |
| 7 | `speaker_notes` HTML Injection | Crafted `<script>` / `<img onerror=...>` inside `deck_slides.speaker_notes` executes in Presenter Night window | 10 | 2 | 1 | 20 | `speaker_notes` rendered exclusively via React text nodes (`{speakerNotes}` — zero `dangerouslySetInnerHTML` / `innerHTML`) (`UT-03, E2E-01`) |
| 8 | Next-Slide Pool Eviction | Advancing slides evicts Audience active slot or Next-Slide preview slot from `POOL_MAX_SLOTS = 5` | 7 | 3 | 2 | 42 | `pinnedSlideIds` keeps `[activeSlide?.id, nextSlide?.id]` pinned under `evictOldestSlotIfFull` (`UT-03`) |
| 9 | Historical `SNAP-*` Mutation | User attempts mutation/CMS write while presenting historical `SNAP-*` snapshot | 9 | 2 | 1 | 18 | Rust `handshake` resolves `version_snapshots` & sets `isHistoricalReadOnly: true`; Host enforces `guardReadOnlyAction` and emits `readonly_snap_lock` telemetry (`IT-03, UT-04, E2E-05`) |
| 10 | Anti-IDOR Project Boundary | Cross-project/tenant `deckId` or tampered `snapshotTag` passed to `/api/v1/decks/:deck_id/presenter/handshake` | 10 | 3 | 1 | 30 | `resolve_deck_scope` verifies `presentation_decks.project_id`, `X-Cowok-Project-Id`, `tenant_id`, `trash_deleted_at IS NULL`, and `version_snapshots` ownership -> uniform `404 E_DECK_NOT_FOUND` (`IT-02`) |
| 11 | Stopwatch Drift, Race & Timer Leak | Concurrent timer Start/Pause/Reset races, `>1s` drift over 10m, or popup-to-inline timer reset | 7 | 3 | 2 | 42 | Host-serialized `TIMER_COMMAND` transactions (`stateRevision += 1`), monotonic `performance.now()` `elapsedAtSyncMs` anchor (<50ms drift / 10m), shared Host timer state across popup/fallback, `beforeunload` `channel.close()` (`UT-03, UT-03B`) |
| 12 | `vite preview` SPA Route 404 | `window.open("/presenter/:deckId")` fails in `vite preview` because `tryServePreviewIndexHtml` only checks `/` | 8 | 4 | 1 | 32 | Extend `tryServePreviewIndexHtml` (`src/lib/vite-bff-plugin.ts:185`) to serve `dist/index.html` + `injectBootCapMeta` for `GET /presenter/:deckId` (`IT-05, E2E-01`) |
| 13 | Wave `06.3+06.4` & FSD Drift | Merge conflicts or FSD cross-feature / Pillar 7 violations on shared `flow` components | 7 | 3 | 2 | 42 | Isolate new Presenter code in dedicated files (`presentation_presenter.rs`, `PresenterConsoleWindow.tsx`, `PresenterInlineSplitFallback.tsx`, `usePresenterSessionController.ts`, `presenter-broadcast.ts`, `presentation-presenter.ts`, `api/presentation-presenter.ts`) with 0 cross-feature imports and 0 `useEffect` state sync in UI components; refactor `CompanionLiveHtmlRightPane.tsx:543` (`UT-04`) |

## 2. Formal-verify candidates
- **Formal-verify candidates:**
  1. Out-of-band `BroadcastChannel` origin + `sessionSecret` constant-time verification (`timingSafeEqualString`) and replay/stale protection (`stateRevision` monotonicity + `(senderInstanceId, commandId)` bounded `128`-entry LRU).
  2. Presenter connection FSM (`transitionPresenterConnectionState` validated via `PresenterConnectionStateSchema`) across `OPENING_POPUP` -> `AUTHENTICATED` -> `POPUP_BLOCKED_INLINE_FALLBACK` -> `CLOSED`.
  3. Dual-end monotonic stopwatch timer state (`elapsedAtSyncMs` anchor) and `guardReadOnlyAction` blocking writes during `SNAP-*` historical presentation sessions.

## 3. Final Verdict
**VERDICT: PASS / APPROVED**
