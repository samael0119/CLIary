# Web 截图

2026-10-09 在本地源码构建后的 CLIary 中，用 Codex IAB 的原生 screenshot API 拍摄。桌面视口 1280×720；工具接口在有滚动条时返回1265px宽的原生截图，不手工改变图像。overview、search、wrapped 为完整页面；compare 展示首屏特性矩阵。原图为 JPEG，README 使用仓库相对链接，可在 GitHub 中显示。

所有收藏、备注和使用记录来自人工演示工作区。没有读取／导入用户个人历史；没有执行 Shell 历史中的命令，也没有扫描机器软件。Catalog 使用程序随附真实信息，安装状态保留「尚未扫描」。截图中的使用次数和趋势仅展示功能，不代表用户真实使用情况。

| 文件 | 页面 | 主题 |
| --- | --- | --- |
| overview-dark.jpg | /，中文工作台概览 | 深色 |
| search-light.jpg | /search，中文任务搜索 | 浅色 |
| compare-light.jpg | /compare?tools=ncdu,gdu,dust，特性矩阵首屏 | 浅色 |
| wrapped-dark.jpg | /wrapped?year=2025，完整年报 | 深色 |

## 复现演示工作区

在仓库根目录运行，directory 必须不存在；脚本会拒绝覆盖已有目录，不改默认 CLIary 配置和数据库。固定的历史示例包含2025全年以及2026年1月至10月8日；首页「近30天」计数会随拍摄日期变化。

```sh
cargo build --locked -p cliary-cli
python3 scripts/seed_web_demo.py --directory /tmp/cliary-web-demo
CLIARY_CONFIG_DIR=/tmp/cliary-web-demo/config \
CLIARY_DATA_DIR=/tmp/cliary-web-demo/data \
CLIARY_CACHE_DIR=/tmp/cliary-web-demo/cache \
TZ=Asia/Shanghai ./target/debug/cliary web
```

打开程序输出的本机 URL，中文界面选择所需主题、搜索任务或比较工具，按表中页面截图。脚本不会自动创建任何截图，也不会启用未来命令采集。默认端口8848，启动前请关闭占用该端口的旧服务。

## CLI GIF

`cli-demo.gif` 为约36秒的循环动画，演示 `search 磁盘空间`、`show ncdu`、`compare ncdu dust`。脚本在92列、24行的 PTY 中运行本地构建的 CLIary，记录实际输出，去除 ANSI 颜色与终端链接控制码，再渲染等宽终端画面。命令以 `--lang zh-CN` 显示中文；不是手写或模型生成的结果。完整输出保存在 [cli-demo.json](cli-demo.json)，无需播放 GIF 也可阅读。

输入动画和结果停留时长是编辑后的演示节奏，不代表实际运行耗时。较长的比较输出先停留在特性矩阵，再向后滚动到命令示例；转场清空画面，不演示安装或执行示例命令。底栏明确标注人工历史和「尚未扫描」。

复现时先按上面的步骤创建独立工作区，再运行：

```sh
# Python 需安装 Pillow；Linux 使用 DejaVu Sans Mono / Noto Sans CJK 字体
python3 scripts/record_cli_demo.py \
  --directory /tmp/cliary-web-demo \
  --output /tmp/cli-demo.gif
```

可用 `--binary` 指定另一个构建的 CLIary。脚本只执行三个只读演示命令，不启用采集、扫描或导入用户历史；不需要启动 Web 服务。输出路径的 GIF 和同名 JSON 会被覆盖，建议生成到临时目录后确认画面再替换仓库资产。
