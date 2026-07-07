use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::event::{KeyCode, MouseEventKind};
use ratatui::{DefaultTerminal, Frame};

use crate::engine::stats::FinalStats;
use crate::engine::{SessionState, TestMode, TestSession};
use crate::languages::{self, LanguageData};
use crate::theme::Theme;
use crate::ui;

const TICK: Duration = Duration::from_millis(33);

/// Which screen is showing. Later phases add Login, Settings, ThemePicker.
pub enum Screen {
    Test,
    Result(FinalStats),
}

pub struct App {
    pub screen: Screen,
    pub session: TestSession,
    pub theme: Theme,
    pub mode: TestMode,
    pub language: LanguageData,
    pub key_release_supported: bool,
    should_quit: bool,
}

impl App {
    pub fn new(mode: TestMode, key_release_supported: bool) -> Self {
        let language = languages::english();
        let session = TestSession::new(mode, &language, key_release_supported);
        Self {
            screen: Screen::Test,
            session,
            theme: Theme::fallback(),
            mode,
            language,
            key_release_supported,
            should_quit: false,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        while !self.should_quit {
            terminal.draw(|f| self.draw(f))?;
            if event::poll(TICK)? {
                match event::read()? {
                    Event::Key(key) => self.on_key(key),
                    Event::Mouse(m) if m.kind == MouseEventKind::Moved => {}
                    _ => {}
                }
            }
            self.on_tick();
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        match &self.screen {
            Screen::Test => ui::test_screen::draw(frame, self),
            Screen::Result(stats) => ui::result_screen::draw(frame, self, stats),
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        let now = Instant::now();

        if key.kind == KeyEventKind::Release {
            self.session.handle_release(key.code, now);
            return;
        }

        // Global bindings.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.should_quit = true;
                return;
            }
            KeyCode::Tab => {
                self.restart();
                return;
            }
            _ => {}
        }

        match &self.screen {
            Screen::Test => {
                self.session.handle_key(key.code, now);
                self.check_finished();
            }
            Screen::Result(_) => {
                if key.code == KeyCode::Enter {
                    self.restart();
                }
            }
        }
    }

    fn on_tick(&mut self) {
        if matches!(self.screen, Screen::Test) {
            self.session.tick();
            self.check_finished();
        }
    }

    fn check_finished(&mut self) {
        if self.session.state == SessionState::Finished {
            let stats = crate::engine::stats::compute(&self.session);
            self.screen = Screen::Result(stats);
        }
    }

    fn restart(&mut self) {
        self.session = TestSession::new(self.mode, &self.language, self.key_release_supported);
        self.screen = Screen::Test;
    }
}
