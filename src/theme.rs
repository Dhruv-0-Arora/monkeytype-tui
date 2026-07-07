//! Theme system: the web's 10-color model, custom theme files on disk, and
//! the resolver that picks the active theme from config. Ships zero built-in
//! themes by design - users create TOML files in {config_dir}/themes/.

use std::path::Path;

use rand::seq::SliceRandom;
use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

use crate::config::{Config, RandomTheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse(hex: &str) -> Option<Rgb> {
        let hex = hex.strip_prefix('#')?;
        match hex.len() {
            6 => {
                let n = u32::from_str_radix(hex, 16).ok()?;
                Some(Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
            }
            3 => {
                let n = u32::from_str_radix(hex, 16).ok()?;
                let (r, g, b) = ((n >> 8) & 0xf, (n >> 4) & 0xf, n & 0xf);
                Some(Rgb((r * 17) as u8, (g * 17) as u8, (b * 17) as u8))
            }
            _ => None,
        }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// HSL lightness in [0, 1], used for random-theme light/dark filters
    /// (matches frontend/src/ts/utils/colors.ts: light means >= 0.5).
    pub fn lightness(self) -> f64 {
        let max = self.0.max(self.1).max(self.2) as f64;
        let min = self.0.min(self.1).min(self.2) as f64;
        (max + min) / 2.0 / 255.0
    }

    pub fn color(self) -> Color {
        Color::Rgb(self.0, self.1, self.2)
    }

    /// Blend toward another color; used to fake opacity on a cell grid.
    pub fn blend(self, other: Rgb, t: f64) -> Rgb {
        let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
        Rgb(
            mix(self.0, other.0),
            mix(self.1, other.1),
            mix(self.2, other.2),
        )
    }
}

/// The 10 theme colors, field order matching customThemeColors indices 0-9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub bg: Rgb,
    pub main: Rgb,
    pub caret: Rgb,
    pub sub: Rgb,
    pub sub_alt: Rgb,
    pub text: Rgb,
    pub error: Rgb,
    pub error_extra: Rgb,
    pub colorful_error: Rgb,
    pub colorful_error_extra: Rgb,
}

impl Theme {
    pub fn from_colors10(colors: &[String; 10]) -> Option<Theme> {
        let p: Vec<Rgb> = colors
            .iter()
            .map(|c| Rgb::parse(c))
            .collect::<Option<_>>()?;
        Some(Theme {
            bg: p[0],
            main: p[1],
            caret: p[2],
            sub: p[3],
            sub_alt: p[4],
            text: p[5],
            error: p[6],
            error_extra: p[7],
            colorful_error: p[8],
            colorful_error_extra: p[9],
        })
    }

    pub fn to_colors10(&self) -> [String; 10] {
        [
            self.bg.to_hex(),
            self.main.to_hex(),
            self.caret.to_hex(),
            self.sub.to_hex(),
            self.sub_alt.to_hex(),
            self.text.to_hex(),
            self.error.to_hex(),
            self.error_extra.to_hex(),
            self.colorful_error.to_hex(),
            self.colorful_error_extra.to_hex(),
        ]
    }

    /// Neutral grayscale fallback used until the user defines a theme.
    pub fn fallback() -> Self {
        Theme {
            bg: Rgb(0x1a, 0x1a, 0x1a),
            main: Rgb(0xd0, 0xd0, 0xd0),
            caret: Rgb(0xff, 0xff, 0xff),
            sub: Rgb(0x66, 0x66, 0x66),
            sub_alt: Rgb(0x2a, 0x2a, 0x2a),
            text: Rgb(0xe6, 0xe6, 0xe6),
            error: Rgb(0xcc, 0x44, 0x44),
            error_extra: Rgb(0x88, 0x2a, 0x2a),
            colorful_error: Rgb(0xcc, 0x44, 0x44),
            colorful_error_extra: Rgb(0x88, 0x2a, 0x2a),
        }
    }

    pub fn is_dark(&self) -> bool {
        self.bg.lightness() < 0.5
    }
}

/// On-disk custom theme: {config_dir}/themes/<name>.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFile {
    pub bg: String,
    pub main: String,
    pub caret: String,
    pub sub: String,
    #[serde(rename = "subAlt")]
    pub sub_alt: String,
    pub text: String,
    pub error: String,
    #[serde(rename = "errorExtra")]
    pub error_extra: String,
    #[serde(rename = "colorfulError")]
    pub colorful_error: String,
    #[serde(rename = "colorfulErrorExtra")]
    pub colorful_error_extra: String,
}

impl ThemeFile {
    pub fn to_theme(&self) -> Option<Theme> {
        Theme::from_colors10(&[
            self.bg.clone(),
            self.main.clone(),
            self.caret.clone(),
            self.sub.clone(),
            self.sub_alt.clone(),
            self.text.clone(),
            self.error.clone(),
            self.error_extra.clone(),
            self.colorful_error.clone(),
            self.colorful_error_extra.clone(),
        ])
    }

    pub fn from_theme(theme: &Theme) -> Self {
        let [bg, main, caret, sub, sub_alt, text, error, error_extra, colorful_error, colorful_error_extra] =
            theme.to_colors10();
        Self {
            bg,
            main,
            caret,
            sub,
            sub_alt,
            text,
            error,
            error_extra,
            colorful_error,
            colorful_error_extra,
        }
    }
}

/// Load every parseable theme file from the themes directory, sorted by name.
pub fn load_themes(dir: &Path) -> Vec<(String, Theme)> {
    let mut themes = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return themes;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.starts_with("._") {
            continue;
        }
        let Some(theme) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str::<ThemeFile>(&s).ok())
            .and_then(|f| f.to_theme())
        else {
            continue;
        };
        themes.push((stem.to_string(), theme));
    }
    themes.sort_by(|a, b| a.0.cmp(&b.0));
    themes
}

/// Does the OS prefer dark mode? None when undetectable (Linux, CI, ...).
pub fn os_prefers_dark() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .ok()?;
        // key exists (and equals "Dark") only in dark mode
        Some(out.status.success())
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Pick the active theme from config + available theme files. Mirrors the web
/// resolver: customTheme wins, then autoSwitchTheme's light/dark slots, then
/// the named theme. Unknown names fall back to the neutral theme.
pub fn resolve(config: &Config, themes: &[(String, Theme)]) -> Theme {
    if config.custom_theme {
        if let Some(theme) = Theme::from_colors10(&config.custom_theme_colors) {
            return theme;
        }
    }
    let name = if config.auto_switch_theme {
        match os_prefers_dark() {
            Some(true) => &config.theme_dark,
            Some(false) => &config.theme_light,
            None => &config.theme,
        }
    } else {
        &config.theme
    };
    by_name(themes, name).unwrap_or_else(Theme::fallback)
}

pub fn by_name(themes: &[(String, Theme)], name: &str) -> Option<Theme> {
    themes.iter().find(|(n, _)| n == name).map(|(_, t)| *t)
}

/// Apply randomTheme on test restart: returns the new theme name to use, or
/// None to keep the current selection. `custom` mode has no local theme-file
/// analog for saved server themes, so it draws from the same local pool.
pub fn pick_random(config: &Config, themes: &[(String, Theme)]) -> Option<String> {
    let mut rng = rand::thread_rng();
    let pool: Vec<&(String, Theme)> = match config.random_theme {
        RandomTheme::Off => return None,
        RandomTheme::On | RandomTheme::Custom => themes.iter().collect(),
        RandomTheme::Fav => themes
            .iter()
            .filter(|(n, _)| config.fav_themes.contains(n))
            .collect(),
        RandomTheme::Light => themes.iter().filter(|(_, t)| !t.is_dark()).collect(),
        RandomTheme::Dark => themes.iter().filter(|(_, t)| t.is_dark()).collect(),
        RandomTheme::Auto => {
            let dark = os_prefers_dark().unwrap_or(true);
            themes.iter().filter(|(_, t)| t.is_dark() == dark).collect()
        }
    };
    pool.choose(&mut rng).map(|(n, _)| n.clone())
}

/// Render state of a single target letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LetterState {
    Untyped,
    Correct,
    Incorrect,
    Extra,
}

/// Letter color resolution, including the web's two remap modes:
/// flipTestColors swaps correct<->untyped; colorfulMode moves correct to main
/// and errors to the colorful error pair.
pub fn letter_color(state: LetterState, theme: &Theme, flip: bool, colorful: bool) -> Color {
    let (correct, untyped) = if flip {
        (theme.sub, theme.text)
    } else {
        (theme.text, theme.sub)
    };
    let (correct, incorrect, extra) = if colorful {
        (theme.main, theme.colorful_error, theme.colorful_error_extra)
    } else {
        (correct, theme.error, theme.error_extra)
    };
    match state {
        LetterState::Untyped => untyped.color(),
        LetterState::Correct => correct.color(),
        LetterState::Incorrect => incorrect.color(),
        LetterState::Extra => extra.color(),
    }
}

pub fn caret_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.caret.color())
        .add_modifier(Modifier::REVERSED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing_round_trips() {
        assert_eq!(Rgb::parse("#e2b714"), Some(Rgb(0xe2, 0xb7, 0x14)));
        assert_eq!(Rgb::parse("#fff"), Some(Rgb(255, 255, 255)));
        assert_eq!(Rgb::parse("e2b714"), None);
        assert_eq!(Rgb(0xe2, 0xb7, 0x14).to_hex(), "#e2b714");
    }

    #[test]
    fn colors10_round_trip_in_web_index_order() {
        let colors: [String; 10] = [
            "#000001".into(),
            "#000002".into(),
            "#000003".into(),
            "#000004".into(),
            "#000005".into(),
            "#000006".into(),
            "#000007".into(),
            "#000008".into(),
            "#000009".into(),
            "#00000a".into(),
        ];
        let theme = Theme::from_colors10(&colors).unwrap();
        // index order: bg, main, caret, sub, subAlt, text, error, errorExtra,
        // colorfulError, colorfulErrorExtra
        assert_eq!(theme.bg, Rgb(0, 0, 1));
        assert_eq!(theme.main, Rgb(0, 0, 2));
        assert_eq!(theme.caret, Rgb(0, 0, 3));
        assert_eq!(theme.sub, Rgb(0, 0, 4));
        assert_eq!(theme.sub_alt, Rgb(0, 0, 5));
        assert_eq!(theme.text, Rgb(0, 0, 6));
        assert_eq!(theme.to_colors10(), colors);
    }

    #[test]
    fn flip_swaps_correct_and_untyped() {
        let theme = Theme::fallback();
        assert_eq!(
            letter_color(LetterState::Correct, &theme, true, false),
            theme.sub.color()
        );
        assert_eq!(
            letter_color(LetterState::Untyped, &theme, true, false),
            theme.text.color()
        );
    }

    #[test]
    fn colorful_uses_main_and_colorful_errors() {
        let mut theme = Theme::fallback();
        theme.colorful_error = Rgb(1, 2, 3);
        assert_eq!(
            letter_color(LetterState::Correct, &theme, false, true),
            theme.main.color()
        );
        assert_eq!(
            letter_color(LetterState::Incorrect, &theme, false, true),
            Rgb(1, 2, 3).color()
        );
    }

    #[test]
    fn theme_file_round_trip() {
        let theme = Theme::fallback();
        let file = ThemeFile::from_theme(&theme);
        assert_eq!(file.to_theme(), Some(theme));
        let toml_text = toml::to_string_pretty(&file).unwrap();
        let back: ThemeFile = toml::from_str(&toml_text).unwrap();
        assert_eq!(back.to_theme(), Some(theme));
    }

    #[test]
    fn resolver_prefers_custom_theme_colors() {
        let config = Config {
            custom_theme: true,
            ..Default::default()
        };
        let resolved = resolve(&config, &[]);
        // default customThemeColors = serika_dark palette
        assert_eq!(resolved.bg, Rgb(0x32, 0x34, 0x37));
        assert_eq!(resolved.main, Rgb(0xe2, 0xb7, 0x14));
    }

    #[test]
    fn resolver_falls_back_on_unknown_name() {
        let config = Config::default(); // theme = "serika_dark", no files on disk
        assert_eq!(resolve(&config, &[]), Theme::fallback());
    }
}
