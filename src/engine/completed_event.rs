//! Assemble the CompletedEvent posted to `POST /results`, mirroring the web's
//! buildCompletedEvent + saveResult (frontend/src/ts/test/test-logic.ts). The
//! backend schema is strict: exactly these keys, no extras. The hash covers
//! the whole object (uid included, "toolong" substitution applied) minus only
//! the `hash` key itself, using the object-hash port.

use serde_json::{json, Map, Value};

use crate::config::Config;
use crate::engine::stats::FinalStats;
use crate::engine::{TestMode, TestSession};
use crate::objecthash;

/// testDuration above which chartData/keySpacing/keyDuration post as "toolong".
const TOO_LONG_S: f64 = 122.0;

/// A JSON number that stays an integer on the wire when integral (the web
/// posts 30, not 30.0). Integer and float forms hash identically (both go
/// through f64 in the objecthash port), so this only affects body cosmetics.
fn json_num(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        json!(x as i64)
    } else {
        json!(x)
    }
}

/// Build the full CompletedEvent for a finished session, hash included.
pub fn build(session: &TestSession, stats: &FinalStats, config: &Config, uid: &str) -> Value {
    let (mode, mode2) = match session.mode {
        TestMode::Time(t) => ("time", t.to_string()),
        TestMode::Words(n) => ("words", n.to_string()),
    };

    let too_long = stats.duration_s > TOO_LONG_S;
    let chart_data = if too_long {
        json!("toolong")
    } else {
        json!({
            "wpm": stats.wpm_per_second,
            "burst": stats.burst_per_second,
            "err": stats.err_per_second,
        })
    };
    let key_spacing = if too_long {
        json!("toolong")
    } else {
        Value::Array(
            session
                .timings
                .key_spacing_ms
                .iter()
                .map(|&ms| json_num(ms))
                .collect(),
        )
    };
    // Legacy terminals have no key-release events: keyDuration stays empty and
    // keyOverlap 0 - honest degradation, never fabricated (PLAN.md).
    let key_duration = if too_long {
        json!("toolong")
    } else {
        Value::Array(
            session
                .timings
                .key_duration_ms
                .iter()
                .map(|&ms| json_num(ms))
                .collect(),
        )
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut map = Map::new();
    let mut put = |k: &str, v: Value| {
        map.insert(k.to_string(), v);
    };
    put("acc", json_num(stats.acc));
    put("afkDuration", json!(stats.afk_seconds));
    put("bailedOut", json!(false)); // TODO(phase5): bailout accounting
    put("blindMode", json!(config.blind_mode));
    put("charStats", json!(stats.char_stats));
    put("charTotal", json!(stats.char_total));
    put("chartData", chart_data);
    put("consistency", json_num(stats.consistency));
    // TODO(phase5): expert/master fail conditions are not implemented in the
    // engine yet, so every completed test really ran at normal difficulty.
    put("difficulty", json!("normal"));
    put("funbox", json!([])); // TODO(phase5)
    put("incompleteTestSeconds", json!(0)); // TODO(phase5)
    put("incompleteTests", json!([])); // TODO(phase5)
    put("keyConsistency", json_num(stats.key_consistency));
    put("keyDuration", key_duration);
    put(
        "keyOverlap",
        json_num(crate::engine::stats::round_to2(
            session.timings.key_overlap_ms,
        )),
    );
    put("keySpacing", key_spacing);
    // TODO(phase5): the generator only serves bundled english today.
    put("language", json!("english"));
    put("lastKeyToEnd", json_num(stats.last_key_to_end_ms));
    put("lazyMode", json!(false)); // TODO(phase5): lazy folding not wired
    put("mode", json!(mode));
    put("mode2", json!(mode2));
    put("numbers", json!(false)); // TODO(phase5): numbers gen not wired
    put("punctuation", json!(false)); // TODO(phase5): punctuation gen not wired
    put("rawWpm", json_num(stats.raw));
    put("restartCount", json!(0)); // TODO(phase5): restart accounting
    put("startToFirstKey", json_num(stats.start_to_first_key_ms));
    put("stopOnLetter", json!(false)); // TODO(phase5): stopOnError not wired
    put("tags", json!([])); // TODO(phase5)
    put("testDuration", json_num(stats.duration_s));
    put("timestamp", json!(timestamp));
    put("uid", json!(uid));
    put("wpm", json_num(stats.wpm));
    put("wpmConsistency", json_num(stats.wpm_consistency));

    let hash = objecthash::object_hash(&Value::Object(map.clone()));
    map.insert("hash".to_string(), json!(hash));
    Value::Object(map)
}

/// Client-side pre-gate mirroring the web's dontSave checks: skip submissions
/// the server is guaranteed to reject.
pub fn submission_block_reason(session: &TestSession, stats: &FinalStats) -> Option<String> {
    match session.mode {
        TestMode::Time(t) if t < 15 => Some("test too short - time tests need 15s+".into()),
        TestMode::Words(n) if n < 10 => Some("test too short - words tests need 10+ words".into()),
        _ => None,
    }
    .or_else(|| {
        if stats.acc < 75.0 {
            Some("accuracy below 75% - the server rejects these".into())
        } else if stats.wpm > 350.0 {
            Some("wpm above 350 - the server rejects these".into())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{stats, SessionState, TestSession};
    use crate::languages;
    use ratatui::crossterm::event::KeyCode;
    use std::time::{Duration, Instant};

    /// Every key the strict backend schema accepts for a time/words test.
    const SCHEMA_KEYS: [&str; 34] = [
        "acc",
        "afkDuration",
        "bailedOut",
        "blindMode",
        "charStats",
        "charTotal",
        "chartData",
        "consistency",
        "difficulty",
        "funbox",
        "hash",
        "incompleteTestSeconds",
        "incompleteTests",
        "keyConsistency",
        "keyDuration",
        "keyOverlap",
        "keySpacing",
        "language",
        "lastKeyToEnd",
        "lazyMode",
        "mode",
        "mode2",
        "numbers",
        "punctuation",
        "rawWpm",
        "restartCount",
        "startToFirstKey",
        "stopOnLetter",
        "tags",
        "testDuration",
        "timestamp",
        "uid",
        "wpm",
        "wpmConsistency",
    ];

    fn scripted_session(word_count: usize, key_interval: Duration) -> TestSession {
        let lang = languages::english();
        let mut session = TestSession::new(TestMode::Words(word_count), &lang, false);
        session.target = (0..word_count).map(|i| format!("word{i}")).collect();
        session.typed = vec![String::new(); word_count];
        let start = Instant::now();
        let mut t = start;
        for i in 0..word_count {
            for c in format!("word{i}").chars() {
                session.handle_key(KeyCode::Char(c), t);
                t += key_interval;
            }
            if i + 1 < word_count {
                session.handle_key(KeyCode::Char(' '), t);
                t += key_interval;
            }
        }
        assert_eq!(session.state, SessionState::Finished);
        session
    }

    #[test]
    fn builds_exactly_the_schema_keys() {
        let mut session = scripted_session(10, Duration::from_millis(100));
        session.finished_duration = Some(Duration::from_secs(20));
        let s = stats::compute(&session);
        let event = build(&session, &s, &crate::config::Config::default(), "uid123");
        let map = event.as_object().expect("object");
        let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = SCHEMA_KEYS.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected);
        assert_eq!(map["mode"], "words");
        assert_eq!(map["mode2"], "10");
        assert_eq!(map["uid"], "uid123");
        // legacy terminal: keyDuration empty, keyOverlap 0, never fabricated
        assert_eq!(map["keyDuration"], serde_json::json!([]));
        assert_eq!(map["keyOverlap"], serde_json::json!(0));
    }

    #[test]
    fn hash_covers_everything_but_hash() {
        let mut session = scripted_session(10, Duration::from_millis(100));
        session.finished_duration = Some(Duration::from_secs(20));
        let s = stats::compute(&session);
        let event = build(&session, &s, &crate::config::Config::default(), "uid123");
        let mut without_hash = event.as_object().unwrap().clone();
        let hash = without_hash.remove("hash").unwrap();
        assert_eq!(
            hash.as_str().unwrap(),
            crate::objecthash::object_hash(&serde_json::Value::Object(without_hash))
        );
    }

    #[test]
    fn long_tests_post_toolong_strings() {
        let mut session = scripted_session(10, Duration::from_millis(100));
        session.finished_duration = Some(Duration::from_secs_f64(123.45));
        let s = stats::compute(&session);
        assert!(s.duration_s > 122.0);
        let event = build(&session, &s, &crate::config::Config::default(), "uid123");
        assert_eq!(event["chartData"], "toolong");
        assert_eq!(event["keySpacing"], "toolong");
        assert_eq!(event["keyDuration"], "toolong");
    }

    #[test]
    fn block_reasons_match_server_gates() {
        let mut session = scripted_session(10, Duration::from_millis(100));
        session.finished_duration = Some(Duration::from_secs(20));
        let s = stats::compute(&session);
        assert_eq!(submission_block_reason(&session, &s), None);

        let mut short = scripted_session(5, Duration::from_millis(100));
        short.finished_duration = Some(Duration::from_secs(5));
        let s_short = stats::compute(&short);
        assert!(submission_block_reason(&short, &s_short)
            .expect("blocked")
            .contains("too short"));

        let mut low_acc = s.clone();
        low_acc.acc = 60.0;
        assert!(submission_block_reason(&session, &low_acc)
            .expect("blocked")
            .contains("accuracy"));
    }
}
