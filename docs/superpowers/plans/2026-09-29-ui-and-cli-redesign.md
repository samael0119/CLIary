# CLIary Web & CLI Display Beautification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved modern developer-first UI redesign for both the Web interface and the CLI terminal output in CLIary.

**Architecture:** 
1. In `cliary-cli`, introduce a dedicated terminal formatting module (`crates/cliary-cli/src/ui.rs`) that detects TTY and `NO_COLOR`, providing colored badges, unicode card boxes, aligned tables, and visual percentage bars for `search`, `show`, `compare`, and `stats`.
2. In `cliary-web`, upgrade `style.css` with the modern design tokens (Emerald / Obsidian Dark theme, glassmorphism topbar, clean card shadows, command snippets with copy button), update `page.html` with theme toggling and search shortcuts, and refine the Axum HTML rendering in `src/lib.rs` for Home, Tool Detail, and Search pages.

**Tech Stack:** Rust (Axum, Askama, Clap, ANSI styling), HTML5, Modern CSS (CSS custom properties, CSS Grid, Flexbox), Vanilla JS.

---

### Task 1: Create CLI Formatting & Theming Module (`crates/cliary-cli/src/ui.rs`)

**Files:**
- Create: `crates/cliary-cli/src/ui.rs`
- Modify: `crates/cliary-cli/src/main.rs:1-8`

- [ ] **Step 1: Write the failing unit tests for CLI UI helpers**

Create `crates/cliary-cli/src/ui.rs` with test cases testing ANSI styling, TTY fallback, bar chart generation, and box drawing truncation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bar_chart() {
        let bar = render_bar(50, 100, 10);
        assert_eq!(bar, "█████░░░░░");
        let bar_zero = render_bar(0, 100, 10);
        assert_eq!(bar_zero, "░░░░░░░░░░");
        let bar_full = render_bar(100, 100, 10);
        assert_eq!(bar_full, "██████████");
    }

    #[test]
    fn test_plain_fallback_strips_or_omits_color() {
        let theme = Theme::plain();
        assert_eq!(theme.bold("hello"), "hello");
        assert_eq!(theme.green("ok"), "ok");
    }
}
```

- [ ] **Step 2: Run test to verify it fails to compile or run**

Run: `cargo test -p cliary-cli --lib`
Expected: FAIL (module not registered or functions undefined)

- [ ] **Step 3: Implement `ui.rs` styling and terminal formatters**

Write the complete implementation in `crates/cliary-cli/src/ui.rs`:
- TTY and `NO_COLOR` detection
- ANSI color helpers (`green`, `cyan`, `yellow`, `dim`, `bold`, `magenta`, `gray`)
- Box drawing card builder (`card_box(title, badge, body_lines)`)
- Text bar chart generator (`render_bar(val, max, width)`)
- Clean column padding helper for aligned table columns

- [ ] **Step 4: Register `mod ui;` in `crates/cliary-cli/src/main.rs` and verify tests pass**

Add `mod ui;` to `crates/cliary-cli/src/main.rs`.
Run: `cargo test -p cliary-cli`
Expected: PASS (all tests pass)

---

### Task 2: Refactor `cliary search` and `cliary show` CLI Commands

**Files:**
- Modify: `crates/cliary-cli/src/main.rs:143-255`
- Test: manual execution via `cargo run -- search disk` and `cargo run -- show ncdu`

- [ ] **Step 1: Update `Command::Search` output**

Refactor the loop in `Command::Search` to use `ui::Theme`:
- Output clean table header: `NAME`, `STATUS`, `CATEGORY`, `DESCRIPTION`.
- Print status with colored indicator: `● installed` in bright green, `○ available` in dim gray.
- Name highlighted in bold cyan.
- Show helpful tip footer: `Tip: Run 'cliary show <tool>' for details or 'cliary web' for UI`.

- [ ] **Step 2: Update `Command::Show` output**

Refactor `Command::Show` into a styled Unicode terminal card:
- Header: `╭─ {tool.name} ── [{status_badge}] ─╮`
- Description block.
- Categories & metadata tag badges.
- Commands section (`├─ 常用速查命令 (Common Commands) ─┤`):
  Highlight command lines with green `$ ` prompt and yellow arguments, with inline comment description.
- Install commands section (`├─ 安装方式 (Install) ─┤`).
- Telemetry & Notes section (`├─ 本地使用画像 (Telemetry & Notes) ─┤`).
- Card closing: `╰──╯`.

- [ ] **Step 3: Test and verify search and show output**

Run: `cargo run -- search disk`
Run: `cargo run -- show ncdu`
Verify: Beautiful alignment, color highlighting in TTY, clean plain fallback when piped `cargo run -- search disk | cat`.

---

### Task 3: Refactor `cliary stats` and `cliary compare` CLI Commands

**Files:**
- Modify: `crates/cliary-cli/src/main.rs:256-300`, `390-430`

- [ ] **Step 1: Update `Command::Stats` with Bar Charts**

Refactor `Command::Stats`:
- Header card with Total Runs, Active Days, Tools Used in aligned summary metrics.
- Top Tools list with percentage bar charts:
  `1. git      ████████████████████ 412 次 (32%)`
- Categories usage distribution with percentage bars.
- Activity breakdown.

- [ ] **Step 2: Update `Command::Compare` with formatted Unicode Table**

Refactor `Command::Compare`:
- Replace raw `\t` output with structured Unicode box table (`┌─┬─┐`, `│ │ │`, `└─┴─┘`).
- Automatically pad columns to maximum width.
- Highlight `installed` status with green checkmark or gray dash.

- [ ] **Step 3: Test stats and compare output**

Run: `cargo run -- stats`
Run: `cargo run -- compare ncdu,gdu`
Verify: Tables and bar charts render cleanly without misalignment.

---

### Task 4: Upgrade Web CSS Stylesheet (`crates/cliary-web/static/style.css`)

**Files:**
- Modify: `crates/cliary-web/static/style.css`

- [ ] **Step 1: Integrate Modern Design Tokens**

In `crates/cliary-web/static/style.css`:
- Define CSS custom properties on `:root` and `[data-theme="dark"]` / `@media (prefers-color-scheme: dark)`:
  - Surface colors: `--surface`, `--surface-elevated`, `--surface-sunken`, `--border`, `--border-subtle`
  - Emerald accents: `--primary`, `--primary-hover`, `--primary-light`, `--primary-border`
  - Dark mode obsidian palette: `#090e0c`, `#101715`, `#15201c`, `#10b981`
- Modern font stack: Inter/system-sans for UI, JetBrains Mono / SF Mono for code and CLI outputs.
- Subtle shadows and elevation tokens (`--shadow-sm`, `--shadow-md`, `--shadow-lg`).

- [ ] **Step 2: Add Component Styles**

- Glassmorphic topbar (`backdrop-filter: blur(12px)`).
- Terminal card with traffic lights and glowing code highlight.
- Metric cards with hover lift (`translateY(-2px)`) and top gradient border.
- Command code blocks (`.neo-code-block`) with quick copy button.
- Tool detail 2-column layout and personal shelf notes area.
- Pulse dot animation (`@keyframes pulse`).

---

### Task 5: Upgrade Web Page Template (`crates/cliary-web/templates/page.html`)

**Files:**
- Modify: `crates/cliary-web/templates/page.html`

- [ ] **Step 1: Update Topbar & Navigation**

In `crates/cliary-web/templates/page.html`:
- Add theme toggle button (Dark / Light) in topbar.
- Add quick search shortcut badge (`⌘K`).
- Add pulsing local workspace indicator in sidebar bottom.
- Add minimal inline JS for:
  - `localStorage` theme preference persistence.
  - Copy command snippet to clipboard with `✓ 已复制` tooltip.

---

### Task 6: Refactor Axum HTML Handlers in `crates/cliary-web/src/lib.rs`

**Files:**
- Modify: `crates/cliary-web/src/lib.rs:135-460`

- [ ] **Step 1: Upgrade `home` handler output**

In `crates/cliary-web/src/lib.rs`:
- Hero section: Command-palette style search input with shortcut hint and tag suggestions (`ncdu`, `ripgrep`, `dust`).
- Modern terminal window with syntax colors and traffic light dots.
- Metric cards with trend indicators.
- Rank list with gradient progress bars.

- [ ] **Step 2: Upgrade `tool` detail handler output**

In `crates/cliary-web/src/lib.rs`:
- Render 2-column detail shell:
  - Left column: Tool hero with icon and version, package manager command snippets with copy button, common command cheatsheet.
  - Right column: Usage statistics card, personal note card with save button.

- [ ] **Step 3: Upgrade `search` handler output**

In `crates/cliary-web/src/lib.rs`:
- Clean tool card grid with status pills, category tags, and subtle hover elevation.

---

### Task 7: Comprehensive Verification & Test Suite

**Files:**
- All modified files

- [ ] **Step 1: Run Rust automated tests**

Run: `cargo test`
Expected: PASS (all tests pass across all crates)

- [ ] **Step 2: Verify CLI outputs in terminal**

Run:
```bash
cargo run -- search usage
cargo run -- show ncdu
cargo run -- stats
cargo run -- compare ncdu,gdu
```
Verify: Visual hierarchy, colors, box borders, and alignment are crisp and clear.

- [ ] **Step 3: Verify Web server compilation and assets**

Run: `cargo check --workspace`
Expected: PASS with 0 warnings/errors.
