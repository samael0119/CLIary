# 历史导入引导（2026-10-08）

用户要求安装／首次运行提供历史导入交互，并追加 Web 预览／导入按钮。本阶段完成 CLI／安装入口与共享 Core，Web 入口在依赖分支继续实现。

- 分支 `feature/2026-10-08-history-onboarding`，基于 `d273115`；提交可用 `git log -1 feature/2026-10-08-history-onboarding` 查询。
- 首次交互运行依次提供安装扫描、未来采集、旧历史导入。旧历史导入状态独立于扫描标志，旧用户也会获得一次提示；跳过有记录，EOF／未完成可以重试。`setup history` 可随时重开。
- Common／custom Bash、Zsh、Fish 路径候选只读文件 metadata，不读内容；尊重当前 Shell 的导出 HISTFILE、ZDOTDIR 和 Fish 的 XDG_DATA_HOME。路径去重，目录排除；自定义路径与多份文件可逐份选择。
- 明确选择后才预览，只展示程序名、数量和可靠日期，不展示参数。别名表可选且不加载 Shell 配置。每份文件应用默认否；无日期观察不会补造日期或进入年报。
- Core 引入 `HistoryImportPlan`：准备时仅保留程序／时间／次数／别名目标，丢弃原始命令；确认时使用同一份解析快照，不重读变更过的文件，数据库去重情况则按提交时重新评估。没有新数据库版本。
- 非交互／JSON 命令不触发历史引导；显式非交互 `setup history` 返回可操作错误。安装器调用同一引导或打印后续入口，支持 `--no-import-history`；`--no-history` 连同引导一并跳过并记住。
- 修正旧 yes/no 提问遇到 EOF 时采用肯定默认的问题；EOF 不启用采集或确认导入。

验证：workspace 60 项测试、Clippy、格式及 diff 检查、安装脚本语法检查通过。新测试覆盖候选路径、无内容读取的跳过／EOF、预览不应用、别名应用／重复次数、文件改写或删除后仅提交已预览快照。实际 TTY 和模拟安装验证使用临时目录及替代 curl／打包二进制，无外网发布或用户 rc 修改。

本阶段不自动推送／合并 main；用户可使用已构建的 `target/debug/cliary setup history` 试用。
