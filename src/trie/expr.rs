use std::fmt;

#[derive(Debug, PartialEq)]
pub struct ExprIndex(u32);

const U24_MAX: u32 = 16777215;

impl ExprIndex {
    pub fn new(expr_index: Option<u32>) -> Self {
        Self(match expr_index {
            Some(idx) if idx >= U24_MAX => {
                panic!(
                    "Invalid expr_index, cannot be >= 2^24 - 1 ({}), was: {}",
                    U24_MAX, idx
                )
            }
            Some(idx) => idx,
            None => U24_MAX,
        })
    }

    #[inline]
    pub fn index(&self) -> Option<u32> {
        match self.0 {
            U24_MAX => None,
            val => Some(val),
        }
    }

    #[inline]
    pub fn _is_word(&self) -> bool {
        self.0 != U24_MAX
    }

    pub fn inner(&self) -> &u32 {
        &self.0
    }

    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}
impl fmt::Display for ExprIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.index()
                .map(|i| i.to_string())
                .unwrap_or_else(|| "-".to_string())
        )
    }
}
