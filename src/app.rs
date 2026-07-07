use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

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
    auth: Arc<AuthManager>,
    auth_worker: AuthWorker,
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
            auth,
            auth_worker,
            should_quit: false,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        while !self.should_quit {
            terminal.draw(|f| self.draw(f))?;
            if event::poll(TICK)? {
                if let Event::Key(key) = event::read()? {
                    self.on_key(key);
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
                LoginAction::SubmitOAuth(provider) => {
                    self.auth_worker.submit(AuthRequest::OAuth(provider));
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
                    }
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
            self.screen = Screen::Result(stats);
        }
    }

    fn restart(&mut self) {
        if let Some(name) = theme::pick_random(&self.config, &self.themes) {
            self.config.theme = name;
            self.theme = theme::resolve(&self.config, &self.themes);
        }
        self.session = new_session(&self.config, &self.language, self.key_release_supported);
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
