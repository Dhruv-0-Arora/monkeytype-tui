use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::auth::OAuthProvider;
use crate::ui::centered;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Email,
    Password,
    Submit,
    Google,
    Github,
}

const ORDER: [Field; 5] = [
    Field::Email,
    Field::Password,
    Field::Submit,
    Field::Google,
    Field::Github,
];

pub struct LoginState {
    email: String,
    password: String,
    focus: usize,
    pub error: Option<String>,
    pub busy: Option<String>,
}

/// What the login screen asks the app to do (auth requests are dispatched by App).
pub enum LoginAction {
    None,
    Back,
    SubmitEmail { email: String, password: String },
    SubmitOAuth(OAuthProvider),
}

impl LoginState {
    pub fn new() -> Self {
        Self {
            email: String::new(),
            password: String::new(),
            focus: 0,
            error: None,
            busy: None,
        }
    }

    pub fn set_error(&mut self, msg: String) {
        self.busy = None;
        self.error = Some(msg);
    }

    pub fn handle(&mut self, code: KeyCode, modifiers: KeyModifiers) -> LoginAction {
        if self.busy.is_some() {
            // ignore input while a login is in flight (except escape)
            if code == KeyCode::Esc {
                return LoginAction::Back;
            }
            return LoginAction::None;
        }
        let field = ORDER[self.focus];
        match code {
            KeyCode::Esc => LoginAction::Back,
            KeyCode::Up => {
                self.focus = self.focus.saturating_sub(1);
                LoginAction::None
            }
            KeyCode::Down | KeyCode::Tab => {
                self.focus = (self.focus + 1) % ORDER.len();
                LoginAction::None
            }
            KeyCode::Backspace => {
                match field {
                    Field::Email => {
                        self.email.pop();
                    }
                    Field::Password => {
                        self.password.pop();
                    }
                    _ => {}
                }
                LoginAction::None
            }
            KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                match field {
                    Field::Email => self.email.push(c),
                    Field::Password => self.password.push(c),
                    _ => {}
                }
                LoginAction::None
            }
            KeyCode::Enter => self.activate(field),
            _ => LoginAction::None,
        }
    }

    fn activate(&mut self, field: Field) -> LoginAction {
        match field {
            Field::Email | Field::Password | Field::Submit => {
                if self.email.trim().is_empty() || self.password.is_empty() {
                    self.error = Some("enter email and password".into());
                    return LoginAction::None;
                }
                self.error = None;
                self.busy = Some("signing in...".into());
                LoginAction::SubmitEmail {
                    email: self.email.trim().to_string(),
                    password: self.password.clone(),
                }
            }
            Field::Google => {
                self.error = None;
                self.busy = Some("waiting for browser (Google)...".into());
                LoginAction::SubmitOAuth(OAuthProvider::Google)
            }
            Field::Github => {
                self.error = None;
                self.busy = Some("waiting for browser (GitHub)...".into());
                LoginAction::SubmitOAuth(OAuthProvider::Github)
            }
        }
    }
}

impl Default for LoginState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw(frame: &mut Frame, app: &App, state: &LoginState) {
    let theme = &app.theme;
    let area = centered(frame.area(), 48, 14);
    let focus = ORDER[state.focus];

    let field_line = |label: &str, value: String, this: Field| -> Line {
        let selected = focus == this;
        let marker = if selected { "> " } else { "  " };
        Line::from(vec![
            Span::styled(
                format!("{marker}{label:<10}"),
                Style::default().fg(if selected {
                    theme.text.color()
                } else {
                    theme.sub.color()
                }),
            ),
            Span::styled(value, Style::default().fg(theme.main.color())),
        ])
    };

    let button = |label: &str, this: Field| -> Line {
        let selected = focus == this;
        let style = if selected {
            Style::default()
                .fg(theme.bg.color())
                .bg(theme.main.color())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text.color())
        };
        Line::from(Span::styled(format!("  [ {label} ]"), style))
    };

    let mut lines = vec![
        Line::from(Span::styled(
            "log in to monkeytype",
            Style::default().fg(theme.main.color()),
        )),
        Line::default(),
        field_line("email", state.email.clone(), Field::Email),
        field_line(
            "password",
            "*".repeat(state.password.chars().count()),
            Field::Password,
        ),
        Line::default(),
        button("sign in", Field::Submit),
        button("continue with Google", Field::Google),
        button("continue with GitHub", Field::Github),
        Line::default(),
    ];

    if let Some(busy) = &state.busy {
        lines.push(Line::from(Span::styled(
            busy.clone(),
            Style::default().fg(theme.main.color()),
        )));
        lines.push(Line::from(Span::styled(
            "(a browser tab should open; esc to cancel)",
            Style::default().fg(theme.sub.color()),
        )));
    } else if let Some(err) = &state.error {
        lines.push(Line::from(Span::styled(
            err.clone(),
            Style::default().fg(theme.error.color()),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "↑↓ move · enter select · esc back",
            Style::default().fg(theme.sub.color()),
        )));
    }

    frame.render_widget(Paragraph::new(lines), area);
}
