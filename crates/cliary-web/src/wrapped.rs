use super::{App, WebResult, error, esc, page, tr};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Html,
};
use cliary_core::Wrapped;
use serde::Deserialize;
use std::fmt::Write;

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
            "A year of captured commands, on this device.",
            "回顾这台设备的年度命令调用记录。"
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
    page(app, "Wrapped", render_report(lang, report))
}

fn render_report(lang: &str, report: &Wrapped) -> String {
    let mut body = heading(lang, &report.year.to_string());
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
        (tr(lang, "Captured runs", "已记录调用"), report.total_runs),
        (tr(lang, "Active days", "活跃天数"), report.active_days),
        (tr(lang, "Tools used", "使用工具数"), report.tools_used),
    ] {
        write!(body, "<div><dt>{label}</dt><dd>{value}</dd></div>").unwrap();
    }
    body.push_str("</dl>");
    write!(body, "<p class='wrapped-scope'>{}</p>", tr(lang,
        "Only captured invocations are counted. Missing records do not mean inactivity; collection gaps cannot be reconstructed. Arguments are never stored.",
        "仅统计已采集的调用。记录空缺不代表没有使用，未采集的时段无法补全；不会保存命令参数。")).unwrap();
    if report.total_runs == 0 {
        write!(body, "<section class='panel wrapped-empty'><h2>{}</h2><p>{}</p><p><code>cliary setup shell --enable</code></p><p>{}</p><a class='text-link' href='/history'>{}</a></section>",
            tr(lang, "No records for this year", "该年暂无使用记录"),
            tr(lang, "Try another year if you have older records. To capture future commands, run:", "如已有其他年份的记录，可切换年份。要采集之后的命令，请运行："),
            tr(lang, "Then open a new terminal. Earlier commands are not imported.", "然后打开新终端；不会导入之前的命令。"),
            tr(lang, "View usage history", "查看使用历史")).unwrap();
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
            tr(lang, "Top tools", "常用工具"), tr(lang, "Up to 10 tools, ranked by captured runs. Unknown tools are included.", "按已记录调用排序，最多展示 10 项，包含未识别的工具。")).unwrap();
        for item in &report.top_tools {
            write!(
                body,
                "<li><code>{}</code><span>{} {}</span></li>",
                esc(&item.name),
                item.count,
                tr(lang, if item.count == 1 { "run" } else { "runs" }, "次")
            )
            .unwrap();
        }
        body.push_str("</ol></section>");
    }
    write!(body, "<section class='panel wrapped-months'><h2>{}</h2><p class='wrapped-caption'>{}</p><table><thead><tr><th scope='col'>{}</th><th scope='col'>{}</th></tr></thead><tbody>",
        tr(lang, "Month by month", "月度趋势"), tr(lang, "Zero means no captured records for that month.", "数值为 0 表示该月没有采集记录。"),
        tr(lang, "Month", "月份"), tr(lang, "Captured runs", "已记录调用")).unwrap();
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
    body
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
            let body = render_report(lang, &report);
            assert!(body.contains("&lt;img src=x onerror=alert(1)&gt;"));
            assert!(!body.contains("<img"));
            assert!(body.contains("2024-02-29 00:00:00"));
            assert!(body.contains("scaleX(1.000000)"));
            assert!(body.contains("scaleX(0.000000)"));
            assert!(!body.contains("No records for this year"));
        }
    }
}
