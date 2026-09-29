# CLIary PRD

> **Discover your tools. Remember how you use them.**  
> **发现你的工具，也记住你如何使用它们。**

---

# 1. 产品名称

## 1.1 名称

**CLIary**

命令行程序名称：

```text
cliary
```

技术标识统一使用小写：

```text
cliary
cliary-core
cliary-cli
cliary-web
CLIARY_*
```

品牌展示统一使用：

```text
CLIary
```

---

## 1.2 命名来源

`CLIary` 由：

```text
CLI + Diary
```

组合而来。

其中：

- **CLI** 代表 Command-Line Interface
- **Diary** 代表记录、历史与长期积累

CLIary 不只是保存“有哪些命令行工具”，还记录：

- 用户发现过哪些工具
- 安装过哪些工具
- 收藏过哪些工具
- 经常使用哪些工具
- 某个工具第一次什么时候使用
- 使用习惯如何随时间发生变化

因此名称同时体现 CLIary 的两个核心价值：

> **CLI 工具目录**

以及：

> **属于开发者自己的 CLI 使用日记**

年度回顾功能对应：

```text
CLIary Wrapped
```

---

# 2. 产品概述

CLIary 是一个面向 Linux 开发者的：

> **CLI 工具发现、收藏、比较、管理与使用历史平台。**

CLIary 解决以下问题：

> 曾经安装或使用过很多优秀的命令行工具，但时间久了忘记它们叫什么、做什么、怎么安装。

以及：

> 使用 CLI 很多年，却很难回顾自己真正使用过哪些工具，以及自己的工具习惯发生了怎样的变化。

CLIary 提供：

- 本机 CLI 工具识别
- 公共 CLI 工具 Catalog
- 工具搜索
- 分类与标签
- 同类工具发现
- 同类工具对比
- 安装状态检测
- 安装方式展示
- 收藏
- Notes
- CLI 工具使用历史
- 使用统计
- CLIary Wrapped 年度回顾
- CLI
- 本地 Web UI
- 中英文支持
- GitHub Catalog 同步
- 可选本地智能搜索

CLIary：

- 不要求登录
- 不依赖中央服务器
- 不需要 SaaS 后端
- 用户数据默认完全保存在本机

---

# 3. 产品核心

CLIary 由三个核心模块组成：

## 3.1 Discover

帮助用户发现工具。

包括：

- Search
- Categories
- Tags
- Similar Tools
- Compare
- Recommendation
- Install Information

解决：

> “有没有一个工具可以做这个？”

---

## 3.2 Shelf

帮助用户管理自己的工具集合。

包括：

- Installed Tools
- Favorites
- Notes
- Custom Tags
- Installation Source
- Local Status

解决：

> “我以前是不是装过这个？”

以及：

> “这个工具当初为什么值得留下？”

---

## 3.3 History

记录用户 CLI 工具使用轨迹。

包括：

- First Used
- Last Used
- Run Count
- Active Days
- Daily Activity
- Monthly Activity
- Category Trends
- CLIary Wrapped

解决：

> “这些年我到底是怎么使用 CLI 的？”

---

# 4. 产品目标

CLIary 应能够帮助用户快速回答：

- 我机器上安装了哪些 CLI 工具？
- `ncdu` 是干什么的？
- 我以前用哪个工具查看磁盘占用？
- 有没有类似 `ncdu` 的工具？
- `ncdu`、`gdu`、`dust` 有什么区别？
- 哪一个已经安装？
- 未安装工具怎么安装？
- 有没有适合某个需求的工具？
- 我最近最常用什么 CLI 工具？
- 某个工具多久没用了？
- 我今年第一次用了哪些工具？
- 我今年使用最多的是哪些工具？
- 我的 CLI 使用习惯今年发生了什么变化？

---

# 5. 产品原则

## 5.1 Local First

核心功能必须能够离线运行。

本地存储：

```text
Catalog
User Data
History
Favorites
Notes
Statistics
```

网络只主要用于：

```text
GitHub Catalog Sync
```

---

## 5.2 One Core, Multiple Interfaces

CLI 和 Web UI 共享同一个 Rust Core。

```text
               cliary-core
                    │
          ┌─────────┴─────────┐
          │                   │
          ▼                   ▼
     cliary-cli          cliary-web
```

禁止 CLI 和 Web 分别实现独立业务逻辑。

---

## 5.3 No Required Server

CLIary 不建设必须依赖的中央服务器。

公共数据链路：

```text
GitHub Repository
       ↓
GitHub Actions
       ↓
GitHub Releases
       ↓
CLIary Sync
       ↓
Local SQLite
```

---

## 5.4 AI Is Optional

AI 属于增强功能，而不是 CLIary 的基础依赖。

关闭 AI 后：

- Search
- Browse
- Compare
- Installed Scan
- History
- Statistics
- Wrapped

都必须正常工作。

---

## 5.5 Structured Facts First

工具事实必须来自结构化 Catalog。

例如：

```text
安装方式
License
Platform
TUI support
Repository
Feature
```

禁止将 LLM 参数知识作为事实来源。

---

## 5.6 Privacy by Default

默认历史记录仅保存工具级数据。

例如用户执行：

```bash
curl "https://example.com/api?token=SECRET"
```

CLIary 默认只记录：

```text
tool: curl
timestamp: ...
machine_id: ...
```

默认不记录：

```text
URL
arguments
token
headers
password
command contents
```

---

## 5.7 Internationalization First

V0.1 至少支持：

```text
English
简体中文
```

i18n 必须从第一版数据结构开始设计。

---

# 6. 目标用户

主要用户：

- Linux 开发者
- 后端开发者
- DevOps
- CLI 爱好者
- HomeLab 用户
- 开源工具爱好者

典型用户：

- 安装过大量 CLI 工具
- 经常从 GitHub 发现新工具
- 过几个月后容易忘记工具名
- 经常搜索某个命令的现代替代品
- 同一类工具可能安装过多个
- 希望长期记录自己的 CLI 使用习惯

---

# 7. 核心使用场景

## 7.1 找回忘记名称的工具

用户只记得：

> 有个终端磁盘分析工具。

执行：

```bash
cliary search "disk usage"
```

或者：

```bash
cliary search "磁盘占用"
```

返回：

```text
ncdu     ✓ Installed
gdu
dust
dua
```

---

## 7.2 查看已安装工具

```bash
cliary installed
```

示例：

```text
NAME       CATEGORY       VERSION     SOURCE

ncdu       Filesystem     2.9         apt
rg         Search         14.1.1      apt
bat        Text           0.25.0      apt
dust       Filesystem     1.1.2       cargo
```

---

# 8. Catalog

公共 Catalog 保存 CLI 工具结构化资料。

示例：

```yaml
id: ncdu
name: ncdu

description:
  en: Interactive disk usage analyzer.
  zh-CN: 交互式磁盘占用分析工具。

categories:
  - filesystem

tags:
  - disk
  - storage
  - tui

homepage: https://dev.yorhel.nl/ncdu

platforms:
  - linux
  - macos

features:
  tui: true
  interactive: true
  delete_files: true

install:
  apt:
    package: ncdu

  pacman:
    package: ncdu

  brew:
    package: ncdu

similar:
  - gdu
  - dust
  - dua
```

---

# 9. Catalog 多语言设计

## 9.1 语言无关字段

以下字段不进行本地化：

```text
id
name
aliases
repository
homepage
license
platforms
install
features
executables
similar
```

---

## 9.2 本地化字段

以下内容允许提供多个 Locale：

```text
description
summary
use_cases
feature descriptions
category display name
tag display name
comparison explanation
```

例如：

```yaml
summary:
  en: Fast terminal disk usage analyzer.
  zh-CN: 快速的终端磁盘占用分析工具。
```

---

# 10. Categories

内部 Category 使用稳定 ID。

例如：

```yaml
id: filesystem

name:
  en: Filesystem
  zh-CN: 文件系统
```

其他分类：

```text
network
development
git
database
media
monitoring
shell
security
http
text
filesystem
```

---

# 11. Tags

Tag 同样采用语言无关 ID：

```yaml
id: disk

name:
  en: Disk
  zh-CN: 磁盘
```

搜索时：

```text
disk
磁盘
磁盘空间
storage
```

均可关联相关工具。

---

# 12. GitHub Catalog

建议：

```text
catalog/
├── schema/
│   └── tool.schema.json
│
├── i18n/
│   ├── categories.yaml
│   └── tags.yaml
│
└── tools/
    ├── filesystem/
    ├── network/
    ├── development/
    ├── git/
    ├── media/
    └── ...
```

---

# 13. 社区贡献

新增工具流程：

```text
Fork
 ↓
新增 YAML
 ↓
Pull Request
 ↓
CI Validation
 ↓
Merge
 ↓
Build Catalog
 ↓
GitHub Release
```

贡献者可以只提供英文资料。

中文翻译可以通过后续 PR 完善。

默认 fallback：

```text
English
```

---

# 14. CI Validation

GitHub Actions 校验：

- Schema
- Tool ID 唯一性
- Category
- Similar Tool 引用
- URL
- Install definition
- Alias 冲突
- Locale
- 必填字段
- English Description

---

# 15. Catalog 发布

Actions 自动生成：

```text
manifest.json
catalog.db.zst
catalog.json.zst
```

Manifest 示例：

```json
{
  "version": 42,
  "generated_at": "2026-09-29T10:00:00Z",
  "tool_count": 1328,
  "languages": [
    "en",
    "zh-CN"
  ]
}
```

同步：

```bash
cliary sync
```

---

# 16. CLIary 本地目录规范

## 配置

```text
~/.config/cliary/
```

例如：

```text
~/.config/cliary/config.toml
```

---

## 数据

```text
~/.local/share/cliary/
```

例如：

```text
~/.local/share/cliary/catalog.db
~/.local/share/cliary/user.db
```

---

## Cache

```text
~/.cache/cliary/
```

---

## 环境变量

统一：

```text
CLIARY_*
```

例如未来可使用：

```text
CLIARY_LANG
CLIARY_DATA_DIR
CLIARY_CONFIG_DIR
```

---

# 17. 数据库拆分

## catalog.db

公共数据：

```text
Tools
Categories
Tags
Descriptions
Install Methods
Features
Similar Tools
Search Index
```

允许 Catalog 更新时整体替换。

---

## user.db

用户数据：

```text
Favorites
Notes
Custom Tags
Installed State
History
Statistics
Machines
Settings
```

Catalog 更新不得覆盖 `user.db`。

---

# 18. 本机工具识别

扫描：

```text
$PATH
/usr/bin
/usr/local/bin
~/.local/bin
~/.cargo/bin
```

MVP Package Manager：

```text
apt / dpkg
cargo
pipx
npm global
snap
flatpak
```

后续：

```text
pacman
dnf
brew
nix
```

---

# 19. Search

支持：

```bash
cliary search ncdu
cliary search disk
cliary search "disk usage analyzer"
cliary search "磁盘空间"
cliary search "找大文件"
```

第一阶段：

```text
SQLite FTS5
+
Alias
+
Tag
+
Category
+
Localized Keywords
```

---

# 20. Tool Detail

```bash
cliary show ncdu
```

展示：

```text
Description
Category
Tags
Homepage
Repository
Installed Status
Installed Version
Install Source
Install Commands
Common Commands
Similar Tools
Favorites
Notes
First Used
Last Used
Run Count
Active Days
```

---

# 21. Similar Tools

相似工具来源：

1. Catalog 人工关联
2. Categories
3. Tags
4. Structured Features
5. 后续 Embedding

例如：

```text
ncdu

Similar Tools

gdu
Performance-oriented alternative

dua
Disk usage management

dust
Modern du-like viewer
```

---

# 22. Compare

执行：

```bash
cliary compare ncdu gdu dust dua
```

比较：

```text
Installed
Language
License
TUI
Interactive
Delete Support
Platform
Install Method
Repository
Maintenance Status
```

不同 Category 可以定义不同的 Compare Fields。

例如磁盘工具可以额外比较：

```text
Interactive Navigation
Delete Files
Performance Focus
Disk Visualization
```

---

# 23. Favorites

```bash
cliary favorite ncdu
```

查看：

```bash
cliary favorites
```

Web UI 提供对应 Favorites 页面。

---

# 24. Notes

用户可以为工具保存个人备注：

```bash
cliary note ncdu
```

例如：

```text
服务器磁盘满时用这个。

常用：

ncdu -x /
```

Notes 仅保存于：

```text
user.db
```

---

# 25. Usage History

Usage History 是 CLIary 的核心功能之一。

默认每次捕获工具执行时记录：

```text
tool_id
timestamp
machine_id
```

可选字段：

```text
shell
exit_code
duration
```

默认不记录完整命令。

---

# 26. Shell Integration

MVP 支持：

```text
bash
zsh
fish
```

设计目标：

- 极低延迟
- 异步记录
- CLIary 不运行时 Shell 不受影响
- CLIary 写入失败不影响命令执行
- 默认仅识别 executable

---

# 27. History 查询

查看单工具：

```bash
cliary history ncdu
```

示例：

```text
ncdu

First used:   2026-03-14
Last used:    2026-09-28
Runs:         37
Active days:  16
```

查看整体：

```bash
cliary history
```

---

# 28. Statistics

支持：

```bash
cliary stats
cliary stats --period 7d
cliary stats --period 30d
cliary stats --year 2026
```

统计：

```text
Most Used Tools
Active Days
Tool Usage
Category Usage
Daily Activity
Monthly Activity
New Tools
Dormant Tools
```

---

# 29. CLIary Wrapped

CLIary Wrapped 是年度回顾功能。

执行：

```bash
cliary wrapped 2026
```

Web UI 提供完整视觉版。

示例：

```text
Your 2026 CLIary

CLI tools used          143
New tools discovered     38
Total executions      8,421
Active days              217
```

Top Tools：

```text
1. rg        1,284
2. git       1,102
3. jq          728
4. docker      613
5. ffmpeg      291
```

---

# 30. Wrapped 特殊统计

## 年度新宠

```text
Breakout Tool

dust

First used:
Mar 18

Runs:
183
```

---

## 被遗忘的收藏

```text
Most Forgotten Favorite

hyperfine

Favorited:
219 days

Used:
2 times
```

---

## 使用习惯变化

例如：

```text
You gradually replaced grep with rg.

Docker usage increased significantly in May.

27 Rust-based CLI tools entered your CLIary this year.
```

这些结果必须基于真实 History 数据计算。

---

# 31. CLI

主命令：

```text
cliary
```

MVP：

```bash
cliary search <query>
cliary show <tool>
cliary compare <tools...>
cliary installed
cliary scan
cliary categories
cliary favorites
cliary favorite <tool>
cliary note <tool>
cliary history [tool]
cliary stats
cliary wrapped [year]
cliary sync
cliary web
cliary config
```

---

# 32. JSON 输出

支持：

```text
--json
```

例如：

```bash
cliary search disk --json
```

JSON 字段必须保持语言无关：

```json
{
  "name": "ncdu",
  "installed": true,
  "run_count": 37
}
```

禁止因为 CLI 语言切换而将 Key 翻译为中文。

---

# 33. 语言设置

默认顺序：

```text
用户显式配置
↓
System Locale
↓
English fallback
```

设置：

```bash
cliary config set language zh-CN
```

或：

```bash
cliary config set language en
```

临时：

```bash
cliary --lang en search disk
```

```bash
cliary --lang zh-CN search 磁盘
```

---

# 34. Web UI

启动：

```bash
cliary web
```

默认：

```text
http://127.0.0.1:8848
```

Web UI 仅监听本机地址。

CLI 与 Web 共用：

```text
cliary-core
catalog.db
user.db
```

---

# 35. Web Home

首页展示：

```text
CLIary

[ Search tools... ]

Installed
128

Favorites
23

Catalog
1,328

Used this month
47
```

以及：

```text
Recently Used
Most Used
Recently Discovered
Categories
```

---

# 36. Tool Page

展示：

```text
ncdu

Interactive disk usage analyzer

✓ Installed

Version
2.9

Installed via
apt

First used
Mar 14, 2026

Last used
Sep 28, 2026

Runs
37

Active days
16
```

以及：

```text
Install
Commands
Similar Tools
Compare
Notes
History
```

---

# 37. History Page

展示：

```text
Usage Timeline
Top Tools
Daily Activity
Monthly Activity
Category Distribution
Recently Discovered
Dormant Tools
```

---

# 38. Compare Page

允许选择多个工具：

```text
ncdu
gdu
dust
dua
```

生成结构化比较表。

---

# 39. CLIary Wrapped Page

Web UI 提供年度可视化页面。

包含：

```text
Total Runs
Active Days
Tools Used
New Tools
Top 10 Tools
Top Categories
Monthly Trend
Breakout Tool
Forgotten Favorite
Usage Changes
```

设计目标：

> 面向 CLI 工具的个人年度回顾。

---

# 40. Rust Workspace

项目结构：

```text
cliary/

Cargo.toml

crates/
├── cliary-core/
├── cliary-cli/
└── cliary-web/
```

---

# 41. cliary-core

负责：

```text
Catalog
Database
Search
Compare
Installed Scan
Sync
Favorites
Notes
History
Statistics
Wrapped
Settings
Locale
```

禁止：

```text
Terminal-specific formatting
HTML rendering
Web routing
```

---

# 42. cliary-cli

负责：

```text
Argument Parsing
Terminal Rendering
Tables
JSON Output
```

推荐：

```text
clap
tabled / comfy-table
```

---

# 43. cliary-web

负责：

```text
Axum
Askama
HTMX
Vanilla JavaScript
Static Assets
Charts
```

Web Handler 直接调用：

```text
cliary-core
```

---

# 44. Core API

Core 应提供类似：

```rust
search()
get_tool()
compare()

scan_installed()
get_installed()

sync_catalog()

add_favorite()
remove_favorite()

save_note()

record_usage()
get_history()
get_stats()
generate_wrapped()

get_settings()
set_language()
```

CLI 和 Web 仅消费这些能力。

---

# 45. 推荐技术栈

## Language

```text
Rust
```

## Web

```text
Axum
Askama
HTMX
Vanilla JavaScript
```

## Database

```text
SQLite
SQLx
FTS5
```

## CLI

```text
clap
tabled / comfy-table
```

## HTTP

```text
reqwest
```

## Compression

```text
zstd
```

## Serialization

```text
serde
serde_json
serde_yaml
```

---

# 46. i18n Architecture

程序 UI 和 Catalog 本地化数据分开。

```text
Program i18n
│
├── CLI
└── Web

Catalog i18n
│
├── Tool Descriptions
├── Categories
├── Tags
└── Use Cases
```

Rust 可考虑：

```text
fluent
fluent-bundle
```

原则：

> Core 业务代码中禁止散落硬编码中文或英文 UI 文案。

---

# 47. AI

V0.1：

```text
No AI required
```

V0.2 可以加入：

```text
Semantic Search
Cross-language Search
Natural Language Query
Automatic Similarity
Compare Summary
Wrapped Summary
```

AI 仅负责：

```text
理解
检索
排序
总结
```

不负责：

```text
事实生成
安装命令生成
版本信息生成
License 判断
```

---

# 48. MVP

## Catalog

- 50～100 个工具
- Tool YAML Schema
- 中英文 Category
- 中英文 Tag
- 英文 Tool Description 必需
- 中文 Description 推荐
- GitHub Actions Validation
- GitHub Release

---

## Core

实现：

```text
SQLite
Catalog Sync
Search
Show
Compare
Installed Detection
Favorites
Notes
Usage History
Statistics
Locale
```

---

## CLI

实现：

```text
search
show
compare
installed
scan
sync
favorite
favorites
note
history
stats
web
config
```

---

## Web

实现：

```text
Home
Search
Tool Detail
Installed
Categories
Favorites
Compare
History
Basic Statistics
Language Switcher
```

---

# 49. V0.1 不要求 Wrapped 完整视觉化

V0.1 只需要保证：

```bash
cliary stats
```

已经持续积累足够的数据。

完整：

```text
CLIary Wrapped
```

可以在 V0.2 完善视觉效果。

但 History 数据模型必须从第一版支持长期积累，避免未来迁移困难。

---

# 50. 后续方向

## Semantic Search

```text
找一个处理 JSON 的工具
```

等价于：

```text
tools for inspecting JSON
```

---

## Machine Profiles

未来支持：

```text
Laptop
Workstation
VPS
Server
```

例如：

```text
Which machines have ncdu installed?
```

---

## Export

```bash
cliary export
```

导出：

```text
Favorites
Notes
Tool List
Settings
```

---

## Import

```bash
cliary import
```

可以用于新机器恢复个人工具架。

---

## Historical Import

导入：

```text
bash history
zsh history
fish history
```

仅提取 executable。

不默认保存完整命令。

---

## Multi-Year History

长期支持：

```text
2026
2027
2028
```

可以比较：

```text
Most Used Tools by Year
Category Changes
Tools You Stopped Using
Tools That Became Permanent
```

---

## TUI

未来可以增加第三个前端：

```text
cliary-core
├── cliary-cli
├── cliary-web
└── cliary-tui
```

---

# 51. 非目标

MVP 不做：

- 用户账号
- SaaS Backend
- 中央数据库强依赖
- 在线聊天机器人
- 通用 Package Manager
- 自动执行任意安装命令
- Shell History 替代品
- 完整命令审计系统
- IDE 插件
- Windows GUI

CLIary 的 History 是：

> CLI 工具使用历史。

而不是：

> 完整 Shell 命令历史。

---

# 52. 成功标准

V0.1 至少满足：

1. 能搜索一个忘记名称的 CLI 工具。
2. 能识别常见已安装工具。
3. 能显示安装状态。
4. 能查看安装方式。
5. 能发现同类工具。
6. 能比较同类工具。
7. CLI 与 Web 基本能力一致。
8. Catalog 可以通过 GitHub PR 扩展。
9. Catalog 可以通过 GitHub Release 更新。
10. 不依赖 CLIary 自建服务器。
11. 能持续记录工具使用历史。
12. 能统计常用工具和使用趋势。
13. 默认不记录敏感命令参数。
14. CLI 支持 English。
15. CLI 支持简体中文。
16. Web 支持 English。
17. Web 支持简体中文。
18. 中文和英文均能搜索工具。
19. Catalog 更新不得覆盖用户数据。
20. 卸载 CLIary 不影响任何已安装 CLI 工具。

---

# 53. 品牌与技术命名规范

## 品牌名称

始终：

```text
CLIary
```

正确：

```text
CLIary
CLIary Wrapped
CLIary Catalog
CLIary Web UI
```

避免：

```text
Cliary
cliAry
CLIARY
```

除环境变量等纯技术场景外。

---

## 命令

始终：

```text
cliary
```

例如：

```bash
cliary search ncdu
cliary web
```

---

## Rust Crates

统一：

```text
cliary-core
cliary-cli
cliary-web
```

未来：

```text
cliary-tui
```

---

## Rust Module / Package Identifier

使用：

```text
cliary
cliary_core
cliary_cli
cliary_web
```

---

## 文件与目录

使用：

```text
cliary
```

例如：

```text
~/.config/cliary/
~/.local/share/cliary/
~/.cache/cliary/
```

---

## 环境变量

使用全大写：

```text
CLIARY_*
```

例如：

```text
CLIARY_LANG
CLIARY_DATA_DIR
```

---

# 54. 一句话定位

English:

> **Discover your tools. Remember how you use them.**

简体中文：

> **发现你的工具，也记住你如何使用它们。**