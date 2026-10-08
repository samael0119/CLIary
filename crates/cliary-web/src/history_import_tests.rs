use super::*;
use cliary_core::Paths;
fn fixture(lang: &str) -> (tempfile::TempDir, App) {
    let root = tempfile::tempdir().unwrap();
    let core = Cliary::at(Paths::new(
        root.path().join("config"),
        root.path().join("data"),
        root.path().join("cache"),
    ))
    .unwrap();
    core.set_language(lang).unwrap();
    (
        root,
        App {
            core: Arc::new(core),
            csrf: "test".into(),
            imports: Default::default(),
        },
    )
}
fn form(root: &std::path::Path, shell: &str, content: &str) -> PreviewForm {
    let file = root.join("history");
    std::fs::write(&file, content).unwrap();
    PreviewForm {
        csrf: "test".into(),
        source: "custom".into(),
        shell: shell.into(),
        path: file.to_string_lossy().into_owned(),
        aliases: String::new(),
    }
}
fn token(html: &str) -> String {
    html.split("name='token' value='")
        .nth(1)
        .unwrap()
        .split('\'')
        .next()
        .unwrap()
        .into()
}
async fn confirm(app: &App, token: &str) -> (StatusCode, Html<String>) {
    apply(
        State(app.clone()),
        Form(ApplyForm {
            csrf: app.csrf.clone(),
            token: token.into(),
        }),
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn preview_is_private_and_apply_uses_reviewed_alias_snapshot_once() {
    let (root, app) = fixture("zh-CN");
    let mut form = form(
        root.path(),
        "zsh",
        ": 1704067200:0;gst --password SECRET\n: 1704153600:0;jq SECRET.json\n",
    );
    let aliases = root.path().join("aliases");
    std::fs::write(&aliases, "alias gst='git status'\n").unwrap();
    form.aliases = aliases.to_string_lossy().into_owned();
    let html = preview(State(app.clone()), Form(form)).await.unwrap().1.0;
    assert!(html.contains("确认导入"));
    assert!(html.contains("gst</code> → <code>git"));
    assert!(!html.contains("SECRET"));
    assert_eq!(app.core.history(None).unwrap().runs, 0);
    let db = rusqlite::Connection::open(&app.core.paths.user_db).unwrap();
    let machine: u64 = db
        .query_row(
            "SELECT count(*) FROM meta WHERE key='machine_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(machine, 0);
    std::fs::write(root.path().join("history"), ": 1704240000:0;ncdu /tmp\n").unwrap();
    std::fs::remove_file(&aliases).unwrap();
    let token = token(&html);
    let (status, html) = confirm(&app, &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.0.contains("历史已导入"));
    assert!(!html.0.contains("SECRET"));
    assert_eq!(app.core.history(Some("git")).unwrap().runs, 1);
    assert_eq!(app.core.history(Some("ncdu")).unwrap().runs, 0);
    assert_eq!(app.core.wrapped(Some(2024)).unwrap().total_runs, 2);
    assert!(!app.core.history_import_onboarding_pending().unwrap());
    assert_eq!(confirm(&app, &token).await.0, StatusCode::GONE);
    assert_eq!(app.core.history(None).unwrap().runs, 2);
}
#[tokio::test]
async fn both_mutations_require_csrf_before_reading_or_applying() {
    let (root, app) = fixture("en");
    let mut form = form(root.path(), "bash", "git status\n");
    form.csrf = "bad".into();
    assert_eq!(
        preview(State(app.clone()), Form(form)).await.unwrap_err().0,
        StatusCode::FORBIDDEN
    );
    assert!(app.imports.0.lock().unwrap().is_empty());
    assert_eq!(
        apply(
            State(app.clone()),
            Form(ApplyForm {
                csrf: "bad".into(),
                token: "anything".into()
            })
        )
        .await
        .unwrap_err()
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(app.core.history(None).unwrap().runs, 0);
    assert!(app.core.undated_history().unwrap().is_empty());
}
#[tokio::test]
async fn errors_preserve_editable_escaped_values_and_do_not_import() {
    for lang in ["en", "zh-CN"] {
        let (_root, app) = fixture(lang);
        let form = PreviewForm {
            csrf: "test".into(),
            source: "custom".into(),
            path: "/missing/<bad>&\"'".into(),
            shell: "bash".into(),
            aliases: String::new(),
        };
        let (status, html) = preview(State(app.clone()), Form(form)).await.unwrap();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(html.0.contains("role='alert'"));
        assert!(html.0.contains("/missing/&lt;bad&gt;&amp;&quot;&#39;"));
        assert!(!html.0.contains("<bad>"));
        assert!(html.0.contains("value='bash' selected"));
        assert!(html.0.contains("name='path' required"));
        assert_eq!(app.core.history(None).unwrap().runs, 0);
        let expired = confirm(&app, "missing").await;
        assert_eq!(expired.0, StatusCode::GONE);
        assert!(expired.1.0.contains("href='/history/import'"));
    }
}
#[tokio::test]
async fn undated_and_duplicate_states_are_honest_in_both_languages() {
    for lang in ["en", "zh-CN"] {
        let (root, app) = fixture(lang);
        let first = preview(
            State(app.clone()),
            Form(form(root.path(), "bash", "git status\ngit diff\ncd /tmp\n")),
        )
        .await
        .unwrap()
        .1
        .0;
        assert!(first.contains(tr(lang, "No reliable dates found.", "未找到可靠日期")));
        assert_eq!(confirm(&app, &token(&first)).await.0, StatusCode::OK);
        assert_eq!(app.core.history(None).unwrap().runs, 0);
        assert_eq!(app.core.undated_history().unwrap()[0].occurrences, 2);
        let repeat = preview(
            State(app.clone()),
            Form(form(root.path(), "bash", "git status\ngit diff\ncd /tmp\n")),
        )
        .await
        .unwrap()
        .1
        .0;
        assert!(repeat.contains("disabled"));
        assert!(repeat.contains(tr(lang, "Nothing new to import", "无需导入")));
        assert_eq!(app.core.undated_history().unwrap()[0].occurrences, 2);
    }
}
#[tokio::test]
async fn expired_and_evicted_previews_cannot_write() {
    let (root, app) = fixture("en");
    let first = preview(
        State(app.clone()),
        Form(form(root.path(), "zsh", ": 1704067200:0;git status\n")),
    )
    .await
    .unwrap()
    .1
    .0;
    let first = token(&first);
    app.imports
        .0
        .lock()
        .unwrap()
        .get_mut(&first)
        .unwrap()
        .created = Instant::now() - PREVIEW_TTL;
    assert_eq!(confirm(&app, &first).await.0, StatusCode::GONE);
    let mut tokens = Vec::new();
    for _ in 0..MAX_PREVIEWS + 1 {
        let html = preview(
            State(app.clone()),
            Form(form(root.path(), "zsh", ": 1704067200:0;git status\n")),
        )
        .await
        .unwrap()
        .1
        .0;
        tokens.push(token(&html));
    }
    assert_eq!(app.imports.0.lock().unwrap().len(), MAX_PREVIEWS);
    assert_eq!(confirm(&app, &tokens[0]).await.0, StatusCode::GONE);
    assert_eq!(app.core.history(None).unwrap().runs, 0);
    assert_eq!(
        confirm(&app, tokens.last().unwrap()).await.0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn entry_points_and_get_never_create_history_records() {
    for lang in ["en", "zh-CN"] {
        let (_root, app) = fixture(lang);
        let html = choose(State(app.clone())).await.unwrap().0;
        assert!(html.contains("action='/history/import/preview'"));
        assert!(html.contains("name='aliases'"));
        assert!(html.contains("id='history-shell'"));
        assert!(html.contains("value='fish'"));
        let history = crate::history(State(app.clone())).await.unwrap().0;
        assert!(
            history.find("href='/history/import'").unwrap() < history.find("usage-steps").unwrap()
        );
        assert_eq!(app.core.history(None).unwrap().runs, 0);
        assert!(app.core.undated_history().unwrap().is_empty());
    }
}
