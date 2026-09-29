use crate::Cliary;
use anyhow::{Result, bail};
use chrono::TimeZone;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageEvent {
    pub executable: String,
    pub tool_id: Option<String>,
    pub timestamp: i64,
    pub machine_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HistorySummary {
    pub tool: Option<String>,
    pub first_used: Option<String>,
    pub last_used: Option<String>,
    pub runs: u64,
    pub active_days: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CountItem {
    pub name: String,
    pub count: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub total_runs: u64,
    pub active_days: u64,
    pub tools_used: u64,
    pub new_tools: u64,
    pub new_tool_names: Vec<String>,
    pub recently_used: Vec<String>,
    pub top_tools: Vec<CountItem>,
    pub category_usage: Vec<CountItem>,
    pub daily_activity: Vec<CountItem>,
    pub monthly_activity: Vec<CountItem>,
    pub dormant_favorites: Vec<String>,
}

impl Cliary {
    pub fn record_usage(&self, executable: &str) -> Result<()> {
        if executable
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        {
            bail!("invalid executable name");
        }
        let executable = executable.rsplit('/').next().unwrap_or(executable);
        if executable.is_empty()
            || executable.len() > 128
            || !executable
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_.+-".contains(c))
        {
            bail!("invalid executable name");
        }
        if executable == "cliary" {
            return Ok(());
        }
        let tool_id = self.tool_for_executable(executable)?.map(|x| x.id);
        let db = self.user_db()?;
        db.execute("INSERT INTO usage_events(executable,tool_id,timestamp,machine_id) VALUES (?1,?2,?3,?4)",
            params![executable, tool_id, chrono::Utc::now().timestamp(), self.machine_id()?])?;
        Ok(())
    }

    pub fn history(&self, tool: Option<&str>) -> Result<HistorySummary> {
        let db = self.user_db()?;
        let (filter_id, executable_names) = match tool {
            Some(name) => match self.get_tool(name)? {
                Some(item) => (Some(item.id), item.executables),
                None => (None, vec![name.to_string()]),
            },
            None => (None, Vec::new()),
        };
        let (first_used, last_used, runs, active_days) = if tool.is_some() {
            db.query_row("SELECT datetime(MIN(timestamp),'unixepoch','localtime'), datetime(MAX(timestamp),'unixepoch','localtime'), COUNT(*), COUNT(DISTINCT date(timestamp,'unixepoch','localtime')) FROM usage_events WHERE tool_id=?1 OR executable IN (SELECT value FROM json_each(?2))",
                params![filter_id, serde_json::to_string(&executable_names)?], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
        } else {
            db.query_row("SELECT datetime(MIN(timestamp),'unixepoch','localtime'), datetime(MAX(timestamp),'unixepoch','localtime'), COUNT(*), COUNT(DISTINCT date(timestamp,'unixepoch','localtime')) FROM usage_events", [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
        };
        Ok(HistorySummary {
            tool: tool.map(str::to_string),
            first_used,
            last_used,
            runs,
            active_days,
        })
    }

    pub fn stats(&self, days: Option<u32>, year: Option<i32>) -> Result<Stats> {
        if days.is_some() && year.is_some() {
            bail!("choose period or year");
        }
        let db = self.user_db()?;
        let start = if let Some(days) = days {
            chrono::Utc::now().timestamp() - i64::from(days) * 86400
        } else if let Some(year) = year {
            local_year_start(year)?
        } else {
            0
        };
        let end = if let Some(year) = year {
            local_year_start(
                year.checked_add(1)
                    .ok_or_else(|| anyhow::anyhow!("invalid year"))?,
            )?
        } else {
            i64::MAX
        };
        let mut stats = Stats::default();
        let mut stmt = db.prepare("SELECT executable,COALESCE(tool_id,executable),COUNT(*),MIN(timestamp),MAX(timestamp) FROM usage_events WHERE timestamp>=?1 AND timestamp<?2 GROUP BY COALESCE(tool_id,executable),executable ORDER BY COUNT(*) DESC")?;
        let rows = stmt
            .query_map(params![start, end], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut counts: HashMap<String, u64> = HashMap::new();
        let mut category: HashMap<String, u64> = HashMap::new();
        for (executable, id, count, _, _) in rows {
            stats.total_runs += count;
            *counts.entry(id.clone()).or_default() += count;
            let catalog_tool = self
                .get_tool(&id)?
                .or_else(|| self.get_tool(&executable).ok().flatten());
            if let Some(tool) = catalog_tool {
                for cat in tool.categories {
                    *category.entry(cat).or_default() += count;
                }
            } else {
                *category.entry("uncategorized".into()).or_default() += count;
            }
        }
        stats.tools_used = counts.len() as u64;
        let mut new_tools = HashSet::new();
        for id in counts.keys() {
            let first: i64 = if let Some(tool) = self.get_tool(id)? {
                db.query_row("SELECT MIN(timestamp) FROM usage_events WHERE tool_id=?1 OR executable IN (SELECT value FROM json_each(?2))",
                    params![id, serde_json::to_string(&tool.executables)?], |row| row.get(0))?
            } else {
                db.query_row(
                    "SELECT MIN(timestamp) FROM usage_events WHERE executable=?1",
                    [id],
                    |row| row.get(0),
                )?
            };
            if first >= start && first < end {
                new_tools.insert(id.clone());
            }
        }
        stats.new_tools = new_tools.len() as u64;
        stats.new_tool_names = new_tools.into_iter().collect();
        stats.new_tool_names.sort();
        stats.top_tools = sorted_counts(counts).into_iter().take(10).collect();
        let mut stmt = db.prepare("SELECT COALESCE(tool_id,executable) FROM usage_events WHERE timestamp>=?1 AND timestamp<?2 GROUP BY COALESCE(tool_id,executable) ORDER BY MAX(timestamp) DESC LIMIT 10")?;
        stats.recently_used = stmt
            .query_map(params![start, end], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        stats.category_usage = sorted_counts(category);
        let mut stmt = db.prepare("SELECT date(timestamp,'unixepoch','localtime'), COUNT(*) FROM usage_events WHERE timestamp>=?1 AND timestamp<?2 GROUP BY 1 ORDER BY 1")?;
        stats.daily_activity = stmt
            .query_map(params![start, end], |row| {
                Ok(CountItem {
                    name: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        stats.active_days = stats.daily_activity.len() as u64;
        let mut stmt = db.prepare("SELECT strftime('%Y-%m',timestamp,'unixepoch','localtime'), COUNT(*) FROM usage_events WHERE timestamp>=?1 AND timestamp<?2 GROUP BY 1 ORDER BY 1")?;
        stats.monthly_activity = stmt
            .query_map(params![start, end], |row| {
                Ok(CountItem {
                    name: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut stmt = db.prepare("SELECT f.tool_id FROM favorites f LEFT JOIN usage_events u ON u.tool_id=f.tool_id AND u.timestamp>=?1 GROUP BY f.tool_id HAVING COUNT(u.id)=0 ORDER BY f.tool_id")?;
        let cutoff = chrono::Utc::now().timestamp() - 90 * 86400;
        stats.dormant_favorites = stmt
            .query_map([cutoff], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(stats)
    }
}

fn local_year_start(year: i32) -> Result<i64> {
    let day = chrono::NaiveDate::from_ymd_opt(year, 1, 1)
        .ok_or_else(|| anyhow::anyhow!("invalid year"))?;
    let midnight = day.and_hms_opt(0, 0, 0).unwrap();
    Ok(chrono::Local
        .from_local_datetime(&midnight)
        .earliest()
        .ok_or_else(|| anyhow::anyhow!("invalid local year boundary"))?
        .timestamp())
}

fn sorted_counts(map: HashMap<String, u64>) -> Vec<CountItem> {
    let mut values = map
        .into_iter()
        .map(|(name, count)| CountItem { name, count })
        .collect::<Vec<_>>();
    values.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
    values
}
