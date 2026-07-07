use monkeytype_tui::{app, engine};
use ratatui::crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::{execute, terminal::supports_keyboard_enhancement};

fn value_of<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
}

fn parse_mode() -> Option<engine::TestMode> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(n) = value_of::<usize>(&args, "--words") {
        Some(engine::TestMode::Words(n))
    } else {
        value_of::<u64>(&args, "--time").map(engine::TestMode::Time)
    }
}

fn main() -> std::io::Result<()> {
    let mode = parse_mode();
    let mut terminal = ratatui::init();

    // Kitty keyboard protocol gives us key-release events, which are the only
    // way to measure keyDuration/keyOverlap in a terminal (see PLAN.md).
    let key_release_supported = supports_keyboard_enhancement().unwrap_or(false);
    if key_release_supported {
        execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            )
        )?;
    }
    // Bracketed paste delivers a pasted OAuth redirect URL as one event instead
    // of hundreds of key events (see login screen).
    let _ = execute!(std::io::stdout(), EnableBracketedPaste);

    let result = app::App::new(mode, key_release_supported).run(&mut terminal);

    let _ = execute!(std::io::stdout(), DisableBracketedPaste);
    if key_release_supported {
        let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}
