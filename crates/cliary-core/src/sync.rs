use crate::{Cliary, Manifest};
use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::time::Duration;

impl Cliary {
    pub fn sync_catalog(&self, url_override: Option<&str>) -> Result<Manifest> {
        let configured = self.config()?.catalog_url;
        let base = url_override
            .or(configured.as_deref())
            .or(option_env!("CLIARY_CATALOG_URL"))
            .context("catalog URL is not configured; use --url or config.toml catalog_url")?;
        if !base.starts_with("https://") {
            bail!("catalog URL must use HTTPS");
        }
        let base = base.trim_end_matches('/');
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        let mut response = agent.get(&format!("{base}/manifest.json")).call()?;
        let mut manifest_bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut manifest_bytes)?;
        if manifest_bytes.len() > 1024 * 1024 {
            bail!("catalog manifest is too large");
        }
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
        if manifest.schema_version != cliary_catalog::SCHEMA_VERSION {
            bail!("unsupported catalog schema version");
        }
        if manifest.version <= self.catalog_version()? {
            return Ok(manifest);
        }
        let mut response = agent.get(&format!("{base}/catalog.db.zst")).call()?;
        let mut compressed = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut compressed)?;
        if compressed.len() > 64 * 1024 * 1024 {
            bail!("catalog download is too large");
        }
        self.install_catalog_bytes(&manifest, &compressed)?;
        Ok(manifest)
    }

    fn install_catalog_bytes(&self, manifest: &Manifest, compressed: &[u8]) -> Result<()> {
        if manifest.schema_version != cliary_catalog::SCHEMA_VERSION {
            bail!("unsupported catalog schema version");
        }
        if hex::encode(Sha256::digest(compressed)) != manifest.sha256 {
            bail!("catalog checksum mismatch");
        }
        let mut raw = Vec::new();
        zstd::stream::read::Decoder::new(compressed)?
            .take(256 * 1024 * 1024 + 1)
            .read_to_end(&mut raw)?;
        if raw.len() > 256 * 1024 * 1024 {
            bail!("catalog database is too large");
        }
        if !raw.starts_with(b"SQLite format 3\0") {
            bail!("invalid catalog database");
        }
        let temp = self.paths.data_dir.join("catalog.db.next");
        fs::write(&temp, &raw)?;
        let check = (|| -> Result<()> {
            // FTS5's integrity check may need a writable connection for its shadow tables.
            let db = Connection::open(&temp)?;
            let integrity: String = db.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
            if integrity != "ok" {
                bail!("catalog integrity check failed: {integrity}");
            }
            let schema: i64 = db
                .query_row(
                    "SELECT value FROM meta WHERE key='schema_version'",
                    [],
                    |row| row.get::<_, String>(0),
                )?
                .parse()?;
            let version: u64 = db
                .query_row("SELECT value FROM meta WHERE key='version'", [], |row| {
                    row.get::<_, String>(0)
                })?
                .parse()?;
            let count: usize = db.query_row("SELECT COUNT(*) FROM tools", [], |row| row.get(0))?;
            if schema != manifest.schema_version
                || version != manifest.version
                || count != manifest.tool_count
            {
                bail!("catalog metadata mismatch");
            }
            Ok(())
        })();
        if let Err(error) = check {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        fs::rename(&temp, &self.paths.catalog_db)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Paths;

    #[test]
    fn update_rejects_corruption_and_preserves_user_data() {
        let root = tempfile::tempdir().unwrap();
        let core = Cliary::at(Paths::new(
            root.path().join("config"),
            root.path().join("data"),
            root.path().join("cache"),
        ))
        .unwrap();
        core.set_favorite("ncdu", true).unwrap();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalog");
        let output = root.path().join("release");
        let manifest = cliary_catalog::publish(&source, &output, 2).unwrap();
        let mut bytes = fs::read(output.join("catalog.db.zst")).unwrap();
        bytes[10] ^= 0xff;
        assert!(core.install_catalog_bytes(&manifest, &bytes).is_err());
        assert_eq!(core.catalog_version().unwrap(), 1);
        let bytes = fs::read(output.join("catalog.db.zst")).unwrap();
        core.install_catalog_bytes(&manifest, &bytes).unwrap();
        assert_eq!(core.catalog_version().unwrap(), 2);
        assert!(core.tool_detail("ncdu").unwrap().unwrap().favorite);
    }
}
