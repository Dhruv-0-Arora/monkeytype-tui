use ratatui::style::{Color, Modifier, Style};

/// A monkeytype theme: the same 10 colors the web frontend uses, in
/// `customThemeColors` order (see PLAN.md). Custom theme files, the picker,
/// and the resolver arrive in Phase 2; Phase 1 only uses the fallback.
#[derive(Debug, Clone)]
#[allow(dead_code)] // bg/subAlt/colorful* are consumed by Phase 2 rendering modes
pub struct Theme {
    pub bg: Color,
    pub main: Color,
    pub caret: Color,
    pub sub: Color,
    pub sub_alt: Color,
    pub text: Color,
    pub error: Color,
    pub error_extra: Color,
    pub colorful_error: Color,
    pub colorful_error_extra: Color,
}

impl Theme {
    /// Neutral fallback used until the user defines a theme. Named terminal
    /// colors so it respects the terminal's own palette.
    pub fn fallback() -> Self {
        Self {
            bg: Color::Reset,
            main: Color::Yellow,
            caret: Color::White,
            sub: Color::DarkGray,
            sub_alt: Color::Black,
            text: Color::Gray,
            error: Color::Red,
            error_extra: Color::LightRed,
            colorful_error: Color::Red,
            colorful_error_extra: Color::LightRed,
        }
    }
}

/// Render state of a single target letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LetterState {
    Untyped,
    Correct,
    Incorrect,
    Extra,
}

/// Web mapping: untyped=sub, correct=text, incorrect=error, extra=errorExtra.
/// flipTestColors / colorfulMode remaps land here in Phase 2.
pub fn letter_style(state: LetterState, theme: &Theme) -> Style {
    match state {
        LetterState::Untyped => Style::default().fg(theme.sub),
        LetterState::Correct => Style::default().fg(theme.text),
        LetterState::Incorrect => Style::default().fg(theme.error),
        LetterState::Extra => Style::default().fg(theme.error_extra),
    }
}

pub fn caret_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.caret)
        .add_modifier(Modifier::REVERSED)
}
