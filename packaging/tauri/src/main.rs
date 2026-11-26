// @story 12.2
// @implements AC-01, AC-04 (Tauri v2 Desktop Sidecar Lifecycle, Rust-Main-Mediated Folder Picker & Drag-and-Drop HMAC Capability Preload Bridge, Compile-Time Pinned Ed25519 Updater Gate)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use harness_daemon::routes::distribution_ops_crypto::{
    decode_verifying_key_base64, SignedManifestEnvelope,
};
use harness_daemon::tauri_trust::{
    verify_sidecar_and_updater_integrity, CapabilityScope, CapabilityVerifyOptions,
    HmacCapabilityToken, SidecarIntegrityManifest, UpdaterSignatureReceipt, VerifyingKey,
    WorkspaceCapabilityRegistry,
};
use serde::{Deserialize, Serialize};
use tauri::{DragDropEvent, Emitter, Manager, WindowEvent};
use tauri_plugin_dialog::DialogExt;

const PLACEHOLDER_UPDATER_PUBKEY: &str = "__COWOK_RELEASE_ED25519_PUBKEY_INJECTED_AT_BUILD__";
const PINNED_UPDATER_PUBKEY_B64: Option<&str> = option_env!("COWOK_RELEASE_ED25519_PUBKEY_B64");
const MAX_DROPPED_CAPABILITY_HISTORY: usize = 128;
const DROPPED_CAPABILITIES_EVENT: &str = "cowok://dropped-capabilities";

fn load_pinned_updater_verifying_key() -> Option<VerifyingKey> {
    let raw = PINNED_UPDATER_PUBKEY_B64?.trim();
    if raw.is_empty() || raw == PLACEHOLDER_UPDATER_PUBKEY || raw.contains("PRIVATE KEY") {
        return None;
    }
    decode_verifying_key_base64(raw).ok()
}

/// Desktop runtime state owned strictly by the Rust main process.
/// Raw host filesystem paths reside exclusively inside `capability_registry` and
/// are never serialized across IPC to the untrusted WebView.
/// The trusted Ed25519 updater verifying key is compile-time pinned and never
/// accepted from WebView IPC.
pub struct DesktopHostState {
    pub capability_registry: Arc<WorkspaceCapabilityRegistry>,
    pub sidecar_loopback_url: Mutex<Option<String>>,
    pub last_applied_updater_seq: Mutex<i64>,
    pub dropped_capabilities: Mutex<Vec<HmacCapabilityToken>>,
    pub pinned_updater_verifying_key: Option<VerifyingKey>,
}

impl Default for DesktopHostState {
    fn default() -> Self {
        Self {
            capability_registry: Arc::new(WorkspaceCapabilityRegistry::new()),
            sidecar_loopback_url: Mutex::new(None),
            last_applied_updater_seq: Mutex::new(0),
            dropped_capabilities: Mutex::new(Vec::new()),
            pinned_updater_verifying_key: load_pinned_updater_verifying_key(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBootStatus {
    pub channel: &'static str,
    pub sidecar_url: Option<String>,
    pub drag_drop_enabled: bool,
    pub support_drawer_enabled: bool,
    pub updater_verification: &'static str,
    pub updater_pubkey_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportDrawerDesktopPayload {
    pub channel: &'static str,
    pub app_version: &'static str,
    pub workspace_handle_id: Option<String>,
    pub workspace_label: Option<String>,
    pub pii_scrubbed: bool,
}

/// Opens the native OS folder picker strictly inside Rust main and mints a least-privilege
/// `WorkspaceRead` opaque HMAC capability token. WebView NEVER supplies a raw host path or
/// chooses an elevated scope; only the opaque `HmacCapabilityToken` crosses IPC.
#[tauri::command]
fn pick_workspace_capability(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopHostState>,
) -> Result<Option<HmacCapabilityToken>, String> {
    let Some(picked_folder) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    let selected_path = picked_folder
        .into_path()
        .map_err(|_| "UNSUPPORTED_FOLDER_SELECTION".to_string())?;
    let now_unix = Utc::now().timestamp();
    let token = state
        .capability_registry
        .issue_capability(
            &selected_path,
            CapabilityScope::WorkspaceRead,
            None,
            now_unix,
        )
        .map_err(|err| err.webview_safe_message())?;
    Ok(Some(token))
}

/// Returns active opaque `HmacCapabilityToken` handles minted by Rust-main OS drag-and-drop
/// window events (zero raw filesystem paths ever cross IPC).
#[tauri::command]
fn list_dropped_capabilities(
    state: tauri::State<'_, DesktopHostState>,
) -> Result<Vec<HmacCapabilityToken>, String> {
    let now_unix = Utc::now().timestamp();
    let mut guard = state
        .dropped_capabilities
        .lock()
        .map_err(|_| "DESKTOP_STATE_LOCK_POISONED".to_string())?;
    guard.retain(|tok| tok.expires_at > now_unix);
    Ok(guard.clone())
}

/// Verifies an opaque capability token and returns a Zero-PII payload for the In-App Support Drawer.
#[tauri::command]
fn open_support_drawer_payload(
    state: tauri::State<'_, DesktopHostState>,
    capability_token: Option<HmacCapabilityToken>,
) -> Result<SupportDrawerDesktopPayload, String> {
    let (handle_id, label) = if let Some(token) = capability_token {
        let now_unix = Utc::now().timestamp();
        let _resolved: PathBuf = state
            .capability_registry
            .verify_and_resolve(
                &token,
                &CapabilityVerifyOptions {
                    required_scope: CapabilityScope::SupportDiagnostics,
                    single_use: false,
                    now_unix,
                },
            )
            .map_err(|err| err.webview_safe_message())?;
        (Some(token.handle_id), Some(token.display_label))
    } else {
        (None, None)
    };

    Ok(SupportDrawerDesktopPayload {
        channel: "desktop",
        app_version: env!("CARGO_PKG_VERSION"),
        workspace_handle_id: handle_id,
        workspace_label: label,
        pii_scrubbed: true,
    })
}

/// Revokes an active WebView capability handle immediately inside Rust main.
#[tauri::command]
fn revoke_workspace_capability(
    state: tauri::State<'_, DesktopHostState>,
    handle_id: String,
) -> bool {
    if let Ok(mut guard) = state.dropped_capabilities.lock() {
        guard.retain(|tok| tok.handle_id != handle_id);
    }
    state.capability_registry.revoke_handle(&handle_id)
}

/// Verifies a signed Ed25519 + SHA-256 sidecar or auto-updater manifest strictly inside Rust main
/// using the compile-time pinned Ed25519 verifying key (fail-closed if unset or placeholder).
#[tauri::command]
fn verify_updater_manifest(
    state: tauri::State<'_, DesktopHostState>,
    envelope: SignedManifestEnvelope<SidecarIntegrityManifest>,
    artifact_root_token: HmacCapabilityToken,
) -> Result<Vec<UpdaterSignatureReceipt>, String> {
    let verifying_key = state
        .pinned_updater_verifying_key
        .as_ref()
        .ok_or_else(|| "UPDATER_PUBKEY_NOT_PINNED".to_string())?;
    let now_unix = Utc::now().timestamp();
    let artifact_root = state
        .capability_registry
        .verify_and_resolve(
            &artifact_root_token,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::ApplyUpdate,
                single_use: true,
                now_unix,
            },
        )
        .map_err(|err| err.webview_safe_message())?;

    let mut seq_guard = state
        .last_applied_updater_seq
        .lock()
        .map_err(|_| "DESKTOP_STATE_LOCK_POISONED".to_string())?;
    let receipts = verify_sidecar_and_updater_integrity(
        &envelope,
        verifying_key,
        &artifact_root,
        *seq_guard,
        now_unix,
    )
    .map_err(|err| err.webview_safe_message())?;
    *seq_guard = envelope.manifest_seq;
    Ok(receipts)
}

#[tauri::command]
fn get_desktop_boot_status(
    state: tauri::State<'_, DesktopHostState>,
) -> Result<DesktopBootStatus, String> {
    let sidecar_url = state
        .sidecar_loopback_url
        .lock()
        .map_err(|_| "DESKTOP_STATE_LOCK_POISONED".to_string())?
        .clone();
    Ok(DesktopBootStatus {
        channel: "desktop",
        sidecar_url,
        drag_drop_enabled: true,
        support_drawer_enabled: true,
        updater_verification: "ed25519+sha256+o_nofollow+nlink1",
        updater_pubkey_pinned: state.pinned_updater_verifying_key.is_some(),
    })
}

fn handle_os_drag_drop_paths(window: &tauri::Window, paths: &[PathBuf]) {
    let state = window.state::<DesktopHostState>();
    let now_unix = Utc::now().timestamp();
    let mut minted = Vec::with_capacity(paths.len());
    for path in paths {
        if let Ok(token) = state.capability_registry.issue_capability(
            path,
            CapabilityScope::AssetImport,
            None,
            now_unix,
        ) {
            minted.push(token);
        }
    }
    if minted.is_empty() {
        return;
    }
    if let Ok(mut guard) = state.dropped_capabilities.lock() {
        guard.retain(|tok| tok.expires_at > now_unix);
        guard.extend(minted.iter().cloned());
        if guard.len() > MAX_DROPPED_CAPABILITY_HISTORY {
            let excess = guard.len() - MAX_DROPPED_CAPABILITY_HISTORY;
            guard.drain(0..excess);
        }
    }
    let _ = window.emit(DROPPED_CAPABILITIES_EVENT, &minted);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(DesktopHostState::default())
        .on_window_event(|window, event| {
            if let WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event {
                handle_os_drag_drop_paths(window, paths);
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_desktop_boot_status,
            pick_workspace_capability,
            list_dropped_capabilities,
            open_support_drawer_payload,
            revoke_workspace_capability,
            verify_updater_manifest,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cowok Design Tauri v2 desktop application");
}
