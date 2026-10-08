use super::{App, WebResult, error, esc, page, tr};
use axum::extract::{Query, State};
use cliary_core::{InstalledTool, Tool};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fmt::Write,
    path::Path,
};

const PAGE_SIZE: usize = 100;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct InstalledQuery {
    filter: String,
    q: String,
    source: String,
    directory: String,
    category: String,
    page: usize,
}

fn directory(item: &InstalledTool) -> String {
    Path::new(&item.path)
        .parent()
        .unwrap_or(Path::new("."))
        .to_string_lossy()
        .into_owned()
}

impl InstalledQuery {
    fn matches(&self, item: &InstalledTool, tool: Option<&Tool>, needle: &str) -> bool {
        let in_scope = match self.filter.as_str() {
            "all" => true,
            "unmatched" => tool.is_none(),
            _ => tool.is_some(),
        };
        in_scope
            && (needle.is_empty()
                || item.executable.to_lowercase().contains(needle)
                || item.path.to_lowercase().contains(needle)
                || tool.is_some_and(|tool| tool.name.to_lowercase().contains(needle)))
            && (self.source.is_empty()
                || if self.source == "__unknown" {
                    item.source.as_deref().unwrap_or_default().is_empty()
                } else {
                    item.source.as_deref() == Some(&self.source)
                })
            && (self.directory.is_empty() || directory(item) == self.directory)
            && (self.category.is_empty()
                || tool.is_some_and(|tool| tool.categories.contains(&self.category)))
    }

    fn url(&self, page: usize) -> String {
        let query = Self {
            page,
            ..self.clone()
        };
        format!(
            "/installed?{}",
            serde_urlencoded::to_string(query).expect("flat query serializes")
        )
    }
}

fn option(body: &mut String, value: &str, label: &str, selected: &str) {
    write!(
        body,
        "<option value='{}'{}>{}</option>",
        esc(value),
        if value == selected { " selected" } else { "" },
        esc(label)
    )
    .unwrap();
}

pub(super) async fn installed(
    State(app): State<App>,
    Query(mut query): Query<InstalledQuery>,
) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let entries = app.core.installed().map_err(error)?;
    let scanned = app.core.has_scanned().map_err(error)?;
    let tools: HashMap<_, _> = app
        .core
        .all_tools()
        .map_err(error)?
        .into_iter()
        .map(|tool| (tool.id.clone(), tool))
        .collect();
    let lookup = |item: &InstalledTool| item.tool_id.as_ref().and_then(|id| tools.get(id));
    if !matches!(query.filter.as_str(), "all" | "unmatched") {
        query.filter = "catalog".into();
    }
    let matched_count = entries.iter().filter(|item| lookup(item).is_some()).count();
    let mut body = format!(
        "<section class='page-intro'><h1>{}</h1><p>{}</p></section><div class='toolbar-card'><span>{}: {} · {}: {} · {}: {}</span><form method='post' action='/scan'><input type='hidden' name='csrf' value='{}'><button type='submit' class='btn btn-primary'>{}</button></form></div>",
        tr(&lang, "Installed Tools", "已安装工具"),
        tr(
            &lang,
            "Find executables in your saved scan by name, source or directory.",
            "按名称、来源或目录定位已保存扫描中的可执行程序。"
        ),
        tr(&lang, "All binaries", "全部程序"),
        entries.len(),
        tr(&lang, "Catalog matches", "已识别"),
        matched_count,
        tr(&lang, "Unmatched", "未收录"),
        entries.len() - matched_count,
        esc(&app.csrf),
        tr(&lang, "Scan Now", "重新扫描")
    );
    if !scanned {
        write!(
            body,
            "<section class='empty-inline'><strong>{}</strong><p>{}</p></section>",
            tr(&lang, "Scan required", "尚未执行扫描"),
            tr(
                &lang,
                "Run a scan to populate this list. No scan does not mean no tools are installed.",
                "请先扫描再查看列表；尚未扫描不代表没有安装工具。"
            )
        )
        .unwrap();
        return page(&app, "Installed", body);
    }
    body.push_str("<form method='get' action='/installed' class='installed-filters'>");
    write!(
        body,
        "<label>{}<select name='filter'>",
        tr(&lang, "Show", "识别状态")
    )
    .unwrap();
    for (value, label) in [
        ("catalog", tr(&lang, "Catalog matches", "已识别工具")),
        ("all", tr(&lang, "All binaries", "全部程序")),
        ("unmatched", tr(&lang, "Unmatched binaries", "未收录程序")),
    ] {
        option(&mut body, value, label, &query.filter);
    }
    write!(body, "</select></label><label class='installed-keyword'>{}<input name='q' type='search' value='{}' placeholder='{}'></label><label>{}<select name='source'>",
        tr(&lang, "Name or path", "名称或路径"), esc(&query.q), tr(&lang, "Search this scan", "搜索本次扫描"), tr(&lang, "Recorded source", "记录来源")).unwrap();
    option(
        &mut body,
        "",
        tr(&lang, "All sources", "全部来源"),
        &query.source,
    );
    option(
        &mut body,
        "__unknown",
        tr(&lang, "Source not detected", "来源未识别"),
        &query.source,
    );
    let sources: BTreeSet<_> = entries
        .iter()
        .filter_map(|item| item.source.as_deref())
        .filter(|s| !s.is_empty())
        .collect();
    for source in &sources {
        option(&mut body, source, source, &query.source);
    }
    if !query.source.is_empty()
        && query.source != "__unknown"
        && !sources.contains(query.source.as_str())
    {
        option(&mut body, &query.source, &query.source, &query.source);
    }
    write!(
        body,
        "</select></label><label>{}<select name='directory'>",
        tr(&lang, "Install directory", "安装目录")
    )
    .unwrap();
    option(
        &mut body,
        "",
        tr(&lang, "All directories", "全部目录"),
        &query.directory,
    );
    let directories: BTreeSet<_> = entries.iter().map(directory).collect();
    for dir in &directories {
        option(&mut body, dir, dir, &query.directory);
    }
    if !query.directory.is_empty() && !directories.contains(&query.directory) {
        option(
            &mut body,
            &query.directory,
            &query.directory,
            &query.directory,
        );
    }
    write!(
        body,
        "</select></label><label>{}<select name='category'>",
        tr(&lang, "Catalog category", "工具库类别")
    )
    .unwrap();
    option(
        &mut body,
        "",
        tr(&lang, "All categories", "全部类别"),
        &query.category,
    );
    let used_categories: BTreeSet<_> = entries
        .iter()
        .filter_map(lookup)
        .flat_map(|tool| &tool.categories)
        .collect();
    for category in app.core.categories().map_err(error)? {
        if used_categories.contains(&category.id) {
            option(
                &mut body,
                &category.id,
                cliary_core::localized(&category.name, &lang),
                &query.category,
            );
        }
    }
    if !query.category.is_empty() && !used_categories.contains(&query.category) {
        option(&mut body, &query.category, &query.category, &query.category);
    }
    write!(body, "</select></label><div class='installed-filter-actions'><button type='submit' class='btn btn-primary'>{}</button><a class='btn btn-outline' href='{}'>{}</a></div></form><p class='installed-help'>{}</p>",
        tr(&lang, "Apply filters", "应用筛选"), esc(&InstalledQuery { filter: query.filter.clone(), ..Default::default() }.url(1)),
        tr(&lang, "Clear filters", "清除筛选"),
        tr(&lang, "Directories describe where binaries were found. They do not establish package ownership. Categories apply only to catalog matches.", "目录表示程序发现位置，不代表包归属；类别筛选仅适用于已收录工具。")).unwrap();

    let needle = query.q.trim().to_lowercase();
    let filtered: Vec<_> = entries
        .iter()
        .filter(|item| query.matches(item, lookup(item), &needle))
        .collect();
    let total = filtered.len();
    let pages = total.div_ceil(PAGE_SIZE).max(1);
    let current = query.page.clamp(1, pages);
    let start = (current - 1) * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(total);
    let mut pagination = String::new();
    write!(
        pagination,
        "<div class='installed-results'><p>{}: {}–{} / {}</p>",
        tr(&lang, "Showing", "显示"),
        if total == 0 { 0 } else { start + 1 },
        end,
        total
    )
    .unwrap();
    if current > 1 {
        write!(
            pagination,
            "<a class='btn btn-outline' rel='prev' href='{}'>{}</a>",
            esc(&query.url(current - 1)),
            tr(&lang, "Previous", "上一页")
        )
        .unwrap();
    }
    if current < pages {
        write!(
            pagination,
            "<a class='btn btn-outline' rel='next' href='{}'>{}</a>",
            esc(&query.url(current + 1)),
            tr(&lang, "Next", "下一页")
        )
        .unwrap();
    }
    pagination.push_str("</div>");
    body.push_str(&pagination);
    if total == 0 {
        write!(body, "<section class='empty-inline'><strong>{}</strong><p>{}</p><a class='btn btn-outline' href='/installed?filter=all'>{}</a></section>",
            if entries.is_empty() { tr(&lang, "No binaries found", "未检测到可执行程序") } else { tr(&lang, "No matching binaries", "没有匹配的程序") },
            tr(&lang, "Try clearing filters or reviewing all binaries. Re-scan after installing new tools.", "尝试清除筛选或查看全部程序；安装新工具后需要重新扫描。"),
            tr(&lang, "View all binaries", "查看全部程序")).unwrap();
    } else {
        body.push_str(
            "<div class='installed-table-wrap'><table class='installed-table'><thead><tr>",
        );
        for label in [
            tr(&lang, "Executable", "程序"),
            tr(&lang, "Version", "版本"),
            tr(&lang, "Source", "来源"),
            tr(&lang, "Directory", "所在目录"),
        ] {
            write!(body, "<th scope='col'>{label}</th>").unwrap();
        }
        body.push_str("</tr></thead><tbody>");
        for item in &filtered[start..end] {
            let name = if let Some(tool) = lookup(item) {
                format!(
                    "<a href='/tools/{}'>{}</a>",
                    esc(&tool.id),
                    esc(&item.executable)
                )
            } else {
                esc(&item.executable)
            };
            write!(body, "<tr><td class='installed-executable'><strong>{}</strong><small>{}</small></td><td>{}</td><td>{}</td><td class='installed-directory' title='{}'>{}</td></tr>",
                name, esc(lookup(item).map(|tool| cliary_core::localized(&tool.description, &lang)).unwrap_or(tr(&lang, "Not in catalog", "工具库未收录"))),
                esc(item.version.as_deref().unwrap_or("—")),
                esc(item.source.as_deref().filter(|s| !s.is_empty()).unwrap_or(tr(&lang, "Not detected", "未识别"))),
                esc(&item.path), esc(&directory(item))).unwrap();
        }
        body.push_str("</tbody></table></div>");
        if pages > 1 {
            body.push_str(&pagination);
        }
    }
    page(&app, "Installed", body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cliary_core::{Cliary, Paths};
    use rusqlite::{Connection, params};
    use std::sync::Arc;

    fn app(lang: &str) -> (tempfile::TempDir, App) {
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
                csrf: "test-token".into(),
            },
        )
    }

    fn insert(app: &App, name: &str, path: &str, source: Option<&str>, tool_id: Option<&str>) {
        let db = Connection::open(&app.core.paths.user_db).unwrap();
        db.execute(
            "INSERT INTO installed VALUES(?1,?2,NULL,?3,?4,0)",
            params![name, path, source, tool_id],
        )
        .unwrap();
        db.execute(
            "INSERT OR REPLACE INTO meta VALUES('installed_scan_state','done')",
            [],
        )
        .unwrap();
    }

    async fn render(app: &App, query: InstalledQuery) -> String {
        installed(State(app.clone()), Query(query)).await.unwrap().0
    }

    fn rows(html: &str) -> &str {
        html.split("<tbody>")
            .nth(1)
            .unwrap()
            .split("</tbody>")
            .next()
            .unwrap()
    }

    #[tokio::test]
    async fn filters_combine_and_missing_catalog_entries_remain_browsable() {
        for lang in ["en", "zh-CN"] {
            let (_root, app) = app(lang);
            insert(&app, "ncdu", "/usr/bin/ncdu", Some("apt"), Some("ncdu"));
            insert(&app, "rg", "/custom/bin/rg", Some("cargo"), Some("rg"));
            insert(&app, "my-tool", "/opt/sdk/bin/my-tool", None, None);
            insert(
                &app,
                "old-tool",
                "/opt/sdk/bin/old-tool",
                None,
                Some("removed-from-catalog"),
            );
            let category = app.core.get_tool("ncdu").unwrap().unwrap().categories[0].clone();
            let html = render(
                &app,
                InstalledQuery {
                    q: " NCDU ".into(),
                    source: "apt".into(),
                    directory: "/usr/bin".into(),
                    category,
                    ..Default::default()
                },
            )
            .await;
            assert!(rows(&html).contains("href='/tools/ncdu'"));
            assert!(!rows(&html).contains("rg"));
            let html = render(
                &app,
                InstalledQuery {
                    filter: "unmatched".into(),
                    source: "__unknown".into(),
                    directory: "/opt/sdk/bin".into(),
                    ..Default::default()
                },
            )
            .await;
            assert!(rows(&html).contains("my-tool"));
            assert!(rows(&html).contains("old-tool"));
            assert!(!rows(&html).contains("href='/tools/"));
            assert!(rows(&html).contains(tr(lang, "Not detected", "未识别")));
            assert!(!rows(&html).contains("system"));
        }
    }

    #[tokio::test]
    async fn pagination_reaches_results_after_500_and_preserves_filters() {
        let (_root, app) = app("en");
        for i in 0..620 {
            insert(
                &app,
                &format!("bin-{i:03}"),
                &format!("/opt/开发 & tests/bin-{i:03}"),
                None,
                None,
            );
        }
        let query = InstalledQuery {
            filter: "all".into(),
            q: "BIN".into(),
            source: "__unknown".into(),
            directory: "/opt/开发 & tests".into(),
            page: 7,
            ..Default::default()
        };
        let html = render(&app, query.clone()).await;
        assert!(html.contains("Showing: 601–620 / 620"));
        assert!(rows(&html).contains("bin-619"));
        assert_eq!(rows(&html).matches("<tr>").count(), 20);
        assert!(html.contains("rel='prev'"));
        assert!(!html.contains("rel='next'"));
        let previous = query.url(6);
        let parsed: InstalledQuery =
            serde_urlencoded::from_str(previous.split_once('?').unwrap().1).unwrap();
        assert_eq!(parsed.directory, query.directory);
        assert_eq!(parsed.q, query.q);
        assert_eq!(parsed.source, query.source);
        assert_eq!(parsed.page, 6);
        assert!(html.contains(&esc(&previous)));
        let html = render(
            &app,
            InstalledQuery {
                page: usize::MAX,
                ..query
            },
        )
        .await;
        assert!(html.contains("Showing: 601–620 / 620"));
    }

    #[tokio::test]
    async fn unscanned_empty_scan_and_no_matches_have_distinct_states() {
        let (_root, app) = app("zh-CN");
        assert!(
            render(&app, InstalledQuery::default())
                .await
                .contains("尚未执行扫描")
        );
        let db = Connection::open(&app.core.paths.user_db).unwrap();
        db.execute(
            "INSERT OR REPLACE INTO meta VALUES('installed_scan_state','done')",
            [],
        )
        .unwrap();
        assert!(
            render(
                &app,
                InstalledQuery {
                    filter: "all".into(),
                    ..Default::default()
                }
            )
            .await
            .contains("未检测到可执行程序")
        );
        insert(&app, "helper", "/usr/bin/helper", None, None);
        let html = render(
            &app,
            InstalledQuery {
                filter: "all".into(),
                q: "missing".into(),
                directory: "/no-longer-present".into(),
                source: "stale-source".into(),
                category: "stale-category".into(),
                ..Default::default()
            },
        )
        .await;
        assert!(html.contains("没有匹配的程序"));
        for value in ["/no-longer-present", "stale-source", "stale-category"] {
            assert!(html.contains(&format!("value='{value}' selected")));
        }
        assert!(html.contains("href='/installed?filter=all'"));
        assert!(html.contains("显示: 0–0 / 0"));
    }

    #[tokio::test]
    async fn names_paths_sources_and_query_values_are_html_escaped() {
        let (_root, app) = app("en");
        insert(
            &app,
            "<script>alert(1)</script>",
            "/opt/a' & 中文/<script>",
            Some("bad' & source"),
            None,
        );
        let html = render(
            &app,
            InstalledQuery {
                filter: "all".into(),
                ..Default::default()
            },
        )
        .await;
        assert!(rows(&html).contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("/opt/a&#39; &amp; 中文"));
        assert!(!html.contains("<script>alert(1)</script>"));
        let html = render(
            &app,
            InstalledQuery {
                filter: "all".into(),
                q: "' onfocus='alert(1)".into(),
                ..Default::default()
            },
        )
        .await;
        assert!(html.contains("value='&#39; onfocus=&#39;alert(1)'"));
        assert!(!html.contains("value='' onfocus='"));
    }
}
