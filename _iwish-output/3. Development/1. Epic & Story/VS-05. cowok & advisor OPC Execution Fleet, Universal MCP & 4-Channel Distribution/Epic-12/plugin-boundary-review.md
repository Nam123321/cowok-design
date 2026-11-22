---
schema: plugin-boundary-review/v1
epic_id: Epic-12
feature_group: "VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution"
applicability: CANDIDATE
delivery_strategy: MULTI_CHANNEL_DISTRIBUTION_PACKS
---

# Plugin Boundary Review — Epic 12 (`12.1`, `12.2`, `12.3`)

| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `single_binary_cli_standalone_parity` | `12.1` | `KERNEL` | `KERNEL_CORE` | `uiux-os` CLI + Local Web mounting `/api/*` standalone daemon alias (`0` Vite-proxy 404s). |
| `tauri_desktop_shell` | `12.2` | `KERNEL` | `KERNEL_CORE` | Native `.dmg`/`.msi`/`.AppImage` shell wrapping `harness-daemon`. |
| `cowok_native_plugin_pack` | `12.2` | `EXTENSION` | `EXTENSION_PLUGIN` | Native plugin manifest & skill pack aligned with `Story 08.5`. |
| `docker_oci_pooled_chromium` | `12.3` | `KERNEL` | `KERNEL_CORE` | `ghcr.io/cowok-ai/uiux-os` OCI image with Pooled Headless Chromium (`<=1` proc per worker) & `ADR-006`. |
