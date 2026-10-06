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
