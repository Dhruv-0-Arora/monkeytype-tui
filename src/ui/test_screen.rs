use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::engine::stats;
use crate::engine::{SessionState, TestMode};
use crate::theme::{caret_style, letter_style, LetterState};
use crate::ui::centered;

/// Max characters per rendered line (maxLineWidth config lands in Phase 2).
const LINE_WIDTH: usize = 64;
/// Visible lines of words, mirroring the web's 3-line window.
const VISIBLE_LINES: usize = 3;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = centered(frame.area(), LINE_WIDTH as u16 + 4, 9);
    let [header, _, words_area, _, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(VISIBLE_LINES as u16),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(frame, app, header);
    draw_words(frame, app, words_area);

    let hint = Line::from(Span::styled(
        "tab restart · esc quit",
        Style::default().fg(app.theme.sub),
    ))
    .centered();
    frame.render_widget(Paragraph::new(hint), footer);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let session = &app.session;
    let theme = &app.theme;

    let progress = match (session.mode, session.state) {
        (TestMode::Time(secs), SessionState::Running | SessionState::Finished) => {
            let left = secs.saturating_sub(session.elapsed().as_secs());
            format!("{left}")
        }
        (TestMode::Time(secs), SessionState::NotStarted) => format!("{secs}"),
        (TestMode::Words(n), _) => format!("{}/{}", session.current, n),
    };

    let live_wpm = if session.state == SessionState::Running {
        let elapsed = session.elapsed().as_secs_f64();
        let correct = session.keystrokes.iter().filter(|k| k.correct).count() as u32;
        format!("{:>3.0}", stats::wpm(correct, elapsed.max(0.5)))
    } else {
        String::new()
    };

    let line = Line::from(vec![
        Span::styled(progress, Style::default().fg(theme.main)),
        Span::raw("   "),
        Span::styled(live_wpm, Style::default().fg(theme.sub)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// A word laid out on a display line, with its per-letter render states.
struct LaidOutWord {
    line: usize,
    letters: Vec<(char, LetterState)>,
    is_current: bool,
    caret_at: usize,
}

fn layout_words(app: &App) -> Vec<LaidOutWord> {
    let session = &app.session;
    let mut out = Vec::new();
    let mut line = 0usize;
    let mut col = 0usize;

    // Lay out from the start; the window selection below picks visible lines.
    // Only a bounded slice matters for display: stop a couple lines past the
    // current word's line to keep this O(window), not O(test length).
    let mut current_line = None;
    for idx in 0..session.target.len() {
        let letters = session.letter_states(idx);
        let width = letters.len();
        if col > 0 && col + width > LINE_WIDTH {
            line += 1;
            col = 0;
        }
        let is_current = idx == session.current;
        if is_current {
            current_line = Some(line);
        }
        out.push(LaidOutWord {
            line,
            letters,
            is_current,
            caret_at: session.typed[idx].chars().count(),
        });
        col += width + 1;
        if let Some(cl) = current_line {
            if line > cl + VISIBLE_LINES {
                break;
            }
        }
    }
    out
}

fn draw_words(frame: &mut Frame, app: &App, area: Rect) {
    let words = layout_words(app);
    let current_line = words
        .iter()
        .find(|w| w.is_current)
        .map(|w| w.line)
        .unwrap_or(0);
    // Keep the active line as the middle visible line once past the top.
    let first_visible = current_line.saturating_sub(1);

    let mut lines: Vec<Line> = Vec::with_capacity(VISIBLE_LINES);
    for line_idx in first_visible..first_visible + VISIBLE_LINES {
        let mut spans: Vec<Span> = Vec::new();
        for word in words.iter().filter(|w| w.line == line_idx) {
            for (i, &(c, state)) in word.letters.iter().enumerate() {
                let mut style = letter_style(state, &app.theme);
                if word.is_current && i == word.caret_at {
                    style = caret_style(&app.theme);
                }
                spans.push(Span::styled(c.to_string(), style));
            }
            // Caret sits after the last typed letter of the current word.
            if word.is_current && word.caret_at >= word.letters.len() {
                spans.push(Span::styled(" ", caret_style(&app.theme)));
            } else {
                spans.push(Span::raw(" "));
            }
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), area);
}
