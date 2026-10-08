mod catalog;
mod history;
mod history_import;
mod history_sources;
mod installed;
mod paths;
mod shell_alias;
mod sync;
mod user;
mod wrapped_insights;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub use catalog::{CompareRow, ToolDetail, ToolResult, compare_feature_label};
pub use cliary_catalog::{InstallMethod, LocalizedName, Manifest, Tool};
pub use history::{CountItem, HistorySummary, Stats, UsageEvent, Wrapped};
pub use history_import::{HistoryFormat, HistoryImportPlan, ImportReport, UndatedTool};
pub use history_sources::{HistoryCandidate, history_candidates};
pub use installed::InstalledTool;
pub use paths::Paths;
pub use shell_alias::AliasResolution;
pub use user::FavoriteEntry;
pub use wrapped_insights::{
    FavoriteInsight, NewToolInsight, ToolChange, WrappedInsights, YearComparison,
};

#[derive(Clone, Debug)]
pub struct Cliary {
    pub paths: Paths,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub language: Option<String>,
    pub catalog_url: Option<String>,
}

impl Cliary {
    pub fn open() -> Result<Self> {
        Self::at(Paths::discover()?)
    }

    pub fn at(paths: Paths) -> Result<Self> {
        paths.ensure()?;
        let this = Self { paths };
        this.init_user()?;
        this.init_catalog()?;
        Ok(this)
    }

    pub(crate) fn catalog_db(&self) -> Result<Connection> {
        let db = Connection::open_with_flags(
            &self.paths.catalog_db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .with_context(|| format!("opening {}", self.paths.catalog_db.display()))?;
        Ok(db)
    }

    pub(crate) fn user_db(&self) -> Result<Connection> {
        let db = Connection::open(&self.paths.user_db)?;
        db.busy_timeout(std::time::Duration::from_millis(500))?;
        Ok(db)
    }

    pub fn config(&self) -> Result<Config> {
        let path = self.paths.config_dir.join("config.toml");
        if !path.exists() {
            return Ok(Config::default());
        }
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }

    pub fn set_language(&self, language: &str) -> Result<()> {
        if !["en", "zh-CN"].contains(&language) {
            anyhow::bail!("language must be en or zh-CN");
        }
        let mut config = self.config()?;
        config.language = Some(language.to_string());
        let temp = self.paths.config_dir.join("config.toml.tmp");
        std::fs::write(&temp, toml::to_string_pretty(&config)?)?;
        restrict_file(&temp)?;
        std::fs::rename(temp, self.paths.config_dir.join("config.toml"))?;
        Ok(())
    }

    pub fn set_catalog_url(&self, url: &str) -> Result<()> {
        if !url.starts_with("https://") {
            anyhow::bail!("catalog URL must use HTTPS");
        }
        let mut config = self.config()?;
        config.catalog_url = Some(url.trim_end_matches('/').to_string());
        let temp = self.paths.config_dir.join("config.toml.tmp");
        std::fs::write(&temp, toml::to_string_pretty(&config)?)?;
        restrict_file(&temp)?;
        std::fs::rename(temp, self.paths.config_dir.join("config.toml"))?;
        Ok(())
    }

    pub fn locale(&self, override_lang: Option<&str>) -> Result<String> {
        if let Some(lang) = override_lang {
            return normalize_locale(lang);
        }
        if let Ok(lang) = std::env::var("CLIARY_LANG") {
            return normalize_locale(&lang);
        }
        if let Some(lang) = self.config()?.language {
            return normalize_locale(&lang);
        }
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(lang) = std::env::var(var) {
                return normalize_locale(&lang);
            }
        }
        Ok("en".into())
    }
}

fn restrict_file(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn normalize_locale(value: &str) -> Result<String> {
    let value = value.to_ascii_lowercase();
    if value.starts_with("zh") {
        Ok("zh-CN".into())
    } else {
        Ok("en".into())
    }
}

pub fn localized<'a>(map: &'a std::collections::BTreeMap<String, String>, lang: &str) -> &'a str {
    map.get(lang)
        .or_else(|| map.get("en"))
        .map(String::as_str)
        .unwrap_or("")
}
