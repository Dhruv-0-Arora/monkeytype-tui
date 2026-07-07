use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // metadata fields feed lazyMode/RTL support in later phases
pub struct LanguageData {
    pub name: String,
    #[serde(default)]
    pub no_lazy_mode: bool,
    #[serde(default)]
    pub ordered_by_frequency: bool,
    #[serde(default)]
    pub right_to_left: bool,
    #[serde(default)]
    pub additional_accents: Vec<Vec<String>>,
    pub words: Vec<String>,
}

/// English is bundled so the app works offline out of the box.
/// Other languages are fetched from monkeytype.com and cached (Phase 5).
pub fn english() -> LanguageData {
    serde_json::from_str(include_str!("../assets/languages/english.json"))
        .expect("bundled english.json is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_english_loads() {
        let lang = english();
        assert_eq!(lang.name, "english");
        assert!(lang.words.len() >= 100);
        assert!(lang.words.iter().all(|w| !w.is_empty()));
    }
}
