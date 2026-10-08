# Web Shell 历史导入设计复核 · 2026-10-08

本记录按 Impeccable `reference/document.md` 与 documenter 的普通扩展规则，对照产品约束、现行实现及本轮验收证据整理。范围为桌面 Web 使用历史中的显式导入流程，模式为 Operate：让用户选择文件、核对结果并完成本地导入。

交付分支为 `codex/2026-10-08-web-history-import`；技术验证详见[Web 历史导入复核记录](2026-10-08-web-history-import.md)。

## 既有系统与本轮边界

[PRODUCT.md](../../PRODUCT.md) 规定 CLIary 面向 Linux／macOS 电脑用户，沿用用户已采用的 Gemini 绿色桌面工作台：固定侧栏、清楚的标题层级、中英文、明暗主题、内容分区和数据表格。用户没有要求强制手机适配。本轮在该身份内扩展操作流程，没有建立新视觉世界、全站组件体系或装饰性动画。

当前根目录没有 `DESIGN.md`；既有视觉依据来自产品约束、现行工作台实现及[上一轮 M2 设计记录](2026-10-08-m2-design.md)。本轮仅写此复核记录，没有创建或改写 `DESIGN.md`、`.impeccable/design.json`，也没有将已有漂移固化为新的全局规则。

## 新增流程与状态

使用历史页顶部提供导入入口；在线年报也提供可进入导入流程的入口。流程在既有工作台内完成：

1. 从检测到的文件中选择，或展开自定义路径与 Shell 格式；可选填写别名快照路径。检测阶段只检查文件位置，用户点击预览时才读取内容。
2. 预览所选文件与格式，逐项核对新增有日期记录、新增无日期观察、已有记录、跳过条目及重新归类数量；提供可靠日期范围、程序名示例和别名归属。
3. 用户显式确认后写入本地工作区。预览有效期为 10 分钟，确认使用已准备的快照；保存时再次检查重复记录。没有新增或重新归类内容时，确认按钮禁用并显示“无需导入”／“Nothing new to import”。
4. 成功页提供查看使用历史、查看年报和预览其他文件的路径。预览失败保留填写内容并提示路径、读取权限、格式与文件限制；导入失败保留可重试的预览，预览失效可重新选择文件。

提交时仅针对导入表单显示“正在处理…”／“Processing…”、禁用提交按钮并设置 `aria-busy`；浏览器 `pageshow` 恢复原按钮文字与可用状态，避免返回页面后一直忙碌。已有语言与主题入口继续控制整个工作台。

文案保留产品事实边界：命令参数被丢弃；无日期观察单独保留，不进入年度或时间统计；当前别名快照可能与旧定义不同；Shell 历史可能遗漏或合并执行，因此不宣称完整执行日志。

## 局部视觉实现

以下为本轮源码中实际使用的局部样式与语义，不作为新全局 token 或命名规则。

| 项目 | 延续及新增实现 |
| --- | --- |
| 配色与可读性 | 延续既有绿色、表面、边线及明暗主题变量。导入区与入口的浅色说明文字局部使用 `#526c5f`；新增主按钮使用 `#047857` 底色与白字，悬停为 `#065f46`。这些颜色沿用上一轮已确认的小字及按钮对比度修正。 |
| 字体与层级 | 延续既有正文与标题字族；新增按钮、字段标签及三级标题为 14px，分区标题为 18px，说明文字为 13px。路径和程序名以既有代码字体帮助核对；标题、说明及数量不使用代码字体。数量使用等宽数字。 |
| 布局与密度 | 导入内容最大宽度 920px，表单最大宽度 760px；路径与格式并排，字段间距 20px，局部分区以顶部 24px 留白和既有细分隔线区分。固定侧栏及工作台外壳保持既有布局，已验收桌面宽度为 1280px 和 1440px。 |
| 表单与核对 | 延续表单 8px 圆角、表面色、边线和焦点反馈。自定义路径与别名帮助使用可展开区域；数量以 `dl` 表达标签、说明与值，避免将操作核对信息铺成新的指标卡体系。 |
| 交互反馈 | 新按钮仅有背景色过渡，悬停不位移；无需导入以禁用与明确文字说明，失败区域使用 `role="alert"`。没有新增装饰性动画。 |

## 证据与验收交接

源码范围为 [history_import.rs](../../crates/cliary-web/src/history_import.rs)、[history_import_tests.rs](../../crates/cliary-web/src/history_import_tests.rs)、[lib.rs](../../crates/cliary-web/src/lib.rs) 的使用历史入口、[wrapped.rs](../../crates/cliary-web/src/wrapped.rs) 的在线入口、[style.css](../../crates/cliary-web/static/style.css) 末尾的导入局部规则，以及 [page.html](../../crates/cliary-web/templates/page.html) 的提交忙碌与 `pageshow` 恢复处理。

本轮验收交接已实际打开确认以下截图；文档整理未重复运行浏览器、测试或 detector。

| 本地截图 | 覆盖内容 |
| --- | --- |
| [form-zh-dark.jpg](../../.impeccable/review/history-import/form-zh-dark.jpg) | 中文暗色文件选择与自定义表单。 |
| [preview-zh-dark.jpg](../../.impeccable/review/history-import/preview-zh-dark.jpg) | 中文暗色预览与确认。 |
| [preview-zh-light.jpg](../../.impeccable/review/history-import/preview-zh-light.jpg) | 中文浅色数量、日期、程序名与别名核对。 |
| [success-zh-light.jpg](../../.impeccable/review/history-import/success-zh-light.jpg) | 中文浅色成功结果及后续入口。 |
| [preview-en-light.jpg](../../.impeccable/review/history-import/preview-en-light.jpg) | 英文浅色预览及重复导入的禁用状态。 |
| [preview-en-dark.jpg](../../.impeccable/review/history-import/preview-en-dark.jpg) | 英文暗色预览。 |
| [error-en-dark.jpg](../../.impeccable/review/history-import/error-en-dark.jpg) | 英文暗色预览错误与恢复路径。 |
| [desktop-1440.jpg](../../.impeccable/review/history-import/desktop-1440.jpg) | 1440px 桌面工作台布局。 |

前七张为 1280px 桌面全页截图，另有 1440px 全页截图。截图与 detector 输出位于本地 `.impeccable/review/history-import/`，作为验收材料不提交 Git；重新克隆仓库不会包含这些图片。

模拟数据位于隔离的 `/tmp` 工作区，未读取或导入真实个人历史。2024 演示先新增 3 条有日期记录与 1 条无日期观察；2025 最终新增 3 条有日期记录，无日期观察已保留，因此新增为 0。英文预览的禁用按钮对应重复数据，不能解读为导入操作不可用。

实施验证交接为 workspace 66 项测试及 Clippy 通过。真实 HTTP smoke 已通过：`no-store`、CSRF、loopback Host 检查、预览不写入、文件变化后仍应用快照、重放返回 410，以及可编辑的 400 错误恢复页。独立 finish reviewer 的结论为 **ship，无需实质修正**。知识图谱项目 `home-hyh-Projects-CLIary` 索引为 ready；新增模块覆盖为 `metadata_match`，模板相关范围为 partial，已通过直接读取补足。

## 检测边界与既有遗留

Detector 仅运行一次，[detector.json](../../.impeccable/review/history-import/detector.json) 记录两项既有提醒：`style.css:479` 的渐变文字和 `style.css:1172` 的宽度过渡。对应旧规则未在本轮修改，未增加 ignore，也未扩展任务去修复它们；本记录不将其认可为后续界面的设计规则。

静态模板中的 `/assets/style.css` 无法由本地文件解析直接关联，但实际样式表已独立扫描；因此 detector 结果不视为模板与 CSS 关联的完整验证。中英文、明暗主题与桌面页面的视觉结果由上述实际打开的截图支撑。全局设计文档缺失和既有样式漂移按普通扩展边界报告，未在本轮补建或修复。
