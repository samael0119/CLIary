//! Parse an explicitly exported alias snapshot as data, never Shell code.
use crate::history_import::first_executable;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AliasResolution {
    pub alias: String,
    pub executable: String,
}

#[derive(Default)]
pub(crate) struct AliasSnapshot {
    definitions: BTreeMap<String, Option<AliasTarget>>,
}
struct AliasTarget {
    executable: String,
    expand_next: bool,
}
pub(crate) enum Resolution {
    Unchanged,
    Resolved(String),
    Unsupported,
}
impl AliasSnapshot {
    pub(crate) fn read(path: &Path) -> Result<Self> {
        if !std::fs::metadata(path)
            .context("cannot inspect alias snapshot")?
            .is_file()
        {
            bail!("alias snapshot must be a regular file");
        }
        let file = File::open(path).context("cannot open alias snapshot")?;
        if !file.metadata()?.is_file() {
            bail!("alias snapshot must be a regular file");
        }
        let mut bytes = Vec::new();
        file.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 4 * 1024 * 1024 {
            bail!("alias snapshot exceeds 4 MiB");
        }
        let text = std::str::from_utf8(&bytes).context("alias snapshot must be UTF-8")?;
        Ok(Self::parse(text))
    }
    fn parse(text: &str) -> Self {
        let mut definitions = BTreeMap::new();
        for line in text.lines() {
            let Ok(words) = shell_words::split(line) else {
                continue;
            };
            // Both `alias -L` in Zsh and `alias -p` in Bash emit this shape.
            // Global/suffix aliases, functions and configuration statements are excluded.
            if words.len() != 2 || words[0] != "alias" {
                continue;
            }
            let Some((name, expansion)) = words[1].split_once('=') else {
                continue;
            };
            if !name.is_empty()
                && name.len() <= 128
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_.+-".contains(c))
            {
                // Parse each definition once, independent of the number of history
                // entries. Discard its arguments rather than retaining expansion bodies.
                let target = first_executable(expansion).map(|executable| {
                    let expand_next = executable != name
                        && expansion.split_whitespace().next() == Some(&executable);
                    AliasTarget {
                        executable,
                        expand_next,
                    }
                });
                definitions.insert(name.to_owned(), target);
            }
        }
        Self { definitions }
    }
    pub(crate) fn resolve(&self, command: &str, executable: &str) -> Resolution {
        // Quoting, escaping, explicit paths and wrapper arguments suppress alias expansion.
        if command.split_whitespace().next() != Some(executable)
            || !self.definitions.contains_key(executable)
        {
            return Resolution::Unchanged;
        }
        let mut name = executable.to_owned();
        let mut seen = BTreeSet::new();
        for _ in 0..16 {
            if !seen.insert(name.clone()) {
                return Resolution::Unsupported;
            }
            let Some(definition) = self.definitions.get(&name) else {
                return Resolution::Resolved(name);
            };
            let Some(target) = definition else {
                return Resolution::Unsupported;
            };
            // Self aliases (e.g. git='git --no-pager') are not recursively expanded.
            if !target.expand_next || !self.definitions.contains_key(&target.executable) {
                return Resolution::Resolved(target.executable.clone());
            }
            name = target.executable.clone();
        }
        Resolution::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exported_aliases_are_data_with_chains_cycles_and_shell_bypasses() {
        let aliases = AliasSnapshot::parse(
            "alias gst='g status'\nalias g='git'\nalias git='git --no-pager'\nalias loop=a\nalias a=loop\nalias bad='git status; touch /SECRET'\nalias sub='git $(touch /SECRET)'\nalias builtin='cd /SECRET'\nalias -g GLOBAL='git'\nfunction gst() { git status; }\n",
        );
        assert!(matches!(aliases.resolve("gst -sb", "gst"),Resolution::Resolved(s) if s=="git"));
        for name in ["loop", "bad", "sub", "builtin"] {
            assert!(matches!(
                aliases.resolve(name, name),
                Resolution::Unsupported
            ));
        }
        for command in ["'gst'", "\\gst", "/usr/bin/gst", "command gst", "sudo gst"] {
            assert!(matches!(
                aliases.resolve(command, "gst"),
                Resolution::Unchanged
            ));
        }
        assert!(matches!(
            aliases.resolve("GLOBAL", "GLOBAL"),
            Resolution::Unchanged
        ));
    }
}
