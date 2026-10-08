use chrono::{Datelike, TimeZone};
use cliary_core::{Cliary, Paths};
use rusqlite::{Connection, params};
fn app() -> (tempfile::TempDir, Cliary) {
    let root = tempfile::tempdir().unwrap();
    let core = Cliary::at(Paths::new(
        root.path().join("config"),
        root.path().join("data"),
        root.path().join("cache"),
    ))
    .unwrap();
    (root, core)
}
fn stamp(year: i32, month: u32, day: u32) -> i64 {
    chrono::Local
        .with_ymd_and_hms(year, month, day, 12, 0, 0)
        .earliest()
        .unwrap()
        .timestamp()
}
fn add(db: &Connection, exe: &str, id: Option<&str>, t: i64, count: u32, source: &str) {
    for _ in 0..count {
        db.execute("INSERT INTO usage_events(executable,tool_id,timestamp,machine_id,source) VALUES (?1,?2,?3,'fixture',?4)",params![exe,id,t,source]).unwrap();
    }
}
#[test]
fn real_annual_insights_keep_first_seen_favorites_sources_and_unknown_tools() {
    let (_root, core) = app();
    let db = Connection::open(&core.paths.user_db).unwrap();
    add(&db, "git", Some("git"), stamp(2023, 5, 1), 2, "capture");
    add(&db, "git", Some("git"), stamp(2024, 5, 1), 1, "zsh");
    add(&db, "ncdu", Some("ncdu"), stamp(2024, 6, 1), 3, "bash");
    add(
        &db,
        "retired",
        Some("retired-id"),
        stamp(2023, 1, 1),
        1,
        "capture",
    );
    add(
        &db,
        "retired",
        Some("retired-id"),
        stamp(2024, 2, 29),
        1,
        "fish",
    );
    // Unmatched old alias must keep a newly recognized tool from appearing newly used.
    add(&db, "btm", None, stamp(2023, 3, 1), 1, "capture");
    add(&db, "btm", Some("bottom"), stamp(2024, 3, 1), 1, "zsh");
    db.execute(
        "INSERT INTO favorites VALUES ('git',?1)",
        [stamp(2023, 1, 1)],
    )
    .unwrap();
    db.execute(
        "INSERT INTO favorites VALUES ('missing-id',?1)",
        [stamp(2023, 2, 1)],
    )
    .unwrap();
    db.execute(
        "INSERT INTO favorites VALUES ('ncdu',?1)",
        [stamp(2024, 12, 20)],
    )
    .unwrap();
    db.execute(
        "INSERT INTO favorites VALUES ('future-id',?1)",
        [stamp(2025, 1, 1)],
    )
    .unwrap();
    db.execute("INSERT INTO history_undated VALUES ('unknown',42)", [])
        .unwrap();
    let r = core.wrapped(Some(2024)).unwrap();
    let i = &r.insights;
    assert_eq!(r.total_runs, 6);
    assert_eq!(i.new_tools_count, 1);
    assert_eq!(i.breakout_tool.as_ref().unwrap().name, "ncdu");
    assert!(i.new_tools.iter().all(|t| t.name != "bottom"));
    assert_eq!(
        i.low_activity_favorites
            .iter()
            .map(|f| (&*f.name, f.runs))
            .collect::<Vec<_>>(),
        vec![("missing-id", 0), ("git", 1)]
    );
    assert_eq!(i.source_counts.iter().map(|c| c.count).sum::<u64>(), 6);
    assert_eq!(i.undated_tools, 1);
    assert!(
        i.top_categories
            .iter()
            .any(|c| c.name == "uncategorized" && c.count == 1)
    );
    let comparison = i.comparison.as_ref().unwrap();
    assert_eq!((comparison.current_runs, comparison.previous_runs), (6, 4));
    assert!((comparison.change_percent.unwrap() - 50.0).abs() < 0.001);
    assert!(
        i.tool_changes
            .iter()
            .any(|c| c.name == "ncdu" && c.previous_runs == 0)
    );
}
#[test]
fn ytd_compares_same_local_cutoff_and_zero_baseline_is_not_infinite_growth() {
    let (_root, core) = app();
    let now = chrono::Local::now();
    let db = Connection::open(&core.paths.user_db).unwrap();
    add(
        &db,
        "git",
        Some("git"),
        stamp(now.year() - 1, 12, 31),
        5,
        "capture",
    );
    db.execute(
        "INSERT INTO favorites VALUES ('git',?1)",
        [stamp(now.year() - 2, 1, 1)],
    )
    .unwrap();
    let r = core.wrapped(None).unwrap();
    let c = r.insights.comparison.as_ref().unwrap();
    assert!(c.year_to_date);
    assert_eq!(c.previous_year, now.year() - 1);
    if now.month() < 12 {
        assert_eq!(c.previous_runs, 0);
    }
    if c.previous_runs == 0 {
        assert!(c.change_percent.is_none());
    }
    assert!(c.current_end.starts_with(&now.year().to_string()));
    assert!(c.previous_end.starts_with(&(now.year() - 1).to_string()));
    assert!(core.wrapped(Some(1)).unwrap().insights.comparison.is_none());
    assert!(
        core.wrapped(Some(now.year() + 1))
            .unwrap()
            .insights
            .low_activity_favorites
            .is_empty()
    );
}
