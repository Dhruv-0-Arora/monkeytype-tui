//! The full web Config, mirrored key-for-key from packages/schemas/src/configs.ts
//! so PATCH /configs round-trips losslessly (Phase 5). Serialized names and enum
//! literals must match the web exactly - the server's PATCH schema is strict.
//!
//! Keys the terminal cannot honor (fontFamily, sounds, customBackground, ...)
//! still live here and appear in the settings UI marked "(no effect in terminal)".

pub mod registry;

use std::path::Path;

use serde::{Deserialize, Serialize};

macro_rules! config_enum {
    ($name:ident { $($variant:ident => $lit:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $(#[serde(rename = $lit)] $variant),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $lit),+ }
            }

            pub fn cycle(self, forward: bool) -> Self {
                let idx = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
                let len = Self::ALL.len();
                let next = if forward { (idx + 1) % len } else { (idx + len - 1) % len };
                Self::ALL[next]
            }
        }
    };
}

config_enum!(Mode { Time => "time", Words => "words", Quote => "quote", Zen => "zen", Custom => "custom" });
config_enum!(Difficulty { Normal => "normal", Expert => "expert", Master => "master" });
config_enum!(QuickRestart { Off => "off", Esc => "esc", Tab => "tab", Enter => "enter" });
config_enum!(RepeatQuotes { Off => "off", Typing => "typing" });
config_enum!(SingleListCommandLine { Manual => "manual", On => "on" });
config_enum!(OnOff { Off => "off", Custom => "custom" });
config_enum!(MinBurst { Off => "off", Fixed => "fixed", Flex => "flex" });
config_enum!(OppositeShiftMode { Off => "off", On => "on", Keymap => "keymap" });
config_enum!(StopOnError { Off => "off", Word => "word", Letter => "letter" });
config_enum!(ConfidenceMode { Off => "off", On => "on", Max => "max" });
config_enum!(IndicateTypos { Off => "off", Below => "below", Replace => "replace", Both => "both" });
config_enum!(CompositionDisplay { Off => "off", Below => "below", Replace => "replace" });
config_enum!(SmoothCaret { Off => "off", Slow => "slow", Medium => "medium", Fast => "fast" });
config_enum!(CaretStyle {
    Off => "off", Default => "default", Block => "block", Outline => "outline",
    Underline => "underline", Carrot => "carrot", Banana => "banana", Monkey => "monkey",
});
config_enum!(PaceCaret {
    Off => "off", Average => "average", Pb => "pb", TagPb => "tagPb",
    Last => "last", Custom => "custom", Daily => "daily",
});
config_enum!(TimerStyle {
    Off => "off", Bar => "bar", Text => "text", Mini => "mini",
    FlashText => "flash_text", FlashMini => "flash_mini",
});
config_enum!(LiveStatStyle { Off => "off", Text => "text", Mini => "mini" });
config_enum!(TimerColor { Black => "black", Sub => "sub", Text => "text", Main => "main" });
config_enum!(TimerOpacity { Q25 => "0.25", Q50 => "0.5", Q75 => "0.75", Full => "1" });
config_enum!(HighlightMode {
    Off => "off", Letter => "letter", Word => "word", NextWord => "next_word",
    NextTwoWords => "next_two_words", NextThreeWords => "next_three_words",
});
config_enum!(TypedEffect { Keep => "keep", Hide => "hide", Fade => "fade", Dots => "dots" });
config_enum!(TapeMode { Off => "off", Letter => "letter", Word => "word" });
config_enum!(TypingSpeedUnit { Wpm => "wpm", Cpm => "cpm", Wps => "wps", Cps => "cps", Wph => "wph" });
config_enum!(KeymapMode { Off => "off", Static => "static", React => "react", Next => "next" });
config_enum!(KeymapStyle {
    Staggered => "staggered", Alice => "alice", Matrix => "matrix", Split => "split",
    SplitMatrix => "split_matrix", Steno => "steno", StenoMatrix => "steno_matrix",
});
config_enum!(KeymapLegendStyle { Lowercase => "lowercase", Uppercase => "uppercase", Blank => "blank", Dynamic => "dynamic" });
config_enum!(KeymapShowTopRow { Always => "always", Layout => "layout", Never => "never" });
config_enum!(CustomBackgroundSize { Cover => "cover", Contain => "contain", Max => "max" });
config_enum!(RandomTheme {
    Off => "off", On => "on", Fav => "fav", Light => "light",
    Dark => "dark", Custom => "custom", Auto => "auto",
});
config_enum!(ShowAverage { Off => "off", Speed => "speed", Acc => "acc", Both => "both" });
config_enum!(MonkeyPowerLevel { Off => "off", L1 => "1", L2 => "2", L3 => "3", L4 => "4" });
config_enum!(Ads { Off => "off", Result => "result", On => "on", Sellout => "sellout" });
config_enum!(PlayTimeWarning { Off => "off", S1 => "1", S3 => "3", S5 => "5", S10 => "10" });

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    // test
    pub punctuation: bool,
    pub numbers: bool,
    pub words: usize,
    pub time: u64,
    pub mode: Mode,
    pub quote_length: Vec<i8>,
    pub language: String,
    pub burst_heatmap: bool,
    // behavior
    pub difficulty: Difficulty,
    pub quick_restart: QuickRestart,
    pub repeat_quotes: RepeatQuotes,
    pub result_saving: bool,
    pub blind_mode: bool,
    pub always_show_words_history: bool,
    pub single_list_command_line: SingleListCommandLine,
    pub min_wpm: OnOff,
    pub min_wpm_custom_speed: f64,
    pub min_acc: OnOff,
    pub min_acc_custom: f64,
    pub min_burst: MinBurst,
    pub min_burst_custom_speed: f64,
    pub british_english: bool,
    pub funbox: Vec<String>,
    pub custom_layoutfluid: Vec<String>,
    pub custom_polyglot: Vec<String>,
    // input
    pub freedom_mode: bool,
    pub strict_space: bool,
    pub opposite_shift_mode: OppositeShiftMode,
    pub stop_on_error: StopOnError,
    pub confidence_mode: ConfidenceMode,
    pub quick_end: bool,
    pub indicate_typos: IndicateTypos,
    pub composition_display: CompositionDisplay,
    pub hide_extra_letters: bool,
    pub lazy_mode: bool,
    pub layout: String,
    pub code_unindent_on_backspace: bool,
    // sound (stubs in the TUI)
    pub sound_volume: f64,
    pub play_sound_on_click: String,
    pub play_sound_on_error: String,
    pub play_time_warning: PlayTimeWarning,
    // caret
    pub smooth_caret: SmoothCaret,
    pub caret_style: CaretStyle,
    pub pace_caret: PaceCaret,
    pub pace_caret_custom_speed: f64,
    pub pace_caret_style: CaretStyle,
    pub repeated_pace: bool,
    // appearance
    pub timer_style: TimerStyle,
    pub live_speed_style: LiveStatStyle,
    pub live_acc_style: LiveStatStyle,
    pub live_burst_style: LiveStatStyle,
    pub timer_color: TimerColor,
    pub timer_opacity: TimerOpacity,
    pub highlight_mode: HighlightMode,
    pub typed_effect: TypedEffect,
    pub tape_mode: TapeMode,
    pub tape_margin: f64,
    pub smooth_line_scroll: bool,
    pub show_all_lines: bool,
    pub always_show_decimal_places: bool,
    pub typing_speed_unit: TypingSpeedUnit,
    pub start_graphs_at_zero: bool,
    pub max_line_width: u32,
    pub font_size: f64,
    pub font_family: String,
    pub keymap_mode: KeymapMode,
    pub keymap_layout: String,
    pub keymap_style: KeymapStyle,
    pub keymap_legend_style: KeymapLegendStyle,
    pub keymap_show_top_row: KeymapShowTopRow,
    pub keymap_size: f64,
    // theme
    pub flip_test_colors: bool,
    pub colorful_mode: bool,
    pub custom_background: String,
    pub custom_background_size: CustomBackgroundSize,
    pub custom_background_filter: [f64; 4],
    pub auto_switch_theme: bool,
    pub theme_light: String,
    pub theme_dark: String,
    pub random_theme: RandomTheme,
    pub fav_themes: Vec<String>,
    pub theme: String,
    pub custom_theme: bool,
    pub custom_theme_colors: [String; 10],
    // hide elements
    pub show_key_tips: bool,
    pub show_out_of_focus_warning: bool,
    pub caps_lock_warning: bool,
    pub show_average: ShowAverage,
    pub show_pb: bool,
    // hidden / other
    pub account_chart: [String; 4],
    pub monkey: bool,
    pub monkey_power_level: MonkeyPowerLevel,
    pub ads: Ads,
}

impl Default for Config {
    /// Web defaults, from frontend/src/ts/constants/default-config.ts.
    fn default() -> Self {
        Self {
            punctuation: false,
            numbers: false,
            words: 50,
            time: 30,
            mode: Mode::Time,
            quote_length: vec![1],
            language: "english".into(),
            burst_heatmap: false,
            difficulty: Difficulty::Normal,
            quick_restart: QuickRestart::Off,
            repeat_quotes: RepeatQuotes::Off,
            result_saving: true,
            blind_mode: false,
            always_show_words_history: false,
            single_list_command_line: SingleListCommandLine::On,
            min_wpm: OnOff::Off,
            min_wpm_custom_speed: 100.0,
            min_acc: OnOff::Off,
            min_acc_custom: 90.0,
            min_burst: MinBurst::Off,
            min_burst_custom_speed: 100.0,
            british_english: false,
            funbox: vec![],
            custom_layoutfluid: vec!["qwerty".into(), "dvorak".into(), "colemak".into()],
            custom_polyglot: vec![
                "english".into(),
                "spanish".into(),
                "french".into(),
                "german".into(),
            ],
            freedom_mode: false,
            strict_space: false,
            opposite_shift_mode: OppositeShiftMode::Off,
            stop_on_error: StopOnError::Off,
            confidence_mode: ConfidenceMode::Off,
            quick_end: false,
            indicate_typos: IndicateTypos::Off,
            composition_display: CompositionDisplay::Replace,
            hide_extra_letters: false,
            lazy_mode: false,
            layout: "default".into(),
            code_unindent_on_backspace: false,
            sound_volume: 0.5,
            play_sound_on_click: "off".into(),
            play_sound_on_error: "off".into(),
            play_time_warning: PlayTimeWarning::Off,
            smooth_caret: SmoothCaret::Medium,
            caret_style: CaretStyle::Default,
            pace_caret: PaceCaret::Off,
            pace_caret_custom_speed: 100.0,
            pace_caret_style: CaretStyle::Default,
            repeated_pace: true,
            timer_style: TimerStyle::Mini,
            live_speed_style: LiveStatStyle::Off,
            live_acc_style: LiveStatStyle::Off,
            live_burst_style: LiveStatStyle::Off,
            timer_color: TimerColor::Main,
            timer_opacity: TimerOpacity::Full,
            highlight_mode: HighlightMode::Letter,
            typed_effect: TypedEffect::Keep,
            tape_mode: TapeMode::Off,
            tape_margin: 50.0,
            smooth_line_scroll: false,
            show_all_lines: false,
            always_show_decimal_places: false,
            typing_speed_unit: TypingSpeedUnit::Wpm,
            start_graphs_at_zero: true,
            max_line_width: 0,
            font_size: 2.0,
            font_family: "Roboto_Mono".into(),
            keymap_mode: KeymapMode::Off,
            keymap_layout: "overrideSync".into(),
            keymap_style: KeymapStyle::Staggered,
            keymap_legend_style: KeymapLegendStyle::Lowercase,
            keymap_show_top_row: KeymapShowTopRow::Layout,
            keymap_size: 1.0,
            flip_test_colors: false,
            colorful_mode: false,
            custom_background: String::new(),
            custom_background_size: CustomBackgroundSize::Cover,
            custom_background_filter: [0.0, 1.0, 1.0, 1.0],
            auto_switch_theme: false,
            theme_light: "serika".into(),
            theme_dark: "serika_dark".into(),
            random_theme: RandomTheme::Off,
            fav_themes: vec![],
            theme: "serika_dark".into(),
            custom_theme: false,
            custom_theme_colors: [
                "#323437".into(),
                "#e2b714".into(),
                "#e2b714".into(),
                "#646669".into(),
                "#2c2e31".into(),
                "#d1d0c5".into(),
                "#ca4754".into(),
                "#7e2a33".into(),
                "#ca4754".into(),
                "#7e2a33".into(),
            ],
            show_key_tips: true,
            show_out_of_focus_warning: true,
            caps_lock_warning: true,
            show_average: ShowAverage::Off,
            show_pb: false,
            account_chart: ["on".into(), "on".into(), "on".into(), "on".into()],
            monkey: false,
            monkey_power_level: MonkeyPowerLevel::Off,
            ads: Ads::Result,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_web_key_names_and_literals() {
        let config = Config::default();
        let json = serde_json::to_value(&config).unwrap();
        // camelCase keys, exact enum literals - the server PATCH is strict
        assert_eq!(json["mode"], "time");
        assert_eq!(json["quickRestart"], "off");
        assert_eq!(json["highlightMode"], "letter");
        assert_eq!(json["timerOpacity"], "1");
        assert_eq!(json["keymapLayout"], "overrideSync");
        assert_eq!(json["customThemeColors"][0], "#323437");
        assert_eq!(json["timerStyle"], "mini");
        assert_eq!(json["fontFamily"], "Roboto_Mono");
        assert_eq!(json["tapeMargin"], 50.0);
    }

    #[test]
    fn toml_round_trip_preserves_everything() {
        let config = Config {
            mode: Mode::Words,
            highlight_mode: HighlightMode::NextTwoWords,
            custom_theme: true,
            ..Default::default()
        };
        let text = toml::to_string_pretty(&config).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            serde_json::to_value(&back).unwrap()
        );
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let config: Config = toml::from_str("mode = \"words\"\nwords = 25").unwrap();
        assert_eq!(config.mode, Mode::Words);
        assert_eq!(config.words, 25);
        assert_eq!(config.time, 30);
        assert_eq!(config.theme, "serika_dark");
    }

    #[test]
    fn enum_cycle_wraps() {
        assert_eq!(Mode::Custom.cycle(true), Mode::Time);
        assert_eq!(Mode::Time.cycle(false), Mode::Custom);
    }
}
