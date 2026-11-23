// @story 09.1
// @implements AC-01, AC-02, AC-03, AC-04 (Two-Store FTS5, Firewall, SSRF, Sanitizer, Zip-Bomb, Lossless Importer & Honest Proof)
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use rea_bridge::{
    count_deterministic_tokens, evaluate_license_and_tos, extract_web_clone_l1_l6,
    sanitize_design_only_text, validate_public_url_ssrf, validate_socket_addr_ssrf, FirewallError,
    FirewallVerdict, ImporterError, ProofStatus, ReaBridgeError, ReaBridgeService,
    ReaExportProofRequest, ReaIngestRequest, ReaSearchRequest, ReaSearchTier, ReaStoreError,
    ReaTwoStore, SourceFormat, PROHIBITED_PORTS, TIER1A_MAX_TOKENS,
};
use tempfile::tempdir;

type ZipEntrySpec<'a> = (&'a str, &'a [u8], u16, u32, Option<u32>);

/// Helper to assemble a raw ZIP archive with full control over entry names, Unix mode attributes,
/// compression method, and declared sizes (for testing valid zips, symlinks, traversal, and zip bombs).
fn build_zip_bytes(entries: &[ZipEntrySpec<'_>]) -> Vec<u8> {
    let mut local_section = Vec::new();
    let mut central_dir = Vec::new();

    for &(name, data, method, external_attr, declared_uncompressed_override) in entries {
        let local_offset = local_section.len() as u32;
        let name_bytes = name.as_bytes();
        let comp_size = data.len() as u32;
        let uncomp_size = declared_uncompressed_override.unwrap_or(comp_size);

        // Local file header PK\x03\x04
        local_section.extend_from_slice(b"PK\x03\x04");
        local_section.extend_from_slice(&20u16.to_le_bytes()); // version
        local_section.extend_from_slice(&0u16.to_le_bytes()); // flags
        local_section.extend_from_slice(&method.to_le_bytes());
        local_section.extend_from_slice(&0u16.to_le_bytes()); // mod time
        local_section.extend_from_slice(&0u16.to_le_bytes()); // mod date
        local_section.extend_from_slice(&0u32.to_le_bytes()); // crc32
        local_section.extend_from_slice(&comp_size.to_le_bytes());
        local_section.extend_from_slice(&uncomp_size.to_le_bytes());
        local_section.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        local_section.extend_from_slice(&0u16.to_le_bytes()); // extra len
        local_section.extend_from_slice(name_bytes);
        local_section.extend_from_slice(data);

        // Central directory header PK\x01\x02
        central_dir.extend_from_slice(b"PK\x01\x02");
        central_dir.extend_from_slice(&0x0314u16.to_le_bytes()); // made by Unix
        central_dir.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central_dir.extend_from_slice(&0u16.to_le_bytes()); // flags
        central_dir.extend_from_slice(&method.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes()); // mod time
        central_dir.extend_from_slice(&0u16.to_le_bytes()); // mod date
        central_dir.extend_from_slice(&0u32.to_le_bytes()); // crc32
        central_dir.extend_from_slice(&comp_size.to_le_bytes());
        central_dir.extend_from_slice(&uncomp_size.to_le_bytes());
        central_dir.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes()); // extra len
        central_dir.extend_from_slice(&0u16.to_le_bytes()); // comment len
        central_dir.extend_from_slice(&0u16.to_le_bytes()); // disk start
        central_dir.extend_from_slice(&0u16.to_le_bytes()); // internal attr
        central_dir.extend_from_slice(&external_attr.to_le_bytes());
        central_dir.extend_from_slice(&local_offset.to_le_bytes());
        central_dir.extend_from_slice(name_bytes);
    }

    let cd_offset = local_section.len() as u32;
    let cd_size = central_dir.len() as u32;
    let count = entries.len() as u16;

    let mut archive = Vec::new();
    archive.extend_from_slice(&local_section);
    archive.extend_from_slice(&central_dir);
    // EOCD PK\x05\x06
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&cd_size.to_le_bytes());
    archive.extend_from_slice(&cd_offset.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive
}

#[test]
fn ac01_two_store_fts5_tenant_isolation_and_tier1a_250_token_cap() {
    let dir = tempdir().expect("tempdir");
    let svc = ReaBridgeService::open(dir.path()).expect("open service");

    let expected_db = dir.path().join(".cowok-design/cache/rea-reference.sqlite3");
    assert!(expected_db.exists(), "Two-Store SQLite cache must exist");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir_mode = std::fs::metadata(dir.path().join(".cowok-design/cache"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let db_mode = std::fs::metadata(&expected_db)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
        assert_eq!(db_mode, 0o600);
    }

    let verbose_summary = (0..420)
        .map(|i| format!("token{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let outcome = svc
        .ingest(
            "tenant-alpha",
            ReaIngestRequest {
                slug: "fintech-treasury-kit".to_string(),
                source_url: "https://github.com/open-design/fintech-kit".to_string(),
                source_format: SourceFormat::Github,
                license_spdx: "MIT".to_string(),
                summary_hint: Some(verbose_summary),
                payload_text: Some(
                    "<header class='hero'><button style='background:#0f766e;padding:12px'>Approve</button></header>"
                        .to_string(),
                ),
                payload_base64: None,
            },
        )
        .expect("ingest alpha");

    assert!(outcome.record.tier1a_tokens <= TIER1A_MAX_TOKENS);
    assert_eq!(
        count_deterministic_tokens(&outcome.record.tier1a_summary),
        outcome.record.tier1a_tokens
    );
    assert_eq!(
        outcome.record.firewall_verdict,
        FirewallVerdict::AdoptPriorArt
    );

    // Tenant B ingests its own pattern
    svc.ingest(
        "tenant-beta",
        ReaIngestRequest {
            slug: "beta-secret-kit".to_string(),
            source_url: "https://github.com/open-design/beta-kit".to_string(),
            source_format: SourceFormat::Github,
            license_spdx: "Apache-2.0".to_string(),
            summary_hint: Some("Beta exclusive fintech treasury dashboard".to_string()),
            payload_text: Some("<main><table class='ledger'></table></main>".to_string()),
            payload_base64: None,
        },
    )
    .expect("ingest beta");

    // Tier-1A search for tenant-alpha returns ONLY alpha record and omits heavy designDna
    let res_1a = svc
        .search(
            "tenant-alpha",
            ReaSearchRequest {
                q: "fintech".to_string(),
                limit: 10,
                tier: ReaSearchTier::Tier1a,
            },
        )
        .expect("search 1a");
    assert_eq!(res_1a.total, 1);
    assert_eq!(res_1a.items[0].slug, "fintech-treasury-kit");
    assert!(res_1a.items[0].design_dna.is_none());

    // Tier-1B returns full Design DNA and still enforces tenant scoping
    let res_1b = svc
        .search(
            "tenant-alpha",
            ReaSearchRequest {
                q: "treasury".to_string(),
                limit: 10,
                tier: ReaSearchTier::Tier1b,
            },
        )
        .expect("search 1b");
    assert_eq!(res_1b.total, 1);
    assert!(res_1b.items[0].design_dna.is_some());

    // Malformed FTS5 operator query never crashes SQLite
    let nasty = svc
        .search(
            "tenant-alpha",
            ReaSearchRequest {
                q: r#"""" OR (fintech) NEAR/5 * ^ : - + NOT"#.to_string(),
                limit: 10,
                tier: ReaSearchTier::Tier1a,
            },
        )
        .expect("malformed FTS5 query must not error");
    assert_eq!(nasty.total, 1);
}

#[test]
fn ac02_license_tos_firewall_and_design_only_sanitizer() {
    assert_eq!(
        evaluate_license_and_tos("MIT", "https://example.com/a", "").verdict,
        FirewallVerdict::AdoptPriorArt
    );
    assert_eq!(
        evaluate_license_and_tos("Apache-2.0", "https://example.com/a", "").verdict,
        FirewallVerdict::AdoptPriorArt
    );
    assert_eq!(
        evaluate_license_and_tos("BSD-3-Clause", "https://example.com/a", "").verdict,
        FirewallVerdict::AdoptPriorArt
    );
    assert_eq!(
        evaluate_license_and_tos("ISC", "https://example.com/a", "").verdict,
        FirewallVerdict::AdoptPriorArt
    );
    assert_eq!(
        evaluate_license_and_tos("CC0-1.0", "https://example.com/a", "").verdict,
        FirewallVerdict::AdoptPriorArt
    );

    assert_eq!(
        evaluate_license_and_tos("LGPL-2.1", "https://example.com/b", "").verdict,
        FirewallVerdict::ReferenceOnly
    );
    assert_eq!(
        evaluate_license_and_tos("MPL-2.0", "https://example.com/b", "").verdict,
        FirewallVerdict::ReferenceOnly
    );
    assert_eq!(
        evaluate_license_and_tos("CC-BY-4.0", "https://example.com/b", "").verdict,
        FirewallVerdict::ReferenceOnly
    );
    assert_eq!(
        evaluate_license_and_tos("UNKNOWN_PUBLIC", "https://example.com/b", "").verdict,
        FirewallVerdict::ReferenceOnly
    );

    for bad_license in [
        "GPL-3.0",
        "AGPL-3.0-only",
        "PROPRIETARY",
        "ALL-RIGHTS-RESERVED",
    ] {
        assert_eq!(
            evaluate_license_and_tos(bad_license, "https://example.com/c", "").verdict,
            FirewallVerdict::Reject,
            "license {bad_license} must be REJECT"
        );
    }
    assert_eq!(
        evaluate_license_and_tos(
            "MIT",
            "https://example.com/bypass-tos",
            "automated scraping is prohibited"
        )
        .verdict,
        FirewallVerdict::Reject
    );

    // Verify REJECT never persists into Two-Store
    let dir = tempdir().expect("tempdir");
    let svc = ReaBridgeService::open(dir.path()).expect("open");
    let rejected = svc.ingest(
        "tenant-1",
        ReaIngestRequest {
            slug: "blocked-gpl".to_string(),
            source_url: "https://github.com/example/gpl-app".to_string(),
            source_format: SourceFormat::Github,
            license_spdx: "AGPL-3.0".to_string(),
            summary_hint: None,
            payload_text: Some("<div>hello</div>".to_string()),
            payload_base64: None,
        },
    );
    assert!(matches!(
        rejected,
        Err(ReaBridgeError::FirewallRejected { .. })
    ));
    let after = svc
        .search(
            "tenant-1",
            ReaSearchRequest {
                q: "".to_string(),
                limit: 10,
                tier: ReaSearchTier::Tier1a,
            },
        )
        .expect("search");
    assert_eq!(after.total, 0);

    // Sanitizer strips SQL, API keys, Bearer/JWT, and /api/admin URLs (built at runtime for Gitleaks hygiene)
    let syn_k1 = ["sk", "proj", "abc1234567890XYZ"].join("-");
    let syn_k2 = ["AIza", "SyD1234567890abcdefghijk"].concat();
    let syn_k3 = ["ghp", "1234567890abcdefghijklmno"].join("_");
    let syn_jwt = [
        "eyJhbGciOiJIUzI1NiJ9",
        "eyJzdWIiOiIxMjM0NTY3OCJ9",
        "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
    ]
    .join(".");
    let dirty = format!(
        "SELECT password_hash FROM users WHERE role = 'admin';\n\
         INSERT INTO audit_tokens VALUES ('x');\n\
         DELETE FROM sessions WHERE id = 1;\n\
         const k1 = \"{syn_k1}\";\n\
         const k2 = \"{syn_k2}\";\n\
         const k3 = \"{syn_k3}\";\n\
         Authorization: Bearer super_secret_token_value_123456\n\
         jwt = {syn_jwt}\n\
         fetch(\"/api/admin/rotate-keys\")"
    );
    let report = sanitize_design_only_text(&dirty);
    assert!(report.sanitized_secrets_count >= 8);
    assert!(!report.sanitized_text.contains("SELECT password_hash"));
    assert!(!report.sanitized_text.contains(&syn_k1));
    assert!(!report.sanitized_text.contains(&syn_k2));
    assert!(!report.sanitized_text.contains(&syn_k3));
    assert!(!report.sanitized_text.contains("/api/admin"));
    assert!(!report.sanitized_text.contains("eyJhbGciOiJIUzI1NiJ9"));
}

#[test]
fn ac03_ssrf_guard_prohibited_ports_and_zip_bomb_security() {
    // Public URL passes
    assert!(validate_public_url_ssrf("https://github.com/cowok/design-tokens").is_ok());

    // Loopback, RFC1918, Link-local, IPv4-mapped IPv6, ULA, metadata all blocked
    for blocked in [
        "http://127.0.0.1/admin",
        "http://localhost:8080/internal",
        "http://10.0.0.5/secret",
        "http://172.16.4.2/secret",
        "http://192.168.1.10/secret",
        "http://169.254.169.254/latest/meta-data",
        "http://[::1]/local",
        "http://[::ffff:127.0.0.1]/mapped",
        "http://[fd00::1]/ula",
        "file:///etc/passwd",
        "https://user:pass@example.com/x",
    ] {
        assert!(
            validate_public_url_ssrf(blocked).is_err(),
            "URL {blocked} must be blocked by SSRF guard"
        );
    }

    // Every prohibited workspace port blocked
    for &port in PROHIBITED_PORTS {
        let u = format!("https://example.com:{port}/design");
        assert!(
            matches!(
                validate_public_url_ssrf(&u),
                Err(FirewallError::ProhibitedPort { port: p }) if p == port
            ),
            "Port {port} must be rejected"
        );
        let sock = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)), port);
        assert!(validate_socket_addr_ssrf(sock).is_err());
    }

    // Zip-bomb: expansion ratio > 25x
    let bomb = build_zip_bytes(&[("tokens.css", b"0123456789", 8, 0, Some(10_000))]);
    let bomb_b64 = B64_STD.encode(&bomb);
    let dir = tempdir().expect("tempdir");
    let svc = ReaBridgeService::open(dir.path()).expect("open");
    let res_bomb = svc.ingest(
        "t1",
        ReaIngestRequest {
            slug: "zip-bomb".to_string(),
            source_url: "local://upload.zip".to_string(),
            source_format: SourceFormat::Zip,
            license_spdx: "MIT".to_string(),
            summary_hint: None,
            payload_text: None,
            payload_base64: Some(bomb_b64),
        },
    );
    assert!(matches!(
        res_bomb,
        Err(ReaBridgeError::Importer(
            ImporterError::ZipRatioExceeded { .. }
        ))
    ));

    // Zip path traversal (`../evil.txt`) rejected
    let trav = build_zip_bytes(&[("../evil.txt", b"boom", 0, 0, None)]);
    let res_trav = rea_bridge::read_zip_safely(&trav);
    assert!(matches!(res_trav, Err(ImporterError::ZipPathTraversal(_))));

    // Zip Unix symlink entry rejected
    let symlink_attr = (0o120777u32) << 16;
    let sym = build_zip_bytes(&[("link.css", b"/etc/passwd", 0, symlink_attr, None)]);
    let res_sym = rea_bridge::read_zip_safely(&sym);
    assert!(matches!(res_sym, Err(ImporterError::ZipSymlinkRejected(_))));

    // Zip > 128 entries rejected
    let names: Vec<String> = (0..130).map(|i| format!("f{i}.txt")).collect();
    let specs: Vec<ZipEntrySpec<'_>> = names
        .iter()
        .map(|n| (n.as_str(), &b"ok"[..], 0, 0, None))
        .collect();
    let too_many = build_zip_bytes(&specs);
    let res_many = rea_bridge::read_zip_safely(&too_many);
    assert!(matches!(
        res_many,
        Err(ImporterError::ZipTooManyEntries { count: 130, .. })
    ));
}

#[cfg(unix)]
#[test]
fn ac03_two_store_rejects_symlink_cache_file() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("tempdir");
    let cache_dir = dir.path().join(".cowok-design/cache");
    std::fs::create_dir_all(&cache_dir).unwrap();
    let victim = dir.path().join("victim.sqlite3");
    std::fs::write(&victim, b"not-a-db").unwrap();
    symlink(&victim, cache_dir.join("rea-reference.sqlite3")).unwrap();

    let err = ReaTwoStore::open(dir.path()).unwrap_err();
    assert!(matches!(err, ReaStoreError::SymlinkRejected(_)));
}

#[test]
fn ac04_lossless_multi_format_importer_and_honest_figma_proof() {
    let dir = tempdir().expect("tempdir");
    let svc = ReaBridgeService::open(dir.path()).expect("open");

    // 1. Lossless .fig preserving raw_fields
    let fig_payload = serde_json::json!({
        "name": "Cowok Design System",
        "document": { "id": "0:0", "type": "DOCUMENT", "children": [{ "name": "Card", "background": "#10b981" }] },
        "raw_fields": {
            "kiwiSchemaVersion": 42,
            "vectorNetworkBlob": "opaque-lossless-bytes",
            "customPluginData": { "gridCols": 12 }
        }
    })
    .to_string();
    let fig_out = svc
        .ingest(
            "tenant-design",
            ReaIngestRequest {
                slug: "cowok-fig-system".to_string(),
                source_url: "local://cowok-system.fig".to_string(),
                source_format: SourceFormat::Fig,
                license_spdx: "MIT".to_string(),
                summary_hint: Some("Lossless Figma design tokens".to_string()),
                payload_text: Some(fig_payload),
                payload_base64: None,
            },
        )
        .expect("ingest fig");
    let raw_fields = fig_out
        .record
        .raw_fields_json
        .as_deref()
        .expect("raw_fields must be preserved");
    assert!(raw_fields.contains("kiwiSchemaVersion"));
    assert!(raw_fields.contains("opaque-lossless-bytes"));

    // 2. Valid .zip design bundle
    let valid_zip = build_zip_bytes(&[
        (
            "tokens.css",
            b":root { --action-primary: #0f766e; font-family: Inter; padding: 16px; }",
            0,
            0,
            None,
        ),
        (
            "layout.html",
            b"<header><nav><button class='card'>Launch</button></nav></header>",
            0,
            0,
            None,
        ),
    ]);
    let zip_out = svc
        .ingest(
            "tenant-design",
            ReaIngestRequest {
                slug: "claude-design-bundle".to_string(),
                source_url: "local://bundle.zip".to_string(),
                source_format: SourceFormat::Zip,
                license_spdx: "Apache-2.0".to_string(),
                summary_hint: None,
                payload_text: None,
                payload_base64: Some(B64_STD.encode(&valid_zip)),
            },
        )
        .expect("ingest valid zip");
    assert_eq!(zip_out.extracted_files.len(), 2);

    // 3. Clipboard, .pen, .wire
    for (slug, fmt, payload) in [
        (
            "clip-hero",
            SourceFormat::Clipboard,
            r##"{"type":"FRAME","name":"Hero","fills":["#0ea5e9"],"components":["button","card"]}"##,
        ),
        (
            "pen-tokens",
            SourceFormat::Pen,
            r##"{"tool":"penpot","palette":["#14b8a6"],"layout":"grid"}"##,
        ),
        (
            "wire-checkout",
            SourceFormat::Wire,
            r#"{"screens":["Cart","Payment"],"components":["modal","form"]}"#,
        ),
    ] {
        let out = svc
            .ingest(
                "tenant-design",
                ReaIngestRequest {
                    slug: slug.to_string(),
                    source_url: format!("local://{slug}"),
                    source_format: fmt,
                    license_spdx: "MIT".to_string(),
                    summary_hint: None,
                    payload_text: Some(payload.to_string()),
                    payload_base64: None,
                },
            )
            .expect("ingest multi-format");
        assert_eq!(out.record.proof_status, ProofStatus::AwaitingVerification);
    }

    // 4. web-clone L1-L6 layers
    let layers = extract_web_clone_l1_l6(
        "<header style='color:#0f766e;font-family:Inter;padding:12px;transition:all 0.2s'><h1>Title</h1><button class='card'>Go</button></header>",
    );
    assert!(!layers.l1_palette.is_empty());
    assert!(!layers.l2_typography.is_empty());
    assert!(!layers.l3_spacing_radius.is_empty());
    assert!(!layers.l4_layout_sections.is_empty());
    assert!(!layers.l5_components.is_empty());
    assert!(!layers.l6_interactions.is_empty());

    // 5. Honest Figma Proof Exporter: awaiting-verification -> reject wrong checksum -> proof-verified on exact SHA-256
    let step1 = svc
        .export_proof(
            "tenant-design",
            ReaExportProofRequest {
                reference_id: fig_out.record.id.clone(),
                roundtrip_checksum: None,
            },
        )
        .expect("export awaiting");
    assert_eq!(step1.proof_status, ProofStatus::AwaitingVerification);
    assert!(!step1.verified);

    let step2_wrong = svc
        .export_proof(
            "tenant-design",
            ReaExportProofRequest {
                reference_id: fig_out.record.id.clone(),
                roundtrip_checksum: Some("deadbeef".repeat(8)),
            },
        )
        .expect("export wrong checksum");
    assert_eq!(step2_wrong.proof_status, ProofStatus::AwaitingVerification);
    assert!(!step2_wrong.verified);
    assert!(!step2_wrong.checksum_matched);

    let step3_valid = svc
        .export_proof(
            "tenant-design",
            ReaExportProofRequest {
                reference_id: fig_out.record.id.clone(),
                roundtrip_checksum: Some(step1.computed_sha256.clone()),
            },
        )
        .expect("export valid checksum");
    assert_eq!(step3_valid.proof_status, ProofStatus::ProofVerified);
    assert!(step3_valid.verified);
    assert!(step3_valid.checksum_matched);

    // Persisted in Two-Store
    let persisted = svc
        .store()
        .get_by_id("tenant-design", &fig_out.record.id)
        .expect("get_by_id")
        .expect("exists");
    assert_eq!(persisted.proof_status, ProofStatus::ProofVerified);
}

#[test]
fn f01_archive_firewall_rejects_hidden_tos_and_downgrades_weak_copyleft() {
    let dir = tempdir().expect("tempdir");
    let svc = ReaBridgeService::open(dir.path()).expect("open");

    // 1. ZIP claiming MIT with hidden ToS reject text inside archive entry -> REJECT
    let bad_zip = build_zip_bytes(&[
        ("tokens.css", b":root { --bg: #fff; }", 0, 0, None),
        (
            "LICENSE.txt",
            b"Copyright Acme Corp. All Rights Reserved. Scraping is prohibited.",
            0,
            0,
            None,
        ),
    ]);
    let res_bad_zip = svc.ingest(
        "t-f01",
        ReaIngestRequest {
            slug: "mit-lying-zip".to_string(),
            source_url: "local://lying.zip".to_string(),
            source_format: SourceFormat::Zip,
            license_spdx: "MIT".to_string(),
            summary_hint: None,
            payload_text: None,
            payload_base64: Some(B64_STD.encode(&bad_zip)),
        },
    );
    assert!(matches!(
        res_bad_zip,
        Err(ReaBridgeError::FirewallRejected { .. })
    ));

    // 2. .fig ZIP archive with reject signal in entry filename -> REJECT
    let bad_fig_zip = build_zip_bytes(&[(
        "bypass-tos/canvas.json",
        br##"{"name":"Leak","palette":["#000"]}"##,
        0,
        0,
        None,
    )]);
    let res_bad_fig = svc.ingest(
        "t-f01",
        ReaIngestRequest {
            slug: "mit-lying-fig".to_string(),
            source_url: "local://lying.fig".to_string(),
            source_format: SourceFormat::Fig,
            license_spdx: "MIT".to_string(),
            summary_hint: None,
            payload_text: None,
            payload_base64: Some(B64_STD.encode(&bad_fig_zip)),
        },
    );
    assert!(matches!(
        res_bad_fig,
        Err(ReaBridgeError::FirewallRejected { .. })
    ));

    // Verify neither rejected archive persisted
    let after_reject = svc
        .search(
            "t-f01",
            ReaSearchRequest {
                q: "".to_string(),
                limit: 10,
                tier: ReaSearchTier::Tier1a,
            },
        )
        .expect("search");
    assert_eq!(after_reject.total, 0);

    // 3. ZIP claiming MIT containing LGPL-2.1 notice -> downgraded to REFERENCE_ONLY
    let lgpl_zip = build_zip_bytes(&[
        ("tokens.css", b":root { --action: #0f766e; }", 0, 0, None),
        (
            "NOTICE.txt",
            b"Portions licensed under GNU Lesser General Public License (LGPL-2.1).",
            0,
            0,
            None,
        ),
    ]);
    let downgraded = svc
        .ingest(
            "t-f01",
            ReaIngestRequest {
                slug: "mit-with-lgpl-archive".to_string(),
                source_url: "local://mixed.zip".to_string(),
                source_format: SourceFormat::Zip,
                license_spdx: "MIT".to_string(),
                summary_hint: None,
                payload_text: None,
                payload_base64: Some(B64_STD.encode(&lgpl_zip)),
            },
        )
        .expect("downgraded archive should persist as REFERENCE_ONLY");
    assert_eq!(downgraded.firewall.verdict, FirewallVerdict::ReferenceOnly);
    assert_eq!(
        downgraded.record.firewall_verdict,
        FirewallVerdict::ReferenceOnly
    );
}

#[test]
fn f02_f09_ssrf_trailing_dot_metadata_goog_and_ipv6_special_ranges() {
    for blocked in [
        "http://localhost./admin",
        "http://sub.localhost./x",
        "http://printer.local./status",
        "http://metadata.google.internal./computeMetadata/v1/",
        "http://metadata.goog/computeMetadata/v1/",
        "http://metadata.goog./computeMetadata/v1/",
        "http://169.254.169.254./latest/meta-data/",
        "http://[2002:c000:0204::1]/6to4",
        "http://[::ffff:0:127.0.0.1]/v4-translated",
        "http://[64:ff9b:1::1]/nat64-local",
        "http://[2001:0:4136:e378::1]/teredo",
        "http://[2001:1ff::1]/ietf-23",
        "http://[2001:db8::1]/doc",
        "http://[0100::1]/discard",
    ] {
        assert!(
            validate_public_url_ssrf(blocked).is_err(),
            "F-02/F-09 URL must be blocked: {blocked}"
        );
    }

    // Legitimate public IPv6 remains allowed
    assert!(validate_public_url_ssrf("https://[2606:2800:220:1:248:1893:25c8:1946]/").is_ok());
    assert!(validate_public_url_ssrf("https://[2001:4860:4860::8888]/").is_ok());
}

#[test]
fn f03_non_network_scheme_smuggling_and_f12_slug_alignment() {
    let dir = tempdir().expect("tempdir");
    let svc = ReaBridgeService::open(dir.path()).expect("open");

    for bad_uri in [
        "javascript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "file:///etc/passwd",
        "//169.254.169.254/latest/meta-data",
        "local://../../etc/passwd",
        "local://design.fig?token=secret",
        "local://design.fig#frag",
        "local://bad\nname.fig",
        "local://",
    ] {
        let res = svc.ingest(
            "t-f03",
            ReaIngestRequest {
                slug: "scheme-smuggle-test".to_string(),
                source_url: bad_uri.to_string(),
                source_format: SourceFormat::Clipboard,
                license_spdx: "MIT".to_string(),
                summary_hint: None,
                payload_text: Some(r#"{"type":"FRAME"}"#.to_string()),
                payload_base64: None,
            },
        );
        assert!(
            matches!(res, Err(ReaBridgeError::Ssrf(_))),
            "F-03 must block non-network smuggled URI: {bad_uri}"
        );
    }

    // F-12: Slugs starting with punctuation or containing '..' must be rejected
    for bad_slug in [".leading-dot", "-leading-dash", "_leading-under", "a..b"] {
        let res = svc.ingest(
            "t-f12",
            ReaIngestRequest {
                slug: bad_slug.to_string(),
                source_url: "local://ok.fig".to_string(),
                source_format: SourceFormat::Clipboard,
                license_spdx: "MIT".to_string(),
                summary_hint: None,
                payload_text: Some(r#"{"type":"FRAME"}"#.to_string()),
                payload_base64: None,
            },
        );
        assert!(
            matches!(res, Err(ReaBridgeError::InvalidSlug(_))),
            "F-12 must reject invalid slug: {bad_slug}"
        );
    }
}

#[test]
fn f04_f07_sanitizes_source_url_json_keys_extracted_files_and_extended_regexes() {
    let dir = tempdir().expect("tempdir");
    let svc = ReaBridgeService::open(dir.path()).expect("open");

    // Synthetic tokens built dynamically at runtime for Gitleaks hygiene
    let stripe_sk = ["sk", "live", "AbCdEf0123456789xyz"].join("_");
    let stripe_rk = ["rk", "live", "ZyXwVu9876543210abc"].join("_");
    let gh_user = ["ghu", "1234567890abcdefghijklmno"].join("_");
    let gh_srv = ["ghs", "0987654321zyxwvutsrqponml"].join("_");
    let gh_ref = ["ghr", "abcdef1234567890ghijklmno"].join("_");
    let basic_auth = ["Authorization: Basic ", "YWRtaW46c3VwZXJzZWNyZXQ="].concat();

    // F-07 direct sanitizer checks
    let dirty = format!(
        "SELECT\n  id,\n  secret_hash\nFROM\n  tenant_secrets\nWHERE active = 1;\n\
         {stripe_sk} {stripe_rk} {gh_user} {gh_srv} {gh_ref}\n\
         {basic_auth}\n\
         rediss://user:pw@cache.example.com:6379/0\n\
         amqps://user:pw@mq.example.com:5671/vhost\n\
         http://172.16.4.2:8080/secret\n\
         http://[::1]:4170/internal\n\
         http://[fd00::42]/vault\n\
         http://0.0.0.0:9000/metrics\n\
         http://billing.internal/v1\n\
         http://db.cluster.local/status"
    );
    let rep = sanitize_design_only_text(&dirty);
    assert!(rep.sanitized_secrets_count >= 15);
    assert!(!rep.sanitized_text.contains("secret_hash"));
    assert!(!rep.sanitized_text.contains(&stripe_sk));
    assert!(!rep.sanitized_text.contains(&stripe_rk));
    assert!(!rep.sanitized_text.contains(&gh_user));
    assert!(!rep.sanitized_text.contains(&gh_srv));
    assert!(!rep.sanitized_text.contains(&gh_ref));
    assert!(!rep.sanitized_text.contains("YWRtaW46c3VwZXJzZWNyZXQ="));
    assert!(!rep.sanitized_text.contains("rediss://"));
    assert!(!rep.sanitized_text.contains("amqps://"));
    assert!(!rep.sanitized_text.contains("172.16.4.2"));
    assert!(!rep.sanitized_text.contains("[::1]"));
    assert!(!rep.sanitized_text.contains("[fd00::42]"));
    assert!(!rep.sanitized_text.contains("0.0.0.0"));
    assert!(!rep.sanitized_text.contains("billing.internal"));
    assert!(!rep.sanitized_text.contains("db.cluster.local"));

    // F-04: JSON object keys & source_url & extracted_files sanitized
    let secret_key_fig = format!(
        r#"{{"name":"Kit","raw_fields":{{"{stripe_sk}":"val","nested":{{"{gh_user}":"ok"}}}}}}"#
    );
    let fig_zip = build_zip_bytes(&[(
        &format!("assets/{stripe_rk}.json"),
        secret_key_fig.as_bytes(),
        0,
        0,
        None,
    )]);
    let out = svc
        .ingest(
            "t-f04",
            ReaIngestRequest {
                slug: "sanitized-archive".to_string(),
                source_url: format!("https://example.com/kit.fig?token={stripe_sk}"),
                source_format: SourceFormat::Fig,
                license_spdx: "MIT".to_string(),
                summary_hint: None,
                payload_text: None,
                payload_base64: Some(B64_STD.encode(&fig_zip)),
            },
        )
        .expect("ingest sanitized archive");

    assert!(!out.record.source_url.contains(&stripe_sk));
    assert!(out.record.source_url.contains("[REDACTED_API_KEY]"));
    let rf = out.record.raw_fields_json.as_deref().unwrap_or("");
    assert!(!rf.contains(&stripe_sk));
    assert!(!rf.contains(&gh_user));
    assert!(!rf.contains(&stripe_rk));
    assert!(out
        .extracted_files
        .iter()
        .all(|f| !f.contains(&stripe_rk) && f.contains("[REDACTED_API_KEY]")));
    assert!(out.record.sanitized_secrets_count >= 4);
}

#[cfg(unix)]
#[test]
fn f05_two_store_symlink_parent_wal_shm_and_connect_toctou_hardening() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    // 1. Symlink .cowok-design directory rejected on open()
    let dir1 = tempdir().expect("tempdir");
    let outside = tempdir().expect("outside");
    symlink(outside.path(), dir1.path().join(".cowok-design")).unwrap();
    let err1 = ReaTwoStore::open(dir1.path()).unwrap_err();
    assert!(matches!(err1, ReaStoreError::SymlinkRejected(_)));

    // 2. Symlink -wal file rejected on open()
    let dir2 = tempdir().expect("tempdir");
    let cache2 = dir2.path().join(".cowok-design/cache");
    std::fs::create_dir_all(&cache2).unwrap();
    let victim_wal = dir2.path().join("victim.wal");
    std::fs::write(&victim_wal, b"victim").unwrap();
    symlink(&victim_wal, cache2.join("rea-reference.sqlite3-wal")).unwrap();
    let err2 = ReaTwoStore::open(dir2.path()).unwrap_err();
    assert!(matches!(err2, ReaStoreError::SymlinkRejected(_)));

    // 3. Valid open + write creates 0o600 DB/WAL/SHM and rejects post-open symlink swap on connect()
    let dir3 = tempdir().expect("tempdir");
    let svc = ReaBridgeService::open(dir3.path()).expect("open");
    svc.ingest(
        "t-f05",
        ReaIngestRequest {
            slug: "wal-perm-check".to_string(),
            source_url: "local://ok.fig".to_string(),
            source_format: SourceFormat::Clipboard,
            license_spdx: "MIT".to_string(),
            summary_hint: None,
            payload_text: Some(r#"{"type":"FRAME"}"#.to_string()),
            payload_base64: None,
        },
    )
    .expect("ingest");

    let db_path = svc.store().db_path().to_path_buf();
    let db_mode = std::fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(db_mode, 0o600);
    for suffix in ["-wal", "-shm"] {
        let mut os = db_path.as_os_str().to_os_string();
        os.push(suffix);
        let p = std::path::PathBuf::from(os);
        if p.exists() {
            let m = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(m, 0o600, "sidecar {suffix} must have mode 0o600");
        }
    }

    // Swap -shm for a symlink after open() and verify subsequent connect() rejects it
    let mut shm_os = db_path.as_os_str().to_os_string();
    shm_os.push("-shm");
    let shm_path = std::path::PathBuf::from(shm_os);
    let _ = std::fs::remove_file(&shm_path);
    let victim_shm = dir3.path().join("victim.shm");
    std::fs::write(&victim_shm, b"victim").unwrap();
    symlink(&victim_shm, &shm_path).unwrap();

    let err_connect = svc.search(
        "t-f05",
        ReaSearchRequest {
            q: "wal".to_string(),
            limit: 5,
            tier: ReaSearchTier::Tier1a,
        },
    );
    assert!(matches!(
        err_connect,
        Err(ReaBridgeError::Store(ReaStoreError::SymlinkRejected(_)))
    ));
}

#[test]
fn f06_license_and_tos_policy_bypasses_blocked() {
    for bad in [
        "GNU General Public License v3.0",
        "GNU Affero General Public License",
        "BSL-1.1",
        "BSL",
        "UNLICENSED",
        "MIT OR UNLICENSED",
        "All Rights Reserved",
    ] {
        assert_eq!(
            evaluate_license_and_tos(bad, "https://example.com/kit", "").verdict,
            FirewallVerdict::Reject,
            "F-06 must reject license: {bad}"
        );
    }

    // UNLICENSE (public domain dedication) remains ADOPT_PRIOR_ART (distinct from UNLICENSED)
    assert_eq!(
        evaluate_license_and_tos("UNLICENSE", "https://example.com/kit", "").verdict,
        FirewallVerdict::AdoptPriorArt
    );

    // Normalized ToS signals with whitespace/hyphens
    for signal in [
        "Bypass   ToS",
        "scraping\nis   prohibited",
        "Do-Not-Scrape",
        "do   not\tscrape",
        "All-Rights-Reserved",
    ] {
        assert_eq!(
            evaluate_license_and_tos("MIT", "https://example.com/kit", signal).verdict,
            FirewallVerdict::Reject,
            "F-06 must reject normalized ToS signal: {signal}"
        );
    }

    // AND/OR precedence: OR must never override an AND clause containing weak-copyleft
    assert_eq!(
        evaluate_license_and_tos(
            "LGPL-2.1 AND MIT OR Apache-2.0",
            "https://example.com/kit",
            ""
        )
        .verdict,
        FirewallVerdict::ReferenceOnly
    );
    assert_eq!(
        evaluate_license_and_tos(
            "MIT OR Apache-2.0 AND LGPL-2.1",
            "https://example.com/kit",
            ""
        )
        .verdict,
        FirewallVerdict::ReferenceOnly
    );
    assert_eq!(
        evaluate_license_and_tos("MIT AND Apache-2.0", "https://example.com/kit", "").verdict,
        FirewallVerdict::AdoptPriorArt
    );
}

#[test]
fn f08_tier1a_token_cap_enforces_char_budget_on_whitespace_free_strings() {
    let huge_no_ws = "a".repeat(5_000);
    assert_eq!(count_deterministic_tokens(&huge_no_ws), 1_250);
    let (capped, count) = rea_bridge::truncate_to_token_cap(&huge_no_ws, TIER1A_MAX_TOKENS);
    assert_eq!(capped.chars().count(), 1_000);
    assert_eq!(count, TIER1A_MAX_TOKENS);
    assert_eq!(count_deterministic_tokens(&capped), TIER1A_MAX_TOKENS);

    let utf8_no_ws = "é".repeat(2_000);
    let (capped_utf8, count_utf8) =
        rea_bridge::truncate_to_token_cap(&utf8_no_ws, TIER1A_MAX_TOKENS);
    assert_eq!(capped_utf8.chars().count(), 1_000);
    assert_eq!(count_utf8, TIER1A_MAX_TOKENS);
}
