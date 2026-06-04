mod distsearch;
mod freqsearch;
pub mod ledger;
mod maxarr;
mod suggestion;
mod term;
#[cfg(test)]
mod tests;

use std::ops::Deref;

use super::NodeRef;
use super::editdist::DistInfo;
use ledger::{KeepDecision, SearchDecision};
use maxarr::{MaxArr, MaxVal};
pub use suggestion::Suggestion;
pub use term::Term;

pub type LedgerLine = ledger::LedgerLine;

/// Records the per-node decisions made while searching. The production
/// [`NoLedger`] implements every method as an inlined no-op, so the search
/// builds no `LedgerLine`s (and allocates no trace strings) when `ACTIVE` is
/// false; [`StateLedger`] keeps the full trace for the TUI/tests.
pub trait Ledger: Default {
    const ACTIVE: bool;
    fn record_dist(&mut self, state: &DistInfo, keep: KeepDecision, search: Option<SearchDecision>);
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
        _state: &DistInfo,
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
        state: &DistInfo,
        keep: KeepDecision,
        search: Option<SearchDecision>,
    ) {
        self.0.push(LedgerLine::dist(state, keep, search));
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
            Some(node) => match node.expr_index().is_some() {
                true => Found::Expr,
                false => Found::Node,
            },
            None => Found::None,
        }
    }
}

impl<'a, V: Deref<Target = [u8]>> NodeRef<'a, V> {
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
        let term = Term::new(&chars);

        let found = self.find(&chars);
        let found_index = found.as_ref().and_then(|f| f.expr_index());
        let match_type = Found::from(&found);
        let mut suggestions: Vec<Suggestion> =
            match found.as_ref().map(|n| (n.percentile(), n.expr_index())) {
                Some((percentile, Some(expr_index))) => {
                    vec![Suggestion::matching(percentile, expr_index)]
                }
                _ => vec![],
            };

        let spellings_cap = match (chars.len(), &match_type) {
            (0 | 1, _) => 0,
            (2, _) => 1,
            (3, Found::Node | Found::Expr) => 1,
            (3, Found::None) => 2,
            (_, Found::None) => 3,
            (_, _) => 2,
        };

        let mut spellings = MaxArr::with_capacity(spellings_cap);
        let mut altpaths = MaxArr::with_capacity(3);
        self.dist_search(&term, &is_candidate, &mut spellings, &mut altpaths, ledger);
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
        // sorts the suggestions so that any match is always first and then the remaining
        // entries are sorted on descending percentile
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
