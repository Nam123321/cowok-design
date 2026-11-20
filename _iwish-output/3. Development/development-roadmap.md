# Lộ Trình Sản Phẩm Toàn Diện & Ma Trận Phụ Thuộc Thực Thi: cowok-design (Open-Source Agent-Native Design & Presentation Harness OS)
*(Comprehensive Product Phased Roadmap & Dependency Matrix)*

> **Phiên bản:** v3.0-Universal  
> **Ngày cập nhật:** 2026-11-20  
> **Trạng thái:** Active SSOT Baseline (Folder `3. Development`)  
> **Bản thể dự án (Archetypes):** universal-hybrid-saas  
> **Ngân sách Worktree đồng thời:** $\le 5$ song song (Tuân thủ trần Worktree Quota $\le 5$)  
> **Hợp đồng Macro-to-Micro Lock:** `_iwish-output/_state/roadmap-lock.json`

---

## 1. Tổng Quan Hiện Trạng & Điểm Tựa Nghiệp Vụ (Maturity Baseline vs. Active Delta)

Hệ thống tính toán lộ trình dựa trên nguyên tắc **Mỏ neo Bậc vào bằng 0 (In-Degree = 0 Anchors)**: các Epics đã đạt trạng thái `completed` được xác lập làm **Maturity Baseline** đóng băng, không tái lập lịch, đóng vai trò là nền tảng cung cấp thực thể dữ liệu sạch cho các Epics đang triển khai (`Active Delta`).

### 1.1. Bảng Điểm Tựa Nền Tảng Đã Hoàn Thành (Maturity Baseline)

| STT | Mã Epic & Tên Module | Trạng Thái | Thực Thể Dữ Liệu & Hợp Đồng Đã Sẵn Sàng Cung Cấp |
| :---: | :--- | :---: | :--- |
| **Epic 01** | Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry | `3` stories | `completed` | Zero-dependency baseline anchor |
| **Epic 03** | Project-Scoped Configurable Constraint Engine & <1ms data-ff-id Patcher | `3` stories | `completed` | Zero-dependency baseline anchor |

### 1.2. Thống Kê Active Delta Cần Triển Khai
- **Tổng số Epics dự án:** 13
- **Epics đã hoàn thành (Maturity Baseline):** 2
- **Epics đang triển khai / Backlog:** 11
- **Số làn sóng phát triển (Waves):** 4 Waves
- **Số luồng song song (Parallel Tracks):** 5 Tracks

---

## 2. Ma Trận Cung Cấp - Tiêu Thụ & Phân Tích Đồ Thị Khối (Provide-Consume Matrix & Dependency DAG)

### 2.1. Bảng Ma Trận Phụ Thuộc Chi Tiết Cấp Epic & Sub-Epic Phase Slices

| Epic ID / Slice | Tên Khối Nghiệp Vụ | Cung Cấp (Provides - Entities & Contracts) | Tiêu Thụ (Consumes - From Epics) | Độ Toàn Vẹn Tiêu Thụ & Rủi Ro Nghẽn (Blocker Risk) |
| :--- | :--- | :--- | :--- | :--- |
| **Epic 02** | JIT Prompt Assembler, KV-Cache Compiler, Hidden SSOT & cowok-design-opt 7 Craft Skills | `Stream-1` | `Epic 05`, `Epic 09`, `Epic 11` | `Epic 01` |
| **Epic 04** | Flow-First 2D SVG Bezier DAG Studio, Security Kernel, CAS Intake & In-Chat Vault | `Stream-2` | `Epic 10` | `Epic 01` |
| **Epic 05** | Option 5 2D Storyboard, Cognitive Sketch-to-Polish, Shared lol_html & 3-Tier Auto Flow DAG | `Stream-3` | `Epic 07` | `Epic 02` |
| **Epic 06** | Dual-Pane 16:9 HTML Presentation, Non-Tech CMS Workbench & AI Multi-Slide Deck Gen | `Stream-4` | *(Không có)* | `Epic 03` |
| **Epic 07** | Pooled 1-Chromium CDP Visual QC & Split Share vs Developer Handoff Exporter | `Stream-5` | `Epic 12`, `Epic 13` | `Epic 05` |
| **Epic 08** | agentskills.io Repackager, Split DE+DR Runtime, 7 Built-in DE Tools, Controlled Tool Jail & LEARN | `Stream-1` | *(Không có)* | `Epic 01` |
| **Epic 09** | Two-Store /rea DNA Forge & 5-Stage Rust DecisionCascadeClient (RubricShield) | `Stream-2` | *(Không có)* | `Epic 02` |
| **Epic 10** | cowok Master Orchestrator, File Leases, Builder!=Scorer & CMP-07 Design Tournament | `Stream-3` | *(Không có)* | `Epic 04` |
| **Epic 11** | POSIX BYOS/BYOK Runner (0o600 FD + -PGID), Cross-CLI Normalizer & Universal Open MCP Gateway | `Stream-4` | *(Không có)* | `Epic 02` |
| **Epic 12** | 4-Channel Packaging, Standalone /api/* Parity & OCI Distribution | `Stream-5` | *(Không có)* | `Epic 07` |
| **Epic 13** | End-to-End Sealed Anti-Goodhart Benchmark & 25-Case /simulate-user Certification Suite | `Stream-1` | *(Không có)* | `Epic 07` |

### 2.2. Sơ Đồ Đồ Thị Khối Liên Kết Phụ Thuộc (Macro-DAG Flowchart)

```mermaid
flowchart TD
flowchart TD
    E01["Epic 01: Cargo Workspace, Domain Contracts ts-rs & 16-Table (Completed)"]
    E03["Epic 03: Project-Scoped Configurable Constraint Engine & <1 (Completed)"]
    E02["Epic 02: JIT Prompt Assembler, KV-Cache Compiler, Hidden SS"]
    E04["Epic 04: Flow-First 2D SVG Bezier DAG Studio, Security Kern"]
    E05["Epic 05: Option 5 2D Storyboard, Cognitive Sketch-to-Polish"]
    E06["Epic 06: Dual-Pane 16:9 HTML Presentation, Non-Tech CMS Wor"]
    E07["Epic 07: Pooled 1-Chromium CDP Visual QC & Split Share vs D"]
    E08["Epic 08: agentskills.io Repackager, Split DE+DR Runtime, 7 "]
    E09["Epic 09: Two-Store /rea DNA Forge & 5-Stage Rust DecisionCa"]
    E10["Epic 10: cowok Master Orchestrator, File Leases, Builder!=S"]
    E11["Epic 11: POSIX BYOS/BYOK Runner 0o600 FD + -PGID, Cross-CLI"]
    E12["Epic 12: 4-Channel Packaging, Standalone /api/* Parity & OC"]
    E13["Epic 13: End-to-End Sealed Anti-Goodhart Benchmark & 25-Cas"]
    E01 --> E02
    E01 --> E04
    E02 --> E05
    E03 --> E06
    E05 --> E07
    E01 --> E08
    E02 --> E09
    E04 --> E10
    E02 --> E11
    E07 --> E12
    E07 --> E13
```

---

## 3. Các Nguyên Tắc Logic Bất Biến Của Dự Án (Core Engineering & Domain Invariants)

Thứ tự ưu tiên và phân bổ Wave không chỉ dựa vào phụ thuộc kỹ thuật thuần túy mà bắt buộc phải tuân thủ nghiêm ngặt **Các Nguyên Tắc Bất Biến Miền Nghiệp Vụ (Domain Invariants)** để loại bỏ rủi ro tài chính, vận hành và phân mảnh kiến trúc:

- Không có invariant cụ thể.

---

## 4. Phân Tích Khả Năng Triển Khai Song Song (Parallel Execution Tracks)

Để tối ưu hóa thời gian bàn giao (Time-to-Market) mà không gây xung đột mã nguồn (Merge Conflicts trên `schema.prisma`) hoặc vượt trần tài nguyên ($\le 5$ worktrees), toàn bộ danh mục Epics được phân rã thành **5 Luồng triển khai song song độc lập**:

### 4.1. Bảng Định Nghĩa 5 Parallel Tracks & Phạm Vi Cô Lập Bảng Dữ Liệu

| Mã Luồng | Tên Luồng & Phạm Vi Chuyên Biệt | Danh Mục Bảng DB Sở Hữu Độc Quyền (Disjoint Write-Set) | Ngân Sách Worktree ($\le 4$) |
| :--- | :--- | :--- | :---: |
| **Stream-1** | VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine (VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine) | `domain_stream-1:CoreTables`, `domain_stream-1:LedgerTables` | `1` worktree |
| **Stream-2** | VS-02. Flow-First Executable Design Workbench & SDI Engine (VS-02. Flow-First Executable Design Workbench & SDI Engine) | `domain_stream-2:CoreTables`, `domain_stream-2:LedgerTables` | `1` worktree |
| **Stream-3** | VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio (VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio) | `domain_stream-3:CoreTables`, `domain_stream-3:LedgerTables` | `1` worktree |
| **Stream-4** | VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe (VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe) | `domain_stream-4:CoreTables`, `domain_stream-4:LedgerTables` | `1` worktree |
| **Stream-5** | VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution (VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution) | `domain_stream-5:CoreTables`, `domain_stream-5:LedgerTables` | `1` worktree |

### 4.2. Ranh Giới Hợp Đồng Đóng Băng (Contract-First Isolation Boundary)
- **Cổng Đồng Bộ Cấp Horizon/Wave (Contract-First & Sync Barrier):** Mọi Track/Stream trong Wave hiện tại phải hoàn tất 100% các bước nhân quả và vượt qua Sync Barrier (`git merge` + đóng băng Schema + E2E Gate) trước khi mở khóa Wave kế tiếp.
- **Cô Lập Stream Song Song & Khóa Schema (Phase 0 Schema Seal Gate):** Các Streams chạy song song trên các Worktrees độc lập, không ghi đè chéo schema. Bất kỳ bảng dùng chung nào phải được khóa ở Phase 0 Frozen Baseline.

---

## 5. Lộ Trình Phát Triển Toàn Diện Theo Làn Sóng (Multi-Wave Phased Master Plan)

### 5.1. Bảng Phân Kỳ Chi Tiết 4 Làn Sóng (Wave 1 $\rightarrow$ Wave 4)

| Làn Sóng | Tên Làn Sóng & Mục Tiêu Trọng Tâm | Danh Mục Epics & Sub-Epic Phase Slices | Lát Cắt Giá Trị Dọc Bàn Giao (Vertical Value Slice) | Điều Kiện Tiền Đề Mở Khóa (Unblock Condition) |
| :---: | :--- | :--- | :--- | :--- |
| **Wave 1** | Wave 1: Vertical Value Increment 1 | `Epic 02`, `Epic 04`, `Epic 06`, `Epic 08` | Delivery of core capabilities for Wave 1 | Baseline hoàn tất |
| **Wave 2** | Wave 2: Vertical Value Increment 2 | `Epic 05`, `Epic 09`, `Epic 11`, `Epic 10` | Delivery of core capabilities for Wave 2 | Wave 1 Sync Barrier hoàn tất |
| **Wave 3** | Wave 3: Vertical Value Increment 3 | `Epic 07` | Delivery of core capabilities for Wave 3 | Wave 2 Sync Barrier hoàn tất |
| **Wave 4** | Wave 4: Vertical Value Increment 4 | `Epic 12`, `Epic 13` | Delivery of core capabilities for Wave 4 | Wave 3 Sync Barrier hoàn tất |

### 5.2. Biểu Đồ Gantt Tiến Độ Thực Thi Đa Làn Sóng (Multi-Wave Phased Gantt)

```mermaid
gantt
    title Lộ Trình Triển Khai Sản Phẩm Theo Làn Sóng & Luồng Song Song (SSOT)
    dateFormat  YYYY-MM-DD
    axisFormat  %d/%m
    excludes    weekends

    section Wave 1
    Epic 02 JIT Prompt Assembler, KV-Cache Compiler, Hidden SS :active, e02, 2026-10-22, 10d
    Epic 04 Flow-First 2D SVG Bezier DAG Studio, Security Kern :active, e04, 2026-10-22, 10d
    Epic 06 Dual-Pane 16:9 HTML Presentation, Non-Tech CMS Wor :active, e06, 2026-10-22, 10d
    Epic 08 agentskills.io Repackager, Split DE+DR Runtime, 7  :active, e08, 2026-10-22, 10d
    section Wave 2
    Epic 05 Option 5 2D Storyboard, Cognitive Sketch-to-Polish : e05, 2026-10-22, 10d
    Epic 09 Two-Store /rea DNA Forge & 5-Stage Rust DecisionCa : e09, 2026-10-22, 10d
    Epic 11 POSIX BYOS/BYOK Runner 0o600 FD + -PGID, Cross-CLI : e11, 2026-10-22, 10d
    Epic 10 cowok Master Orchestrator, File Leases, Builder!=S : e10, 2026-10-22, 10d
    section Wave 3
    Epic 07 Pooled 1-Chromium CDP Visual QC & Split Share vs D : e07, 2026-10-22, 10d
    section Wave 4
    Epic 12 4-Channel Packaging, Standalone /api/* Parity & OC : e12, 2026-10-22, 10d
    Epic 13 End-to-End Sealed Anti-Goodhart Benchmark & 25-Cas : e13, 2026-10-22, 10d
```

---

## 6. Kế Hoạch Hành Động Tức Thời Cho Sprint Hiện Tại (Immediate P0 Action Slices)

Từ Wave hiện tại, hệ thống lọc ra các **Lát Cắt Hành Động P0 Tức Thời (Immediate P0 Action Slices)** có bậc vào bằng 0 (`In-Degree == 0`), đã sẵn sàng 100% tài liệu tiền đề và có thể tiến hành lập Spec Stack hoặc code ngay lập tức:

| Mã Lát Cắt | Module / Epic Trọng Tâm | Danh Sách Story IDs Triển Khai Ngay | Luồng (Track) | Lý Do Kích Hoạt Tức Thời (Activation Rationale) |
| :---: | :--- | :--- | :---: | :--- |
| **P0.1** | **Epic 02**: JIT Prompt Assembler, KV-Cache Compiler, Hidden SSOT & cowok-design-opt 7 Craft Skills (Immediate P0 Anchor) | `02.1`, `02.2`, `02.3` | `Stream-1` | Zero blocker risk anchor for Wave 1 unblocking downstream business workflows. |
| **P0.2** | **Epic 04**: Flow-First 2D SVG Bezier DAG Studio, Security Kernel, CAS Intake & In-Chat Vault (Immediate P0 Anchor) | `04.1`, `04.2`, `04.3` | `Stream-2` | Zero blocker risk anchor for Wave 1 unblocking downstream business workflows. |
| **P0.3** | **Epic 06**: Dual-Pane 16:9 HTML Presentation, Non-Tech CMS Workbench & AI Multi-Slide Deck Gen (Immediate P0 Anchor) | `06.1`, `06.2`, `06.3` | `Stream-4` | Zero blocker risk anchor for Wave 1 unblocking downstream business workflows. |
| **P0.4** | **Epic 08**: agentskills.io Repackager, Split DE+DR Runtime, 7 Built-in DE Tools, Controlled Tool Jail & LEARN (Immediate P0 Anchor) | `08.1`, `08.2`, `08.3` | `Stream-1` | Zero blocker risk anchor for Wave 1 unblocking downstream business workflows. |

---

> [!NOTE]
> Bản roadmap này được bảo vệ bởi cổng kiểm định **Category A Zero-Trust** (`validate-product-roadmap.py`). Khi có bất kỳ thay đổi kiến trúc hoặc khi một Epic hoàn thành tại `/iwish-feature-retrospective`, các Auto-Trigger Hooks sẽ tự động cập nhật và duy trì tính nhất quán tuyệt đối.
