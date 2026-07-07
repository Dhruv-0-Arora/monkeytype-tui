use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::languages::LanguageData;

/// Uniform random word source. Punctuation/numbers/lazyMode transforms are
/// wired in later phases; the generator is the single place they will live.
pub struct WordGenerator {
    words: Vec<String>,
    rng: SmallRng,
    last: Option<usize>,
}

impl WordGenerator {
    pub fn new(lang: &LanguageData) -> Self {
        Self {
            words: lang.words.clone(),
            rng: SmallRng::from_entropy(),
            last: None,
        }
    }

    #[cfg(test)]
    pub fn with_seed(lang: &LanguageData, seed: u64) -> Self {
        Self {
            words: lang.words.clone(),
            rng: SmallRng::seed_from_u64(seed),
            last: None,
        }
    }

    pub fn next_words(&mut self, n: usize) -> Vec<String> {
        (0..n).map(|_| self.next_word()).collect()
    }

    /// Random word, avoiding an immediate repeat (matches web behavior).
    fn next_word(&mut self) -> String {
        loop {
            let idx = self.rng.gen_range(0..self.words.len());
            if self.words.len() > 1 && self.last == Some(idx) {
                continue;
            }
            self.last = Some(idx);
            return self.words[idx].clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages;

    #[test]
    fn generates_requested_count_from_language() {
        let lang = languages::english();
        let mut generator = WordGenerator::with_seed(&lang, 42);
        let words = generator.next_words(50);
        assert_eq!(words.len(), 50);
        assert!(words.iter().all(|w| lang.words.contains(w)));
    }

    #[test]
    fn avoids_immediate_repeats() {
        let lang = languages::english();
        let mut generator = WordGenerator::with_seed(&lang, 7);
        let words = generator.next_words(200);
        assert!(words.windows(2).all(|w| w[0] != w[1]));
    }
}
