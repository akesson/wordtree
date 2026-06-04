//! Incremental edit-distance carried down the trie: one dynamic-programming row
//! per node, where `row[j]` is the edit distance between the first `j` characters
//! of the query and the word spelled out by the path from the root to this node.
//!
//! Walking the trie, a node at depth `d` (its path spells a `d`-character word)
//! derives its row from its parent's row (`prev`, depth `d-1`) and grandparent's
//! row (`prev_prev`, depth `d-2`). The grandparent row is what lets us award a
//! Damerau transposition (swap of two adjacent characters) a cost of one.
//!
//! Two quantities drive the search:
//! - `row[n]` (n = query length) is the full edit distance of the query to this
//!   node's word — a complete-word suggestion when the node is a word and this is
//!   `<= MAX_DIST`.
//! - `row_min(row)` is the *prefix* edit distance: the best edit distance of the
//!   query to any prefix of this node's word, i.e. a lower bound on the distance
//!   to every word below this node. When it exceeds `MAX_DIST` the whole subtree
//!   can be pruned; when it is `<= MAX_DIST` the node is also a viable
//!   autocomplete seed.
//!
//! The recurrence is the *optimal string alignment* form of Damerau-Levenshtein
//! (adjacent transpositions are not re-edited). It is exact for distances up to
//! one — the only range the suggester uses — and only diverges from unrestricted
//! Damerau-Levenshtein at distances of two or more.

/// The largest edit distance the suggester corrects. Raising this is the only
/// change needed to offer distance-2 suggestions.
pub const MAX_DIST: u8 = 1;

/// The row for the empty word (the root's notional parent): matching the first
/// `j` query characters against an empty word costs `j` deletions. Distances are
/// saturated into `u8`, which only matters for absurdly long queries and never
/// affects the `<= MAX_DIST` decisions the search makes.
pub fn base_row(query_len: usize) -> Vec<u8> {
    (0..=query_len).map(|j| j.min(u8::MAX as usize) as u8).collect()
}

/// The DP recurrence, written into `cur` (length `query.len() + 1`) from the
/// parent row `prev`, grandparent row `prev_prev`, the parent's character
/// `prev_char` (`None` at the root level), and this node's character `ch`.
fn compute_into(
    cur: &mut [u8],
    prev: &[u8],
    prev_prev: &[u8],
    prev_char: Option<char>,
    ch: char,
    query: &[char],
) {
    // Matching the whole query against this one-longer word prefix starts from
    // "delete everything" and gets one cheaper per real match below.
    cur[0] = prev[0].saturating_add(1);
    for j in 1..=query.len() {
        let cost = u8::from(query[j - 1] != ch);
        let mut v = prev[j]
            .saturating_add(1) // deletion: word has a char the query lacks
            .min(cur[j - 1].saturating_add(1)) // insertion: query has an extra char
            .min(prev[j - 1].saturating_add(cost)); // match or substitution
        // Damerau transposition: query[j-2]query[j-1] == ch(prev_char) reversed.
        if j >= 2 && prev_char == Some(query[j - 1]) && ch == query[j - 2] {
            v = v.min(prev_prev[j - 2].saturating_add(1));
        }
        cur[j] = v;
    }
}

/// Fill `rows[depth]` (the row for a node at trie depth `depth`) in place, from
/// its parent row `rows[depth-1]` and grandparent row `rows[depth-2]`. The buffer
/// is reused across siblings and grown lazily, so a whole trie walk allocates
/// only `O(max depth)` rows rather than one per visited node.
pub fn fill_row(
    rows: &mut Vec<Vec<u8>>,
    depth: usize,
    query: &[char],
    prev_char: Option<char>,
    ch: char,
) {
    debug_assert!(depth >= 1);
    if rows.len() <= depth {
        rows.push(vec![0u8; query.len() + 1]);
    }
    let (left, right) = rows.split_at_mut(depth);
    let prev = &left[depth - 1];
    // At depth 1 there is no grandparent; `prev_char` is `None` there so the
    // transposition branch (the only reader of `prev_prev`) never fires.
    let prev_prev: &[u8] = if depth >= 2 { &left[depth - 2] } else { prev };
    compute_into(&mut right[0], prev, prev_prev, prev_char, ch, query);
}

/// The smallest value in a row: the query's edit distance to the closest prefix
/// of this node's word, and a lower bound on the distance to any word below it.
pub fn row_min(row: &[u8]) -> u8 {
    row.iter().copied().min().unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference: edit distance of `a` to `b` computed by walking the rows the
    /// same way the trie search does (feeding `b`'s characters one at a time).
    fn dl(a: &str, b: &str) -> u8 {
        let q: Vec<char> = a.chars().collect();
        let w: Vec<char> = b.chars().collect();
        let mut rows: Vec<Vec<u8>> = vec![base_row(q.len())];
        let mut prev_char = None;
        for (depth, &c) in w.iter().enumerate() {
            fill_row(&mut rows, depth + 1, &q, prev_char, c);
            prev_char = Some(c);
        }
        rows[w.len()][q.len()]
    }

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
            ("ab", "ba", 1),     // transposition
            ("abcd", "abdc", 1), // transposition at the end
            ("teh", "the", 1),   // transposition needing the grandparent row
            ("xabcd", "xbacd", 1), // transposition in the middle
        ];
        for (a, b, want) in cases {
            assert_eq!(dl(a, b), want, "dl({a:?}, {b:?})");
        }
    }

    #[test]
    fn transposition_is_one_not_two() {
        // Plain Levenshtein would score these 2; Damerau scores 1.
        assert_eq!(dl("ab", "ba"), 1);
        assert_eq!(dl("teh", "the"), 1);
        assert_eq!(dl("converse", "covnerse"), 1);
    }

    #[test]
    fn base_row_is_deletion_ladder() {
        assert_eq!(base_row(0), vec![0]);
        assert_eq!(base_row(4), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn row_min_is_prefix_distance() {
        // query "abx" against the growing word "ab": closest prefix is "ab".
        let q: Vec<char> = "abx".chars().collect();
        let mut rows: Vec<Vec<u8>> = vec![base_row(q.len())];
        fill_row(&mut rows, 1, &q, None, 'a'); // word "a"
        fill_row(&mut rows, 2, &q, Some('a'), 'b'); // word "ab"
        assert_eq!(rows[2][q.len()], 1); // DL("abx","ab") == 1
        assert_eq!(row_min(&rows[2]), 0); // "ab" is an exact prefix of "abx"
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
    fn agrees_with_strsim_up_to_dist1() {
        // Our recurrence is optimal-string-alignment; strsim is unrestricted
        // Damerau-Levenshtein. They can only differ at distance >= 2, so once
        // both are clamped at 2 they must agree everywhere. This exhaustively
        // pins that the engine's distance matches the canonical oracle in the
        // 0/1 range it actually relies on.
        let strings = small_strings();
        for a in &strings {
            for b in &strings {
                let ours = dl(a, b).min(2);
                let oracle = (strsim::damerau_levenshtein(a, b) as u8).min(2);
                assert_eq!(ours, oracle, "dl({a:?}, {b:?})");
            }
        }
    }

    #[test]
    fn unicode_counts_as_one_edit() {
        // A multi-byte character is a single edit, not one-per-byte.
        assert_eq!(dl("för", "for"), 1); // substitute ö -> o
        assert_eq!(dl("fr", "för"), 1); // delete ö
        assert_eq!(dl("allmän", "allman"), 1); // substitute ä -> a
    }
}
