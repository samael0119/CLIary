use anyhow::{Context, Result, bail};
use cliary_core::Cliary;
use std::{
    fs,
    path::{Path, PathBuf},
};

const BEGIN: &str = "# >>> CLIary shell integration >>>";
const END: &str = "# <<< CLIary shell integration <<<";

pub fn current_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .and_then(|x| {
            std::path::Path::new(&x)
                .file_name()
                .map(|x| x.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "bash".into())
}

pub fn configure(core: &Cliary, shell: &str, enable: bool) -> Result<()> {
    let (name, source, rc) = match shell {
        "bash" => (
            "bash",
            include_str!("../shell/bash.sh"),
            home()?.join(".bashrc"),
        ),
        "zsh" => (
            "zsh",
            include_str!("../shell/zsh.sh"),
            home()?.join(".zshrc"),
        ),
        "fish" => (
            "fish",
            include_str!("../shell/fish.fish"),
            home()?.join(".config/fish/config.fish"),
        ),
        _ => bail!("supported shells: bash, zsh, fish"),
    };
    let hook_dir = core.paths.config_dir.join("shell");
    fs::create_dir_all(&hook_dir)?;
    let hook = hook_dir.join(name);
    if enable {
        let binary = std::env::current_exe()?;
        fs::write(
            &hook,
            source.replace(
                "@CLIARY_BIN@",
                &format!("'{}'", shell_quote(&binary.to_string_lossy())),
            ),
        )?;
    }
    if let Some(parent) = rc.parent() {
        fs::create_dir_all(parent)?;
    }
    let current = fs::read_to_string(&rc).unwrap_or_default();
    let mut updated = remove_block(&current)?;
    if enable {
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(BEGIN);
        updated.push('\n');
        updated.push_str(&format!(
            "source '{}'\n",
            shell_quote(&hook.to_string_lossy())
        ));
        updated.push_str(END);
        updated.push('\n');
    }
    if updated != current {
        if rc.exists() {
            let backup = backup_path(&rc);
            if !backup.exists() {
                fs::copy(&rc, backup)?;
            }
        }
        let temp = rc.with_extension("cliary.tmp");
        fs::write(&temp, updated)?;
        if rc.exists() {
            fs::set_permissions(&temp, fs::metadata(&rc)?.permissions())?;
        }
        fs::rename(temp, &rc)?;
    }
    if !enable {
        let _ = fs::remove_file(hook);
    }
    Ok(())
}

fn home() -> Result<PathBuf> {
    dirs::home_dir().context("home directory unavailable")
}
fn backup_path(rc: &Path) -> PathBuf {
    PathBuf::from(format!("{}.cliary.bak", rc.display()))
}
fn shell_quote(path: &str) -> String {
    path.replace('\'', "'\\''")
}
fn remove_block(value: &str) -> Result<String> {
    let mut result = String::new();
    let mut inside = false;
    for line in value.lines() {
        if line == BEGIN {
            if inside {
                bail!("nested CLIary shell block")
            };
            inside = true;
            continue;
        }
        if line == END {
            if !inside {
                bail!("unmatched CLIary shell block")
            };
            inside = false;
            continue;
        }
        if !inside {
            result.push_str(line);
            result.push('\n');
        }
    }
    if inside {
        bail!("unfinished CLIary shell block")
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_block_removes_without_touching_user_lines() {
        let input = format!(
            "export PATH=/custom/bin\n{BEGIN}\nsource '/tmp/hook'\n{END}\nalias ll='ls -l'\n"
        );
        assert_eq!(
            remove_block(&input).unwrap(),
            "export PATH=/custom/bin\nalias ll='ls -l'\n"
        );
        assert!(remove_block(BEGIN).is_err());
    }
}
