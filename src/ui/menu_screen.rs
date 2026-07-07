use ratatui::crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{Action, App};
use crate::ui::centered;

const ITEMS: &[(&str, &str)] = &[
    ("resume", "back to the test"),
    ("settings", "all config options"),
    ("themes", "pick or edit themes"),
    ("quit", "exit monkeytype-tui"),
];

#[derive(Default)]
pub struct MenuState {
    pub selected: usize,
}

impl MenuState {
    pub fn handle(&mut self, code: KeyCode) -> Action {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(ITEMS.len() - 1);
                Action::None
            }
            KeyCode::Esc => Action::CloseToTest,
            KeyCode::Enter => match ITEMS[self.selected].0 {
                "resume" => Action::CloseToTest,
                "settings" => Action::OpenSettings,
                "themes" => Action::OpenThemes,
                _ => Action::Quit,
            },
            KeyCode::Char('q') => Action::Quit,
            _ => Action::None,
        }
    }
}

pub fn draw(frame: &mut Frame, app: &App, state: &MenuState) {
    let theme = &app.theme;
    let area = centered(frame.area(), 40, ITEMS.len() as u16 + 2);
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        "monkeytype-tui",
        Style::default().fg(theme.main.color()),
    ))
    .centered()];
    lines.push(Line::default());
    for (i, (name, hint)) in ITEMS.iter().enumerate() {
        let selected = i == state.selected;
        let marker = if selected { "> " } else { "  " };
        let name_style = if selected {
            Style::default().fg(theme.main.color())
        } else {
            Style::default().fg(theme.text.color())
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker}{name:<10}"), name_style),
            Span::styled(*hint, Style::default().fg(theme.sub.color())),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
}
