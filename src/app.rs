use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

use crate::api::worker::{ApiEvent, ApiRequest, ApiWorker};
use crate::api::{ApeClient, PostResultData};
use crate::auth::worker::{AuthEvent, AuthRequest, AuthWorker};
use crate::auth::{AuthManager, Session, TokenStore};
use crate::config::{Config, Mode};
use crate::engine::stats::FinalStats;
use crate::engine::{SessionState, TestMode, TestSession};
use crate::languages::{self, LanguageData};
use crate::storage::Paths;
use crate::theme::{self, Theme};
use crate::ui::{self, login_screen::LoginAction};

const TICK: Duration = Duration::from_millis(33);

// screens are singletons; variant size imbalance is irrelevant here
#[allow(clippy::large_enum_variant)]
pub enum Screen {
    Test,
    Result(FinalStats),
    Menu(ui::menu_screen::MenuState),
    Settings(ui::settings_screen::SettingsState),
    Themes(ui::theme_screen::ThemeState),
    Login(ui::login_screen::LoginState),
}

/// Where the just-finished test's submission stands (shown on the result screen).
pub enum SubmissionStatus {
    Idle,
    /// Not submitted, with the reason (logged out, saving off, would be rejected).
    Skipped(String),
    InFlight,
    Saved(PostResultData),
    Failed {
        message: String,
        retryable: bool,
    },
}

/// What a screen asks the app to do after handling a key.
pub enum Action {
    None,
    Quit,
    Restart,
    OpenMenu,
    OpenSettings,
    OpenThemes,
    OpenLogin,
    Logout,
    CloseToTest,
    ConfigChanged,
    ThemesChanged,
}

pub struct App {
    pub screen: Screen,
    pub session: TestSession,
    pub config: Config,
    pub theme: Theme,
    pub themes: Vec<(String, Theme)>,
    pub language: LanguageData,
    pub key_release_supported: bool,
    pub last_key: Option<(char, Instant)>,
    pub paths: Option<Paths>,
    /// Logged-in user, if any.
    pub account: Option<Session>,
    /// Outcome of an auth attempt that finished while off the login screen.
    pub auth_notice: Option<String>,
    pub submission: SubmissionStatus,
    /// The last built CompletedEvent, kept for retry on transient failures.
    last_event: Option<serde_json::Value>,
    auth: Arc<AuthManager>,
    auth_worker: AuthWorker,
    api_worker: ApiWorker,
    should_quit: bool,
}

impl App {
    pub fn new(cli_mode: Option<TestMode>, key_release_supported: bool) -> Self {
        let paths = Paths::resolve();
        let mut config = paths
            .as_ref()
            .map(|p| Config::load(&p.config_file))
            .unwrap_or_default();
        if let Some(paths) = &paths {
            let _ = paths.ensure_dirs();
        }
        crate::logging::init(paths.as_ref().map(|p| p.data_dir.as_path()));
        crate::logging::debug("app start");
        match cli_mode {
            Some(TestMode::Time(t)) => {
                config.mode = Mode::Time;
                config.time = t;
            }
            Some(TestMode::Words(w)) => {
                config.mode = Mode::Words;
                config.words = w;
            }
            None => {}
        }

        let themes = paths
            .as_ref()
            .map(|p| theme::load_themes(&p.themes_dir))
            .unwrap_or_default();
        let theme = theme::resolve(&config, &themes);
        let language = languages::english();
        let session = new_session(&config, &language, key_release_supported);

        let tokens_file = paths
            .as_ref()
            .map(|p| p.tokens_file.clone())
            .unwrap_or_else(|| std::path::PathBuf::from("tokens.json"));
        let auth = Arc::new(AuthManager::new(TokenStore::new(tokens_file)));
        let mut auth_worker = AuthWorker::spawn(auth.clone());
        // Restore a prior session off-thread so startup never blocks on network.
        auth_worker.submit(AuthRequest::Restore);
        let api_worker = ApiWorker::spawn(
            ApeClient::new(auth.clone()),
            paths.as_ref().map(|p| p.results_file.clone()),
        );

        Self {
            screen: Screen::Test,
            session,
            config,
            theme,
            themes,
            language,
            key_release_supported,
            last_key: None,
            paths,
            account: None,
            auth_notice: None,
            submission: SubmissionStatus::Idle,
            last_event: None,
            auth,
            auth_worker,
            api_worker,
            should_quit: false,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        while !self.should_quit {
            terminal.draw(|f| self.draw(f))?;
            if event::poll(TICK)? {
                match event::read()? {
                    Event::Key(key) => self.on_key(key),
                    Event::Paste(text) => {
                        if let Screen::Login(state) = &mut self.screen {
                            state.paste(&text);
                        }
                    }
                    _ => {}
                }
            }
            self.on_tick();
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        ui::draw_background(frame, &self.theme);
        match &self.screen {
            Screen::Test => ui::test_screen::draw(frame, self),
            Screen::Result(stats) => ui::result_screen::draw(frame, self, stats),
            Screen::Menu(state) => ui::menu_screen::draw(frame, self, state),
            Screen::Settings(state) => ui::settings_screen::draw(frame, self, state),
            Screen::Themes(state) => ui::theme_screen::draw(frame, self, state),
            Screen::Login(state) => ui::login_screen::draw(frame, self, state),
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        let now = Instant::now();

        if key.kind == KeyEventKind::Release {
            self.session.handle_release(key.code, now);
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        if let KeyCode::Char(c) = key.code {
            self.last_key = Some((c, now));
        }

        // Login screen dispatches auth requests through the app.
        if let Screen::Login(state) = &mut self.screen {
            match state.handle(key.code, key.modifiers) {
                LoginAction::None => {}
                LoginAction::Back => self.apply(Action::OpenMenu),
                LoginAction::SubmitEmail { email, password } => {
                    self.auth_worker
                        .submit(AuthRequest::Email { email, password });
                }
                LoginAction::StartOAuth(provider) => {
                    self.auth_worker.submit(AuthRequest::OAuthBegin(provider));
                }
                LoginAction::FinishOAuth(url) => {
                    self.auth_worker.submit(AuthRequest::OAuthFinish(url));
                }
                LoginAction::CancelOAuth => {
                    self.auth_worker.submit(AuthRequest::OAuthCancel);
                }
            }
            return;
        }

        let action = match &mut self.screen {
            Screen::Test => match key.code {
                KeyCode::Esc => Action::OpenMenu,
                KeyCode::Tab => Action::Restart,
                code => {
                    self.session.handle_key(code, now);
                    Action::None
                }
            },
            Screen::Result(_) => match key.code {
                KeyCode::Esc => Action::OpenMenu,
                KeyCode::Tab | KeyCode::Enter => Action::Restart,
                KeyCode::Char('r')
                    if matches!(
                        self.submission,
                        SubmissionStatus::Failed {
                            retryable: true,
                            ..
                        }
                    ) =>
                {
                    self.retry_submission();
                    Action::None
                }
                _ => Action::None,
            },
            Screen::Menu(state) => state.handle(key.code),
            Screen::Settings(state) => state.handle(key.code, &mut self.config),
            Screen::Themes(state) => state.handle(
                key.code,
                &mut self.config,
                &self.themes,
                self.paths.as_ref(),
            ),
            Screen::Login(_) => Action::None, // handled above
        };
        self.apply(action);
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => self.should_quit = true,
            Action::Restart => self.restart(),
            Action::OpenMenu => {
                self.screen = Screen::Menu(ui::menu_screen::MenuState::new(self.account.is_some()));
            }
            Action::OpenSettings => {
                self.screen = Screen::Settings(ui::settings_screen::SettingsState::new());
            }
            Action::OpenThemes => {
                self.screen = Screen::Themes(ui::theme_screen::ThemeState::new(&self.themes));
            }
            Action::OpenLogin => {
                self.auth_notice = None;
                self.screen = Screen::Login(ui::login_screen::LoginState::new());
            }
            Action::Logout => {
                self.auth.logout();
                self.account = None;
                self.screen = Screen::Menu(ui::menu_screen::MenuState::new(false));
            }
            Action::CloseToTest => {
                self.save_config();
                self.restart();
            }
            Action::ConfigChanged => {
                self.theme = theme::resolve(&self.config, &self.themes);
                self.save_config();
            }
            Action::ThemesChanged => {
                if let Some(paths) = &self.paths {
                    self.themes = theme::load_themes(&paths.themes_dir);
                }
                self.theme = theme::resolve(&self.config, &self.themes);
                self.save_config();
            }
        }
    }

    fn on_tick(&mut self) {
        // Drain completed auth operations.
        if let Some(event) = self.auth_worker.poll() {
            match event {
                AuthEvent::LoggedIn(session) => {
                    self.account = Some(session);
                    self.auth_notice = None;
                    // Interactive login closes the login screen; a background
                    // restore just updates state without disturbing the test.
                    if matches!(self.screen, Screen::Login(_)) {
                        self.restart();
                    }
                }
                AuthEvent::NotRestored => {}
                AuthEvent::Failed(msg) => {
                    if let Screen::Login(state) = &mut self.screen {
                        state.set_error(msg);
                    } else {
                        // The user navigated away mid-login; surface the
                        // outcome in the menu instead of dropping it.
                        self.auth_notice = Some(format!("login failed: {msg}"));
                    }
                }
                AuthEvent::OAuthPending {
                    provider,
                    auth_uri,
                    open_failed,
                } => {
                    if let Screen::Login(state) = &mut self.screen {
                        state.set_oauth_prompt(provider, auth_uri, open_failed);
                    } else {
                        // Flow abandoned before the URL came back.
                        self.auth_worker.submit(AuthRequest::OAuthCancel);
                    }
                }
                AuthEvent::Cancelled => {}
            }
        }

        // Drain completed API operations.
        if let Some(event) = self.api_worker.poll() {
            match event {
                ApiEvent::ResultPosted { outcome, session } => {
                    // Propagate a mid-flight token refresh back to the account.
                    if self.account.as_ref().is_some_and(|a| a.uid == session.uid) {
                        self.account = Some(session);
                    }
                    self.submission = match outcome {
                        Ok(data) => SubmissionStatus::Saved(data),
                        Err(e) => SubmissionStatus::Failed {
                            message: e.to_string(),
                            retryable: e.is_retryable(),
                        },
                    };
                }
            }
        }

        if matches!(self.screen, Screen::Test) {
            self.session.tick();
            self.check_finished();
        }
    }

    fn check_finished(&mut self) {
        if self.session.state == SessionState::Finished {
            let stats = crate::engine::stats::compute(&self.session);
            self.submission = self.submit_result(&stats);
            self.screen = Screen::Result(stats);
        }
    }

    /// Kick off result submission for the just-finished session, or explain
    /// why it is skipped.
    fn submit_result(&mut self, stats: &crate::engine::stats::FinalStats) -> SubmissionStatus {
        use crate::engine::completed_event;
        let Some(session) = self.account.clone() else {
            return SubmissionStatus::Skipped("sign in to save results".into());
        };
        if !self.config.result_saving {
            return SubmissionStatus::Skipped("result saving is disabled".into());
        }
        if let Some(reason) = completed_event::submission_block_reason(&self.session, stats) {
            return SubmissionStatus::Skipped(format!("not saved: {reason}"));
        }
        let event = completed_event::build(&self.session, stats, &self.config, &session.uid);
        self.last_event = Some(event.clone());
        self.api_worker
            .submit(ApiRequest::PostResult { session, event });
        SubmissionStatus::InFlight
    }

    /// Resubmit the stashed event after a transient failure (`r` on the
    /// result screen).
    fn retry_submission(&mut self) {
        let (Some(session), Some(event)) = (self.account.clone(), self.last_event.clone()) else {
            return;
        };
        self.api_worker
            .submit(ApiRequest::PostResult { session, event });
        self.submission = SubmissionStatus::InFlight;
    }

    fn restart(&mut self) {
        if let Some(name) = theme::pick_random(&self.config, &self.themes) {
            self.config.theme = name;
            self.theme = theme::resolve(&self.config, &self.themes);
        }
        self.session = new_session(&self.config, &self.language, self.key_release_supported);
        self.submission = SubmissionStatus::Idle;
        self.last_event = None;
        self.screen = Screen::Test;
    }

    fn save_config(&self) {
        if let Some(paths) = &self.paths {
            let _ = self.config.save(&paths.config_file);
        }
    }
}

fn new_session(config: &Config, language: &LanguageData, release_events: bool) -> TestSession {
    let mode = match config.mode {
        Mode::Words => TestMode::Words(config.words.max(1)),
        Mode::Time | Mode::Quote | Mode::Zen | Mode::Custom => TestMode::Time(config.time.max(1)),
    };
    let mut session = TestSession::new(mode, language, release_events);
    session.quick_end = config.quick_end;
    session
}
