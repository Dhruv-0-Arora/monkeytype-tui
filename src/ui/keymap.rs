//! Keymap widget: static / react / next modes on a qwerty layout.
//! Layout files (config.keymapLayout) are a later phase; qwerty ships in v1.

use std::time::Instant;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::config::{Config, KeymapLegendStyle, KeymapMode};
use crate::theme::Theme;

const ROWS: [&str; 4] = ["qwertyuiop", "asdfghjkl;", "zxcvbnm,./", " "];
const ROW_INDENT: [usize; 4] = [0, 1, 2, 4];

/// How long a pressed key stays lit in react mode.
const REACT_MS: u128 = 250;

pub fn height(config: &Config) -> u16 {
    if config.keymap_mode == KeymapMode::Off {
        0
    } else {
        ROWS.len() as u16 + 1
    }
}

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    config: &Config,
    theme: &Theme,
    next_char: Option<char>,
    last_key: Option<(char, Instant)>,
) {
    if config.keymap_mode == KeymapMode::Off {
        return;
    }
    let reacting = last_key.filter(|(_, at)| at.elapsed().as_millis() < REACT_MS);

    let mut lines: Vec<Line> = vec![Line::default()];
    for (row, keys) in ROWS.iter().enumerate() {
        let mut spans: Vec<Span> = vec![Span::raw(" ".repeat(ROW_INDENT[row] + 1))];
        for key in keys.chars() {
            let legend = match config.keymap_legend_style {
                KeymapLegendStyle::Uppercase => key.to_ascii_uppercase(),
                KeymapLegendStyle::Blank => ' ',
                // dynamic follows the shift state on the web; lowercase here
                KeymapLegendStyle::Lowercase | KeymapLegendStyle::Dynamic => key,
            };
            let is_next = config.keymap_mode == KeymapMode::Next
                && next_char.is_some_and(|c| c.eq_ignore_ascii_case(&key));
            let is_react = config.keymap_mode == KeymapMode::React
                && reacting.is_some_and(|(c, _)| c.eq_ignore_ascii_case(&key));
            let style = if is_next || is_react {
                Style::default()
                    .fg(theme.bg.color())
                    .bg(theme.main.color())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.sub.color())
            };
            let label = if key == ' ' {
                format!("[{: <14}]", legend)
            } else {
                format!("[{legend}]")
            };
            spans.push(Span::styled(label, style));
        }
        lines.push(Line::from(spans).centered());
    }
    frame.render_widget(Paragraph::new(lines), area);
}
