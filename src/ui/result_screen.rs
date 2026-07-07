use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Sparkline};
use ratatui::Frame;

use crate::app::App;
use crate::engine::stats::FinalStats;
use crate::ui::{centered, format_speed};

pub fn draw(frame: &mut Frame, app: &App, stats: &FinalStats) {
    let theme = &app.theme;
    let config = &app.config;
    let unit = config.typing_speed_unit.as_str();
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
            Span::styled(
                format!("{label:>4} "),
                Style::default().fg(theme.sub.color()),
            ),
            Span::styled(value, Style::default().fg(theme.main.color())),
        ])
    };

    frame.render_widget(
        Paragraph::new(vec![
            big(unit, format_speed(stats.wpm, config)),
            big("raw", format_speed(stats.raw, config)),
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
            Span::styled("chars ", Style::default().fg(theme.sub.color())),
            Span::styled(
                format!("{correct}/{incorrect}/{extra}/{missed}"),
                Style::default().fg(theme.text.color()),
            ),
            Span::styled(
                format!(
                    "  errors {}  time {:.1}s",
                    stats.err_per_second.iter().sum::<u32>(),
                    stats.duration_s
                ),
                Style::default().fg(theme.sub.color()),
            ),
        ])),
        chars_row,
    );

    let burst: Vec<u64> = stats.raw_per_second.iter().map(|&v| v as u64).collect();
    if !burst.is_empty() {
        frame.render_widget(
            Sparkline::default()
                .data(&burst)
                .style(Style::default().fg(theme.main.color())),
            chart_area,
        );
    }

    if config.show_key_tips {
        let hint = Line::from(Span::styled(
            "tab/enter next test · esc menu",
            Style::default().fg(theme.sub.color()),
        ))
        .centered();
        frame.render_widget(Paragraph::new(hint), hint_row);
    }
}
