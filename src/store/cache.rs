//! SQLite cache: models, tags, changelog, per-gateway health.
//!
//! Everything the catalog needs offline lives here. API keys never touch this file.

use crate::providers::ModelItem;
use crate::store::tags::{db, IGNORE};
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Cache {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub builtin: bool,
}

#[derive(Debug, Clone)]
pub struct ModelRow {
    pub pk: i64,
    pub gateway_id: String,
    pub model_id: String,
    pub owner: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub removed_at: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayStatus {
    pub last_success_at: Option<i64>,
    pub last_error_at: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChangeEntry {
    pub gateway_id: String,
    pub model_id: String,
    pub change: String,
    pub at: i64,
    pub ack: bool,
}

#[derive(Debug, Default, Clone)]
pub struct DiffSummary {
    /// First-ever fetch of a gateway (import, not "new models": no changelog, no toast).
    pub imported: bool,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub returned: Vec<String>,
    pub total: usize,
}

impl Cache {
    pub fn open(path: &Path) -> rusqlite::Result<Cache> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::migrate(&conn)?;
        Ok(Cache { conn: Arc::new(Mutex::new(conn)) })
    }

    fn migrate(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS models (
                pk         INTEGER PRIMARY KEY AUTOINCREMENT,
                gateway_id TEXT NOT NULL,
                model_id   TEXT NOT NULL,
                owner      TEXT,
                api_created INTEGER,
                first_seen INTEGER NOT NULL,
                last_seen  INTEGER NOT NULL,
                removed_at INTEGER,
                UNIQUE(gateway_id, model_id)
            );
            CREATE INDEX IF NOT EXISTS idx_models_gw ON models(gateway_id, model_id);
            CREATE INDEX IF NOT EXISTS idx_models_first ON models(first_seen);

            CREATE TABLE IF NOT EXISTS tags (
                id      INTEGER PRIMARY KEY AUTOINCREMENT,
                name    TEXT NOT NULL UNIQUE,
                builtin INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS gateway_tags (
                gateway_id TEXT NOT NULL,
                tag_id     INTEGER NOT NULL,
                PRIMARY KEY (gateway_id, tag_id)
            );

            CREATE TABLE IF NOT EXISTS model_tags (
                model_pk INTEGER NOT NULL,
                tag_id   INTEGER NOT NULL,
                PRIMARY KEY (model_pk, tag_id)
            );

            CREATE TABLE IF NOT EXISTS changelog (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                gateway_id TEXT NOT NULL,
                model_id   TEXT NOT NULL,
                change     TEXT NOT NULL,
                at         INTEGER NOT NULL,
                ack        INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_changelog_at ON changelog(at DESC);

            CREATE TABLE IF NOT EXISTS gateway_status (
                gateway_id      TEXT PRIMARY KEY,
                last_success_at INTEGER,
                last_error_at   INTEGER,
                last_error      TEXT
            );",
        )?;
        // Built-in tags drive favorites and hiding; they are protected in the UI.
        for name in ["favorite", IGNORE] {
            conn.execute(
                "INSERT OR IGNORE INTO tags (name, builtin) VALUES (?1, 1)",
                [name],
            )?;
        }
        Ok(())
    }

    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Apply a fetched model list against the cache and record the changelog.
    pub fn apply_fetch(
        &self,
        gateway_id: &str,
        items: &[ModelItem],
        now: i64,
    ) -> Result<DiffSummary, String> {
        // Dedupe fetched ids (some gateways return duplicates).
        let mut fetched: BTreeMap<String, &ModelItem> = BTreeMap::new();
        for it in items {
            if !it.id.is_empty() {
                fetched.insert(it.id.clone(), it);
            }
        }

        let mut conn = self.lock();
        let tx = conn.transaction().map_err(db)?;
        let existing: HashMap<String, Option<i64>> = {
            let mut stmt = tx
                .prepare("SELECT model_id, removed_at FROM models WHERE gateway_id = ?1")
                .map_err(db)?;
            let rows = stmt
                .query_map([gateway_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
                })
                .map_err(db)?;
            rows.filter_map(|r| r.ok()).collect()
        };

        let mut summary = DiffSummary { total: fetched.len(), ..Default::default() };
        let ids: std::collections::HashSet<String> = fetched.keys().cloned().collect();
        let d = crate::diff::compute(&existing, &ids);

        if existing.is_empty() {
            // First-ever fetch: an import. Full catalog in-app, no changelog, no toast.
            summary.imported = true;
            for it in fetched.values() {
                tx.execute(
                    "INSERT INTO models (gateway_id, model_id, owner, api_created, first_seen, last_seen)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    rusqlite::params![gateway_id, it.id, it.owner, it.api_created, now],
                )
                .map_err(db)?;
            }
        } else {
            for id in &d.added {
                let it = fetched[id];
                tx.execute(
                    "INSERT INTO models (gateway_id, model_id, owner, api_created, first_seen, last_seen)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    rusqlite::params![gateway_id, id, it.owner, it.api_created, now],
                )
                .map_err(db)?;
                tx.execute(
                    "INSERT INTO changelog (gateway_id, model_id, change, at) VALUES (?1, ?2, 'added', ?3)",
                    rusqlite::params![gateway_id, id, now],
                )
                .map_err(db)?;
            }
            for id in &d.removed {
                tx.execute(
                    "UPDATE models SET removed_at = ?1 WHERE gateway_id = ?2 AND model_id = ?3 AND removed_at IS NULL",
                    rusqlite::params![now, gateway_id, id],
                )
                .map_err(db)?;
                tx.execute(
                    "INSERT INTO changelog (gateway_id, model_id, change, at) VALUES (?1, ?2, 'removed', ?3)",
                    rusqlite::params![gateway_id, id, now],
                )
                .map_err(db)?;
            }
            for id in &d.returned {
                tx.execute(
                    "UPDATE models SET removed_at = NULL WHERE gateway_id = ?1 AND model_id = ?2",
                    rusqlite::params![gateway_id, id],
                )
                .map_err(db)?;
                tx.execute(
                    "INSERT INTO changelog (gateway_id, model_id, change, at) VALUES (?1, ?2, 'returned', ?3)",
                    rusqlite::params![gateway_id, id, now],
                )
                .map_err(db)?;
            }
        }

        // Refresh last_seen / owner for everything still present.
        {
            let mut stmt = tx
                .prepare(
                    "UPDATE models SET last_seen = ?1,
                        owner = COALESCE(?2, owner),
                        api_created = COALESCE(?3, api_created)
                     WHERE gateway_id = ?4 AND model_id = ?5",
                )
                .map_err(db)?;
            for it in fetched.values() {
                stmt.execute(rusqlite::params![now, it.owner, it.api_created, gateway_id, it.id])
                    .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        summary.added = d.added;
        summary.removed = d.removed;
        summary.returned = d.returned;
        Ok(summary)
    }

    // ---- catalog queries -------------------------------------------------

    pub fn all_models(&self) -> Result<Vec<ModelRow>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT pk, gateway_id, model_id, owner, first_seen, last_seen, removed_at
                 FROM models ORDER BY model_id COLLATE NOCASE",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(ModelRow {
                    pk: r.get(0)?,
                    gateway_id: r.get(1)?,
                    model_id: r.get(2)?,
                    owner: r.get(3)?,
                    first_seen: r.get(4)?,
                    last_seen: r.get(5)?,
                    removed_at: r.get(6)?,
                })
            })
            .map_err(db)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// tag_id -> display name, all tags (built-ins first).
    pub fn tags(&self) -> Result<Vec<Tag>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, name, builtin FROM tags ORDER BY builtin DESC, name COLLATE NOCASE")
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Tag { id: r.get(0)?, name: r.get(1)?, builtin: r.get::<_, i64>(2)? != 0 })
            })
            .map_err(db)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Raw (model_pk, tag_id, tag_name) links; caller builds maps.
    pub fn model_tag_links(&self) -> Result<Vec<(i64, i64, String)>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT mt.model_pk, mt.tag_id, t.name FROM model_tags mt JOIN tags t ON t.id = mt.tag_id",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))
            .map_err(db)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// gateway_id -> tag ids.
    pub fn gateway_tags(&self) -> Result<HashMap<String, Vec<i64>>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT gateway_id, tag_id FROM gateway_tags")
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(db)?;
        let mut map: HashMap<String, Vec<i64>> = HashMap::new();
        for r in rows.filter_map(|r| r.ok()) {
            map.entry(r.0).or_default().push(r.1);
        }
        Ok(map)
    }

    /// Sidebar count: visible (non-removed, non-hidden) models carrying the tag.
    pub fn tag_model_count(&self, tag_id: i64) -> Result<i64, String> {
        let conn = self.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM model_tags mt
             JOIN models m ON m.pk = mt.model_pk
             WHERE mt.tag_id = ?1 AND m.removed_at IS NULL
               AND m.pk NOT IN (
                   SELECT mt2.model_pk FROM model_tags mt2
                   JOIN tags t2 ON t2.id = mt2.tag_id WHERE t2.name = 'ignore')",
            [tag_id],
            |r| r.get(0),
        )
        .map_err(db)
    }

    // ---- changelog --------------------------------------------------------

    pub fn changelog(&self, limit: usize) -> Result<Vec<ChangeEntry>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT gateway_id, model_id, change, at, ack FROM changelog
                 ORDER BY at DESC, id DESC LIMIT ?1",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map([limit as i64], |r| {
                Ok(ChangeEntry {
                    gateway_id: r.get(0)?,
                    model_id: r.get(1)?,
                    change: r.get(2)?,
                    at: r.get(3)?,
                    ack: r.get::<_, i64>(4)? != 0,
                })
            })
            .map_err(db)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn unacked_count(&self) -> Result<i64, String> {
        let conn = self.lock();
        conn.query_row("SELECT COUNT(*) FROM changelog WHERE ack = 0", [], |r| r.get(0))
            .map_err(db)
    }

    pub fn ack_all(&self) -> Result<(), String> {
        let conn = self.lock();
        conn.execute("UPDATE changelog SET ack = 1 WHERE ack = 0", []).map_err(db)?;
        Ok(())
    }

    // ---- gateway health ---------------------------------------------------

    pub fn gateway_status(&self, gateway_id: &str) -> GatewayStatus {
        let conn = self.lock();
        conn.query_row(
            "SELECT last_success_at, last_error_at, last_error FROM gateway_status WHERE gateway_id = ?1",
            [gateway_id],
            |r| {
                Ok(GatewayStatus {
                    last_success_at: r.get(0)?,
                    last_error_at: r.get(1)?,
                    last_error: r.get(2)?,
                })
            },
        )
        .unwrap_or_default()
    }

    pub fn set_gateway_ok(&self, gateway_id: &str, now: i64) {
        let conn = self.lock();
        let _ = conn.execute(
            "INSERT INTO gateway_status (gateway_id, last_success_at) VALUES (?1, ?2)
             ON CONFLICT(gateway_id) DO UPDATE SET last_success_at = ?2",
            rusqlite::params![gateway_id, now],
        );
    }

    pub fn set_gateway_error(&self, gateway_id: &str, err: &str, now: i64) {
        let conn = self.lock();
        let _ = conn.execute(
            "INSERT INTO gateway_status (gateway_id, last_error_at, last_error) VALUES (?1, ?2, ?3)
             ON CONFLICT(gateway_id) DO UPDATE SET last_error_at = ?2, last_error = ?3",
            rusqlite::params![gateway_id, now, err],
        );
    }

    // ---- deletion ---------------------------------------------------------

    /// Remove every trace of a gateway (its models keep nothing behind).
    pub fn delete_gateway_data(&self, gateway_id: &str) -> Result<(), String> {
        let conn = self.lock();
        conn.execute_batch(&format!(
            "BEGIN;
             DELETE FROM model_tags WHERE model_pk IN (SELECT pk FROM models WHERE gateway_id = '{gid}');
             DELETE FROM models WHERE gateway_id = '{gid}';
             DELETE FROM gateway_tags WHERE gateway_id = '{gid}';
             DELETE FROM gateway_status WHERE gateway_id = '{gid}';
             DELETE FROM changelog WHERE gateway_id = '{gid}';
             COMMIT;",
            gid = gateway_id.replace('\'', "''"),
        ))
        .map_err(db)?;
        Ok(())
    }

    /// Total visible (non-removed, non-hidden) model count for the status bar.
    pub fn visible_count(&self) -> Result<i64, String> {
        let conn = self.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM models m WHERE m.removed_at IS NULL
               AND m.pk NOT IN (
                   SELECT mt.model_pk FROM model_tags mt
                   JOIN tags t ON t.id = mt.tag_id WHERE t.name = 'ignore')",
            [],
            |r| r.get(0),
        )
        .map_err(db)
    }

    pub fn count_new(&self, cutoff: i64) -> Result<i64, String> {
        let conn = self.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM models m WHERE m.removed_at IS NULL AND m.first_seen >= ?1
               AND m.pk NOT IN (
                   SELECT mt.model_pk FROM model_tags mt
                   JOIN tags t ON t.id = mt.tag_id WHERE t.name = 'ignore')",
            [cutoff],
            |r| r.get(0),
        )
        .map_err(db)
    }

    pub fn count_removed(&self) -> Result<i64, String> {
        let conn = self.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM models m WHERE m.removed_at IS NOT NULL
               AND m.pk NOT IN (
                   SELECT mt.model_pk FROM model_tags mt
                   JOIN tags t ON t.id = mt.tag_id WHERE t.name = 'ignore')",
            [],
            |r| r.get(0),
        )
        .map_err(db)
    }

    pub fn count_hidden(&self) -> Result<i64, String> {
        let conn = self.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM model_tags mt JOIN tags t ON t.id = mt.tag_id
             WHERE t.name = 'ignore'",
            [],
            |r| r.get(0),
        )
        .map_err(db)
    }
}

/// Deterministic pleasant color for a tag chip.
pub fn tag_color(name: &str) -> egui::Color32 {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    let hue = (h % 360) as f32;
    let (r, g, b) = hsl_to_rgb(hue, 0.55, 0.45);
    egui::Color32::from_rgb(r, g, b)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}
