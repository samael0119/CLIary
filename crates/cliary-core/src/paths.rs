use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub catalog_db: PathBuf,
    pub user_db: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Self> {
        let home = dirs::home_dir().context("cannot determine home directory")?;
        let config_dir = path(
            "CLIARY_CONFIG_DIR",
            "XDG_CONFIG_HOME",
            home.join(".config"),
            "cliary",
        );
        let data_dir = path(
            "CLIARY_DATA_DIR",
            "XDG_DATA_HOME",
            home.join(".local/share"),
            "cliary",
        );
        let cache_dir = path(
            "CLIARY_CACHE_DIR",
            "XDG_CACHE_HOME",
            home.join(".cache"),
            "cliary",
        );
        Ok(Self::new(config_dir, data_dir, cache_dir))
    }

    pub fn new(config_dir: PathBuf, data_dir: PathBuf, cache_dir: PathBuf) -> Self {
        let catalog_db = data_dir.join("catalog.db");
        let user_db = data_dir.join("user.db");
        Self {
            config_dir,
            data_dir,
            cache_dir,
            catalog_db,
            user_db,
        }
    }

    pub fn ensure(&self) -> Result<()> {
        for dir in [&self.config_dir, &self.data_dir, &self.cache_dir] {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        Ok(())
    }
}

fn path(override_name: &str, xdg_name: &str, default: PathBuf, suffix: &str) -> PathBuf {
    if let Some(value) = std::env::var_os(override_name) {
        return PathBuf::from(value);
    }
    std::env::var_os(xdg_name)
        .map(PathBuf::from)
        .unwrap_or(default)
        .join(suffix)
}
