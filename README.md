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

`search`, `show`, `compare`, `installed`, `scan`, `categories`, `favorites`, `favorite`, `note`, `history`, `stats`, `sync`, `web`, `config`, and `setup shell` are available in V0.1. Read commands support `--json`, whose keys stay in English. `cliary note <tool>` opens `$EDITOR`; `--set` and `--delete` are available for scripts. `cliary stats --period 30d` and `cliary stats --year 2026` select periods.

`--lang en` and `--lang zh-CN` temporarily override the language. `cliary config set language zh-CN` saves it. Search indexes both languages regardless of the UI language.

## Browse installed tools

The Web **Installed** page defaults to catalog matches. Choose **All binaries** or **Unmatched binaries** to browse the complete saved scan, including programs not in the Catalog. Combine name/path search, recorded source, exact executable directory, and Catalog category filters. Missing sources are shown as **Not detected**; directories describe discovery locations and do not establish package ownership. Category filters apply only to Catalog matches.

Results are paginated in groups of 100 without truncating the scan. Pagination preserves filters in the URL; applying new filters starts at the first page. **Clear filters** retains the selected match status. Install or remove tools, then re-scan to update the snapshot.

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

Complete CLIary Wrapped, semantic/AI search, historical shell import, user accounts, and automatic execution of installation commands are planned for later releases.
