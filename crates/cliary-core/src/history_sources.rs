//! Candidate discovery inspects filenames/metadata only, never command contents.
use crate::HistoryFormat;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct HistoryCandidate {
    pub shell: HistoryFormat,
    pub path: PathBuf,
}

pub fn history_candidates() -> Result<Vec<HistoryCandidate>> {
    let home = dirs::home_dir().context("home directory unavailable")?;
    let shell = std::env::var("SHELL").unwrap_or_default();
    let shell = Path::new(&shell)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("");
    let env_path = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let histfile = env_path("HISTFILE");
    let zdotdir = env_path("ZDOTDIR");
    let data = env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
    Ok(find_candidates(
        &home,
        shell,
        histfile.as_deref(),
        zdotdir.as_deref(),
        &data,
    ))
}

fn find_candidates(
    home: &Path,
    shell: &str,
    histfile: Option<&Path>,
    zdotdir: Option<&Path>,
    data: &Path,
) -> Vec<HistoryCandidate> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut add = |format, path: PathBuf| {
        if path.metadata().is_ok_and(|m| m.is_file()) {
            let identity = path.canonicalize().unwrap_or_else(|_| path.clone());
            if seen.insert(identity) {
                result.push(HistoryCandidate {
                    shell: format,
                    path,
                });
            }
        }
    };
    if let Some(file) = histfile.filter(|p| !p.as_os_str().is_empty()) {
        match shell {
            "zsh" => add(HistoryFormat::Zsh, file.to_owned()),
            "bash" => add(HistoryFormat::Bash, file.to_owned()),
            _ => (),
        }
    }
    if let Some(dir) = zdotdir.filter(|p| !p.as_os_str().is_empty()) {
        add(HistoryFormat::Zsh, dir.join(".zsh_history"));
    }
    add(HistoryFormat::Zsh, home.join(".zsh_history"));
    add(HistoryFormat::Bash, home.join(".bash_history"));
    add(HistoryFormat::Fish, data.join("fish/fish_history"));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidates_respect_custom_paths_deduplicate_and_ignore_directories() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path();
        let custom = home.join("custom");
        std::fs::write(&custom, "not opened by discovery").unwrap();
        std::fs::write(home.join(".bash_history"), "").unwrap();
        std::fs::create_dir(home.join(".zsh_history")).unwrap();
        let fish = home.join("fish/fish_history");
        std::fs::create_dir_all(fish.parent().unwrap()).unwrap();
        std::fs::write(&fish, "").unwrap();
        let found = find_candidates(home, "zsh", Some(&custom), None, home);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].path, custom);
        assert_eq!(found[1].shell.name(), "bash");
        assert_eq!(found[2].path, fish);
        assert_eq!(
            find_candidates(home, "bash", Some(&home.join(".bash_history")), None, home).len(),
            2
        );
        assert_eq!(
            find_candidates(home, "fish", Some(&custom), None, home).len(),
            2
        );
    }
}
