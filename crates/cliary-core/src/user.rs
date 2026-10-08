use crate::{Cliary, Tool};
use anyhow::{Result, bail};
use rand::RngCore;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A saved bookmark, including entries no longer present in the current Catalog.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FavoriteEntry {
    pub id: String,
    pub tool: Option<Tool>,
}

impl Cliary {
    pub(crate) fn init_user(&self) -> Result<()> {
        let db = self.user_db()?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > 1 {
            bail!("user database was created by a newer CLIary version");
        }
        db.execute_batch("PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS favorites(tool_id TEXT PRIMARY KEY, created_at INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS notes(tool_id TEXT PRIMARY KEY, body TEXT NOT NULL, updated_at INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS installed(executable TEXT PRIMARY KEY, path TEXT NOT NULL, version TEXT, source TEXT, tool_id TEXT, scanned_at INTEGER NOT NULL);
            CREATE INDEX IF NOT EXISTS idx_installed_tool ON installed(tool_id);
            CREATE TABLE IF NOT EXISTS usage_events(id INTEGER PRIMARY KEY, executable TEXT NOT NULL, tool_id TEXT, timestamp INTEGER NOT NULL, machine_id TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS idx_usage_time ON usage_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_usage_tool ON usage_events(tool_id,timestamp);
            CREATE INDEX IF NOT EXISTS idx_usage_exe ON usage_events(executable,timestamp);
            PRAGMA user_version=1;")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.paths.user_db, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn machine_id(&self) -> Result<String> {
        let db = self.user_db()?;
        if let Some(value) = db
            .query_row("SELECT value FROM meta WHERE key='machine_id'", [], |row| {
                row.get(0)
            })
            .optional()?
        {
            return Ok(value);
        }
        let mut random = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut random);
        let id = hex::encode(random);
        db.execute(
            "INSERT OR IGNORE INTO meta VALUES ('machine_id', ?1)",
            [&id],
        )?;
        Ok(
            db.query_row("SELECT value FROM meta WHERE key='machine_id'", [], |row| {
                row.get(0)
            })?,
        )
    }

    pub fn set_favorite(&self, tool: &str, favorite: bool) -> Result<()> {
        let db = self.user_db()?;
        // Saved IDs take priority over current aliases. Removing a retired entry
        // must not accidentally remove a different tool that acquired its name.
        if !favorite && db.execute("DELETE FROM favorites WHERE tool_id=?1", [tool])? > 0 {
            return Ok(());
        }
        let tool = self.get_tool(tool)?;
        let Some(tool) = tool else {
            if favorite {
                bail!("unknown tool");
            }
            return Ok(());
        };
        if favorite {
            db.execute(
                "INSERT OR IGNORE INTO favorites VALUES (?1,?2)",
                params![tool.id, chrono::Utc::now().timestamp()],
            )?;
        } else {
            db.execute("DELETE FROM favorites WHERE tool_id=?1", [&tool.id])?;
        }
        Ok(())
    }

    /// Remove a persisted ID without resolving current aliases. Safe to retry,
    /// even when a new Catalog entry uses the retired ID as an alias.
    pub fn remove_favorite(&self, id: &str) -> Result<()> {
        self.user_db()?
            .execute("DELETE FROM favorites WHERE tool_id=?1", [id])?;
        Ok(())
    }

    /// Return every saved bookmark. Metadata is resolved by exact Catalog ID;
    /// aliases are for user input, not persisted identities.
    pub fn favorite_entries(&self) -> Result<Vec<FavoriteEntry>> {
        let db = self.user_db()?;
        let mut stmt =
            db.prepare("SELECT tool_id FROM favorites ORDER BY created_at DESC, tool_id ASC")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut tools: HashMap<_, _> = self
            .all_tools()?
            .into_iter()
            .map(|tool| (tool.id.clone(), tool))
            .collect();
        Ok(ids
            .into_iter()
            .map(|id| FavoriteEntry {
                tool: tools.remove(&id),
                id,
            })
            .collect())
    }

    /// Compatibility API: only bookmarks with current Catalog metadata.
    /// Use `favorite_entries` to display or manage all saved bookmarks.
    pub fn favorites(&self) -> Result<Vec<Tool>> {
        Ok(self
            .favorite_entries()?
            .into_iter()
            .filter_map(|entry| entry.tool)
            .collect())
    }

    pub fn save_note(&self, tool: &str, body: &str) -> Result<()> {
        let tool = self
            .get_tool(tool)?
            .ok_or_else(|| anyhow::anyhow!("unknown tool"))?;
        if body.len() > 64 * 1024 {
            bail!("note exceeds 64 KiB");
        }
        let db = self.user_db()?;
        if body.trim().is_empty() {
            db.execute("DELETE FROM notes WHERE tool_id=?1", [&tool.id])?;
        } else {
            db.execute("INSERT INTO notes VALUES (?1,?2,?3) ON CONFLICT(tool_id) DO UPDATE SET body=excluded.body,updated_at=excluded.updated_at", params![tool.id, body, chrono::Utc::now().timestamp()])?;
        }
        Ok(())
    }
}
