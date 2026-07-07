use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Sparkline};
use ratatui::Frame;

use crate::app::App;
use crate::engine::stats::FinalStats;
use crate::ui::centered;

pub fn draw(frame: &mut Frame, app: &App, stats: &FinalStats) {
    let theme = &app.theme;
    let area = centered(frame.area(), 64, 12);
    let [wpm_row, acc_row, chars_row, _, chart_area, _, hint_row] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    let big = |label: &str, value: String| -> Line {
        Line::from(vec![
            Span::styled(format!("{label:>4} "), Style::default().fg(theme.sub)),
            Span::styled(value, Style::default().fg(theme.main)),
        ])
    };

    frame.render_widget(
        Paragraph::new(vec![
            big("wpm", format!("{:.2}", stats.wpm)),
            big("raw", format!("{:.2}", stats.raw)),
        ]),
        wpm_row,
    );
    frame.render_widget(
        Paragraph::new(big(
            "acc",
            format!(
                "{:.2}%  consistency {:.2}%  key {:.2}%",
                stats.acc, stats.consistency, stats.key_consistency
            ),
        )),
        acc_row,
    );

    let [correct, incorrect, extra, missed] = stats.char_stats;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("chars ", Style::default().fg(theme.sub)),
            Span::styled(
                format!("{correct}/{incorrect}/{extra}/{missed}"),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!(
                    "  errors {}  time {:.1}s",
                    stats.err_per_second.iter().sum::<u32>(),
                    stats.duration_s
                ),
                Style::default().fg(theme.sub),
            ),
        ])),
        chars_row,
    );

    let burst: Vec<u64> = stats.raw_per_second.iter().map(|&v| v as u64).collect();
    if !burst.is_empty() {
        frame.render_widget(
            Sparkline::default()
                .data(&burst)
                .style(Style::default().fg(theme.main)),
            chart_area,
        );
    }

    let hint = Line::from(Span::styled(
        "tab/enter next test · esc quit",
        Style::default().fg(theme.sub),
    ))
    .centered();
    frame.render_widget(Paragraph::new(hint), hint_row);
}
