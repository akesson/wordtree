//! Incremental, **banded** edit-distance carried down the trie: one
//! dynamic-programming band per node holding the cells of the conceptual row
//! `row[j] = edit_distance(query[0..j], word_spelled_to_this_node)` that lie
//! within `±K` of the diagonal (`K` = the max edit distance the search keeps).
//!
//! Walking the trie, a node at depth `d` (its path spells a `d`-character word)
//! derives its band from its parent's band (`prev`, depth `d-1`) and
//! grandparent's band (`pp`, depth `d-2`, needed only for transposition), given
//! the node's character `ch` and the parent's character.
//!
//! ## Why a band
//!
//! `row[j] = edit_distance(query[0..j], word_d)` is at least `|j - d|` (you need
//! that many indels just to fix the length difference). So any cell with
//! `|j - d| > K` is already `> K` and can never be a kept correction nor lower a
//! surviving `row_min`. We therefore store only the `2K+1` cells with
//! `|j - d| <= K`. By Ukkonen's argument the optimal alignment to any cell whose
//! true distance is `<= K` stays inside that band, so **every in-range value the
//! search acts on is computed exactly**; cells whose true distance exceeds `K`
//! may be over-estimated, but they are `> K` either way, so all keep/prune
//! decisions are identical to a full-row DP. Banding is invisible to results.
//!
//! ## Local-index convention
//!
//! A band is `[u8; W]` with `W = 2K + 1`. Local index `o ∈ 0..W` maps to query
//! column `j = d + o - K`; the diagonal (`j = d`) sits at `o = K`. A cell whose
//! column falls outside `[0, n]` (`n` = query length), or whose recurrence has no
//! in-band predecessor, holds the [`OOB`] sentinel. The band shift turns every
//! neighbour into a *constant* local offset:
//!
//! ```text
//! cur[o] = min(
//!     prev[o+1] + 1,            // deletion      (parent column j)
//!     prev[o]   + cost,         // match / sub   (parent column j-1)
//!     cur[o-1]  + 1,            // insertion     (current column j-1)
//!     pp[o]     + 1,            // transposition (grandparent column j-2)
//! )
//! ```
//!
//! Two quantities drive the search (see [`band_dist`] and [`row_min`]):
//! - the full-query distance — the band cell for column `n` — a spelling
//!   correction when the node is a word and this is `<= K`.
//! - `row_min` — the smallest band cell — the distance to the closest prefix and
//!   a lower bound on every word below this node; once it exceeds `K` the whole
//!   subtree is pruned.
//!
//! The recurrence is the *optimal string alignment* form of Damerau-Levenshtein
//! (adjacent transpositions are not re-edited). It is exact for distances up to
//! one — the only range the suggester uses — and only diverges from unrestricted
//! Damerau-Levenshtein at distances of two or more.

/// The largest edit distance the suggester corrects at the default band width.
/// Raising the effective distance means widening the band ([`BAND`]) the search
/// is instantiated with — it is no longer threaded through the walk as a value.
pub const MAX_DIST: u8 = 1;

/// Default band width = `2 * MAX_DIST + 1`. The only width the production search
/// is currently instantiated with.
pub const BAND: usize = 2 * MAX_DIST as usize + 1;

/// Sentinel for a band cell whose query column is out of range (`< 0` or `> n`)
/// or whose recurrence had no in-band predecessor. It is larger than any distance
/// the search keeps, so it never wins a `min` and never reads as a correction.
pub const OOB: u8 = u8::MAX;

/// `K` (the max edit distance) for a band of width `W` — i.e. `(W - 1) / 2`.
#[inline]
const fn k_of(w: usize) -> usize {
    (w - 1) / 2
}

/// The depth-0 band (the root's notional parent, the empty word). Column
/// `j = o - K` holds `j` (matching `j` query characters against the empty word
/// costs `j` deletions) when `0 <= j <= query_len`, else [`OOB`].
pub fn base_band<const W: usize>(query_len: usize) -> [u8; W] {
    let k = k_of(W);
    let mut band = [OOB; W];
    for (o, cell) in band.iter_mut().enumerate() {
        // column j = 0 + o - k
        if o >= k {
            let j = o - k;
            if j <= query_len {
                *cell = j.min(u8::MAX as usize) as u8;
            }
        }
    }
    band
}

/// Fill `rows[depth]` (the band for a node at trie `depth`) in place, from its
/// parent band `rows[depth-1]` and grandparent band `rows[depth-2]`. The buffer
/// is reused across siblings and grown lazily, so a whole trie walk allocates
/// `O(max depth)` fixed-width bands rather than one per visited node.
pub fn fill_band<const W: usize>(
    rows: &mut Vec<[u8; W]>,
    depth: usize,
    query: &[char],
    prev_char: Option<char>,
    ch: char,
) {
    debug_assert!(depth >= 1);
    if rows.len() <= depth {
        rows.push([OOB; W]);
    }
    let (left, right) = rows.split_at_mut(depth);
    let prev = &left[depth - 1];
    // At depth 1 there is no grandparent; `prev_char` is `None` there so the
    // transposition branch (the only reader of `pp`) never fires.
    let pp: &[u8; W] = if depth >= 2 { &left[depth - 2] } else { prev };
    compute_band_into(&mut right[0], prev, pp, depth, prev_char, ch, query);
}

/// The banded DP recurrence, written into `cur` from the parent band `prev`,
/// grandparent band `pp`, the node `depth`, the parent's character `prev_char`
/// (`None` at the root level), and this node's character `ch`. See the module
/// doc-comment for the local-index mapping.
fn compute_band_into<const W: usize>(
    cur: &mut [u8; W],
    prev: &[u8; W],
    pp: &[u8; W],
    depth: usize,
    prev_char: Option<char>,
    ch: char,
    query: &[char],
) {
    let k = k_of(W) as isize;
    let n = query.len() as isize;
    for o in 0..W {
        // column j = depth + o - k
        let j = depth as isize + o as isize - k;
        if j < 0 || j > n {
            cur[o] = OOB;
            continue;
        }
        let j = j as usize;
        let mut best = OOB;

        // deletion: parent column j (= prev[o+1]) — word has a char query lacks.
        if o + 1 < W && prev[o + 1] != OOB {
            best = best.min(prev[o + 1].saturating_add(1));
        }
        // match / substitution: parent column j-1 (= prev[o]).
        if j >= 1 && prev[o] != OOB {
            let cost = u8::from(query[j - 1] != ch);
            best = best.min(prev[o].saturating_add(cost));
        }
        // insertion: current column j-1 (= cur[o-1]) — query has an extra char.
        if o >= 1 && j >= 1 && cur[o - 1] != OOB {
            best = best.min(cur[o - 1].saturating_add(1));
        }
        // transposition: query[j-2]query[j-1] == ch·prev_char reversed (= pp[o]).
        if depth >= 2
            && j >= 2
            && prev_char == Some(query[j - 1])
            && ch == query[j - 2]
            && pp[o] != OOB
        {
            best = best.min(pp[o].saturating_add(1));
        }

        cur[o] = best;
    }
}

/// The full-query distance recorded at a node of trie `depth`: the band cell for
/// column `n` (= `query_len`). [`OOB`] when column `n` is outside the band, i.e.
/// the distance exceeds `K` and the node is not a correction.
pub fn band_dist<const W: usize>(band: &[u8; W], depth: usize, query_len: usize) -> u8 {
    let k = k_of(W) as isize;
    let o = query_len as isize - depth as isize + k;
    if o >= 0 && (o as usize) < W {
        band[o as usize]
    } else {
        OOB
    }
}

/// The smallest value in a band: the query's edit distance to the closest prefix
/// of this node's word, and a lower bound on the distance to any word below it.
/// [`OOB`] cells are larger than any kept distance, so they are ignored unless
/// the whole band is out of range (in which case the subtree is pruned).
pub fn row_min(row: &[u8]) -> u8 {
    row.iter().copied().min().unwrap_or(OOB)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference: edit distance of `a` to `b` computed by walking the bands the
    /// same way the trie search does (feeding `b`'s characters one at a time), at
    /// band width `W`. Distances above `K = (W-1)/2` come back as [`OOB`].
    fn dl<const W: usize>(a: &str, b: &str) -> u8 {
        let q: Vec<char> = a.chars().collect();
        let w: Vec<char> = b.chars().collect();
        let mut rows: Vec<[u8; W]> = vec![base_band::<W>(q.len())];
        let mut prev_char = None;
        for (depth, &c) in w.iter().enumerate() {
            fill_band(&mut rows, depth + 1, &q, prev_char, c);
            prev_char = Some(c);
        }
        band_dist(&rows[w.len()], w.len(), q.len())
    }

    /// Wide enough that the band covers the whole conceptual row for the test
    /// strings, so `dl::<WIDE>` reproduces an unbanded DP and can report exact
    /// distances of 2 and 3.
    const WIDE: usize = 15;

    #[test]
    fn exact_values_table() {
        // Optimal-string-alignment distances (agree with classic DL for these).
        let cases = [
            ("", "", 0),
            ("a", "", 1),
            ("", "a", 1),
            ("abc", "abc", 0),
            ("kitten", "sitting", 3),
            ("flaw", "lawn", 2),
            ("ab", "ba", 1),       // transposition
            ("abcd", "abdc", 1),   // transposition at the end
            ("teh", "the", 1),     // transposition needing the grandparent band
            ("xabcd", "xbacd", 1), // transposition in the middle
        ];
        for (a, b, want) in cases {
            assert_eq!(dl::<WIDE>(a, b), want, "dl({a:?}, {b:?})");
        }
    }

    #[test]
    fn transposition_is_one_not_two() {
        // Plain Levenshtein would score these 2; Damerau scores 1.
        assert_eq!(dl::<WIDE>("ab", "ba"), 1);
        assert_eq!(dl::<WIDE>("teh", "the"), 1);
        assert_eq!(dl::<WIDE>("converse", "covnerse"), 1);
    }

    #[test]
    fn base_band_is_clamped_deletion_ladder() {
        // K=1 band (W=3): local o -> column o-1. Column -1 is OOB, then 0, 1.
        assert_eq!(base_band::<3>(4), [OOB, 0, 1]);
        // A query shorter than the band clamps the high columns to OOB.
        assert_eq!(base_band::<3>(0), [OOB, 0, OOB]);
        // A wider band exposes more of the ladder: o-3 for o in 0..7.
        assert_eq!(base_band::<7>(4), [OOB, OOB, OOB, 0, 1, 2, 3]);
    }

    #[test]
    fn row_min_is_prefix_distance() {
        // query "abx" against the growing word "ab": closest prefix is "ab".
        let q: Vec<char> = "abx".chars().collect();
        let mut rows: Vec<[u8; 3]> = vec![base_band::<3>(q.len())];
        fill_band(&mut rows, 1, &q, None, 'a'); // word "a"
        fill_band(&mut rows, 2, &q, Some('a'), 'b'); // word "ab"
        assert_eq!(band_dist(&rows[2], 2, q.len()), 1); // DL("abx","ab") == 1
        assert_eq!(row_min(&rows[2]), 0); // "ab" is an exact prefix of "abx"
    }

    #[test]
    fn narrow_band_reports_out_of_band_as_oob() {
        // When the length difference exceeds K, the query column falls outside the
        // band, so the full-query distance reads back as OOB (the search treats it
        // as "> K" and prunes). A wide band reports the true value instead.
        assert_eq!(dl::<3>("ab", "abcde"), OOB); // 3 insertions, length diff 3
        assert_eq!(dl::<3>("abcde", "ab"), OOB); // 3 deletions, length diff 3
        assert_eq!(dl::<WIDE>("ab", "abcde"), 3);
        // A near-diagonal distance of 2/3 (length diff <= K) stays IN the band, so
        // the band may still report it exactly — OOB is about band geometry, not
        // distance magnitude.
        assert_eq!(dl::<3>("flaw", "lawn"), 2); // length diff 0 -> in band
        // ...and distance-1 answers are identical at any width.
        assert_eq!(dl::<3>("teh", "the"), 1);
        assert_eq!(dl::<3>("abcd", "abce"), 1);
    }

    #[test]
    fn agrees_with_strsim_up_to_dist1() {
        // Our recurrence is optimal-string-alignment; strsim is unrestricted
        // Damerau-Levenshtein. They can only differ at distance >= 2, so once
        // both are clamped at 2 they must agree everywhere. A WIDE band makes
        // `dl` report exact distances so the clamp is meaningful.
        let strings = small_strings();
        for a in &strings {
            for b in &strings {
                let ours = dl::<WIDE>(a, b).min(2);
                let oracle = (strsim::damerau_levenshtein(a, b) as u8).min(2);
                assert_eq!(ours, oracle, "dl({a:?}, {b:?})");
            }
        }
    }

    #[test]
    fn narrow_band_matches_wide_in_the_kept_range() {
        // The property the production search relies on: for every pair, the K=1
        // band agrees with the wide band whenever the true distance is <= 1, and
        // reports "> 1" (OOB) otherwise. Exhaustive over short strings.
        let strings = small_strings();
        for a in &strings {
            for b in &strings {
                let narrow = dl::<3>(a, b);
                let wide = dl::<WIDE>(a, b);
                if wide <= 1 {
                    assert_eq!(narrow, wide, "kept-range mismatch dl({a:?}, {b:?})");
                } else {
                    assert!(narrow > 1, "narrow should be > 1 for dl({a:?}, {b:?})");
                }
            }
        }
    }

    /// Every string up to length 4 over a 3-letter alphabet.
    fn small_strings() -> Vec<String> {
        let alphabet = ['a', 'b', 'c'];
        let mut out = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..4 {
            let mut next = Vec::new();
            for s in &frontier {
                for c in alphabet {
                    let mut t = s.clone();
                    t.push(c);
                    next.push(t);
                }
            }
            out.extend(next.iter().cloned());
            frontier = next;
        }
        out
    }

    #[test]
    fn unicode_counts_as_one_edit() {
        // A multi-byte character is a single edit, not one-per-byte.
        assert_eq!(dl::<WIDE>("för", "for"), 1); // substitute ö -> o
        assert_eq!(dl::<WIDE>("fr", "för"), 1); // delete ö
        assert_eq!(dl::<WIDE>("allmän", "allman"), 1); // substitute ä -> a
    }
}
