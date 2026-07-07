//! Stat formulas ported from the web frontend
//! (frontend/src/ts/test/events/stats.ts and packages/util/src/numbers.ts).
//! Exact rounding parity is a Phase 2 task; formulas here match the source.

use super::{SessionState, TestSession};

/// charStats order matches CompletedEvent: [correctWordChars, incorrect, extra, missed].
#[derive(Debug, Clone, Default)]
pub struct FinalStats {
    pub wpm: f64,
    pub raw: f64,
    pub acc: f64,
    pub consistency: f64,
    pub key_consistency: f64,
    pub char_stats: [u32; 4],
    pub duration_s: f64,
    /// Raw speed of each whole second of the test ("burst" history).
    pub raw_per_second: Vec<f64>,
    /// Errors in each whole second of the test.
    pub err_per_second: Vec<u32>,
}

pub fn round_to2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
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
    let duration_s = session.elapsed().as_secs_f64().max(0.001);

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
    let count_partial = matches!(session.mode, super::TestMode::Time(_));
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

    // Keypress accuracy.
    let total_presses = session.keystrokes.len() as u32;
    let correct_presses = session.keystrokes.iter().filter(|k| k.correct).count() as u32;

    // Per-second buckets for burst/error history.
    let seconds = duration_s.ceil().max(1.0) as usize;
    let mut chars_per_second = vec![0u32; seconds];
    let mut err_per_second = vec![0u32; seconds];
    for k in &session.keystrokes {
        let bucket = (k.at.as_secs_f64().floor() as usize).min(seconds - 1);
        chars_per_second[bucket] += 1;
        if !k.correct {
            err_per_second[bucket] += 1;
        }
    }
    // Web burst history is Math.round()ed per second (getBurstHistory).
    let raw_per_second: Vec<f64> = chars_per_second
        .iter()
        .map(|&c| (f64::from(c) * 12.0).round())
        .collect();

    let spacing = &session.timings.key_spacing_ms;
    let spacing_head = &spacing[..spacing.len().saturating_sub(1)];

    FinalStats {
        wpm: round_to2(wpm(correct_word_chars + correct_spaces, duration_s)),
        raw: round_to2(raw_wpm(all_correct_chars + incorrect + extra, duration_s)),
        acc: round_to2(accuracy(correct_presses, total_presses)),
        consistency: round_to2(consistency(&raw_per_second)),
        key_consistency: round_to2(consistency(spacing_head)),
        char_stats: [correct_word_chars, incorrect, extra, missed],
        duration_s,
        raw_per_second,
        err_per_second,
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
    }
}
