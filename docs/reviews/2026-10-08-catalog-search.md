# 标签与搜索改进交付（2026-10-08）

## 范围与实现

- 55 个工具全部补充中英文任务关键词，保留既有用途、安装方式和事实；新增 28 个本地化用途标签。
- Core 统一按名称／别名／可执行文件优先、字段权重、词频稀缺程度和最长匹配短语排序；中文短语与英文基础复数归一化，降低泛类目和虚词造成的噪声。
- 复用现有 Catalog schema，保留 FTS5 数据给旧消费者；新排名直接读取 Catalog 字段，当前 55 工具规模无需额外索引服务。
- 内置目录增加来源 revision 和内容摘要，已识别未修改旧目录自动刷新。已同步的版本 >1 和自定义目录保留；显式 `sync --bundled` 可离线替换目录，不修改 user.db。
- 未接入模型、未增加运行时依赖、未修改界面结构或 GitHub Actions。

## 验证

- 全 workspace 69 项测试通过，含新增 3 项搜索质量／全量身份优先／目录刷新与数据保全测试。
- Clippy 全 targets `-D warnings`、格式、diff、55 个 YAML schema 及 Catalog 引用检查通过。
- 真实 CLI 在临时目录验证 61 个固定查询；已有 54 工具和 55 工具的初始目录副本升级通过，个人备注保全验证通过。未修改真实用户数据库或导入历史。
- 实际 Web HTTP 检索验证目录空间、Git 提交及 JSON 字段查询，新用途标签可在工具详情正常显示。测试服务已关闭。
- 隔离 CLI 全流程的本机调试版查询中位数约 20 ms，包含进程启动和数据库工作；不代表其他设备／更大目录的性能承诺。

## 固定样例的前后对照

基线为 main `c9a1194` 的二进制，查询与预期相关工具集合在改动前冻结。Top-3 命中表示前三项至少有一个预期相关工具，并不表示前三项全部准确。这是开发回归集，未做真实用户采样或模型对照；不能将结果推广为任意自然语言的准确率。

| 查询类型 | 基线 | 改进后 |
| --- | --- | --- |
| 名称／别名等 Top-1 | 5/8 | 8/8 |
| 任务描述 Top-3 | 38/47（80.9%） | 47/47（100%） |
| 空白／无关输入正确空结果 | 4/6 | 6/6 |

另遍历全部工具 ID、显示名、别名、可执行文件，验证 Top-1 身份无误。

| 查询 | 基线前三项 | 改进后三项 |
| --- | --- | --- |
| 磁盘空间 | dua、dust、gdu | dua、dust、gdu |
| 想知道电脑里哪个文件夹占地方 | ag、bat、cat | du、dua、dust |
| 目录太大，想找出占空间的文件 | du、ls、bat | du、dua、dust |
| 交互式分析磁盘占用 | dua、ncdu、gdu | dua、gdu、ncdu |
| 查找文件 | fd、find、ag | fd、find |
| 按文件名搜索目录 | fd、ag、du | fd、find、fzf |
| 忘了文件名，只记得一小部分 | ag、bat、cat | fzf、fd、find |
| 在源码里查一段文字 | 无结果 | ag、rg、grep |
| 递归搜索文本内容 | grep、rg、cat | rg、ag、grep |
| 看一下仓库过去的提交 | 无结果 | git、lazygit、tig |
| 查看 Git 提交历史 | delta、gh、git | git、lazygit、tig |
| 在终端交互管理 Git | gh、tig、lazygit | tig、lazygit、git |
| 查看代码修改差异 | delta、ag、bat | delta、git、lazygit |
| 把 JSON 排版成容易看的样子 | jq、bottom、btop | jq、httpie、xh |
| 从 JSON 中提取字段 | jq、wget、yt-dlp | jq、yq、httpie |
| 编辑 YAML 配置里的字段 | yq、nvim、vim | yq、awk、jq |
| 替换文本中的字符串 | sed、awk、bat | sed、top、ag |
| 按列处理文本数据 | awk、yq、jq | awk |
| 带语法高亮看文件 | bat、delta、du | bat、delta、du |
| 发送 HTTP 请求测试接口 | curl、httpie、xh | httpie、curl、xh |
| 下载远程文件 | wget、curl、ag | aria2、wget、curl |
| 远程登录服务器 | curl、gh、httpie | ssh |
| 查看进程和 CPU 内存占用 | du、watch、ncdu | bottom、btop、htop |
| 保持终端会话，分屏工作 | tmux、zellij、bat | tmux、zellij、k9s |
| 文件改变时自动执行命令 | entr、watch、ag | entr、watch |
| 测量命令运行耗时 | entr、watch、docker | hyperfine |
| 编译 TypeScript | typescript、openssl、gpg | typescript |
| 管理 Kubernetes 集群 | kubectl、k9s、podman | k9s、kubectl、lazygit |
| 连接 PostgreSQL 数据库 | psql、sqlite3、redis-cli | psql、redis-cli、ssh |
| 打开 SQLite 数据库执行 SQL | sqlite3、psql、redis-cli | sqlite3、psql、entr |
| 访问 Redis 键值数据库 | redis-cli、du、yq | redis-cli |
| 转换视频格式 | ffmpeg、sed、yt-dlp | ffmpeg |
| 下载网站视频 | yt-dlp、aria2、curl | yt-dlp、aria2、curl |
| 加密和签名文件 | gpg、ag、bat | gpg、openssl、ssh |
| 处理 TLS 证书 | openssl、jq、yq | openssl |
| which folders consume disk space | du、dua、dust | du、dua、dust |
| find files by name | find、ag、fd | fd、find、fzf |
| search source code for text | rg、ag、fzf | ag、rg、grep |
| inspect earlier repository commits | 无结果 | git、tig、lazygit |
| make JSON readable | jq | jq、httpie、xh |
| extract fields from yaml | jq、yq、yt-dlp | yq、jq、yt-dlp |
| monitor memory and CPU usage | bottom、du、dua | bottom、btop、htop |
| split terminal into panes | bottom、btop、dust | tmux、zellij |
| build and run containers | docker、entr、podman | docker、podman |
| list directory contents | ls、cat、du | eza、ls |
| read a file page by page | delta、less、find | less、bat、cat |
| edit text in terminal | nvim、vim、awk | nvim、vim、k9s |

## 使用与边界

本地工作分支：`codex/2026-10-08-catalog-search`。当前 `target/debug/cliary` 已构建，可直接 `search "任务描述"` 或 `web` 试用。

检索依赖已整理的词与资料，不理解任意句式、否定条件或“最适合”等主观意图。模糊任务仍可能召回部分相关工具。语言覆盖主要为中英文。目录来源识别采用完整内容摘要，未知旧版或自定义内容不会自动覆盖；可显式离线刷新。

英文复数归一化是小范围规则，不是完整词形分析。当前每次搜索构造字段词典；未来目录明显扩大时应针对实际延迟评估缓存或 FTS 字段索引，保持同一回归集验证。
