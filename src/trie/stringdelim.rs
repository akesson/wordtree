pub trait StringDelim {
    fn prefix_two_char(prefix: &str, chr1: char, chr2: char) -> String;
    fn prefix_one_char(prefix: &str, chr: char) -> String;
}

impl StringDelim for String {
    #[inline]
    fn prefix_two_char(prefix: &str, chr1: char, chr2: char) -> String {
        let mut string = String::with_capacity(prefix.len() + 2);
        string.push_str(prefix);
        string.push(chr1);
        string.push(chr2);
        string
    }

    #[inline]
    fn prefix_one_char(prefix: &str, chr: char) -> String {
        let mut string = String::with_capacity(prefix.len() + 2);
        string.push_str(prefix);
        string.push(chr);
        string
    }
}
