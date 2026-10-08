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
            .context("catalog URL is not configured; use --from DIRECTORY, --url or config.toml catalog_url")?;
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

    /// Import already downloaded release assets without network access or credentials.
    pub fn sync_catalog_from_dir(&self, directory: &std::path::Path) -> Result<Manifest> {
        let read = |name: &str, limit: u64| -> Result<Vec<u8>> {
            let mut options = fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NONBLOCK);
            }
            let file = options
                .open(directory.join(name))
                .with_context(|| format!("cannot open Catalog asset {name}"))?;
            if !file.metadata()?.is_file() {
                bail!("Catalog asset {name} must be a regular file");
            }
            let mut bytes = Vec::new();
            file.take(limit + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > limit {
                bail!("Catalog asset {name} is too large");
            }
            Ok(bytes)
        };
        let manifest: Manifest = serde_json::from_slice(&read("manifest.json", 1024 * 1024)?)?;
        if manifest.schema_version != cliary_catalog::SCHEMA_VERSION {
            bail!("unsupported catalog schema version");
        }
        if manifest.version <= self.catalog_version()? {
            return Ok(manifest);
        }
        self.install_catalog_bytes(&manifest, &read("catalog.db.zst", 64 * 1024 * 1024)?)?;
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
    #[test]
    fn offline_sync_validates_assets_and_preserves_personal_database() {
        let root = tempfile::tempdir().unwrap();
        let core = Cliary::at(Paths::new(
            root.path().join("config"),
            root.path().join("data"),
            root.path().join("cache"),
        ))
        .unwrap();
        core.set_favorite("ncdu", true).unwrap();
        let personal = fs::read(&core.paths.user_db).unwrap();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalog");
        let output = root.path().join("assets");
        cliary_catalog::publish(&source, &output, 2).unwrap();
        let original = fs::read(output.join("catalog.db.zst")).unwrap();
        fs::write(output.join("catalog.db.zst"), b"corrupt").unwrap();
        assert!(
            core.sync_catalog_from_dir(&output)
                .unwrap_err()
                .to_string()
                .contains("checksum")
        );
        assert_eq!(core.catalog_version().unwrap(), 1);
        fs::write(output.join("catalog.db.zst"), original).unwrap();
        assert_eq!(core.sync_catalog_from_dir(&output).unwrap().version, 2);
        assert_eq!(core.catalog_version().unwrap(), 2);
        // Repeating an already installed version needs no download or database write.
        fs::remove_file(output.join("catalog.db.zst")).unwrap();
        assert_eq!(core.sync_catalog_from_dir(&output).unwrap().version, 2);
        assert_eq!(fs::read(&core.paths.user_db).unwrap(), personal);
    }

    #[test]
    fn offline_sync_rejects_large_and_non_regular_manifests() {
        let root = tempfile::tempdir().unwrap();
        let core = Cliary::at(Paths::new(
            root.path().join("config"),
            root.path().join("data"),
            root.path().join("cache"),
        ))
        .unwrap();
        let output = root.path().join("assets");
        fs::create_dir_all(output.join("manifest.json")).unwrap();
        assert!(core.sync_catalog_from_dir(&output).is_err());
        fs::remove_dir(output.join("manifest.json")).unwrap();
        fs::write(output.join("manifest.json"), vec![b' '; 1024 * 1024 + 1]).unwrap();
        assert!(
            core.sync_catalog_from_dir(&output)
                .unwrap_err()
                .to_string()
                .contains("too large")
        );
        assert_eq!(core.catalog_version().unwrap(), 1);
    }
}
