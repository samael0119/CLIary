use cliary_core::{Cliary, HistoryFormat, Paths};
use rusqlite::{Connection, params};
use std::path::Path;
fn app() -> (tempfile::TempDir, Cliary) {
    let root = tempfile::tempdir().unwrap();
    let app = Cliary::at(Paths::new(
        root.path().join("config"),
        root.path().join("data"),
        root.path().join("cache"),
    ))
    .unwrap();
    (root, app)
}
fn file(root: &Path, text: &str) -> std::path::PathBuf {
    let path = root.join("history");
    std::fs::write(&path, text).unwrap();
    path
}
#[test]
fn preview_is_private_and_import_is_idempotent_preserving_multiplicity() {
    let (root, core) = app();
    let path = file(
        root.path(),
        ": 1704067200:0;git --token SECRET_ONE\n: 1704067200:0;git --token SECRET_TWO\n: 1704153600:0;/usr/bin/jq secret.json\n",
    );
    let preview = core
        .import_history(&path, HistoryFormat::Zsh, false)
        .unwrap();
    assert_eq!(preview.new_timed, 3);
    assert_eq!(preview.sample_tools, vec!["git", "jq"]);
    assert_eq!(core.history(None).unwrap().runs, 0);
    let db = Connection::open(&core.paths.user_db).unwrap();
    assert_eq!(
        db.query_row::<u64, _, _>(
            "SELECT COUNT(*) FROM meta WHERE key='machine_id'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
    assert!(!serde_json::to_string(&preview).unwrap().contains("SECRET"));
    assert_eq!(
        core.import_history(&path, HistoryFormat::Zsh, true)
            .unwrap()
            .new_timed,
        3
    );
    let repeat = core
        .import_history(&path, HistoryFormat::Zsh, true)
        .unwrap();
    assert_eq!(repeat.new_timed, 0);
    assert_eq!(repeat.duplicates, 3);
    assert_eq!(core.history(Some("git")).unwrap().runs, 2);
    assert_eq!(core.wrapped(Some(2024)).unwrap().total_runs, 3);
    let saved: String = db
        .query_row(
            "SELECT group_concat(executable || source || machine_id) FROM usage_events",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!saved.contains("SECRET"));
    assert!(!saved.contains("secret.json"));
    // A renamed copy and another format do not duplicate the same observations.
    let copy = root.path().join("copy");
    std::fs::copy(&path, &copy).unwrap();
    assert_eq!(
        core.import_history(&copy, HistoryFormat::Zsh, true)
            .unwrap()
            .new_timed,
        0
    );
}
#[test]
fn untimed_observations_never_receive_a_guessed_year() {
    let (root, core) = app();
    let path = file(root.path(), "git status\ngit diff\nncdu .\ncd /tmp\n");
    let imported = core
        .import_history(&path, HistoryFormat::Bash, true)
        .unwrap();
    assert_eq!(
        (imported.undated, imported.skipped, imported.new_undated),
        (3, 1, 3)
    );
    assert_eq!(core.stats(None, None).unwrap().total_runs, 0);
    let undated = core.undated_history().unwrap();
    assert_eq!(
        (&*undated[0].executable, undated[0].occurrences),
        ("git", 2)
    );
    assert_eq!(
        core.import_history(&path, HistoryFormat::Bash, true)
            .unwrap()
            .new_undated,
        0
    );
    assert_eq!(core.wrapped(Some(2024)).unwrap().total_runs, 0);
}
#[test]
fn bash_and_fish_formats_skip_ambiguous_or_invalid_entries() {
    let (root, core) = app();
    let path = file(
        root.path(),
        "#1704067200\nFOO=private sudo -u root /usr/bin/git status\n#1704153600\ngit status | jq .\n#999999999999999999999\nncdu .\n#1704240000\ngit status\necho continuation\n",
    );
    let r = core
        .import_history(&path, HistoryFormat::Bash, true)
        .unwrap();
    assert_eq!(r.timed, 1);
    assert_eq!(r.skipped, 3);
    let path = file(
        root.path(),
        "- cmd: jq 'a: b'\n  when: 1704326400\n  paths:\n    - /SECRET_PATH\n- cmd: git status\\nrm private\n  when: 1704412800\n- cmd: ncdu .\n- cmd: curl --header token\n  when: broken\n",
    );
    let r = core
        .import_history(&path, HistoryFormat::Fish, true)
        .unwrap();
    assert_eq!((r.timed, r.undated, r.skipped), (1, 1, 2));
    assert_eq!(core.history(None).unwrap().runs, 2);
}
#[test]
fn live_overlap_deduplicates_but_other_seconds_and_tools_remain() {
    let (root, core) = app();
    let machine = core.machine_id().unwrap();
    let db = Connection::open(&core.paths.user_db).unwrap();
    db.execute("INSERT INTO usage_events(executable,tool_id,timestamp,machine_id) VALUES ('git','git',1704067200,?1)",[machine]).unwrap();
    let path = file(
        root.path(),
        ": 1704067200:0;git status\n: 1704067200:0;git diff\n: 1704067201:0;git log\n: 1704067200:0;jq .\n",
    );
    let r = core
        .import_history(&path, HistoryFormat::Zsh, true)
        .unwrap();
    assert_eq!((r.new_timed, r.duplicates), (3, 1));
    assert_eq!(
        core.import_history(&path, HistoryFormat::Zsh, true)
            .unwrap()
            .new_timed,
        0
    );
    assert_eq!(core.history(None).unwrap().runs, 4);
    assert_eq!(
        db.query_row::<u64, _, _>(
            "SELECT COUNT(*) FROM usage_events WHERE source='zsh'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        3
    );
}
#[test]
fn version_one_migrates_without_losing_favorites_notes_or_usage() {
    let (root, core) = app();
    let db = Connection::open(&core.paths.user_db).unwrap();
    db.execute_batch("DROP INDEX idx_usage_identity_time; ALTER TABLE usage_events DROP COLUMN source; PRAGMA user_version=1;").unwrap();
    db.execute(
        "INSERT INTO usage_events VALUES (1,'git','git',1704067200,'old-machine')",
        [],
    )
    .unwrap();
    db.execute("INSERT INTO favorites VALUES ('git',1704067200)", [])
        .unwrap();
    db.execute("INSERT INTO notes VALUES ('git','retain',1704067200)", [])
        .unwrap();
    drop(db);
    let upgraded = Cliary::at(core.paths.clone()).unwrap();
    assert_eq!(upgraded.history(None).unwrap().runs, 1);
    assert_eq!(upgraded.favorite_entries().unwrap().len(), 1);
    assert_eq!(
        Connection::open(&upgraded.paths.user_db)
            .unwrap()
            .query_row::<String, _, _>("SELECT body FROM notes WHERE tool_id='git'", [], |r| r
                .get(0))
            .unwrap(),
        "retain"
    );
    let db = Connection::open(&upgraded.paths.user_db).unwrap();
    assert_eq!(
        db.query_row::<String, _, _>(
            "SELECT source FROM usage_events WHERE id=?1",
            params![1],
            |r| r.get(0)
        )
        .unwrap(),
        "capture"
    );
    let invalid = file(
        root.path(),
        ": broken:0;git secret\n: 999999999999:0;git secret\n",
    );
    assert_eq!(
        upgraded
            .import_history(&invalid, HistoryFormat::Zsh, true)
            .unwrap()
            .skipped,
        2
    );
}

#[test]
fn metafied_zsh_bytes_decode_and_failed_input_leaves_records_untouched() {
    let (root, core) = app();
    let path = root.path().join("metafied");
    let command = ": 1704067200:0;git status 包含中文\n";
    let mut encoded = Vec::new();
    for byte in command.bytes() {
        if byte >= 0x80 {
            encoded.push(0x83);
            encoded.push(byte ^ 32);
        } else {
            encoded.push(byte);
        }
    }
    std::fs::write(&path, encoded).unwrap();
    assert_eq!(
        core.import_history(&path, HistoryFormat::Zsh, true)
            .unwrap()
            .new_timed,
        1
    );
    std::fs::write(&path, [0x83]).unwrap();
    assert!(
        core.import_history(&path, HistoryFormat::Zsh, true)
            .is_err()
    );
    assert!(
        core.import_history(root.path(), HistoryFormat::Bash, true)
            .is_err()
    );
    assert_eq!(core.history(None).unwrap().runs, 1);
}
