use crate::{Cliary, InstalledTool, LocalizedName, Tool};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub description: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub common_commands: Vec<String>,
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

/// Shared labels for known Catalog feature keys. New keys retain their stable ID.
pub fn compare_feature_label<'a>(key: &'a str, lang: &str) -> &'a str {
    match (key, lang == "zh-CN") {
        ("tui", false) => "Terminal UI",
        ("tui", true) => "终端界面",
        ("interactive", false) => "Interactive",
        ("interactive", true) => "交互操作",
        ("delete_files", false) => "Delete files",
        ("delete_files", true) => "删除文件",
        ("parallel_scan", false) => "Parallel scan",
        ("parallel_scan", true) => "并行扫描",
        _ => key,
    }
}

impl Cliary {
    pub(crate) fn init_catalog(&self) -> Result<()> {
        use sha2::{Digest, Sha256};
        let revision = hex::encode(Sha256::digest(format!(
            "{}{}{}",
            EMBEDDED_TOOLS.join("\n"),
            CATEGORIES,
            TAGS
        )));
        if self.paths.catalog_db.exists() {
            let db = self.catalog_db()?;
            let version: String =
                db.query_row("SELECT value FROM meta WHERE key='version'", [], |r| {
                    r.get(0)
                })?;
            // Synced catalogs own their version; never replace them with bundled data.
            if version != "1" {
                return Ok(());
            }
            let previous: Option<String> = db
                .query_row(
                    "SELECT value FROM meta WHERE key='bundled_revision'",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            if previous.as_deref() == Some(&revision) {
                return Ok(());
            }
            let recorded: Option<String> = db
                .query_row(
                    "SELECT value FROM meta WHERE key='bundled_digest'",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            let actual = catalog_digest(&db)?;
            // The pre-upgrade bundled v1 had no origin marker. Only its exact content
            // is eligible; a custom/unrecognized v1 is left intact.
            const LEGACY_BUNDLES: &[&str] = &[
                // Last 55-tool bundle, and the earlier 54-tool serialized catalog.
                "cd98e6046d63ac59d7a97261eb513634f1a05b00c268e8f9bb6da3262356a50a",
                "124acd7502c68aa83e88451cf9023052d0c05fed187386dce9363c837f38cc29",
            ];
            let recognized = recorded.as_deref().map_or_else(
                || LEGACY_BUNDLES.contains(&actual.as_str()),
                |digest| digest == actual,
            );
            if !recognized {
                return Ok(());
            }
        }
        self.refresh_bundled_catalog()
    }

    /// Explicit offline replacement with this binary's Catalog; preserves user.db.
    pub fn refresh_bundled_catalog(&self) -> Result<()> {
        use sha2::{Digest, Sha256};
        let revision = hex::encode(Sha256::digest(format!(
            "{}{}{}",
            EMBEDDED_TOOLS.join("\n"),
            CATEGORIES,
            TAGS
        )));
        let tools = EMBEDDED_TOOLS
            .iter()
            .map(|x| cliary_catalog::parse_tool(x))
            .collect::<Result<Vec<_>>>()?;
        let categories = cliary_catalog::parse_names(CATEGORIES)?;
        let tags = cliary_catalog::parse_names(TAGS)?;
        let temp = self
            .paths
            .data_dir
            .join(format!("catalog-{}.initial", rand::random::<u64>()));
        let result = (|| -> Result<()> {
            cliary_catalog::build_database(&temp, &tools, &categories, &tags, 1)?;
            let db = rusqlite::Connection::open(&temp)?;
            let digest = catalog_digest(&db)?;
            db.execute(
                "INSERT INTO meta(key,value) VALUES ('bundled_revision',?1),('bundled_digest',?2)",
                params![revision, digest],
            )?;
            drop(db);
            std::fs::rename(&temp, &self.paths.catalog_db)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
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
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        if query.len() > 4096 {
            bail!("search query must be at most 4096 bytes");
        }
        crate::search::rank(query, self.all_tools()?, &self.categories()?, &self.tags()?)
            .into_iter()
            .take(limit)
            .map(|tool| self.result_for(tool))
            .collect()
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
        let mut seen = std::collections::HashSet::new();
        for name in names {
            let detail = self
                .tool_detail(name)?
                .with_context(|| format!("unknown tool: {name}"))?;
            if !seen.insert(detail.tool.id.clone()) {
                continue;
            }
            rows.push(CompareRow {
                id: detail.tool.id,
                name: detail.tool.name,
                description: detail.tool.description,
                common_commands: detail.tool.common_commands,
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
        if rows.len() < 2 {
            bail!("compare requires at least 2 distinct tools");
        }
        Ok(rows)
    }
}

// Hash the canonical stored records including names/tags; never touch user.db.
fn catalog_digest(db: &Connection) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for table in ["tools", "categories", "tags"] {
        let mut stmt = db.prepare(&format!("SELECT data FROM {table} ORDER BY id"))?;
        for value in stmt.query_map([], |row| row.get::<_, String>(0))? {
            digest.update(value?.as_bytes());
            digest.update(b"\n");
        }
    }
    Ok(hex::encode(digest.finalize()))
}
