use super::Ledger;
use super::ledger::KeepDecision::{
    CandidateDiscarded, DistTooBig, MatchKept, NodeMatch, PercentileTooLow, WordKept,
};
use super::ledger::SearchDecision::{DoSearch, MinDistTooBig, NoChildren};

use super::{MaxArr, MaxVal, NodeRef, Suggestion};
use crate::editdist::{band_dist, base_band, fill_band, row_min};
use std::ops::Deref;

#[derive(Debug)]
pub struct AltPath<'a, V: Deref<Target = [u8]>> {
    pub node: NodeRef<'a, V>,
    pub max_child_percentile: u16,
    pub path: String,
}

impl<'a, V: Deref<Target = [u8]>> AltPath<'a, V> {
    fn from(node: &NodeRef<'a, V>, path: String) -> Self {
        Self {
            node: node.clone(),
            max_child_percentile: node.max_child_percentile(),
            path,
        }
    }
}
impl<V: Deref<Target = [u8]>> std::fmt::Display for AltPath<'_, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} {:.1}%",
            self.node.char(),
            self.max_child_percentile as f32 / 10.0
        )
    }
}

impl<V: Deref<Target = [u8]>> MaxVal<u16> for AltPath<'_, V> {
    fn value(&self) -> u16 {
        self.max_child_percentile
    }
}

type Spellings = MaxArr<u16, Suggestion>;
type AltPaths<'a, V> = MaxArr<u16, AltPath<'a, V>>;

impl<'a, V: Deref<Target = [u8]>> NodeRef<'a, V> {
    /// Walks the trie computing an incremental, banded Damerau-Levenshtein DP per
    /// node (see [`crate::editdist`]); the const `W` is the band width `2K+1` for
    /// the max edit distance `K`. Collects complete words within `K` as spelling
    /// corrections, and nodes whose word is within `K` (and that have promising
    /// descendants) as autocomplete seeds (`altpaths`). A subtree is pruned as
    /// soon as its smallest band value exceeds `K`.
    ///
    /// The walk is depth-first so the DP bands live in a single per-depth buffer
    /// stack (`rows`) reused across siblings — the whole search allocates
    /// `O(max trie depth)` fixed-width bands, not one per visited node.
    pub fn dist_search<const W: usize, F, L: Ledger>(
        &self,
        query: &[char],
        is_candidate: &F,
        spellings: &mut Spellings,
        altpaths: &mut AltPaths<'a, V>,
        ledger: &mut L,
    ) where
        F: Fn(u32) -> bool,
    {
        if spellings.capacity() == 0 {
            return;
        }
        let query_str = if L::ACTIVE {
            query.iter().collect::<String>()
        } else {
            String::new()
        };
        let mut rows: Vec<[u8; W]> = vec![base_band::<W>(query.len())];
        self.dist_walk::<W, F, L>(
            1,
            None,
            "",
            query,
            &query_str,
            &mut rows,
            is_candidate,
            spellings,
            altpaths,
            ledger,
        );
    }

    /// Processes the sibling group starting at `self`, all at trie `depth`,
    /// recursing depth-first into each node's children.
    #[allow(clippy::too_many_arguments)]
    fn dist_walk<const W: usize, F, L: Ledger>(
        &self,
        depth: usize,
        prev_char: Option<char>,
        parent_path: &str,
        query: &[char],
        query_str: &str,
        rows: &mut Vec<[u8; W]>,
        is_candidate: &F,
        spellings: &mut Spellings,
        altpaths: &mut AltPaths<'a, V>,
        ledger: &mut L,
    ) where
        F: Fn(u32) -> bool,
    {
        let n = query.len();
        // The max edit distance this band width encodes (W = 2K + 1).
        let k = ((W - 1) / 2) as u8;
        let mut cursor = self.clone();
        loop {
            let ch = cursor.char();
            fill_band::<W>(rows, depth, query, prev_char, ch);
            let dist = band_dist::<W>(&rows[depth], depth, n);
            let min = row_min(&rows[depth]);
            let path = if L::ACTIVE {
                format!("{parent_path}{ch}")
            } else {
                String::new()
            };

            // IS THE NODE A COMPLETE-WORD CORRECTION? Resolve the per-word value
            // lazily: the `is_word()` bit probe is cheap, the rank query behind
            // `word_value()` only fires on the in-distance word arm.
            let keep = if dist == 0 {
                // exact word — already emitted as `Matching` by `find()`;
                // an exact prefix node has nothing to keep.
                if cursor.is_word() {
                    MatchKept
                } else {
                    NodeMatch
                }
            } else if dist <= k {
                // a complete word within edit distance — a spelling correction
                match cursor.word_value() {
                    Some((percentile, expr_index)) => {
                        if Some(percentile) > spellings.min_value() {
                            if is_candidate(expr_index) {
                                spellings.add(Suggestion::spelling(percentile, expr_index));
                                WordKept
                            } else {
                                CandidateDiscarded
                            }
                        } else {
                            PercentileTooLow
                        }
                    }
                    None => DistTooBig,
                }
            } else {
                DistTooBig
            };

            // IS THE NODE AN AUTOCOMPLETE SEED? Seed when the whole query is within
            // `k` of this node's word — i.e. the node is the typed word corrected
            // (e.g. "blla" -> "alla"); `freq_search` then completes from it. Using
            // the full-word distance (not the prefix distance `min`) avoids seeding
            // shallow high-frequency nodes (a single corrected letter) that would
            // flood unrelated completions.
            if altpaths.capacity() > 0
                && dist <= k
                && Some(cursor.max_child_percentile()) > altpaths.min_value()
            {
                altpaths.add(AltPath::from(&cursor, path.clone()));
            }

            let children = cursor.children();
            let search = if min > k {
                MinDistTooBig
            } else if children.is_some() {
                DoSearch
            } else {
                NoChildren
            };
            // The banded DP cannot represent distances beyond `k`; out-of-range
            // cells read back as `OOB`. Report them to the ledger capped at `k+1`
            // ("> k") so the trace stays bounded and readable.
            ledger.record_dist(
                &path,
                query_str,
                dist.min(k + 1),
                min.min(k + 1),
                keep,
                Some(search),
            );

            // DESCEND UNLESS THE WHOLE SUBTREE IS TOO FAR
            if min <= k
                && let Some(children) = children
            {
                children.dist_walk::<W, F, L>(
                    depth + 1,
                    Some(ch),
                    &path,
                    query,
                    query_str,
                    rows,
                    is_candidate,
                    spellings,
                    altpaths,
                    ledger,
                );
            }

            if !cursor.move_to_next_sibling() {
                break;
            }
        }
    }
}

#[test]
fn test_direct() {
    use crate::search::StateLedger;
    use crate::trie::TreeFn;

    let mut generator = crate::Builder::new();
    generator.add_words(vec![
        ("abcd", 0, 1),
        ("a123", 0, 2),
        ("ab23", 0, 6),
        ("abc3", 0, 7),
    ]);
    generator.organize_into_folders(3);

    let nodes = generator.to_tree();
    let chars = "abcd".chars().collect::<Vec<char>>();
    let mut ledger = StateLedger::default();
    let mut spellings = Spellings::with_capacity(3);
    let mut altpaths = AltPaths::with_capacity(3);
    nodes.root().dist_search::<{ crate::editdist::BAND }, _, _>(
        &chars,
        &|_| true,
        &mut spellings,
        &mut altpaths,
        &mut ledger,
    );

    insta::assert_snapshot!(
        ledger
            .0
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join("\n")
    );
    // abc3 is the only word within edit distance 1 of "abcd" (substitute d->3).
    assert_eq!("Spelling 7 0.0%", spellings.to_string());
}
