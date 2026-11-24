# ADR-009: Zero-Hardcode AI Harness Spine, 3-Bucket Skill Ownership, Layer 1 Data Storytelling & Closed-Loop Studio-to-Skill Evolution (`TDR-009`)

**Status:** ACCEPTED  
**Date:** 2026-11-24  
**Deciders:** Product Owner, System Architect, AI Harness Lead  
**Supersedes / Extends:** `ADR-004`, `ADR-006`, `ADR-007`, `ADR-008`  
**Target Implementation Stories:** `Story 08.7` (Pre-Wave C Keystone), `Story 10.3` (Reflective Skill PR Pipeline), `Story 13.1` (Sealed `design-craft` SemVer Gating)

---

## 1. Context
Kiểm toán mã nguồn thực tế trên `cowok-design` sau Wave B (`05.5`, `06.5`, `10.1`, `14.1`) phát hiện 3 nhóm khoảng trống kiến trúc cần chuẩn hóa dứt điểm trước khi mở rộng sang Wave C–E:

1. **Split-Brain & Hardcode trong AI Harness Core:**
   - `crates/spec-registry/src/skill_router.rs` vẫn khóa cứng `CRAFT_SKILL_SPECS: [CraftSkillSpec; 7]` kèm bộ khớp từ khóa chỉ tiếng Anh (`intent.contains(...)`), đồng thời chèn `active_skills` vào giữa tiền tố hệ thống (gây phá prefix KV-cache).
   - `mount_default_design_craft_plugin()` (`crates/plugin-host/src/default_mount.rs`) chưa được nối vào luồng khởi động và định tuyến động của `crates/harness-daemon` + `orchestrator.rs`.
   - `crates/bot-fleet/src/byok_gateway.rs` khóa cứng `default_model_for_provider(...)` và **chưa phát payload prompt caching wire-native** (`cache_control: { type: "ephemeral" }` cho Anthropic, `cachedContent` cho Gemini, `prompt_cache_key` cho OpenAI).
   - Trùng lặp sở hữu skill thiết kế giữa `packages/builtin-skills/skills/` (di sản lịch sử) và `packages/builtin-skills/design-craft/`.
2. **Tầng Data Storytelling (Layer 1) trong Studio Biểu đồ & Dashboard (`Story 06.5`):**
   - Đã triển khai tích hợp nguyên bản repo cộng đồng MIT `abdiraimov-otabek/dashboard-design-skill` tại `upstream-skills/dashboard-design/` (tự động đồng bộ Cron `Story 08.4`) và nâng cấp `data-viz-storyteller v2.0` (`SKILL.md` + 3 file `references/{storytelling-arc,chart-selection-matrix,html-canvas-contract}.md`), cần chốt thành chuẩn chính thức trong PRD & Architecture.
3. **Thiếu 2 Cầu nối Vòng lặp Tự tiến hóa (Closed-Loop Self-Evolve):**
   - **Bridge 1 (Manual Studio Correction → `gate_failure_ledger`):** Khi người dùng trong `Studio Biểu đồ & Dashboard` phải sửa thủ công lặp lại các lỗi AI (`>= 3` lần, ví dụ bấm `✦ Chuẩn hóa` hoặc đổi loại biểu đồ AI chọn sai), tín hiệu này chưa chảy về bảng `V18a gate_failure_ledger` để đề xuất thẻ `LEARN`.
   - **Bridge 2 (Reflective Skill PR Pipeline: `advisor 10.3` + `Sealed Eval 13.1` → `design-craft`):** Chưa có quy chuẩn đóng gói bản vá `SKILL.md`/`references/*.md` theo phong cách GEPA Reflective Optimization đi qua kiểm định mù `Sealed Anti-Goodhart Benchmark` trước khi mở PR nâng bản SemVer `v1.x` cho `design-craft`.

---

## 2. Decision

### 2.1. Quy tắc 3-Bucket Canonical Skill Ownership (`1 Capability = 1 Canonical Owner`)
Tách bạch tuyệt đối 3 vùng sở hữu tri thức để chấm dứt split-brain và giữ tính độc lập cho repo mở `design-craft` (theo mô hình **Model C**: phát triển trong monorepo ở giai đoạn Wave B–D, tách thành repo công khai độc lập `cowok-ai/design-craft` hậu Wave D):
1. **`packages/design-craft/` (Canonical Owner — 100% Design, UX & Data Storytelling Skills + `references/` + Portable DE/QC Tools + Multi-Agent Pack):**
   - Sở hữu duy nhất toàn bộ kỹ năng thiết kế & trình bày (`phase-*`, `craft/*`, `domain-business/*` gồm `data-viz-storyteller v2.0`).
   - Không phụ thuộc ngược vào mã nguồn Rust/TS nội bộ của `cowok-design`; cài độc lập được cho `Cowok.ai`, `Claude Code`, `Codex`, `Cursor`, `Antigravity`, `OpenClaw`.
2. **`packages/host-skills/` (Thay thế `packages/builtin-skills/skills/` — Chỉ chứa Host-Runtime Skills):**
   - Chỉ giữ các kỹ năng gắn chặt vòng đời daemon/harness của `cowok-design` (khởi tạo workspace, quản trị SPEC 01..09, điều phối swarm/lease nội bộ). Loại bỏ/deprecate bản sao skill thiết kế trùng với `design-craft`.
3. **`upstream-skills/` (100% Verbatim Community Repos — Zero Forking):**
   - Lưu nguyên bản byte-for-byte các repo cộng đồng có giấy phép mở hợp lệ kèm tệp metadata mỏng `upstream.json` (khai báo `repoUrl`, `pinnedCommit`, `license`, `cronSchedule`) để **Story `08.4` GitHub Cron Sync Scheduler** tự động quét cập nhật.
   - Gói tiên phong: `abdiraimov-otabek/dashboard-design-skill` (MIT, pinned `a31edee`, Cron `0 0 3 * * *`).

### 2.2. Chuẩn hóa Layer 1 Data Storytelling, Semantic Role Contract & Triết lý Minimum Floor cho Toàn Bộ Skills (`FR-34`, `FR-39`)
- **Hiến chương Kỹ năng & Tài liệu Tham chiếu (Zero Hardcode, Zero Hardlink, Minimum Floor — Never Maximum Ceiling):**
  - **Zero Hardcode / Zero Domain Leak:** Mọi `SKILL.md` và `references/*.md` tuyệt đối không khóa cứng ID nghiệp vụ demo (như `kpi-revenue-card`, `Q4 Revenue`), không hardcode mã màu hex, không chốt cứng tên token CSS mâu thuẫn nhau giữa các skill (`--surface-primary` vs `--bg-surface`), và không chốt con số tĩnh lỗi thời trong prompt (`67 styles`, `50 palettes`, `24 verbs`, `1280x720`) — luôn đọc động từ `tokens.css` / catalog hoặc đặt sàn `>= N`.
  - **Zero Hardlink / Standalone Purity:** Mọi skill thuộc `packages/design-craft/` phải tự khép kín 100% (không trỏ sang thư mục legacy `packages/builtin-skills/skills/` như 3 liên kết đứt `flow-state-completer`, `reference-design-contract`, `self-healing-eval-gate` trước đó; không dùng đường dẫn tuyệt đối `/Users/...` hay `../../`).
  - **Minimum Floor, Never Maximum Ceiling:** Mọi skill chia rõ 3 tầng: (1) *Non-Negotiable Minimum Floor* (`>= N` tiêu chuẩn trung thực/an toàn bắt buộc), (2) *Recommended Core Patterns*, và (3) *Open-Ended Extensible Palette* trao quyền cho Agent tự do suy diễn và sáng tạo thêm pattern mới phù hợp với đặc thù dữ liệu/domain.
  - **Progressive Disclosure Đạt Chuẩn (Loại bỏ Stub 9 dòng):** Nâng cấp 7 Craft Skills đang ở dạng stub 9 dòng (`archify-diagram`, `dashboard-ux`, `impeccable-polish`, `oklch-palette`, `sketch-monochrome`, `stageshow-slide`, `typography`) thành `SKILL.md` (`<= 100 tok` frontmatter) + `references/*.md` chuyên sâu (và hợp nhất 4 cặp skill trùng ngữ nghĩa với thư mục legacy `skills/`).
- **Nâng cấp `data-viz-storyteller v2.1` (3-Tier Semantic Role & Full 5+8 Storytelling Patterns):**
  - **Upstream Verbatim Peer:** `dashboard-design` (MIT nguyên bản, 10 references + 3 HTML examples, auto-sync Cron `08.4`).
  - **Semantic Role HTML Contract (`references/html-canvas-contract.md`):** Thay thế 5 anchor hardcode demo bằng hợp đồng vai trò ngữ nghĩa bất biến theo domain (`data-ff-role="action-title | kpi-card | chart-panel | data-datum | annotation-callout | breakdown-panel | provenance-footnote"`, `data-row-id`, `data-whatif-handle`, `data-ff-callout-for`, `data-ff-compare-pair`, `data-audience-mode="ceo-3s | detailed"`), kèm bảng chuẩn **5 quy tắc Honest & Wordless Auto-Fix + Undo** và bất biến `sourceRows` divergence.
  - **Narrative & Extensible Pattern Catalog (`references/storytelling-arc.md` & `chart-selection-matrix.md`):** Phủ đủ **5 Core UX Patterns** (`AI Lens Strip >= 3`, `Contextual Point Actions 1-điểm & 2-điểm`, `Honest & Wordless 1-Click Auto-Fix`, `What-If Scenario Drag + Divergence Banner + Save Slide`, `Audience Mode CEO 3s ⇄ Chi tiết`) **CỘNG** **8 Extensible Patterns gợi mở** (`Small Multiples`, `Event-Marker / Regime-Shift Bands`, `Cross-Filtering`, `Scrollytelling Reveal`, `Variance Waterfall`, `Distribution & Uncertainty Encoding`, `Slopegraph & Benchmark Bands`, `Interactive Sensitivity Sliders`) — khẳng định rõ Agent có toàn quyền sáng tạo thêm.
- Khớp 1:1 với bề mặt **Option D v2 Resizable Master-Detail `Studio Biểu đồ & Dashboard`** (`RightDrawer <= 85vw`, Control Plane 2 cấp + Bảng nhập trực tiếp + Divergence Banner + Undo/Redo + Sub-Chat đồng bộ về Main Chat SQLite + Toolbar Canvas hợp nhất 2 hàng).

### 2.3. Zero-Hardcode AI Harness Spine (`Story 08.7` — Pre-Wave C)
1. **`UnifiedSkillCatalog` (Thay thế 100% `CRAFT_SKILL_SPECS` hardcode):**
   - Tự động quét và hợp nhất 4 nguồn động khi boot và khi hot-reload: (a) `packages/host-skills`, (b) `mount_default_design_craft_plugin()` (`packages/design-craft`), (c) `upstream-skills/*` (verbatim MIT/Apache), và (d) SQLite `skill_plugins (state = 'ACTIVE')` + thẻ `LEARN` đã duyệt `HUMAN_ONLY`.
   - Loại bỏ hoàn toàn mảng tĩnh 7 phần tử và bộ khớp từ khóa chỉ tiếng Anh; sử dụng đa ngôn ngữ (VI/EN/...) qua FTS5 BM25 + `DecisionCascadeClient` (`Story 09.2`) với điểm fallback xác định.
2. **2-Breakpoint Cache-Safe Prompt Compiler (Chuẩn Prompt Caching Ngành):**
   - **Zone 1 — Static Cached Prefix (Bất biến xuyên phiên):** System identity + Tool schemas + **Tier-1 Skill Index** (`name` + `description <= 100 tok`/skill, sắp xếp deterministic theo `skill_id`) + Project `DESIGN.md` / `tokens.css` hash.
   - **Zone 2 — Dynamic Turn Tail / Tool Results (Cuối ngữ cảnh):** **Tier-2 `SKILL.md`** body và **Tier-3 `references/*.md`** chỉ được nạp JIT vào turn/tool-result khi được kích hoạt (tuyệt đối KHÔNG chèn vào Zone 1 gây cache miss). Kích hoạt **Cache-Safe Compaction** khi lịch sử đạt ngưỡng `75%` cửa sổ ngữ cảnh.
3. **Wire-Native LLM Provider Caching & Dynamic Model Registry:**
   - Gỡ bỏ `default_model_for_provider` cứng trong `crates/bot-fleet/src/byok_gateway.rs`; đọc từ Dynamic Model Registry (JSON/SQLite cấu hình được qua Settings & Cloud Sync, có capability flags `supports_prompt_cache`, `context_window`, `tier`).
   - Phát tham số cache gốc của từng nhà cung cấp: Anthropic `cache_control: { "type": "ephemeral" }` tại 2 điểm ngắt (System Prefix + Tool/History Tail), Gemini `cachedContent` / context caching handle, OpenAI `prompt_cache_key` ổn định theo `(project_id, catalog_sha256)`.

### 2.4. Hai Cầu nối Closed-Loop Self-Evolve (`Bridge 1` & `Bridge 2`)
1. **Bridge 1 — `Manual Studio Correction → V18a gate_failure_ledger` (`Story 08.7`):**
   - Ghi nhận cục bộ (opt-in, ẩn danh, Zero-PII) các mẫu sửa lỗi lặp lại trong Studio (ví dụ: người dùng đổi loại biểu đồ ngay sau khi AI sinh, hoặc bấm `✦ Chuẩn hóa` tự sửa nhãn/tỷ trọng/tương phản).
   - Khi cùng một `correction_signature` đạt ngưỡng `hit_count >= 3`, đẩy vào `V18a gate_failure_ledger` để kích hoạt đề xuất thẻ `LEARN` (yêu cầu duyệt `HUMAN_ONLY` trước khi kích hoạt vào `UnifiedSkillCatalog`).
2. **Bridge 2 — `Reflective Skill PR Pipeline` (`Story 10.3` + `Story 13.1` → `design-craft`):**
   - `advisor` (`Story 10.3`) đọc các cụm lỗi từ `gate_failure_ledger` & `creative_memory_events`, chạy chẩn đoán phản chiếu kiểu **GEPA (Genetic-Pareto Reflective Prompt/Skill Optimization)** để đề xuất bản vá `SKILL.md` / `references/*.md` dưới dạng Patch/PR vào nhánh `design-craft` (Staging).
   - Mọi bản vá BẮT BUỘC vượt qua **Sealed Anti-Goodhart Benchmark (`Story 13.1`)** với đối chứng bắt cặp (`with_skill > baseline`, `0` hồi quy trên toàn bộ các brief bao gồm cặp `data-viz-storyteller` + `dashboard-design`) và duyệt `HUMAN_ONLY` trước khi cắt bản phát hành SemVer `v1.x` để `Story 08.4` Cron tự động phân phối lại cho toàn bộ người dùng.

---

## 3. Consequences
- **Tối ưu chi phí & tốc độ:** Giữ ổn định Zone 1 Static Cached Prefix + wire-native provider caching giúp giảm `60%–85%` chi phí input token và độ trễ TTFT trên các phiên thiết kế dài kỳ.
- **0 Hardcode & Đa ngôn ngữ thật:** Mọi skill mới cài từ `design-craft`, `upstream-skills` (như `dashboard-design`), hoặc repo GitHub người dùng đều tự động tham gia định tuyến cho cả prompt tiếng Việt lẫn tiếng Anh mà không cần biên dịch lại Rust.
- **Vòng lặp tiến hóa an toàn chống Goodhart:** Kết nối trực tiếp trải nghiệm sửa tay thực tế của người dùng trên Studio vào `gate_failure_ledger` $\to$ `advisor` $\to$ `Sealed Eval` $\to$ `design-craft` SemVer mà vẫn giữ 100% kiểm soát `HUMAN_ONLY`.
- **Trade-off chấp nhận:** Chèn `Story 08.7` làm bước **Pre-Wave C** trước khi vào Wave C (`07.1`, `10.2`, `11.2`, `14.2`) để đảm bảo Swarm (`10.2`) và Universal Open MCP (`11.2`) kế thừa ngay `UnifiedSkillCatalog` không hardcode thay vì phải viết lại 2 lần.
