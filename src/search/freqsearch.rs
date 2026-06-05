use super::Ledger;
use super::ledger::KeepDecision::{CandidateDiscarded, NotAWord, PercentileTooLow, WordKept};
use super::ledger::SearchDecision::{DoSearch, MaxChildPercentileTooSmall, NoChildren};

use super::{MaxArr, NodeRef, Suggestion};
use std::collections::VecDeque;
use std::ops::Deref;

pub type TracePath = String;

impl<'a, V: Deref<Target = [u8]>> NodeRef<'a, V> {
    /// searches so that the node arr is read sequentially (though with jumps)
    pub fn freq_search<F, L: Ledger>(
        &self,
        is_candidate: &F,
        selector: &mut MaxArr<u16, Suggestion>,
        path: &str,
        ledger: &mut L,
    ) where
        F: Fn(u32) -> bool,
    {
        let mut to_search: VecDeque<(NodeRef<'a, V>, TracePath)> = VecDeque::new();

        let seed = if L::ACTIVE {
            path.to_string()
        } else {
            String::new()
        };
        to_search.push_back((self.clone(), seed));

        while let Some((mut childcursor, path)) = to_search.pop_front() {
            loop {
                let path = if L::ACTIVE {
                    format!("{}{}", path, childcursor.char())
                } else {
                    String::new()
                };

                let keep = if let Some((percentile, expr_index)) = childcursor.word_value() {
                    if Some(percentile) > selector.min_value() {
                        if is_candidate(expr_index) {
                            selector.add(Suggestion::extension(percentile, expr_index));
                            WordKept
                        } else {
                            CandidateDiscarded
                        }
                    } else {
                        PercentileTooLow
                    }
                } else {
                    NotAWord
                };

                // The queue receives a fresh child node (not the borrowed
                // `childcursor`), so the push can happen in-branch and
                // `record_freq` records once afterwards.
                let search = if Some(childcursor.max_child_percentile()) < selector.min_value() {
                    MaxChildPercentileTooSmall
                } else if let Some(child) = childcursor.children() {
                    to_search.push_back((child, path.clone()));
                    DoSearch
                } else {
                    NoChildren
                };

                ledger.record_freq(&childcursor, &path, keep, search);

                if !childcursor.move_to_next_sibling() {
                    break;
                }
            }
        }
    }
}

#[test]
fn test_basic() {
    use crate::search::NoLedger;
    use crate::trie::TreeFn;

    let mut generator = crate::Builder::new();
    generator.add_words(vec![
        ("abcd", 121, 1),
        ("a123", 132, 2),
        ("ab23", 112, 6),
        ("abc3", 143, 7),
    ]);
    generator.organize_into_folders(3);

    let nodes = generator.to_tree();
    let mut ledger = NoLedger::default();
    let mut selector = MaxArr::<u16, Suggestion>::with_capacity(3);
    nodes
        .root()
        .freq_search(&|_| true, &mut selector, "hi", &mut ledger);

    // insta::assert_snapshot!(ledger
    //     .iter()
    //     .map(|e| e.to_string())
    //     .collect::<Vec<String>>()
    //     .join("\n"));

    assert_eq!(
        "Extension 2 13.2%, Extension 1 12.1%, Extension 7 14.3%",
        selector.to_string()
    )
}

#[test]
fn comp_opt() {
    assert!(Some(3) > Some(2));
    assert!(Some(2) == Some(2));
    assert!(None < Some(2));
    assert!(Some(2) > None);
    assert!(None::<usize> <= None);
}
