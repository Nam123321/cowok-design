# Party Mode Transcript — Epic 01: Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry

- **Epic ID:** `Epic-01`
- **Value Stream:** `VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine`
- **Execution Mode:** Deep Mode (`CS = 6.0` — Architectural Foundation & 16-Table SQLite WAL Genesis)
- **Participants:**
  - `PM` (Product Management — Scope, 9-Spec Provenance, Cross-Project Brand Reuse)
  - `Architect` (System Architecture — 11-Crate Workspace, `ts-rs` Type Bridge, Loopback Axum Daemon)
  - `Review` (Adversarial Risk & Security — `StaleFile` SHA-256 OCC, Path Traversal Jail, Atomic Writes)
  - `Data-Architect` (Database & Storage — 16 SQLite WAL Tables, 5-Stream Isolation, `notify` Watcher Debounce)
  - `AI-Engineer` (Agent Ops & Orchestration — `cowok` Orchestrator Context Diet, `CONFIRMED`/`INFERRED`/`MISSING` Tagging)

---

## Round 1: Initial Proposal, Boundary Challenge & Concurrency Pushback

### 1.1 `PM` (Product Framing & Non-Tech User Empathy)
> **Position:** `Epic 01` là viên gạch đầu tiên (`In-Degree = 0`) của toàn bộ **UI/UX OS (FlowForge Harness OS)**. Mục tiêu cốt lõi của người dùng (cả đội Product UI/UX, đội Marketing làm HTML Presentation Deck, và đội vận hành One-Person Company qua `cowok`) là đi từ một bản mô tả ý tưởng thô (`Product Brief`) hoặc một repo sẵn có ra **9 Foundational Project Context Specs** (`01-product-overview.md` đến `09-data-and-copy.md`).
>
> **Thách thức đặt ra:** Nếu người dùng chỉ nhập 3 dòng ý tưởng ngắn ngủi (ví dụ: *"Làm trang landing page và deck giới thiệu cho sản phẩm SaaS quản lý kho"*), hệ thống không được phép "bịa" (hallucinate) các quy định thương hiệu như thể người dùng đã xác nhận. Mọi thông tin trong 9 file spec phải gắn nhãn nguồn gốc (`CONFIRMED`, `INFERRED`, `MISSING`) rõ ràng, đồng thời cho phép tái sử dụng hồ sơ thương hiệu (`04-design-direction.md`, `08-brand-guidelines.md`) giữa nhiều dự án đan xen nhau (`FR-11`, `FR-12`).

### 1.2 `Architect` (Workspace Topology & `ts-rs` Zero-Drift Bridge)
> **Pushback & Thiết kế:** Tôi đồng ý với `PM`, nhưng để tránh việc sau này phải đập đi xây lại từ TypeScript sang Rust, chúng ta đã chốt ở **ADR-001** là dựng **Cargo Workspace 11 crates** ngay từ `Story 01.1`.
>
> **Điểm mấu chốt cần khóa chặt:**
> 1. Trong `Story 01.1`, chúng ta phải khởi tạo đầy đủ 11 crate manifests (`crates/domain`, `crates/spec-registry`, `crates/constraint-engine`, `crates/flow-graph`, `crates/html-forge`, `crates/presentation-cms`, `crates/plugin-host`, `crates/rea-bridge`, `crates/bot-fleet`, `crates/mcp-gateway`, `crates/harness-daemon`) để khóa ranh giới phụ thuộc (DAG crate compilation), dù logic nghiệp vụ của `Epic 01` tập trung hoàn toàn vào 3 crate gốc: `crates/domain`, `crates/spec-registry`, và `crates/harness-daemon`.
> 2. Mọi struct trong `crates/domain` (`Project`, `ProjectSpec`, `ProvenanceStatus`, `ConstraintProfile`, `StaleFileError`) phải gắn `#[derive(Serialize, Deserialize, TS)]` và xuất trực tiếp ra `packages/shared-types/src/generated/`. Nếu `cargo test -p domain` và `pnpm --filter @uxui-os/shared-types typecheck` lệch nhau dù chỉ 1 trường, CI phải fail ngay lập tức.

### 1.3 `Data-Architect` (16 SQLite WAL Tables & 2-Way File Sync Loop Hazard)
> **Pushback kỹ thuật (`CRITICAL`):** Trong `Story 01.2`, chúng ta khởi tạo toàn bộ **16 bảng SQLite WAL** (phục vụ cả 5 Stream từ `Stream-1` đến `Stream-5` theo `2.2. database-spec.md`) để không bao giờ xảy ra xung đột schema migration khi chạy song song các Wave sau. Tuy nhiên, có **2 bẫy kỹ thuật cực kỳ nguy hiểm** khi kết hợp SQLite WAL với `notify` file watcher:
> 1. **Infinite Watcher Echo Loop (Vòng lặp vô tận Disk $\leftrightarrow$ DB):** Khi `crates/spec-registry` ghi file `product/specs/01-product-overview.md` xuống đĩa, `notify` watcher của hệ điều hành (FSEvents trên macOS) sẽ bắn sự kiện `Modify(Data)` ngược lại sau vài mili-giây. Nếu watcher không đối chiếu `content_sha256` vừa ghi trong bộ nhớ đệm (`in_flight_writes` hoặc cột `content_sha256` trong bảng `PROJECT_SPECS`), nó sẽ kích hoạt lệnh ghi ngược vào SQLite và tạo vòng lặp vô tận.
> 2. **SQLITE_BUSY dưới tải đa luồng:** Khi `cowok` điều phối 3-4 Execution Bots cùng đọc/ghi trạng thái, nếu chỉ bật `PRAGMA journal_mode = WAL;` mà quên đặt `PRAGMA busy_timeout = 5000; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;` trên mọi kết nối `rusqlite`, các lệnh ghi đồng thời sẽ bắn lỗi `SQLITE_BUSY` ngay lập tức.

### 1.4 `Review` (Zero-Trust Security & `StaleFile` OCC)
> **Adversarial Challenge:** Tôi bổ sung 2 lỗ hổng bảo mật và toàn vẹn dữ liệu phải chặn ngay ở `Epic 01`:
> 1. **Lost Update giữa Human và Bot (`StaleFile` OCC):** Giả sử Human đang sửa `04-design-direction.md` trên UI, cùng lúc `spec-writer` bot cũng gửi lệnh cập nhật file đó. Nếu API `PUT /api/v1/projects/:id/specs/:spec_key` không bắt buộc truyền `expected_sha256`, người ghi sau sẽ xóa sạch công sức của người ghi trước. Bắt buộc phải trả về `409 Conflict` kèm mã lỗi chuẩn `STALE_FILE` (`actual_sha256` vs `expected_sha256`).
> 2. **Path Traversal qua `spec_key` hoặc `seed_repo_path`:** Nếu request truyền `spec_key = "../../etc/passwd"` hoặc symlink trỏ ra ngoài thư mục dự án, `crates/spec-registry` phải chặn cứng bằng kiểm tra whitelist 9 khóa chuẩn (`01-product-overview` .. `09-data-and-copy`) kết hợp `canonicalize()` + `O_NOFOLLOW`.

### 1.5 `AI-Engineer` (Context Diet cho `cowok` Orchestrator & `advisor` Meta-Agent)
> **Pushback về Agent Ops:** Khi `Story 01.3` tổng hợp 9 Foundational Specs, nếu mỗi lần `cowok` hoặc `advisor` (Dr. Eggbot) cần kiểm tra trạng thái dự án mà phải đọc toàn văn cả 9 file Markdown (có thể dài >40KB), ngân sách context sẽ bị phình to không cần thiết. Bảng `PROJECT_SPECS` trong SQLite phải lưu sẵn `summary_digest`, `provenance_counts_json` (`{"confirmed": 12, "inferred": 4, "missing": 1}`), và `content_sha256` để `cowok` có thể truy vấn metadata với **0 token LLM (`LANE_1_DETERMINISTIC`)** dưới `2ms`, chỉ nạp nội dung file thực sự cần thiết.

---

## Round 2: Resolution, Concrete Contracts & Convergence

### 2.1 `Data-Architect` & `Architect` — Chốt Cơ Chế Chống Echo Loop & 16-Table Genesis
- **Giải pháp đã thống nhất cho `Story 01.2`:**
  1. **Hash-Gated Watcher Suppression:** Trước khi `notify` watcher xử lý sự kiện file thay đổi trên đĩa, nó băm nhanh nội dung file bằng `sha256` và so sánh với `content_sha256` hiện tại trong bảng `PROJECT_SPECS`. Nếu hai mã băm trùng khớp (`disk_sha256 == db_sha256`), sự kiện được bỏ qua ngay lập tức (`NO-OP`), triệt tiêu 100% vòng lặp echo mà không cần timer phức tạp.
  2. **Atomic Temp-File Rename:** Mọi thao tác ghi từ `crates/spec-registry` xuống `product/specs/<spec_key>.md` đều ghi vào `.tmp.<uuid>` cùng thư mục, gọi `sync_all()`, sau đó mới `std::fs::rename()` nguyên tử.
  3. **Connection Pragmas Chuẩn Hóa:** Mọi kết nối `rusqlite` khởi tạo qua `SqlitePool::open_wal()` đều thực thi:
     ```sql
     PRAGMA journal_mode = WAL;
     PRAGMA synchronous = NORMAL;
     PRAGMA busy_timeout = 5000;
     PRAGMA foreign_keys = ON;
     ```

### 2.2 `Review` & `PM` — Chốt Hợp Đồng `StaleFile` OCC & 9-Spec Whitelist
- **Giải pháp đã thống nhất cho `Story 01.1`, `01.2`, `01.3`:**
  1. Định nghĩa enum `CanonicalSpecKey` cố định đúng 9 giá trị trong `crates/domain`:
     - `ProductOverview` (`01-product-overview.md`)
     - `UserPersonasAndJtbds` (`02-user-personas-and-jtbds.md`)
     - `InformationArchitecture` (`03-information-architecture.md`)
     - `DesignDirection` (`04-design-direction.md`)
     - `ComponentInventory` (`05-component-inventory.md`)
     - `InteractionPatterns` (`06-interaction-patterns.md`)
     - `ResponsiveAndA11y` (`07-responsive-and-a11y.md`)
     - `BrandGuidelines` (`08-brand-guidelines.md`)
     - `DataAndCopy` (`09-data-and-copy.md`)
     Mọi `spec_key` nằm ngoài enum này bị từ chối ngay ở tầng serde/Axum (`422 INVALID_SPEC_KEY`), loại bỏ hoàn toàn nguy cơ Path Traversal khi ghi spec.
  2. Mọi lệnh cập nhật spec (`update_spec`) yêu cầu `expected_sha256: Option<String>` (chỉ `None` khi khởi tạo lần đầu `version == 0`). Nếu lệch mã băm, trả về `HarnessError::StaleFile { spec_key, expected_sha256, actual_sha256 }` (HTTP `409 Conflict`).

### 2.3 `AI-Engineer` — Chốt Cấu Trúc Provenance & Cross-Project Brand Reuse
- **Giải pháp đã thống nhất cho `Story 01.3`:**
  1. Pipeline tổng hợp 9 Foundational Specs trong `crates/harness-daemon` phân tách rõ từng mục thông tin kèm thẻ `[CONFIRMED]`, `[INFERRED]`, hoặc `[MISSING]` và lưu thống kê vào cột `provenance_summary_json` của bảng `PROJECT_SPECS`.
  2. Hỗ trợ tham số `inherit_brand_from_project_id: Option<String>` khi tạo dự án mới (`POST /api/v1/projects`), tự động sao chép có kiểm chứng phiên bản (`SPEC_REVISIONS`) của `04-design-direction.md` và `08-brand-guidelines.md` sang dự án mới để các đội UI/UX, Marketing và Presentation dùng chung một chuẩn thương hiệu (`FR-12`).

---

## Final Party Mode Consensus Verdict
- **Consensus Status:** `CONVERGED (5/5 Roles Approved)`
- **Unresolved Architectural Conflicts:** `0`
- **Story Decomposition Readiness:** `Story 01.1` $\rightarrow$ `Story 01.2` $\rightarrow$ `Story 01.3` (`max_phase: "IMPLEMENT"`).
