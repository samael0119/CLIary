mod shell;
mod ui;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use cliary_core::{Cliary, localized};
use serde_json::{Value, json};
use std::io::{IsTerminal, Write};

type CliCompareDimension<'a> = (
    &'a str,
    Box<dyn Fn(&cliary_core::CompareRow) -> String + 'a>,
);

#[derive(Parser)]
#[command(
    name = "cliary",
    version,
    about = "Discover your tools. Remember how you use them."
)]
struct Args {
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    lang: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Search {
        query: String,
    },
    Show {
        tool: String,
    },
    Compare {
        tools: Vec<String>,
    },
    Installed {
        #[arg(long)]
        all: bool,
    },
    Scan,
    Categories,
    Favorites,
    Favorite {
        tool: String,
        #[arg(long)]
        remove: bool,
        /// Remove this saved ID without resolving current names or aliases.
        #[arg(long, requires = "remove")]
        exact_id: bool,
    },
    Note {
        tool: String,
        #[arg(long)]
        set: Option<String>,
        #[arg(long)]
        delete: bool,
    },
    History {
        tool: Option<String>,
        /// Show undated imported observations, excluded from usage statistics.
        #[arg(long, conflicts_with = "tool")]
        undated: bool,
    },
    /// Preview or apply a selected Shell history file. Never imports automatically.
    ImportHistory {
        #[arg(long, value_enum)]
        shell: ImportShell,
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        apply: bool,
    },
    Stats {
        #[arg(long)]
        period: Option<String>,
        #[arg(long)]
        year: Option<i32>,
    },
    /// Annual usage report; defaults to the current local year.
    Wrapped {
        #[arg(allow_hyphen_values = true)]
        year: Option<i32>,
    },
    Sync {
        #[arg(long)]
        url: Option<String>,
    },
    Web,
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
    #[command(hide = true)]
    Internal {
        #[command(subcommand)]
        command: InternalCommand,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ImportShell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Subcommand)]
enum ConfigCommand {
    Show,
    Set { key: String, value: String },
}
#[derive(Subcommand)]
enum SetupCommand {
    Shell {
        #[arg(long)]
        shell: Option<String>,
        #[arg(long)]
        enable: bool,
        #[arg(long)]
        disable: bool,
    },
}
#[derive(Subcommand)]
enum InternalCommand {
    Record { executable: String },
    SkipScan,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if let Command::Internal {
        command: InternalCommand::Record { executable },
    } = &args.command
    {
        if let Ok(core) = Cliary::open() {
            let _ = core.record_usage(executable);
        }
        return Ok(());
    }
    let core = Cliary::open()?;
    if let Command::Internal {
        command: InternalCommand::SkipScan,
    } = &args.command
    {
        core.skip_initial_scan()?;
        return Ok(());
    }
    let lang = core.locale(args.lang.as_deref())?;
    if !matches!(
        &args.command,
        Command::ImportHistory { .. }
            | Command::Scan
            | Command::Sync { .. }
            | Command::Config { .. }
            | Command::Setup { .. }
    ) {
        if core.initial_scan_pending()? && !args.json && std::io::stdin().is_terminal() {
            first_run(&core, &lang)?;
        } else if core.ensure_initial_scan()? && !args.json {
            eprintln!(
                "{}",
                tr(
                    &lang,
                    "Initial installed-tool scan complete.",
                    "已完成首次安装工具扫描。"
                )
            );
        }
    }
    let theme = ui::Theme::detect();
    match args.command {
        Command::Search { query } => {
            let results = core.search(&query, 50)?;
            if args.json {
                out(json!(results))?
            } else if results.is_empty() {
                println!(
                    "  {}",
                    theme.dim(tr(
                        &lang,
                        "No tools found matching your query.",
                        "未找到匹配的命令行工具。"
                    ))
                );
            } else {
                println!();
                println!(
                    "  {:<18} {:<15} {}",
                    theme.bold(tr(&lang, "NAME", "工具名称")),
                    theme.bold(tr(&lang, "STATUS", "本地状态")),
                    theme.bold(tr(&lang, "DESCRIPTION", "描述"))
                );
                println!("  {}", theme.dim(&"─".repeat(72)));
                for r in &results {
                    let name_str = if r.installed {
                        theme.bright_green(&format!("{:<18}", r.tool.name))
                    } else {
                        theme.cyan(&format!("{:<18}", r.tool.name))
                    };
                    let status_str = theme.format_status(r.installed);
                    let desc = localized(&r.tool.description, &lang);
                    println!("  {name_str} {status_str} {desc}");
                }
                println!();
                println!(
                    "  {}",
                    theme.dim(tr(
                        &lang,
                        "Tip: Run 'cliary show <tool>' for commands, or 'cliary web' for UI",
                        "提示：运行 'cliary show <工具名>' 查看速查指令，或 'cliary web' 开启网页看板"
                    ))
                );
                println!();
            }
        }
        Command::Show { tool } => {
            let d = core.tool_detail(&tool)?.context("tool not found")?;
            if args.json {
                out(json!(d))?
            } else {
                let width = 76;
                let status_badge = if let Some(i) = &d.installed {
                    format!(
                        "{} v{} via {}",
                        tr(&lang, "Installed", "已就绪"),
                        i.version.as_deref().unwrap_or("?"),
                        i.source.as_deref().unwrap_or("?")
                    )
                } else if !core.has_scanned()? {
                    tr(&lang, "Not scanned", "尚未扫描").to_string()
                } else {
                    tr(&lang, "Not installed", "未安装").to_string()
                };

                println!();
                println!("{}", theme.box_header(&d.tool.name, &status_badge, width));
                println!(
                    "{}",
                    theme.box_line(&theme.dim(localized(&d.tool.description, &lang)))
                );
                println!("{}", theme.box_line(""));

                let category_data = core.categories()?;
                let tag_data = core.tags()?;
                let category_names = d
                    .tool
                    .categories
                    .iter()
                    .map(|id| {
                        category_data
                            .iter()
                            .find(|x| &x.id == id)
                            .map(|x| localized(&x.name, &lang))
                            .unwrap_or(id)
                            .to_string()
                    })
                    .collect::<Vec<_>>();
                let tag_names = d
                    .tool
                    .tags
                    .iter()
                    .map(|id| {
                        tag_data
                            .iter()
                            .find(|x| &x.id == id)
                            .map(|x| localized(&x.name, &lang))
                            .unwrap_or(id)
                            .to_string()
                    })
                    .collect::<Vec<_>>();

                let mut meta_parts = Vec::new();
                if !category_names.is_empty() {
                    meta_parts.push(format!(
                        "{}: {}",
                        theme.bold(tr(&lang, "Categories", "分类")),
                        theme.cyan(&category_names.join(", "))
                    ));
                }
                if let Some(lic) = &d.tool.license {
                    meta_parts.push(format!(
                        "{}: {}",
                        theme.bold(tr(&lang, "License", "协议")),
                        lic
                    ));
                }
                if let Some(homepage) = &d.tool.homepage {
                    meta_parts.push(format!(
                        "{}: {}",
                        theme.bold(tr(&lang, "Homepage", "主页")),
                        theme.link(homepage, homepage)
                    ));
                }
                if let Some(repo) = &d.tool.repository {
                    meta_parts.push(format!(
                        "{}: {}",
                        theme.bold(tr(&lang, "Repo", "仓库")),
                        theme.link(repo, repo)
                    ));
                }
                for line in meta_parts {
                    println!("{}", theme.box_line(&line));
                }
                if !tag_names.is_empty() {
                    println!(
                        "{}",
                        theme.box_line(&format!(
                            "{}: {}",
                            theme.bold(tr(&lang, "Tags", "标签")),
                            theme.dim(&tag_names.join(", "))
                        ))
                    );
                }

                if !d.tool.common_commands.is_empty() {
                    println!(
                        "{}",
                        theme.box_section(tr(&lang, "Common Commands", "常用速查指令"), width)
                    );
                    for command in &d.tool.common_commands {
                        println!(
                            "{}",
                            theme.box_line(&format!(
                                "  {} {}",
                                theme.green("$"),
                                theme.bold(command)
                            ))
                        );
                    }
                }

                if !d.tool.install.is_empty() {
                    println!(
                        "{}",
                        theme.box_section(tr(&lang, "Install Methods", "安装方式"), width)
                    );
                    for (manager, method) in &d.tool.install {
                        let cmd = method.command.as_deref().unwrap_or(&method.package);
                        println!(
                            "{}",
                            theme.box_line(&format!(
                                "  {:<8} {}",
                                theme.dim(&format!("{manager}:")),
                                theme.yellow(cmd)
                            ))
                        );
                    }
                }

                println!(
                    "{}",
                    theme.box_section(tr(&lang, "Telemetry & Notes", "本地使用画像与心得"), width)
                );
                println!(
                    "{}",
                    theme.box_line(&format!(
                        "  {}: {}   {}: {} 天",
                        tr(&lang, "Runs", "累计运行"),
                        theme.bold(&d.run_count.to_string()),
                        tr(&lang, "Active days", "活跃天数"),
                        theme.bold(&d.active_days.to_string()),
                    ))
                );
                if let Some(first) = &d.first_used {
                    println!(
                        "{}",
                        theme.box_line(&format!(
                            "  {}: {}   {}: {}",
                            tr(&lang, "First used", "首次使用"),
                            theme.dim(first),
                            tr(&lang, "Last used", "最近使用"),
                            theme.dim(d.last_used.as_deref().unwrap_or("—"))
                        ))
                    );
                }
                if let Some(note) = &d.note
                    && !note.trim().is_empty()
                {
                    println!(
                        "{}",
                        theme.box_line(&format!(
                            "  {}: {}",
                            theme.bold(tr(&lang, "Note", "个人笔记")),
                            note
                        ))
                    );
                }
                if !d.similar.is_empty() {
                    let sim_names = d
                        .similar
                        .iter()
                        .map(|x| x.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!(
                        "{}",
                        theme.box_line(&format!(
                            "  {}: {}",
                            theme.bold(tr(&lang, "Similar tools", "相似工具推荐")),
                            theme.cyan(&sim_names)
                        ))
                    );
                }

                println!("{}", theme.box_footer(width));
                println!();
            }
        }
        Command::Compare { tools } => {
            let rows = core.compare(&tools)?;
            if args.json {
                out(json!(rows))?
            } else if rows.is_empty() {
                println!(
                    "  {}",
                    theme.dim(tr(
                        &lang,
                        "No tools found for comparison.",
                        "未找到可供对比的工具。"
                    ))
                );
            } else {
                println!();
                println!(
                    "  {}",
                    theme.bold(tr(&lang, "TOOL COMPARISON", "工具横向对比"))
                );
                println!("  {}", theme.dim(&"─".repeat(72)));

                let scanned = core.has_scanned()?;
                let dimensions: [CliCompareDimension<'_>; 7] = [
                    (
                        tr(&lang, "Installed", "安装状态"),
                        Box::new(|r| {
                            if !scanned {
                                theme.dim(tr(&lang, "Not scanned", "尚未扫描"))
                            } else if r.installed {
                                theme.green(tr(&lang, "Installed", "已安装"))
                            } else {
                                theme.dim(tr(&lang, "Not detected", "未检测到"))
                            }
                        }),
                    ),
                    (
                        tr(&lang, "Version", "版本号"),
                        Box::new(|r| r.version.clone().unwrap_or_else(|| "—".into())),
                    ),
                    (
                        tr(&lang, "Source", "包来源"),
                        Box::new(|r| r.source.clone().unwrap_or_else(|| "—".into())),
                    ),
                    (
                        tr(&lang, "Language", "开发语言"),
                        Box::new(|r| {
                            r.implementation_language
                                .clone()
                                .unwrap_or_else(|| "—".into())
                        }),
                    ),
                    (
                        tr(&lang, "License", "开源协议"),
                        Box::new(|r| r.license.clone().unwrap_or_else(|| "—".into())),
                    ),
                    (
                        tr(&lang, "Platforms", "支持平台"),
                        Box::new(|r| r.platforms.join(", ")),
                    ),
                    (
                        tr(&lang, "Maintenance", "维护状态"),
                        Box::new(|r| r.maintenance_status.clone().unwrap_or_else(|| "—".into())),
                    ),
                ];

                let mut header = format!("  {:<14}", theme.dim(tr(&lang, "ATTRIBUTE", "对比维度")));
                for r in &rows {
                    header.push_str(&format!("  {:<24}", theme.bold(&r.name)));
                }
                println!("{header}");
                println!("  {}", theme.dim(&"─".repeat(72)));

                for (dim_label, extractor) in dimensions {
                    let mut line = format!("  {:<14}", theme.cyan(dim_label));
                    for r in &rows {
                        let val = extractor(r);
                        line.push_str(&format!("  {:<24}", val));
                    }
                    println!("{line}");
                }
                let features: std::collections::BTreeSet<_> =
                    rows.iter().flat_map(|r| r.features.keys()).collect();
                for key in features {
                    let mut line = format!(
                        "  {:<14}",
                        theme.cyan(cliary_core::compare_feature_label(key, &lang))
                    );
                    for r in &rows {
                        let value = match r.features.get(key) {
                            Some(true) => tr(&lang, "Supported", "支持"),
                            Some(false) => tr(&lang, "Not supported", "不支持"),
                            None => tr(&lang, "Not recorded", "未记录"),
                        };
                        line.push_str(&format!("  {value:<24}"));
                    }
                    println!("{line}");
                }
                println!("\n  {}", theme.dim(tr(&lang,
                    "Features are Catalog records; missing data is not a negative. Scan status reflects the saved scan.",
                    "特性来自工具库；未记录不等于不支持。安装状态基于已保存扫描。")));
                for row in &rows {
                    println!("\n  {}", theme.bold(&row.name));
                    let description = localized(&row.description, &lang);
                    println!(
                        "  {}",
                        if description.is_empty() {
                            tr(&lang, "Purpose not recorded.", "用途尚未记录。")
                        } else {
                            description
                        }
                    );
                    for (manager, method) in &row.install {
                        println!(
                            "  {}: {manager}:{}",
                            tr(&lang, "Package", "安装包"),
                            method.package
                        );
                    }
                    if let Some(repository) = &row.repository {
                        println!("  {}: {repository}", tr(&lang, "Repository", "仓库"));
                    }
                    if !row.common_commands.is_empty() {
                        println!(
                            "  {}",
                            theme.dim(tr(
                                &lang,
                                "Catalog command examples (not executed):",
                                "工具库命令示例（不会执行）："
                            ))
                        );
                        for command in &row.common_commands {
                            println!("    {command}");
                        }
                    }
                }
                println!();
            }
        }
        Command::Installed { all } => {
            let installed = core.installed()?;
            if args.json {
                out(json!(installed))?
            } else {
                let items: Vec<_> = installed
                    .into_iter()
                    .filter(|x| all || x.tool_id.is_some())
                    .collect();
                if items.is_empty() {
                    println!(
                        "  {}",
                        theme.dim(tr(
                            &lang,
                            "No installed tools found. Run 'cliary scan' to discover.",
                            "未发现已安装工具。运行 'cliary scan' 开始扫描。"
                        ))
                    );
                } else {
                    println!();
                    println!(
                        "  {:<20} {:<16} {:<12} {}",
                        theme.bold(tr(&lang, "EXECUTABLE", "程序名称")),
                        theme.bold(tr(&lang, "VERSION", "版本")),
                        theme.bold(tr(&lang, "SOURCE", "来源")),
                        theme.bold(tr(&lang, "TOOL ID", "目录ID"))
                    );
                    println!("  {}", theme.dim(&"─".repeat(68)));
                    for i in items {
                        let name_str = theme.bright_green(&format!("{:<20}", i.executable));
                        let ver = i.version.unwrap_or_else(|| "—".into());
                        let src = i.source.unwrap_or_else(|| "—".into());
                        let tid = i.tool_id.unwrap_or_default();
                        println!(
                            "  {name_str} {:<16} {:<12} {}",
                            theme.dim(&ver),
                            theme.cyan(&src),
                            theme.dim(&tid)
                        );
                    }
                    println!();
                }
            }
        }
        Command::Scan => {
            let entries = core.scan_installed()?;
            status(
                args.json,
                &format!("{} {}", tr(&lang, "Scanned", "已扫描"), entries.len()),
            )?;
        }
        Command::Categories => {
            let cats = core.categories()?;
            if args.json {
                out(json!(cats))?
            } else {
                println!();
                println!(
                    "  {:<18} {}",
                    theme.bold(tr(&lang, "CATEGORY ID", "分类标识")),
                    theme.bold(tr(&lang, "NAME", "分类名称"))
                );
                println!("  {}", theme.dim(&"─".repeat(50)));
                for c in cats {
                    println!("  {:<18} {}", theme.cyan(&c.id), localized(&c.name, &lang));
                }
                println!();
            }
        }
        Command::Favorites => {
            let favorites = core.favorite_entries()?;
            if args.json {
                let entries = favorites
                    .into_iter()
                    .map(|entry| match entry.tool {
                        Some(tool) => serde_json::to_value(tool),
                        None => Ok(json!({"id": entry.id, "catalog_available": false})),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                out(json!(entries))?
            } else if favorites.is_empty() {
                println!(
                    "  {}",
                    theme.dim(tr(
                        &lang,
                        "No favorites yet. Use 'cliary favorite <tool>' to bookmark.",
                        "暂无收藏工具。使用 'cliary favorite <工具名>' 添加收藏。"
                    ))
                );
            } else {
                println!();
                println!(
                    "  {:<18} {}",
                    theme.bold(tr(&lang, "TOOL", "工具名称")),
                    theme.bold(tr(&lang, "DESCRIPTION", "描述"))
                );
                println!("  {}", theme.dim(&"─".repeat(68)));
                for f in favorites {
                    let description = f.tool.as_ref().map(|tool| localized(&tool.description, &lang))
                        .unwrap_or_else(|| tr(&lang,
                            "Catalog entry unavailable; remove with 'cliary favorite <id> --remove --exact-id'.",
                            "当前工具库无此资料；可用 'cliary favorite <id> --remove --exact-id' 取消收藏。"));
                    println!("  {:<18} {}", theme.bright_green(&f.id), description);
                }
                println!();
            }
        }
        Command::Favorite {
            tool,
            remove,
            exact_id,
        } => {
            if exact_id {
                core.remove_favorite(&tool)?;
            } else {
                core.set_favorite(&tool, !remove)?;
            }
            status(
                args.json,
                if remove {
                    tr(&lang, "Favorite removed", "已取消收藏")
                } else {
                    tr(&lang, "Favorite added", "已收藏")
                },
            )?;
        }
        Command::Note { tool, set, delete } => {
            if delete {
                core.save_note(&tool, "")?;
            } else if let Some(text) = set {
                core.save_note(&tool, &text)?;
            } else {
                edit_note(&core, &tool)?;
            }
            status(args.json, tr(&lang, "Note saved", "备注已保存"))?;
        }
        Command::History { tool, undated } => {
            if undated {
                let items = core.undated_history()?;
                if args.json {
                    out(json!(items))?;
                } else {
                    println!(
                        "{}",
                        tr(
                            &lang,
                            "Undated observations — excluded from all statistics",
                            "无日期历史记录，不进入任何时间统计"
                        )
                    );
                    for item in items {
                        println!("{}  {}", item.executable, item.occurrences);
                    }
                }
                return Ok(());
            }
            let history = core.history(tool.as_deref())?;
            if args.json {
                out(json!(history))?
            } else {
                println!();
                println!("  {}", theme.bold(tr(&lang, "USAGE HISTORY", "使用历史")));
                println!("  {}", theme.dim(&"─".repeat(50)));
                println!(
                    "  {:<16} {}",
                    theme.dim(tr(&lang, "Runs", "运行次数")),
                    theme.bold(&history.runs.to_string())
                );
                println!(
                    "  {:<16} {} 天",
                    theme.dim(tr(&lang, "Active days", "活跃天数")),
                    theme.bold(&history.active_days.to_string())
                );
                println!(
                    "  {:<16} {}",
                    theme.dim(tr(&lang, "First used", "首次使用")),
                    history.first_used.unwrap_or_else(|| "—".into())
                );
                println!(
                    "  {:<16} {}",
                    theme.dim(tr(&lang, "Last used", "最近使用")),
                    history.last_used.unwrap_or_else(|| "—".into())
                );
                println!();
            }
        }
        Command::Stats { period, year } => {
            let days = period.as_deref().map(parse_period).transpose()?;
            let stats = core.stats(days, year)?;
            if args.json {
                out(json!(stats))?
            } else {
                let period_desc = if let Some(d) = days {
                    format!("近 {d} 天")
                } else if let Some(y) = year {
                    format!("{y} 年")
                } else {
                    "全部历史".to_string()
                };

                println!();
                println!(
                    "{}",
                    theme.box_header(&format!("CLIary 使用统计分析 ({period_desc})"), "", 76)
                );
                println!(
                    "{}",
                    theme.box_line(&format!(
                        "  {}: {}    {}: {} 天    {}: {} 款",
                        theme.bold(tr(&lang, "Total runs", "总运行次数")),
                        theme.bright_green(&stats.total_runs.to_string()),
                        theme.bold(tr(&lang, "Active days", "活跃天数")),
                        theme.cyan(&stats.active_days.to_string()),
                        theme.bold(tr(&lang, "Tools used", "使用工具数")),
                        theme.yellow(&stats.tools_used.to_string())
                    ))
                );
                println!("{}", theme.box_line(""));

                if !stats.top_tools.is_empty() {
                    println!(
                        "{}",
                        theme.box_section(tr(&lang, "Top Tools", "最常使用工具 (Top Tools)"), 76)
                    );
                    let max_run = stats.top_tools.first().map(|x| x.count).unwrap_or(1).max(1);
                    for (idx, item) in stats.top_tools.iter().take(8).enumerate() {
                        let bar = ui::render_bar(item.count as usize, max_run as usize, 20);
                        let pct = if stats.total_runs > 0 {
                            item.count * 100 / stats.total_runs
                        } else {
                            0
                        };
                        println!(
                            "{}",
                            theme.box_line(&format!(
                                "  {:>2}. {:<14} {} {:>4} 次 ({:>2}%)",
                                idx + 1,
                                theme.bold(&item.name),
                                theme.green(&bar),
                                item.count,
                                pct
                            ))
                        );
                    }
                }

                if !stats.category_usage.is_empty() {
                    println!(
                        "{}",
                        theme.box_section(tr(&lang, "Category Distribution", "分类偏好分布"), 76)
                    );
                    let max_cat = stats
                        .category_usage
                        .first()
                        .map(|x| x.count)
                        .unwrap_or(1)
                        .max(1);
                    for item in stats.category_usage.iter().take(6) {
                        let bar = ui::render_bar(item.count as usize, max_cat as usize, 16);
                        println!(
                            "{}",
                            theme.box_line(&format!(
                                "  {:<18} {} {:>4} 次",
                                theme.cyan(&item.name),
                                theme.dim(&bar),
                                item.count
                            ))
                        );
                    }
                }

                if stats.new_tools > 0 {
                    println!(
                        "{}",
                        theme.box_section(tr(&lang, "New Discoveries", "最新发现工具"), 76)
                    );
                    println!(
                        "{}",
                        theme.box_line(&format!(
                            "  {} 款: {}",
                            stats.new_tools,
                            theme.dim(&stats.new_tool_names.join(", "))
                        ))
                    );
                }

                if !stats.dormant_favorites.is_empty() {
                    println!(
                        "{}",
                        theme.box_section(tr(&lang, "Dormant Favorites", "很久未用的收藏"), 76)
                    );
                    println!(
                        "{}",
                        theme.box_line(&format!(
                            "  {}",
                            theme.dim(&stats.dormant_favorites.join(", "))
                        ))
                    );
                }

                println!("{}", theme.box_footer(76));
                println!();
            }
        }
        Command::ImportHistory { shell, file, apply } => {
            let format = match shell {
                ImportShell::Bash => cliary_core::HistoryFormat::Bash,
                ImportShell::Zsh => cliary_core::HistoryFormat::Zsh,
                ImportShell::Fish => cliary_core::HistoryFormat::Fish,
            };
            let report = core.import_history(&file, format, apply)?;
            if args.json {
                out(json!(report))?;
            } else {
                println!(
                    "{}",
                    tr(
                        &lang,
                        if apply {
                            "History import applied"
                        } else {
                            "Preview only — nothing imported"
                        },
                        if apply {
                            "历史导入完成"
                        } else {
                            "仅预览，尚未导入"
                        }
                    )
                );
                println!(
                    "{}: {} · {}: {} · {}: {}",
                    tr(&lang, "New dated records", "新增有日期记录"),
                    report.new_timed,
                    tr(&lang, "Undated observations", "无日期记录"),
                    report.undated,
                    tr(&lang, "Skipped / duplicates", "跳过 / 重复"),
                    format_args!("{} / {}", report.skipped, report.duplicates)
                );
                if let (Some(first), Some(last)) = (report.first_timestamp, report.last_timestamp) {
                    let date = |t| {
                        chrono::DateTime::from_timestamp(t, 0)
                            .unwrap()
                            .with_timezone(&chrono::Local)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                    };
                    println!(
                        "{}: {} → {}",
                        tr(&lang, "Local date range", "本地日期范围"),
                        date(first),
                        date(last)
                    );
                }
                println!(
                    "{}: {}",
                    tr(&lang, "Sample tools", "工具预览"),
                    report.sample_tools.join(", ")
                );
                println!(
                    "{}",
                    tr(
                        &lang,
                        "No arguments saved. Undated observations are excluded from annual statistics. Shell history may omit executions; aliases and compound commands cannot be reconstructed.",
                        "不保存参数，无日期记录不进入年度统计。Shell 历史可能遗漏调用；不会还原别名或复合命令。"
                    )
                );
                if !apply {
                    println!(
                        "{}",
                        tr(
                            &lang,
                            "Add --apply to import this file. Same-second identical tools are deduplicated against live capture.",
                            "添加 --apply 才会导入此文件；会与同秒同工具的实时采集记录去重。"
                        )
                    );
                }
            }
        }
        Command::Wrapped { year } => {
            let report = core.wrapped(year)?;
            if args.json {
                out(json!(report))?;
            } else {
                println!("\n  {} — {}", theme.bold("CLIary Wrapped"), report.year);
                if report.is_current_year {
                    println!(
                        "  {}",
                        tr(&lang, "This year is still in progress.", "本年度尚未结束。")
                    );
                }
                println!(
                    "  {}: {} · {}: {} · {}: {}",
                    tr(&lang, "Captured runs", "已记录调用"),
                    report.total_runs,
                    tr(&lang, "Active days", "活跃天数"),
                    report.active_days,
                    tr(&lang, "Tools used", "使用工具数"),
                    report.tools_used
                );
                println!("  {}", theme.dim(tr(&lang,
                    "Local calendar time. Captured invocations only; gaps do not mean inactivity.",
                    "按本地日历统计，仅包含已采集的调用；记录空缺不代表没有使用。")));
                if report.total_runs == 0 {
                    println!(
                        "\n  {}",
                        tr(
                            &lang,
                            "No records for this year. Try another year. To capture future commands:",
                            "该年暂无记录，可尝试其他年份。采集之后的命令："
                        )
                    );
                    println!("  cliary setup shell --enable");
                    println!(
                        "  {}",
                        tr(
                            &lang,
                            "Then open a new terminal. Earlier commands are not imported.",
                            "然后打开新终端；不会导入之前的命令。"
                        )
                    );
                } else {
                    println!(
                        "  {}: {} → {}",
                        tr(&lang, "First / last recorded", "首条 / 末条记录"),
                        report.first_recorded.as_deref().unwrap_or("—"),
                        report.last_recorded.as_deref().unwrap_or("—")
                    );
                    println!(
                        "\n  {}",
                        theme.bold(tr(&lang, "Top tools (up to 10)", "常用工具（最多 10 项）"))
                    );
                    for (rank, item) in report.top_tools.iter().enumerate() {
                        println!("  {:>2}. {}  {}", rank + 1, item.name, item.count);
                    }
                }
                println!(
                    "\n  {}",
                    theme.bold(tr(&lang, "Captured runs by month", "每月已记录调用"))
                );
                for month in &report.monthly_activity {
                    println!("  {}  {}", month.name, month.count);
                }
                println!();
            }
        }
        Command::Sync { url } => {
            let manifest = core.sync_catalog(url.as_deref())?;
            if args.json {
                out(json!(manifest))?
            } else {
                println!(
                    "{} v{}: {} {}",
                    tr(&lang, "Catalog", "目录"),
                    manifest.version,
                    manifest.tool_count,
                    tr(&lang, "tools", "个工具")
                );
            }
        }
        Command::Web => cliary_web::serve(core)?,
        Command::Config { command } => match command {
            None | Some(ConfigCommand::Show) => {
                let config = core.config()?;
                if args.json {
                    out(json!(config))?
                } else {
                    println!(
                        "language = {}\ncatalog_url = {}",
                        config.language.unwrap_or_else(|| "auto".into()),
                        config.catalog_url.unwrap_or_default()
                    );
                }
            }
            Some(ConfigCommand::Set { key, value }) => {
                match key.as_str() {
                    "language" => core.set_language(&value)?,
                    "catalog_url" => core.set_catalog_url(&value)?,
                    _ => bail!("supported keys: language, catalog_url"),
                };
                status(args.json, tr(&lang, "Config updated", "配置已更新"))?;
            }
        },
        Command::Setup { command } => match command {
            SetupCommand::Shell {
                shell,
                enable,
                disable,
            } => {
                if enable && disable {
                    bail!("choose --enable or --disable");
                }
                let shell = shell.unwrap_or_else(shell::current_shell);
                shell::configure(&core, &shell, !disable)?;
                status(
                    args.json,
                    if disable {
                        tr(&lang, "Shell integration disabled", "Shell 集成已关闭")
                    } else {
                        tr(&lang, "Shell integration enabled", "Shell 集成已启用")
                    },
                )?;
            }
        },
        Command::Internal { .. } => unreachable!(),
    }
    Ok(())
}

fn tr<'a>(lang: &str, en: &'a str, zh: &'a str) -> &'a str {
    if lang == "zh-CN" { zh } else { en }
}
fn first_run(core: &Cliary, lang: &str) -> Result<()> {
    eprintln!(
        "\n{}\n{}\n",
        tr(lang, "Welcome to CLIary", "欢迎使用 CLIary"),
        tr(
            lang,
            "Let's set up your local workspace. This takes about a minute.",
            "我们先设置你的本地工作空间，大约需要一分钟。"
        )
    );
    if ask_yes_no(
        tr(
            lang,
            "Scan installed CLI tools now? [Y/n] ",
            "现在扫描已安装的命令行工具？[Y/n] ",
        ),
        true,
    )? {
        eprintln!(
            "{}",
            tr(lang, "Scanning installed tools…", "正在扫描已安装工具…")
        );
        let count = core.scan_installed()?.len();
        eprintln!(
            "{} {count}",
            tr(lang, "Executables found:", "发现可执行程序：")
        );
    } else {
        core.skip_initial_scan()?;
        eprintln!(
            "{}",
            tr(
                lang,
                "Skipped. Run `cliary scan` whenever you're ready.",
                "已跳过。你可以随时运行 `cliary scan`。"
            )
        );
    }
    let shell_name = shell::current_shell();
    if ["bash", "zsh", "fish"].contains(&shell_name.as_str())
        && ask_yes_no(
            tr(
                lang,
                "Enable private tool usage history for this shell? [Y/n] ",
                "为当前 Shell 启用工具使用历史？[Y/n] ",
            ),
            true,
        )?
    {
        if let Err(err) = shell::configure(core, &shell_name, true) {
            eprintln!(
                "{}: {err}",
                tr(lang, "Shell setup failed", "Shell 设置失败")
            );
        } else {
            eprintln!(
                "{}",
                tr(
                    lang,
                    "Shell history enabled. Open a new terminal to start recording executable names only.",
                    "Shell 历史已启用。打开新终端后开始记录，仅保存可执行文件名。"
                )
            );
        }
    }
    eprintln!(
        "\n{}\n",
        tr(
            lang,
            "Try `cliary search 磁盘空间` or `cliary web` next.",
            "接下来试试 `cliary search 磁盘空间` 或 `cliary web`。"
        )
    );
    Ok(())
}
fn ask_yes_no(prompt: &str, default_yes: bool) -> Result<bool> {
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    let answer = answer.trim().to_ascii_lowercase();
    Ok(match answer.as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default_yes,
    })
}
fn out(value: Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
fn status(json: bool, message: &str) -> Result<()> {
    if json {
        out(serde_json::json!({"ok":true,"message":message}))
    } else {
        println!("{message}");
        Ok(())
    }
}
fn parse_period(value: &str) -> Result<u32> {
    let Some(days) = value.strip_suffix('d') else {
        bail!("period must be like 7d or 30d")
    };
    Ok(days.parse()?)
}

fn edit_note(core: &Cliary, tool: &str) -> Result<()> {
    let detail = core.tool_detail(tool)?.context("tool not found")?;
    let file = core
        .paths
        .cache_dir
        .join(format!("note-{}.txt", std::process::id()));
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut f = options.open(&file)?;
    f.write_all(detail.note.unwrap_or_default().as_bytes())?;
    drop(f);
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let parts = shell_words::split(&editor)?;
    let (program, args) = parts.split_first().context("empty EDITOR")?;
    let status = std::process::Command::new(program)
        .args(args)
        .arg(&file)
        .status()?;
    let result = if status.success() {
        core.save_note(tool, &std::fs::read_to_string(&file)?)
    } else {
        Err(anyhow::anyhow!("editor failed"))
    };
    let _ = std::fs::remove_file(&file);
    result
}
