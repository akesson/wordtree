mod distsearch;
mod freqsearch;
pub mod ledger;
mod maxarr;
mod suggestion;
#[cfg(test)]
mod tests;

use std::ops::Deref;

use super::NodeRef;
use ledger::{KeepDecision, SearchDecision};
use maxarr::{MaxArr, MaxVal};
pub use suggestion::Suggestion;

pub type LedgerLine = ledger::LedgerLine;

/// Records the per-node decisions made while searching. The production
/// [`NoLedger`] implements every method as an inlined no-op, so the search
/// builds no `LedgerLine`s (and allocates no trace strings) when `ACTIVE` is
/// false; [`StateLedger`] keeps the full trace for the TUI/tests.
pub trait Ledger: Default {
    const ACTIVE: bool;
    #[allow(clippy::too_many_arguments)]
    fn record_dist(
        &mut self,
        word: &str,
        query: &str,
        dist: u8,
        min: u8,
        keep: KeepDecision,
        search: Option<SearchDecision>,
    );
    fn record_freq<V: Deref<Target = [u8]>>(
        &mut self,
        node: &NodeRef<'_, V>,
        path: &str,
        keep: KeepDecision,
        search: SearchDecision,
    );
}

#[derive(Default)]
pub struct NoLedger {}
impl Ledger for NoLedger {
    const ACTIVE: bool = false;

    fn record_dist(
        &mut self,
        _word: &str,
        _query: &str,
        _dist: u8,
        _min: u8,
        _keep: KeepDecision,
        _search: Option<SearchDecision>,
    ) {
    }
    fn record_freq<V: Deref<Target = [u8]>>(
        &mut self,
        _node: &NodeRef<'_, V>,
        _path: &str,
        _keep: KeepDecision,
        _search: SearchDecision,
    ) {
    }
}

#[derive(Default)]
pub struct StateLedger(pub Vec<ledger::LedgerLine>);

impl Ledger for StateLedger {
    const ACTIVE: bool = true;

    fn record_dist(
        &mut self,
        word: &str,
        query: &str,
        dist: u8,
        min: u8,
        keep: KeepDecision,
        search: Option<SearchDecision>,
    ) {
        self.0
            .push(LedgerLine::dist(word, query, dist, min, keep, search));
    }
    fn record_freq<V: Deref<Target = [u8]>>(
        &mut self,
        node: &NodeRef<'_, V>,
        path: &str,
        keep: KeepDecision,
        search: SearchDecision,
    ) {
        self.0.push(LedgerLine::freq(node, path, keep, search));
    }
}

enum Found {
    None,
    Node,
    Expr,
}

impl Found {
    fn from<V: Deref<Target = [u8]>>(found: &Option<NodeRef<'_, V>>) -> Self {
        match found {
            Some(node) => match node.is_word() {
                true => Found::Expr,
                false => Found::Node,
            },
            None => Found::None,
        }
    }
}

/// The number of spelling corrections [`NodeRef::corrections_with_ledger`] and
/// [`NodeRef::suggestions_with_ledger`] gather, by query length and whether an
/// exact prefix/word was found. Short queries admit fewer corrections (a 1-char
/// query has too many words within edit distance to be useful).
fn spellings_cap(query_len: usize, found: &Found) -> usize {
    match (query_len, found) {
        (0 | 1, _) => 0,
        (2, _) => 1,
        (3, Found::Node | Found::Expr) => 1,
        (3, Found::None) => 2,
        (_, Found::None) => 3,
        (_, _) => 2,
    }
}

/// Order the collected suggestions (any exact match first, then descending
/// percentile) and drop duplicate `expr_index`es, keeping the exact match when
/// one is present.
fn finish(mut suggestions: Vec<Suggestion>, found_index: Option<u32>) -> Vec<Suggestion> {
    suggestions.sort();
    if let Some(found_index) = found_index {
        suggestions.dedup_by(|a, b| {
            !a.is_match() && (a.expr_index == found_index || a.expr_index == b.expr_index)
        });
    } else {
        suggestions.dedup_by_key(|v| v.expr_index);
    }
    suggestions
}

impl<'a, V: Deref<Target = [u8]>> NodeRef<'a, V> {
    /// Resolve the exact match for `chars`: the matched node (if any), its
    /// `expr_index`, the match classification that sizes the spelling cap, and
    /// the seed suggestion list — a single `Matching` when the query is itself a
    /// word, otherwise empty. Shared by all three searchers.
    fn exact_match(
        &self,
        chars: &[char],
    ) -> (Option<NodeRef<'a, V>>, Option<u32>, Found, Vec<Suggestion>) {
        let found = self.find(chars);
        // One rank query resolves both the index and the seed `Matching`.
        let word_value = found.as_ref().and_then(|f| f.word_value());
        let found_index = word_value.map(|(_, expr_index)| expr_index);
        let match_type = Found::from(&found);
        let suggestions = match word_value {
            Some((percentile, expr_index)) => vec![Suggestion::matching(percentile, expr_index)],
            None => vec![],
        };
        (found, found_index, match_type, suggestions)
    }

    /// The full as-you-type result: the exact match, fuzzy spelling corrections
    /// ([`Self::dist_search`]) and frequency-ranked completions
    /// ([`Self::freq_search`]) merged into one ranked, de-duplicated list. When
    /// the query is not an exact prefix, completions are seeded from the distance
    /// walk's `altpaths` so a typo like `blla` still completes to `alltid`. For
    /// one job at a time use [`Self::completions_with_ledger`] (completion-only)
    /// or [`Self::corrections_with_ledger`] (fuzzy-only).
    pub fn suggestions_with_ledger<F, L: Ledger>(
        &self,
        search: &str,
        is_candidate: F,
        ledger: &mut L,
    ) -> Vec<Suggestion>
    where
        F: Fn(u32) -> bool,
    {
        let chars: Vec<char> = search.chars().collect();
        let (found, found_index, match_type, mut suggestions) = self.exact_match(&chars);

        let mut spellings = MaxArr::with_capacity(spellings_cap(chars.len(), &match_type));
        let mut altpaths = MaxArr::with_capacity(3);
        self.dist_search::<{ crate::editdist::BAND }, _, _>(
            &chars,
            &is_candidate,
            &mut spellings,
            &mut altpaths,
            ledger,
        );
        suggestions.extend(spellings.into_iter());

        if let Some(start) = found.as_ref().and_then(|n| n.children()) {
            let mut extensions = MaxArr::with_capacity(6 - suggestions.len());
            start.freq_search(&is_candidate, &mut extensions, search, ledger);
            suggestions.extend(extensions.into_iter());
        } else {
            let mut extensions = MaxArr::with_capacity(6 - suggestions.len());
            for altpath in altpaths.into_iter() {
                altpath
                    .node
                    .freq_search(&is_candidate, &mut extensions, &altpath.path, ledger);
            }
            suggestions.extend(extensions.into_iter());
        }
        finish(suggestions, found_index)
    }

    /// Fuzzy spelling corrections only: the exact match (when the query is a
    /// word) plus complete words within the configured edit distance of the
    /// query, via the Damerau-Levenshtein walk ([`Self::dist_search`]). Runs no
    /// completion sweep, so it never returns prefix extensions — the spell-check
    /// slice of [`Self::suggestions_with_ledger`].
    pub fn corrections_with_ledger<F, L: Ledger>(
        &self,
        search: &str,
        is_candidate: F,
        ledger: &mut L,
    ) -> Vec<Suggestion>
    where
        F: Fn(u32) -> bool,
    {
        let chars: Vec<char> = search.chars().collect();
        let (_found, found_index, match_type, mut suggestions) = self.exact_match(&chars);

        let mut spellings = MaxArr::with_capacity(spellings_cap(chars.len(), &match_type));
        // Corrections never seed completions, so the altpath collector is a
        // no-op sink: capacity 0 makes `dist_search` skip altpath bookkeeping.
        let mut altpaths = MaxArr::with_capacity(0);
        self.dist_search::<{ crate::editdist::BAND }, _, _>(
            &chars,
            &is_candidate,
            &mut spellings,
            &mut altpaths,
            ledger,
        );
        suggestions.extend(spellings.into_iter());

        finish(suggestions, found_index)
    }

    /// Prefix completions only: the exact match (when the query is a word) plus
    /// the highest-frequency words extending it, via the frequency sweep
    /// ([`Self::freq_search`]) from the matched node's children. Runs no
    /// Damerau-Levenshtein walk, so a query that is not an exact prefix of any
    /// word returns nothing — the autocomplete slice of
    /// [`Self::suggestions_with_ledger`].
    pub fn completions_with_ledger<F, L: Ledger>(
        &self,
        prefix: &str,
        is_candidate: F,
        ledger: &mut L,
    ) -> Vec<Suggestion>
    where
        F: Fn(u32) -> bool,
    {
        let chars: Vec<char> = prefix.chars().collect();
        let (found, found_index, _match_type, mut suggestions) = self.exact_match(&chars);

        if let Some(start) = found.and_then(|n| n.children()) {
            let mut extensions = MaxArr::with_capacity(6 - suggestions.len());
            start.freq_search(&is_candidate, &mut extensions, prefix, ledger);
            suggestions.extend(extensions.into_iter());
        }

        finish(suggestions, found_index)
    }

    pub fn find(&self, chars: &[char]) -> Option<NodeRef<'a, V>> {
        let mut cursor = self.clone();
        let mut to_find = chars.len();

        for (i, chr) in chars.iter().enumerate() {
            match cursor.move_to_sibling_matching(*chr) {
                true => to_find -= 1,
                false => return None,
            }
            if i + 1 == chars.len() {
                break;
            }
            cursor = cursor.children()?;
        }
        match to_find {
            0 => Some(cursor),
            _ => None,
        }
    }
}
