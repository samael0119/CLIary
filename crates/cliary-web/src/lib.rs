mod installed;

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use cliary_core::{Cliary, localized};
use rand::RngCore;
use serde::Deserialize;
use std::{fmt::Write, sync::Arc};

#[derive(Clone)]
struct App {
    core: Arc<Cliary>,
    csrf: String,
}
#[derive(Template)]
#[template(path = "page.html")]
struct Page<'a> {
    display_title: &'a str,
    lang: &'a str,
    body: &'a str,
    csrf: &'a str,
    active: &'a str,
}
type WebResult = Result<Html<String>, (StatusCode, String)>;
type CompareDimension<'a> = (
    &'static str,
    &'static str,
    Box<dyn Fn(&cliary_core::CompareRow) -> String + 'a>,
);
fn error(e: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}
fn esc(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn tr<'a>(lang: &str, en: &'a str, zh: &'a str) -> &'a str {
    if lang == "zh-CN" { zh } else { en }
}
fn page(app: &App, title: &str, body: String) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let active = match title {
        "Home" => "home",
        "Search" => "search",
        "Installed" => "installed",
        "Categories" => "categories",
        "Favorites" => "favorites",
        "Compare" => "compare",
        "History" => "history",
        "Statistics" => "stats",
        _ => "tool",
    };
    let display_title = if lang == "zh-CN" {
        match active {
            "home" => "概览",
            "search" => "发现工具",
            "installed" => "已安装工具",
            "categories" => "分类目录",
            "favorites" => "我的收藏",
            "compare" => "工具比较",
            "history" => "使用历史",
            "stats" => "统计分析",
            _ => title,
        }
    } else {
        title
    };
    Ok(Html(
        Page {
            display_title,
            lang: &lang,
            body: &body,
            csrf: &app.csrf,
            active,
        }
        .render()
        .map_err(|e| error(e.into()))?,
    ))
}
fn check(app: &App, csrf: &str) -> Result<(), (StatusCode, String)> {
    if app.csrf == csrf {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, "invalid CSRF token".into()))
    }
}

pub fn serve(core: Cliary) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let mut bytes = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut bytes);
            let app = App {
                core: Arc::new(core),
                csrf: hex::encode(bytes),
            };
            let router = Router::new()
                .route("/", get(home))
                .route("/search", get(search))
                .route("/tools/{id}", get(tool))
                .route("/tools/{id}/favorite", post(favorite))
                .route("/tools/{id}/note", post(note))
                .route("/installed", get(installed::installed))
                .route("/scan", post(scan))
                .route("/categories", get(categories))
                .route("/favorites", get(favorites))
                .route("/compare", get(compare))
                .route("/history", get(history))
                .route("/stats", get(stats))
                .route("/language", post(set_language))
                .route("/assets/style.css", get(css))
                .route("/assets/htmx.min.js", get(htmx))
                .layer(middleware::from_fn(only_loopback_host))
                .with_state(app);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:8848").await?;
            println!("CLIary Web UI: http://127.0.0.1:8848");
            axum::serve(listener, router).await?;
            anyhow::Ok(())
        })
}
async fn only_loopback_host(request: Request<axum::body::Body>, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|x| x.to_str().ok())
        .unwrap_or("");
    if host != "127.0.0.1:8848" && host != "localhost:8848" {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}
async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/style.css"),
    )
}
async fn htmx() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../static/htmx.min.js"),
    )
}

async fn home(State(app): State<App>) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let stats = app.core.stats(Some(30), None).map_err(error)?;
    let installed = app.core.installed().map_err(error)?.len();
    let scanned = app.core.has_scanned().map_err(error)?;
    let installed_display = if scanned {
        installed.to_string()
    } else {
        "—".into()
    };
    let favorites = app.core.favorites().map_err(error)?.len();
    let catalog = app.core.catalog_count().map_err(error)?;
    let mut body = format!(
        "<section class='hero'><div class='hero-copy'><span class='eyebrow hero-eyebrow'><span class='eyebrow-line'></span>{}</span><h1>{}<br><em>{}</em></h1><p>{}</p><form class='hero-search' action='/search'><span class='search-glyph'>⌕</span><input name='q' autofocus aria-label='{}' placeholder='{}'><button>{} <span class='kbd-tag' style='background:rgba(255,255,255,0.2); border:none; color:#fff;'>↵</span></button></form><div class='hero-hint'><span>{}</span> · <span>{} <a href='/search?q=ncdu'>ncdu</a>, <a href='/search?q=ripgrep'>ripgrep</a>, <a href='/search?q=dust'>dust</a></span></div></div><div class='hero-art' aria-hidden='true'><div class='terminal'><div class='terminal-top'><span class='terminal-dots'><i></i><i></i><i></i></span><span>cliary — local workspace</span><span>⌘</span></div><div class='terminal-body'><p><span class='prompt'>❯</span> cliary search <span class='terminal-string'>“disk usage”</span></p><p class='terminal-result'><span class='terminal-check'>●</span> ncdu <span>interactive disk usage</span></p><p class='terminal-result'><span class='terminal-check'>●</span> gdu <span>fast disk analyzer</span></p><p><span class='prompt'>❯</span> <span class='terminal-cursor'></span></p></div></div><div class='art-chip'><span class='local-dot'></span>{}</div></div></section><section class='metric-grid' aria-label='{}'><a class='metric-card' href='/installed'><span class='metric-icon'>▤</span><strong>{installed_display}</strong><span>{}</span><span class='metric-arrow'>↗</span></a><a class='metric-card' href='/favorites'><span class='metric-icon'>♡</span><strong>{favorites}</strong><span>{}</span><span class='metric-arrow'>↗</span></a><a class='metric-card' href='/categories'><span class='metric-icon'>▦</span><strong>{catalog}</strong><span>{}</span><span class='metric-arrow'>↗</span></a><a class='metric-card' href='/stats'><span class='metric-icon'>◷</span><strong>{}</strong><span>{}</span><span class='metric-arrow'>↗</span></a></section><div class='dashboard-grid'><section class='panel panel-featured'><div class='panel-heading'><div><span class='eyebrow'>{}</span><h2>{}</h2></div><a class='text-link' href='/stats'>{} ↗</a></div><div class='rank-list'>",
        tr(
            &lang,
            "DISCOVER · ORGANIZE · REMEMBER",
            "发现 · 整理 · 记住"
        ),
        tr(&lang, "Your tools,", "你的工具，"),
        tr(&lang, "all in one place.", "尽在掌握。"),
        tr(
            &lang,
            "A quiet home for every command-line tool you use and every great one you have yet to find.",
            "为常用的命令行工具安个家，也发现下一个趁手的好工具。"
        ),
        tr(&lang, "Search tools", "搜索工具"),
        tr(
            &lang,
            "Try “disk usage” or “磁盘空间”",
            "试试“磁盘空间”或“disk usage”"
        ),
        tr(&lang, "Explore", "开始探索"),
        tr(
            &lang,
            "Search by name, purpose, or category",
            "按名称、用途或分类搜索"
        ),
        tr(&lang, "Suggestions:", "推荐："),
        tr(&lang, "PRIVATE BY DESIGN", "数据只在本机"),
        tr(&lang, "Workspace at a glance", "工作空间概览"),
        if scanned {
            tr(&lang, "Installed tools", "已安装工具")
        } else {
            tr(&lang, "Not scanned yet", "尚未扫描")
        },
        tr(&lang, "Favorites", "我的收藏"),
        tr(&lang, "Catalog tools", "目录工具"),
        stats.total_runs,
        tr(&lang, "Runs · last 30 days", "近 30 天使用次数"),
        tr(&lang, "YOUR ACTIVITY", "你的使用情况"),
        tr(&lang, "Most used tools", "最常使用的工具"),
        tr(&lang, "View insights", "查看统计")
    );
    for (index, item) in stats.top_tools.iter().take(5).enumerate() {
        write!(
            body,
            "<a class='rank-row' href='/search?q={}'><span class='rank-index'>{:02}</span><span class='rank-name'>{}</span><span class='rank-track'><span style='width:{}%'></span></span><strong>{}</strong><span class='row-arrow'>↗</span></a>",
            esc(&item.name),
            index + 1,
            esc(&item.name),
            item.count * 100 / stats.top_tools.first().map(|x| x.count).unwrap_or(1).max(1),
            item.count,
        )
        .unwrap();
    }
    if stats.top_tools.is_empty() {
        write!(body, "<div class='empty-inline'><span class='empty-icon'>⌁</span><strong>{}</strong><p>{}</p><a href='/history'>{} ↗</a></div>",tr(&lang,"Your story starts here","你的使用记录从这里开始"),tr(&lang,"Enable shell history to see the tools you reach for most often.","启用 Shell 历史记录后，这里会展示你最常使用的工具。"),tr(&lang,"Learn about history","了解历史记录")).unwrap();
    }
    write!(
        body,
        "</div></section><section class='panel panel-recent'><div class='panel-heading'><div><span class='eyebrow'>{}</span><h2>{}</h2></div><a class='text-link' href='/history'>↗</a></div><div class='recent-list'>",
        tr(&lang, "PICK UP WHERE YOU LEFT OFF", "继续上次的探索"),
        tr(&lang, "Recently used", "最近使用")
    )
    .unwrap();
    for name in stats.recently_used.iter().take(5) {
        write!(body, "<a class='recent-row' href='/search?q={}'><span class='mini-avatar'>{}</span><span>{}</span><span class='row-arrow'>↗</span></a>", esc(name), esc(&name.chars().next().unwrap_or('›').to_string()), esc(name)).unwrap();
    }
    if stats.recently_used.is_empty() {
        write!(
            body,
            "<div class='empty-inline compact'><span class='empty-icon'>◷</span><p>{}</p></div>",
            tr(
                &lang,
                "No activity yet. Your recent tools will appear here.",
                "还没有使用记录，最近用过的工具会显示在这里。"
            )
        )
        .unwrap();
    }
    write!(
        body,
        "</div></section></div><section class='explore-section'><div class='section-heading'><div><span class='eyebrow'>{}</span><h2>{}</h2><p>{}</p></div><a class='text-link' href='/categories'>{} ↗</a></div><div class='category-grid'>",
        tr(&lang, "FIND YOUR NEXT FAVORITE", "发现下一个趁手工具"),
        tr(&lang, "Explore by category", "按分类探索"),
        tr(&lang, "Browse a curated collection of useful command-line tools.", "浏览精心整理的命令行工具目录。"),
        tr(&lang, "All categories", "全部分类")
    )
    .unwrap();
    let tools = app.core.all_tools().map_err(error)?;
    for (index, category) in app
        .core
        .categories()
        .map_err(error)?
        .into_iter()
        .take(6)
        .enumerate()
    {
        let count = tools
            .iter()
            .filter(|x| x.categories.contains(&category.id))
            .count();
        write!(
            body,
            "<a class='category-card' href='/search?q={}'><span class='category-symbol color-{}'>{}</span><span class='category-info'><strong>{}</strong><small>{} {}</small></span><span class='row-arrow'>↗</span></a>",
            esc(&category.id),
            index % 6,
            ["⌘", "▧", "◈", "◇", "▦", "⌁"][index % 6],
            esc(localized(&category.name, &lang)),
            count,
            tr(&lang, "tools", "个工具")
        )
        .unwrap();
    }
    body.push_str("</div></section>");
    page(&app, "Home", body)
}
#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}
async fn search(State(app): State<App>, Query(query): Query<SearchQuery>) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let q = query.q.unwrap_or_default();
    let q_trimmed = q.trim();
    let is_empty = q_trimmed.is_empty();

    let (items, count): (Vec<(cliary_core::Tool, bool)>, usize) = if is_empty {
        let all = app.core.all_tools().map_err(error)?;
        let count = all.len();
        let installed = app.core.installed().map_err(error)?;
        let inst_set: std::collections::HashSet<String> =
            installed.into_iter().filter_map(|x| x.tool_id).collect();
        let list = all
            .into_iter()
            .map(|t| {
                let inst = inst_set.contains(&t.id);
                (t, inst)
            })
            .collect();
        (list, count)
    } else {
        let results = app.core.search(q_trimmed, 100).map_err(error)?;
        let count = results.len();
        let list = results.into_iter().map(|r| (r.tool, r.installed)).collect();
        (list, count)
    };

    let mut body = format!(
        "<section class='page-intro'><span class='eyebrow'>{}</span><h1>{}</h1><p>{}</p></section><form class='search-form' action='/search'><span class='search-glyph'>⌕</span><input name='q' value='{}' placeholder='{}' aria-label='{}'><button>{} ↗</button></form><div class='chips' style='margin-top:-8px; margin-bottom:20px;'><span style='color:var(--muted); font-size:11px; align-self:center;'>{}</span><a class='chip' href='/search?q=disk'>💾 {}</a><a class='chip' href='/search?q=search'>🔍 {}</a><a class='chip' href='/search?q=git'>🐙 Git</a><a class='chip' href='/search?q=network'>🌐 {}</a><a class='chip' href='/search?q=text'>📝 {}</a></div><div class='result-meta'><span>{} <strong>{}</strong> {}</span><span>{}</span></div><div class='tool-list'>",
        tr(&lang, "CURATED TOOL CATALOG", "精心整理的工具目录"),
        tr(&lang, "Find the right tool.", "找到合适的工具。"),
        tr(
            &lang,
            "Search by name or describe what you want to do.",
            "输入名称，或描述你想完成的任务。"
        ),
        esc(&q),
        tr(
            &lang,
            "Try “disk usage” or “磁盘空间”",
            "试试“磁盘空间”或“disk usage”"
        ),
        tr(&lang, "Search tools", "搜索工具"),
        tr(&lang, "Search", "搜索"),
        tr(&lang, "Quick tags:", "热门搜索："),
        tr(&lang, "Disk", "磁盘分析"),
        tr(&lang, "Search", "代码搜索"),
        tr(&lang, "Network", "网络传输"),
        tr(&lang, "Text", "文本处理"),
        if is_empty {
            tr(&lang, "CATALOG CONTAINS", "工具库共收录")
        } else {
            tr(&lang, "SHOWING", "找到")
        },
        count,
        tr(&lang, "TOOLS", "个工具"),
        if is_empty {
            tr(&lang, "Showing all catalog tools", "展示全部精选工具")
        } else {
            tr(&lang, "Matching your search", "匹配你的检索结果")
        }
    );

    for (t, installed) in items {
        write!(
            body,
            "<a class='tool-card' href='/tools/{}'><span class='tool-avatar'>{}</span><span class='tool-copy'><strong>{}</strong><small>{}</small></span><span class='tool-card-end'>{}<span class='row-arrow'>↗</span></span></a>",
            esc(&t.id),
            esc(&t.name.chars().next().unwrap_or('›').to_string()),
            esc(&t.name),
            esc(localized(&t.description, &lang)),
            if installed {
                format!("<span class='status-pill installed-badge'><span class='local-dot'></span>{}</span>", tr(&lang, "Installed", "已安装"))
            } else {
                String::new()
            }
        )
        .unwrap();
    }

    if body.ends_with("<div class='tool-list'>") {
        write!(
            body,
            "<div class='empty-search'><span class='empty-icon'>⌕</span><h2>{}</h2><p>{}</p><a class='btn btn-primary' href='/search'>{}</a></div>",
            tr(&lang, "No matching tools found", "未找到匹配的工具"),
            tr(
                &lang,
                "Try another keyword, category, or clear search to browse all tools.",
                "试试更换关键词或分类，也可以清空搜索查看所有收录工具。"
            ),
            tr(&lang, "Browse all tools", "查看全部工具")
        )
        .unwrap();
    }
    body.push_str("</div>");
    page(&app, "Search", body)
}

async fn tool(State(app): State<App>, Path(id): Path<String>) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let Some(d) = app.core.tool_detail(&id).map_err(error)? else {
        return Err((StatusCode::NOT_FOUND, "tool not found".into()));
    };
    let t = &d.tool;
    let mut body = format!(
        "<nav class='breadcrumb-trail' aria-label='Breadcrumb'><a href='/'>{}</a><span class='breadcrumb-sep'>/</span><a href='/search'>{}</a><span class='breadcrumb-sep'>/</span><strong>{}</strong></nav><section class='detail-header'><div class='detail-heading'><span class='detail-avatar'>{}</span><div><span class='eyebrow'>{}</span><h1>{}</h1><p class='lead'>{}</p></div></div><div class='badges'>",
        tr(&lang, "Overview", "概览"),
        tr(&lang, "Discovery", "发现工具"),
        esc(&t.name),
        esc(&t.name.chars().next().unwrap_or('›').to_string()),
        tr(&lang, "TOOL PROFILE", "工具资料"),
        esc(&t.name),
        esc(localized(&t.description, &lang))
    );
    if d.installed.is_some() {
        write!(
            body,
            "<span class='status-pill installed-badge'><span class='local-dot'></span>{}</span>",
            tr(&lang, "Installed", "已就绪")
        )
        .unwrap()
    }
    let categories = app.core.categories().map_err(error)?;
    let tags = app.core.tags().map_err(error)?;
    for cat_id in &t.categories {
        let label = categories
            .iter()
            .find(|x| &x.id == cat_id)
            .map(|x| localized(&x.name, &lang))
            .unwrap_or(cat_id);
        write!(
            body,
            "<a class='tag-pill' href='/search?q={}'>🏷️ {}</a>",
            esc(cat_id),
            esc(label)
        )
        .unwrap();
    }
    for tag_id in &t.tags {
        let label = tags
            .iter()
            .find(|x| &x.id == tag_id)
            .map(|x| localized(&x.name, &lang))
            .unwrap_or(tag_id);
        write!(
            body,
            "<a class='tag-pill' href='/search?q={}'>#{}</a>",
            esc(tag_id),
            esc(label)
        )
        .unwrap();
    }
    body.push_str("</div></section>");
    if !app.core.has_scanned().map_err(error)? {
        write!(body,"<form class='scan-notice' method='post' action='/scan'><input type='hidden' name='csrf' value='{}'><span>{}</span><button>{}</button></form>",app.csrf,tr(&lang,"Installation status is unknown until you scan this machine.","尚未扫描本机，因此暂时无法判断安装状态。"),tr(&lang,"Scan now","立即扫描")).unwrap();
    }

    body.push_str("<div class='detail-meta-card'>");
    if let Some(installed) = &d.installed {
        write!(
            body,
            "<div class='detail-meta-item'><small>{}</small><strong>{}</strong></div><div class='detail-meta-item'><small>{}</small><strong>{}</strong></div>",
            tr(&lang, "Version", "版本"),
            esc(installed.version.as_deref().unwrap_or("—")),
            tr(&lang, "Source", "来源"),
            esc(installed.source.as_deref().unwrap_or("—"))
        )
        .unwrap();
    }
    body.push_str("<div class='detail-meta-links'>");
    if let Some(homepage) = &t.homepage {
        write!(
            body,
            "<a class='detail-meta-link' href='{}' rel='noreferrer' target='_blank'>🌐 {} ↗</a>",
            esc(homepage),
            tr(&lang, "Homepage", "主页")
        )
        .unwrap();
    }
    if let Some(repository) = &t.repository {
        write!(
            body,
            "<a class='detail-meta-link' href='{}' rel='noreferrer' target='_blank'>🐙 {} ↗</a>",
            esc(repository),
            tr(&lang, "Repository", "仓库")
        )
        .unwrap();
    }
    body.push_str("</div></div>");

    body.push_str("<div class='cards grid-4 detail-metrics'>");
    let metrics = [
        (
            tr(&lang, "Total Runs", "使用次数"),
            d.run_count.to_string(),
            "⚡",
            "card-icon",
        ),
        (
            tr(&lang, "Active Days", "活跃天数"),
            d.active_days.to_string(),
            "◷",
            "card-icon color-1",
        ),
        (
            tr(&lang, "First Used", "首次使用"),
            d.first_used.clone().unwrap_or_else(|| "—".into()),
            "🕒",
            "card-icon color-2",
        ),
        (
            tr(&lang, "Last Used", "最近使用"),
            d.last_used.clone().unwrap_or_else(|| "—".into()),
            "✦",
            "card-icon color-3",
        ),
    ];
    for (label, value, icon, icon_class) in metrics {
        write!(
            body,
            "<div><div class='card-top'><span class='{}'>{}</span></div><strong>{}</strong><span>{}</span></div>",
            icon_class,
            icon,
            esc(&value),
            label
        )
        .unwrap()
    }
    write!(
        body,
        "</div><div class='detail-layout'><div class='detail-main'><section class='content-panel'><span class='eyebrow'>{}</span><h2>{}</h2><div class='list command-list'>",
        tr(&lang, "GET STARTED", "开始使用"),
        tr(&lang, "Install", "安装方式")
    )
    .unwrap();
    for (manager, method) in &t.install {
        let cmd = method.command.as_deref().unwrap_or(&method.package);
        write!(
            body,
            "<div class='code-snippet'><div><span class='manager-label' style='color:var(--muted); font-size:11px; margin-right:8px; font-weight:700;'>{}</span><code>{}</code></div><button type='button' class='copy-btn' data-copy='{}'>{}</button></div>",
            esc(manager),
            esc(cmd),
            esc(cmd),
            tr(&lang, "Copy", "复制")
        )
        .unwrap();
    }
    if t.install.is_empty() {
        write!(
            body,
            "<p class='section-empty' style='color:var(--muted); font-size:13px; margin:0;'>{}</p>",
            tr(
                &lang,
                "No installation commands are listed yet.",
                "暂未收录安装命令。"
            )
        )
        .unwrap();
    }
    body.push_str("</div></section>");
    if !t.common_commands.is_empty() {
        write!(
            body,
            "<section class='content-panel'><span class='eyebrow'>{}</span><h2>{}</h2><div class='list command-list'>",
            tr(&lang, "QUICK REFERENCE", "快速参考"),
            tr(&lang, "Common commands", "常用命令")
        )
        .unwrap();
        for command in &t.common_commands {
            write!(
                body,
                "<div class='code-snippet'><div><span style='color:var(--green); margin-right:6px;'>$</span><code>{}</code></div><button type='button' class='copy-btn' data-copy='{}'>{}</button></div>",
                esc(command),
                esc(command),
                tr(&lang, "Copy", "复制")
            )
            .unwrap();
        }
        body.push_str("</div></section>");
    }
    if !d.similar.is_empty() {
        write!(
            body,
            "<section class='content-panel'><span class='eyebrow'>{}</span><h2>{}</h2><div class='similar-grid'>",
            tr(&lang, "KEEP EXPLORING", "继续探索"),
            tr(&lang, "Similar tools", "相似工具")
        )
        .unwrap();
        for other in &d.similar {
            write!(
                body,
                "<a class='similar-card' href='/tools/{}'><span class='mini-avatar'>{}</span><span>{}</span><span class='row-arrow'>↗</span></a>",
                esc(&other.id),
                esc(&other.name.chars().next().unwrap_or('›').to_string()),
                esc(&other.name)
            )
            .unwrap();
        }
        body.push_str("</div></section>");
    }
    write!(
        body,
        "</div><aside class='detail-side'><section class='content-panel'><span class='eyebrow'>{}</span><h2>{}</h2><p style='color:var(--muted); font-size:13px;'>{}</p>",
        tr(&lang, "PERSONAL SPACE", "个人空间"),
        tr(&lang, "My shelf", "我的工具架"),
        tr(&lang, "Keep this tool close and add a note for future you.", "收藏这个工具，给未来的自己留一条备注。")
    )
    .unwrap();
    write!(
        body,
        "<form method='post' action='/tools/{}/favorite' style='margin-bottom:14px;'><input type='hidden' name='csrf' value='{}'><input type='hidden' name='action' value='{}'><button type='submit' class='btn btn-block {}'>{}</button></form>",
        esc(&t.id),
        app.csrf,
        if d.favorite { "remove" } else { "add" },
        if d.favorite { "btn-danger" } else { "btn-outline" },
        if d.favorite { tr(&lang, "♥ Remove favorite", "♥ 取消收藏") } else { tr(&lang, "♡ Add to favorites", "♡ 收藏到书签") }
    ).unwrap();
    write!(
        body,
        "<form method='post' action='/tools/{}/note'><input type='hidden' name='csrf' value='{}'><textarea name='body' rows='4' placeholder='{}'>{}</textarea><button type='submit' class='btn btn-primary btn-block'>{}</button></form>",
        esc(&t.id),
        app.csrf,
        tr(&lang, "Add personal notes or tips...", "记录使用心得或常用参数..."),
        esc(d.note.as_deref().unwrap_or("")),
        tr(&lang, "Save note", "保存备注")
    ).unwrap();
    write!(
        body,
        "<div class='specs-list'><div class='specs-row'><span class='specs-label'>{}</span><span class='specs-val'>{}</span></div><div class='specs-row'><span class='specs-label'>{}</span><span class='specs-val'>{}</span></div><div class='specs-row'><span class='specs-label'>{}</span><span class='specs-val'>{}</span></div></div>",
        tr(&lang, "License", "许可证"),
        esc(t.license.as_deref().unwrap_or("—")),
        tr(&lang, "Platforms", "运行平台"),
        if t.platforms.is_empty() { "—".into() } else { esc(&t.platforms.join(", ")) },
        tr(&lang, "Executables", "可执行程序"),
        if t.executables.is_empty() { "—".into() } else { esc(&t.executables.join(", ")) }
    ).unwrap();
    body.push_str("</section></aside></div>");
    page(&app, &t.name, body)
}

#[derive(Deserialize)]
struct FavoriteForm {
    csrf: String,
    action: String,
}
async fn favorite(
    State(app): State<App>,
    Path(id): Path<String>,
    Form(form): Form<FavoriteForm>,
) -> Result<Redirect, (StatusCode, String)> {
    check(&app, &form.csrf)?;
    app.core
        .set_favorite(&id, form.action == "add")
        .map_err(error)?;
    Ok(Redirect::to(&format!("/tools/{}", esc(&id))))
}

#[derive(Deserialize)]
struct NoteForm {
    csrf: String,
    body: String,
}
async fn note(
    State(app): State<App>,
    Path(id): Path<String>,
    Form(form): Form<NoteForm>,
) -> Result<Redirect, (StatusCode, String)> {
    check(&app, &form.csrf)?;
    app.core.save_note(&id, &form.body).map_err(error)?;
    Ok(Redirect::to(&format!("/tools/{}", esc(&id))))
}

#[derive(Deserialize)]
struct CsrfForm {
    csrf: String,
}
async fn scan(
    State(app): State<App>,
    Form(form): Form<CsrfForm>,
) -> Result<Redirect, (StatusCode, String)> {
    check(&app, &form.csrf)?;
    app.core.scan_installed().map_err(error)?;
    Ok(Redirect::to("/installed"))
}

async fn categories(State(app): State<App>) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let tools = app.core.all_tools().map_err(error)?;
    let mut body = format!(
        "<section class='page-intro'><span class='eyebrow'>{}</span><h1>{}</h1><p>{}</p></section><div class='category-grid'>",
        tr(&lang, "CURATED DIRECTORY", "精心整理"),
        tr(&lang, "Tool Categories", "分类目录"),
        tr(
            &lang,
            "Explore command-line tools organized by purpose and workflow.",
            "按工作流与用途探索收录的优秀命令行工具。"
        )
    );
    for (index, cat) in app
        .core
        .categories()
        .map_err(error)?
        .into_iter()
        .enumerate()
    {
        let count = tools
            .iter()
            .filter(|x| x.categories.contains(&cat.id))
            .count();
        write!(
            body,
            "<a class='category-card' href='/search?q={}'><span class='category-symbol color-{}'>{}</span><span class='category-info'><strong>{}</strong><small>{} {}</small></span><span class='row-arrow'>↗</span></a>",
            esc(&cat.id),
            index % 6,
            ["⌘", "▧", "◈", "◇", "▦", "⌁"][index % 6],
            esc(localized(&cat.name, &lang)),
            count,
            tr(&lang, "tools", "个工具")
        )
        .unwrap();
    }
    body.push_str("</div>");
    page(&app, "Categories", body)
}

async fn favorites(State(app): State<App>) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let favorites = app.core.favorites().map_err(error)?;
    let mut body = format!(
        "<section class='page-intro'><span class='eyebrow'>{}</span><h1>{}</h1><p>{}</p></section>",
        tr(&lang, "PERSONAL BOOKMARKS", "个人书签"),
        tr(&lang, "My Favorites", "我的收藏"),
        tr(
            &lang,
            "Your handpicked collection of frequently reached-for tools.",
            "你最常随手调用的常用精选工具集合。"
        )
    );
    if favorites.is_empty() {
        write!(
            body,
            "<div class='empty-inline'><span class='empty-icon'>♡</span><strong>{}</strong><p>{}</p><a class='btn btn-primary' href='/search'>{} ↗</a></div>",
            tr(&lang, "No favorites added yet", "还没有添加任何收藏"),
            tr(&lang, "Explore the catalog or search for tools to bookmark your favorites.", "去工具库浏览或搜索你喜欢的工具并加入收藏吧。"),
            tr(&lang, "Discover tools", "发现工具")
        ).unwrap();
    } else {
        write!(
            body,
            "<div class='result-meta'><span>{} <strong>{}</strong> {}</span><span>{}</span></div><div class='tool-list'>",
            tr(&lang, "SAVED", "已收藏"),
            favorites.len(),
            tr(&lang, "TOOLS", "个工具"),
            tr(&lang, "Ready for quick access", "随时快捷调用")
        ).unwrap();
        for t in favorites {
            write!(
                body,
                "<a class='tool-card' href='/tools/{}'><span class='tool-avatar'>{}</span><span class='tool-copy'><strong>{}</strong><small>{}</small></span><span class='tool-card-end'><span class='row-arrow'>↗</span></span></a>",
                esc(&t.id),
                esc(&t.name.chars().next().unwrap_or('›').to_string()),
                esc(&t.name),
                esc(localized(&t.description, &lang))
            )
            .unwrap();
        }
        body.push_str("</div>");
    }
    page(&app, "Favorites", body)
}

#[derive(Deserialize)]
struct CompareQuery {
    tools: Option<String>,
}
async fn compare(State(app): State<App>, Query(query): Query<CompareQuery>) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let input = query.tools.unwrap_or_default();
    let names = input
        .split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut body = format!(
        "<section class='page-intro'><span class='eyebrow'>{}</span><h1>{}</h1><p>{}</p></section><form class='search-form' action='/compare'><span class='search-glyph'>⇄</span><input name='tools' value='{}' placeholder='ncdu, gdu, dust' aria-label='Tools to compare'><button>{} ↗</button></form><div class='chips' style='margin-top:-8px; margin-bottom:20px;'><span style='color:var(--muted); font-size:11px; align-self:center;'>{}</span><a class='chip' href='/compare?tools=ncdu,gdu'>ncdu vs gdu</a><a class='chip' href='/compare?tools=bat,cat'>bat vs cat</a><a class='chip' href='/compare?tools=ripgrep,grep'>ripgrep vs grep</a><a class='chip' href='/compare?tools=eza,ls'>eza vs ls</a><a class='chip' href='/compare?tools=fd,find'>fd vs find</a></div>",
        tr(&lang, "SIDE-BY-SIDE", "多维对比"),
        tr(&lang, "Compare Tools", "工具横向对比"),
        tr(
            &lang,
            "Evaluate commands side by side across features, platforms, and maintenance.",
            "横向对比多个工具在特性、语言、安装方式与维护状态上的异同。"
        ),
        esc(&input),
        tr(&lang, "Compare", "开始对比"),
        tr(&lang, "Popular presets:", "推荐对比：")
    );

    if names.len() < 2 {
        write!(
            body,
            "<div class='section-heading' style='margin-top:24px;'><div><span class='eyebrow'>{}</span><h2>{}</h2><p>{}</p></div></div><div class='compare-preset-card'><a class='compare-preset-item' href='/compare?tools=ncdu,gdu'><strong>ncdu vs gdu <span>↗</span></strong><span>{}</span></a><a class='compare-preset-item' href='/compare?tools=bat,cat'><strong>bat vs cat <span>↗</span></strong><span>{}</span></a><a class='compare-preset-item' href='/compare?tools=ripgrep,grep'><strong>ripgrep vs grep <span>↗</span></strong><span>{}</span></a><a class='compare-preset-item' href='/compare?tools=eza,ls'><strong>eza vs ls <span>↗</span></strong><span>{}</span></a><a class='compare-preset-item' href='/compare?tools=fd,find'><strong>fd vs find <span>↗</span></strong><span>{}</span></a></div>",
            tr(&lang, "EXPLORE DIFFERENCES", "探索差异"),
            tr(&lang, "Popular Comparisons", "常用对比组合"),
            tr(&lang, "Select a preset above or type 2 or more tool names to compare.", "点击上方推荐组合或在输入框中输入 2 个及以上工具名称进行比对。"),
            tr(&lang, "Disk usage: TUI vs speed", "磁盘分析：交互终端 vs 极速多线程"),
            tr(&lang, "File viewer: syntax highlighting vs standard", "文本查看：代码高亮 vs 系统原生"),
            tr(&lang, "Text search: blazing fast vs classic POSIX", "文本搜索：现代多线程 vs 传统工具"),
            tr(&lang, "File list: modern glyphs & git vs classic", "目录浏览：现代化带图标 vs 传统列表"),
            tr(&lang, "File find: intuitive syntax vs powerful posix", "查找文件：直观语法 vs 标准 find")
        ).unwrap();
    } else {
        let rows = app.core.compare(&names).map_err(error)?;
        if rows.len() < 2 {
            write!(
                body,
                "<div class='empty-inline'><span class='empty-icon'>⇄</span><strong>{}</strong><p>{}</p></div>",
                tr(&lang, "Tools not found in catalog", "未在工具库中找到足够的匹配工具"),
                tr(&lang, "Please check tool spelling or try one of the recommended presets above.", "请检查工具名称拼写，或点击上方的推荐对比组合。")
            ).unwrap();
        } else {
            body.push_str("<div class='compare-matrix-wrap'><table class='compare-table'><thead><tr><th class='compare-attr-th'>");
            body.push_str(tr(&lang, "DIMENSION", "对比维度"));
            body.push_str("</th>");
            for r in &rows {
                write!(
                    body,
                    "<th class='compare-tool-th'><div class='compare-tool-card'><span class='mini-avatar'>{}</span><a href='/tools/{}'><strong>{}</strong></a></div></th>",
                    esc(&r.name.chars().next().unwrap_or('›').to_string()),
                    esc(&r.id),
                    esc(&r.name)
                ).unwrap();
            }
            body.push_str("</tr></thead><tbody>");

            let dimensions: Vec<CompareDimension<'_>> = vec![
                (
                    "Status",
                    "运行状态",
                    Box::new(|r| {
                        if r.installed {
                            format!(
                                "<span class='status-pill installed-badge'><span class='local-dot'></span>{}</span>",
                                tr(&lang, "Installed", "已就绪")
                            )
                        } else {
                            format!(
                                "<span class='dim'>{}</span>",
                                tr(&lang, "Not installed", "未安装")
                            )
                        }
                    }),
                ),
                (
                    "Installed Version",
                    "已装版本",
                    Box::new(|r| {
                        r.version
                            .as_deref()
                            .map(|v| format!("<code>{}</code>", esc(v)))
                            .unwrap_or_else(|| "<span class='dim'>—</span>".into())
                    }),
                ),
                (
                    "Package Source",
                    "安装来源",
                    Box::new(|r| {
                        r.source
                            .as_deref()
                            .map(|s| format!("<span class='badge-source'>{}</span>", esc(s)))
                            .unwrap_or_else(|| "<span class='dim'>—</span>".into())
                    }),
                ),
                (
                    "Language",
                    "开发语言",
                    Box::new(|r| {
                        r.implementation_language
                            .as_deref()
                            .map(esc)
                            .unwrap_or_else(|| "<span class='dim'>—</span>".into())
                    }),
                ),
                (
                    "License",
                    "开源协议",
                    Box::new(|r| {
                        r.license
                            .as_deref()
                            .map(|l| format!("<span class='badge-subtle'>{}</span>", esc(l)))
                            .unwrap_or_else(|| "<span class='dim'>—</span>".into())
                    }),
                ),
                (
                    "Platforms",
                    "支持平台",
                    Box::new(|r| {
                        if r.platforms.is_empty() {
                            "<span class='dim'>—</span>".into()
                        } else {
                            esc(&r.platforms.join(", "))
                        }
                    }),
                ),
                (
                    "Maintenance",
                    "维护状态",
                    Box::new(|r| {
                        r.maintenance_status
                            .as_deref()
                            .map(esc)
                            .unwrap_or_else(|| "<span class='dim'>unknown</span>".into())
                    }),
                ),
                (
                    "Key Features",
                    "核心特性",
                    Box::new(|r| {
                        if r.features.is_empty() {
                            "<span class='dim'>—</span>".into()
                        } else {
                            r.features
                                .iter()
                                .map(|(k, v)| format!("<code>{}={}</code>", esc(k), v))
                                .collect::<Vec<_>>()
                                .join(" ")
                        }
                    }),
                ),
                (
                    "Install Methods",
                    "支持的包管理器",
                    Box::new(|r| {
                        if r.install.is_empty() {
                            "<span class='dim'>—</span>".into()
                        } else {
                            r.install
                                .iter()
                                .map(|(mgr, meth)| {
                                    format!("<code>{}:{}</code>", esc(mgr), esc(&meth.package))
                                })
                                .collect::<Vec<_>>()
                                .join(" ")
                        }
                    }),
                ),
                (
                    "Repository",
                    "代码仓库",
                    Box::new(|r| {
                        if let Some(repo) = &r.repository {
                            format!(
                                "<a class='detail-meta-link' href='{}' target='_blank' rel='noreferrer'>🐙 {} ↗</a>",
                                esc(repo),
                                tr(&lang, "Repository", "仓库链接")
                            )
                        } else {
                            "<span class='dim'>—</span>".into()
                        }
                    }),
                ),
            ];

            for (dim_en, dim_zh, extractor) in dimensions {
                write!(
                    body,
                    "<tr><th class='compare-attr-th'>{}</th>",
                    tr(&lang, dim_en, dim_zh)
                )
                .unwrap();
                for r in &rows {
                    write!(body, "<td class='compare-cell'>{}</td>", extractor(r)).unwrap();
                }
                body.push_str("</tr>");
            }

            body.push_str("</tbody></table></div>");
        }
    }
    page(&app, "Compare", body)
}

fn usage_empty_state(lang: &str, route: &str, last_used: Option<&str>) -> String {
    if let Some(last_used) = last_used {
        return format!(
            "<section class='panel usage-guide'><h2>{}</h2><p>{}</p><p>{}: <time>{}</time></p><a class='btn btn-outline' href='/history'>{}</a></section>",
            tr(lang, "No activity in the last 30 days", "近 30 天暂无活动"),
            tr(
                lang,
                "Earlier records are still available in Usage History. This view only includes the last 30 days.",
                "早期记录仍保留在使用历史中，此页面仅统计近 30 天。"
            ),
            tr(lang, "Last recorded activity", "最近一次记录"),
            esc(last_used),
            tr(lang, "View usage history", "查看使用历史")
        );
    }
    format!(
        "<section class='panel usage-guide'><h2>{}</h2><p>{}</p><ol class='usage-steps'><li><strong>{}</strong><p>{}</p><pre><code>cliary setup shell --enable</code></pre><details><summary>{}</summary><pre><code>cliary setup shell --shell bash --enable\ncliary setup shell --shell zsh --enable\ncliary setup shell --shell fish --enable</code></pre></details></li><li><strong>{}</strong><p>{}</p></li><li><strong>{}</strong><p>{}</p></li></ol><a class='btn btn-primary' href='{}'>{}</a><p class='usage-privacy'>{}</p><details class='usage-help'><summary>{}</summary><p>{}</p><p>{}</p><pre><code>cliary history\ncliary setup shell --disable</code></pre></details></section>",
        tr(lang, "Start your usage diary", "开始记录工具使用"),
        tr(
            lang,
            "No usage has been recorded in this workspace. Scanning finds installed tools; recording usage requires optional Shell integration.",
            "此工作区尚无使用记录。扫描用于发现已安装工具；使用历史需要另外启用可选的 Shell 集成。"
        ),
        tr(lang, "Enable recording in your terminal", "在终端启用采集"),
        tr(
            lang,
            "Run this command in the Shell you use. Bash, Zsh and Fish are supported.",
            "在你使用的 Shell 中执行以下命令，支持 Bash、Zsh 和 Fish。"
        ),
        tr(lang, "Choose a Shell explicitly", "手动指定 Shell"),
        tr(lang, "Open a new terminal", "打开一个新终端"),
        tr(
            lang,
            "The hook takes effect in new Shell sessions. Existing terminals need to reload their Shell configuration.",
            "采集脚本在新的 Shell 会话中生效；已有终端需要重新加载 Shell 配置。"
        ),
        tr(lang, "Use a tool, then refresh", "使用工具后刷新页面"),
        tr(
            lang,
            "Run an external command such as git --version, then wait for the next prompt. Recording happens in the background.",
            "运行外部命令，例如 git --version，等待下一个命令提示符后再刷新。记录会在后台写入。"
        ),
        esc(route),
        tr(lang, "Refresh records", "刷新记录"),
        tr(
            lang,
            "Only executable names, timestamps and a local machine ID are stored. Command arguments are never saved; existing Shell history is not imported.",
            "仅保存可执行文件名、时间和本机标识，不保存命令参数，也不导入已有 Shell 历史。"
        ),
        tr(
            lang,
            "Already enabled, but still empty?",
            "已启用，仍然没有记录？"
        ),
        tr(
            lang,
            "If cliary is not in PATH, replace it with your binary's path (for a local build: ./target/debug/cliary). Check that the terminal and Web UI use the same data directory, including any CLIARY_DATA_DIR or XDG_DATA_HOME override.",
            "若 cliary 不在 PATH 中，请替换为实际程序路径（本地构建可用 ./target/debug/cliary）。检查终端与 Web 页面使用相同的数据目录，包括 CLIARY_DATA_DIR 或 XDG_DATA_HOME 设置。"
        ),
        tr(
            lang,
            "Check records from the terminal. Bash history settings can skip some commands. To stop collecting new records, disable the hook and open a new terminal; saved records remain.",
            "可在终端检查记录；Bash 的历史设置可能跳过部分命令。若要停止采集新记录，禁用集成后打开新终端；已保存的记录会保留。"
        )
    )
}

async fn history(State(app): State<App>) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let summary = app.core.history(None).map_err(error)?;
    if summary.runs == 0 {
        return page(
            &app,
            "History",
            format!(
                "<section class='page-intro'><h1>{}</h1><p>{}</p></section>{}",
                tr(&lang, "Usage History", "使用历史"),
                tr(
                    &lang,
                    "See the tools you use and how your habits evolve.",
                    "了解常用工具，积累真实的使用轨迹。"
                ),
                usage_empty_state(&lang, "/history", None)
            ),
        );
    }
    let stats = app.core.stats(None, None).map_err(error)?;
    let mut body = format!(
        "<section class='page-intro'><span class='eyebrow'>{}</span><h1>{}</h1><p>{}</p></section><div class='cards grid-4'><div><div class='card-top'><span class='card-icon'>⚡</span></div><strong>{}</strong><span>{}</span></div><div><div class='card-top'><span class='card-icon color-1'>◷</span></div><strong>{}</strong><span>{}</span></div><div><div class='card-top'><span class='card-icon color-2'>⌘</span></div><strong>{}</strong><span>{}</span></div><div><div class='card-top'><span class='card-icon color-3'>🕒</span></div><strong>{}</strong><span>{}</span></div></div><div class='dashboard-grid'><section class='panel'><div class='panel-heading'><div><span class='eyebrow'>{}</span><h2>{}</h2></div></div><div class='rank-list'>",
        tr(&lang, "EXECUTION LOG", "执行记录"),
        tr(&lang, "Usage History", "使用历史"),
        tr(
            &lang,
            "Private telemetry recorded strictly on this machine.",
            "本地 Shell 记录的工具使用轨迹，100% 留存在本机。"
        ),
        summary.runs,
        tr(&lang, "Total Runs", "总运行次数"),
        summary.active_days,
        tr(&lang, "Active Days", "活跃天数"),
        stats.tools_used,
        tr(&lang, "Tools Used", "使用工具数"),
        summary.last_used.as_deref().unwrap_or("—"),
        tr(&lang, "Last Activity", "最近使用"),
        tr(&lang, "TOP TOOLS", "高频排行榜"),
        tr(&lang, "Most Used Tools", "最常用工具")
    );
    let max_run = stats.top_tools.first().map(|x| x.count).unwrap_or(1).max(1);
    for (index, item) in stats.top_tools.iter().take(8).enumerate() {
        write!(
            body,
            "<a class='rank-row' href='/search?q={}'><span class='rank-index'>{:02}</span><span class='rank-name'>{}</span><span class='rank-track'><span style='width:{}%'></span></span><strong>{}</strong><span class='row-arrow'>↗</span></a>",
            esc(&item.name),
            index + 1,
            esc(&item.name),
            item.count * 100 / max_run,
            item.count
        ).unwrap();
    }
    write!(
        body,
        "</div></section><section class='panel'><div class='panel-heading'><div><span class='eyebrow'>{}</span><h2>{}</h2></div></div><div class='recent-list'>",
        tr(&lang, "RECENT", "最近记录"),
        tr(&lang, "Recently Used", "最近使用工具")
    ).unwrap();
    for name in stats.recently_used.iter().take(8) {
        write!(
            body,
            "<a class='recent-row' href='/search?q={}'><span class='mini-avatar'>{}</span><span>{}</span><span class='row-arrow'>↗</span></a>",
            esc(name),
            esc(&name.chars().next().unwrap_or('›').to_string()),
            esc(name)
        ).unwrap();
    }
    if stats.recently_used.is_empty() {
        write!(
            body,
            "<div class='empty-inline compact'><span class='empty-icon'>◷</span><p>{}</p></div>",
            tr(&lang, "No recent tools", "暂无最近使用")
        )
        .unwrap();
    }
    write!(body, "</div></section></div>").unwrap();
    page(&app, "History", body)
}

async fn stats(State(app): State<App>) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let stats = app.core.stats(Some(30), None).map_err(error)?;
    if stats.total_runs == 0 {
        let summary = app.core.history(None).map_err(error)?;
        return page(
            &app,
            "Statistics",
            format!(
                "<section class='page-intro'><h1>{}</h1><p>{}</p></section>{}",
                tr(&lang, "Statistics · 30 days", "使用统计 · 近 30 天"),
                tr(
                    &lang,
                    "Trends based on the tools you actually use.",
                    "基于真实工具使用记录，了解活动趋势。"
                ),
                usage_empty_state(&lang, "/stats", summary.last_used.as_deref())
            ),
        );
    }
    let mut body = format!(
        "<section class='page-intro'><span class='eyebrow'>{}</span><h1>{}</h1><p>{}</p></section><div class='cards grid-3'><div><div class='card-top'><span class='card-icon'>⚡</span></div><strong>{}</strong><span>{}</span></div><div><div class='card-top'><span class='card-icon color-1'>◷</span></div><strong>{}</strong><span>{}</span></div><div><div class='card-top'><span class='card-icon color-2'>✦</span></div><strong>{}</strong><span>{}</span></div></div>",
        tr(&lang, "DEEP INSIGHTS", "深度洞察"),
        tr(&lang, "Statistics · 30 days", "使用统计 · 近 30 天"),
        tr(
            &lang,
            "Trends and frequency of your daily developer workflow.",
            "分析你的日常命令行工作流分布与习惯。"
        ),
        stats.total_runs,
        tr(&lang, "Total Runs", "总执行次数"),
        stats.active_days,
        tr(&lang, "Active Days", "活跃天数"),
        stats.new_tools,
        tr(&lang, "New Tools", "新探索工具")
    );

    // Daily activity
    write!(
        body,
        "<section class='panel'><div class='panel-heading'><div><span class='eyebrow'>{}</span><h2>{}</h2></div></div>",
        tr(&lang, "DAILY ACTIVITY", "每日活动分布"),
        tr(&lang, "Daily Activity · Last 30 Days", "每日活动分布 · 近 30 天")
    ).unwrap();
    if stats.daily_activity.is_empty() {
        write!(
            body,
            "<div class='empty-inline compact'><span class='empty-icon'>◷</span><p>{}</p></div>",
            tr(
                &lang,
                "No activity recorded in the last 30 days.",
                "近 30 天内暂无活动数据。"
            )
        )
        .unwrap();
    } else {
        let max = stats
            .daily_activity
            .iter()
            .map(|x| x.count)
            .max()
            .unwrap_or(1)
            .max(1);
        body.push_str("<div class='chart-container'>");
        for day in stats.daily_activity {
            write!(
                body,
                "<div class='chart-row'><span class='chart-label'>{}</span><span class='chart-track'><i style='width:{}%'></i></span><b class='chart-val'>{}</b></div>",
                esc(&day.name),
                day.count * 100 / max,
                day.count
            ).unwrap();
        }
        body.push_str("</div>");
    }
    body.push_str("</section>");

    // Monthly activity
    write!(
        body,
        "<section class='panel'><div class='panel-heading'><div><span class='eyebrow'>{}</span><h2>{}</h2></div></div>",
        tr(&lang, "MONTHLY TRENDS", "每月趋势"),
        tr(&lang, "Monthly Activity Breakdown", "每月活动分布")
    ).unwrap();
    if stats.monthly_activity.is_empty() {
        write!(
            body,
            "<div class='empty-inline compact'><span class='empty-icon'>▥</span><p>{}</p></div>",
            tr(
                &lang,
                "No monthly activity recorded yet.",
                "暂无每月活动数据。"
            )
        )
        .unwrap();
    } else {
        let month_max = stats
            .monthly_activity
            .iter()
            .map(|x| x.count)
            .max()
            .unwrap_or(1)
            .max(1);
        body.push_str("<div class='chart-container'>");
        for month in stats.monthly_activity {
            write!(
                body,
                "<div class='chart-row'><span class='chart-label'>{}</span><span class='chart-track'><i style='width:{}%'></i></span><b class='chart-val'>{}</b></div>",
                esc(&month.name),
                month.count * 100 / month_max,
                month.count
            ).unwrap();
        }
        body.push_str("</div>");
    }
    body.push_str("</section>");

    page(&app, "Statistics", body)
}
#[derive(Deserialize)]
struct LanguageForm {
    csrf: String,
    language: String,
}
async fn set_language(
    State(app): State<App>,
    Form(form): Form<LanguageForm>,
) -> Result<Redirect, (StatusCode, String)> {
    check(&app, &form.csrf)?;
    app.core.set_language(&form.language).map_err(error)?;
    Ok(Redirect::to("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cliary_core::Paths;

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

    #[tokio::test]
    async fn empty_usage_pages_explain_activation_in_both_languages() {
        for lang in ["en", "zh-CN"] {
            let (_root, app) = app(lang);
            for (route, html) in [
                ("/history", history(State(app.clone())).await.unwrap().0),
                ("/stats", stats(State(app.clone())).await.unwrap().0),
            ] {
                assert!(html.contains("cliary setup shell --enable"));
                for shell in ["bash", "zsh", "fish"] {
                    assert!(html.contains(&format!("--shell {shell} --enable")));
                }
                assert!(html.contains(&format!("href='{route}'")));
                assert!(html.contains("CLIARY_DATA_DIR"));
                assert!(html.contains("cliary setup shell --disable"));
                assert!(!html.contains("在终端中运行命令行工具即可自动记录"));
                assert!(!html.contains("Run tools in your terminal to record usage here."));
                assert!(html.contains(tr(
                    lang,
                    "Command arguments are never saved",
                    "不保存命令参数"
                )));
                assert!(html.contains(tr(lang, "Open a new terminal", "打开一个新终端")));
            }
            // Viewing guidance must not enable capture or create fabricated events.
            assert_eq!(app.core.history(None).unwrap().runs, 0);
            assert!(!app.core.paths.config_dir.join("shell").exists());
        }
    }

    #[tokio::test]
    async fn existing_usage_keeps_real_history_and_charts() {
        let (_root, app) = app("zh-CN");
        app.core.record_usage("my-test-tool").unwrap();
        let history = history(State(app.clone())).await.unwrap().0;
        let stats = stats(State(app)).await.unwrap().0;
        assert!(history.contains("my-test-tool"));
        assert!(stats.contains("chart-row"));
        assert!(!history.contains("usage-steps"));
        assert!(!stats.contains("usage-steps"));
    }

    #[tokio::test]
    async fn older_usage_is_not_mistaken_for_missing_capture() {
        for lang in ["en", "zh-CN"] {
            let (_root, app) = app(lang);
            app.core.record_usage("my-old-tool").unwrap();
            let db = rusqlite::Connection::open(&app.core.paths.user_db).unwrap();
            db.execute("UPDATE usage_events SET timestamp=946684800", [])
                .unwrap();
            let html = stats(State(app.clone())).await.unwrap().0;
            assert!(html.contains(tr(
                lang,
                "No activity in the last 30 days",
                "近 30 天暂无活动"
            )));
            assert!(html.contains("href='/history'"));
            let last_used = app.core.history(None).unwrap().last_used.unwrap();
            assert!(html.contains(&last_used));
            assert!(!html.contains("cliary setup shell --enable"));
            assert!(history(State(app)).await.unwrap().0.contains("my-old-tool"));
        }
    }
}
