use cliary_core::{Cliary, Paths};
use tempfile::TempDir;

fn app() -> (TempDir, Cliary) {
    let root = TempDir::new().unwrap();
    let paths = Paths::new(
        root.path().join("config"),
        root.path().join("data"),
        root.path().join("cache"),
    );
    let core = Cliary::at(paths).unwrap();
    (root, core)
}

#[test]
fn bilingual_discovery_and_comparison_work_offline() {
    let (_root, core) = app();
    assert!(core.catalog_count().unwrap() >= 50);
    for query in ["disk usage", "磁盘空间", "找大文件"] {
        assert!(
            core.search(query, 20)
                .unwrap()
                .iter()
                .any(|r| r.tool.id == "ncdu"),
            "query: {query}"
        );
    }
    assert!(
        core.similar("ncdu", 5)
            .unwrap()
            .iter()
            .any(|x| x.id == "gdu")
    );
    let rows = core.compare(&["ncdu".into(), "gdu".into()]).unwrap();
    assert_eq!(rows.len(), 2);
}

#[test]
fn usage_is_tool_level_and_unknown_tools_survive() {
    let (_root, core) = app();
    core.record_usage("ncdu").unwrap();
    core.record_usage("my-local-tool").unwrap();
    assert!(
        core.record_usage("curl https://example.com/?token=secret")
            .is_err()
    );
    assert_eq!(core.history(Some("ncdu")).unwrap().runs, 1);
    assert_eq!(core.history(Some("my-local-tool")).unwrap().runs, 1);
    let stats = core.stats(None, None).unwrap();
    assert_eq!(stats.total_runs, 2);
    assert_eq!(stats.tools_used, 2);
    assert_eq!(core.machine_id().unwrap().len(), 32);
    let db = rusqlite::Connection::open(&core.paths.user_db).unwrap();
    let mut stmt = db.prepare("PRAGMA table_info(usage_events)").unwrap();
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        columns,
        vec!["id", "executable", "tool_id", "timestamp", "machine_id"]
    );
}

#[test]
fn catalog_replacement_preserves_user_data() {
    let (_root, core) = app();
    core.set_favorite("ncdu", true).unwrap();
    core.save_note("ncdu", "use on servers").unwrap();
    core.record_usage("ncdu").unwrap();
    let next = core.paths.data_dir.join("next.db");
    let tools = core.all_tools().unwrap();
    let categories = core.categories().unwrap();
    let tags = core.tags().unwrap();
    cliary_catalog::build_database(&next, &tools, &categories, &tags, 2).unwrap();
    std::fs::rename(next, &core.paths.catalog_db).unwrap();
    assert_eq!(core.catalog_version().unwrap(), 2);
    let detail = core.tool_detail("ncdu").unwrap().unwrap();
    assert!(detail.favorite);
    assert_eq!(detail.note.as_deref(), Some("use on servers"));
    assert_eq!(detail.run_count, 1);
}

#[test]
fn wrapped_respects_local_year_edges_leap_day_and_tool_identity() {
    use chrono::TimeZone;
    let (_root, core) = app();
    let db = rusqlite::Connection::open(&core.paths.user_db).unwrap();
    let local = |year, month, day, hour, minute, second| {
        chrono::Local
            .with_ymd_and_hms(year, month, day, hour, minute, second)
            .earliest()
            .unwrap()
            .timestamp()
    };
    for (exe, id, timestamp) in [
        ("ncdu", Some("ncdu"), local(2023, 12, 31, 23, 59, 59)),
        ("rg", Some("ripgrep"), local(2024, 1, 1, 0, 0, 0)),
        ("ripgrep", Some("ripgrep"), local(2024, 2, 29, 12, 0, 0)),
        ("rg", Some("ripgrep"), local(2024, 2, 29, 12, 0, 1)),
        ("my-tool", None, local(2024, 12, 31, 23, 59, 59)),
        ("ncdu", Some("ncdu"), local(2025, 1, 1, 0, 0, 0)),
    ] {
        db.execute("INSERT INTO usage_events(executable,tool_id,timestamp,machine_id) VALUES (?1,?2,?3,'fixture')", rusqlite::params![exe, id, timestamp]).unwrap();
    }
    let report = core.wrapped(Some(2024)).unwrap();
    assert_eq!(report.total_runs, 4);
    assert_eq!(report.active_days, 3);
    assert_eq!(report.tools_used, 2);
    assert_eq!(
        report.first_recorded.as_deref(),
        Some("2024-01-01 00:00:00")
    );
    assert_eq!(report.last_recorded.as_deref(), Some("2024-12-31 23:59:59"));
    assert_eq!(report.top_tools[0].name, "ripgrep");
    assert_eq!(report.top_tools[0].count, 3);
    assert_eq!(report.top_tools[1].name, "my-tool");
    assert_eq!(report.monthly_activity.len(), 12);
    assert_eq!(report.monthly_activity[0].name, "2024-01");
    assert_eq!(report.monthly_activity[0].count, 1);
    assert_eq!(report.monthly_activity[1].count, 2);
    assert_eq!(report.monthly_activity[2].count, 0);
    assert_eq!(report.monthly_activity[11].count, 1);
    assert_eq!(
        report.monthly_activity.iter().map(|m| m.count).sum::<u64>(),
        report.total_runs
    );
    assert_eq!(core.wrapped(Some(2023)).unwrap().total_runs, 1);
    assert_eq!(core.wrapped(Some(2025)).unwrap().total_runs, 1);
    assert_eq!(core.history(None).unwrap().runs, 6);
}

#[test]
fn wrapped_defaults_to_local_year_and_handles_empty_invalid_and_top_ten() {
    use chrono::{Datelike, TimeZone};
    let (_root, core) = app();
    let empty = core.wrapped(None).unwrap();
    assert_eq!(empty.year, chrono::Local::now().year());
    assert!(empty.is_current_year);
    assert_eq!(empty.total_runs, 0);
    assert_eq!(empty.active_days, 0);
    assert_eq!(empty.tools_used, 0);
    assert!(empty.first_recorded.is_none());
    assert!(empty.last_recorded.is_none());
    assert!(empty.top_tools.is_empty());
    assert_eq!(empty.monthly_activity.len(), 12);
    assert!(empty.monthly_activity.iter().all(|m| m.count == 0));
    for year in [i32::MIN, -1, 0, 9999, i32::MAX] {
        assert!(core.wrapped(Some(year)).is_err());
    }
    let db = rusqlite::Connection::open(&core.paths.user_db).unwrap();
    let timestamp = chrono::Local
        .with_ymd_and_hms(2024, 5, 1, 12, 0, 0)
        .earliest()
        .unwrap()
        .timestamp();
    for i in (0..12).rev() {
        db.execute(
            "INSERT INTO usage_events(executable,timestamp,machine_id) VALUES (?1,?2,'fixture')",
            rusqlite::params![format!("tool-{i:02}"), timestamp],
        )
        .unwrap();
    }
    let report = core.wrapped(Some(2024)).unwrap();
    assert_eq!(report.tools_used, 12);
    assert_eq!(report.top_tools.len(), 10);
    assert_eq!(report.top_tools[0].name, "tool-00");
    assert_eq!(report.top_tools[9].name, "tool-09");
    assert!(!report.is_current_year);
}
