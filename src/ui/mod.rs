pub mod keymap;
pub mod menu_screen;
pub mod result_screen;
pub mod settings_screen;
pub mod test_screen;
pub mod theme_screen;

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::theme::Theme;

/// Fill the whole frame with the theme background color.
pub fn draw_background(frame: &mut Frame, theme: &Theme) {
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg.color())),
        frame.area(),
    );
}

/// Center a `width` x `height` box inside `area`.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

/// Format a speed value per config: unit conversion + decimal places.
pub fn format_speed(wpm: f64, config: &crate::config::Config) -> String {
    let v = crate::engine::stats::convert_speed(wpm, config.typing_speed_unit);
    if config.always_show_decimal_places {
        format!("{v:.2}")
    } else {
        format!("{}", v.round())
    }
}
