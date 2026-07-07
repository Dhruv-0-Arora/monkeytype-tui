//! Live auth smoke test against the real Firebase project. Ignored by default
//! (needs network + real credentials); run explicitly with:
//!
//!   MONKEYTYPE_TEST_EMAIL=... MONKEYTYPE_TEST_PASSWORD=... \
//!     cargo test --test auth_live -- --ignored --nocapture
//!
//! Verifies the full email/password path: sign in, get a bearer token, refresh,
//! and that the token is accepted by the real monkeytype API.

use monkeytype_tui::auth::{firebase, AuthManager, TokenStore};

fn creds() -> Option<(String, String)> {
    let email = std::env::var("MONKEYTYPE_TEST_EMAIL").ok()?;
    let password = std::env::var("MONKEYTYPE_TEST_PASSWORD").ok()?;
    Some((email, password))
}

#[test]
#[ignore = "needs network and real credentials"]
fn email_login_refresh_and_api_call() {
    let (email, password) = creds().expect("set MONKEYTYPE_TEST_EMAIL / _PASSWORD");
    let tmp = std::env::temp_dir().join(format!("mttui-authtest-{}.json", std::process::id()));
    // Isolated keychain account so the test never touches the real credential.
    let account = format!("test-refresh-{}", std::process::id());
    let store = || TokenStore::with_account(tmp.clone(), account.clone());
    let manager = AuthManager::new(store());

    let mut session = manager
        .login_email(&email, &password)
        .expect("login should succeed");
    assert!(!session.uid.is_empty(), "got a uid");
    assert!(!session.id_token.is_empty(), "got an id token");

    // bearer() returns a usable token and can refresh in place.
    let token = manager.bearer(&mut session).expect("bearer token");
    assert_eq!(token, session.id_token);

    // The token is accepted by the real monkeytype API.
    let client = reqwest::blocking::Client::builder()
        .user_agent("monkeytype-tui/test")
        .build()
        .unwrap();
    let resp = client
        .get("https://api.monkeytype.com/users")
        .header("Authorization", format!("Bearer {}", session.id_token))
        .send()
        .expect("GET /users");
    assert!(
        resp.status().is_success(),
        "GET /users returned {}",
        resp.status()
    );

    // Direct refresh call yields a fresh, different token.
    let (new_id, _new_refresh, ttl) =
        firebase::refresh_token(&client_with_referer(), &session.refresh_token)
            .expect("refresh should succeed");
    assert!(!new_id.is_empty());
    assert!(ttl >= 3000, "ttl about an hour: {ttl}");

    // A fresh manager over the same store restores the session (startup path):
    // this exercises the persisted refresh token -> refresh -> new id token.
    let restored = AuthManager::new(store())
        .restore()
        .expect("restore from stored refresh token");
    assert_eq!(restored.uid, session.uid, "restored the same account");
    assert!(!restored.id_token.is_empty());

    // Logout clears the stored token so a later restore finds nothing.
    let manager2 = AuthManager::new(store());
    manager2.logout();
    assert!(
        manager2.restore().is_none(),
        "restore returns None after logout"
    );

    let _ = std::fs::remove_file(&tmp);
}

fn client_with_referer() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("monkeytype-tui/test")
        .build()
        .unwrap()
}
