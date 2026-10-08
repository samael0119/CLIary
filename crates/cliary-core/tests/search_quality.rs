use cliary_core::{Cliary, Paths};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    group: String,
    query: String,
    expected: Vec<String>,
    rank: usize,
}
fn fixture() -> (tempfile::TempDir, Cliary) {
    let root = tempfile::tempdir().unwrap();
    let core = Cliary::at(Paths::new(
        root.path().join("config"),
        root.path().join("data"),
        root.path().join("cache"),
    ))
    .unwrap();
    (root, core)
}
#[test]
fn independent_queries_measure_discovery_quality() {
    let (_root, core) = fixture();
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../../tests/fixtures/search-queries.json")).unwrap();
    let mut task_count = 0;
    let mut task_hits = 0;
    let mut misses = Vec::new();
    for case in cases {
        let results = core.search(&case.query, 50).unwrap();
        let actual: Vec<_> = results
            .iter()
            .take(case.rank)
            .map(|r| r.tool.id.as_str())
            .collect();
        let ok = if case.expected.is_empty() {
            results.is_empty()
        } else {
            actual
                .iter()
                .any(|id| case.expected.iter().any(|e| e == id))
        };
        if case.group == "task" {
            task_count += 1;
            task_hits += usize::from(ok);
            if !ok {
                misses.push(format!("{}: {:?}", case.query, actual));
            }
        } else {
            assert!(ok, "{}: {:?}", case.query, actual);
        }
    }
    eprintln!("Task Top-3: {task_hits}/{task_count}; misses: {misses:?}");
    assert!(task_count >= 40);
    assert!(task_hits * 100 >= task_count * 90, "{misses:?}");
}
#[test]
fn identity_limit_and_catalog_metadata_remain_correct() {
    let (_root, core) = fixture();
    for tool in core.all_tools().unwrap() {
        for name in std::iter::once(&tool.id)
            .chain(std::iter::once(&tool.name))
            .chain(&tool.aliases)
            .chain(&tool.executables)
        {
            assert_eq!(core.search(name, 1).unwrap()[0].tool.id, tool.id, "{name}");
        }
        for lang in ["en", "zh-CN"] {
            assert!(!tool.keywords[lang].is_empty(), "{}", tool.id);
        }
    }
    assert!(core.search("disk", 0).unwrap().is_empty());
    assert!(core.search(&"a".repeat(4097), 5).is_err());
    core.set_favorite("ncdu", true).unwrap();
    core.record_usage("ncdu").unwrap();
    let result = &core.search("ncdu", 1).unwrap()[0];
    assert!(result.favorite);
    assert_eq!(result.run_count, 1);
    assert_eq!(
        core.search("Find files", 5)
            .unwrap()
            .iter()
            .map(|r| &r.tool.id)
            .collect::<Vec<_>>(),
        core.search("find file", 5)
            .unwrap()
            .iter()
            .map(|r| &r.tool.id)
            .collect::<Vec<_>>()
    );
    assert!(!core.search("磁盘占用 disk usage", 5).unwrap().is_empty());
}
#[test]
fn bundle_refresh_preserves_custom_catalogs_and_user_data() {
    let (_root, core) = fixture();
    core.set_favorite("ncdu", true).unwrap();
    core.save_note("ncdu", "keep me").unwrap();
    core.record_usage("ncdu").unwrap();
    let db = rusqlite::Connection::open(&core.paths.catalog_db).unwrap();
    db.execute(
        "UPDATE meta SET value='previous-bundle' WHERE key='bundled_revision'",
        [],
    )
    .unwrap();
    drop(db);
    let reopened = Cliary::at(core.paths.clone()).unwrap();
    let detail = reopened.tool_detail("ncdu").unwrap().unwrap();
    assert!(detail.favorite);
    assert_eq!(detail.note.as_deref(), Some("keep me"));
    assert_eq!(detail.run_count, 1);
    let db = rusqlite::Connection::open(&core.paths.catalog_db).unwrap();
    db.execute(
        "UPDATE meta SET value='previous-bundle' WHERE key='bundled_revision'",
        [],
    )
    .unwrap();
    db.execute(
        "UPDATE tools SET data=json_set(data,'$.description.en','custom content') WHERE id='ncdu'",
        [],
    )
    .unwrap();
    drop(db);
    let reopened = Cliary::at(core.paths.clone()).unwrap();
    assert_eq!(
        reopened.get_tool("ncdu").unwrap().unwrap().description["en"],
        "custom content"
    );
    let db = rusqlite::Connection::open(&core.paths.catalog_db).unwrap();
    db.execute("DELETE FROM meta WHERE key LIKE 'bundled_%'", [])
        .unwrap();
    drop(db);
    assert_eq!(
        Cliary::at(core.paths.clone())
            .unwrap()
            .get_tool("ncdu")
            .unwrap()
            .unwrap()
            .description["en"],
        "custom content"
    );
    let db = rusqlite::Connection::open(&core.paths.catalog_db).unwrap();
    db.execute("UPDATE meta SET value='99' WHERE key='version'", [])
        .unwrap();
    drop(db);
    assert_eq!(
        Cliary::at(core.paths.clone())
            .unwrap()
            .catalog_version()
            .unwrap(),
        99
    );
    core.refresh_bundled_catalog().unwrap();
    assert_eq!(core.catalog_version().unwrap(), 1);
    assert_ne!(
        core.get_tool("ncdu").unwrap().unwrap().description["en"],
        "custom content"
    );
    let detail = core.tool_detail("ncdu").unwrap().unwrap();
    assert!(detail.favorite);
    assert_eq!(detail.note.as_deref(), Some("keep me"));
    assert_eq!(detail.run_count, 1);
}
