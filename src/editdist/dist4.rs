use derive_more::Display;
#[cfg(test)]
use insta::assert_snapshot;

#[derive(Display, Debug, PartialEq, Eq, Clone, Copy)]
pub enum Ptrn {
    Begin,
    AllSame,
    DifferentT4,
    // There is no InsertT1 because InsertT2 has a step of 2
    InsertT2,
    InsertT3,
    DeleteT1,
    DeleteT2,
    DeleteT3,
    SwitchT1T2,
    SwitchT2T3,
    SwitchT3T4,
    ReplaceT1,
    ReplaceT2,
    ReplaceT3,
    TooDifferent,
}

impl Ptrn {
    pub fn step(&self) -> u8 {
        match self {
            Self::DeleteT1 => 0,
            Self::InsertT2 => 2,
            _ => 1,
        }
    }

    pub fn distance_increment(&self) -> u8 {
        match self {
            Self::DifferentT4 => 1,
            Self::TooDifferent => 8,
            _ => 0,
        }
    }

    /// When there's a delete or insert then the two strings are mis-aligned until
    /// corrected by the step() func. While they are misaligned, if a hit is found
    /// the final distance needs to be compensated.
    pub fn misalign_corrector(&self) -> usize {
        match self {
            Self::DeleteT1 | Self::DeleteT2 | Self::DeleteT3 | Self::InsertT3 | Self::InsertT2 => 1,
            _ => 0,
        }
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn dist4_cmp(
    t1: Option<char>,
    t2: Option<char>,
    t3: Option<char>,
    t4: char,
    f1: Option<char>,
    f2: Option<char>,
    f3: Option<char>,
    f4: char,
) -> Ptrn {
    let t4 = Some(t4);
    let f4 = Some(f4);

    if (t1, t2, t3, t4) == (f1, f2, f3, f4) {
        return Ptrn::AllSame;
    }

    if (t1, t2, t3) == (f1, f2, f3) && t4 != f4 {
        return Ptrn::DifferentT4;
    }

    // 1234 <-> _234
    if (t2, t3, t4) == (f2, f3, f4) && t1 != f1 {
        return Ptrn::ReplaceT1;
    }

    // 1234 <-> 1_34
    if (t1, t3, t4) == (f1, f3, f4) && t2 != f2 {
        return Ptrn::ReplaceT2;
    }

    // 1234 <-> 12_4
    if (t1, t2, t4) == (f1, f2, f4) && t3 != f3 {
        return Ptrn::ReplaceT3;
    }

    // 1234 <-> 2134
    if (t1, t2, t3, t4) == (f2, f1, f3, f4) {
        return Ptrn::SwitchT1T2;
    }

    // 1234 <-> 1324
    if (t1, t2, t3, t4) == (f1, f3, f2, f4) {
        return Ptrn::SwitchT2T3;
    }

    // 1234 <-> 1243
    if (t1, t2, t3, t4) == (f1, f2, f4, f3) {
        return Ptrn::SwitchT3T4;
    }

    // 1234 <-> _123
    // There is no InsertT1 because InsertT2 has a step of 2

    // 1234 <-> 1_23
    if (t1, t2, t3) == (f1, f3, f4) {
        return Ptrn::InsertT2;
    }

    // 1234 <-> 12_3 (not 1243)
    if (t1, t2, t3) == (f1, f2, f4) && f3 != t4 {
        return Ptrn::InsertT3;
    }

    // 1234 <-> 2345
    if (t2, t3, t4) == (f1, f2, f3) {
        return Ptrn::DeleteT1;
    }

    // 1234 <-> 1345
    if (t1, t3, t4) == (f1, f2, f3) {
        return Ptrn::DeleteT2;
    }

    // 1234 <-> 1245 (not 1243)
    if (t1, t2, t4) == (f1, f2, f3) && t3 != f4 {
        return Ptrn::DeleteT3;
    }

    Ptrn::TooDifferent
}

#[test]
fn all_same() {
    assert_eq!(dist4("1234", "1234"), Ptrn::AllSame);
}

#[test]
fn different_f4() {
    assert_eq!(dist4("1234", "1230"), Ptrn::DifferentT4);
}

#[test]
fn insert_f2() {
    assert_eq!(dist4("1234", "1_23"), Ptrn::InsertT2);
}

#[test]
fn insert_f3() {
    assert_eq!(dist4("1234", "12_3"), Ptrn::InsertT3);
}

#[test]
fn delete_f1() {
    assert_eq!(dist4("1234", "2345"), Ptrn::DeleteT1);
}

#[test]
fn delete_f2() {
    assert_eq!(dist4("1234", "1345"), Ptrn::DeleteT2);
}

#[test]
fn delete_f3() {
    assert_eq!(dist4("1234", "1245"), Ptrn::DeleteT3);
}

#[test]
fn replace_f1() {
    assert_eq!(dist4("1234", "_234"), Ptrn::ReplaceT1);
}

#[test]
fn replace_f2() {
    assert_eq!(dist4("1234", "1_34"), Ptrn::ReplaceT2);
}

#[test]
fn replace_f3() {
    assert_eq!(dist4("1234", "12_4"), Ptrn::ReplaceT3);
}

#[test]
fn switch_f1_f2() {
    assert_eq!(dist4("1234", "2134"), Ptrn::SwitchT1T2);
}

#[test]
fn switch_f2_f3() {
    assert_eq!(dist4("1234", "1324"), Ptrn::SwitchT2T3);
}

#[test]
fn switch_f3_f4() {
    assert_eq!(dist4("1234", "1243"), Ptrn::SwitchT3T4);
}

#[cfg(test)]
fn dist4(t: &str, f: &str) -> Ptrn {
    let t: Vec<char> = t.chars().collect();
    let f: Vec<char> = f.chars().collect();

    dist4_cmp(
        Some(t[0]),
        Some(t[1]),
        Some(t[2]),
        t[3],
        Some(f[0]),
        Some(f[1]),
        Some(f[2]),
        f[3],
    )
}

#[cfg(test)]
type CharSet = (Option<char>, Option<char>, Option<char>, char);
#[cfg(test)]
fn char_vec(txt: &str) -> Vec<CharSet> {
    let chars: Vec<char> = txt.chars().collect();
    let mut vec: Vec<CharSet> = Vec::new();
    vec.push((None, None, None, chars[0]));
    vec.push((None, None, Some(chars[0]), chars[1]));
    vec.push((None, Some(chars[0]), Some(chars[1]), chars[2]));
    for i in 3..chars.len() {
        vec.push((
            Some(chars[i - 3]),
            Some(chars[i - 2]),
            Some(chars[i - 1]),
            chars[i],
        ));
    }
    vec
}

#[cfg(test)]
fn cmp_str((t1, t2, t3, t4): CharSet, (f1, f2, f3, f4): CharSet) -> (String, Ptrn) {
    let ptrn = dist4_cmp(t1, t2, t3, t4, f1, f2, f3, f4);
    (
        format!(
            "{}{}{}{} <-> {}{}{}{}  {}ᵈⁱˢᵗ {}ᵃˡⁱᵍⁿ {}ˢᵗᵉᵖ  {}",
            t1.unwrap_or(' '),
            t2.unwrap_or(' '),
            t3.unwrap_or(' '),
            t4,
            f1.unwrap_or(' '),
            f2.unwrap_or(' '),
            f3.unwrap_or(' '),
            f4,
            ptrn.distance_increment(),
            ptrn.misalign_corrector(),
            ptrn.step(),
            ptrn
        ),
        ptrn,
    )
}

#[cfg(test)]
fn cmp(term: &str, found: &str) -> String {
    let t = char_vec(term);
    let f = char_vec(found);

    let mut vec: Vec<String> = vec!["term - found".to_string()];
    let mut j = 0;
    for trm in t {
        let (txt, ptrn) = cmp_str(trm, f[j]);
        vec.push(txt);
        j += ptrn.step() as usize;
        if j >= f.len() {
            break;
        }
    }
    vec.join("\n")
}

#[test]
fn substitute_alla_start() {
    assert_snapshot!(cmp("alls¹²³", "_lls¹²³"))
}

#[test]
fn substitute_alla_mid() {
    assert_snapshot!(cmp("alla¹²³", "a_la¹²³"))
}

#[test]
fn substitute_alla_end() {
    assert_snapshot!(cmp("alla¹²³", "all_¹²³"))
}

#[test]
fn insert_alla_start() {
    assert_snapshot!(cmp("alla¹²³", "_alla¹²"))
}

#[test]
fn insert_alla_mid() {
    assert_snapshot!(cmp("alla¹²³", "al_la¹²"))
}

#[test]
fn insert_alla_end() {
    assert_snapshot!(cmp("alla¹", "alla_"))
}

#[test]
fn delete_allas_start() {
    assert_snapshot!(cmp("allas¹²", "llas¹²³"))
}

#[test]
fn delete_allas_mid() {
    assert_snapshot!(cmp("allas¹²", "alas¹²³"))
}

#[test]
fn delete_allas_end() {
    assert_snapshot!(cmp("allas¹²", "alla¹²³"))
}

#[test]
fn switch_flyga_start() {
    assert_snapshot!(cmp("flyga", "lfyga"))
}

#[test]
fn switch_flyga_mid() {
    assert_snapshot!(cmp("flyga", "flgya"))
}

#[test]
fn switch_flyga_end() {
    assert_snapshot!(cmp("flyga", "flyag"))
}

#[test]
fn cmp_oppen_dope() {
    assert_snapshot!(cmp("oppe", "Dope"))
}

#[test]
fn cmp_trab_tabi() {
    assert_snapshot!(cmp("trab", "tabi"))
}
