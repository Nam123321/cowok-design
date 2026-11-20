---
type: I-Wish UI & Host Contract Spec
story_id: "04.6"
title: "UI Specification — SQLite Chat Persistence, CAS Intake & In-Chat Key Vault: Story 04.6"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: ChatPrimitivesBridge / LaunchpadPanels
navigation_entry: Primary Chat Composer, Attachment Dropzone & In-Chat Vault Cards
companion_surfaces:
  ai_chat_drawer: true
  command_palette: false
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='chat-composer-input']"
  - "[data-ff-id='cas-file-dropzone']"
  - "[data-ff-id='decision-engine-onboarding-card']"
  - "[data-ff-id='vault-receipt-card']"
colocated_slice_dir: src/features/chat-vault
registration_mode: workbench_chat_stream
components:
  - AppShell [5.2] (CMP-02)
  - DecisionEngineOnboardingCard [5.29]
  - VaultReceiptCard [5.15]
  - Toast [5.12]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 04.6: Persistent Chat, CAS Dropzone & Zero-Leak Vault Cards

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-02 -> Epic 04 -> Story 04.6 -> ChatPrimitivesBridge -> DecisionEngineOnboardingCard / VaultReceiptCard`

## Host Integration Contract
- **`host_portal`:** `workbenches` (Chat sidebar inside `WorkbenchShell` + `LaunchpadPanels`).
- **`shell_mode`:** `shelled` (Inline chat stream cards + binary CAS upload pill without leaving main workbench).
- **`shell_e2e_selectors`:** `[data-ff-id='chat-composer-input']`, `[data-ff-id='cas-file-dropzone']`, `[data-ff-id='decision-engine-onboarding-card']`, `[data-ff-id='vault-receipt-card']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| Persistent Chat Composer & Paste Guard | ChatPrimitivesBridge [5.4] | `src/features/chat-vault/ChatPrimitivesBridge.tsx` | EXTEND (replace `sessionStorage`, intercept `onPaste`) |
| `.csv/.tsv/.xlsx/.fig` Intake | File Attachment Pill [5.8] | `src/features/chat-vault/CasDropzonePanel.tsx` | EXTEND (`<input accept=".csv,.tsv,.xlsx,.fig,.pdf,.png,.svg">`) |
| Key Setup Card (`aistudio` / `typesafe`) | DecisionEngineOnboardingCard [5.29] | `src/features/chat-vault/DecisionEngineOnboardingCard.tsx` | NEW_REGISTRY_CARD |
| Masked Secret Receipt | VaultReceiptCard [5.15] | `src/features/chat-vault/VaultReceiptCard.tsx` | NEW_REGISTRY_CARD |

## Component Hierarchy
```text
WorkbenchShell (`CMP-02`)
└── ChatStreamPanel (`src/features/chat-vault/ChatPrimitivesBridge.tsx`)
    ├── CasDropzonePanel (`cas-file-dropzone`, `onPaste` binary capture)
    ├── DecisionEngineOnboardingCard (`decision-engine-onboarding-card`, guide links)
    ├── VaultReceiptCard (`vault-receipt-card`, Presenter Mask + 429 pill + [Replace/Revoke])
    └── ChatComposerForm (`chat-composer-input`, Pre-Persistence Secret Interceptor)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| Vault Receipt Border | `--border-vault-verified` | `1px solid var(--accent-success)` | `VaultReceiptCard` |
| Presenter Mask Blur | `--privacy-mask-filter` | `blur(4px)` on hover-reveal | Mask API prefix when presenting |
| Pill `429` Alert Color | `--status-warn` | `var(--amber-500)` | Rate-limit status badge |
| Touch Target Minimum | `--hit-target-min` | `44px` | `[Replace/Revoke]` & Lock buttons |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Paste API key into chat | `rawSecretEnvelope` matched client-side | Intercepted BEFORE `V4` insert -> `POST /api/v1/vault/chat-ingest` | `0%` secret in SQLite/React state; renders `VaultReceiptCard` |
| Drag/Paste `.csv/.fig` | File size `<= 25MB` & valid magic-byte | `e.preventDefault()` -> `POST /api/v1/cas/upload` | Blob stored at `.cowok-design/cas/<sha256>`, no base64 in textarea |
| Close & reopen tab | Session saved in `V4` | `GET /api/v1/chat-sessions/:sid/messages` | `100%` messages & receipts restored |
