use std::collections::HashMap;
use std::time::Instant;

use ratatui::crossterm::event::KeyCode;

/// Captures the keystroke timing data the backend wants for anticheat.
///
/// `key_spacing` (press-to-press deltas) works on every terminal.
/// `key_duration` and `key_overlap` need key-release events, which only
/// kitty-protocol terminals deliver. On legacy terminals those stay empty;
/// we never fabricate timing data (see PLAN.md).
pub struct KeystrokeTimings {
    pub key_spacing_ms: Vec<f64>,
    pub key_duration_ms: Vec<f64>,
    pub key_overlap_ms: f64,
    pub release_events_available: bool,
    last_press: Option<Instant>,
    held: HashMap<KeyCode, Instant>,
    overlap_since: Option<Instant>,
}

impl KeystrokeTimings {
    pub fn new(release_events_available: bool) -> Self {
        Self {
            key_spacing_ms: Vec::new(),
            key_duration_ms: Vec::new(),
            key_overlap_ms: 0.0,
            release_events_available,
            last_press: None,
            held: HashMap::new(),
            overlap_since: None,
        }
    }

    pub fn on_press(&mut self, code: KeyCode, now: Instant) {
        if let Some(last) = self.last_press {
            self.key_spacing_ms
                .push(now.duration_since(last).as_secs_f64() * 1000.0);
        }
        self.last_press = Some(now);

        if self.release_events_available {
            self.held.entry(code).or_insert(now);
            if self.held.len() >= 2 && self.overlap_since.is_none() {
                self.overlap_since = Some(now);
            }
        }
    }

    pub fn on_release(&mut self, code: KeyCode, now: Instant) {
        if let Some(pressed_at) = self.held.remove(&code) {
            self.key_duration_ms
                .push(now.duration_since(pressed_at).as_secs_f64() * 1000.0);
        }
        if self.held.len() < 2 {
            if let Some(since) = self.overlap_since.take() {
                self.key_overlap_ms += now.duration_since(since).as_secs_f64() * 1000.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn spacing_is_press_to_press() {
        let start = Instant::now();
        let mut t = KeystrokeTimings::new(false);
        t.on_press(KeyCode::Char('a'), start);
        t.on_press(KeyCode::Char('b'), start + Duration::from_millis(100));
        t.on_press(KeyCode::Char('c'), start + Duration::from_millis(250));
        assert_eq!(t.key_spacing_ms.len(), 2);
        assert!((t.key_spacing_ms[0] - 100.0).abs() < 1.0);
        assert!((t.key_spacing_ms[1] - 150.0).abs() < 1.0);
    }

    #[test]
    fn duration_and_overlap_need_release_events() {
        let start = Instant::now();
        let mut t = KeystrokeTimings::new(true);
        t.on_press(KeyCode::Char('a'), start);
        t.on_press(KeyCode::Char('b'), start + Duration::from_millis(20));
        t.on_release(KeyCode::Char('a'), start + Duration::from_millis(60));
        t.on_release(KeyCode::Char('b'), start + Duration::from_millis(80));
        assert_eq!(t.key_duration_ms.len(), 2);
        assert!((t.key_duration_ms[0] - 60.0).abs() < 1.0);
        assert!((t.key_duration_ms[1] - 60.0).abs() < 1.0);
        // both keys were held together from t=20 to t=60
        assert!((t.key_overlap_ms - 40.0).abs() < 1.0);
    }

    #[test]
    fn legacy_terminal_records_no_durations() {
        let start = Instant::now();
        let mut t = KeystrokeTimings::new(false);
        t.on_press(KeyCode::Char('a'), start);
        t.on_release(KeyCode::Char('a'), start + Duration::from_millis(50));
        assert!(t.key_duration_ms.is_empty());
        assert_eq!(t.key_overlap_ms, 0.0);
    }
}
