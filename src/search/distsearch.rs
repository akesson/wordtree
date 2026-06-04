use super::Ledger;
use super::ledger::KeepDecision::{
    AbsDistTooBig, CandidateDiscarded, MatchKept, NodeMatch as KeepNodeMatch, PercentileTooLow,
    WordKept,
};
use super::ledger::SearchDecision::{DoSearch, RelDistTooBig, SearchTermEnd};

use super::{DistInfo, MaxArr, MaxVal, NodeRef, Suggestion, Term};
use std::collections::VecDeque;
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
    /// searches so that the node arr is read sequentially (though with jumps)
    pub fn dist_search<F, L: Ledger>(
        &self,
        term: &Term,
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

        let mut to_search: VecDeque<(NodeRef<'a, V>, DistInfo)> = VecDeque::from(vec![(
            self.clone(),
            DistInfo::start(term.clone(), L::ACTIVE),
        )]);

        while let Some((mut childcursor, prev_state)) = to_search.pop_front() {
            while let Some(state) = prev_state.next(childcursor.char()) {
                // IS THE NODE A MATCH THAT SHOULD BE KEPT
                let keep = match (state.abs_dist(), childcursor.expr_index()) {
                    // word match
                    (0, Some(_)) => MatchKept,
                    // node match
                    (0, None) => KeepNodeMatch,
                    // alternative spelling
                    (1, Some(expr_index)) => {
                        if Some(childcursor.percentile()) > spellings.min_value() {
                            if is_candidate(expr_index) {
                                spellings.add(Suggestion::spelling(
                                    childcursor.percentile(),
                                    expr_index,
                                ));
                                WordKept
                            } else {
                                CandidateDiscarded
                            }
                        } else {
                            PercentileTooLow
                        }
                    }
                    // more than one spelling correction
                    (_, _) => AbsDistTooBig,
                };

                // IS THE NODE AN ALTERNATIVE PATH (NON-FINISHED SPELLING)
                if state.abs_dist() <= 1
                    && Some(childcursor.max_child_percentile()) > altpaths.min_value()
                {
                    let path = if L::ACTIVE {
                        state.path()
                    } else {
                        String::new()
                    };
                    altpaths.add(AltPath::from(&childcursor, path));
                }

                // SHOULD THE NODE BE ADDED TO THE TO_SEARCH QUEUE.
                // `record_dist` borrows `state`, so it runs before the
                // `DoSearch` arm moves `state` into the queue.
                if state.term.remaining() <= 0 {
                    ledger.record_dist(&state, keep, Some(SearchTermEnd));
                } else if state.rel_dist() >= 2 {
                    ledger.record_dist(&state, keep, Some(RelDistTooBig));
                } else if let Some(children) = childcursor.children() {
                    ledger.record_dist(&state, keep, Some(DoSearch));
                    to_search.push_back((children, state));
                } else {
                    ledger.record_dist(&state, keep, None);
                }

                if !childcursor.move_to_next_sibling() {
                    break;
                }
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
    let term = Term::new(&chars);
    let mut spellings = Spellings::with_capacity(3);
    let mut altpaths = AltPaths::with_capacity(3);
    nodes
        .root()
        .dist_search(&term, &|_| true, &mut spellings, &mut altpaths, &mut ledger);

    insta::assert_snapshot!(
        ledger
            .0
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join("\n")
    );
    assert_eq!("Spelling 7 0.0%", spellings.to_string());
}
