# v0.1 发布前修复与合并检查（2026-10-08）

## 范围与结论

用户授权修复前一轮评估中的扫描超时、共享按钮对比度、私有仓库安装／同步下载和安装包验证，并合并当前搜索改进。基于 `198f00e` 搜索分支建立 `codex/2026-10-08-v01-release-fixes`；此前 main 为 `c9a1194`。本轮没有新增模型依赖、Agent 命令记录或处理 GitHub Actions，也不发布 tag／Release。

## 改动

- 包管理器清单读取使用独立 Unix 进程组、非阻塞输出、3 秒总读取截止时间及 8 MiB 上限；错误／超时清理同组子进程，取消可能永远等待的读取线程 join。目录扫描仍保留程序，失败清单的归属可能显示未识别。3 秒按清单命令计算，不是整个扫描的承诺。
- 主按钮共享默认背景 `#047857`、悬停 `#065f46`、白字，明暗主题均使用可读颜色；通用悬停不再覆盖 inline-form。
- 安装器新增 `--github`，由已登录的 GitHub CLI 下载私有 Release；先固定 tag 再下载同一次 Release 的程序包和 SHA256SUMS，不保存或转发认证信息。
- 安装器新增 `--from DIRECTORY` 离线安装、可选 `--tag` 指定版本。校验后原子替换二进制，支持旧 Web 程序仍在运行时升级。无有效校验和、损坏或符号链接程序包均不会替换旧程序。
- 新增 `cliary sync --from DIRECTORY`，导入已经下载的 manifest.json 与 catalog.db.zst，复用现有校验及替换流程；与 --url／--bundled 互斥。读取文件大小有上限，Unix 非阻塞打开可拒绝 FIFO，不修改 user.db。
- README 记录私有／离线流程，纠正「main 合并就一定发布」的假设。

私有 Catalog 使用 `gh release download` 后本地导入，不在 Core 中新增凭证存储或网络认证层。普通 HTTPS 同步仍用于公开资产；旧的匿名 GitHub 下载 URL 无法访问私有 Release。

## 验证

| 检查 | 结果 |
| --- | --- |
| cargo test --workspace --locked | 73 个测试通过；包含读取截止时间、继承管道、输出大小／UTF-8，以及离线 Catalog 校验／数据保留 |
| cargo clippy --workspace --all-targets --locked -- -D warnings | 通过 |
| cargo fmt --all -- --check；git diff --check | 通过 |
| python3 scripts/validate_catalog.py | 55 条工具通过 |
| cargo build --release --locked -p cliary-cli | 本机 Linux x86_64 构建通过 |
| CLIARY_TEST_BINARY=target/release/cliary python3 scripts/test_install.py | 6 项通过：安装／升级保留收藏备注历史、坏校验保留原程序、符号链接拒绝、gh 固定版本下载、下载失败保留原程序、选项错误拒绝 |
| Release 二进制完整扫描挂起复现 | 模拟 cargo 主进程退出、后代保留 stdout，整体 scan 约 3.159 秒完成；后代延迟写标记未发生，进程组已清理 |
| 原生 tar.gz＋SHA256SUMS 安装包 | 隔离目录下旧 debug Web 二进制运行时升级到 release；旧进程存活、新文件内容正确、收藏备注历史一致；后续 sync --from 更新 Catalog v2 不改变上述个人数据 |
| CLI 实际操作 | sync 选项冲突返回 2；2023 年报 HTML 导出成功且 Unix 权限私有 |

安装测试首次开发运行暴露了缺少选项值的解析问题，已修复；修改脚本与当时运行中的测试发生读取竞态，之后在稳定源码上重新用 release 二进制运行全部 6 项并通过。

测试均使用隔离目录及人工历史，没有导入／改动用户个人历史或 Shell 启动文件。安装包、临时数据库和年报验证样例在 `/tmp/cliary-v01-release-check/`，不提交临时数据或二进制。

## UI 证据与审查

在原生 Codex IAB 桌面视口 1280×720 检查首页、搜索、扫描和比较，覆盖中文浅色与英文深色。`.impeccable/review/v01-buttons/` 保存截图和 computed evidence；有滚动条页面接口返回原生 JPEG 1265×712，无滚动条页面为1280×720，未手工裁剪。

白字默认背景对比度 5.484:1，悬停颜色 7.684:1；默认从3.77:1改善到超过4.5:1。默认状态来自浏览器计算值，悬停状态来自加载后的 CSS 规则／色值计算，未声称自动化实际鼠标悬停。键盘 Tab 可聚焦搜索按钮，2px 焦点圈可见。用户明确无需强制手机适配，本轮只验证电脑布局。

Impeccable finish reviewer 的限定结论：`disposition: ship`；只覆盖本轮按钮修复。Documenter 确认无须为局部修复新增 DESIGN.md，保留既有系统。

检测器只运行一次，报告既有渐变文字（482行）和 width 动画（1175行）。这两处相对起点没有修改，不属于此次按钮修复，不扩大范围处理或新增忽略规则；现有设计债务仍保留。

## 发布限制

- 只在本机 Linux x86_64 实际构建／安装验证；macOS、ARM64 仍须对应环境验收。
- 私有仓库查询尚无 Release 资产，gh 下载分支使用受控替身验证参数、固定 tag 和错误处理；不能宣称已经完成真实私有 Release 下载的端到端验收。正式发布资产后需要按 README 试一次。
- 当前结果是可测试的 v0.1 候选代码；本轮合并不是正式发布。GitHub Actions 继续按用户要求暂缓。
