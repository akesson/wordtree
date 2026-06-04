use std::ops::Deref;

use super::{DistInfo, Ledger, NodeRef};

pub enum KeepDecision {
    MatchKept,
    NodeMatch,
    WordKept,
    PercentileTooLow,
    AbsDistTooBig,
    NotAWord,
    CandidateDiscarded,
}

impl KeepDecision {
    pub fn str(&self) -> &'static str {
        match self {
            Self::MatchKept => "word match kept",
            Self::NodeMatch => "node match",
            Self::WordKept => "word kept",
            Self::PercentileTooLow => "too low percentile",
            Self::AbsDistTooBig => "abs. dist. too big",
            Self::NotAWord => "not a word",
            Self::CandidateDiscarded => "candidate discarded",
        }
    }
}

pub enum SearchDecision {
    RelDistTooBig,
    SearchTermEnd,
    DoSearch,
    MaxChildPercentileTooSmall,
    NoChildren,
}

impl SearchDecision {
    pub fn str(&self) -> &'static str {
        match self {
            Self::RelDistTooBig => "rel. dist. too big",
            Self::DoSearch => "do search",
            Self::SearchTermEnd => "search term end",
            Self::MaxChildPercentileTooSmall => "max child percentile too small",
            Self::NoChildren => "no children",
        }
    }
}

pub enum LineType {
    Dist {
        term: String,
        state: String,
        distance: u8,
        increment: u8,
    },
    Freq {
        chr: char,
        percentile: u16,
        max_child_percentile: u16,
    },
}

impl std::fmt::Display for LineType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Dist {
                term,
                state,
                distance,
                increment,
            } => write!(
                f,
                "{}: {}, dist({}), incr({})",
                term, state, distance, increment,
            ),
            Self::Freq {
                chr,
                percentile,
                max_child_percentile,
            } => write!(
                f,
                "{} {:.1}% (max {:.1}%)",
                chr,
                *percentile as f32 / 10.0,
                *max_child_percentile as f32 / 10.0
            ),
        }
    }
}

pub struct LedgerLine {
    pub line: LineType,
    pub keep: Option<KeepDecision>,
    pub search: Option<SearchDecision>,
    pub path: String,
}

impl LedgerLine {
    pub fn dist<L: Ledger>(state: &DistInfo) -> Self {
        Self {
            line: LineType::Dist {
                term: state.term.to_string(),
                state: state.matched_pattern.to_string(),
                distance: state.distance,
                increment: state.matched_pattern.step(),
            },
            keep: None,
            search: None,
            path: state.found_string(),
        }
    }

    pub fn freq<V: Deref<Target = [u8]>>(node: NodeRef<'_, V>, path: &str) -> Self {
        Self {
            line: LineType::Freq {
                chr: node.char(),
                percentile: node.percentile(),
                max_child_percentile: node.max_child_percentile(),
            },
            keep: None,
            search: None,
            path: path.to_string(),
        }
    }

    pub fn search(&mut self, decision: SearchDecision) {
        self.search = Some(decision)
    }

    pub fn keep(&mut self, decision: KeepDecision) {
        self.keep = Some(decision)
    }
}

impl std::fmt::Display for LedgerLine {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} <-> {}, keep: {}, search: {}",
            self.path,
            self.line,
            self.keep.as_ref().map(|d| d.str()).unwrap_or("-"),
            self.search.as_ref().map(|d| d.str()).unwrap_or("-"),
        )
    }
}
