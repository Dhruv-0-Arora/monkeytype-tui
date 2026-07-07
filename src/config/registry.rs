//! Declarative settings catalog: one row per config key. The settings screen
//! renders straight from this table, so adding a setting is one entry here.

use super::Config;

pub struct SettingDef {
    pub key: &'static str,
    pub group: &'static str,
    /// Present for config-sync fidelity but has no effect in a terminal.
    pub stub: bool,
    pub display: fn(&Config) -> String,
    pub cycle: fn(&mut Config, bool),
}

fn on_off(v: bool) -> String {
    if v { "on" } else { "off" }.into()
}

macro_rules! toggle {
    ($key:literal, $group:literal, $field:ident $(, $stub:literal)?) => {
        SettingDef {
            key: $key,
            group: $group,
            stub: false $(|| $stub)?,
            display: |c| on_off(c.$field),
            cycle: |c, _| c.$field = !c.$field,
        }
    };
}

macro_rules! choice {
    ($key:literal, $group:literal, $field:ident $(, $stub:literal)?) => {
        SettingDef {
            key: $key,
            group: $group,
            stub: false $(|| $stub)?,
            display: |c| c.$field.as_str().into(),
            cycle: |c, fwd| c.$field = c.$field.cycle(fwd),
        }
    };
}

macro_rules! preset_num {
    ($key:literal, $group:literal, $field:ident, [$($preset:expr),+]) => {
        SettingDef {
            key: $key,
            group: $group,
            stub: false,
            display: |c| c.$field.to_string(),
            cycle: |c, fwd| {
                let presets = [$($preset),+];
                let pos = presets.iter().position(|p| *p >= c.$field);
                let idx = match (pos, fwd) {
                    (Some(i), true) if presets[i] == c.$field => (i + 1).min(presets.len() - 1),
                    (Some(i), true) => i,
                    (Some(i), false) => i.saturating_sub(1),
                    (None, _) => presets.len() - 1,
                };
                c.$field = presets[idx];
            },
        }
    };
}

macro_rules! readonly {
    ($key:literal, $group:literal, $field:ident $(, $stub:literal)?) => {
        SettingDef {
            key: $key,
            group: $group,
            stub: false $(|| $stub)?,
            display: |c| c.$field.to_string(),
            cycle: |_, _| {},
        }
    };
}

/// Full catalog in web settings-page order.
pub fn all() -> Vec<SettingDef> {
    vec![
        // test
        toggle!("punctuation", "test", punctuation),
        toggle!("numbers", "test", numbers),
        choice!("mode", "test", mode),
        preset_num!("time", "test", time, [15, 30, 60, 120]),
        preset_num!("words", "test", words, [10, 25, 50, 100]),
        readonly!("language", "test", language),
        // behavior
        choice!("difficulty", "behavior", difficulty),
        choice!("quickRestart", "behavior", quick_restart),
        toggle!("blindMode", "behavior", blind_mode),
        choice!("minWpm", "behavior", min_wpm),
        preset_num!(
            "minWpmCustomSpeed",
            "behavior",
            min_wpm_custom_speed,
            [50.0, 75.0, 100.0, 125.0, 150.0]
        ),
        choice!("minAcc", "behavior", min_acc),
        preset_num!(
            "minAccCustom",
            "behavior",
            min_acc_custom,
            [75.0, 80.0, 90.0, 95.0, 99.0]
        ),
        choice!("minBurst", "behavior", min_burst),
        toggle!("britishEnglish", "behavior", british_english, true),
        toggle!("resultSaving", "behavior", result_saving),
        // input
        toggle!("freedomMode", "input", freedom_mode),
        toggle!("strictSpace", "input", strict_space),
        choice!("oppositeShiftMode", "input", opposite_shift_mode, true),
        choice!("stopOnError", "input", stop_on_error),
        choice!("confidenceMode", "input", confidence_mode),
        toggle!("quickEnd", "input", quick_end),
        choice!("indicateTypos", "input", indicate_typos),
        toggle!("hideExtraLetters", "input", hide_extra_letters),
        toggle!("lazyMode", "input", lazy_mode),
        // sound (all stubs: no audio in the terminal yet)
        readonly!("soundVolume", "sound", sound_volume, true),
        readonly!("playSoundOnClick", "sound", play_sound_on_click, true),
        readonly!("playSoundOnError", "sound", play_sound_on_error, true),
        choice!("playTimeWarning", "sound", play_time_warning, true),
        // caret
        choice!("smoothCaret", "caret", smooth_caret, true),
        choice!("caretStyle", "caret", caret_style),
        choice!("paceCaret", "caret", pace_caret),
        preset_num!(
            "paceCaretCustomSpeed",
            "caret",
            pace_caret_custom_speed,
            [50.0, 75.0, 100.0, 125.0, 150.0]
        ),
        choice!("paceCaretStyle", "caret", pace_caret_style),
        toggle!("repeatedPace", "caret", repeated_pace),
        // appearance
        choice!("timerStyle", "appearance", timer_style),
        choice!("liveSpeedStyle", "appearance", live_speed_style),
        choice!("liveAccStyle", "appearance", live_acc_style),
        choice!("liveBurstStyle", "appearance", live_burst_style),
        choice!("timerColor", "appearance", timer_color),
        choice!("timerOpacity", "appearance", timer_opacity),
        choice!("highlightMode", "appearance", highlight_mode),
        choice!("typedEffect", "appearance", typed_effect),
        choice!("tapeMode", "appearance", tape_mode),
        preset_num!(
            "tapeMargin",
            "appearance",
            tape_margin,
            [10.0, 25.0, 50.0, 75.0, 90.0]
        ),
        toggle!("smoothLineScroll", "appearance", smooth_line_scroll, true),
        toggle!("showAllLines", "appearance", show_all_lines),
        toggle!(
            "alwaysShowDecimalPlaces",
            "appearance",
            always_show_decimal_places
        ),
        choice!("typingSpeedUnit", "appearance", typing_speed_unit),
        toggle!("startGraphsAtZero", "appearance", start_graphs_at_zero),
        preset_num!(
            "maxLineWidth",
            "appearance",
            max_line_width,
            [0, 40, 60, 80, 100, 120]
        ),
        readonly!("fontSize", "appearance", font_size, true),
        readonly!("fontFamily", "appearance", font_family, true),
        choice!("keymapMode", "appearance", keymap_mode),
        choice!("keymapStyle", "appearance", keymap_style, true),
        choice!("keymapLegendStyle", "appearance", keymap_legend_style),
        choice!("keymapShowTopRow", "appearance", keymap_show_top_row, true),
        // theme
        toggle!("flipTestColors", "theme", flip_test_colors),
        toggle!("colorfulMode", "theme", colorful_mode),
        toggle!("customTheme", "theme", custom_theme),
        choice!("randomTheme", "theme", random_theme),
        toggle!("autoSwitchTheme", "theme", auto_switch_theme),
        readonly!("theme", "theme", theme),
        readonly!("themeLight", "theme", theme_light),
        readonly!("themeDark", "theme", theme_dark),
        readonly!("customBackground", "theme", custom_background, true),
        // hide elements
        toggle!("showKeyTips", "hide elements", show_key_tips),
        toggle!(
            "showOutOfFocusWarning",
            "hide elements",
            show_out_of_focus_warning
        ),
        toggle!("capsLockWarning", "hide elements", caps_lock_warning, true),
        choice!("showAverage", "hide elements", show_average),
        toggle!("showPb", "hide elements", show_pb),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_cycles_without_panicking() {
        let mut config = Config::default();
        for def in all() {
            (def.cycle)(&mut config, true);
            (def.cycle)(&mut config, false);
            let _ = (def.display)(&config);
        }
    }

    #[test]
    fn preset_cycling_moves_through_presets() {
        let mut config = Config::default();
        let defs = all();
        let time = defs.iter().find(|d| d.key == "time").unwrap();
        assert_eq!(config.time, 30);
        (time.cycle)(&mut config, true);
        assert_eq!(config.time, 60);
        (time.cycle)(&mut config, false);
        (time.cycle)(&mut config, false);
        assert_eq!(config.time, 15);
        (time.cycle)(&mut config, false);
        assert_eq!(config.time, 15, "clamps at the low end");
    }
}
