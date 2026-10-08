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

    #[cfg(unix)]
    #[test]
    fn zsh_hook_uses_expanded_executable_without_passing_arguments_or_functions() {
        use std::{os::unix::fs::PermissionsExt, process::Command};
        if Command::new("zsh").arg("--version").output().is_err() {
            eprintln!("Zsh not available; hook integration test not exercised");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let recorder = root.path().join("fake recorder");
        let log = root.path().join("recorded");
        fs::write(
            &recorder,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CLIARY_TEST_LOG\"\n",
        )
        .unwrap();
        fs::set_permissions(&recorder, fs::Permissions::from_mode(0o700)).unwrap();
        let hook = root.path().join("hook");
        fs::write(
            &hook,
            include_str!("../shell/zsh.sh").replace(
                "@CLIARY_BIN@",
                &format!("'{}'", shell_quote(&recorder.to_string_lossy())),
            ),
        )
        .unwrap();
        let script = format!(
            "source '{}'\n__cliary_zsh_preexec 'gst SECRET' 'git status SECRET' 'git status SECRET'\n__cliary_zsh_preexec 'gco SECRET' 'git checkout SECRET' 'git checkout SECRET'\n__cliary_zsh_preexec 'print SECRET' 'print SECRET' 'print SECRET'\ngit() {{ :; }}\n__cliary_zsh_preexec 'gst SECRET' 'git status SECRET' 'git status SECRET'\n",
            shell_quote(&hook.to_string_lossy())
        );
        let result = Command::new("zsh")
            .args(["-f", "-c", &script])
            .env("CLIARY_TEST_LOG", &log)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let mut lines = Vec::new();
        for _ in 0..100 {
            lines = fs::read_to_string(&log)
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect();
            if lines.len() >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        lines.sort();
        assert_eq!(
            lines,
            vec![
                "internal record gco --resolved-executable git",
                "internal record gst --resolved-executable git"
            ]
        );
    }
}
