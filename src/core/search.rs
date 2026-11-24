use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::core::entry::FileEntry;

/// Motor de búsqueda difusa (fuzzy) de alto rendimiento impulsado por `nucleo-matcher`.
pub struct SearchEngine {
    matcher: Matcher,
    utf32_buf: Vec<char>,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            utf32_buf: Vec::with_capacity(128),
        }
    }

    /// Filtra y ordena entradas por relevancia difusa respecto a `query`.
    /// Si `query` está vacío o solo contiene espacios, devuelve todas las entradas en orden original.
    pub fn filter<'a>(&mut self, entries: &'a [FileEntry], query: &str) -> Vec<&'a FileEntry> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return entries.iter().collect();
        }

        let pattern = Pattern::new(
            trimmed,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut scored: Vec<(u32, &'a FileEntry)> = entries
            .iter()
            .filter_map(|entry| {
                self.utf32_buf.clear();
                let haystack = Utf32Str::new(&entry.name, &mut self.utf32_buf);
                let score = pattern.score(haystack, &mut self.matcher)?;
                Some((score, entry))
            })
            .collect();

        scored.sort_by(|(score_a, entry_a), (score_b, entry_b)| {
            score_b
                .cmp(score_a)
                .then_with(|| entry_b.is_dir().cmp(&entry_a.is_dir()))
                .then_with(|| {
                    entry_a
                        .name
                        .to_ascii_lowercase()
                        .cmp(&entry_b.name.to_ascii_lowercase())
                })
        });

        scored.into_iter().map(|(_, entry)| entry).collect()
    }
}
