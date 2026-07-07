use ratatui::crossterm::event::KeyCode;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{Action, App};
use crate::ui::centered;

struct Item {
    id: &'static str,
    label: &'static str,
    hint: &'static str,
}

pub struct MenuState {
    items: Vec<Item>,
    pub selected: usize,
}

impl MenuState {
    pub fn new(logged_in: bool) -> Self {
        let mut items = vec![
            Item {
                id: "resume",
                label: "resume",
                hint: "back to the test",
            },
            Item {
                id: "settings",
                label: "settings",
                hint: "all config options",
            },
            Item {
                id: "themes",
                label: "themes",
                hint: "pick or edit themes",
            },
        ];
        if logged_in {
            items.push(Item {
                id: "logout",
                label: "log out",
                hint: "sign out of your account",
            });
        } else {
            items.push(Item {
                id: "login",
                label: "log in",
                hint: "sign in to save results",
            });
        }
        items.push(Item {
            id: "quit",
            label: "quit",
            hint: "exit monkeytype-tui",
        });
        Self { items, selected: 0 }
    }

    pub fn handle(&mut self, code: KeyCode) -> Action {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.items.len() - 1);
                Action::None
            }
            KeyCode::Esc => Action::CloseToTest,
            KeyCode::Enter => match self.items[self.selected].id {
                "resume" => Action::CloseToTest,
                "settings" => Action::OpenSettings,
                "themes" => Action::OpenThemes,
                "login" => Action::OpenLogin,
                "logout" => Action::Logout,
                _ => Action::Quit,
            },
            KeyCode::Char('q') => Action::Quit,
            _ => Action::None,
        }
    }
}

impl Default for MenuState {
    fn default() -> Self {
        Self::new(false)
    }
}

pub fn draw(frame: &mut Frame, app: &App, state: &MenuState) {
    let theme = &app.theme;
    let notice_rows = if app.auth_notice.is_some() { 2 } else { 0 };
    let area = centered(frame.area(), 44, state.items.len() as u16 + 4 + notice_rows);
    let account_line = match &app.account {
        Some(session) => {
            let who = session
                .email
                .clone()
                .unwrap_or_else(|| session.uid.chars().take(8).collect());
            format!("signed in as {who}")
        }
        None => "not signed in".to_string(),
    };

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "monkeytype-tui",
            Style::default().fg(theme.main.color()),
        ))
        .centered(),
        Line::from(Span::styled(
            account_line,
            Style::default().fg(theme.sub.color()),
        ))
        .centered(),
        Line::default(),
    ];
    if let Some(notice) = &app.auth_notice {
        lines.push(
            Line::from(Span::styled(
                notice.clone(),
                Style::default().fg(theme.error.color()),
            ))
            .centered(),
        );
        lines.push(Line::default());
    }
    for (i, item) in state.items.iter().enumerate() {
        let selected = i == state.selected;
        let marker = if selected { "> " } else { "  " };
        let name_style = if selected {
            Style::default().fg(theme.main.color())
        } else {
            Style::default().fg(theme.text.color())
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker}{:<10}", item.label), name_style),
            Span::styled(item.hint, Style::default().fg(theme.sub.color())),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
}
