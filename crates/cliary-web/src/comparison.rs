use super::{App, WebResult, error, esc, page, tr};
use axum::extract::{Query, State};
use cliary_core::CompareRow;
use serde::Deserialize;
use std::fmt::Write;

type CompareDimension<'a> = (
    &'static str,
    &'static str,
    Box<dyn Fn(&CompareRow) -> String + 'a>,
);

fn recorded(value: &str, lang: &str) -> String {
    if value.is_empty() {
        format!(
            "<span class='dim'>{}</span>",
            tr(lang, "Not recorded", "未记录")
        )
    } else {
        esc(value)
    }
}

fn row(body: &mut String, label: &str, rows: &[CompareRow], cell: impl Fn(&CompareRow) -> String) {
    write!(
        body,
        "<tr><th scope='row' class='compare-attr-th'>{}</th>",
        esc(label)
    )
    .unwrap();
    for r in rows {
        write!(body, "<td class='compare-cell'>{}</td>", cell(r)).unwrap();
    }
    body.push_str("</tr>");
}

#[derive(Deserialize)]
pub(super) struct CompareQuery {
    tools: Option<String>,
}
pub(super) async fn compare(
    State(app): State<App>,
    Query(query): Query<CompareQuery>,
) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    let input = query.tools.unwrap_or_default();
    let names = input
        .split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut body = format!(
        "<section class='page-intro'><h1>{}</h1><p>{}</p></section><form class='search-form' action='/compare'><input name='tools' value='{}' placeholder='ncdu, gdu, dust' aria-label='{}' aria-describedby='compare-help'><button>{} ↗</button></form><div class='chips' style='margin-top:-8px; margin-bottom:20px;'><span style='color:var(--muted); font-size:11px; align-self:center;'>{}</span><a class='chip' href='/compare?tools=ncdu,gdu'>ncdu vs gdu</a><a class='chip' href='/compare?tools=bat,cat'>bat vs cat</a><a class='chip' href='/compare?tools=ripgrep,grep'>ripgrep vs grep</a><a class='chip' href='/compare?tools=eza,ls'>eza vs ls</a><a class='chip' href='/compare?tools=fd,find'>fd vs find</a></div>",
        tr(&lang, "Compare Tools", "工具横向对比"),
        tr(
            &lang,
            "Evaluate commands side by side across features, platforms, and maintenance.",
            "横向对比多个工具在特性、语言、安装方式与维护状态上的异同。"
        ),
        esc(&input),
        tr(&lang, "Tools to compare", "要比较的工具"),
        tr(&lang, "Compare", "开始对比"),
        tr(&lang, "Popular presets:", "推荐对比：")
    );

    write!(
        body,
        "<p id='compare-help' class='compare-help'>{}</p>",
        tr(
            &lang,
            "Enter 2–8 distinct tool names, separated by commas. Aliases are supported.",
            "输入 2–8 个不同工具名称，用逗号分隔；支持工具别名。"
        )
    )
    .unwrap();
    let mut ids = std::collections::HashSet::new();
    let mut unknown = Vec::new();
    if (2..=8).contains(&names.len()) {
        for name in &names {
            if let Some(detail) = app.core.tool_detail(name).map_err(error)? {
                ids.insert(detail.tool.id);
            } else {
                unknown.push(name.as_str());
            }
        }
    }
    let invalid = if !unknown.is_empty() {
        Some(format!(
            "{}: {}",
            tr(&lang, "Not found in Catalog", "工具库中未找到"),
            unknown.join(", ")
        ))
    } else if !names.is_empty() && (!(2..=8).contains(&names.len()) || ids.len() < 2) {
        Some(
            tr(
                &lang,
                "Compare requires 2–8 distinct tools.",
                "请提供 2–8 个不同的工具。",
            )
            .to_string(),
        )
    } else {
        None
    };
    if let Some(message) = invalid {
        write!(body, "<section class='empty-inline' role='alert'><strong>{}</strong><p>{}</p><a href='/search'>{}</a></section>", esc(&message),
            tr(&lang, "Check the names above or choose a preset. Search the Catalog for valid names.", "请修改上方名称或选择推荐组合；也可以搜索工具库确认名称。"),
            tr(&lang, "Search tools", "搜索工具")).unwrap();
    } else if names.is_empty() {
        write!(
            body,
            "<div class='section-heading' style='margin-top:24px;'><div><h2>{}</h2><p>{}</p></div></div><div class='compare-preset-card'><a class='compare-preset-item' href='/compare?tools=ncdu,gdu'><strong>ncdu vs gdu <span>↗</span></strong><span>{}</span></a><a class='compare-preset-item' href='/compare?tools=bat,cat'><strong>bat vs cat <span>↗</span></strong><span>{}</span></a><a class='compare-preset-item' href='/compare?tools=ripgrep,grep'><strong>ripgrep vs grep <span>↗</span></strong><span>{}</span></a><a class='compare-preset-item' href='/compare?tools=eza,ls'><strong>eza vs ls <span>↗</span></strong><span>{}</span></a><a class='compare-preset-item' href='/compare?tools=fd,find'><strong>fd vs find <span>↗</span></strong><span>{}</span></a></div>",
            tr(&lang, "Popular Comparisons", "常用对比组合"),
            tr(&lang, "Select a preset above or type 2 or more tool names to compare.", "点击上方推荐组合或在输入框中输入 2 个及以上工具名称进行比对。"),
            tr(&lang, "Disk usage analyzers", "磁盘空间分析工具"),
            tr(&lang, "File viewer: syntax highlighting vs standard", "文本查看：代码高亮 vs 系统原生"),
            tr(&lang, "Text search tools", "文本搜索工具"),
            tr(&lang, "File list: modern glyphs & git vs classic", "目录浏览：现代化带图标 vs 传统列表"),
            tr(&lang, "File find: intuitive syntax vs powerful posix", "查找文件：直观语法 vs 标准 find")
        ).unwrap();
    } else {
        let rows = app.core.compare(&names).map_err(error)?;
        write!(body, "<p class='compare-help'>{}</p><div class='compare-matrix-wrap' tabindex='0' aria-label='{}'><table class='compare-table'><thead><tr><th scope='col' class='compare-attr-th'>",
                tr(&lang, "Features and examples come from the Catalog. Not recorded does not mean not supported; installation status reflects your saved scan. Command examples are not executed.",
                "特性与示例来自工具库；未记录不等于不支持。安装状态基于已保存扫描，命令示例不会执行。"),
                tr(&lang, "Tool comparison table", "工具比较表格")).unwrap();
        body.push_str(tr(&lang, "DIMENSION", "对比维度"));
        body.push_str("</th>");
        for r in &rows {
            write!(
                    body,
                    "<th scope='col' class='compare-tool-th'><div class='compare-tool-card'><span class='mini-avatar'>{}</span><a href='/tools/{}'><strong>{}</strong></a></div></th>",
                    esc(&r.name.chars().next().unwrap_or('›').to_string()),
                    esc(&r.id),
                    esc(&r.name)
                ).unwrap();
        }
        body.push_str("</tr></thead><tbody>");

        row(
            &mut body,
            tr(&lang, "Purpose", "用途说明"),
            &rows,
            |r| {
                let text = cliary_core::localized(&r.description, &lang);
                recorded(text, &lang)
            },
        );
        let features: std::collections::BTreeSet<_> =
            rows.iter().flat_map(|r| r.features.keys()).collect();
        if features.is_empty() {
            row(&mut body, tr(&lang, "Features", "特性"), &rows, |_| {
                recorded("", &lang)
            });
        }
        for key in features {
            let differs = rows.iter().any(|r| r.features.get(key) == Some(&true))
                && rows.iter().any(|r| r.features.get(key) == Some(&false));
            write!(
                body,
                "<tr{}><th scope='row' class='compare-attr-th'>{}",
                if differs {
                    " class='compare-feature-diff'"
                } else {
                    ""
                },
                esc(cliary_core::compare_feature_label(key, &lang))
            )
            .unwrap();
            if differs {
                write!(
                    body,
                    "<small>{}</small>",
                    tr(&lang, "Values differ", "存在差异")
                )
                .unwrap();
            }
            body.push_str("</th>");
            for r in &rows {
                let value = match r.features.get(key) {
                    Some(true) => tr(&lang, "Supported", "支持"),
                    Some(false) => tr(&lang, "Not supported", "不支持"),
                    None => tr(&lang, "Not recorded", "未记录"),
                };
                write!(body, "<td class='compare-cell'>{}</td>", value).unwrap();
            }
            body.push_str("</tr>");
        }
        let scanned = app.core.has_scanned().map_err(error)?;
        let dimensions: Vec<CompareDimension<'_>> = vec![
            (
                "Status",
                "运行状态",
                Box::new(|r| {
                    if !scanned {
                        format!(
                            "<span class='dim'>{}</span>",
                            tr(&lang, "Not scanned", "尚未扫描")
                        )
                    } else if r.installed {
                        format!(
                            "<span class='status-pill installed-badge'><span class='local-dot'></span>{}</span>",
                            tr(&lang, "Installed", "已安装")
                        )
                    } else {
                        format!(
                            "<span class='dim'>{}</span>",
                            tr(&lang, "Not detected", "未检测到")
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
                        .unwrap_or_else(|| recorded("", &lang))
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
                            "<a class='detail-meta-link' href='{}' target='_blank' rel='noreferrer'>{} ↗</a>",
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
                "<tr><th scope='row' class='compare-attr-th'>{}</th>",
                tr(&lang, dim_en, dim_zh)
            )
            .unwrap();
            for r in &rows {
                write!(body, "<td class='compare-cell'>{}</td>", extractor(r)).unwrap();
            }
            body.push_str("</tr>");
        }

        row(
            &mut body,
            tr(&lang, "Command examples", "命令示例"),
            &rows,
            |r| {
                if r.common_commands.is_empty() {
                    return recorded("", &lang);
                }
                r.common_commands
                    .iter()
                    .map(|command| format!("<code class='compare-command'>{}</code>", esc(command)))
                    .collect::<Vec<_>>()
                    .join("")
            },
        );
        body.push_str("</tbody></table></div>");
    }
    page(&app, "Compare", body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cliary_core::{Cliary, Paths};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn app(lang: &str) -> (TempDir, App) {
        let root = TempDir::new().unwrap();
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
                imports: Default::default(),
            },
        )
    }

    async fn html(app: &App, tools: &str) -> String {
        compare(
            State(app.clone()),
            Query(CompareQuery {
                tools: Some(tools.into()),
            }),
        )
        .await
        .unwrap()
        .0
    }

    fn feature_row<'a>(html: &'a str, label: &str) -> &'a str {
        html.split(&format!("class='compare-attr-th'>{label}"))
            .nth(1)
            .unwrap()
            .split("</tr>")
            .next()
            .unwrap()
    }

    #[tokio::test]
    async fn feature_matrix_distinguishes_false_unknown_and_real_differences() {
        for lang in ["en", "zh-CN"] {
            let (_root, app) = app(lang);
            let body = html(&app, "ncdu,dust,du").await;
            let interactive = feature_row(
                &body,
                cliary_core::compare_feature_label("interactive", lang),
            );
            for value in [
                tr(lang, "Supported", "支持"),
                tr(lang, "Not supported", "不支持"),
                tr(lang, "Not recorded", "未记录"),
            ] {
                assert!(interactive.contains(&format!(">{value}</td>")));
            }
            assert!(interactive.contains(tr(lang, "Values differ", "存在差异")));
            let parallel = feature_row(
                &body,
                cliary_core::compare_feature_label("parallel_scan", lang),
            );
            assert!(!parallel.contains(tr(lang, "Values differ", "存在差异")));
            assert_eq!(
                parallel.matches(tr(lang, "Not recorded", "未记录")).count(),
                2
            );
            assert_eq!(body.matches("class='compare-feature-diff'").count(), 3);
            assert!(body.contains("ncdu -x /"));
            assert!(body.contains(tr(lang, "Purpose", "用途说明")));
            assert!(body.contains(tr(lang, "Not scanned", "尚未扫描")));
            assert!(!body.contains("delete_files=true"));

            let db = rusqlite::Connection::open(&app.core.paths.user_db).unwrap();
            db.execute(
                "INSERT INTO meta(key,value) VALUES('installed_scan_state','done')",
                [],
            )
            .unwrap();
            db.execute("INSERT INTO installed VALUES ('ncdu','/usr/bin/ncdu','test-version','apt','ncdu',0)", []).unwrap();
            let scanned = html(&app, "ncdu,dust").await;
            assert!(!scanned.contains(tr(lang, "Not scanned", "尚未扫描")));
            assert!(scanned.contains(tr(lang, "Installed", "已安装")));
            assert!(scanned.contains(tr(lang, "Not detected", "未检测到")));
            assert!(scanned.contains("test-version"));
            assert_eq!(app.core.history(None).unwrap().runs, 0);
        }
    }

    #[tokio::test]
    async fn invalid_input_stays_editable_and_aliases_share_one_column() {
        let (_root, app) = app("zh-CN");
        for tools in [
            "ncdu",
            "ncdu,ncdu",
            "rg,ripgrep",
            "ncdu,不存在",
            "ncdu,gdu,dust,dua,du,rg,grep,fd,find",
        ] {
            let body = html(&app, tools).await;
            assert!(body.contains("role='alert'"), "{tools}");
            assert!(body.contains(&format!("value='{}'", esc(tools))));
            assert!(body.contains("href='/search'"));
            assert!(!body.contains("class='compare-table'"));
        }
        let empty = html(&app, " , ").await;
        assert!(empty.contains("常用对比组合"));
        assert!(!empty.contains("role='alert'"));
        let aliases = html(&app, "rg,ripgrep,grep").await;
        assert_eq!(aliases.matches("class='compare-tool-th'").count(), 2);
        let eight = html(&app, "ncdu,gdu,dust,dua,du,rg,grep,fd").await;
        assert_eq!(eight.matches("class='compare-tool-th'").count(), 8);
    }

    #[tokio::test]
    async fn sparse_catalog_and_untrusted_text_are_displayed_without_execution() {
        let (root, app) = app("zh-CN");
        let sparse = html(&app, "ripgrep,grep").await;
        assert!(feature_row(&sparse, "特性").contains("未记录"));
        assert!(!sparse.contains("class='compare-feature-diff'"));
        let mut tool = app.core.tool_detail("ncdu").unwrap().unwrap().tool;
        tool.description = [("en".into(), "<script>alert('purpose')</script>".into())].into();
        let marker = root.path().join("command-executed");
        tool.common_commands = vec![format!("touch {} && echo '<img src=x>'", marker.display())];
        tool.features.insert("<svg onload=alert(1)>".into(), true);
        let db = rusqlite::Connection::open(&app.core.paths.catalog_db).unwrap();
        db.execute(
            "UPDATE tools SET data=?1 WHERE id='ncdu'",
            [serde_json::to_string(&tool).unwrap()],
        )
        .unwrap();
        let body = html(&app, "ncdu,dust").await;
        assert!(body.contains("&lt;script&gt;alert(&#39;purpose&#39;)&lt;/script&gt;"));
        assert!(body.contains("&lt;svg onload=alert(1)&gt;"));
        assert!(body.contains("&lt;img src=x&gt;"));
        assert!(!body.contains("<script>alert('purpose')"));
        assert!(!marker.exists());
        let invalid = html(&app, "ncdu,<script>alert(1)</script>").await;
        assert!(invalid.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!invalid.contains("<script>alert(1)</script>"));
    }
}
