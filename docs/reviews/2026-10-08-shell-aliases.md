# Shell 别名识别（2026-10-08）

需求：`gst` 是用户的 Zsh Git 插件别名，实际执行 `git status`，应计入 Git 使用统计。

## 实现

- 分支：`feature/2026-10-08-shell-alias-resolution`，基于 M2 展示／导出分支 `7091c72`；本轮提交用 `git log -1 feature/2026-10-08-shell-alias-resolution` 查询。未自动推送或合并 main。
- Zsh 实时采集使用 `preexec` 的第三个参数，即 Zsh 给出的完整别名展开结果；只提取两个程序名传给 recorder，保留输入名作为依据，按展开后的 Catalog 工具 ID 归类。`gst → g → git status` 也可归到 Git。
- `import-history --aliases-file PATH` 支持用户显式导出的 Zsh `alias -L`／Bash `alias -p` 普通别名表。只解析数据，不 source 文件、不加载插件或用户 rc、不执行别名内容。文件限 UTF-8 普通文件、4 MiB。
- 每条定义只解析一次，立即丢弃参数；历史中的每次解析只做至多 16 步名称查找，避免历史重复调用导致反复解析长定义。
- 预览新增最多 20 项 `alias_resolutions`（名称与目标程序，不含参数），以及 `reclassified_records`。应用时可归类当前选定历史中同名、同时间、同本机的未识别导入记录；不改日期、原始程序名、去重 key，不新增重复事件。不覆盖已有工具 ID 或实时采集记录。
- 无数据库版本变更。统计及年报按已有工具 ID 合并；`history gst` 仍可核对输入名。CLI、Core、README 与回归测试已更新。

## 验证

- 全 workspace 55 项测试通过，含别名链／循环／危险展开／quote 和 wrapper 绕过、导入预览、已有记录归类、去重、已归类 ID 保全，以及 Zsh hook 不泄露参数、不误认函数的实际 Shell 测试。
- 消除重复定义解析后，9 项历史导入测试与别名解析测试再次通过；Clippy 全目标 `-D warnings`、fmt、diff 检查及最终构建通过。
- 隔离 CLI 演示：先导入 4 条未识别的 `gst`／`gcl`／`ll`；别名预览归类 4 条、未写入；应用后仍为 4 条，年报变为 Git 3 条、ls 1 条；再次应用新增与归类均为 0。
- 真正交互式 `zsh -d -f -i` 使用临时 hook 和合成别名，调用 `gst → g → git status`，最终保存 `executable=gst, tool_id=git, source=capture`。没有加载用户 rc 或读取个人历史；测试进程已退出。
- 示例与日志在 `/tmp/cliary-alias-review-*`；测试不写入用户真实数据。

## 试用

在已加载插件的日常 Zsh 中导出别名表，先查看预览再应用：

```sh
cliary_aliases_file="$(mktemp)"
builtin alias -L > "$cliary_aliases_file"
./target/debug/cliary import-history --shell zsh --file "$HISTFILE" --aliases-file "$cliary_aliases_file"
./target/debug/cliary import-history --shell zsh --file "$HISTFILE" --aliases-file "$cliary_aliases_file" --apply
rm -- "$cliary_aliases_file"
./target/debug/cliary history git
./target/debug/cliary wrapped
```

实时采集需更新旧 hook：`./target/debug/cliary setup shell --shell zsh --enable`，然后打开新终端。开发代理未代用户修改 rc。

## 边界

- 当前别名表是当前配置的依据，不能证明过去每次执行的配置；变更过定义时应选取适当的历史快照。
- 只统计工具层级，不分别排名 `git status`／`git checkout`；不保存子命令参数。
- 函数、全局／后缀别名、循环、复杂展开不猜测。若同一名称在选定历史中混有 quoted、escaped、显式路径或 wrapper 参数形式，保守保留原归类。
- 尚未收录的目标工具不伪造 Catalog ID；无日期观察保留输入名且不进入年报。Bash／Fish 的实时 hook 仍保留原有行为，历史中的普通 Bash 别名可通过显式快照识别。
- 导出的临时文件包含别名定义，可能有私有参数；`mktemp` 创建私有文件，用完删除。SQLite 和预览不保存这些定义或参数。

机制依据：[Zsh preexec](https://zsh.sourceforge.io/Doc/Release/Functions.html)、[Zsh alias -L](https://zsh.sourceforge.io/Doc/Release/Shell-Builtin-Commands.html)。
