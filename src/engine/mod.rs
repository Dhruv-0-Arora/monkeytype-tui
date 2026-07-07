pub mod input;
pub mod stats;
pub mod timing;
pub mod words;

use std::time::{Duration, Instant};

use crate::languages::LanguageData;
use crate::theme::LetterState;
use timing::KeystrokeTimings;
use words::WordGenerator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestMode {
    Time(u64),
    Words(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    NotStarted,
    Running,
    Finished,
}

/// One recorded keypress, used to build per-second chart data.
pub struct Keystroke {
    pub at: Duration,
    pub correct: bool,
}

/// State machine for one active typing test. Input transitions live in
/// `input.rs`, final stat computation in `stats.rs`.
pub struct TestSession {
    pub mode: TestMode,
    pub target: Vec<String>,
    pub typed: Vec<String>,
    pub current: usize,
    pub state: SessionState,
    pub timings: KeystrokeTimings,
    pub keystrokes: Vec<Keystroke>,
    pub started_at: Option<Instant>,
    pub finished_duration: Option<Duration>,
    /// config.quickEnd: end the test at full length of the last word even if
    /// it has errors (a correct last word always ends the test, like the web).
    pub quick_end: bool,
    generator: WordGenerator,
}

/// How many words to keep ahead of the caret in time mode.
const TIME_MODE_BUFFER: usize = 60;

impl TestSession {
    pub fn new(mode: TestMode, lang: &LanguageData, release_events_available: bool) -> Self {
        let mut generator = WordGenerator::new(lang);
        let count = match mode {
            TestMode::Time(_) => TIME_MODE_BUFFER,
            TestMode::Words(n) => n,
        };
        let target = generator.next_words(count);
        let typed = vec![String::new(); target.len()];
        Self {
            mode,
            target,
            typed,
            current: 0,
            state: SessionState::NotStarted,
            timings: KeystrokeTimings::new(release_events_available),
            keystrokes: Vec::new(),
            started_at: None,
            finished_duration: None,
            quick_end: false,
            generator,
        }
    }

    pub fn elapsed(&self) -> Duration {
        match (self.state, self.started_at) {
            (SessionState::Finished, _) => self.finished_duration.unwrap_or_default(),
            (_, Some(started)) => started.elapsed(),
            _ => Duration::ZERO,
        }
    }

    /// Called every UI tick. Ends a time-mode test when the clock runs out.
    pub fn tick(&mut self) {
        if self.state != SessionState::Running {
            return;
        }
        if let TestMode::Time(seconds) = self.mode {
            if self.elapsed() >= Duration::from_secs(seconds) {
                self.finish(Duration::from_secs(seconds));
            }
        }
    }

    pub fn finish(&mut self, duration: Duration) {
        self.state = SessionState::Finished;
        self.finished_duration = Some(duration);
    }

    /// Keep the rolling word buffer ahead of the caret in time mode.
    pub fn extend_if_needed(&mut self) {
        if !matches!(self.mode, TestMode::Time(_)) {
            return;
        }
        if self.current + TIME_MODE_BUFFER / 2 >= self.target.len() {
            let more = self.generator.next_words(TIME_MODE_BUFFER / 2);
            self.typed.extend(vec![String::new(); more.len()]);
            self.target.extend(more);
        }
    }

    /// Per-letter render states for word `idx`, including extra letters.
    pub fn letter_states(&self, idx: usize) -> Vec<(char, LetterState)> {
        let target: Vec<char> = self.target[idx].chars().collect();
        let typed: Vec<char> = self.typed[idx].chars().collect();
        let mut out = Vec::with_capacity(target.len().max(typed.len()));
        for (i, &t) in target.iter().enumerate() {
            let state = match typed.get(i) {
                None => LetterState::Untyped,
                Some(&c) if c == t => LetterState::Correct,
                Some(_) => LetterState::Incorrect,
            };
            out.push((t, state));
        }
        for &c in typed.iter().skip(target.len()) {
            out.push((c, LetterState::Extra));
        }
        out
    }

    pub fn word_is_correct(&self, idx: usize) -> bool {
        self.typed[idx] == self.target[idx]
    }
}
