use crate::{Cliary, Tool};
use anyhow::{Result, bail};
use rand::RngCore;
use rusqlite::{OptionalExtension, params};

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
        let tool = self
            .get_tool(tool)?
            .ok_or_else(|| anyhow::anyhow!("unknown tool"))?;
        let db = self.user_db()?;
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

    pub fn favorites(&self) -> Result<Vec<Tool>> {
        let db = self.user_db()?;
        let mut stmt = db.prepare("SELECT tool_id FROM favorites ORDER BY created_at DESC")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.into_iter()
            .map(|id| {
                self.get_tool(&id)?
                    .ok_or_else(|| anyhow::anyhow!("catalog no longer contains favorite {id}"))
            })
            .collect()
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
