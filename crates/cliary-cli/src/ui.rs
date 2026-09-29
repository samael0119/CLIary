use std::io::IsTerminal;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub enabled: bool,
}

#[allow(dead_code)]
impl Theme {
    pub fn detect() -> Self {
        let is_tty = std::io::stdout().is_terminal();
        let no_color = std::env::var_os("NO_COLOR").is_some()
            || std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false);
        Self {
            enabled: is_tty && !no_color,
        }
    }

    pub fn plain() -> Self {
        Self { enabled: false }
    }

    pub fn bold(&self, text: &str) -> String {
        if self.enabled {
            format!("\x1b[1m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn dim(&self, text: &str) -> String {
        if self.enabled {
            format!("\x1b[90m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn green(&self, text: &str) -> String {
        if self.enabled {
            format!("\x1b[32m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn bright_green(&self, text: &str) -> String {
        if self.enabled {
            format!("\x1b[92;1m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn cyan(&self, text: &str) -> String {
        if self.enabled {
            format!("\x1b[36m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn yellow(&self, text: &str) -> String {
        if self.enabled {
            format!("\x1b[33m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn magenta(&self, text: &str) -> String {
        if self.enabled {
            format!("\x1b[35m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn format_status(&self, installed: bool) -> String {
        if self.enabled {
            if installed {
                "\x1b[92m●\x1b[0m \x1b[32minstalled\x1b[0m   ".to_string()
            } else {
                "\x1b[90m○ available\x1b[0m   ".to_string()
            }
        } else if installed {
            "● installed   ".to_string()
        } else {
            "○ available   ".to_string()
        }
    }

    pub fn link(&self, url: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
        } else {
            format!("{text} ({url})")
        }
    }

    pub fn box_header(&self, title: &str, badge: &str, width: usize) -> String {
        let title_len = title.chars().count();
        let badge_len = if badge.is_empty() {
            0
        } else {
            badge.chars().count() + 3
        }; // " [badge]"
        let prefix = format!("╭─ {} ", self.bold(title));
        let badge_str = if badge.is_empty() {
            String::new()
        } else {
            format!(" [{}] ", self.green(badge))
        };
        let fixed_chars = 3 + title_len + 1 + badge_len + 1; // "╭─ " + title + " " + badge + " "
        let dashes = width.saturating_sub(fixed_chars);
        let border = "─".repeat(dashes.max(2));
        if self.enabled {
            format!("{prefix}{badge_str}{}{}", self.dim(&border), self.dim("─╮"))
        } else {
            format!("╭─ {title} {badge_str}{border}─╮")
        }
    }

    pub fn box_section(&self, title: &str, width: usize) -> String {
        let title_len = title.chars().count();
        let prefix = format!("├─ {} ", self.bold(title));
        let fixed_chars = 3 + title_len + 1;
        let dashes = width.saturating_sub(fixed_chars);
        let border = "─".repeat(dashes.max(2));
        if self.enabled {
            format!("{prefix}{}{}", self.dim(&border), self.dim("─┤"))
        } else {
            format!("├─ {title} {border}─┤")
        }
    }

    pub fn box_footer(&self, width: usize) -> String {
        let dashes = width.saturating_sub(2);
        let border = "─".repeat(dashes.max(2));
        if self.enabled {
            format!("{}{}{}", self.dim("╰"), self.dim(&border), self.dim("╯"))
        } else {
            format!("╰{border}╯")
        }
    }

    pub fn box_line(&self, content: &str) -> String {
        if self.enabled {
            format!("{} {}", self.dim("│"), content)
        } else {
            format!("│ {content}")
        }
    }
}

pub fn render_bar(val: usize, max: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let max = max.max(1);
    let filled = (val.min(max) * width + max / 2) / max;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

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
        assert_eq!(theme.format_status(true), "● installed   ");
        assert_eq!(theme.format_status(false), "○ available   ");
        assert_eq!(
            theme.link("https://example.com", "Example"),
            "Example (https://example.com)"
        );
    }
}
