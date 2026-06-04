use super::ledger::KeepDecision::{CandidateDiscarded, NotAWord, PercentileTooLow, WordKept};
use super::ledger::SearchDecision::{DoSearch, MaxChildPercentileTooSmall, NoChildren};
use super::{Ledger, LedgerLine};

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

        if L::ACTIVE {
            to_search.push_back((self.clone(), path.to_string()));
        } else {
            to_search.push_back((self.clone(), "".to_string()));
        }

        while let Some((mut childcursor, path)) = to_search.pop_front() {
            loop {
                let path = format!("{}{}", path, childcursor.char());
                let mut line = LedgerLine::freq(childcursor.clone(), &path);

                if let Some(expr_index) = childcursor.expr_index() {
                    if Some(childcursor.percentile()) > selector.min_value() {
                        if is_candidate(expr_index) {
                            selector
                                .add(Suggestion::extension(childcursor.percentile(), expr_index));
                            if L::ACTIVE {
                                line.keep(WordKept)
                            }
                        } else if L::ACTIVE {
                            line.keep(CandidateDiscarded);
                        }
                    } else if L::ACTIVE {
                        line.keep(PercentileTooLow)
                    }
                } else if L::ACTIVE {
                    line.keep(NotAWord)
                }

                if Some(childcursor.max_child_percentile()) < selector.min_value() {
                    if L::ACTIVE {
                        line.search(MaxChildPercentileTooSmall);
                    }
                } else if let Some(child) = childcursor.children() {
                    if L::ACTIVE {
                        line.search(DoSearch);
                    }

                    to_search.push_back((child, path.clone()));
                } else if L::ACTIVE {
                    line.search(NoChildren);
                }

                if L::ACTIVE {
                    ledger.push(line);
                }

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
