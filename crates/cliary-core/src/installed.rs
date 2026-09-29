use crate::Cliary;
use anyhow::Result;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledTool {
    pub executable: String,
    pub path: String,
    pub version: Option<String>,
    pub source: Option<String>,
    pub tool_id: Option<String>,
}

impl Cliary {
    pub fn initial_scan_pending(&self) -> Result<bool> {
        let db = self.user_db()?;
        let state: Option<String> = db
            .query_row(
                "SELECT value FROM meta WHERE key='installed_scan_state'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(state.is_none())
    }

    pub fn has_scanned(&self) -> Result<bool> {
        let db = self.user_db()?;
        let state: Option<String> = db
            .query_row(
                "SELECT value FROM meta WHERE key='installed_scan_state'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(state.as_deref() == Some("done"))
    }

    /// Scan once on the first non-interactive launch. An installer can explicitly defer this.
    pub fn ensure_initial_scan(&self) -> Result<bool> {
        if !self.initial_scan_pending()? {
            return Ok(false);
        }
        self.scan_installed()?;
        Ok(true)
    }

    pub fn skip_initial_scan(&self) -> Result<()> {
        self.user_db()?.execute(
            "INSERT INTO meta(key,value) VALUES('installed_scan_state','skipped') ON CONFLICT(key) DO UPDATE SET value='skipped'",
            [],
        )?;
        Ok(())
    }

    pub fn installed(&self) -> Result<Vec<InstalledTool>> {
        let db = self.user_db()?;
        let mut stmt = db.prepare(
            "SELECT executable,path,version,source,tool_id FROM installed ORDER BY executable",
        )?;
        Ok(stmt
            .query_map([], |row| {
                Ok(InstalledTool {
                    executable: row.get(0)?,
                    path: row.get(1)?,
                    version: row.get(2)?,
                    source: row.get(3)?,
                    tool_id: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn scan_installed(&self) -> Result<Vec<InstalledTool>> {
        let mut known = HashMap::new();
        let mut methods = HashMap::new();
        for tool in self.all_tools()? {
            for name in &tool.executables {
                known.insert(name.clone(), tool.id.clone());
            }
            methods.insert(tool.id, tool.install);
        }
        let packages = package_inventory();
        let mut entries = BTreeMap::new();
        for directory in search_paths() {
            if let Ok(items) = fs::read_dir(&directory) {
                for item in items.flatten() {
                    let path = item.path();
                    let Some(executable) = path.file_name().and_then(|x| x.to_str()) else {
                        continue;
                    };
                    if entries.contains_key(executable) || !is_executable(&path) {
                        continue;
                    }
                    let tool_id = known.get(executable).cloned();
                    let mut source = source_from_path(&path);
                    let mut version = None;
                    if let Some(id) = &tool_id
                        && let Some(installs) = methods.get(id)
                    {
                        for (manager, method) in installs {
                            if let Some(v) =
                                packages.get(&(manager.clone(), method.package.clone()))
                            {
                                source = Some(manager.clone());
                                version = Some(v.clone());
                                break;
                            }
                        }
                    }
                    entries.insert(
                        executable.to_string(),
                        InstalledTool {
                            executable: executable.to_string(),
                            path: path.to_string_lossy().into_owned(),
                            version,
                            source,
                            tool_id,
                        },
                    );
                }
            }
        }
        let mut db = self.user_db()?;
        let tx = db.transaction()?;
        tx.execute("DELETE FROM installed", [])?;
        let scanned_at = chrono::Utc::now().timestamp();
        for entry in entries.values() {
            tx.execute(
                "INSERT INTO installed VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    entry.executable,
                    entry.path,
                    entry.version,
                    entry.source,
                    entry.tool_id,
                    scanned_at
                ],
            )?;
        }
        tx.execute(
            "INSERT INTO meta(key,value) VALUES('installed_scan_state','done') ON CONFLICT(key) DO UPDATE SET value='done'",
            [],
        )?;
        tx.commit()?;
        Ok(entries.into_values().collect())
    }
}

fn search_paths() -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut raw: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    raw.extend([
        PathBuf::from("/usr/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
    ]);
    if let Some(home) = dirs::home_dir() {
        raw.extend([home.join(".local/bin"), home.join(".cargo/bin")]);
    }
    for path in raw {
        if seen.insert(path.clone()) {
            result.push(path);
        }
    }
    result
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn source_from_path(path: &Path) -> Option<String> {
    let p = path.to_string_lossy();
    if p.contains("/.cargo/bin/") {
        Some("cargo".into())
    } else if p.contains("/.local/bin/") {
        Some("local".into())
    } else if p.starts_with("/snap/bin/") {
        Some("snap".into())
    } else if p.starts_with("/opt/homebrew/") {
        Some("brew".into())
    } else {
        None
    }
}

fn package_inventory() -> HashMap<(String, String), String> {
    let mut map = HashMap::new();
    if cfg!(target_os = "linux") {
        if let Some(output) = command("dpkg-query", &["-W", "-f=${Package}\t${Version}\n"]) {
            for line in output.lines() {
                if let Some((name, version)) = line.split_once('\t') {
                    map.insert(("apt".into(), name.into()), version.into());
                }
            }
        }
        if let Some(output) = command("snap", &["list"]) {
            for line in output.lines().skip(1) {
                let values: Vec<_> = line.split_whitespace().collect();
                if values.len() > 1 {
                    map.insert(("snap".into(), values[0].into()), values[1].into());
                }
            }
        }
        if let Some(output) = command(
            "flatpak",
            &["list", "--app", "--columns=application,version"],
        ) {
            for line in output.lines() {
                if let Some((name, version)) = line.split_once('\t') {
                    map.insert(("flatpak".into(), name.into()), version.into());
                }
            }
        }
    }
    if cfg!(target_os = "macos")
        && let Some(output) = command("brew", &["list", "--versions"])
    {
        for line in output.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(name), Some(version)) = (parts.next(), parts.next()) {
                map.insert(("brew".into(), name.into()), version.into());
            }
        }
    }
    if let Some(output) = command("cargo", &["install", "--list"]) {
        for line in output.lines().filter(|x| !x.starts_with(' ')) {
            if let Some((name, version)) = line.split_once(" v") {
                map.insert(
                    ("cargo".into(), name.into()),
                    version.trim_end_matches(':').into(),
                );
            }
        }
    }
    if let Some(output) = command("npm", &["list", "-g", "--depth=0", "--json"])
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&output)
        && let Some(deps) = value.get("dependencies").and_then(|x| x.as_object())
    {
        for (name, item) in deps {
            if let Some(version) = item.get("version").and_then(|x| x.as_str()) {
                map.insert(("npm".into(), name.clone()), version.into());
            }
        }
    }
    if let Some(output) = command("pipx", &["list", "--json"])
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&output)
        && let Some(venvs) = value.get("venvs").and_then(|x| x.as_object())
    {
        for (name, item) in venvs {
            if let Some(version) = item
                .pointer("/metadata/main_package/package_version")
                .and_then(|x| x.as_str())
            {
                map.insert(("pipx".into(), name.clone()), version.into());
            }
        }
    }
    map
}

fn command(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    });
    let start = Instant::now();
    while child.try_wait().ok()?.is_none() {
        if start.elapsed() > Duration::from_secs(3) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let status = child.wait().ok()?;
    let output = reader.join().ok()??;
    if status.success() {
        String::from_utf8(output).ok()
    } else {
        None
    }
}
