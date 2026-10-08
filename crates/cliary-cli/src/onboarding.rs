use crate::tr;
use anyhow::Result;
use cliary_core::{Cliary, HistoryCandidate, HistoryFormat};
use std::{
    io::{BufRead, Write},
    path::PathBuf,
};

pub fn offer_history_import(core: &Cliary, lang: &str) -> Result<()> {
    let candidates = cliary_core::history_candidates()?;
    let finished = guide(
        core,
        lang,
        &candidates,
        &mut std::io::stdin().lock(),
        &mut std::io::stderr().lock(),
    )?;
    if finished {
        core.complete_history_import_onboarding()?;
    }
    Ok(())
}
fn line(input: &mut impl BufRead, output: &mut impl Write, prompt: &str) -> Result<Option<String>> {
    write!(output, "{prompt}")?;
    output.flush()?;
    let mut value = String::new();
    if input.read_line(&mut value)? == 0 {
        return Ok(None);
    }
    Ok(Some(value.trim().to_owned()))
}
fn yes(input: &mut impl BufRead, output: &mut impl Write, prompt: &str) -> Result<Option<bool>> {
    loop {
        let Some(value) = line(input, output, prompt)? else {
            return Ok(None);
        };
        match value.to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(Some(true)),
            "" | "n" | "no" => return Ok(Some(false)),
            _ => writeln!(output, "y / n")?,
        }
    }
}
fn selected_path(value: &str) -> PathBuf {
    if let Some(tail) = value.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        home.join(tail)
    } else {
        PathBuf::from(value)
    }
}
fn guide(
    core: &Cliary,
    lang: &str,
    candidates: &[HistoryCandidate],
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<bool> {
    writeln!(
        output,
        "\n{}",
        tr(
            lang,
            "Bring in existing Shell history",
            "导入已有 Shell 历史"
        )
    )?;
    writeln!(
        output,
        "{}",
        tr(
            lang,
            "Bash, Zsh and Fish histories can fill in older records. Terminal tabs usually share a Shell history file. Files are read only after you choose to preview; arguments are never saved.",
            "Bash、Zsh、Fish 历史可补充旧记录。终端标签通常共用 Shell 历史文件；选择预览后才读取内容，不保存命令参数。"
        )
    )?;
    if candidates.is_empty() {
        writeln!(
            output,
            "{}",
            tr(
                lang,
                "No history file found at common paths. You can provide a custom path.",
                "常见路径未找到历史文件，可手动指定文件。"
            )
        )?;
    } else {
        writeln!(
            output,
            "{}",
            tr(
                lang,
                "Available files (contents not read):",
                "找到的文件（尚未读取内容）："
            )
        )?;
        for (i, c) in candidates.iter().enumerate() {
            writeln!(
                output,
                "  {}. {}  {}",
                i + 1,
                c.shell.name(),
                c.path.display()
            )?;
        }
    }
    match yes(
        input,
        output,
        tr(
            lang,
            "Select a history file and preview it now? [y/N] ",
            "现在选择历史文件并预览？[y/N] ",
        ),
    )? {
        None => return Ok(false),
        Some(false) => {
            writeln!(
                output,
                "{}",
                tr(
                    lang,
                    "Skipped. Run cliary setup history later, or open History in the Web UI.",
                    "已跳过。稍后运行 cliary setup history，或打开 Web 使用历史页。"
                )
            )?;
            return Ok(true);
        }
        Some(true) => (),
    }
    loop {
        writeln!(
            output,
            "{}",
            tr(
                lang,
                "0. Custom file; Enter to finish.",
                "0. 自定义文件；回车结束。"
            )
        )?;
        let selected = loop {
            let Some(value) = line(input, output, tr(lang, "File number: ", "选择文件编号："))?
            else {
                return Ok(false);
            };
            if value.is_empty() {
                return Ok(true);
            }
            if value == "0" {
                let format = loop {
                    let Some(value) = line(
                        input,
                        output,
                        tr(
                            lang,
                            "Shell format: 1 Bash / 2 Zsh / 3 Fish (Enter to finish): ",
                            "历史格式：1 Bash / 2 Zsh / 3 Fish（回车结束）：",
                        ),
                    )?
                    else {
                        return Ok(false);
                    };
                    match value.as_str() {
                        "1" => break HistoryFormat::Bash,
                        "2" => break HistoryFormat::Zsh,
                        "3" => break HistoryFormat::Fish,
                        "" => return Ok(true),
                        _ => continue,
                    }
                };
                let Some(value) = line(
                    input,
                    output,
                    tr(
                        lang,
                        "History file path (~/ supported): ",
                        "历史文件路径（支持 ~/）：",
                    ),
                )?
                else {
                    return Ok(false);
                };
                if value.is_empty() {
                    return Ok(true);
                }
                break HistoryCandidate {
                    shell: format,
                    path: selected_path(&value),
                };
            }
            if let Ok(n) = value.parse::<usize>()
                && let Some(c) = n.checked_sub(1).and_then(|n| candidates.get(n))
            {
                break c.clone();
            }
            writeln!(
                output,
                "{}",
                tr(
                    lang,
                    "Choose a listed number or 0.",
                    "请输入列表中的编号或 0。"
                )
            )?;
        };
        let aliases = if matches!(selected.shell, HistoryFormat::Zsh | HistoryFormat::Bash) {
            writeln!(
                output,
                "{}",
                tr(
                    lang,
                    "Aliases such as gst need definitions to count toward git. In your usual Shell, export a private alias file with alias -L (Zsh) or alias -p (Bash); remove it after importing. CLIary never loads your startup scripts.",
                    "gst 等别名需要定义才能归到 git。在日常 Shell 中用 alias -L（Zsh）或 alias -p（Bash）导出私有别名文件，用完删除；CLIary 不加载启动脚本。"
                )
            )?;
            let Some(value) = line(
                input,
                output,
                tr(
                    lang,
                    "Alias file path (optional; Enter to skip): ",
                    "别名表路径（可选；回车跳过）：",
                ),
            )?
            else {
                return Ok(false);
            };
            if value.is_empty() {
                None
            } else {
                Some(selected_path(&value))
            }
        } else {
            None
        };
        let preview = core
            .prepare_history_import(&selected.path, selected.shell, aliases.as_deref())
            .and_then(|plan| {
                core.preview_history_import(&plan)
                    .map(|report| (plan, report))
            });
        match preview {
            Err(err) => writeln!(
                output,
                "{}: {err}",
                tr(lang, "Preview failed; nothing imported", "预览失败，未导入")
            )?,
            Ok((plan, report)) => {
                writeln!(
                    output,
                    "\n{}: {}",
                    tr(lang, "Preview", "预览"),
                    selected.path.display()
                )?;
                writeln!(
                    output,
                    "{}: {} · {}: {} · {}: {} · {}: {}",
                    tr(lang, "Dated", "有日期"),
                    report.timed,
                    tr(lang, "Undated", "无日期"),
                    report.undated,
                    tr(lang, "Skipped", "跳过"),
                    report.skipped,
                    tr(lang, "Duplicates", "重复"),
                    report.duplicates
                )?;
                writeln!(
                    output,
                    "{}: {} · {}: {} · {}: {}",
                    tr(lang, "New dated", "新增有日期"),
                    report.new_timed,
                    tr(lang, "New undated observations", "新增无日期观察"),
                    report.new_undated,
                    tr(lang, "Existing records to classify", "待归类旧记录"),
                    report.reclassified_records
                )?;
                if let (Some(first), Some(last)) = (report.first_timestamp, report.last_timestamp) {
                    let date = |t| {
                        chrono::DateTime::from_timestamp(t, 0)
                            .unwrap()
                            .with_timezone(&chrono::Local)
                            .format("%Y-%m-%d %H:%M")
                            .to_string()
                    };
                    writeln!(
                        output,
                        "{}: {} → {}",
                        tr(lang, "Local date range", "本地日期范围"),
                        date(first),
                        date(last)
                    )?;
                }
                writeln!(
                    output,
                    "{}: {}",
                    tr(lang, "Executable sample", "程序名预览"),
                    report.sample_tools.join(", ")
                )?;
                for a in &report.alias_resolutions {
                    writeln!(output, "  {} → {}", a.alias, a.executable)?;
                }
                writeln!(
                    output,
                    "{}",
                    tr(
                        lang,
                        "Undated observations cannot enter annual reports. History may omit commands; alias snapshots cannot prove past configuration. No commands are executed.",
                        "无日期观察无法进入年报；历史可能遗漏命令，别名表不能证明过去配置。不会执行历史命令。"
                    )
                )?;
                if report.new_timed + report.new_undated + report.reclassified_records == 0 {
                    writeln!(
                        output,
                        "{}",
                        tr(
                            lang,
                            "No new records or classifications to apply.",
                            "没有需要新增或归类的记录。"
                        )
                    )?;
                } else {
                    match yes(
                        input,
                        output,
                        tr(lang, "Apply this import? [y/N] ", "导入这份历史？[y/N] "),
                    )? {
                        None => return Ok(false),
                        Some(false) => writeln!(
                            output,
                            "{}",
                            tr(lang, "Preview only; nothing imported.", "仅预览，未导入。")
                        )?,
                        Some(true) => match core.apply_history_import(&plan) {
                            Ok(applied) => writeln!(
                                output,
                                "{}: {} / {} / {}",
                                tr(
                                    lang,
                                    "Imported: dated / undated / classified",
                                    "导入完成：有日期 / 无日期 / 归类"
                                ),
                                applied.new_timed,
                                applied.new_undated,
                                applied.reclassified_records
                            )?,
                            Err(err) => writeln!(
                                output,
                                "{}: {err}",
                                tr(lang, "Import failed", "导入失败")
                            )?,
                        },
                    }
                }
            }
        }
        match yes(
            input,
            output,
            tr(
                lang,
                "Preview another Shell history file? [y/N] ",
                "继续预览另一份 Shell 历史？[y/N] ",
            ),
        )? {
            Some(true) => continue,
            Some(false) => return Ok(true),
            None => return Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    fn fixture() -> (tempfile::TempDir, Cliary, Vec<HistoryCandidate>) {
        let root = tempfile::tempdir().unwrap();
        let core = Cliary::at(cliary_core::Paths::new(
            root.path().join("config"),
            root.path().join("data"),
            root.path().join("cache"),
        ))
        .unwrap();
        let path = root.path().join("history");
        std::fs::write(&path, ": 1704067200:0;gst SECRET_ARG\n").unwrap();
        (
            root,
            core,
            vec![HistoryCandidate {
                shell: HistoryFormat::Zsh,
                path,
            }],
        )
    }
    #[test]
    fn default_skip_and_eof_do_not_read_contents_or_import() {
        let (_root, core, mut files) = fixture();
        files[0].path = Path::new("/does/not/exist").into();
        let mut output = Vec::new();
        assert!(guide(&core, "zh-CN", &files, &mut &b"\n"[..], &mut output).unwrap());
        assert!(!guide(&core, "en", &files, &mut &b""[..], &mut output).unwrap());
        assert_eq!(core.history(None).unwrap().runs, 0);
        assert!(core.history_import_onboarding_pending().unwrap());
        assert!(
            !String::from_utf8(output)
                .unwrap()
                .contains("Preview failed")
        );
    }
    #[test]
    fn preview_does_not_apply_without_explicit_yes() {
        let (_root, core, files) = fixture();
        let mut output = Vec::new();
        assert!(
            guide(
                &core,
                "zh-CN",
                &files,
                &mut &b"y\n1\n\n\nn\n"[..],
                &mut output
            )
            .unwrap()
        );
        assert_eq!(core.history(None).unwrap().runs, 0);
        assert!(!String::from_utf8(output).unwrap().contains("SECRET_ARG"));
        let mut output = Vec::new();
        assert!(!guide(&core, "en", &files, &mut &b"y\n1\n\n"[..], &mut output).unwrap());
        assert_eq!(core.history(None).unwrap().runs, 0);
    }
    #[test]
    fn apply_with_aliases_and_repeat_preserve_counts() {
        let (root, core, files) = fixture();
        let aliases = root.path().join("aliases");
        std::fs::write(&aliases, "alias gst='git status'\n").unwrap();
        let input = format!("y\n1\n{}\ny\nn\n", aliases.display());
        let mut output = Vec::new();
        assert!(guide(&core, "en", &files, &mut input.as_bytes(), &mut output).unwrap());
        assert_eq!(core.history(Some("git")).unwrap().runs, 1);
        core.complete_history_import_onboarding().unwrap();
        assert!(!core.history_import_onboarding_pending().unwrap());
        let input = format!("y\n1\n{}\nn\n", aliases.display());
        assert!(guide(&core, "en", &files, &mut input.as_bytes(), &mut output).unwrap());
        assert_eq!(core.history(None).unwrap().runs, 1);
    }
}
