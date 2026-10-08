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
        vec![
            "id",
            "executable",
            "tool_id",
            "timestamp",
            "machine_id",
            "source"
        ]
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
fn comparison_keeps_catalog_evidence_and_missing_features() {
    let (_root, core) = app();
    let rows = core
        .compare(&["ncdu".into(), "dust".into(), "du".into()])
        .unwrap();
    assert_eq!(
        rows[0].description,
        core.tool_detail("ncdu").unwrap().unwrap().tool.description
    );
    assert_eq!(rows[0].common_commands, vec!["ncdu /", "ncdu -x /"]);
    assert_eq!(rows[0].features.get("tui"), Some(&true));
    assert_eq!(rows[1].features.get("tui"), Some(&false));
    assert_eq!(rows[2].features.get("tui"), None);
    assert_eq!(rows[1].features.get("parallel_scan"), None);
    let json = serde_json::to_value(&rows).unwrap();
    assert_eq!(json[0]["common_commands"][0], "ncdu /");
    assert_eq!(json[1]["features"]["tui"], false);
    assert!(json[1]["features"].get("parallel_scan").is_none());
}

#[test]
fn comparison_resolves_aliases_without_duplicate_columns() {
    let (_root, core) = app();
    let rows = core
        .compare(&["rg".into(), "ripgrep".into(), "grep".into()])
        .unwrap();
    assert_eq!(
        rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["rg", "grep"]
    );
    assert!(core.compare(&["rg".into(), "ripgrep".into()]).is_err());
    assert!(core.compare(&["ncdu".into(), "not-a-tool".into()]).is_err());
    assert!(core.compare(&["ncdu".into()]).is_err());
    assert!(core.compare(&vec!["ncdu".into(); 9]).is_err());
    assert_eq!(
        cliary_core::compare_feature_label("future-feature", "zh-CN"),
        "future-feature"
    );
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

#[test]
fn removed_catalog_favorites_do_not_break_listing_or_removal() {
    let (_root, core) = app();
    core.set_favorite("ncdu", true).unwrap();
    core.set_favorite("gdu", true).unwrap();
    let tools = core.all_tools().unwrap();
    replace_catalog_without(&core, &tools, "ncdu");
    assert_eq!(core.favorites().unwrap().len(), 1);
    core.set_favorite("ncdu", false).unwrap();
    core.set_favorite("ncdu", false).unwrap();
    assert!(core.set_favorite("ncdu", true).is_err());
}

fn replace_catalog_without(core: &Cliary, tools: &[cliary_core::Tool], id: &str) {
    let mut remaining = tools.to_vec();
    remaining.retain(|tool| tool.id != id);
    for tool in &mut remaining {
        tool.similar.retain(|similar| similar != id);
    }
    let next = core.paths.data_dir.join("next.db");
    cliary_catalog::build_database(
        &next,
        &remaining,
        &core.categories().unwrap(),
        &core.tags().unwrap(),
        2,
    )
    .unwrap();
    std::fs::rename(next, &core.paths.catalog_db).unwrap();
}

#[test]
fn favorite_metadata_recovers_and_removal_preserves_notes_and_history() {
    let (_root, core) = app();
    core.set_favorite("ncdu", true).unwrap();
    core.save_note("ncdu", "saved before catalog update")
        .unwrap();
    core.record_usage("ncdu").unwrap();
    let tools = core.all_tools().unwrap();
    let catalog_copy = core.paths.data_dir.join("original.db");
    std::fs::copy(&core.paths.catalog_db, &catalog_copy).unwrap();
    replace_catalog_without(&core, &tools, "ncdu");
    let entries = core.favorite_entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "ncdu");
    assert!(entries[0].tool.is_none());
    std::fs::rename(catalog_copy, &core.paths.catalog_db).unwrap();
    assert_eq!(
        core.favorite_entries().unwrap()[0]
            .tool
            .as_ref()
            .unwrap()
            .id,
        "ncdu"
    );
    replace_catalog_without(&core, &tools, "ncdu");
    core.remove_favorite("ncdu").unwrap();
    core.remove_favorite("ncdu").unwrap();
    assert!(core.favorite_entries().unwrap().is_empty());
    let db = rusqlite::Connection::open(&core.paths.user_db).unwrap();
    assert_eq!(
        db.query_row("SELECT body FROM notes WHERE tool_id='ncdu'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "saved before catalog update"
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM usage_events WHERE tool_id='ncdu'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn favorite_ids_are_not_reinterpreted_as_current_aliases() {
    let (_root, core) = app();
    core.set_favorite("ncdu", true).unwrap();
    core.set_favorite("gdu", true).unwrap();
    let mut tools = core.all_tools().unwrap();
    tools
        .iter_mut()
        .find(|t| t.id == "gdu")
        .unwrap()
        .aliases
        .push("ncdu".into());
    replace_catalog_without(&core, &tools, "ncdu");
    let entries = core.favorite_entries().unwrap();
    assert!(
        entries
            .iter()
            .find(|e| e.id == "ncdu")
            .unwrap()
            .tool
            .is_none()
    );
    assert_eq!(core.favorites().unwrap()[0].id, "gdu");
    core.set_favorite("ncdu", false).unwrap();
    core.remove_favorite("ncdu").unwrap();
    assert_eq!(core.favorite_entries().unwrap()[0].id, "gdu");
}

#[test]
fn normal_favorite_aliases_and_catalog_errors_still_work() {
    let (_root, core) = app();
    core.set_favorite("btm", true).unwrap();
    core.set_favorite("btm", true).unwrap();
    assert_eq!(core.favorite_entries().unwrap()[0].id, "bottom");
    core.set_favorite("btm", false).unwrap();
    assert!(core.favorite_entries().unwrap().is_empty());
    core.set_favorite("gdu", true).unwrap();
    std::fs::remove_file(&core.paths.catalog_db).unwrap();
    assert!(core.favorite_entries().is_err());
    core.remove_favorite("gdu").unwrap();
}
