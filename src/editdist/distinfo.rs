use super::dist4::{Ptrn, dist4_cmp};
use crate::Term;
#[derive(Debug, PartialEq, Clone)]
pub struct DistInfo<'a> {
    pub term: Term<'a>,
    /// chars in reading order f1 f2 f3
    pub f1: Option<char>,
    pub f2: Option<char>,
    pub f3: Option<char>,
    pub f4: char,
    pub distance: u8,
    pub matched_pattern: Ptrn,
    pub chars: Vec<char>,
    /// When false (the `NoLedger` case) the `chars` trace is not accumulated,
    /// avoiding a per-node `Vec<char>` clone whose only consumer is `path()`.
    track_path: bool,
}

impl std::fmt::Display for DistInfo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} <-> {}, {}ᵈ {}ˢ {}",
            self.term_string(),
            self.found_string(),
            self.distance,
            self.matched_pattern.step(),
            self.matched_pattern
        )
    }
}

impl<'a> DistInfo<'a> {
    pub fn start(term: Term<'a>, track_path: bool) -> Self {
        Self {
            term,
            f1: None,
            f2: None,
            f3: None,
            f4: ' ',
            distance: 0,
            matched_pattern: Ptrn::Begin,
            chars: Vec::new(),
            track_path,
        }
    }

    #[inline]
    fn cloned(
        &self,
        term: Term<'a>,
        f1: Option<char>,
        f2: Option<char>,
        f3: Option<char>,
        f4: char,
    ) -> Self {
        DistInfo {
            term,
            f1,
            f2,
            f3,
            f4,
            distance: self.distance,
            matched_pattern: self.matched_pattern,
            chars: if self.track_path {
                let mut chars = self.chars.clone();
                chars.push(f4);
                chars
            } else {
                Vec::new()
            },
            track_path: self.track_path,
        }
    }

    pub fn next(&self, found: char) -> Option<Self> {
        let mut nxt = if self.matched_pattern == Ptrn::Begin {
            self.cloned(self.term.clone(), None, None, None, found)
        } else {
            self.cloned(
                self.term.incremented(self.matched_pattern.step() as usize),
                self.f2,
                self.f3,
                Some(self.f4),
                found,
            )
        };
        let t4 = nxt.term.relative(0)?;
        let (t1, t2, t3) = (
            nxt.term.relative(-3),
            nxt.term.relative(-2),
            nxt.term.relative(-1),
        );
        let ptrn = dist4_cmp(t1, t2, t3, t4, nxt.f1, nxt.f2, nxt.f3, nxt.f4);
        nxt.distance += ptrn.distance_increment();
        nxt.matched_pattern = ptrn;
        Some(nxt)
    }

    pub fn abs_dist(&self) -> usize {
        self.distance as usize
            + self.term.remaining().unsigned_abs()
            + self.matched_pattern.misalign_corrector()
    }

    pub fn rel_dist(&self) -> u8 {
        self.distance
    }

    pub fn term_string(&self) -> String {
        format!(
            "{}{}{}{}",
            self.term.relative(-3).unwrap_or(' '),
            self.term.relative(-2).unwrap_or(' '),
            self.term.relative(-1).unwrap_or(' '),
            self.term.relative(0).unwrap_or(' '),
        )
    }

    pub fn found_string(&self) -> String {
        format!(
            "{}{}{}{}",
            self.f1.unwrap_or(' '),
            self.f2.unwrap_or(' '),
            self.f3.unwrap_or(' '),
            self.f4,
        )
    }

    pub fn path(&self) -> String {
        String::from_iter(self.chars.iter())
    }

    #[cfg(test)]
    fn dist_str(&self) -> String {
        format!("rel: {}, abs: {}", self.rel_dist(), self.abs_dist())
    }
}

#[test]
fn basic() {
    let chars = "hello".chars().collect::<Vec<char>>();

    let curr = DistInfo::start(Term::new(&chars), true);
    assert_eq!(curr.matched_pattern, Ptrn::Begin);

    let curr = curr.next('h').unwrap();
    assert_eq!(curr.to_string(), "   h <->    h, 0ᵈ 1ˢ AllSame");
    assert_eq!(curr.dist_str(), "rel: 0, abs: 4");

    let curr = curr.next('e').unwrap();
    assert_eq!(curr.to_string(), "  he <->   he, 0ᵈ 1ˢ AllSame");
    assert_eq!(curr.dist_str(), "rel: 0, abs: 3");

    let curr = curr.next('f').unwrap();
    assert_eq!(curr.to_string(), " hel <->  hef, 1ᵈ 1ˢ DifferentT4");
    assert_eq!(curr.dist_str(), "rel: 1, abs: 3");

    let curr = curr.next('l').unwrap();
    assert_eq!(curr.to_string(), "hell <-> hefl, 1ᵈ 1ˢ ReplaceT3");
    assert_eq!(curr.dist_str(), "rel: 1, abs: 2");

    let curr = curr.next('o').unwrap();
    assert_eq!(curr.to_string(), "ello <-> eflo, 1ᵈ 1ˢ ReplaceT2");
    assert_eq!(curr.dist_str(), "rel: 1, abs: 1");
}
