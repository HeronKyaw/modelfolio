//! Tag helpers: creation, renaming, deletion, assignment and sidebar counts.
//!
//! Tags are the only grouping primitive: a "group" is just a tag you filter on.
//! Built-in tags `favorite` and `ignore` are protected and drive the star / hide features.

use crate::store::cache::Cache;
use rusqlite::Connection;

pub const FAVORITE: &str = "favorite";
pub const IGNORE: &str = "ignore";

pub fn builtin_id(conn: &Connection, name: &str) -> Option<i64> {
    conn.query_row(
        "SELECT id FROM tags WHERE name = ?1 AND builtin = 1",
        [name],
        |r| r.get(0),
    )
    .ok()
}

impl Cache {
    pub fn create_tag(&self, name: &str) -> Result<i64, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Tag name is required".into());
        }
        let name = normalize(name);
        let conn = self.lock();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tags WHERE name = ?1 COLLATE NOCASE)",
                [&name],
                |r| r.get(0),
            )
            .map_err(db)?;
        if exists {
            return Err(format!("Tag '{}' already exists", capitalize(&name)));
        }
        conn.execute("INSERT INTO tags (name, builtin) VALUES (?1, 0)", [&name])
            .map_err(db)?;
        Ok(conn.last_insert_rowid())
    }

    pub fn rename_tag(&self, tag_id: i64, new_name: &str) -> Result<(), String> {
        let name = normalize(new_name.trim());
        if name.is_empty() {
            return Err("Tag name is required".into());
        }
        let conn = self.lock();
        let builtin: bool = conn
            .query_row("SELECT builtin FROM tags WHERE id = ?1", [tag_id], |r| r.get(0))
            .map_err(db)?;
        if builtin {
            return Err("Built-in tags cannot be renamed".into());
        }
        let taken: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tags WHERE name = ?1 COLLATE NOCASE AND id != ?2)",
                rusqlite::params![name, tag_id],
                |r| r.get(0),
            )
            .map_err(db)?;
        if taken {
            return Err(format!("Tag '{}' already exists", capitalize(&name)));
        }
        conn.execute("UPDATE tags SET name = ?1 WHERE id = ?2", rusqlite::params![name, tag_id])
            .map_err(db)?;
        Ok(())
    }

    /// Deleting a tag unassigns it; it never deletes gateways or models.
    pub fn delete_tag(&self, tag_id: i64) -> Result<(), String> {
        let conn = self.lock();
        let builtin: bool = conn
            .query_row("SELECT builtin FROM tags WHERE id = ?1", [tag_id], |r| r.get(0))
            .map_err(db)?;
        if builtin {
            return Err("Built-in tags cannot be deleted".into());
        }
        conn.execute("DELETE FROM model_tags WHERE tag_id = ?1", [tag_id]).map_err(db)?;
        conn.execute("DELETE FROM gateway_tags WHERE tag_id = ?1", [tag_id]).map_err(db)?;
        conn.execute("DELETE FROM tags WHERE id = ?1", [tag_id]).map_err(db)?;
        Ok(())
    }

    pub fn set_model_tag(&self, model_pk: i64, tag_id: i64, on: bool) -> Result<(), String> {
        let conn = self.lock();
        if on {
            conn.execute(
                "INSERT OR IGNORE INTO model_tags (model_pk, tag_id) VALUES (?1, ?2)",
                rusqlite::params![model_pk, tag_id],
            )
            .map_err(db)?;
        } else {
            conn.execute(
                "DELETE FROM model_tags WHERE model_pk = ?1 AND tag_id = ?2",
                rusqlite::params![model_pk, tag_id],
            )
            .map_err(db)?;
        }
        Ok(())
    }

    pub fn set_gateway_tag(&self, gateway_id: &str, tag_id: i64, on: bool) -> Result<(), String> {
        let conn = self.lock();
        if on {
            conn.execute(
                "INSERT OR IGNORE INTO gateway_tags (gateway_id, tag_id) VALUES (?1, ?2)",
                rusqlite::params![gateway_id, tag_id],
            )
            .map_err(db)?;
        } else {
            conn.execute(
                "DELETE FROM gateway_tags WHERE gateway_id = ?1 AND tag_id = ?2",
                rusqlite::params![gateway_id, tag_id],
            )
            .map_err(db)?;
        }
        Ok(())
    }

    pub fn set_favorite(&self, model_pk: i64, on: bool) -> Result<(), String> {
        let conn = self.lock();
        if let Some(id) = builtin_id(&conn, FAVORITE) {
            if on {
                conn.execute(
                    "INSERT OR IGNORE INTO model_tags (model_pk, tag_id) VALUES (?1, ?2)",
                    rusqlite::params![model_pk, id],
                )
                .map_err(db)?;
            } else {
                conn.execute(
                    "DELETE FROM model_tags WHERE model_pk = ?1 AND tag_id = ?2",
                    rusqlite::params![model_pk, id],
                )
                .map_err(db)?;
            }
        }
        Ok(())
    }

    pub fn set_hidden(&self, model_pk: i64, on: bool) -> Result<(), String> {
        let conn = self.lock();
        if let Some(id) = builtin_id(&conn, IGNORE) {
            if on {
                conn.execute(
                    "INSERT OR IGNORE INTO model_tags (model_pk, tag_id) VALUES (?1, ?2)",
                    rusqlite::params![model_pk, id],
                )
                .map_err(db)?;
            } else {
                conn.execute(
                    "DELETE FROM model_tags WHERE model_pk = ?1 AND tag_id = ?2",
                    rusqlite::params![model_pk, id],
                )
                .map_err(db)?;
            }
        }
        Ok(())
    }
}

/// Case-insensitive-safe storage name: lowercase for matching consistency.
fn normalize(name: &str) -> String {
    name.chars().flat_map(|c| c.to_lowercase()).collect::<String>().trim().to_string()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub fn db(e: rusqlite::Error) -> String {
    e.to_string()
}
