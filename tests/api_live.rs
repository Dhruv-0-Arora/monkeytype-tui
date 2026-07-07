//! Live Phase 4 smoke test against the real monkeytype API, using the
//! dedicated throwaway account ONLY (never a real account - it posts results
//! and deliberately provokes rejections). Ignored by default; run with:
//!
//!   set -a; source .env.local; set +a
//!   cargo test --test api_live -- --ignored --nocapture
//!
//! Flow: login -> GET /users -> drive a scripted ~60wpm words-25 session
//! (modest speed stays clear of the >130wpm key-data anticheat path) ->
//! POST /results -> GET /results/last cross-check -> immediate resubmit to
//! provoke 466/462 -> a 5s time-mode event to provoke 460.
//!
//! Note: a passing POST does not prove object-hash parity - the server's hash
//! check is config-gated. The committed vector suite is the parity proof.

use std::time::{Duration, Instant};

use monkeytype_tui::api::{ApeClient, ApiError};
use monkeytype_tui::auth::{AuthManager, TokenStore};
use monkeytype_tui::config::Config;
use monkeytype_tui::engine::{completed_event, stats, SessionState, TestMode, TestSession};
use monkeytype_tui::languages;
use ratatui::crossterm::event::KeyCode;
use std::sync::Arc;

fn creds() -> Option<(String, String)> {
    let email = std::env::var("MONKEYTYPE_TEST_EMAIL").ok()?;
    let password = std::env::var("MONKEYTYPE_TEST_PASSWORD").ok()?;
    Some((email, password))
}

/// Type real generated words at ~60wpm with human-ish jitter.
fn run_scripted_session(mode: TestMode) -> TestSession {
    let lang = languages::english();
    let mut session = TestSession::new(mode, &lang, false);
    let start = Instant::now();
    let mut t = start;
    let mut i: u64 = 0;
    let mut interval = || {
        // 180-220ms per key (~60wpm), deterministic jitter
        i += 1;
        Duration::from_millis(180 + (i * 13) % 41)
    };
    loop {
        let word = session.target[session.current].clone();
        let already: usize = session.typed[session.current].chars().count();
        for c in word.chars().skip(already) {
            session.handle_key(KeyCode::Char(c), t);
            t += interval();
            if session.state == SessionState::Finished {
                return session;
            }
        }
        session.handle_key(KeyCode::Char(' '), t);
        t += interval();
        if session.state == SessionState::Finished {
            return session;
        }
        if let TestMode::Time(secs) = mode {
            if t.duration_since(start) >= Duration::from_secs(secs) {
                session.finish(Duration::from_secs(secs));
                return session;
            }
        }
    }
}

#[test]
#[ignore = "needs network and real credentials"]
fn live_result_submission_roundtrip() {
    let (email, password) = creds().expect("set MONKEYTYPE_TEST_EMAIL / _PASSWORD");
    let tmp = std::env::temp_dir().join(format!("mttui-apilive-{}.json", std::process::id()));
    let account = format!("test-api-live-{}", std::process::id());
    let auth = Arc::new(AuthManager::new(TokenStore::with_account(
        tmp.clone(),
        account,
    )));
    let mut session = auth
        .login_email(&email, &password)
        .expect("login should succeed");
    let client = ApeClient::new(auth.clone());

    // Sanity: the token works.
    let user = client.get_user(&mut session).expect("GET /users");
    println!(
        "logged in as {}",
        user.get("name").and_then(|n| n.as_str()).unwrap_or("?")
    );

    // A words-25 test at ~60wpm.
    let test = run_scripted_session(TestMode::Words(25));
    let s = stats::compute(&test);
    println!(
        "scripted test: wpm {} acc {} duration {}s",
        s.wpm, s.acc, s.duration_s
    );
    assert!(s.wpm < 130.0, "stay under the anticheat key-data threshold");
    assert_eq!(
        completed_event::submission_block_reason(&test, &s),
        None,
        "scripted test must be submittable"
    );

    let event = completed_event::build(&test, &s, &Config::default(), &session.uid);
    let posted = client
        .post_result(&mut session, &event)
        .expect("POST /results should accept the scripted result");
    println!(
        "saved: insertedId {} xp {} isPb {}",
        posted.inserted_id, posted.xp, posted.is_pb
    );

    // The saved result comes back as the most recent one.
    let last = client
        .get_last_result(&mut session)
        .expect("GET /results/last");
    assert_eq!(last["wpm"], event["wpm"], "server kept our wpm");
    assert_eq!(last["mode"], "words");

    // Immediate resubmission of the same event must be rejected (466 duplicate
    // or 462 spacing, depending on server config).
    match client.post_result(&mut session, &event) {
        Ok(_) => panic!("duplicate resubmission should be rejected"),
        Err(ApiError::Server { status, .. }) => {
            println!("duplicate resubmit rejected with {status}");
            assert!(
                matches!(status, 462 | 466),
                "expected 462/466, got {status}"
            );
        }
        Err(other) => panic!("expected a server rejection, got {other:?}"),
    }

    // A 5s time-mode result trips the local pre-gate (the server would 460).
    let short = run_scripted_session(TestMode::Time(5));
    let short_stats = stats::compute(&short);
    let reason = completed_event::submission_block_reason(&short, &short_stats)
        .expect("short test must be blocked locally");
    println!("short test blocked locally: {reason}");
    // Bypass the gate to verify the server agrees (460 test too short).
    let short_event =
        completed_event::build(&short, &short_stats, &Config::default(), &session.uid);
    match client.post_result(&mut session, &short_event) {
        Ok(_) => panic!("5s test should be rejected"),
        Err(ApiError::Server { status, .. }) => {
            println!("short test rejected with {status}");
            assert_eq!(status, 460);
        }
        Err(other) => panic!("expected 460, got {other:?}"),
    }

    let _ = std::fs::remove_file(&tmp);
}
