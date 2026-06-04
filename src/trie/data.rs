#[derive(Eq, PartialEq, Copy, Clone)]
pub struct TwoBoolTwoU10 {
    pub b1: u8,
    pub b2: u8,
    pub b3: u8,
}

/**
 * Byte one
 */
impl TwoBoolTwoU10 {
    pub fn new(bool1: bool, bool2: bool, num1: u16, num2: u16) -> Self {
        if num1 > 1000 || num2 > 1000 {
            panic!("Percentile bigger than 1000");
        }
        let v1 = num1.to_le_bytes();
        let v2 = num2.to_le_bytes();

        let b1 = v1[0];
        let b2 = v2[0];

        let mut b3: u8 = 0;
        if bool1 {
            b3 |= 0b1000_0000;
        }
        if bool2 {
            b3 |= 0b0100_0000;
        }

        let b3v1 = v1[1] & 0b0000_0011;
        let b3v2 = v2[1] & 0b0000_0011;
        b3 |= b3v1;
        b3 |= b3v2 << 2;

        // println!("b1: {}, b2: {} b3: {:#b}", b1, b2, b3);

        Self { b1, b2, b3 }
    }

    pub fn get_num1(self) -> u16 {
        self.b1 as u16 | ((self.b3 & 0b0000_0011) as u16) << 8
    }
    pub fn get_num2(self) -> u16 {
        self.b2 as u16 | ((self.b3 & 0b0000_1100) as u16) << 6
    }

    pub fn get_bool1(self) -> bool {
        (self.b3 & 0b1000_0000) != 0
    }
    pub fn get_bool2(self) -> bool {
        (self.b3 & 0b0100_0000) != 0
    }

    pub fn from_raw(b1: u8, b2: u8, b3: u8) -> Self {
        Self { b1, b2, b3 }
    }
}

impl std::fmt::Debug for TwoBoolTwoU10 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TwoBoolTwoU10")
            .field("bool1", &self.get_bool1())
            .field("bool2", &self.get_bool2())
            .field("num1", &self.get_num1())
            .field("num2", &self.get_num2())
            .finish()
    }
}

impl std::fmt::Display for TwoBoolTwoU10 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}₁, {}₂, {}₁, {}₂",
            self.get_bool1(),
            self.get_bool2(),
            self.get_num1(),
            self.get_num2(),
        )
    }
}

#[test]
fn test_two() {
    assert_eq!(
        "false₁, false₂, 1₁, 1₂",
        TwoBoolTwoU10::new(false, false, 1, 1).to_string()
    );
    assert_eq!(
        "true₁, true₂, 0₁, 0₂",
        TwoBoolTwoU10::new(true, true, 0, 0).to_string()
    );

    assert_eq!(
        "true₁, true₂, 1000₁, 1000₂",
        TwoBoolTwoU10::new(true, true, 1000, 1000).to_string()
    );

    assert_eq!(
        "false₁, true₂, 0₁, 1000₂",
        TwoBoolTwoU10::new(false, true, 0, 1000).to_string()
    );

    assert_eq!(
        "true₁, false₂, 1000₁, 0₂",
        TwoBoolTwoU10::new(true, false, 1000, 0).to_string()
    );
}
