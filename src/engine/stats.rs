//! Stat formulas ported from the web frontend
//! (frontend/src/ts/test/events/stats.ts and packages/util/src/numbers.ts),
//! with frontend-exact rounding: the backend anticheat cross-checks these
//! numbers against each other, so parity matters beyond display.

use super::{SessionState, TestMode, TestSession};

/// charStats order matches CompletedEvent: [correctWordChars, incorrect, extra, missed].
/// Like the web, correctWordChars includes the committed space after each
/// correct word - the same number feeds both wpm and charStats[0].
#[derive(Debug, Clone, Default)]
pub struct FinalStats {
    pub wpm: f64,
    pub raw: f64,
    pub acc: f64,
    pub consistency: f64,
    pub key_consistency: f64,
    pub wpm_consistency: f64,
    pub char_stats: [u32; 4],
    /// allCorrect + incorrect + extra (CompletedEvent charTotal).
    pub char_total: u32,
    pub duration_s: f64,
    /// Cumulative wpm at each timer boundary (chartData.wpm, integers).
    pub wpm_per_second: Vec<u32>,
    /// Raw speed within each timer bucket (chartData.burst, integers).
    pub burst_per_second: Vec<u32>,
    /// Incorrect keypresses within each timer bucket (chartData.err).
    pub err_per_second: Vec<u32>,
    /// roundTo2 ms from the last keystroke to the end of the test.
    pub last_key_to_end_ms: f64,
    /// ms from test start to the first keystroke - always 0 here, because a
    /// TUI test starts on the first keypress (the web arms its timer first).
    pub start_to_first_key_ms: f64,
    /// Whole timer buckets with no input activity (CompletedEvent afkDuration).
    pub afk_seconds: u32,
}

/// JS Math.round: half-up toward +infinity (Rust's .round() is half away from
/// zero, which differs for negatives).
fn js_round(y: f64) -> f64 {
    (y + 0.5).floor()
}

/// The web's roundTo2 from packages/util/src/numbers.ts:
/// Math.round((x + Number.EPSILON) * 100) / 100.
pub fn round_to2(x: f64) -> f64 {
    js_round((x + f64::EPSILON) * 100.0) / 100.0
}

/// Chart timer boundaries in ms, matching the web: one per whole elapsed
/// second, plus a fractional tail bucket for non-timed tests when the
/// remainder is at least half a second. Keystrokes past the last boundary are
/// dropped from chart data (they still count toward totals).
pub fn timer_boundaries(duration_s: f64, timed: bool) -> Vec<f64> {
    let mut out: Vec<f64> = (1..=duration_s.floor() as u64)
        .map(|i| (i * 1000) as f64)
        .collect();
    if !timed && duration_s - duration_s.floor() >= 0.5 {
        out.push(duration_s * 1000.0);
    }
    out
}

/// Convert a wpm value into the configured display unit (web's typing-speed
/// unit multipliers: cpm x5, wps /60, cps x5/60, wph x60).
pub fn convert_speed(wpm: f64, unit: crate::config::TypingSpeedUnit) -> f64 {
    use crate::config::TypingSpeedUnit as U;
    match unit {
        U::Wpm => wpm,
        U::Cpm => wpm * 5.0,
        U::Wps => wpm / 60.0,
        U::Cps => wpm * 5.0 / 60.0,
        U::Wph => wpm * 60.0,
    }
}

pub fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// Population standard deviation, matching packages/util/src/numbers.ts.
pub fn std_dev(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let m = mean(xs);
    (xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / xs.len() as f64).sqrt()
}

/// The web's "kogasa" consistency curve: 100*(1 - tanh(cov + cov^3/3 + cov^5/5)).
pub fn kogasa(cov: f64) -> f64 {
    100.0 * (1.0 - (cov + cov.powi(3) / 3.0 + cov.powi(5) / 5.0).tanh())
}

/// Consistency of a series: kogasa of its coefficient of variation.
pub fn consistency(xs: &[f64]) -> f64 {
    let m = mean(xs);
    if m == 0.0 {
        return 0.0;
    }
    kogasa(std_dev(xs) / m).clamp(0.0, 100.0)
}

pub fn wpm(correct_word_chars: u32, duration_s: f64) -> f64 {
    if duration_s <= 0.0 {
        return 0.0;
    }
    f64::from(correct_word_chars) * 12.0 / duration_s
}

pub fn raw_wpm(total_chars: u32, duration_s: f64) -> f64 {
    if duration_s <= 0.0 {
        return 0.0;
    }
    f64::from(total_chars) * 12.0 / duration_s
}

pub fn accuracy(correct_presses: u32, total_presses: u32) -> f64 {
    if total_presses == 0 {
        return 0.0;
    }
    f64::from(correct_presses) / f64::from(total_presses) * 100.0
}

pub fn compute(session: &TestSession) -> FinalStats {
    debug_assert_eq!(session.state, SessionState::Finished);
    // The web rounds the test duration to 2dp seconds (testDuration).
    let duration_s = round_to2(session.elapsed().as_secs_f64()).max(0.001);
    let timed = matches!(session.mode, TestMode::Time(_));

    // charStats, walked over every word the user touched.
    let mut correct_word_chars = 0u32;
    let mut incorrect = 0u32;
    let mut extra = 0u32;
    let mut missed = 0u32;
    let mut all_correct_chars = 0u32;
    let mut correct_spaces = 0u32;

    let touched = session
        .typed
        .iter()
        .rposition(|t| !t.is_empty())
        .map_or(0, |i| i + 1);
    // The web counts the partially-typed last word's correct prefix toward
    // correctWord in timed tests (countPartial in getChars).
    let count_partial = timed;
    for idx in 0..touched {
        let target: Vec<char> = session.target[idx].chars().collect();
        let typed: Vec<char> = session.typed[idx].chars().collect();
        for (i, &t) in target.iter().enumerate() {
            match typed.get(i) {
                None => {
                    // only count missed letters on words the user moved past
                    if idx < session.current {
                        missed += 1;
                    }
                }
                Some(&c) if c == t => all_correct_chars += 1,
                Some(_) => incorrect += 1,
            }
        }
        extra += typed.len().saturating_sub(target.len()) as u32;
        if !typed.is_empty() && typed == target {
            correct_word_chars += target.len() as u32;
            // a space follows every correct word except the final one typed
            if idx + 1 < touched || idx < session.current {
                correct_spaces += 1;
            }
        } else if idx + 1 == touched && count_partial {
            // correct prefix of the in-progress final word
            correct_word_chars += target
                .iter()
                .zip(typed.iter())
                .take_while(|(t, c)| t == c)
                .count() as u32;
        }
    }
    // The web's chars.correctWord and allCorrect both include the committed
    // correct spaces; wpm, raw, charStats and charTotal all build on them.
    let correct_word_chars = correct_word_chars + correct_spaces;
    let all_correct_chars = all_correct_chars + correct_spaces;

    // Keypress accuracy.
    let total_presses = session.keystrokes.len() as u32;
    let correct_presses = session.keystrokes.iter().filter(|k| k.correct).count() as u32;

    // Chart histories on the web's timer boundaries.
    let boundaries = timer_boundaries(duration_s, timed);
    let mut press_counts = vec![0u32; boundaries.len()];
    let mut err_per_second = vec![0u32; boundaries.len()];
    for k in &session.keystrokes {
        let at_ms = k.at.as_secs_f64() * 1000.0;
        // first boundary at or after the keystroke; past the last -> dropped
        if let Some(bucket) = boundaries.iter().position(|&b| at_ms <= b) {
            press_counts[bucket] += 1;
            if !k.correct {
                err_per_second[bucket] += 1;
            }
        }
    }
    let burst_per_second: Vec<u32> = press_counts
        .iter()
        .enumerate()
        .map(|(i, &presses)| {
            let start_ms = if i == 0 { 0.0 } else { boundaries[i - 1] };
            let interval_s = (boundaries[i] - start_ms) / 1000.0;
            js_round(f64::from(presses) * 12.0 / interval_s) as u32
        })
        .collect();

    // Cumulative wpm at each boundary from the progress samples.
    let wpm_per_second: Vec<u32> = boundaries
        .iter()
        .map(|&b| {
            let chars = session
                .progress_samples
                .iter()
                .rev()
                .find(|(at, _)| at.as_secs_f64() * 1000.0 <= b)
                .map_or(0, |&(_, chars)| chars);
            js_round(f64::from(chars) * 12.0 / (b / 1000.0)) as u32
        })
        .collect();

    // A bucket with no keystroke and no progress sample (backspaces produce
    // samples) had no input activity at all: afk.
    let afk_seconds = boundaries
        .iter()
        .enumerate()
        .filter(|&(i, &b)| {
            let start_ms = if i == 0 { 0.0 } else { boundaries[i - 1] };
            press_counts[i] == 0
                && !session.progress_samples.iter().any(|(at, _)| {
                    let ms = at.as_secs_f64() * 1000.0;
                    ms > start_ms && ms <= b
                })
        })
        .count() as u32;

    let last_key_to_end_ms = session.keystrokes.last().map_or(0.0, |k| {
        round_to2((duration_s * 1000.0 - k.at.as_secs_f64() * 1000.0).max(0.0))
    });

    let spacing = &session.timings.key_spacing_ms;
    let spacing_head = &spacing[..spacing.len().saturating_sub(1)];
    let burst_f64: Vec<f64> = burst_per_second.iter().map(|&v| f64::from(v)).collect();
    let wpm_f64: Vec<f64> = wpm_per_second.iter().map(|&v| f64::from(v)).collect();

    FinalStats {
        wpm: round_to2(wpm(correct_word_chars, duration_s)),
        raw: round_to2(raw_wpm(all_correct_chars + incorrect + extra, duration_s)),
        acc: round_to2(accuracy(correct_presses, total_presses)),
        consistency: round_to2(consistency(&burst_f64)),
        key_consistency: round_to2(consistency(spacing_head)),
        wpm_consistency: round_to2(consistency(&wpm_f64)),
        char_stats: [correct_word_chars, incorrect, extra, missed],
        char_total: all_correct_chars + incorrect + extra,
        duration_s,
        wpm_per_second,
        burst_per_second,
        err_per_second,
        last_key_to_end_ms,
        start_to_first_key_ms: 0.0,
        afk_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wpm_formula() {
        // 25 correct-word chars in 30s -> 25*12/30 = 10 wpm
        assert_eq!(wpm(25, 30.0), 10.0);
        // 300 chars in 60s -> 60 wpm
        assert_eq!(wpm(300, 60.0), 60.0);
    }

    #[test]
    fn raw_includes_errors_and_extras() {
        assert_eq!(raw_wpm(100 + 10 + 5, 60.0), 23.0);
    }

    #[test]
    fn accuracy_is_per_keypress() {
        assert_eq!(accuracy(90, 100), 90.0);
        assert_eq!(accuracy(0, 0), 0.0);
    }

    #[test]
    fn kogasa_at_zero_cov_is_100() {
        assert_eq!(kogasa(0.0), 100.0);
    }

    #[test]
    fn constant_series_is_perfectly_consistent() {
        assert_eq!(consistency(&[60.0, 60.0, 60.0, 60.0]), 100.0);
    }

    #[test]
    fn variable_series_is_less_consistent() {
        let c = consistency(&[30.0, 90.0, 30.0, 90.0]);
        assert!(c > 0.0 && c < 70.0, "got {c}");
    }

    #[test]
    fn population_std_dev() {
        let sd = std_dev(&[1.0, 2.0, 3.0]);
        assert!((sd - (2.0f64 / 3.0).sqrt()).abs() < 1e-12);
    }

    #[test]
    fn round_to2_matches_web_rounding() {
        assert_eq!(round_to2(92.456), 92.46);
        assert_eq!(round_to2(100.0), 100.0);
        // Number.EPSILON nudge makes exact halves round up like the web
        assert_eq!(round_to2(0.005), 0.01);
        assert_eq!(round_to2(26.905), 26.91);
    }

    #[test]
    fn timed_boundaries_are_whole_seconds() {
        assert_eq!(timer_boundaries(30.0, true).len(), 30);
        assert_eq!(timer_boundaries(30.0, true)[29], 30_000.0);
        // timed tests never get a fractional tail bucket
        assert_eq!(timer_boundaries(30.9, true).len(), 30);
    }

    #[test]
    fn untimed_tail_bucket_needs_half_second() {
        assert_eq!(timer_boundaries(34.3, false).len(), 34);
        let b = timer_boundaries(34.7, false);
        assert_eq!(b.len(), 35);
        assert!((b[34] - 34_700.0).abs() < 1e-9);
        // sub-second test still gets one bucket if it lasted >= 0.5s
        assert_eq!(timer_boundaries(0.8, false).len(), 1);
        assert_eq!(timer_boundaries(0.3, false).len(), 0);
    }
}
