// @story 09.1
// @implements AC-01, AC-02 (Real Awesome-RE Visual App Catalog indexed in Two-Store SQLite FTS5)
use std::sync::OnceLock;

use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::firewall::FirewallVerdict;
use crate::store::{sanitize_fts5_query, ReaStoreError};

const EMBEDDED_AWESOME_RE_CATALOG_JSON: &str = include_str!(
    "../../../packages/builtin-skills/design-craft/catalogs/awesome-re-ui-catalog.json"
);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwesomeReCatalogItem {
    pub slug: String,
    pub name: String,
    pub repo_url: String,
    pub category: String,
    #[serde(default)]
    pub subcategory: String,
    pub stars: u64,
    pub license_spdx: String,
    pub firewall_verdict: FirewallVerdict,
    #[serde(default)]
    pub lang: String,
    pub description: String,
    #[serde(default)]
    pub topics: Vec<String>,
    pub layout_archetype: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbeddedCatalogFile {
    upstream_repo: String,
    upstream_commit: String,
    items: Vec<AwesomeReCatalogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwesomeReCatalogResponse {
    pub upstream_repo: String,
    pub upstream_commit: String,
    pub total_catalog_entries: usize,
    pub query: String,
    pub sanitized_fts_query: String,
    pub total: usize,
    pub items: Vec<AwesomeReCatalogItem>,
}

struct ParsedCatalog {
    checksum: String,
    upstream_repo: String,
    upstream_commit: String,
    items: Vec<AwesomeReCatalogItem>,
}

static PARSED_CATALOG: OnceLock<ParsedCatalog> = OnceLock::new();

fn embedded_catalog() -> &'static ParsedCatalog {
    PARSED_CATALOG.get_or_init(|| {
        let mut hasher = Sha256::new();
        hasher.update(EMBEDDED_AWESOME_RE_CATALOG_JSON.as_bytes());
        let checksum = format!("{:x}", hasher.finalize());
        let parsed: EmbeddedCatalogFile = serde_json::from_str(EMBEDDED_AWESOME_RE_CATALOG_JSON)
            .expect("embedded awesome-re-ui-catalog.json must be valid JSON");
        ParsedCatalog {
            checksum,
            upstream_repo: parsed.upstream_repo,
            upstream_commit: parsed.upstream_commit,
            items: parsed.items,
        }
    })
}

pub fn ensure_awesome_re_catalog_schema_and_seed(
    conn: &mut Connection,
) -> Result<(), ReaStoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS awesome_re_catalog_meta (
             key TEXT PRIMARY KEY NOT NULL,
             value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS awesome_re_catalog (
             slug TEXT PRIMARY KEY NOT NULL,
             name TEXT NOT NULL,
             repo_url TEXT NOT NULL,
             category TEXT NOT NULL,
             subcategory TEXT NOT NULL,
             stars INTEGER NOT NULL,
             license_spdx TEXT NOT NULL,
             firewall_verdict TEXT NOT NULL,
             lang TEXT NOT NULL,
             description TEXT NOT NULL,
             topics_json TEXT NOT NULL,
             layout_archetype TEXT NOT NULL
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS awesome_re_catalog_fts USING fts5(
             slug UNINDEXED,
             name,
             description,
             category,
             subcategory,
             topics,
             layout_archetype,
             tokenize='unicode61'
         );",
    )?;

    let catalog = embedded_catalog();
    let existing_sha: Option<String> = conn
        .query_row(
            "SELECT value FROM awesome_re_catalog_meta WHERE key = 'catalog_sha256'",
            [],
            |row| row.get(0),
        )
        .ok();
    if existing_sha.as_deref() == Some(catalog.checksum.as_str()) {
        return Ok(());
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute("DELETE FROM awesome_re_catalog", [])?;
    tx.execute("DELETE FROM awesome_re_catalog_fts", [])?;
    {
        let mut ins_row = tx.prepare(
            "INSERT INTO awesome_re_catalog (
                 slug, name, repo_url, category, subcategory, stars,
                 license_spdx, firewall_verdict, lang, description, topics_json, layout_archetype
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;
        let mut ins_fts = tx.prepare(
            "INSERT INTO awesome_re_catalog_fts (
                 slug, name, description, category, subcategory, topics, layout_archetype
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for item in &catalog.items {
            let topics_json =
                serde_json::to_string(&item.topics).unwrap_or_else(|_| "[]".to_string());
            let topics_text = item.topics.join(" ");
            ins_row.execute(params![
                item.slug,
                item.name,
                item.repo_url,
                item.category,
                item.subcategory,
                item.stars as i64,
                item.license_spdx,
                item.firewall_verdict.as_str(),
                item.lang,
                item.description,
                topics_json,
                item.layout_archetype,
            ])?;
            ins_fts.execute(params![
                item.slug,
                item.name,
                item.description,
                item.category,
                item.subcategory,
                topics_text,
                item.layout_archetype,
            ])?;
        }
    }
    tx.execute(
        "INSERT INTO awesome_re_catalog_meta (key, value) VALUES ('catalog_sha256', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![catalog.checksum],
    )?;
    tx.commit()?;
    Ok(())
}

fn map_catalog_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AwesomeReCatalogItem> {
    let verdict_str: String = row.get(7)?;
    let topics_json: String = row.get(10)?;
    let topics: Vec<String> = serde_json::from_str(&topics_json).unwrap_or_default();
    let stars_raw: i64 = row.get(5)?;
    Ok(AwesomeReCatalogItem {
        slug: row.get(0)?,
        name: row.get(1)?,
        repo_url: row.get(2)?,
        category: row.get(3)?,
        subcategory: row.get(4)?,
        stars: stars_raw.max(0) as u64,
        license_spdx: row.get(6)?,
        firewall_verdict: FirewallVerdict::from_db_str(&verdict_str)
            .unwrap_or(FirewallVerdict::ReferenceOnly),
        lang: row.get(8)?,
        description: row.get(9)?,
        topics,
        layout_archetype: row.get(11)?,
    })
}

pub fn query_awesome_re_catalog(
    conn: &mut Connection,
    raw_query: &str,
    limit: usize,
) -> Result<AwesomeReCatalogResponse, ReaStoreError> {
    ensure_awesome_re_catalog_schema_and_seed(conn)?;
    let catalog = embedded_catalog();
    let capped_limit = limit.clamp(1, 20) as i64;
    let sanitized = sanitize_fts5_query(raw_query);
    let mut items = Vec::new();

    if sanitized.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT slug, name, repo_url, category, subcategory, stars,
                    license_spdx, firewall_verdict, lang, description, topics_json, layout_archetype
             FROM awesome_re_catalog
             ORDER BY stars DESC, slug ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![capped_limit], map_catalog_row)?;
        for r in rows {
            items.push(r?);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT c.slug, c.name, c.repo_url, c.category, c.subcategory, c.stars,
                    c.license_spdx, c.firewall_verdict, c.lang, c.description, c.topics_json, c.layout_archetype
             FROM awesome_re_catalog_fts
             JOIN awesome_re_catalog c ON c.slug = awesome_re_catalog_fts.slug
             WHERE awesome_re_catalog_fts MATCH ?1
             ORDER BY bm25(awesome_re_catalog_fts), c.stars DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![sanitized, capped_limit], map_catalog_row)?;
        for r in rows {
            items.push(r?);
        }
    }

    Ok(AwesomeReCatalogResponse {
        upstream_repo: catalog.upstream_repo.clone(),
        upstream_commit: catalog.upstream_commit.clone(),
        total_catalog_entries: catalog.items.len(),
        query: raw_query.to_string(),
        sanitized_fts_query: sanitized,
        total: items.len(),
        items,
    })
}
