//! Conservative, opt-in import. Raw commands never leave the parser or reach SQLite.
use crate::Cliary;
use crate::shell_alias::{AliasSnapshot, Resolution};
use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryFormat {
    Bash,
    Zsh,
    Fish,
}
impl HistoryFormat {
    fn source(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UndatedTool {
    pub executable: String,
    pub occurrences: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub applied: bool,
    pub entries: u64,
    pub timed: u64,
    pub undated: u64,
    pub skipped: u64,
    pub duplicates: u64,
    pub new_timed: u64,
    pub new_undated: u64,
    pub first_timestamp: Option<i64>,
    pub last_timestamp: Option<i64>,
    /// Executable-only preview, capped at 20. Never raw commands or paths.
    pub sample_tools: Vec<String>,
    /// Only the alias and executable, never the expansion's arguments.
    #[serde(default)]
    pub alias_resolutions: Vec<crate::AliasResolution>,
    /// Existing unmatched imported records classified by this explicit snapshot.
    #[serde(default)]
    pub reclassified_records: u64,
}
struct Parsed {
    timed: BTreeMap<(String, i64), u64>,
    undated: BTreeMap<String, u64>,
    report: ImportReport,
    resolved: BTreeMap<String, String>,
    unexpanded: std::collections::BTreeSet<String>,
}

impl Cliary {
    /// Preview by default. Reads only the explicitly selected regular file, <=32 MiB.
    pub fn import_history(
        &self,
        path: &Path,
        format: HistoryFormat,
        apply: bool,
    ) -> Result<ImportReport> {
        self.import_history_with_aliases(path, format, apply, None)
    }

    /// Alias definitions are an explicit user-selected snapshot, not a guess about old configuration.
    pub fn import_history_with_aliases(
        &self,
        path: &Path,
        format: HistoryFormat,
        apply: bool,
        aliases_file: Option<&Path>,
    ) -> Result<ImportReport> {
        let aliases = aliases_file
            .map(AliasSnapshot::read)
            .transpose()?
            .unwrap_or_default();
        if !std::fs::metadata(path)
            .context("cannot inspect selected history file")?
            .is_file()
        {
            bail!("history must be a regular file");
        }
        let file = File::open(path).context("cannot open selected history file")?;
        if !file.metadata()?.is_file() {
            bail!("history must be a regular file");
        }
        let mut bytes = Vec::new();
        file.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 32 * 1024 * 1024 {
            bail!("history file exceeds 32 MiB");
        }
        // Zsh metafies bytes using 0x83 followed by byte XOR 32.
        if matches!(format, HistoryFormat::Zsh) {
            let mut decoded = Vec::with_capacity(bytes.len());
            let mut iter = bytes.into_iter();
            while let Some(byte) = iter.next() {
                decoded.push(if byte == 0x83 {
                    iter.next().context("truncated Zsh metafied byte")? ^ 32
                } else {
                    byte
                });
            }
            bytes = decoded;
        }
        let text = std::str::from_utf8(&bytes)
            .context("history must be UTF-8; export a UTF-8 copy first")?;
        let mut parsed = parse(text, format, &aliases);
        let mut db = self.user_db()?;
        let tx = db.transaction_with_behavior(if apply {
            rusqlite::TransactionBehavior::Immediate
        } else {
            rusqlite::TransactionBehavior::Deferred
        })?;
        // Reading a preview must not create a machine identity or import metadata.
        let existing_machine: Option<String> = tx
            .query_row("SELECT value FROM meta WHERE key='machine_id'", [], |r| {
                r.get(0)
            })
            .optional()?;
        let machine = existing_machine.unwrap_or_else(|| {
            use rand::RngCore;
            let mut random = [0u8; 16];
            rand::thread_rng().fill_bytes(&mut random);
            hex::encode(random)
        });
        if apply {
            tx.execute(
                "INSERT OR IGNORE INTO meta VALUES ('machine_id',?1)",
                [&machine],
            )?;
        }
        let catalog = self.catalog_db()?;
        let tools = catalog
            .prepare("SELECT id, data FROM tools")?
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut ids = BTreeMap::new();
        for (id, data) in tools {
            let tool: crate::Tool = serde_json::from_str(&data)?;
            for exe in tool.executables {
                ids.insert(exe, id.clone());
            }
        }
        for ((executable, timestamp), count) in &parsed.timed {
            let target = parsed.resolved.get(executable).unwrap_or(executable);
            let tool_id = ids.get(target);
            if target != executable
                && let Some(id) = tool_id
            {
                let reclassified: u64 = tx.query_row("SELECT COUNT(*) FROM usage_events WHERE executable=?1 AND timestamp=?2 AND machine_id=?3 AND source<>'capture' AND tool_id IS NULL", params![executable,timestamp,machine], |r| r.get(0))?;
                parsed.report.reclassified_records += reclassified;
                if apply {
                    tx.execute("UPDATE usage_events SET tool_id=?1 WHERE executable=?2 AND timestamp=?3 AND machine_id=?4 AND source<>'capture' AND tool_id IS NULL", params![id,executable,timestamp,machine])?;
                }
            }
            // Count only live records here; imported multiplicity is tracked by keys.
            let live: u64 = tx.query_row("SELECT COUNT(*) FROM usage_events WHERE executable=?1 AND timestamp=?2 AND machine_id=?3 AND source='capture'", params![executable,timestamp,machine], |r| r.get(0))?;
            for ordinal in 0..*count {
                let key = hex::encode(Sha256::digest(format!(
                    "{machine}\0{executable}\0{timestamp}\0{ordinal}"
                )));
                let exists: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM history_import_keys WHERE import_key=?1)",
                    [&key],
                    |r| r.get(0),
                )?;
                if exists || ordinal < live {
                    parsed.report.duplicates += 1;
                    continue;
                }
                parsed.report.new_timed += 1;
                if apply {
                    tx.execute(
                        "INSERT INTO history_import_keys VALUES (?1,?2)",
                        params![key, format.source()],
                    )?;
                    tx.execute("INSERT INTO usage_events(executable,tool_id,timestamp,machine_id,source) VALUES (?1,?2,?3,?4,?5)", params![executable,tool_id,timestamp,machine,format.source()])?;
                }
            }
        }
        for (exe, count) in &parsed.undated {
            let previous: u64 = tx
                .query_row(
                    "SELECT occurrences FROM history_undated WHERE executable=?1",
                    [exe],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or(0);
            parsed.report.new_undated += count.saturating_sub(previous);
            if apply {
                tx.execute("INSERT INTO history_undated VALUES (?1,?2) ON CONFLICT(executable) DO UPDATE SET occurrences=MAX(occurrences,excluded.occurrences)", params![exe,count])?;
            }
        }
        if apply {
            tx.commit()?;
        } // Otherwise rollback on drop.
        parsed.report.applied = apply;
        Ok(parsed.report)
    }

    /// These observations have no date and are intentionally excluded from all statistics.
    pub fn undated_history(&self) -> Result<Vec<UndatedTool>> {
        Ok(self.user_db()?.prepare("SELECT executable,occurrences FROM history_undated ORDER BY occurrences DESC,executable")?
            .query_map([], |r| Ok(UndatedTool { executable:r.get(0)?, occurrences:r.get(1)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
fn parse(text: &str, format: HistoryFormat, aliases: &AliasSnapshot) -> Parsed {
    let mut parsed = Parsed {
        timed: BTreeMap::new(),
        undated: BTreeMap::new(),
        report: ImportReport::default(),
        resolved: BTreeMap::new(),
        unexpanded: std::collections::BTreeSet::new(),
    };
    let mut pending: Option<(String, Option<i64>)> = None;
    let timestamped_bash = matches!(format, HistoryFormat::Bash)
        && text.lines().any(|l| {
            l.starts_with('#') && l[1..].chars().all(|c| c.is_ascii_digit()) && l.len() > 1
        });
    for line in text.lines() {
        match format {
            HistoryFormat::Fish => {
                if let Some(command) = line.strip_prefix("- cmd: ") {
                    flush(&mut parsed, pending.take(), aliases);
                    // Fish's YAML-like escaping represents newlines and backslashes only.
                    pending = Some((unescape_fish(command), None));
                } else if let Some(when) = line.strip_prefix("  when: ")
                    && let Some((_, timestamp)) = pending.as_mut()
                {
                    *timestamp = Some(when.parse().unwrap_or(i64::MIN));
                }
            }
            HistoryFormat::Bash if timestamped_bash => {
                if let Some(stamp) = line
                    .strip_prefix('#')
                    .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                {
                    flush(&mut parsed, pending.take(), aliases);
                    pending = Some((String::new(), Some(stamp.parse().unwrap_or(i64::MIN))));
                } else if let Some((command, _)) = pending.as_mut() {
                    if !command.is_empty() {
                        command.push('\n');
                    }
                    command.push_str(line);
                } else {
                    flush(&mut parsed, Some((line.into(), None)), aliases);
                }
            }
            HistoryFormat::Zsh => {
                if let Some((command, _)) = pending.as_mut() {
                    // Backslash-continued history entry.
                    command.push('\n');
                    command.push_str(line);
                    if !line.ends_with('\\') {
                        flush(&mut parsed, pending.take(), aliases);
                    }
                    continue;
                }
                let record = if let Some(prefix) = line.strip_prefix(": ") {
                    if let Some((metadata, command)) = prefix.split_once(';') {
                        let timestamp = metadata
                            .split_once(':')
                            .and_then(|(t, d)| {
                                if d.parse::<u64>().is_ok() {
                                    t.parse().ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(i64::MIN);
                        (command.into(), Some(timestamp))
                    } else {
                        (line.into(), Some(i64::MIN))
                    }
                } else {
                    (line.into(), None)
                };
                if line.ends_with('\\') {
                    pending = Some(record);
                } else {
                    flush(&mut parsed, Some(record), aliases);
                }
            }
            _ => flush(&mut parsed, Some((line.into(), None)), aliases),
        }
    }
    flush(&mut parsed, pending, aliases);
    // A mixed snapshot containing quoted/explicit calls is not evidence that every
    // occurrence of that name was an alias. Keep its original classification.
    for name in &parsed.unexpanded {
        parsed.resolved.remove(name);
    }
    parsed.report.alias_resolutions = parsed
        .resolved
        .iter()
        .take(20)
        .map(|(alias, executable)| crate::AliasResolution {
            alias: alias.clone(),
            executable: executable.clone(),
        })
        .collect();
    parsed.report.sample_tools = parsed
        .timed
        .keys()
        .map(|(e, _)| e.clone())
        .chain(parsed.undated.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .take(20)
        .collect();
    parsed
}
fn flush(parsed: &mut Parsed, record: Option<(String, Option<i64>)>, aliases: &AliasSnapshot) {
    let Some((command, timestamp)) = record else {
        return;
    };
    if command.trim().is_empty() || command.trim_start().starts_with('#') {
        return;
    }
    parsed.report.entries += 1;
    let Some(executable) = first_executable(&command) else {
        parsed.report.skipped += 1;
        return;
    };
    match aliases.resolve(&command, &executable) {
        Resolution::Resolved(target) => {
            parsed.resolved.insert(executable.clone(), target);
        }
        Resolution::Unchanged => {
            parsed.unexpanded.insert(executable.clone());
        }
        Resolution::Unsupported => {
            parsed.report.skipped += 1;
            return;
        }
    }
    if let Some(timestamp) = timestamp {
        if timestamp <= 0
            || timestamp > chrono::Utc::now().timestamp()
            || chrono::DateTime::from_timestamp(timestamp, 0).is_none()
        {
            parsed.report.skipped += 1;
            return;
        }
        *parsed.timed.entry((executable, timestamp)).or_default() += 1;
        parsed.report.timed += 1;
        parsed.report.first_timestamp = Some(
            parsed
                .report
                .first_timestamp
                .map_or(timestamp, |v| v.min(timestamp)),
        );
        parsed.report.last_timestamp = Some(
            parsed
                .report
                .last_timestamp
                .map_or(timestamp, |v| v.max(timestamp)),
        );
    } else {
        *parsed.undated.entry(executable).or_default() += 1;
        parsed.report.undated += 1;
    }
}
fn unescape_fish(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other)
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
/// Do not guess executions inside pipelines, substitutions, compound or multiline commands.
pub(crate) fn first_executable(command: &str) -> Option<String> {
    if command.chars().any(|c| "\n\r\0;|&<>`()".contains(c)) {
        return None;
    }
    let words = shell_words::split(command).ok()?;
    let mut i = 0;
    while words.get(i).is_some_and(|w| assignment(w)) {
        i += 1;
    }
    loop {
        let word = words.get(i)?;
        match word.as_str() {
            "builtin" => return None,
            "command" => {
                i += 1;
                if words.get(i).is_some_and(|w| w == "--") {
                    i += 1;
                }
            }
            "sudo" | "env" => {
                let wrapper = word.as_str();
                i += 1;
                while let Some(w) = words.get(i) {
                    if w == "--" {
                        i += 1;
                        break;
                    }
                    if assignment(w)
                        || [
                            "-E",
                            "-H",
                            "-n",
                            "-i",
                            "--preserve-env",
                            "--ignore-environment",
                        ]
                        .contains(&w.as_str())
                    {
                        i += 1;
                        continue;
                    }
                    if wrapper == "sudo" && ["-u", "-g", "--user", "--group"].contains(&w.as_str())
                    {
                        i += 2;
                        continue;
                    }
                    if w.starts_with('-') {
                        return None;
                    }
                    break;
                }
            }
            _ => break,
        }
    }
    let name = words.get(i)?.rsplit('/').next()?;
    if name.is_empty()
        || name.len() > 128
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.+-".contains(c))
    {
        return None;
    }
    if [
        "cliary", "cd", "echo", "printf", "export", "set", "unset", "alias", "unalias", "source",
        ".", "read", "exit", "return", "history", "pwd", "true", "false", "test", "eval", "exec",
        "if", "then", "else", "fi", "for", "while", "do", "done", "function", "end", "begin",
        "and", "or", "not",
    ]
    .contains(&name)
    {
        return None;
    }
    Some(name.into())
}
fn assignment(s: &str) -> bool {
    s.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name
                .chars()
                .enumerate()
                .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
    })
}
