use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::config::{
    CaretStyle, Config, HighlightMode, LiveStatStyle, TapeMode, TimerColor, TimerOpacity,
    TimerStyle, TypedEffect,
};
use crate::engine::{stats, SessionState, TestMode, TestSession};
use crate::theme::{letter_color, LetterState, Theme};
use crate::ui::{centered, format_speed, keymap};

/// Visible lines of words, mirroring the web's 3-line window.
const VISIBLE_LINES: usize = 3;

pub fn draw(frame: &mut Frame, app: &App) {
    let config = &app.config;
    let line_width = effective_line_width(config, frame.area());
    let words_height = if config.show_all_lines {
        frame.area().height.saturating_sub(8).max(3)
    } else {
        VISIBLE_LINES as u16
    };
    let keymap_height = keymap::height(config);
    let total_height = 4 + words_height + 2 + keymap_height;
    let area = centered(frame.area(), line_width as u16 + 4, total_height);

    let [timer_area, header, _, words_area, _, keymap_area, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(words_height),
        Constraint::Length(1),
        Constraint::Length(keymap_height),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_timer(frame, app, timer_area, header);
    if config.tape_mode != TapeMode::Off {
        draw_tape(frame, app, words_area, line_width);
    } else {
        draw_words(frame, app, words_area, line_width, words_height as usize);
    }
    keymap::draw(
        frame,
        keymap_area,
        config,
        &app.theme,
        next_expected_char(&app.session),
        app.last_key,
    );
    if config.show_key_tips {
        let hint = Line::from(Span::styled(
            "tab restart · esc menu",
            Style::default().fg(app.theme.sub.color()),
        ))
        .centered();
        frame.render_widget(Paragraph::new(hint), footer);
    }
}

fn effective_line_width(config: &Config, area: Rect) -> usize {
    let max = area.width.saturating_sub(4) as usize;
    let configured = if config.max_line_width == 0 {
        64
    } else {
        config.max_line_width.clamp(20, 1000) as usize
    };
    configured.min(max).max(20)
}

fn timer_fg(config: &Config, theme: &Theme) -> Style {
    let color = match config.timer_color {
        TimerColor::Black => theme.bg.color(),
        TimerColor::Sub => theme.sub.color(),
        TimerColor::Text => theme.text.color(),
        TimerColor::Main => theme.main.color(),
    };
    let mut style = Style::default().fg(color);
    // no alpha on a cell grid: approximate opacity by dimming
    if config.timer_opacity != TimerOpacity::Full {
        style = style.add_modifier(Modifier::DIM);
    }
    style
}

fn draw_timer(frame: &mut Frame, app: &App, timer_area: Rect, header: Rect) {
    let config = &app.config;
    let session = &app.session;
    let theme = &app.theme;

    let (progress_text, ratio) = match (session.mode, session.state) {
        (TestMode::Time(secs), SessionState::Running) => {
            let left = secs.saturating_sub(session.elapsed().as_secs());
            (
                left.to_string(),
                session.elapsed().as_secs_f64() / secs as f64,
            )
        }
        (TestMode::Time(secs), _) => (secs.to_string(), 0.0),
        (TestMode::Words(n), _) => (
            format!("{}/{}", session.current.min(n), n),
            session.current as f64 / n.max(1) as f64,
        ),
    };

    match config.timer_style {
        TimerStyle::Off => {}
        TimerStyle::Bar => {
            frame.render_widget(
                Gauge::default()
                    .ratio(ratio.clamp(0.0, 1.0))
                    .gauge_style(timer_fg(config, theme))
                    .label(""),
                timer_area,
            );
        }
        // text/mini/flash variants all render as text at this cell size;
        // flash_* additionally only show right after each second boundary
        TimerStyle::Text | TimerStyle::Mini | TimerStyle::FlashText | TimerStyle::FlashMini => {
            let flash = matches!(
                config.timer_style,
                TimerStyle::FlashText | TimerStyle::FlashMini
            );
            let show = !flash || session.elapsed().subsec_millis() < 500;
            if show {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        progress_text,
                        timer_fg(config, theme),
                    ))),
                    timer_area,
                );
            }
        }
    }

    // live stats row (mini variants sit in the header; text variants too -
    // separate placements need more vertical budget than a terminal has)
    let mut spans: Vec<Span> = Vec::new();
    if session.state == SessionState::Running {
        let elapsed = session.elapsed().as_secs_f64().max(0.5);
        if config.live_speed_style != LiveStatStyle::Off {
            let correct = session.keystrokes.iter().filter(|k| k.correct).count() as u32;
            spans.push(Span::styled(
                format!("{} ", format_speed(stats::wpm(correct, elapsed), config)),
                Style::default().fg(theme.sub.color()),
            ));
        }
        if config.live_acc_style != LiveStatStyle::Off {
            let correct = session.keystrokes.iter().filter(|k| k.correct).count() as u32;
            let total = session.keystrokes.len() as u32;
            spans.push(Span::styled(
                format!("{:.0}% ", stats::accuracy(correct, total)),
                Style::default().fg(theme.sub.color()),
            ));
        }
        if config.live_burst_style != LiveStatStyle::Off {
            let second = session.elapsed().as_secs() as usize;
            let burst = session
                .keystrokes
                .iter()
                .filter(|k| k.at.as_secs() as usize == second)
                .count() as f64
                * 12.0;
            spans.push(Span::styled(
                format_speed(burst, config),
                Style::default().fg(theme.sub.color()),
            ));
        }
    }
    if !spans.is_empty() {
        frame.render_widget(Paragraph::new(Line::from(spans)).right_aligned(), header);
    }
}

fn next_expected_char(session: &TestSession) -> Option<char> {
    let target: Vec<char> = session.target[session.current].chars().collect();
    let pos = session.typed[session.current].chars().count();
    target.get(pos).copied().or(Some(' '))
}

/// Resolve a letter's display state honoring blindMode and hideExtraLetters.
fn display_state(state: LetterState, config: &Config) -> Option<LetterState> {
    match state {
        LetterState::Extra if config.hide_extra_letters || config.blind_mode => None,
        LetterState::Incorrect if config.blind_mode => Some(LetterState::Correct),
        s => Some(s),
    }
}

/// Whole-word emphasis for highlightMode word/next_word*.
fn word_emphasized(config: &Config, word_idx: usize, current: usize) -> bool {
    match config.highlight_mode {
        HighlightMode::Word => word_idx == current,
        HighlightMode::NextWord => (current..=current + 1).contains(&word_idx),
        HighlightMode::NextTwoWords => (current..=current + 2).contains(&word_idx),
        HighlightMode::NextThreeWords => (current..=current + 3).contains(&word_idx),
        _ => false,
    }
}

fn letter_span(
    c: char,
    state: LetterState,
    word_idx: usize,
    app: &App,
    past_word: bool,
) -> Option<Span<'static>> {
    let config = &app.config;
    let theme = &app.theme;
    let state = display_state(state, config)?;

    // typedEffect applies to words the caret has moved past
    if past_word {
        match config.typed_effect {
            TypedEffect::Keep => {}
            TypedEffect::Hide => {
                return Some(Span::styled(
                    c.to_string(),
                    Style::default().fg(theme.bg.color()),
                ));
            }
            TypedEffect::Fade => {
                return Some(Span::styled(
                    c.to_string(),
                    Style::default()
                        .fg(theme.sub.color())
                        .add_modifier(Modifier::DIM),
                ));
            }
            TypedEffect::Dots => {
                return Some(Span::styled(
                    "·".to_string(),
                    Style::default().fg(theme.sub.color()),
                ));
            }
        }
    }

    let mut color = letter_color(state, theme, config.flip_test_colors, config.colorful_mode);
    // word-level highlight brightens untyped letters of emphasized words
    if state == LetterState::Untyped {
        if word_emphasized(config, word_idx, app.session.current) {
            color = letter_color(
                LetterState::Correct,
                theme,
                config.flip_test_colors,
                config.colorful_mode,
            );
        } else if config.highlight_mode == HighlightMode::Off {
            // off: no distinction for position; keep everything sub
        }
    }
    Some(Span::styled(c.to_string(), Style::default().fg(color)))
}

fn caret_span(app: &App, c: char) -> Span<'static> {
    let theme = &app.theme;
    match app.config.caret_style {
        CaretStyle::Off => Span::styled(
            c.to_string(),
            Style::default().fg(letter_color(
                LetterState::Untyped,
                theme,
                app.config.flip_test_colors,
                app.config.colorful_mode,
            )),
        ),
        CaretStyle::Underline => Span::styled(
            c.to_string(),
            Style::default()
                .fg(theme.caret.color())
                .add_modifier(Modifier::UNDERLINED),
        ),
        CaretStyle::Outline => Span::styled(
            c.to_string(),
            Style::default()
                .fg(theme.caret.color())
                .add_modifier(Modifier::REVERSED | Modifier::DIM),
        ),
        // default/block/carrot/banana/monkey: a cell grid renders them all as
        // a block caret (documented approximation, see PLAN.md)
        _ => Span::styled(
            c.to_string(),
            Style::default()
                .fg(theme.caret.color())
                .add_modifier(Modifier::REVERSED),
        ),
    }
}

struct LaidOutWord {
    line: usize,
    letters: Vec<(char, LetterState)>,
    word_idx: usize,
    caret_at: usize,
}

fn layout_words(session: &TestSession, line_width: usize) -> Vec<LaidOutWord> {
    let mut out = Vec::new();
    let mut line = 0usize;
    let mut col = 0usize;
    let mut current_line = None;
    for idx in 0..session.target.len() {
        let letters = session.letter_states(idx);
        let width = letters.len();
        if col > 0 && col + width > line_width {
            line += 1;
            col = 0;
        }
        if idx == session.current {
            current_line = Some(line);
        }
        out.push(LaidOutWord {
            line,
            letters,
            word_idx: idx,
            caret_at: session.typed[idx].chars().count(),
        });
        col += width + 1;
        if let Some(cl) = current_line {
            // lay out a bounded slice past the current line, not the whole test
            if line > cl + VISIBLE_LINES + 1 {
                break;
            }
        }
    }
    out
}

fn draw_words(frame: &mut Frame, app: &App, area: Rect, line_width: usize, visible: usize) {
    let session = &app.session;
    let words = layout_words(session, line_width);
    let current_line = words
        .iter()
        .find(|w| w.word_idx == session.current)
        .map(|w| w.line)
        .unwrap_or(0);
    let first_visible = if app.config.show_all_lines {
        0
    } else {
        current_line.saturating_sub(1)
    };

    let mut lines: Vec<Line> = Vec::with_capacity(visible);
    for line_idx in first_visible..first_visible + visible {
        let mut spans: Vec<Span> = Vec::new();
        for word in words.iter().filter(|w| w.line == line_idx) {
            let is_current = word.word_idx == session.current;
            let past = word.word_idx < session.current;
            for (i, &(c, state)) in word.letters.iter().enumerate() {
                if is_current && i == word.caret_at {
                    spans.push(caret_span(app, c));
                } else if let Some(span) = letter_span(c, state, word.word_idx, app, past) {
                    spans.push(span);
                }
            }
            if is_current && word.caret_at >= word.letters.len() {
                spans.push(caret_span(app, ' '));
            } else {
                spans.push(Span::raw(" "));
            }
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// tapeMode: one line, caret pinned at tapeMargin% of the width, words scroll.
fn draw_tape(frame: &mut Frame, app: &App, area: Rect, line_width: usize) {
    let session = &app.session;
    let config = &app.config;
    let margin_cols = (line_width as f64 * config.tape_margin.clamp(10.0, 90.0) / 100.0) as usize;

    // Flatten (char, state, word_idx, is_caret) around the caret.
    let mut cells: Vec<(char, LetterState, usize)> = Vec::new();
    let mut caret_cell = 0usize;
    let from_word = if config.tape_mode == TapeMode::Word {
        session.current
    } else {
        session.current.saturating_sub(8)
    };
    for idx in from_word.saturating_sub(8)..session.target.len().min(session.current + 30) {
        let letters = session.letter_states(idx);
        let caret_here = idx == session.current;
        for (i, (c, state)) in letters.iter().enumerate() {
            if caret_here && i == session.typed[idx].chars().count() {
                caret_cell = cells.len();
            }
            cells.push((*c, *state, idx));
        }
        if caret_here && session.typed[idx].chars().count() >= letters.len() {
            caret_cell = cells.len();
        }
        cells.push((' ', LetterState::Untyped, idx));
    }

    let start = caret_cell.saturating_sub(margin_cols);
    let mut spans: Vec<Span> = Vec::new();
    for (offset, (c, state, word_idx)) in cells.iter().enumerate().skip(start).take(line_width) {
        if offset == caret_cell {
            spans.push(caret_span(app, *c));
        } else if let Some(span) =
            letter_span(*c, *state, *word_idx, app, *word_idx < session.current)
        {
            spans.push(span);
        } else {
            spans.push(Span::raw(" "));
        }
    }
    let mid = area.y + area.height / 2;
    let line_area = Rect {
        x: area.x,
        y: mid,
        width: area.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(Line::from(spans)), line_area);
}
