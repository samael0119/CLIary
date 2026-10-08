//! Local, explicitly selected files. Prepared plans contain no raw commands.
use super::*;
use cliary_core::{HistoryFormat, HistoryImportPlan, ImportReport};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

const PREVIEW_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_PREVIEWS: usize = 4;

#[derive(Clone, Default)]
pub(super) struct Previews(Arc<Mutex<HashMap<String, Pending>>>);
struct Pending {
    plan: HistoryImportPlan,
    created: Instant,
    path: String,
    shell: String,
}
impl Previews {
    fn insert(
        &self,
        plan: HistoryImportPlan,
        path: String,
        shell: String,
    ) -> anyhow::Result<String> {
        let mut pending = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("preview unavailable"))?;
        pending.retain(|_, p| p.created.elapsed() < PREVIEW_TTL);
        if pending.len() >= MAX_PREVIEWS
            && let Some(oldest) = pending
                .iter()
                .min_by_key(|(_, p)| p.created)
                .map(|(k, _)| k.clone())
        {
            pending.remove(&oldest);
        }
        let mut bytes = [0; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let token = hex::encode(bytes);
        pending.insert(
            token.clone(),
            Pending {
                plan,
                created: Instant::now(),
                path,
                shell,
            },
        );
        Ok(token)
    }
}
#[derive(Deserialize, Default)]
pub(super) struct PreviewForm {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    shell: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    aliases: String,
}
#[derive(Deserialize)]
pub(super) struct ApplyForm {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    token: String,
}
type ImportResult = Result<(StatusCode, Html<String>), (StatusCode, String)>;
fn response(app: &App, status: StatusCode, body: String) -> ImportResult {
    Ok((status, page(app, "History", body)?))
}
fn intro(lang: &str, title: &str, copy: &str) -> String {
    format!(
        "<div class='history-import'><a class='text-link' href='/history'>{}</a><section class='page-intro'><h1>{}</h1><p>{}</p></section>",
        tr(lang, "Back to history", "返回使用历史"),
        esc(title),
        esc(copy)
    )
}
fn alias_field(lang: &str, id: &str, value: &str) -> String {
    format!(
        "<div class='import-field'><label for='{id}'>{}</label><input id='{id}' name='aliases' value='{}' placeholder='{}' aria-describedby='alias-help'><span class='import-caption'>{}</span></div>",
        tr(lang, "Alias snapshot (optional)", "别名快照路径（可选）"),
        esc(value),
        tr(lang, "Path to an exported alias file", "导出的别名文件路径"),
        tr(
            lang,
            "Leave blank to keep original executable names.",
            "留空则保留原始程序名；可用别名快照把 gst 归到 git。"
        )
    )
}
fn form_body(
    app: &App,
    lang: &str,
    values: &PreviewForm,
    failure: Option<&str>,
) -> Result<String, (StatusCode, String)> {
    let mut body = intro(
        lang,
        tr(lang, "Import Shell history", "导入 Shell 历史"),
        tr(
            lang,
            "Choose a file on the computer running CLIary. Preview first; nothing is imported until you confirm.",
            "选择运行 CLIary 的这台电脑上的文件。先预览，确认后才导入。",
        ),
    );
    if let Some(message) = failure {
        write!(body,"<div class='import-message' role='alert'><h2>{}</h2><p>{}</p><p class='import-caption'>{}</p></div>",tr(lang,"Could not preview this file","无法预览这份文件"),tr(lang,"Check the path, read permission and Shell format, then try again. History files must be UTF-8 and at most 32 MiB; alias snapshots at most 4 MiB.","请检查路径、读取权限和 Shell 格式后重试。历史文件需为 UTF-8，最大32 MiB；别名快照最大4 MiB。"),esc(message)).unwrap();
    }
    let candidates = cliary_core::history_candidates().map_err(error)?;
    let custom_open = candidates.is_empty() || values.source == "custom" || failure.is_some();
    if candidates.is_empty() {
        write!(
            body,
            "<p>{}</p>",
            tr(
                lang,
                "No standard history files found. Enter your actual file below.",
                "未发现常见位置的历史文件，请在下方填写实际路径。"
            )
        )
        .unwrap();
    } else {
        write!(body,"<section class='import-source'><h2>{}</h2><form method='post' action='/history/import/preview' class='import-form' data-history-import><input type='hidden' name='csrf' value='{}'><div class='import-field'><label for='history-source'>{}</label><select id='history-source' name='source'>",tr(lang,"Detected files","检测到的文件"),esc(&app.csrf),tr(lang,"History file","历史文件")).unwrap();
        for candidate in candidates {
            let path = candidate.path.to_string_lossy();
            let value = format!("{}:{path}", candidate.shell.name());
            write!(
                body,
                "<option value='{}' {}>{} · {}</option>",
                esc(&value),
                if values.source == value {
                    "selected"
                } else {
                    ""
                },
                candidate.shell.name(),
                esc(&path)
            )
            .unwrap();
        }
        write!(body,"</select><span class='import-caption'>{}</span></div>{}<button class='btn btn-primary' type='submit'>{}</button></form></section>",tr(lang,"Only filenames have been checked. File contents are read when you click Preview.","目前只检查了文件位置，点击预览时才读取内容。"),alias_field(lang,"detected-aliases",&values.aliases),tr(lang,"Preview this file","预览这份文件")).unwrap();
    }
    write!(body,"<details class='import-custom' {}><summary>{}</summary><form method='post' action='/history/import/preview' class='import-form' data-history-import><input type='hidden' name='csrf' value='{}'><input type='hidden' name='source' value='custom'><div class='import-fields'><div class='import-field'><label for='history-path'>{}</label><input id='history-path' name='path' required value='{}' placeholder='~/.zsh_history' spellcheck='false'></div><div class='import-field'><label for='history-shell'>{}</label><select id='history-shell' name='shell'>",if custom_open {"open"} else {""},tr(lang,"Choose another file","选择其他文件"),esc(&app.csrf),tr(lang,"History file path","历史文件路径"),esc(&values.path),tr(lang,"Shell format","Shell 格式")).unwrap();
    for shell in ["zsh", "bash", "fish"] {
        write!(
            body,
            "<option value='{shell}' {}>{shell}</option>",
            if values.shell == shell {
                "selected"
            } else {
                ""
            }
        )
        .unwrap();
    }
    write!(body,"</select></div></div>{}<button class='btn btn-primary' type='submit'>{}</button></form></details><section class='import-notes'><h2>{}</h2><p>{}</p><p>{}</p><details id='alias-help'><summary>{}</summary><p>{}</p><pre><code>zsh:  alias -L &gt; ~/cliary-aliases.txt\nbash: alias -p &gt; ~/cliary-aliases.txt</code></pre><p>{}</p></details></section></div>",alias_field(lang,"custom-aliases",&values.aliases),tr(lang,"Preview this file","预览这份文件"),tr(lang,"What gets saved","保存哪些内容"),tr(lang,"Only executable names, counts and reliable dates are saved; command arguments are discarded. Dated entries feed statistics and Wrapped. Undated entries stay in a separate tool list, without invented dates.","只保存程序名、次数和可靠日期，丢弃命令参数。有日期记录进入统计和年报；无日期记录单独列出，不补造日期。"),tr(lang,"Repeated import does not accumulate duplicates. To capture future commands, set up Shell integration separately.","重复导入不会重复累加。若要采集之后的命令，需另行启用 Shell 集成。"),tr(lang,"Resolve aliases such as gst","识别 gst 等别名"),tr(lang,"Export aliases in the terminal where they are defined, then enter the snapshot path above. CLIary does not load your Shell configuration.","在定义了别名的终端中导出，再把快照路径填入上方。CLIary 不会加载 Shell 配置。"),tr(lang,"A current snapshot may differ from old definitions. Fish history is supported; Fish functions are not alias snapshots.","当前快照可能与历史定义不同。支持 Fish 历史；Fish 函数不属于别名快照。")).unwrap();
    Ok(body)
}
pub(super) async fn choose(State(app): State<App>) -> WebResult {
    let lang = app.core.locale(None).map_err(error)?;
    page(
        &app,
        "History",
        form_body(&app, &lang, &PreviewForm::default(), None)?,
    )
}
fn path(value: &str) -> anyhow::Result<PathBuf> {
    let value = value.trim();
    anyhow::ensure!(!value.is_empty(), "history path is required");
    if let Some(rest) = value.strip_prefix("~/") {
        Ok(PathBuf::from(
            std::env::var_os("HOME")
                .ok_or_else(|| anyhow::anyhow!("home directory unavailable"))?,
        )
        .join(rest))
    } else {
        Ok(PathBuf::from(value))
    }
}
fn selection(form: &PreviewForm) -> anyhow::Result<(PathBuf, HistoryFormat)> {
    let (shell, file) = if form.source == "custom" {
        (form.shell.as_str(), form.path.as_str())
    } else {
        form.source
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("select a history file"))?
    };
    let shell = match shell {
        "zsh" => HistoryFormat::Zsh,
        "bash" => HistoryFormat::Bash,
        "fish" => HistoryFormat::Fish,
        _ => anyhow::bail!("choose Bash, Zsh or Fish"),
    };
    Ok((path(file)?, shell))
}
fn date(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|v| v.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "—".into())
}
fn report_body(
    app: &App,
    lang: &str,
    path: &str,
    shell: &str,
    report: &ImportReport,
    token: Option<&str>,
    failure: Option<&str>,
) -> String {
    let done = report.applied;
    let mut body = intro(
        lang,
        tr(
            lang,
            if done {
                "History imported"
            } else {
                "Review before importing"
            },
            if done {
                "历史已导入"
            } else {
                "确认导入内容"
            },
        ),
        tr(
            lang,
            if done {
                "The entries below have been added to your local workspace."
            } else {
                "This preview has not changed your records. Check the results before confirming."
            },
            if done {
                "以下记录已保存到本地工作区。"
            } else {
                "预览没有改变现有记录，请核对结果后确认。"
            },
        ),
    );
    if let Some(message) = failure {
        write!(
            body,
            "<div class='import-message' role='alert'><h2>{}</h2><p>{}</p><p>{}</p></div>",
            tr(lang, "Import did not finish", "导入未完成"),
            tr(
                lang,
                "Your preview is still available. Try again or preview another file.",
                "预览仍然可用，可重试或重新选择文件。"
            ),
            esc(message)
        )
        .unwrap();
    }
    write!(body,"<section class='import-results'><h2>{}</h2><p class='import-file'><code>{}</code><span>{}</span></p><dl class='import-counts'>",tr(lang,"Selected file","所选文件"),esc(path),esc(shell)).unwrap();
    for (label, value, help) in [
        (
            tr(
                lang,
                if done {
                    "Added dated entries"
                } else {
                    "New dated entries"
                },
                if done {
                    "已导入有日期记录"
                } else {
                    "新增有日期记录"
                },
            ),
            report.new_timed,
            tr(lang, "Included in statistics and Wrapped", "进入统计和年报"),
        ),
        (
            tr(
                lang,
                if done {
                    "Added undated observations"
                } else {
                    "New undated observations"
                },
                if done {
                    "已导入无日期观察"
                } else {
                    "新增无日期观察"
                },
            ),
            report.new_undated,
            tr(
                lang,
                "Kept separately; excluded from dated statistics",
                "单独保留，不进入有日期统计",
            ),
        ),
        (
            tr(lang, "Already retained", "已有记录"),
            report.duplicates,
            tr(lang, "Will not be added again", "不会重复添加"),
        ),
        (
            tr(lang, "Skipped entries", "跳过条目"),
            report.skipped,
            tr(
                lang,
                "Complex or unsupported commands",
                "复杂或不支持的命令",
            ),
        ),
        (
            tr(lang, "Existing records reclassified", "已有记录重新归类"),
            report.reclassified_records,
            tr(
                lang,
                "Matched using this alias snapshot",
                "按本次别名快照匹配",
            ),
        ),
    ] {
        write!(
            body,
            "<div><dt>{label}<small>{help}</small></dt><dd>{value}</dd></div>"
        )
        .unwrap();
    }
    body.push_str("</dl>");
    if let (Some(first), Some(last)) = (report.first_timestamp, report.last_timestamp) {
        write!(
            body,
            "<p class='import-caption'>{}: <time>{}</time> – <time>{}</time></p>",
            tr(lang, "Dated range", "有日期范围"),
            date(first),
            date(last)
        )
        .unwrap();
    } else {
        write!(
            body,
            "<p class='import-caption'>{}</p>",
            tr(
                lang,
                "No reliable dates found. These observations cannot populate an annual report.",
                "未找到可靠日期，这些观察记录无法进入年报。"
            )
        )
        .unwrap();
    }
    if !report.sample_tools.is_empty() {
        write!(
            body,
            "<h3>{}</h3><p class='import-tools'>",
            tr(
                lang,
                "Executable preview (up to 20)",
                "程序名预览（最多20项）"
            )
        )
        .unwrap();
        for tool in &report.sample_tools {
            write!(body, "<code>{}</code>", esc(tool)).unwrap();
        }
        body.push_str("</p>");
    }
    if !report.alias_resolutions.is_empty() {
        write!(
            body,
            "<h3>{}</h3><ul class='import-aliases'>",
            tr(lang, "Alias attribution", "别名归属")
        )
        .unwrap();
        for alias in &report.alias_resolutions {
            write!(
                body,
                "<li><code>{}</code> → <code>{}</code></li>",
                esc(&alias.alias),
                esc(&alias.executable)
            )
            .unwrap();
        }
        body.push_str("</ul>");
    }
    body.push_str("</section><div class='import-actions'>");
    if let Some(token) = token {
        let new = report.new_timed + report.new_undated + report.reclassified_records;
        write!(body,"<form method='post' action='/history/import/apply' data-history-import><input type='hidden' name='csrf' value='{}'><input type='hidden' name='token' value='{}'><button class='btn btn-primary' {}>{}</button></form>",esc(&app.csrf),esc(token),if new == 0 {"disabled"} else {""},tr(lang,if new == 0 {"Nothing new to import"} else {"Confirm import"},if new == 0 {"无需导入"} else {"确认导入"})).unwrap();
    } else {
        write!(body,"<a class='btn btn-primary' href='/history'>{}</a><a class='btn btn-outline' href='/wrapped'>{}</a>",tr(lang,"View history","查看使用历史"),tr(lang,"View Wrapped","查看年报")).unwrap();
    }
    write!(body,"<a class='text-link' href='/history/import'>{}</a></div><p class='import-caption'>{}</p></div>",tr(lang,"Preview another file","预览其他文件"),tr(lang,if done {"Shell history may omit or merge executions; imported entries are observations, not a complete execution log."} else {"The preview lasts 10 minutes. Confirm imports this snapshot even if the file changes. Duplicate counts are checked again when saving."},if done {"Shell 历史可能遗漏或合并命令；导入条目是观察记录，不是完整执行日志。"} else {"预览保留10分钟；确认时使用这份快照，即使原文件发生变化。保存时会重新检查已有记录。"})).unwrap();
    body
}
pub(super) async fn preview(State(app): State<App>, Form(form): Form<PreviewForm>) -> ImportResult {
    check(&app, &form.csrf)?;
    let lang = app.core.locale(None).map_err(error)?;
    let selection = selection(&form).and_then(|(file, shell)| {
        Ok((
            file,
            shell,
            if form.aliases.trim().is_empty() {
                None
            } else {
                Some(path(&form.aliases)?)
            },
        ))
    });
    let prepared = match selection {
        Ok((file, shell, aliases)) => {
            let core = app.core.clone();
            tokio::task::spawn_blocking(move || {
                let plan = core.prepare_history_import(&file, shell, aliases.as_deref())?;
                let report = core.preview_history_import(&plan)?;
                anyhow::Ok((
                    plan,
                    report,
                    file.to_string_lossy().into_owned(),
                    shell.name().to_owned(),
                ))
            })
            .await
            .map_err(|e| error(e.into()))?
        }
        Err(e) => Err(e),
    };
    match prepared {
        Ok((plan, report, path, shell)) => {
            let token = app
                .imports
                .insert(plan, path.clone(), shell.clone())
                .map_err(error)?;
            response(
                &app,
                StatusCode::OK,
                report_body(&app, &lang, &path, &shell, &report, Some(&token), None),
            )
        }
        Err(e) => response(
            &app,
            StatusCode::BAD_REQUEST,
            form_body(&app, &lang, &form, Some(&e.to_string()))?,
        ),
    }
}
enum Applied {
    Done(ImportReport, String, String),
    Retry(ImportReport, String, String, String),
    Expired,
}
pub(super) async fn apply(State(app): State<App>, Form(form): Form<ApplyForm>) -> ImportResult {
    check(&app, &form.csrf)?;
    let lang = app.core.locale(None).map_err(error)?;
    let worker = app.clone();
    let token = form.token.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut pending = worker
            .imports
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("preview unavailable"))?;
        pending.retain(|_, p| p.created.elapsed() < PREVIEW_TTL);
        let Some(p) = pending.get(&token) else {
            return anyhow::Ok(Applied::Expired);
        };
        // Holding the lock serializes double submits; failed transactions retain the plan.
        match worker.core.apply_history_import(&p.plan) {
            Ok(report) => {
                let path = p.path.clone();
                let shell = p.shell.clone();
                pending.remove(&token);
                let _ = worker.core.complete_history_import_onboarding();
                Ok(Applied::Done(report, path, shell))
            }
            Err(e) => Ok(Applied::Retry(
                worker.core.preview_history_import(&p.plan)?,
                p.path.clone(),
                p.shell.clone(),
                e.to_string(),
            )),
        }
    })
    .await
    .map_err(|e| error(e.into()))?
    .map_err(error)?;
    match result {
        Applied::Done(report, path, shell) => response(
            &app,
            StatusCode::OK,
            report_body(&app, &lang, &path, &shell, &report, None, None),
        ),
        Applied::Retry(report, path, shell, e) => response(
            &app,
            StatusCode::SERVICE_UNAVAILABLE,
            report_body(
                &app,
                &lang,
                &path,
                &shell,
                &report,
                Some(&form.token),
                Some(&e),
            ),
        ),
        Applied::Expired => response(
            &app,
            StatusCode::GONE,
            format!(
                "{}<div class='import-message' role='alert'><p>{}</p></div><a class='btn btn-primary' href='/history/import'>{}</a></div>",
                intro(
                    &lang,
                    tr(&lang, "Preview no longer available", "预览已失效"),
                    tr(
                        &lang,
                        "Nothing was imported by this request.",
                        "本次请求没有导入任何记录。"
                    )
                ),
                tr(
                    &lang,
                    "The preview expired, was already used, or the app restarted. Preview the file again to continue.",
                    "预览已超时、已使用或程序已重启，请重新预览文件后继续。"
                ),
                tr(&lang, "Preview again", "重新预览")
            ),
        ),
    }
}

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/history/import", get(choose))
        .route("/history/import/preview", post(preview))
        .route("/history/import/apply", post(apply))
        .layer(middleware::from_fn(no_cache))
}
async fn no_cache(request: Request<axum::body::Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}
pub(super) fn entry(lang: &str) -> String {
    format!(
        "<div class='history-import-entry'><a class='btn btn-primary' href='/history/import'>{}</a></div>",
        tr(lang, "Import existing history", "导入旧历史")
    )
}
#[cfg(test)]
#[path = "history_import_tests.rs"]
mod tests;
