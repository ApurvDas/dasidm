//! SQLite persistence: each download is one JSON row; settings are one JSON value.

use crate::model::Item;
use crate::settings::Settings;
use rusqlite::{Connection, params};
use std::path::Path;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA cache_size=-512;
             CREATE TABLE IF NOT EXISTS downloads (id TEXT PRIMARY KEY, added_at INTEGER NOT NULL, json TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        Ok(Self { conn })
    }

    pub fn load_items(&self) -> rusqlite::Result<Vec<Item>> {
        let mut stmt = self.conn.prepare("SELECT json FROM downloads ORDER BY added_at")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).filter_map(|j| serde_json::from_str(&j).ok()).collect())
    }

    pub fn save_item(&self, item: &Item) -> rusqlite::Result<()> {
        let json = serde_json::to_string(item).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO downloads (id, added_at, json) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json",
            params![item.id, item.added_at, json],
        )?;
        Ok(())
    }

    /// Save many rows in one transaction (checkpoints).
    pub fn save_items<'a>(&mut self, items: impl IntoIterator<Item = &'a Item>) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO downloads (id, added_at, json) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET json = excluded.json",
            )?;
            for item in items {
                let json = serde_json::to_string(item).unwrap_or_default();
                stmt.execute(params![item.id, item.added_at, json])?;
            }
        }
        tx.commit()
    }

    pub fn delete_items(&mut self, ids: &[String]) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        for id in ids {
            tx.execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
        }
        tx.commit()
    }

    pub fn load_settings(&self) -> Settings {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = 'settings'", [], |r| r.get::<_, String>(0))
            .ok()
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES ('settings', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![serde_json::to_string(s).unwrap_or_default()],
        )?;
        Ok(())
    }
}
