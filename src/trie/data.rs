/// The 2-byte node `info` word: two flags plus the 10-bit
/// `max_child_percentile` pruning bound. The node's own `percentile` and
/// `expr_index` are *not* here — they live in the side `values` table (see
/// [`super::rank`]), because they only exist on word nodes.
#[derive(Eq, PartialEq, Copy, Clone)]
pub struct Info {
    pub b1: u8,
    pub b2: u8,
}

impl Info {
    pub fn new(is_folder: bool, is_last_sibling: bool, max_child_percentile: u16) -> Self {
        if max_child_percentile > 1000 {
            panic!("Percentile bigger than 1000");
        }
        let v = max_child_percentile.to_le_bytes();
        // b1: low 8 bits of the percentile; b2: flags in the high bits, the
        // remaining 2 percentile bits in the low bits.
        let mut b2 = v[1] & 0b0000_0011;
        if is_folder {
            b2 |= 0b1000_0000;
        }
        if is_last_sibling {
            b2 |= 0b0100_0000;
        }
        Self { b1: v[0], b2 }
    }

    pub fn max_child_percentile(self) -> u16 {
        self.b1 as u16 | ((self.b2 & 0b0000_0011) as u16) << 8
    }

    pub fn is_folder(self) -> bool {
        (self.b2 & 0b1000_0000) != 0
    }

    pub fn is_last_sibling(self) -> bool {
        (self.b2 & 0b0100_0000) != 0
    }

    pub fn from_raw(b1: u8, b2: u8) -> Self {
        Self { b1, b2 }
    }
}

impl std::fmt::Debug for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Info")
            .field("is_folder", &self.is_folder())
            .field("is_last_sibling", &self.is_last_sibling())
            .field("max_child_percentile", &self.max_child_percentile())
            .finish()
    }
}

#[test]
fn test_info() {
    fn roundtrip(is_folder: bool, is_last_sibling: bool, max_child: u16) {
        let info = Info::new(is_folder, is_last_sibling, max_child);
        let back = Info::from_raw(info.b1, info.b2);
        assert_eq!(back.is_folder(), is_folder);
        assert_eq!(back.is_last_sibling(), is_last_sibling);
        assert_eq!(back.max_child_percentile(), max_child);
    }
    roundtrip(false, false, 0);
    roundtrip(true, true, 1000);
    roundtrip(false, true, 1000);
    roundtrip(true, false, 0);
    roundtrip(true, true, 513);
}
