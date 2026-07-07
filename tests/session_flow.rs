//! End-to-end engine test: a full words-mode session driven with controlled
//! timestamps, checked against hand-computed stats.

use std::time::{Duration, Instant};

use monkeytype_tui::engine::{stats, SessionState, TestMode, TestSession};
use monkeytype_tui::languages;
use ratatui::crossterm::event::KeyCode;

#[test]
fn full_words_test_produces_expected_stats() {
    let lang = languages::english();
    let mut session = TestSession::new(TestMode::Words(5), &lang, false);
    session.target = (0..5).map(|i| format!("word{i}")).collect();
    session.typed = vec![String::new(); 5];

    // Type all five words perfectly, one key every 100ms.
    let start = Instant::now();
    let mut t = start;
    let mut press = |session: &mut TestSession, c: char| {
        session.handle_key(KeyCode::Char(c), t);
        t += Duration::from_millis(100);
    };
    for i in 0..5 {
        for c in format!("word{i}").chars() {
            press(&mut session, c);
        }
        if i < 4 {
            press(&mut session, ' ');
        }
    }

    assert_eq!(session.state, SessionState::Finished);

    // Fix the duration so the assertions are deterministic.
    session.finished_duration = Some(Duration::from_secs(30));
    let s = stats::compute(&session);

    // 25 correct-word chars + 4 correct spaces -> 29 * 12 / 30 = 11.6 wpm;
    // like the web, charStats[0] and raw include the committed correct spaces.
    assert_eq!(s.wpm, 11.6);
    assert_eq!(s.raw, 11.6);
    assert_eq!(s.acc, 100.0);
    assert_eq!(s.char_stats, [29, 0, 0, 0]);
    assert_eq!(s.char_total, 29);
    assert_eq!(s.err_per_second.iter().sum::<u32>(), 0);
    // 29 keypresses -> 28 spacing samples, all 100ms -> perfect key consistency
    assert_eq!(session.timings.key_spacing_ms.len(), 28);
    assert_eq!(s.key_consistency, 100.0);
}

#[test]
fn errors_and_extras_are_counted() {
    let lang = languages::english();
    let mut session = TestSession::new(TestMode::Words(2), &lang, false);
    session.target = vec!["ab".into(), "cd".into()];
    session.typed = vec![String::new(); 2];

    let start = Instant::now();
    let mut t = start;
    let mut press = |session: &mut TestSession, c: char| {
        session.handle_key(KeyCode::Char(c), t);
        t += Duration::from_millis(50);
    };

    // word 1: "ax" (1 correct, 1 incorrect), advance with space (incorrect word)
    press(&mut session, 'a');
    press(&mut session, 'x');
    press(&mut session, ' ');
    // word 2: "cd" typed correctly ends the test on the last letter
    press(&mut session, 'c');
    press(&mut session, 'd');
    assert_eq!(session.state, SessionState::Finished);

    session.finished_duration = Some(Duration::from_secs(10));
    let s = stats::compute(&session);

    // charStats: [correctWordChars, incorrect, extra, missed]
    assert_eq!(s.char_stats, [2, 1, 0, 0]);
    // presses: a correct, x incorrect, space incorrect (word was wrong),
    // c correct, d correct -> 3 of 5 = 60%
    assert_eq!(s.acc, 60.0);
}
