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
