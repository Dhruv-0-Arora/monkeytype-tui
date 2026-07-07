use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{Action, App};
use crate::config::registry::{self, SettingDef};
use crate::config::Config;

pub struct SettingsState {
    defs: Vec<SettingDef>,
    selected: usize,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            defs: registry::all(),
            selected: 0,
        }
    }

    pub fn handle(&mut self, code: KeyCode, config: &mut Config) -> Action {
        match code {
            KeyCode::Esc => Action::OpenMenu,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.defs.len() - 1);
                Action::None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                (self.defs[self.selected].cycle)(config, false);
                Action::ConfigChanged
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                (self.defs[self.selected].cycle)(config, true);
                Action::ConfigChanged
            }
            _ => Action::None,
        }
    }
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw(frame: &mut Frame, app: &App, state: &SettingsState) {
    let theme = &app.theme;
    let area = frame.area();
    let width = 58.min(area.width.saturating_sub(4)) as usize;
    let inner = Rect {
        x: area.x + (area.width - width as u16) / 2,
        y: area.y + 1,
        width: width as u16,
        height: area.height.saturating_sub(2),
    };

    // Build display rows: group headers interleaved with settings.
    let mut rows: Vec<Line> = Vec::new();
    let mut row_of_selected = 0;
    let mut last_group = "";
    for (i, def) in state.defs.iter().enumerate() {
        if def.group != last_group {
            last_group = def.group;
            rows.push(Line::default());
            rows.push(Line::from(Span::styled(
                def.group.to_string(),
                Style::default()
                    .fg(theme.main.color())
                    .add_modifier(Modifier::BOLD),
            )));
        }
        if i == state.selected {
            row_of_selected = rows.len();
        }
        let selected = i == state.selected;
        let marker = if selected { "> " } else { "  " };
        let value = (def.display)(&app.config);
        let stub_note = if def.stub {
            "  (no effect in terminal)"
        } else {
            ""
        };
        let key_style = if selected {
            Style::default().fg(theme.text.color())
        } else {
            Style::default().fg(theme.sub.color())
        };
        let value_style = if def.stub {
            Style::default().fg(theme.sub.color())
        } else if selected {
            Style::default().fg(theme.main.color())
        } else {
            Style::default().fg(theme.text.color())
        };
        rows.push(Line::from(vec![
            Span::styled(format!("{marker}{:<28}", def.key), key_style),
            Span::styled(value, value_style),
            Span::styled(stub_note, Style::default().fg(theme.sub.color())),
        ]));
    }

    // Stateless scroll: keep the selected row roughly centered.
    let visible = inner.height.saturating_sub(2) as usize;
    let scroll = row_of_selected.saturating_sub(visible / 2);

    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled("settings  ", Style::default().fg(theme.main.color())),
        Span::styled(
            "↑↓ select · ←→ change · esc back",
            Style::default().fg(theme.sub.color()),
        ),
    ])];
    lines.extend(rows.into_iter().skip(scroll).take(visible));
    frame.render_widget(Paragraph::new(lines), inner);
}
