#[derive(Debug, Clone, PartialEq)]
pub struct Term<'a> {
    chars: &'a [char],
    idx: usize,
}

impl<'a> Term<'a> {
    pub fn new(chars: &'a [char]) -> Self {
        Self { chars, idx: 0 }
    }
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn incremented(&self, inc: usize) -> Self {
        Self {
            chars: self.chars,
            idx: self.idx + inc,
        }
    }

    pub fn remaining(&self) -> isize {
        self.len() as isize - 1 - self.idx as isize
    }

    /// **PREVIOUS PREVIOUS**
    #[inline]
    pub fn p_p(&self) -> Option<char> {
        match self.idx > 1 {
            true => self.get(self.idx - 2),
            false => None,
        }
    }

    /// **PREVIOUS**
    #[inline]
    pub fn p(&self) -> Option<char> {
        match self.idx > 0 {
            true => self.get(self.idx - 1),
            false => None,
        }
    }

    /// **CURRENT**
    #[inline]
    pub fn c(&self) -> Option<char> {
        self.get(self.idx)
    }

    /// **NEXT**
    #[inline]
    pub fn n(&self) -> Option<char> {
        self.get(self.idx + 1)
    }
    #[inline]
    pub fn relative(&self, offset: isize) -> Option<char> {
        let idx = self.idx as isize + offset;
        match idx >= 0 && idx < self.len() as isize {
            true => Some(self.chars[idx as usize]),
            false => None,
        }
    }

    #[inline]
    pub fn get(&self, idx: usize) -> Option<char> {
        match idx < self.len() {
            true => Some(self.chars[idx]),
            false => None,
        }
    }
}

impl std::fmt::Display for Term<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut chars = self.chars.to_vec();
        if self.idx >= self.len() {
            chars.extend((0..(self.idx + 1 - self.len())).map(|_| ' '));
        }
        write!(f, "{}", underscored(self.idx, &chars))
    }
}

fn underscored(idx: usize, chars: &[char]) -> String {
    let mut txt = String::with_capacity(chars.len() + 1);
    for (i, chr) in chars.iter().enumerate() {
        match (i == idx, *chr == ' ') {
            (true, true) => txt.push('_'),
            (true, false) => {
                txt.push(*chr);
                txt.push('\u{0332}');
            }
            _ => txt.push(*chr),
        }
    }
    txt
}

#[test]
fn test_basic() {
    let chars = "Hello".chars().collect::<Vec<char>>();
    let term = Term::new(&chars);

    assert_eq!("H̲ello", term.to_string());
    assert_eq!(Some('H'), term.get(0));
    assert_eq!(Some('e'), term.get(1));
    assert_eq!(Some('l'), term.get(2));
    assert_eq!(Some('l'), term.get(3));
    assert_eq!(Some('o'), term.get(4));
    assert_eq!(None, term.get(5));
}

#[cfg(test)]
fn relatives(term: &Term) -> (Option<char>, Option<char>, Option<char>) {
    (term.relative(-1), term.relative(0), term.relative(1))
}

#[test]
fn test_relative() {
    let chars = "12".chars().collect::<Vec<char>>();
    let term = Term::new(&chars);
    assert_eq!((None, Some('1'), Some('2')), relatives(&term));

    let term = term.incremented(1);
    assert_eq!((Some('1'), Some('2'), None), relatives(&term));

    let term = term.incremented(1);
    assert_eq!((Some('2'), None, None), relatives(&term));
}

#[test]
fn test_remaining() {
    let chars = "Hi".chars().collect::<Vec<char>>();
    let term = Term::new(&chars);
    assert_eq!(1, term.remaining());
    assert_eq!("H̲i", term.to_string());

    let term = term.incremented(1);
    assert_eq!(0, term.remaining());
    assert_eq!("Hi̲", term.to_string());

    let term = term.incremented(1);
    assert_eq!(-1, term.remaining());
    assert_eq!("Hi_", term.to_string());

    let term = term.incremented(1);
    assert_eq!(-2, term.remaining());
    assert_eq!("Hi _", term.to_string());
}

#[test]
fn test_prev_and_next() {
    let chars = "Hello".chars().collect::<Vec<char>>();
    let term = Term::new(&chars);

    assert_eq!(None, term.p_p());
    assert_eq!(None, term.p());
    assert_eq!(Some('H'), term.c());
    assert_eq!(Some('e'), term.n());

    let term = term.incremented(2);
    assert_eq!(Some('H'), term.p_p());
    assert_eq!(Some('e'), term.p());
    assert_eq!(Some('l'), term.c());
    assert_eq!(Some('l'), term.n());

    let term = term.incremented(2);
    assert_eq!(Some('l'), term.p_p());
    assert_eq!(Some('l'), term.p());
    assert_eq!(Some('o'), term.c());
    assert_eq!(None, term.n());

    let term = term.incremented(2);
    assert_eq!(Some('o'), term.p_p());
    assert_eq!(None, term.p());
    assert_eq!(None, term.c());
    assert_eq!(None, term.n());

    let term = term.incremented(1);
    assert_eq!(None, term.p_p());
    assert_eq!(None, term.p());
    assert_eq!(None, term.c());
    assert_eq!(None, term.n());
}
