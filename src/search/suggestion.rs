use super::MaxVal;
use derive_more::Display;
use std::cmp::{Ord, Ordering, PartialOrd};

#[derive(Display, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum SuggestionType {
    Matching,
    Extension,
    Spelling,
    AltExt,
}

impl SuggestionType {
    pub fn as_char(&self) -> char {
        match self {
            Self::Matching => 'm',
            Self::Extension => 'e',
            Self::Spelling => 's',
            Self::AltExt => 'a',
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct Suggestion {
    pub expr_index: u32,
    pub percentile: u16,
    pub kind: SuggestionType,
}

impl Suggestion {
    pub fn spelling(percentile: u16, expr_index: u32) -> Self {
        Self {
            expr_index,
            percentile,
            kind: SuggestionType::Spelling,
        }
    }

    pub fn extension(percentile: u16, expr_index: u32) -> Self {
        Self {
            expr_index,
            percentile,
            kind: SuggestionType::Extension,
        }
    }

    pub fn matching(percentile: u16, expr_index: u32) -> Self {
        Self {
            expr_index,
            percentile,
            kind: SuggestionType::Matching,
        }
    }

    pub fn is_match(&self) -> bool {
        self.kind == SuggestionType::Matching
    }
}

impl PartialOrd for Suggestion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Suggestion {
    fn cmp(&self, other: &Self) -> Ordering {
        use self::SuggestionType::Matching;
        match (self.kind, other.kind) {
            (Matching, Matching) => other.percentile.cmp(&self.percentile),
            (Matching, _) => Ordering::Less,
            (_, Matching) => Ordering::Greater,
            _ => other.percentile.cmp(&self.percentile),
        }
    }
}

impl std::fmt::Display for Suggestion {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} {} {:.1}%",
            self.kind,
            self.expr_index,
            self.percentile as f32 / 10.0
        )
    }
}

impl MaxVal<u16> for Suggestion {
    fn value(&self) -> u16 {
        self.percentile
    }
}
