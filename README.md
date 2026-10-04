# CLIary

**Discover your tools. Remember how you use them.**  
**发现你的工具，也记住你如何使用它们。**

CLIary is a local-first catalog, shelf, and usage diary for command-line tools on Linux and macOS. The Rust CLI and local Web UI share the same Core and SQLite data. There is no account or required server.

## Build and try

```sh
cargo build --release -p cliary-cli
target/release/cliary search "disk usage"
target/release/cliary --lang zh-CN search 磁盘空间
target/release/cliary show ncdu
target/release/cliary compare ncdu gdu dust dua
target/release/cliary scan
target/release/cliary installed
target/release/cliary web
```

`cliary web` serves `http://127.0.0.1:8848` on the loopback interface only. A bundled Catalog makes the first run fully offline. The Release installer downloads a checksum-verified binary, then asks whether to scan installed tools and enable Shell history; **Yes** is the interactive default for each. For unattended installs, pass `--scan` or `--no-scan` and `--enable-history` or `--no-history`. If you run a locally built binary directly, its first interactive launch offers the same setup. Non-interactive first launches scan automatically but leave Shell history disabled.

If you skip scanning, installation status remains **Not scanned** until you run `cliary scan`.

Shell capture can be enabled or removed later:

```sh
cliary setup shell --enable
cliary setup shell --disable
cliary setup shell --shell fish --enable
```

CLIary stores the executable name, timestamp, and a random local machine ID for each captured invocation. It never stores command arguments. Bash history settings may cause some commands to be skipped; compound commands are represented by their first external executable. The Shell hook runs recording in the background so database failures do not interrupt the original command.

## Commands

`search`, `show`, `compare`, `installed`, `scan`, `categories`, `favorites`, `favorite`, `note`, `history`, `stats`, `wrapped`, `sync`, `web`, `config`, and `setup shell` are available. Read commands support `--json`, whose keys stay in English. `cliary note <tool>` opens `$EDITOR`; `--set` and `--delete` are available for scripts. `cliary stats --period 30d` and `cliary stats --year 2026` select periods.

## Annual report / 年度报告

```sh
cliary wrapped                  # Current local year / 当前本地年份
cliary wrapped 2024             # A specific calendar year / 指定日历年
cliary wrapped 2024 --json      # Structured report / 结构化报告
cliary --lang zh-CN wrapped 2024
```

In the Web UI, choose **Wrapped / 年度报告** from the sidebar, or visit `/wrapped?year=2024`. Reports show captured runs, active days, tools used, first/last records, the top ten tools, and all twelve months. Executable aliases recorded with the same tool ID count as one tool; unknown executables remain visible. The current year is marked as in progress. Years from 1 through 9998 are accepted.

Statistics use the device's local calendar time at report generation. Only captured invocations are counted: a zero means no records, and collection gaps cannot be reconstructed. Enable `cliary setup shell --enable` and open a new terminal to capture future commands; earlier commands are not imported. Command arguments are never stored. Reports are generated locally, require no AI or network connection, and do not modify usage events.

网页侧栏选择「年度报告」，可切换年份查看真实调用量、活跃天数、常用工具与十二个月的趋势。数据来自已采集的调用，不代表完整使用历史；空月份不等于没有使用。当前年度会提示尚未结束。

`--lang en` and `--lang zh-CN` temporarily override the language. `cliary config set language zh-CN` saves it. Search indexes both languages regardless of the UI language.

## Data and updates

- Config: `~/.config/cliary/config.toml`
- Shared Catalog: `~/.local/share/cliary/catalog.db`
- Favorites, notes, installed snapshot, and usage events: `~/.local/share/cliary/user.db`
- Cache: `~/.cache/cliary/`

XDG base directories and `CLIARY_CONFIG_DIR`, `CLIARY_DATA_DIR`, and `CLIARY_CACHE_DIR` are supported. A Catalog update replaces only `catalog.db`. Release builds have a built-in GitHub Catalog URL; source builds can use `cliary sync --url https://github.com/OWNER/REPO/releases/latest/download` or set `catalog_url` in `config.toml`.

## Contribute Catalog entries

Add one YAML file under `catalog/tools/<category>/`. English description and at least one executable are required; Simplified Chinese descriptions are encouraged. Categories and tags use stable IDs in `catalog/i18n/`. Validate locally:

```sh
python3 -m pip install PyYAML jsonschema
python3 scripts/validate_catalog.py
cargo run -p cliary-catalog --bin cliary-catalog-build -- validate
cargo test --workspace
```

The GitHub Actions workflow validates PRs and publishes a new Catalog plus four platform binaries after a merge to `main`. `cliary sync` checks the manifest and SHA-256 digest before replacing the local Catalog.

## V0.1 boundaries

Wrapped currently provides a factual annual summary. Data-backed special insights, year-over-year comparisons, export, semantic/AI search, historical shell import, user accounts, and automatic execution of installation commands are planned for later releases.
