use crate::{Cliary, InstalledTool, LocalizedName, Tool};
use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

include!(concat!(env!("OUT_DIR"), "/embedded_tools.rs"));
const CATEGORIES: &str = include_str!("../../../catalog/i18n/categories.yaml");
const TAGS: &str = include_str!("../../../catalog/i18n/tags.yaml");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: Tool,
    pub installed: bool,
    pub favorite: bool,
    pub run_count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDetail {
    pub tool: Tool,
    pub installed: Option<InstalledTool>,
    pub favorite: bool,
    pub note: Option<String>,
    pub first_used: Option<String>,
    pub last_used: Option<String>,
    pub run_count: u64,
    pub active_days: u64,
    pub similar: Vec<Tool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareRow {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub source: Option<String>,
    pub license: Option<String>,
    pub implementation_language: Option<String>,
    pub maintenance_status: Option<String>,
    pub platforms: Vec<String>,
    pub features: std::collections::BTreeMap<String, bool>,
    pub install: std::collections::BTreeMap<String, cliary_catalog::InstallMethod>,
    pub repository: Option<String>,
}

impl Cliary {
    pub(crate) fn init_catalog(&self) -> Result<()> {
        if self.paths.catalog_db.exists() {
            return Ok(());
        }
        let tools = EMBEDDED_TOOLS
            .iter()
            .map(|x| cliary_catalog::parse_tool(x))
            .collect::<Result<Vec<_>>>()?;
        let categories = cliary_catalog::parse_names(CATEGORIES)?;
        let tags = cliary_catalog::parse_names(TAGS)?;
        let temp = self.paths.data_dir.join("catalog.db.initial");
        cliary_catalog::build_database(&temp, &tools, &categories, &tags, 1)?;
        std::fs::rename(temp, &self.paths.catalog_db)?;
        Ok(())
    }

    pub fn catalog_version(&self) -> Result<u64> {
        Ok(self
            .catalog_db()?
            .query_row("SELECT value FROM meta WHERE key='version'", [], |row| {
                row.get::<_, String>(0)
            })?
            .parse()?)
    }

    pub fn catalog_count(&self) -> Result<u64> {
        Ok(self
            .catalog_db()?
            .query_row("SELECT COUNT(*) FROM tools", [], |row| row.get(0))?)
    }

    pub fn categories(&self) -> Result<Vec<LocalizedName>> {
        self.names("categories")
    }
    pub fn tags(&self) -> Result<Vec<LocalizedName>> {
        self.names("tags")
    }

    fn names(&self, table: &str) -> Result<Vec<LocalizedName>> {
        let db = self.catalog_db()?;
        let sql = if table == "categories" {
            "SELECT data FROM categories ORDER BY id"
        } else {
            "SELECT data FROM tags ORDER BY id"
        };
        let mut stmt = db.prepare(sql)?;
        let values = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
            .into_iter()
            .map(|x| Ok(serde_json::from_str(&x)?))
            .collect()
    }

    pub fn all_tools(&self) -> Result<Vec<Tool>> {
        let db = self.catalog_db()?;
        let mut stmt = db.prepare("SELECT data FROM tools ORDER BY name")?;
        let values = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
            .into_iter()
            .map(|x| Ok(serde_json::from_str(&x)?))
            .collect()
    }

    pub fn get_tool(&self, name: &str) -> Result<Option<Tool>> {
        let db = self.catalog_db()?;
        if let Some(json) = db
            .query_row(
                "SELECT data FROM tools WHERE id=?1 OR name=?1 COLLATE NOCASE",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            return Ok(Some(serde_json::from_str(&json)?));
        }
        Ok(self.all_tools()?.into_iter().find(|tool| {
            tool.aliases.iter().any(|x| x.eq_ignore_ascii_case(name))
                || tool.executables.iter().any(|x| x == name)
        }))
    }

    pub fn tool_for_executable(&self, executable: &str) -> Result<Option<Tool>> {
        Ok(self
            .all_tools()?
            .into_iter()
            .find(|tool| tool.executables.iter().any(|x| x == executable)))
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ToolResult>> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let db = self.catalog_db()?;
        let mut fts = HashMap::new();
        let expression = q
            .split_whitespace()
            .take(8)
            .map(|word| format!("\"{}\"*", word.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" OR ");
        if !expression.is_empty()
            && let Ok(mut stmt) = db.prepare("SELECT id, bm25(tool_search) FROM tool_search WHERE tool_search MATCH ?1 LIMIT 200")
            && let Ok(rows) = stmt.query_map([expression], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))) {
            for row in rows.flatten() { fts.insert(row.0, row.1); }
        }
        let mut stmt = db.prepare("SELECT id,text FROM tool_search")?;
        let indexed = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut scored = Vec::new();
        for (id, text) in indexed {
            let lower = text.to_lowercase();
            let mut score = if fts.contains_key(&id) { 20 } else { 0 };
            if id == q {
                score += 100;
            }
            if id.starts_with(&q) {
                score += 30;
            }
            if lower.contains(&q) {
                score += 15;
            }
            for token in q.split_whitespace() {
                if lower.contains(token) {
                    score += 4;
                }
            }
            if !q.is_ascii() {
                let chars: Vec<_> = q.chars().collect();
                for pair in chars.windows(2) {
                    let pair = pair.iter().collect::<String>();
                    if lower.contains(&pair) {
                        score += 2;
                    }
                }
            }
            if score > 0 {
                scored.push((id, score));
            }
        }
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut results = Vec::new();
        for (id, _) in scored.into_iter().take(limit) {
            let tool = self
                .get_tool(&id)?
                .context("search index references missing tool")?;
            results.push(self.result_for(tool)?);
        }
        Ok(results)
    }

    fn result_for(&self, tool: Tool) -> Result<ToolResult> {
        let db = self.user_db()?;
        let installed: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM installed WHERE tool_id=?1)",
            [&tool.id],
            |row| row.get(0),
        )?;
        let favorite: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM favorites WHERE tool_id=?1)",
            [&tool.id],
            |row| row.get(0),
        )?;
        let run_count: u64 = db.query_row("SELECT COUNT(*) FROM usage_events WHERE tool_id=?1 OR executable IN (SELECT value FROM json_each(?2))", params![tool.id, serde_json::to_string(&tool.executables)?], |row| row.get(0))?;
        Ok(ToolResult {
            tool,
            installed,
            favorite,
            run_count,
        })
    }

    pub fn tool_detail(&self, name: &str) -> Result<Option<ToolDetail>> {
        let Some(tool) = self.get_tool(name)? else {
            return Ok(None);
        };
        let db = self.user_db()?;
        let installed = db.query_row("SELECT executable,path,version,source,tool_id FROM installed WHERE tool_id=?1 LIMIT 1", [&tool.id], |row| Ok(InstalledTool { executable: row.get(0)?, path: row.get(1)?, version: row.get(2)?, source: row.get(3)?, tool_id: row.get(4)? })).optional()?;
        let favorite: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM favorites WHERE tool_id=?1)",
            [&tool.id],
            |row| row.get(0),
        )?;
        let note = db
            .query_row(
                "SELECT body FROM notes WHERE tool_id=?1",
                [&tool.id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let history = self.history(Some(&tool.id))?;
        let similar = self.similar(&tool.id, 6)?;
        Ok(Some(ToolDetail {
            tool,
            installed,
            favorite,
            note,
            first_used: history.first_used,
            last_used: history.last_used,
            run_count: history.runs,
            active_days: history.active_days,
            similar,
        }))
    }

    pub fn similar(&self, name: &str, limit: usize) -> Result<Vec<Tool>> {
        let source = self.get_tool(name)?.context("tool not found")?;
        let mut others = self
            .all_tools()?
            .into_iter()
            .filter(|x| x.id != source.id)
            .map(|tool| {
                let score = (if source.similar.contains(&tool.id) {
                    100
                } else {
                    0
                }) + tool
                    .categories
                    .iter()
                    .filter(|x| source.categories.contains(x))
                    .count()
                    * 10
                    + tool.tags.iter().filter(|x| source.tags.contains(x)).count() * 2;
                (tool, score)
            })
            .filter(|(_, score)| *score > 0)
            .collect::<Vec<_>>();
        others.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.id.cmp(&b.0.id)));
        Ok(others.into_iter().take(limit).map(|x| x.0).collect())
    }

    pub fn compare(&self, names: &[String]) -> Result<Vec<CompareRow>> {
        if names.len() < 2 || names.len() > 8 {
            bail!("compare requires 2 to 8 tools");
        }
        let mut rows = Vec::new();
        for name in names {
            let detail = self
                .tool_detail(name)?
                .with_context(|| format!("unknown tool: {name}"))?;
            rows.push(CompareRow {
                id: detail.tool.id,
                name: detail.tool.name,
                installed: detail.installed.is_some(),
                version: detail.installed.as_ref().and_then(|x| x.version.clone()),
                source: detail.installed.as_ref().and_then(|x| x.source.clone()),
                license: detail.tool.license,
                implementation_language: detail.tool.implementation_language,
                maintenance_status: detail.tool.maintenance_status,
                platforms: detail.tool.platforms,
                features: detail.tool.features,
                install: detail.tool.install,
                repository: detail.tool.repository,
            });
        }
        Ok(rows)
    }
}
