use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LocalizedName {
    pub id: String,
    pub name: BTreeMap<String, String>,
    #[serde(default)]
    pub keywords: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InstallMethod {
    pub package: String,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub executables: Vec<String>,
    pub description: BTreeMap<String, String>,
    #[serde(default)]
    pub summary: BTreeMap<String, String>,
    #[serde(default)]
    pub keywords: BTreeMap<String, Vec<String>>,
    pub categories: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub implementation_language: Option<String>,
    #[serde(default)]
    pub maintenance_status: Option<String>,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
    #[serde(default)]
    pub install: BTreeMap<String, InstallMethod>,
    #[serde(default)]
    pub similar: Vec<String>,
    #[serde(default)]
    pub common_commands: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u64,
    pub schema_version: i64,
    pub generated_at: String,
    pub tool_count: usize,
    pub languages: Vec<String>,
    pub sha256: String,
}

pub fn parse_tool(text: &str) -> Result<Tool> {
    Ok(serde_yaml::from_str(text)?)
}

pub fn parse_names(text: &str) -> Result<Vec<LocalizedName>> {
    Ok(serde_yaml::from_str(text)?)
}

pub fn load_sources(root: &Path) -> Result<(Vec<Tool>, Vec<LocalizedName>, Vec<LocalizedName>)> {
    let mut files = Vec::new();
    collect_yaml(&root.join("tools"), &mut files)?;
    files.sort();
    let tools = files
        .iter()
        .map(|path| {
            parse_tool(
                &fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
            )
            .with_context(|| format!("parsing {}", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let categories = parse_names(&fs::read_to_string(root.join("i18n/categories.yaml"))?)?;
    let tags = parse_names(&fs::read_to_string(root.join("i18n/tags.yaml"))?)?;
    validate(&tools, &categories, &tags)?;
    Ok((tools, categories, tags))
}

fn collect_yaml(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("reading {}", root.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_yaml(&path, files)?;
        } else if path.extension().is_some_and(|x| x == "yaml") {
            files.push(path);
        }
    }
    Ok(())
}

pub fn validate(
    tools: &[Tool],
    categories: &[LocalizedName],
    tags: &[LocalizedName],
) -> Result<()> {
    if tools.is_empty() {
        bail!("catalog must contain tools");
    }
    let category_ids: HashSet<_> = categories.iter().map(|x| x.id.as_str()).collect();
    let tag_ids: HashSet<_> = tags.iter().map(|x| x.id.as_str()).collect();
    if category_ids.len() != categories.len() || tag_ids.len() != tags.len() {
        bail!("duplicate category or tag id");
    }
    let mut ids = HashSet::new();
    let mut names = HashMap::new();
    for item in categories.iter().chain(tags) {
        if item.id.is_empty() || item.name.get("en").is_none_or(String::is_empty) {
            bail!("localized name {} lacks English", item.id);
        }
        check_locales(&item.name, &item.id)?;
        check_locales(&item.keywords, &item.id)?;
    }
    for tool in tools {
        if !valid_id(&tool.id) || tool.name.trim().is_empty() {
            bail!("invalid tool id/name: {}", tool.id);
        }
        if !ids.insert(tool.id.as_str()) {
            bail!("duplicate tool id: {}", tool.id);
        }
        if tool
            .description
            .get("en")
            .is_none_or(|x| x.trim().is_empty())
        {
            bail!("missing English description: {}", tool.id);
        }
        check_locales(&tool.description, &tool.id)?;
        check_locales(&tool.summary, &tool.id)?;
        check_locales(&tool.keywords, &tool.id)?;
        if tool.categories.is_empty()
            || tool
                .categories
                .iter()
                .any(|x| !category_ids.contains(x.as_str()))
        {
            bail!("invalid category: {}", tool.id);
        }
        if tool.tags.iter().any(|x| !tag_ids.contains(x.as_str())) {
            bail!("invalid tag: {}", tool.id);
        }
        if tool.executables.is_empty() {
            bail!("missing executable: {}", tool.id);
        }
        for name in std::iter::once(&tool.id)
            .chain(tool.aliases.iter())
            .chain(tool.executables.iter())
        {
            if let Some(other) = names.insert(name.to_ascii_lowercase(), tool.id.as_str())
                && other != tool.id
            {
                bail!("name or executable conflict: {name} ({other}, {})", tool.id);
            }
        }
        for url in [&tool.homepage, &tool.repository].into_iter().flatten() {
            if !url::Url::parse(url).is_ok_and(|value| {
                ["https", "http"].contains(&value.scheme()) && value.host_str().is_some()
            }) {
                bail!("invalid URL in {}: {url}", tool.id);
            }
        }
        for method in tool.install.values() {
            if method.package.trim().is_empty() {
                bail!("empty install package: {}", tool.id);
            }
        }
    }
    for tool in tools {
        for similar in &tool.similar {
            if similar == &tool.id || !ids.contains(similar.as_str()) {
                bail!("invalid similar reference {} -> {}", tool.id, similar);
            }
        }
    }
    Ok(())
}

fn check_locales<T>(value: &BTreeMap<String, T>, context: &str) -> Result<()> {
    if value.keys().any(|key| key != "en" && key != "zh-CN") {
        bail!("unsupported locale in {context}");
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn build_database(
    path: &Path,
    tools: &[Tool],
    categories: &[LocalizedName],
    tags: &[LocalizedName],
    version: u64,
) -> Result<()> {
    validate(tools, categories, tags)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    let mut db = Connection::open(path)?;
    let tx = db.transaction()?;
    tx.execute_batch("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE tools(id TEXT PRIMARY KEY, name TEXT NOT NULL, data TEXT NOT NULL);
        CREATE TABLE categories(id TEXT PRIMARY KEY, data TEXT NOT NULL);
        CREATE TABLE tags(id TEXT PRIMARY KEY, data TEXT NOT NULL);
        CREATE VIRTUAL TABLE tool_search USING fts5(id UNINDEXED, text, tokenize='unicode61 remove_diacritics 2');")?;
    tx.execute(
        "INSERT INTO meta VALUES ('schema_version', ?1)",
        [SCHEMA_VERSION.to_string()],
    )?;
    tx.execute(
        "INSERT INTO meta VALUES ('version', ?1)",
        [version.to_string()],
    )?;
    for item in categories {
        tx.execute(
            "INSERT INTO categories VALUES (?1,?2)",
            params![item.id, serde_json::to_string(item)?],
        )?;
    }
    for item in tags {
        tx.execute(
            "INSERT INTO tags VALUES (?1,?2)",
            params![item.id, serde_json::to_string(item)?],
        )?;
    }
    for tool in tools {
        tx.execute(
            "INSERT INTO tools VALUES (?1,?2,?3)",
            params![tool.id, tool.name, serde_json::to_string(tool)?],
        )?;
        let mut terms = vec![tool.id.clone(), tool.name.clone()];
        terms.extend(tool.aliases.clone());
        terms.extend(tool.description.values().cloned());
        terms.extend(tool.summary.values().cloned());
        for keywords in tool.keywords.values() {
            terms.extend(keywords.clone());
        }
        for id in &tool.categories {
            terms.push(id.clone());
            if let Some(item) = categories.iter().find(|x| &x.id == id) {
                terms.extend(item.name.values().cloned());
                for keywords in item.keywords.values() {
                    terms.extend(keywords.clone());
                }
            }
        }
        for id in &tool.tags {
            terms.push(id.clone());
            if let Some(item) = tags.iter().find(|x| &x.id == id) {
                terms.extend(item.name.values().cloned());
                for keywords in item.keywords.values() {
                    terms.extend(keywords.clone());
                }
            }
        }
        tx.execute(
            "INSERT INTO tool_search(id,text) VALUES (?1,?2)",
            params![tool.id, terms.join(" ")],
        )?;
    }
    tx.commit()?;
    db.execute_batch("PRAGMA journal_mode=DELETE; VACUUM;")?;
    Ok(())
}

pub fn publish(root: &Path, output: &Path, version: u64) -> Result<Manifest> {
    fs::create_dir_all(output)?;
    let (tools, categories, tags) = load_sources(root)?;
    let db_path = output.join("catalog.db");
    build_database(&db_path, &tools, &categories, &tags, version)?;
    let compressed = zstd::stream::encode_all(fs::File::open(&db_path)?, 9)?;
    fs::write(output.join("catalog.db.zst"), &compressed)?;
    let json = serde_json::to_vec(&tools)?;
    fs::write(
        output.join("catalog.json.zst"),
        zstd::stream::encode_all(json.as_slice(), 9)?,
    )?;
    use sha2::Digest;
    let manifest = Manifest {
        version,
        schema_version: SCHEMA_VERSION,
        generated_at: chrono::Utc::now().to_rfc3339(),
        tool_count: tools.len(),
        languages: vec!["en".into(), "zh-CN".into()],
        sha256: hex::encode(sha2::Sha256::digest(&compressed)),
    };
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(manifest)
}
