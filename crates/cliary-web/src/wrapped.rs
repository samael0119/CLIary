use super::{App, WebResult, error, esc, page, tr};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use cliary_core::Wrapped;
use serde::Deserialize;
use std::{collections::HashSet, fmt::Write};

#[derive(Default, Deserialize)]
pub(super) struct WrappedQuery {
    year: Option<String>,
}

pub(super) async fn wrapped(
    State(app): State<App>,
    Query(query): Query<WrappedQuery>,
) -> Result<(StatusCode, Html<String>), (StatusCode, String)> {
    let lang = app.core.locale(None).map_err(error)?;
    let input = query.year.unwrap_or_default();
    let year = if input.trim().is_empty() {
        None
    } else {
        match input.trim().parse::<i32>() {
            Ok(year) if Wrapped::supports_year(year) => Some(year),
            _ => {
                let body = format!(
                    "{}<section class='panel wrapped-empty' role='alert'><h2>{}</h2><p>{}</p></section>",
                    heading(&lang, &input),
                    tr(&lang, "Invalid year", "年份无效"),
                    tr(
                        &lang,
                        "Enter a whole year between 1 and 9998, then view the report again.",
                        "请输入 1 至 9998 之间的整数年份，再查看报告。"
                    )
                );
                return page(&app, "Wrapped", body).map(|html| (StatusCode::BAD_REQUEST, html));
            }
        }
    };
    let report = app.core.wrapped(year).map_err(error)?;
    report_page(&app, &lang, &report).map(|html| (StatusCode::OK, html))
}

fn heading(lang: &str, input: &str) -> String {
    format!(
        "<section class='page-intro wrapped-intro'><div><h1>{}</h1><p>{}</p></div><form class='wrapped-year' action='/wrapped' method='get'><label for='wrapped-year'>{}</label><div><input id='wrapped-year' name='year' type='number' min='1' max='9998' required value='{}' aria-describedby='wrapped-year-help'><button class='btn btn-primary'>{}</button></div><small id='wrapped-year-help'>{}</small></form></section>",
        tr(lang, "CLIary Wrapped", "年度报告"),
        tr(
            lang,
            "A year of retained command records, on this device.",
            "回顾这台设备保留的年度命令记录。"
        ),
        tr(lang, "Year", "年份"),
        esc(input),
        tr(lang, "View report", "查看报告"),
        tr(
            lang,
            "Uses your local calendar year.",
            "按本地日历年份统计。"
        )
    )
}

fn report_page(app: &App, lang: &str, report: &Wrapped) -> WebResult {
    let known = app
        .core
        .all_tools()
        .map_err(error)?
        .into_iter()
        .map(|t| t.id)
        .collect();
    page(app, "Wrapped", render_report(lang, report, &known, false))
}

fn render_report(lang: &str, report: &Wrapped, known: &HashSet<String>, offline: bool) -> String {
    let mut body = if offline {
        format!("<h1>CLIary Wrapped · {}</h1>", report.year)
    } else {
        heading(lang, &report.year.to_string())
    };
    if !offline {
        write!(body,"<nav class='wrapped-export' aria-label='{}'><a class='btn btn-outline' href='/wrapped/export?year={}&amp;format=html' hx-boost='false' download>{}</a><a class='text-link' href='/wrapped/export?year={}&amp;format=json' hx-boost='false' download>{}</a></nav>",tr(lang,"Export report","导出报告"),report.year,tr(lang,"Save HTML report","保存 HTML 报告"),report.year,tr(lang,"Download JSON","下载 JSON")).unwrap();
    }
    write!(
        body,
        "<div class='wrapped-context'><h2>{} {}</h2><p>{}</p></div>",
        report.year,
        tr(lang, "in review", "年回顾"),
        if report.is_current_year {
            tr(
                lang,
                "This year is still in progress; totals may grow.",
                "本年度尚未结束，记录会继续累积。",
            )
        } else {
            tr(
                lang,
                "January 1 through December 31, local time.",
                "本地时间 1 月 1 日至 12 月 31 日。",
            )
        }
    )
    .unwrap();
    body.push_str("<dl class='wrapped-summary'>");
    for (label, value) in [
        (tr(lang, "Recorded entries", "记录条数"), report.total_runs),
        (tr(lang, "Active days", "活跃天数"), report.active_days),
        (tr(lang, "Tools used", "使用工具数"), report.tools_used),
        (
            tr(lang, "First recorded this year", "本年首次记录"),
            report.insights.new_tools_count,
        ),
    ] {
        write!(body, "<div><dt>{label}</dt><dd>{value}</dd></div>").unwrap();
    }
    body.push_str("</dl>");
    write!(body, "<p class='wrapped-scope'>{}</p>", tr(lang,
        "Includes dated capture and imported history entries. History may omit or merge executions; gaps do not mean inactivity. No command arguments are saved.",
        "统计有日期的采集与导入记录。历史可能遗漏或合并调用，空缺不代表没有使用；不保存命令参数。")).unwrap();
    if report.total_runs == 0 {
        write!(body, "<section class='panel wrapped-empty'><h2>{}</h2><p>{}</p><p><code>cliary setup shell --enable</code></p><p>{}</p>{}</section>",
            tr(lang, "No records for this year", "该年暂无使用记录"),
            tr(lang, "Try another year if you have older records. To capture future commands, run:", "如已有其他年份的记录，可切换年份。要采集之后的命令，请运行："),
            tr(lang, "Open a new terminal to capture future commands. Existing history can be imported explicitly.", "打开新终端采集之后的命令；旧历史可另行显式导入。"),
            if offline {String::new()} else {format!("<a class='text-link' href='/history'>{}</a>",tr(lang,"View usage history","查看使用历史"))}).unwrap();
    } else {
        write!(
            body,
            "<p class='wrapped-range'>{}: <time>{}</time> / <time>{}</time></p>",
            tr(lang, "First / last recorded", "首条 / 末条记录"),
            esc(report.first_recorded.as_deref().unwrap_or("—")),
            esc(report.last_recorded.as_deref().unwrap_or("—"))
        )
        .unwrap();
    }
    body.push_str("<div class='wrapped-columns'>");
    if !report.top_tools.is_empty() {
        write!(body, "<section class='panel'><h2>{}</h2><p class='wrapped-caption'>{}</p><ol class='wrapped-ranking'>",
            tr(lang, "Top tools", "常用工具"), tr(lang, "Up to 10 tools, ranked by recorded entries. Unmatched tools are included.", "按记录条数排序，最多展示 10 项，包含未识别的工具。")).unwrap();
        for item in &report.top_tools {
            write!(
                body,
                "<li><code>{}</code><span>{} {}</span></li>",
                tool_label(&item.name, known, offline),
                item.count,
                tr(
                    lang,
                    if item.count == 1 { "entry" } else { "entries" },
                    "条"
                )
            )
            .unwrap();
        }
        body.push_str("</ol></section>");
    }
    write!(body, "<section class='panel wrapped-months'><h2>{}</h2><p class='wrapped-caption'>{}</p><table><thead><tr><th scope='col'>{}</th><th scope='col'>{}</th></tr></thead><tbody>",
        tr(lang, "Month by month", "月度趋势"), tr(lang, "Zero means no dated records for that month.", "数值为 0 表示该月没有日期明确的记录。"),
        tr(lang, "Month", "月份"), tr(lang, "Recorded entries", "记录条数")).unwrap();
    let maximum = report
        .monthly_activity
        .iter()
        .map(|m| m.count)
        .max()
        .unwrap_or(0)
        .max(1);
    for month in &report.monthly_activity {
        let fraction = month.count as f64 / maximum as f64;
        write!(body, "<tr><th scope='row'>{}</th><td><span class='wrapped-track' aria-hidden='true'><i style='transform:scaleX({fraction:.6})'></i></span><span class='wrapped-count'>{}</span></td></tr>", esc(&month.name), month.count).unwrap();
    }
    body.push_str("</tbody></table></section></div>");
    render_insights(&mut body, lang, report, known, offline);
    body
}

fn tool_label(name: &str, known: &HashSet<String>, offline: bool) -> String {
    let text = esc(name);
    if offline || !known.contains(name) {
        return text;
    }
    let encoded = serde_urlencoded::to_string([("id", name)]).unwrap();
    format!(
        "<a href='/tools/{}'>{text}</a>",
        esc(encoded
            .strip_prefix("id=")
            .unwrap()
            .replace('+', "%20")
            .as_str())
    )
}
fn render_insights(
    body: &mut String,
    lang: &str,
    report: &Wrapped,
    known: &HashSet<String>,
    offline: bool,
) {
    let i = &report.insights;
    body.push_str("<div class='wrapped-insights'>");
    write!(body,"<section class='wrapped-section'><h2>{}</h2><p class='wrapped-caption'>{}</p>",tr(lang,"Tools entering your records","新进入记录的工具"),tr(lang,"First observed this year in retained dated records; this is not an installation date. Up to ten, ranked by entries.","首次出现在本年度保留的有日期记录中，不代表安装日期。按记录数展示最多十项。")).unwrap();
    if let Some(item) = &i.breakout_tool {
        write!(
            body,
            "<p class='wrapped-breakout'>{}: <strong><code>{}</code></strong> · {} {}</p>",
            tr(lang, "Top newly recorded tool", "年度新宠"),
            tool_label(&item.name, known, offline),
            item.runs,
            tr(lang, "entries", "条记录")
        )
        .unwrap();
    }
    if i.new_tools.is_empty() {
        write!(
            body,
            "<p>{}</p>",
            tr(
                lang,
                "No newly recorded tools in this year.",
                "该年没有首次记录的新工具。"
            )
        )
        .unwrap();
    } else {
        write!(body,"<table class='wrapped-detail-table'><thead><tr><th scope='col'>{}</th><th scope='col'>{}</th><th scope='col'>{}</th></tr></thead><tbody>",tr(lang,"Tool","工具"),tr(lang,"First recorded","首次记录"),tr(lang,"Entries","记录数")).unwrap();
        for item in &i.new_tools {
            write!(
                body,
                "<tr><th scope='row'><code>{}</code></th><td><time>{}</time></td><td>{}</td></tr>",
                tool_label(&item.name, known, offline),
                esc(&item.first_recorded),
                item.runs
            )
            .unwrap();
        }
        body.push_str("</tbody></table>");
    }
    body.push_str("</section>");
    write!(body,"<section class='wrapped-section'><h2>{}</h2><p class='wrapped-caption'>{}</p>",tr(lang,"Where your tools belong","常用工具分类"),tr(lang,"Uses the current Catalog. Categories can overlap, so their sum may exceed the entry count.","采用当前工具库分类；工具可归属多个类别，因此分类总数可能超过记录数。")).unwrap();
    if i.top_categories.is_empty() {
        write!(
            body,
            "<p>{}</p>",
            tr(lang, "No dated category records.", "暂无有日期的分类记录。")
        )
        .unwrap();
    } else {
        body.push_str("<dl class='wrapped-source-list'>");
        for item in &i.top_categories {
            write!(
                body,
                "<div><dt>{}</dt><dd>{}</dd></div>",
                esc(if item.name == "uncategorized" {
                    tr(lang, "Uncategorized", "未分类")
                } else {
                    i.category_labels
                        .get(&item.name)
                        .map(|names| cliary_core::localized(names, lang))
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&item.name)
                }),
                item.count
            )
            .unwrap();
        }
        body.push_str("</dl>");
    }
    body.push_str("</section></div>");
    if let Some(c) = &i.comparison {
        write!(body,"<section class='wrapped-section'><h2>{}</h2><p class='wrapped-caption'>{}</p><p class='wrapped-comparison-total'>{}: <strong>{}</strong> · {}: <strong>{}</strong> <span>{}</span></p><p class='wrapped-caption'><time>{}</time> / <time>{}</time></p>",tr(lang,if c.year_to_date {"Compared with last year's same period"} else {"Compared with the previous year"},if c.year_to_date {"与上年同期相比"} else {"与上一年度相比"}),tr(lang,"Changes describe retained records. Unequal collection coverage can explain a change; it does not establish tool replacement.","变化描述保留记录。采集覆盖差异也会影响数字，不据此判断工具替代关系。"),c.previous_year,c.previous_runs,report.year,c.current_runs,esc(&c.change_percent.map(|v|format!("{v:+.1}%")).unwrap_or_else(||tr(lang,"No previous-year baseline; percentage unavailable","上年没有基数，无法计算比例").into())),esc(&c.previous_end),esc(&c.current_end)).unwrap();
        if i.tool_changes.is_empty() {
            write!(
                body,
                "<p>{}</p>",
                tr(
                    lang,
                    "No recorded differences for this comparison window.",
                    "该对比时段没有记录差异。"
                )
            )
            .unwrap();
        } else {
            write!(body,"<table class='wrapped-detail-table'><thead><tr><th scope='col'>{}</th><th scope='col'>{}</th><th scope='col'>{}</th><th scope='col'>{}</th></tr></thead><tbody>",tr(lang,"Tool","工具"),c.previous_year,report.year,tr(lang,"Entry difference","记录差值")).unwrap();
            for item in &i.tool_changes {
                let delta = if item.current_runs >= item.previous_runs {
                    format!("+{}", item.current_runs - item.previous_runs)
                } else {
                    format!("−{}", item.previous_runs - item.current_runs)
                };
                write!(body,"<tr><th scope='row'><code>{}</code></th><td>{}</td><td>{}</td><td>{delta}</td></tr>",tool_label(&item.name,known,offline),item.previous_runs,item.current_runs).unwrap();
            }
            body.push_str("</tbody></table>");
        }
        body.push_str("</section>");
    }
    write!(body,"<section class='wrapped-section'><h2>{}</h2><p class='wrapped-caption'>{}</p>",tr(lang,"Favorites with few records","记录较少的收藏"),tr(lang,"Currently saved favorites held at least 90 days by the report cutoff, with at most two dated entries after saving in this year. No records is not proof of no usage.","截至报告时段末已收藏至少90天，且本年度收藏后的有日期记录不超过2条。没有记录不代表没有使用；仅包含当前仍收藏的工具。")).unwrap();
    if i.low_activity_favorites.is_empty() {
        write!(
            body,
            "<p>{}</p>",
            tr(
                lang,
                "No favorites meet this observation rule.",
                "没有符合此观察条件的收藏。"
            )
        )
        .unwrap();
    } else {
        write!(body,"<table class='wrapped-detail-table'><thead><tr><th scope='col'>{}</th><th scope='col'>{}</th><th scope='col'>{}</th></tr></thead><tbody>",tr(lang,"Tool","工具"),tr(lang,"Saved on","收藏时间"),tr(lang,"Entries after saving","收藏后记录")).unwrap();
        for item in &i.low_activity_favorites {
            write!(
                body,
                "<tr><th scope='row'><code>{}</code></th><td><time>{}</time></td><td>{}</td></tr>",
                tool_label(&item.name, known, offline),
                esc(&item.favorited_at),
                item.runs
            )
            .unwrap();
        }
        body.push_str("</tbody></table>");
    }
    body.push_str("</section>");
    write!(body,"<section class='wrapped-section wrapped-provenance'><h2>{}</h2><p class='wrapped-caption'>{}</p><dl class='wrapped-source-list'>",tr(lang,"What this report includes","报告的数据范围"),tr(lang,"Sources below account for this year's dated entries. Shell history settings can omit or deduplicate commands; imported entries are observations, not a complete execution log.","以下来源对应该年度有日期的记录。Shell 历史设置可能遗漏或合并命令，导入条目是观察记录，不是完整执行日志。")).unwrap();
    for item in &i.source_counts {
        write!(
            body,
            "<div><dt>{}</dt><dd>{}</dd></div>",
            esc(if item.name == "capture" {
                tr(lang, "Live Shell capture", "Shell 实时采集")
            } else {
                &item.name
            }),
            item.count
        )
        .unwrap();
    }
    write!(
        body,
        "</dl><p>{}: <strong>{}</strong> — {}</p>",
        tr(lang, "Undated tools in this workspace", "工作区无日期工具"),
        i.undated_tools,
        tr(
            lang,
            "excluded from all years and time-based statistics.",
            "未归入任何年份，也不进入时间统计。"
        )
    )
    .unwrap();
    if !offline {
        write!(body,"<details class='wrapped-import-help'><summary>{}</summary><p>{}</p><pre><code>cliary import-history --shell zsh --file \"$HISTFILE\"\ncliary import-history --shell zsh --file \"$HISTFILE\" --apply\ncliary history --undated</code></pre><p>{}</p></details>",tr(lang,"Bring in existing Shell history","导入已有 Shell 历史"),tr(lang,"Use your actual history file. Preview first, add --apply to import. Bash and Fish are supported with --shell bash / fish.","使用实际历史文件，先预览，添加 --apply 才导入。也支持 --shell bash / fish。"),tr(lang,"No arguments saved; complex entries are skipped. Repeated import does not accumulate duplicates.","不保存参数，复杂条目会跳过；重复导入不会重复累加。")).unwrap();
    }
    body.push_str("</section>");
}

/// Standalone, offline, script-free report; data is shared with the live page and JSON.
pub(crate) fn standalone(report: &Wrapped, lang: &str) -> String {
    let body = render_report(lang, report, &HashSet::new(), true);
    format!(
        "<!doctype html><html lang='{}'><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><meta http-equiv='Content-Security-Policy' content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\"><title>CLIary Wrapped · {}</title><style>{}\nbody{{margin:0;background:var(--bg);color:var(--ink)}}main{{max-width:1120px;margin:0 auto;padding:40px}}@media print{{main{{padding:0}}.panel{{break-inside:avoid}}}}</style></head><body><main>{body}</main></body></html>",
        esc(lang),
        report.year,
        include_str!("../static/style.css")
    )
}
#[derive(Default, Deserialize)]
pub(super) struct ExportQuery {
    year: Option<String>,
    format: Option<String>,
}
pub(super) async fn export(
    State(app): State<App>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, (StatusCode, String)> {
    let lang = app.core.locale(None).map_err(error)?;
    let year = match query.year.as_deref() {
        None | Some("") => None,
        Some(s) => Some(
            s.parse::<i32>()
                .ok()
                .filter(|y| Wrapped::supports_year(*y))
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        tr(
                            &lang,
                            "Invalid year; use 1–9998.",
                            "年份无效，请使用1至9998。",
                        )
                        .into(),
                    )
                })?,
        ),
    };
    let format = query.format.as_deref().unwrap_or("html");
    let report = app.core.wrapped(year).map_err(error)?;
    let (mime, body) = match format {
        "html" => ("text/html; charset=utf-8", standalone(&report, &lang)),
        "json" => (
            "application/json",
            serde_json::to_string_pretty(&report).map_err(|e| error(e.into()))?,
        ),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                tr(
                    &lang,
                    "Choose html or json export.",
                    "请选择html或json格式。",
                )
                .into(),
            ));
        }
    };
    Ok((
        [
            ("content-type", mime.to_string()),
            (
                "content-disposition",
                format!(
                    "attachment; filename=\"cliary-wrapped-{}.{format}\"",
                    report.year
                ),
            ),
            ("cache-control", "no-store".into()),
            ("x-content-type-options", "nosniff".into()),
        ],
        body,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cliary_core::{Cliary, CountItem, Paths};
    use std::sync::Arc;

    fn app() -> (tempfile::TempDir, App) {
        let root = tempfile::TempDir::new().unwrap();
        let core = Cliary::at(Paths::new(
            root.path().join("config"),
            root.path().join("data"),
            root.path().join("cache"),
        ))
        .unwrap();
        (
            root,
            App {
                core: Arc::new(core),
                csrf: "test".into(),
            },
        )
    }

    #[tokio::test]
    async fn default_and_empty_year_reports_have_twelve_months_and_recovery() {
        let (_root, app) = app();
        let (status, Html(body)) = wrapped(State(app.clone()), Query(WrappedQuery::default()))
            .await
            .unwrap();
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("This year is still in progress"));
        assert!(body.contains("No records for this year"));
        assert!(body.contains("cliary setup shell --enable"));
        assert_eq!(body.matches("scope='row'").count(), 12);
        app.core.set_language("zh-CN").unwrap();
        let (_, Html(body)) = wrapped(
            State(app),
            Query(WrappedQuery {
                year: Some("2024".into()),
            }),
        )
        .await
        .unwrap();
        assert!(body.contains("该年暂无使用记录"));
        assert!(body.contains("2024-12"));
        assert!(!body.contains("本年度尚未结束"));
    }

    #[tokio::test]
    async fn invalid_years_return_recoverable_bad_requests_and_escape_input() {
        let (_root, app) = app();
        for year in [
            "0",
            "-1",
            "9999",
            "2147483648",
            "2024.5",
            "<script>alert(1)</script>",
        ] {
            let (status, Html(body)) = wrapped(
                State(app.clone()),
                Query(WrappedQuery {
                    year: Some(year.into()),
                }),
            )
            .await
            .unwrap();
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(body.contains("Invalid year"));
            assert!(body.contains("action='/wrapped'"));
            assert!(!body.contains("<script>alert(1)</script>"));
        }
    }

    #[test]
    fn populated_report_is_bilingual_and_escapes_persisted_labels() {
        let report = Wrapped {
            year: 2024,
            is_current_year: false,
            total_runs: 7,
            active_days: 2,
            tools_used: 1,
            first_recorded: Some("2024-02-29 00:00:00".into()),
            last_recorded: Some("2024-12-31 23:59:59".into()),
            top_tools: vec![CountItem {
                name: "<img src=x onerror=alert(1)>".into(),
                count: 7,
            }],
            monthly_activity: (1..=12)
                .map(|m| CountItem {
                    name: format!("2024-{m:02}"),
                    count: if m == 2 { 7 } else { 0 },
                })
                .collect(),
            ..Wrapped::default()
        };
        for lang in ["en", "zh-CN"] {
            let body = render_report(lang, &report, &HashSet::new(), false);
            assert!(body.contains("&lt;img src=x onerror=alert(1)&gt;"));
            assert!(!body.contains("<img"));
            assert!(body.contains("2024-02-29 00:00:00"));
            assert!(body.contains("scaleX(1.000000)"));
            assert!(body.contains("scaleX(0.000000)"));
            assert!(!body.contains("No records for this year"));
        }
    }
}

#[cfg(test)]
#[path = "wrapped_export_tests.rs"]
mod export_tests;
