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
fn retired_catalog_ids_keep_their_history_and_first_seen_dates() {
    use chrono::TimeZone;
    let (_root, core) = app();
    let old = chrono::Local
        .with_ymd_and_hms(2023, 12, 31, 12, 0, 0)
        .earliest()
        .unwrap()
        .timestamp();
    let recent = chrono::Local
        .with_ymd_and_hms(2024, 2, 29, 12, 0, 0)
        .earliest()
        .unwrap()
        .timestamp();
    let db = rusqlite::Connection::open(&core.paths.user_db).unwrap();
    for exe in ["btm", "http", "tsc"] {
        core.record_usage(exe).unwrap();
        core.record_usage(exe).unwrap();
    }
    db.execute(
        "UPDATE usage_events SET timestamp=CASE WHEN id%2=1 THEN ?1 ELSE ?2 END",
        rusqlite::params![old, recent],
    )
    .unwrap();
    let mut tools = core
        .all_tools()
        .unwrap()
        .into_iter()
        .filter(|t| !["bottom", "httpie", "typescript"].contains(&t.id.as_str()))
        .collect::<Vec<_>>();
    for tool in &mut tools {
        tool.similar
            .retain(|id| !["bottom", "httpie", "typescript"].contains(&id.as_str()));
    }
    let next = core.paths.data_dir.join("next.db");
    cliary_catalog::build_database(
        &next,
        &tools,
        &core.categories().unwrap(),
        &core.tags().unwrap(),
        2,
    )
    .unwrap();
    std::fs::rename(next, &core.paths.catalog_db).unwrap();
    for id in ["bottom", "httpie", "typescript"] {
        assert!(core.get_tool(id).unwrap().is_none());
        let summary = core.history(Some(id)).unwrap();
        assert_eq!(summary.runs, 2, "retired ID {id}");
        assert_eq!(summary.active_days, 2);
        assert_eq!(summary.first_used.as_deref(), Some("2023-12-31 12:00:00"));
        assert_eq!(summary.last_used.as_deref(), Some("2024-02-29 12:00:00"));
    }
    for exe in ["btm", "http", "tsc"] {
        assert_eq!(core.history(Some(exe)).unwrap().runs, 2);
    }
    let report = core.stats(None, Some(2024)).unwrap();
    assert_eq!(report.total_runs, 3);
    assert_eq!(report.active_days, 1);
    assert_eq!(report.tools_used, 3);
    assert_eq!(report.new_tools, 0); // First recorded in the previous year.
    assert!(report.new_tool_names.is_empty());
    assert_eq!(
        report
            .top_tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["bottom", "httpie", "typescript"]
    );
    assert_eq!(report.category_usage[0].name, "uncategorized");
    assert_eq!(report.category_usage[0].count, 3);
    core.record_usage("btm").unwrap(); // Removed entries are now unrecognized.
    assert_eq!(core.history(Some("bottom")).unwrap().runs, 2);
    assert_eq!(core.history(Some("btm")).unwrap().runs, 3);
    assert_eq!(core.stats(Some(30), None).unwrap().total_runs, 1);
    assert_eq!(core.stats(None, None).unwrap().total_runs, 7);
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM usage_events WHERE tool_id='bottom'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM usage_events WHERE tool_id IS NULL",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    let missing = core.history(Some("never-recorded")).unwrap();
    assert_eq!(missing.runs, 0);
    assert!(missing.first_used.is_none() && missing.last_used.is_none());
}

#[test]
fn unknown_command_matching_a_catalog_id_does_not_break_stats() {
    let (_root, core) = app();
    // The real Catalog entry is bottom -> btm. A private command named bottom
    // is valid to capture, but has no recorded Catalog ID.
    core.record_usage("bottom").unwrap();
    let db = rusqlite::Connection::open(&core.paths.user_db).unwrap();
    assert!(
        db.query_row("SELECT tool_id FROM usage_events", [], |r| r
            .get::<_, Option<String>>(0))
            .unwrap()
            .is_none()
    );
    let report = core.stats(Some(30), None).unwrap();
    assert_eq!(report.total_runs, 1);
    assert_eq!(report.new_tool_names, ["bottom"]);
    assert_eq!(report.top_tools[0].name, "bottom");
    assert_eq!(core.history(Some("bottom")).unwrap().runs, 1);
}

#[test]
fn historical_id_remains_queryable_when_catalog_resolution_changes() {
    let (_root, core) = app();
    core.record_usage("btm").unwrap();
    let mut tools = core.all_tools().unwrap();
    for tool in &mut tools {
        if tool.id == "bottom" {
            tool.id = "bottom-next".into();
            tool.name = "bottom".into();
            tool.executables = vec!["btm-next".into()];
        }
        for similar in &mut tool.similar {
            if similar == "bottom" {
                *similar = "bottom-next".into();
            }
        }
    }
    let next = core.paths.data_dir.join("next.db");
    cliary_catalog::build_database(
        &next,
        &tools,
        &core.categories().unwrap(),
        &core.tags().unwrap(),
        2,
    )
    .unwrap();
    std::fs::rename(next, &core.paths.catalog_db).unwrap();
    assert_eq!(core.get_tool("bottom").unwrap().unwrap().id, "bottom-next");
    // Display-name lookup now resolves differently; the saved ID still counts.
    assert_eq!(core.history(Some("bottom")).unwrap().runs, 1);
    assert_eq!(core.history(Some("btm")).unwrap().runs, 1);
    core.record_usage("btm-next").unwrap();
    assert_eq!(core.history(Some("bottom-next")).unwrap().runs, 1);
    assert_eq!(core.stats(Some(30), None).unwrap().total_runs, 2);
    assert_eq!(core.history(None).unwrap().runs, 2);
}

#[test]
fn newly_recognized_alias_keeps_the_original_first_recorded_date() {
    use chrono::TimeZone;
    let (_root, core) = app();
    let db = rusqlite::Connection::open(&core.paths.user_db).unwrap();
    let old = chrono::Local
        .with_ymd_and_hms(2023, 12, 31, 12, 0, 0)
        .earliest()
        .unwrap()
        .timestamp();
    let new = chrono::Local
        .with_ymd_and_hms(2024, 1, 1, 12, 0, 0)
        .earliest()
        .unwrap()
        .timestamp();
    db.execute("INSERT INTO usage_events(executable,tool_id,timestamp,machine_id) VALUES ('btm',NULL,?1,'fixture'),('btm','bottom',?2,'fixture')", rusqlite::params![old, new]).unwrap();
    let stats = core.stats(None, Some(2024)).unwrap();
    assert_eq!(stats.total_runs, 1);
    assert_eq!(stats.new_tools, 0);
    assert!(stats.new_tool_names.is_empty());
    assert_eq!(core.history(Some("bottom")).unwrap().runs, 2);
    assert_eq!(core.history(Some("btm")).unwrap().runs, 2);
    assert_eq!(core.history(Some("not-a-command")).unwrap().runs, 0);
    let empty = core.stats(None, Some(2022)).unwrap();
    assert_eq!(empty.total_runs, 0);
    assert_eq!(empty.new_tools, 0);
}
