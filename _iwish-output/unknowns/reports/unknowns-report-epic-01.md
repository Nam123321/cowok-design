# 4-Quadrant Unknowns & Risk Discovery Report — Epic 01

- **Epic ID:** `Epic-01`
- **Title:** `Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry`
- **Evaluated By:** `Unknowns Analyst` + `Review Agent (Edge Case Guardian)`
- **Status:** `RESOLVED / MITIGATED IN STORY SPECS`

---

## Quadrant 1: Known Unknowns (`known_unknowns`)
*Những biến số kỹ thuật đã biết trước cần kiểm chứng và khóa chặt bằng bài test thực tế trong quá trình triển khai.*

| ID | Risk / Question | Impact | Concrete Mitigation Bound into Epic 01 Stories |
| :--- | :--- | :---: | :--- |
| **KU-01.1** | `ts-rs` xuất kiểu cho các enum phân biệt (`#[serde(tag = "kind", content = "payload")]`) và kiểu `u64`/`i64` sang TypeScript (`bigint` vs `number`) có tương thích trực tiếp với JSON `serde` và Zod không? | `HIGH` | Trong **Story 01.1**, dùng `#[ts(type = "number")]` cho các trường kích thước/timestamp vừa khung `MAX_SAFE_INTEGER` hoặc chuỗi ISO-8601 cho thời gian, kèm unit test round-trip JSON giữa Rust `serde_json` và TypeScript Zod validator. |
| **KU-01.2** | Thời gian khởi tạo 16 bảng SQLite WAL kèm chỉ mục (indexes) qua `refinery` trên máy người dùng khi khởi động lạnh (cold start) mất bao lâu? | `MEDIUM` | Trong **Story 01.2**, gộp toàn bộ DDL khởi tạo 16 bảng vào một migration transaction duy nhất (`V1__initial_16_tables.sql`), cam kết thời gian chạy `< 15ms` trên ổ SSD tiêu chuẩn. |

---

## Quadrant 2: Unknown Unknowns (`unknown_unknowns`)
*Những hiện tượng ranh giới hệ điều hành và hệ thống file phát sinh ngoài dự tính thông thường.*

| ID | Hidden Failure Mode | Severity | Concrete Prevention Mechanism |
| :--- | :--- | :---: | :--- |
| **UU-01.1** | **macOS FSEvents Coalescing & Delayed Echo:** Trên macOS, `notify` watcher có thể gộp nhiều sự kiện sửa file trong khoảng `50-200ms` hoặc bắn sự kiện trễ sau khi transaction SQLite đã đóng, gây ghi đè ngược (echo loop). | `CRITICAL` | Trong **Story 01.2**, áp dụng **Hash-Gated Watcher Suppression**: `notify` handler luôn tính `sha256(disk_bytes)` và so sánh với `PROJECT_SPECS.content_sha256` trong SQLite; nếu trùng khớp thì bỏ qua ngay (`NO-OP`). |
| **UU-01.2** | **Partial Read khi Editor bên ngoài đang lưu file:** Khi người dùng mở file `product/specs/04-design-direction.md` bằng VS Code/Cursor và bấm Save, trình soạn thảo có thể truncate file về `0 bytes` trong vài micro-giây trước khi flush nội dung mới. | `HIGH` | Trong **Story 01.2**, nếu watcher phát hiện file `0 bytes` đột ngột đối với spec đã có nội dung trước đó, thực hiện debounce `50ms` và đọc lại trước khi đồng bộ vào SQLite; từ chối ghi đè spec rỗng nếu không có cờ xóa tường minh. |

---

## Quadrant 3: Assumptions (`assumptions`)
*Các giả định kiến trúc cần được biến thành điều kiện kiểm tra tường minh (Fail-Closed Assertions).*

| ID | Architectural Assumption | Verification / Fail-Closed Guard |
| :--- | :--- | :--- |
| **AS-01.1** | Giả định cổng nội bộ `127.0.0.1:4170` của `crates/harness-daemon` luôn rảnh khi khởi động ứng dụng. | Trong **Story 01.3**, `harness-daemon` hỗ trợ cờ `--port` / biến môi trường `UXUI_OS_DAEMON_PORT` và khả năng bind cổng động (`127.0.0.1:0`) ghi file discovery `.uxui-os/daemon.lock` để UI Shell và MCP Gateway tự động kết nối mà không chết cứng ở cổng `4170`. |
| **AS-01.2** | Giả định thư mục dự án (`workspace_root`) không chứa symlink độc hại trỏ ra ngoài hệ thống file của người dùng. | Trong **Story 01.2 & 01.3**, mọi đường dẫn file dưới `product/specs/` đều phải qua bộ kiểm tra `canonicalize()` và xác nhận nằm gọn trong `workspace_root` trước khi đọc/ghi. |

---

## Quadrant 4: Blind Spots (`blind_spots`)
*Các điểm mù giữa giao diện Human, đội quân Execution Bot và cơ chế tái sử dụng đa dự án.*

| ID | Blind Spot | Resolution Bound into Epic 01 |
| :--- | :--- | :--- |
| **BS-01.1** | **Xung đột kế thừa Brand Guideline giữa các dự án (`Cross-Project Brand Drift`):** Khi dự án Marketing HTML Deck kế thừa `08-brand-guidelines.md` từ dự án Product UI/UX, nếu dự án gốc cập nhật guideline thì dự án con không biết mình đang dùng bản cũ. | Trong **Story 01.2 & 01.3**, bảng `PROJECT_SPECS` và `SPEC_REVISIONS` lưu trữ `source_project_id` và `source_revision_sha256` để `cowok` và UI Shell hiển thị cảnh báo *"Upstream Brand Guideline Updated"* cho phép đồng bộ có kiểm soát (`FR-12`). |
| **BS-01.2** | **Thiếu phân biệt giữa dữ liệu người dùng xác nhận và dữ liệu AI suy luận trong 9 Specs:** Nếu lưu chung thành văn bản thuần, các bot hạ nguồn (`ui-builder`, `audit-guard`) sẽ coi thông tin suy luận tạm thời là luật cứng. | Trong **Story 01.1 & 01.3**, chuẩn hóa cấu trúc `SpecSectionProvenance` (`CONFIRMED`, `INFERRED`, `MISSING`) ngay trong `crates/domain` và cột `provenance_summary_json` của bảng `PROJECT_SPECS`. |
