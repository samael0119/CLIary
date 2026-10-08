use crate::{Cliary, CountItem, history::local_year_start};
use anyhow::Result;
use chrono::{Datelike, TimeZone, Timelike};
use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WrappedInsights {
    pub new_tools: Vec<NewToolInsight>,
    pub new_tools_count: u64,
    pub breakout_tool: Option<NewToolInsight>,
    pub top_categories: Vec<CountItem>,
    pub category_labels: BTreeMap<String, BTreeMap<String, String>>,
    pub low_activity_favorites: Vec<FavoriteInsight>,
    pub comparison: Option<YearComparison>,
    pub tool_changes: Vec<ToolChange>,
    pub source_counts: Vec<CountItem>,
    pub undated_tools: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewToolInsight {
    pub name: String,
    pub runs: u64,
    pub first_recorded: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FavoriteInsight {
    pub name: String,
    pub runs: u64,
    pub favorited_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YearComparison {
    pub previous_year: i32,
    pub current_runs: u64,
    pub previous_runs: u64,
    pub change_percent: Option<f64>,
    pub current_end: String,
    pub previous_end: String,
    pub year_to_date: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolChange {
    pub name: String,
    pub current_runs: u64,
    pub previous_runs: u64,
}
fn counts(tx: &Transaction<'_>, start: i64, end: i64) -> Result<BTreeMap<String, u64>> {
    Ok(tx.prepare("SELECT COALESCE(tool_id,executable),COUNT(*) FROM usage_events WHERE timestamp>=?1 AND timestamp<?2 GROUP BY 1")?
        .query_map(params![start,end],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?)
}
fn local_string(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .unwrap()
        .with_timezone(&chrono::Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}
impl Cliary {
    pub(crate) fn wrapped_insights(
        &self,
        tx: &Transaction<'_>,
        year: i32,
        start: i64,
        end: i64,
    ) -> Result<WrappedInsights> {
        let current = counts(tx, start, end)?;
        // Group by saved identities across all years: Catalog changes cannot create a new date.
        let first = tx
            .prepare(
                "SELECT COALESCE(tool_id,executable),MIN(timestamp) FROM usage_events GROUP BY 1",
            )?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
        let tools = self.all_tools()?;
        let unmatched_first=tx.prepare("SELECT executable,MIN(timestamp) FROM usage_events WHERE tool_id IS NULL GROUP BY executable")?
            .query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))?.collect::<rusqlite::Result<BTreeMap<_,_>>>()?;
        let mut new_tools = current
            .iter()
            .filter_map(|(name, runs)| {
                let mut timestamp = *first.get(name)?;
                if let Some(tool) = tools.iter().find(|t| &t.id == name) {
                    for exe in &tool.executables {
                        if let Some(previous) = unmatched_first.get(exe) {
                            timestamp = timestamp.min(*previous);
                        }
                    }
                }
                (timestamp >= start && timestamp < end).then(|| NewToolInsight {
                    name: name.clone(),
                    runs: *runs,
                    first_recorded: local_string(timestamp),
                })
            })
            .collect::<Vec<_>>();
        new_tools.sort_by(|a, b| b.runs.cmp(&a.runs).then(a.name.cmp(&b.name)));
        let new_tools_count = new_tools.len() as u64;
        let breakout_tool = new_tools.first().cloned();
        new_tools.truncate(10);
        let mut categories = HashMap::<String, u64>::new();
        let by_id = tools
            .iter()
            .map(|t| (t.id.as_str(), t))
            .collect::<HashMap<_, _>>();
        let by_exe = tools
            .iter()
            .flat_map(|t| t.executables.iter().map(move |e| (e.as_str(), t)))
            .collect::<HashMap<_, _>>();
        // Current Catalog categories, one count per assigned category; not mutually exclusive.
        for (name, runs) in &current {
            if let Some(tool) = by_id
                .get(name.as_str())
                .or_else(|| by_exe.get(name.as_str()))
            {
                if tool.categories.is_empty() {
                    *categories.entry("uncategorized".into()).or_default() += runs;
                }
                for category in &tool.categories {
                    *categories.entry(category.clone()).or_default() += runs;
                }
            } else {
                *categories.entry("uncategorized".into()).or_default() += runs;
            }
        }
        let mut top_categories = categories
            .into_iter()
            .map(|(name, count)| CountItem { name, count })
            .collect::<Vec<_>>();
        top_categories.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
        top_categories.truncate(10);
        let now = chrono::Local::now();
        let observation_end = end.min(now.timestamp() + 1);
        let favorites=tx.prepare("SELECT tool_id,created_at FROM favorites WHERE created_at<=?1 ORDER BY created_at,tool_id")?
            .query_map([observation_end-90*86400],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut low_activity_favorites = Vec::new();
        for (name, created_at) in favorites {
            let aliases = by_id.get(name.as_str()).map(|t| &t.executables);
            let runs:u64=tx.query_row("SELECT COUNT(*) FROM usage_events WHERE timestamp>=?1 AND timestamp<?2 AND (tool_id=?3 OR (tool_id IS NULL AND (executable=?3 OR executable IN (SELECT value FROM json_each(?4)))))",params![start.max(created_at),observation_end,name,serde_json::to_string(&aliases)?],|r|r.get(0))?;
            if runs <= 2 {
                low_activity_favorites.push(FavoriteInsight {
                    name,
                    runs,
                    favorited_at: local_string(created_at),
                });
            }
        }
        if observation_end <= start {
            low_activity_favorites.clear();
        }
        low_activity_favorites.sort_by(|a, b| {
            a.runs
                .cmp(&b.runs)
                .then(a.favorited_at.cmp(&b.favorited_at))
                .then(a.name.cmp(&b.name))
        });
        low_activity_favorites.truncate(10);
        let mut comparison = None;
        let mut tool_changes = Vec::new();
        if year > 1 {
            let year_to_date = year == now.year();
            let current_end = if year_to_date {
                now.timestamp() + 1
            } else {
                end
            };
            let previous_start = local_year_start(year - 1)?;
            let previous_end = if year_to_date {
                // Same local month/day/time; Feb 29 clamps to Feb 28 in the prior year.
                let mut day = now.day();
                let date = loop {
                    if let Some(d) = chrono::NaiveDate::from_ymd_opt(year - 1, now.month(), day) {
                        break d;
                    }
                    day -= 1;
                };
                chrono::Local
                    .from_local_datetime(
                        &date
                            .and_hms_opt(now.hour(), now.minute(), now.second())
                            .unwrap(),
                    )
                    .earliest()
                    .map(|t| t.timestamp() + 1)
            } else {
                Some(start)
            };
            if let Some(previous_end) = previous_end {
                let previous = counts(tx, previous_start, previous_end)?;
                let comparable_current = counts(tx, start, current_end)?;
                let current_runs = comparable_current.values().sum::<u64>();
                let previous_runs = previous.values().sum::<u64>();
                comparison = Some(YearComparison {
                    previous_year: year - 1,
                    current_runs,
                    previous_runs,
                    change_percent: (previous_runs > 0)
                        .then(|| (current_runs as f64 / previous_runs as f64 - 1.0) * 100.0),
                    current_end: local_string(current_end - 1),
                    previous_end: local_string(previous_end - 1),
                    year_to_date,
                });
                let names = comparable_current
                    .keys()
                    .chain(previous.keys())
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>();
                tool_changes = names
                    .into_iter()
                    .filter_map(|name| {
                        let current_runs = comparable_current.get(&name).copied().unwrap_or(0);
                        let previous_runs = previous.get(&name).copied().unwrap_or(0);
                        (current_runs != previous_runs).then_some(ToolChange {
                            name,
                            current_runs,
                            previous_runs,
                        })
                    })
                    .collect();
                tool_changes.sort_by(|a, b| {
                    b.current_runs
                        .abs_diff(b.previous_runs)
                        .cmp(&a.current_runs.abs_diff(a.previous_runs))
                        .then(a.name.cmp(&b.name))
                });
                tool_changes.truncate(10);
            }
        }
        let source_counts=tx.prepare("SELECT source,COUNT(*) FROM usage_events WHERE timestamp>=?1 AND timestamp<?2 GROUP BY source ORDER BY COUNT(*) DESC,source")?
            .query_map(params![start,end],|r|Ok(CountItem{name:r.get(0)?,count:r.get(1)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let undated_tools =
            tx.query_row("SELECT COUNT(*) FROM history_undated", [], |r| r.get(0))?;
        let category_labels = self
            .categories()?
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect();
        Ok(WrappedInsights {
            new_tools,
            new_tools_count,
            breakout_tool,
            top_categories,
            category_labels,
            low_activity_favorites,
            comparison,
            tool_changes,
            source_counts,
            undated_tools,
        })
    }
}
