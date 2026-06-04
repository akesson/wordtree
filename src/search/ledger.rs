use std::ops::Deref;

use super::NodeRef;

pub enum KeepDecision {
    MatchKept,
    NodeMatch,
    WordKept,
    PercentileTooLow,
    DistTooBig,
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
            Self::DistTooBig => "dist too big",
            Self::NotAWord => "not a word",
            Self::CandidateDiscarded => "candidate discarded",
        }
    }
}

pub enum SearchDecision {
    MinDistTooBig,
    DoSearch,
    MaxChildPercentileTooSmall,
    NoChildren,
}

impl SearchDecision {
    pub fn str(&self) -> &'static str {
        match self {
            Self::MinDistTooBig => "min dist too big",
            Self::DoSearch => "do search",
            Self::MaxChildPercentileTooSmall => "max child percentile too small",
            Self::NoChildren => "no children",
        }
    }
}

pub enum LineType {
    Dist {
        query: String,
        /// edit distance of the whole query to this node's word
        dist: u8,
        /// smallest distance in the row (prefix distance / prune key)
        min: u8,
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
            Self::Dist { query, dist, min } => {
                write!(f, "{}, dist({}), min({})", query, dist, min)
            }
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
    /// the word spelled out by the path from the root to this node
    pub path: String,
}

impl LedgerLine {
    pub fn dist(
        word: &str,
        query: &str,
        dist: u8,
        min: u8,
        keep: KeepDecision,
        search: Option<SearchDecision>,
    ) -> Self {
        Self {
            line: LineType::Dist {
                query: query.to_string(),
                dist,
                min,
            },
            keep: Some(keep),
            search,
            path: word.to_string(),
        }
    }

    pub fn freq<V: Deref<Target = [u8]>>(
        node: &NodeRef<'_, V>,
        path: &str,
        keep: KeepDecision,
        search: SearchDecision,
    ) -> Self {
        Self {
            line: LineType::Freq {
                chr: node.char(),
                percentile: node.percentile(),
                max_child_percentile: node.max_child_percentile(),
            },
            keep: Some(keep),
            search: Some(search),
            path: path.to_string(),
        }
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
