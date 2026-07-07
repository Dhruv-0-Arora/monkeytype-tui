use ratatui::crossterm::event::KeyCode;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{Action, App};
use crate::config::Config;
use crate::storage::Paths;
use crate::theme::{Rgb, Theme, ThemeFile};
use crate::ui::centered;

const FIELD_NAMES: [&str; 10] = [
    "bg",
    "main",
    "caret",
    "sub",
    "subAlt",
    "text",
    "error",
    "errorExtra",
    "colorfulError",
    "colorfulErrorExtra",
];

#[allow(clippy::large_enum_variant)]
enum EditorMode {
    /// Picking from the theme list (plus the "custom colors" entry).
    Pick,
    /// Editing the 10 hex fields of a theme file (or custom colors).
    Edit {
        name: String,
        colors: [String; 10],
        field: usize,
        /// Text being typed into the focused field.
        input: String,
    },
}

pub struct ThemeState {
    mode: EditorMode,
    selected: usize,
    entries: Vec<String>,
    pub message: Option<String>,
}

impl ThemeState {
    pub fn new(themes: &[(String, Theme)]) -> Self {
        let mut entries: Vec<String> = themes.iter().map(|(n, _)| n.clone()).collect();
        entries.push("[custom colors]".into());
        entries.push("[new theme]".into());
        Self {
            mode: EditorMode::Pick,
            selected: 0,
            entries,
            message: None,
        }
    }

    pub fn handle(
        &mut self,
        code: KeyCode,
        config: &mut Config,
        themes: &[(String, Theme)],
        paths: Option<&Paths>,
    ) -> Action {
        match &mut self.mode {
            EditorMode::Pick => self.handle_pick(code, config, themes),
            EditorMode::Edit { .. } => self.handle_edit(code, config, paths),
        }
    }

    fn handle_pick(
        &mut self,
        code: KeyCode,
        config: &mut Config,
        themes: &[(String, Theme)],
    ) -> Action {
        let custom_idx = self.entries.len() - 2;
        let new_idx = self.entries.len() - 1;
        match code {
            KeyCode::Esc => Action::OpenMenu,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.entries.len() - 1);
                Action::None
            }
            KeyCode::Char('f') => {
                // toggle favorite for randomTheme "fav"
                if self.selected < custom_idx {
                    let name = self.entries[self.selected].clone();
                    if let Some(pos) = config.fav_themes.iter().position(|f| *f == name) {
                        config.fav_themes.remove(pos);
                    } else {
                        config.fav_themes.push(name);
                    }
                    return Action::ConfigChanged;
                }
                Action::None
            }
            KeyCode::Char('e') => {
                // edit selected theme (or custom colors)
                if self.selected == custom_idx {
                    self.open_editor("[custom colors]".into(), config.custom_theme_colors.clone());
                } else if self.selected < custom_idx {
                    let name = self.entries[self.selected].clone();
                    if let Some(theme) = crate::theme::by_name(themes, &name) {
                        self.open_editor(name, theme.to_colors10());
                    }
                }
                Action::None
            }
            KeyCode::Enter => {
                if self.selected == new_idx {
                    self.open_editor(String::new(), Theme::fallback().to_colors10());
                    Action::None
                } else if self.selected == custom_idx {
                    config.custom_theme = true;
                    Action::ConfigChanged
                } else {
                    config.custom_theme = false;
                    config.theme = self.entries[self.selected].clone();
                    Action::ConfigChanged
                }
            }
            _ => Action::None,
        }
    }

    fn open_editor(&mut self, name: String, colors: [String; 10]) {
        self.mode = EditorMode::Edit {
            input: colors[0].clone(),
            name,
            colors,
            field: 0,
        };
    }

    fn handle_edit(&mut self, code: KeyCode, config: &mut Config, paths: Option<&Paths>) -> Action {
        let EditorMode::Edit {
            name,
            colors,
            field,
            input,
        } = &mut self.mode
        else {
            return Action::None;
        };
        match code {
            KeyCode::Esc => {
                self.mode = EditorMode::Pick;
                Action::None
            }
            KeyCode::Up => {
                colors[*field] = input.clone();
                *field = field.saturating_sub(1);
                *input = colors[*field].clone();
                Action::None
            }
            KeyCode::Down | KeyCode::Tab => {
                colors[*field] = input.clone();
                *field = (*field + 1) % 10;
                *input = colors[*field].clone();
                Action::None
            }
            KeyCode::Backspace => {
                // Editing the name row is field 10 semantics kept simple:
                // name edits happen through the input only for new themes.
                input.pop();
                Action::None
            }
            KeyCode::Char(c) if c.is_ascii_hexdigit() || c == '#' => {
                if input.len() < 7 {
                    input.push(c.to_ascii_lowercase());
                }
                Action::None
            }
            KeyCode::Char(c) if name.is_empty() && (c.is_ascii_alphanumeric() || c == '_') => {
                // While the theme has no name yet, letters type the name.
                name.push(c);
                Action::None
            }
            KeyCode::Enter => {
                colors[*field] = input.clone();
                // validate all fields before saving
                if colors.iter().any(|c| Rgb::parse(c).is_none()) {
                    self.message = Some("invalid hex color - use #rrggbb".into());
                    return Action::None;
                }
                if name == "[custom colors]" {
                    config.custom_theme_colors = colors.clone();
                    config.custom_theme = true;
                    self.mode = EditorMode::Pick;
                    return Action::ConfigChanged;
                }
                if name.is_empty() {
                    self.message = Some("type a name for the new theme first".into());
                    return Action::None;
                }
                let Some(paths) = paths else {
                    self.message = Some("no config directory available".into());
                    return Action::None;
                };
                let theme = Theme::from_colors10(colors).expect("validated above");
                let file = ThemeFile::from_theme(&theme);
                let path = paths.themes_dir.join(format!("{name}.toml"));
                let body = toml::to_string_pretty(&file).unwrap_or_default();
                if std::fs::write(&path, body).is_err() {
                    self.message = Some(format!("failed to write {}", path.display()));
                    return Action::None;
                }
                config.theme = name.clone();
                config.custom_theme = false;
                let display_name = name.clone();
                self.mode = EditorMode::Pick;
                self.message = Some(format!("saved theme '{display_name}'"));
                Action::ThemesChanged
            }
            _ => Action::None,
        }
    }
}

pub fn draw(frame: &mut Frame, app: &App, state: &ThemeState) {
    match &state.mode {
        EditorMode::Pick => draw_pick(frame, app, state),
        EditorMode::Edit {
            name,
            colors,
            field,
            input,
        } => draw_edit(frame, app, state, name, colors, *field, input),
    }
}

fn draw_pick(frame: &mut Frame, app: &App, state: &ThemeState) {
    let theme = &app.theme;
    let area = centered(frame.area(), 56, (state.entries.len() as u16 + 5).min(24));
    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("themes  ", Style::default().fg(theme.main.color())),
            Span::styled(
                "enter apply · e edit · f favorite · esc back",
                Style::default().fg(theme.sub.color()),
            ),
        ]),
        Line::default(),
    ];
    for (i, name) in state.entries.iter().enumerate() {
        let selected = i == state.selected;
        let marker = if selected { "> " } else { "  " };
        let fav = if app.config.fav_themes.contains(name) {
            "* "
        } else {
            "  "
        };
        let mut spans = vec![Span::styled(
            format!("{marker}{fav}{name:<24}"),
            if selected {
                Style::default().fg(theme.main.color())
            } else {
                Style::default().fg(theme.text.color())
            },
        )];
        // live preview line rendered in the theme's own colors
        if let Some(t) = crate::theme::by_name(&app.themes, name) {
            spans.push(Span::styled(
                "mon",
                Style::default().fg(t.main.color()).bg(t.bg.color()),
            ));
            spans.push(Span::styled(
                "key",
                Style::default().fg(t.text.color()).bg(t.bg.color()),
            ));
            spans.push(Span::styled(
                "type",
                Style::default().fg(t.sub.color()).bg(t.bg.color()),
            ));
        }
        lines.push(Line::from(spans));
    }
    if state.entries.len() == 2 {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "no theme files yet - create one with [new theme]",
            Style::default().fg(theme.sub.color()),
        )));
    }
    if let Some(msg) = &state.message {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            msg.clone(),
            Style::default().fg(theme.main.color()),
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

#[allow(clippy::too_many_arguments)]
fn draw_edit(
    frame: &mut Frame,
    app: &App,
    state: &ThemeState,
    name: &str,
    colors: &[String; 10],
    field: usize,
    input: &str,
) {
    let theme = &app.theme;
    let area = centered(frame.area(), 56, 17);
    let title = if name.is_empty() {
        "new theme - type a name, then edit colors".to_string()
    } else {
        format!("editing '{name}'")
    };
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(title, Style::default().fg(theme.main.color()))),
        Line::from(Span::styled(
            "↑↓/tab field · hex digits edit · enter save · esc cancel",
            Style::default().fg(theme.sub.color()),
        )),
        Line::default(),
    ];
    for (i, field_name) in FIELD_NAMES.iter().enumerate() {
        let focused = i == field;
        let value = if focused { input } else { &colors[i] };
        let swatch_style = Rgb::parse(value)
            .map(|rgb| Style::default().bg(rgb.color()))
            .unwrap_or_default();
        let marker = if focused { "> " } else { "  " };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{marker}{field_name:<20}"),
                if focused {
                    Style::default()
                        .fg(theme.text.color())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.sub.color())
                },
            ),
            Span::styled(
                format!("{value:<8}"),
                Style::default().fg(theme.text.color()),
            ),
            Span::styled("      ", swatch_style),
        ]));
    }
    if let Some(msg) = &state.message {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            msg.clone(),
            Style::default().fg(theme.error.color()),
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}
