use std::time::Instant;

use ratatui::crossterm::event::KeyCode;

use super::{Keystroke, SessionState, TestMode, TestSession};

/// Outcome of feeding one key into the session, for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputOutcome {
    Ignored,
    Accepted,
    Finished,
}

impl TestSession {
    /// Feed a key press. Only printable chars, space, and backspace matter;
    /// everything else is the caller's business (restart, quit, ...).
    pub fn handle_key(&mut self, code: KeyCode, now: Instant) -> InputOutcome {
        if self.state == SessionState::Finished {
            return InputOutcome::Ignored;
        }
        match code {
            KeyCode::Char(' ') => self.handle_space(now),
            KeyCode::Char(c) => self.handle_char(c, now),
            KeyCode::Backspace => self.handle_backspace(now),
            _ => InputOutcome::Ignored,
        }
    }

    pub fn handle_release(&mut self, code: KeyCode, now: Instant) {
        if self.state == SessionState::Running {
            self.timings.on_release(code, now);
        }
    }

    fn start_if_needed(&mut self, now: Instant) {
        if self.state == SessionState::NotStarted {
            self.state = SessionState::Running;
            self.started_at = Some(now);
        }
    }

    fn record(&mut self, code: KeyCode, correct: bool, now: Instant) {
        self.timings.on_press(code, now);
        let at = now.duration_since(self.started_at.expect("session started"));
        self.keystrokes.push(Keystroke { at, correct });
    }

    fn handle_char(&mut self, c: char, now: Instant) -> InputOutcome {
        self.start_if_needed(now);
        let target: Vec<char> = self.target[self.current].chars().collect();
        let pos = self.typed[self.current].chars().count();

        // Match the web's extra-letter cap so words can't grow unboundedly.
        if pos >= target.len() + 20 {
            return InputOutcome::Ignored;
        }

        let correct = target.get(pos) == Some(&c);
        self.typed[self.current].push(c);
        self.record(KeyCode::Char(c), correct, now);
        self.sample_progress(now);

        // A correctly completed final word always ends the test; with
        // quickEnd it also ends at full length even if the word has errors.
        if self.is_last_word() {
            let full_length = self.typed[self.current].chars().count()
                >= self.target[self.current].chars().count();
            if self.word_is_correct(self.current) || (self.quick_end && full_length) {
                let elapsed = self.elapsed();
                self.finish(elapsed);
                return InputOutcome::Finished;
            }
        }
        InputOutcome::Accepted
    }

    fn handle_space(&mut self, now: Instant) -> InputOutcome {
        // Leading space on an empty word is ignored, like the web.
        if self.state == SessionState::NotStarted || self.typed[self.current].is_empty() {
            return InputOutcome::Ignored;
        }
        let correct = self.word_is_correct(self.current);
        self.record(KeyCode::Char(' '), correct, now);

        if self.is_last_word() {
            self.sample_progress(now);
            let elapsed = self.elapsed();
            self.finish(elapsed);
            return InputOutcome::Finished;
        }
        self.current += 1;
        self.extend_if_needed();
        self.sample_progress(now);
        InputOutcome::Accepted
    }

    fn handle_backspace(&mut self, now: Instant) -> InputOutcome {
        if self.state != SessionState::Running {
            return InputOutcome::Ignored;
        }
        if !self.typed[self.current].is_empty() {
            self.typed[self.current].pop();
            self.sample_progress(now);
            return InputOutcome::Accepted;
        }
        // Move back to the previous word only if it was left incorrect,
        // matching the web's default (freedomMode changes this in Phase 2).
        if self.current > 0 && !self.word_is_correct(self.current - 1) {
            self.current -= 1;
            self.sample_progress(now);
            return InputOutcome::Accepted;
        }
        InputOutcome::Ignored
    }

    fn is_last_word(&self) -> bool {
        match self.mode {
            TestMode::Words(_) => self.current + 1 == self.target.len(),
            TestMode::Time(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages;

    fn words_session(n: usize) -> TestSession {
        let lang = languages::english();
        let mut session = TestSession::new(TestMode::Words(n), &lang, false);
        // deterministic targets for assertions
        session.target = (0..n).map(|i| format!("word{i}")).collect();
        session.typed = vec![String::new(); n];
        session
    }

    fn type_str(session: &mut TestSession, s: &str) -> InputOutcome {
        let mut out = InputOutcome::Ignored;
        for c in s.chars() {
            out = session.handle_key(KeyCode::Char(c), Instant::now());
        }
        out
    }

    #[test]
    fn typing_a_word_and_space_advances() {
        let mut s = words_session(3);
        type_str(&mut s, "word0");
        assert!(s.word_is_correct(0));
        assert_eq!(
            s.handle_key(KeyCode::Char(' '), Instant::now()),
            InputOutcome::Accepted
        );
        assert_eq!(s.current, 1);
    }

    #[test]
    fn leading_space_is_ignored() {
        let mut s = words_session(2);
        type_str(&mut s, "wo");
        s.handle_key(KeyCode::Char(' '), Instant::now());
        assert_eq!(s.current, 1);
        assert_eq!(
            s.handle_key(KeyCode::Char(' '), Instant::now()),
            InputOutcome::Ignored
        );
        assert_eq!(s.current, 1);
    }

    #[test]
    fn backspace_edits_current_word_only_when_nonempty() {
        let mut s = words_session(2);
        type_str(&mut s, "wx");
        s.handle_key(KeyCode::Backspace, Instant::now());
        assert_eq!(s.typed[0], "w");
    }

    #[test]
    fn backspace_returns_to_incorrect_previous_word_only() {
        let mut s = words_session(3);
        type_str(&mut s, "wrong");
        s.handle_key(KeyCode::Char(' '), Instant::now());
        assert_eq!(s.current, 1);
        // previous word incorrect -> allowed back
        assert_eq!(
            s.handle_key(KeyCode::Backspace, Instant::now()),
            InputOutcome::Accepted
        );
        assert_eq!(s.current, 0);

        // fix it, advance, then backspace must NOT go back
        s.typed[0].clear();
        type_str(&mut s, "word0");
        s.handle_key(KeyCode::Char(' '), Instant::now());
        assert_eq!(s.current, 1);
        assert_eq!(
            s.handle_key(KeyCode::Backspace, Instant::now()),
            InputOutcome::Ignored
        );
        assert_eq!(s.current, 1);
    }

    #[test]
    fn finishing_last_word_ends_words_mode() {
        let mut s = words_session(2);
        type_str(&mut s, "word0");
        s.handle_key(KeyCode::Char(' '), Instant::now());
        let out = type_str(&mut s, "word1");
        assert_eq!(out, InputOutcome::Finished);
        assert_eq!(s.state, SessionState::Finished);
    }

    #[test]
    fn extra_letters_recorded_as_incorrect() {
        let mut s = words_session(2);
        type_str(&mut s, "word0xx");
        assert_eq!(s.typed[0], "word0xx");
        let states = s.letter_states(0);
        assert_eq!(states.len(), 7);
        assert_eq!(states[5].1, crate::theme::LetterState::Extra);
        // 5 correct presses + 2 extra (incorrect) presses
        let correct = s.keystrokes.iter().filter(|k| k.correct).count();
        assert_eq!(correct, 5);
        assert_eq!(s.keystrokes.len(), 7);
    }
}
