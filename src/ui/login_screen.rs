use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
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

/// An in-progress browser OAuth flow: the URL the browser was pointed at and
/// the buffer collecting the redirect URL the user pastes back.
pub struct OAuthPrompt {
    pub provider: OAuthProvider,
    pub auth_uri: String,
    pub open_failed: bool,
    pub paste: String,
}

pub struct LoginState {
    email: String,
    password: String,
    focus: usize,
    pub error: Option<String>,
    pub busy: Option<String>,
    pub oauth: Option<OAuthPrompt>,
}

/// What the login screen asks the app to do (auth requests are dispatched by App).
pub enum LoginAction {
    None,
    Back,
    SubmitEmail {
        email: String,
        password: String,
    },
    StartOAuth(OAuthProvider),
    /// Complete OAuth with the pasted redirect URL.
    FinishOAuth(String),
    /// Abandon the in-progress OAuth flow.
    CancelOAuth,
}

impl LoginState {
    pub fn new() -> Self {
        Self {
            email: String::new(),
            password: String::new(),
            focus: 0,
            error: None,
            busy: None,
            oauth: None,
        }
    }

    pub fn set_error(&mut self, msg: String) {
        self.busy = None;
        self.oauth = None;
        self.error = Some(msg);
    }

    /// The worker built the provider URL; show it and await the pasted redirect.
    pub fn set_oauth_prompt(
        &mut self,
        provider: OAuthProvider,
        auth_uri: String,
        open_failed: bool,
    ) {
        self.busy = None;
        self.error = None;
        self.oauth = Some(OAuthPrompt {
            provider,
            auth_uri,
            open_failed,
            paste: String::new(),
        });
    }

    /// Route pasted text (bracketed paste) into the focused input.
    pub fn paste(&mut self, text: &str) {
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if let Some(prompt) = &mut self.oauth {
            prompt.paste.push_str(&clean);
            return;
        }
        if self.busy.is_some() {
            return;
        }
        match ORDER[self.focus] {
            Field::Email => self.email.push_str(&clean),
            Field::Password => self.password.push_str(&clean),
            _ => {}
        }
    }

    pub fn handle(&mut self, code: KeyCode, modifiers: KeyModifiers) -> LoginAction {
        if let Some(prompt) = &mut self.oauth {
            return match code {
                KeyCode::Esc => {
                    self.oauth = None;
                    LoginAction::CancelOAuth
                }
                KeyCode::Backspace => {
                    prompt.paste.pop();
                    LoginAction::None
                }
                KeyCode::Enter => {
                    if prompt.paste.trim().is_empty() {
                        LoginAction::None
                    } else {
                        let url = prompt.paste.trim().to_string();
                        self.oauth = None;
                        self.busy = Some("completing sign-in...".into());
                        LoginAction::FinishOAuth(url)
                    }
                }
                KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                    prompt.paste.push(c);
                    LoginAction::None
                }
                _ => LoginAction::None,
            };
        }
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
                self.busy = Some("contacting Google...".into());
                LoginAction::StartOAuth(OAuthProvider::Google)
            }
            Field::Github => {
                self.error = None;
                self.busy = Some("contacting GitHub...".into());
                LoginAction::StartOAuth(OAuthProvider::Github)
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
    if let Some(prompt) = &state.oauth {
        draw_oauth_prompt(frame, app, prompt);
        return;
    }
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
            "(esc to cancel)",
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

fn draw_oauth_prompt(frame: &mut Frame, app: &App, prompt: &OAuthPrompt) {
    let theme = &app.theme;
    let width = frame.area().width.saturating_sub(8).clamp(40, 90);
    let area = centered(frame.area(), width, frame.area().height.min(26));

    let status = if prompt.open_failed {
        "could not open a browser - open this URL manually:"
    } else {
        "a browser tab should have opened; if not, open this URL manually:"
    };

    // Show only the tail of a long paste so the line stays readable.
    let paste_display = {
        let chars: Vec<char> = prompt.paste.chars().collect();
        let max = width.saturating_sub(6) as usize;
        if chars.len() > max {
            format!("…{}", chars[chars.len() - max..].iter().collect::<String>())
        } else {
            prompt.paste.clone()
        }
    };

    let lines = vec![
        Line::from(Span::styled(
            format!("sign in with {}", prompt.provider.label()),
            Style::default().fg(theme.main.color()),
        )),
        Line::default(),
        Line::from(Span::styled(
            format!("1. {status}"),
            Style::default().fg(theme.text.color()),
        )),
        Line::from(Span::styled(
            prompt.auth_uri.clone(),
            Style::default().fg(theme.sub.color()),
        )),
        Line::default(),
        Line::from(Span::styled(
            format!(
                "2. finish signing in with {} in the browser",
                prompt.provider.label()
            ),
            Style::default().fg(theme.text.color()),
        )),
        Line::default(),
        Line::from(Span::styled(
            "3. copy the FULL final URL from the browser address bar",
            Style::default().fg(theme.text.color()),
        )),
        Line::from(Span::styled(
            "   (the page may look blank or show an error - that is expected)",
            Style::default().fg(theme.sub.color()),
        )),
        Line::default(),
        Line::from(Span::styled(
            "4. paste it here and press enter:",
            Style::default().fg(theme.text.color()),
        )),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(theme.main.color())),
            Span::styled(paste_display, Style::default().fg(theme.main.color())),
        ]),
        Line::default(),
        Line::from(Span::styled(
            "enter submit · esc cancel",
            Style::default().fg(theme.sub.color()),
        )),
    ];

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}
