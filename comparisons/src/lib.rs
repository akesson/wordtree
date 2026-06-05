//! Shared scaffolding for the wordtree comparative study.
//!
//! One canonical, de-duplicated word list per language drives every engine, so
//! all comparisons are apples-to-apples. See `REPORT.md` for methodology and the
//! fairness caveats baked into each engine wrapper below.

use std::collections::{HashMap, HashSet};

pub mod alloc_counter;

/// Languages bundled in `../benches/data/`.
pub const LANGS: [&str; 2] = ["en", "sv"];

/// One dictionary row: `word \t percentile \t expr_index` (TSV column order).
///
/// `percentile` is wordtree's frequency signal (0..=1000, higher = more common);
/// `expr_index` is the external id `index_of` resolves to.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Row {
    pub word: String,
    pub percentile: u16,
    pub expr_index: u32,
}

/// Absolute path to a bundled, zstd-compressed TSV word list (reused, not copied).
pub fn data_path(lang: &str) -> String {
    format!(
        "{}/../benches/data/{}.tsv.zst",
        env!("CARGO_MANIFEST_DIR"),
        lang
    )
}

/// A loaded, de-duplicated word list plus the lookups every engine/metric needs.
///
/// De-duplication keeps the *first* occurrence of each word, and the identical
/// `rows` slice is fed to every engine — so a correctness gate can assert they
/// all resolve a word to the same `expr_index`.
pub struct Dataset {
    pub lang: String,
    pub rows: Vec<Row>,
    by_word: HashMap<String, usize>,
    by_expr: HashMap<u32, usize>,
}

impl Dataset {
    pub fn load(lang: &str) -> Dataset {
        let raw = read_rows(&data_path(lang));
        let mut rows = Vec::with_capacity(raw.len());
        let mut seen = HashSet::new();
        for r in raw {
            if seen.insert(r.word.clone()) {
                rows.push(r);
            }
        }
        let by_word = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.word.clone(), i))
            .collect();
        let by_expr = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.expr_index, i))
            .collect();
        Dataset {
            lang: lang.to_string(),
            rows,
            by_word,
            by_expr,
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn expr_of(&self, word: &str) -> Option<u32> {
        self.by_word.get(word).map(|&i| self.rows[i].expr_index)
    }
    pub fn word_of_expr(&self, expr: u32) -> Option<&str> {
        self.by_expr.get(&expr).map(|&i| self.rows[i].word.as_str())
    }
    pub fn percentile_of(&self, word: &str) -> Option<u16> {
        self.by_word.get(word).map(|&i| self.rows[i].percentile)
    }
}

/// Read + decompress a `.tsv.zst` word list. Mirrors `benches/data.rs::read_csv`
/// but deserialises into the local [`Row`] (wordtree's `TsvEntry` has private
/// fields, so we keep the comparison crate decoupled from its internals).
pub fn read_rows(path: &str) -> Vec<Row> {
    let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
    let dec = zstd::Decoder::new(f).expect("zstd decoder");
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        // Some bundled words contain a bare `"`; treat it as an ordinary character
        // (matches benches/data.rs / Tree::read_tsv after fix 85713e6). Without
        // this, csv swallows whole spans between quotes and ~22% en / ~44% sv
        // words silently vanish.
        .quoting(false)
        .from_reader(std::io::BufReader::new(dec));
    rdr.deserialize::<Row>()
        .collect::<Result<Vec<Row>, _>>()
        .expect("parse TSV rows")
}

// ---------------------------------------------------------------------------
// Engine builders. Each takes the same `&[Row]` and returns a ready-to-query
// structure. Build cost lives here (called once, outside criterion's `iter`).
// ---------------------------------------------------------------------------

/// wordtree — built exactly like its own benches (`Tree::from_tsv`): add each
/// word, then `organize_into_folders(100)`.
pub fn build_wordtree(rows: &[Row]) -> wordtree::Tree {
    let mut b = wordtree::Builder::new();
    for r in rows {
        b.add_word(&r.word, r.percentile, r.expr_index);
    }
    b.organize_into_folders(100);
    b.to_tree()
}

/// `fst::Map` (BurntSushi) — ordered byte-keyed map, value = `expr_index`.
/// Keys must be strictly increasing by bytes, so we sort + dedup first.
pub fn build_fst_map(rows: &[Row]) -> fst::Map<Vec<u8>> {
    let mut pairs: Vec<(&str, u64)> = rows
        .iter()
        .map(|r| (r.word.as_str(), r.expr_index as u64))
        .collect();
    pairs.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    pairs.dedup_by(|a, b| a.0 == b.0);
    let mut builder = fst::MapBuilder::memory();
    for (k, v) in pairs {
        builder.insert(k, v).expect("fst insert (sorted)");
    }
    fst::Map::new(builder.into_inner().expect("fst finish")).expect("fst map")
}

/// Naive baseline: `HashMap<word, expr_index>`.
pub fn build_hashmap(rows: &[Row]) -> HashMap<String, u32> {
    rows.iter()
        .map(|r| (r.word.clone(), r.expr_index))
        .collect()
}

/// Naive baseline: sorted `Vec<(word, expr_index)>` for binary search.
pub fn build_sorted_vec(rows: &[Row]) -> Vec<(String, u32)> {
    let mut v: Vec<(String, u32)> = rows
        .iter()
        .map(|r| (r.word.clone(), r.expr_index))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

pub fn sorted_vec_get(v: &[(String, u32)], word: &str) -> Option<u32> {
    v.binary_search_by(|probe| probe.0.as_str().cmp(word))
        .ok()
        .map(|i| v[i].1)
}

/// boomphf via `BoomHashMap` — the key-storing variant. The MPHF maps each word
/// to a dense slot; the map keeps the keys and values in parallel arrays (~3
/// bits/item Mphf overhead on top). Storing the keys lets it *reject* non-members
/// (via `get_key_id`), so unlike a bare `Mphf` there are no false positives — and
/// its measured size now honestly includes the keys.
pub struct Boomphf {
    pub map: boomphf::hashmap::BoomHashMap<String, u32>,
}

pub fn build_boomphf(rows: &[Row]) -> Boomphf {
    // Parallel key/value arrays (already unique after the quoting fix).
    let mut seen = HashSet::new();
    let mut keys: Vec<String> = Vec::new();
    let mut data: Vec<u32> = Vec::new();
    for r in rows {
        if seen.insert(r.word.clone()) {
            keys.push(r.word.clone());
            data.push(r.expr_index);
        }
    }
    Boomphf {
        map: boomphf::hashmap::BoomHashMap::new(keys, data),
    }
}

impl Boomphf {
    pub fn get(&self, word: &str) -> Option<u32> {
        // `BoomHashMap::get` returns a value for any input (random for
        // non-members), so confirm membership against the stored keys first.
        self.map.get_key_id(word)?;
        self.map.get(word).copied()
    }
}

/// SymSpell — Symmetric-Delete spelling correction, frequency-ranked.
/// Configured to edit distance 1 to match wordtree's Damerau-Levenshtein ≤1.
/// Loaded in-memory line by line ("word\tpercentile"); Unicode strategy keeps
/// Swedish letters and symbol tokens intact.
pub fn build_symspell(rows: &[Row]) -> symspell::SymSpell<symspell::UnicodeStringStrategy> {
    use symspell::{SymSpellBuilder, UnicodeStringStrategy};
    let mut sym: symspell::SymSpell<UnicodeStringStrategy> = SymSpellBuilder::default()
        .max_dictionary_edit_distance(1)
        .count_threshold(0)
        .build()
        .expect("symspell build");
    for r in rows {
        // term_index 0, count_index 1, tab separator
        let line = format!("{}\t{}", r.word, r.percentile);
        sym.load_dictionary_line(&line, 0, 1, "\t");
    }
    sym
}

/// PruningRadixTrie — frequency-ranked *prefix* autocomplete (Wolf Garbe's
/// algorithm, the one wordtree's `max_child_percentile` pruning is modelled on).
/// payload = expr_index, weight = percentile. Compared only on the autocomplete
/// sub-task (it does not do spelling correction).
pub fn build_pruning_trie(rows: &[Row]) -> pruning_radix_trie::PruningRadixTrie<u32, u32> {
    let mut trie = pruning_radix_trie::PruningRadixTrie::new();
    for r in rows {
        trie.add(&r.word, r.expr_index, r.percentile as u32);
    }
    trie
}

// ---------------------------------------------------------------------------
// Fuzzy-suggestion adapters — each returns the matched WORDS (frequency-ranked)
// so quality can be scored uniformly against the brute-force ground truth.
// Latency benches call these same adapters.
// ---------------------------------------------------------------------------

/// wordtree suggestions, decoded back to words via `expr_index`. Includes all
/// suggestion kinds (Matching/Spelling/Extension/AltExt) — its real API cost.
pub fn wordtree_suggest_words(tree: &wordtree::Tree, ds: &Dataset, q: &str) -> Vec<String> {
    use wordtree::TreeFn;
    tree.suggestions(q, |_| true)
        .into_iter()
        .filter_map(|s| ds.word_of_expr(s.expr_index).map(|w| w.to_string()))
        .collect()
}

/// wordtree prefix completions only (`completions()`), decoded to words. The
/// completion-only path runs no fuzzy walk, so this is the fair counterpart to
/// the pruning-radix-trie's top-k prefix lookup.
pub fn wordtree_complete_words(tree: &wordtree::Tree, ds: &Dataset, q: &str) -> Vec<String> {
    use wordtree::TreeFn;
    tree.completions(q, |_| true)
        .into_iter()
        .filter_map(|s| ds.word_of_expr(s.expr_index).map(|w| w.to_string()))
        .collect()
}

/// wordtree fuzzy corrections only (`corrections()`), decoded to words — the
/// spell-check path (Matching + Spelling) with no completion sweep.
pub fn wordtree_correct_words(tree: &wordtree::Tree, ds: &Dataset, q: &str) -> Vec<String> {
    use wordtree::TreeFn;
    tree.corrections(q, |_| true)
        .into_iter()
        .filter_map(|s| ds.word_of_expr(s.expr_index).map(|w| w.to_string()))
        .collect()
}

/// SymSpell lookup at distance 1, all candidates (already frequency-ranked).
pub fn symspell_suggest_words(
    sym: &symspell::SymSpell<symspell::UnicodeStringStrategy>,
    q: &str,
) -> Vec<String> {
    sym.lookup(q, symspell::Verbosity::All, 1)
        .into_iter()
        .map(|s| s.term)
        .collect()
}

/// fst Levenshtein automaton at distance 1. fst returns matches in lexicographic
/// order and stores no frequency, so we re-rank with the side percentile table —
/// that ranking work is part of the measured cost. NOTE: fst's automaton is
/// plain Levenshtein (no transposition), unlike wordtree/symspell (Damerau).
pub fn fst_suggest_words(map: &fst::Map<Vec<u8>>, ds: &Dataset, q: &str) -> Vec<String> {
    use fst::{IntoStreamer, Streamer, automaton::Levenshtein};
    let lev = match Levenshtein::new(q, 1) {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    let mut stream = map.search(&lev).into_stream();
    let mut hits: Vec<(String, u16)> = Vec::new();
    while let Some((k, _v)) = stream.next() {
        if let Ok(w) = std::str::from_utf8(k) {
            let p = ds.percentile_of(w).unwrap_or(0);
            hits.push((w.to_string(), p));
        }
    }
    hits.sort_by_key(|h| std::cmp::Reverse(h.1));
    hits.into_iter().map(|(w, _)| w).collect()
}

/// PruningRadixTrie top-k prefix completions (autocomplete sub-task).
pub fn pruning_prefix_words(
    trie: &pruning_radix_trie::PruningRadixTrie<u32, u32>,
    prefix: &str,
    k: usize,
) -> Vec<String> {
    trie.find(prefix, k)
        .into_iter()
        .map(|r| r.term.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Ground truth (brute force) + query generation.
// ---------------------------------------------------------------------------

/// All dictionary words within Damerau-Levenshtein ≤1 of `q`, frequency-ranked.
/// This is the correctness oracle for the suggestion quality metric and doubles
/// as the "brute force" latency baseline.
pub fn ground_truth_dl1(ds: &Dataset, q: &str) -> Vec<String> {
    let mut hits: Vec<(&str, u16)> = ds
        .rows
        .iter()
        .filter(|r| strsim::damerau_levenshtein(q, &r.word) <= 1)
        .map(|r| (r.word.as_str(), r.percentile))
        .collect();
    hits.sort_by_key(|h| std::cmp::Reverse(h.1));
    hits.into_iter().map(|(w, _)| w.to_string()).collect()
}

/// Top-k dictionary words sharing `prefix`, frequency-ranked (autocomplete oracle).
pub fn ground_truth_prefix(ds: &Dataset, prefix: &str, k: usize) -> Vec<String> {
    let mut hits: Vec<(&str, u16)> = ds
        .rows
        .iter()
        .filter(|r| r.word.starts_with(prefix))
        .map(|r| (r.word.as_str(), r.percentile))
        .collect();
    hits.sort_by_key(|h| std::cmp::Reverse(h.1));
    hits.into_iter().take(k).map(|w| w.0.to_string()).collect()
}

/// The single edit applied to a target word to synthesise a realistic typo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Delete,
    Insert,
    Substitute,
    Transpose,
}

impl EditKind {
    pub fn label(self) -> &'static str {
        match self {
            EditKind::Delete => "delete",
            EditKind::Insert => "insert",
            EditKind::Substitute => "substitute",
            EditKind::Transpose => "transpose",
        }
    }
    pub const ALL: [EditKind; 4] = [
        EditKind::Delete,
        EditKind::Insert,
        EditKind::Substitute,
        EditKind::Transpose,
    ];
}

/// A fuzzy-suggestion test case: a `text` typo derived from `target` by `kind`.
#[derive(Debug, Clone)]
pub struct FuzzyQuery {
    pub text: String,
    pub target: String,
    pub kind: EditKind,
}

/// Apply one deterministic single-character edit at the word's midpoint.
/// Returns `None` when the edit can't be formed (e.g. word too short, or a
/// transpose of two identical neighbours).
pub fn apply_edit(word: &str, kind: EditKind) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    if n < 4 {
        return None;
    }
    let mid = n / 2;
    let mut out = chars.clone();
    match kind {
        EditKind::Delete => {
            out.remove(mid);
        }
        EditKind::Insert => {
            out.insert(mid, 'x');
        }
        EditKind::Substitute => {
            let repl = if chars[mid] == 'x' { 'y' } else { 'x' };
            out[mid] = repl;
        }
        EditKind::Transpose => {
            if chars[mid - 1] == chars[mid] {
                return None;
            }
            out.swap(mid - 1, mid);
        }
    }
    Some(out.into_iter().collect())
}

/// Build a deterministic fuzzy query set: pick the highest-frequency clean
/// (lowercase a–z) words of a workable length, then derive one typo per edit
/// kind. `per_kind` caps how many targets contribute to each kind.
pub fn fuzzy_query_set(ds: &Dataset, per_kind: usize) -> Vec<FuzzyQuery> {
    let mut targets: Vec<&Row> = ds
        .rows
        .iter()
        .filter(|r| {
            let len = r.word.chars().count();
            (5..=12).contains(&len) && r.word.chars().all(|c| c.is_ascii_lowercase())
        })
        .collect();
    targets.sort_by_key(|r| std::cmp::Reverse(r.percentile));

    let mut out = Vec::new();
    for kind in EditKind::ALL {
        let mut made = 0;
        for r in &targets {
            if made >= per_kind {
                break;
            }
            if let Some(text) = apply_edit(&r.word, kind) {
                out.push(FuzzyQuery {
                    text,
                    target: r.word.clone(),
                    kind,
                });
                made += 1;
            }
        }
    }
    out
}

/// Representative prefixes for the autocomplete sub-task: the most common
/// 2- and 3-letter prefixes among clean words.
pub fn prefix_query_set(ds: &Dataset, count: usize) -> Vec<String> {
    let mut freq: HashMap<String, u64> = HashMap::new();
    for r in &ds.rows {
        let chars: Vec<char> = r.word.chars().collect();
        if chars.len() >= 4 && chars.iter().all(|c| c.is_ascii_lowercase()) {
            for plen in [2usize, 3] {
                let p: String = chars[..plen].iter().collect();
                *freq.entry(p).or_default() += r.percentile as u64;
            }
        }
    }
    let mut v: Vec<(String, u64)> = freq.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().take(count).map(|(p, _)| p).collect()
}

/// Recall of a returned word list against a ground-truth set: fraction of the
/// truth words that appear in `returned`.
pub fn recall(returned: &[String], truth: &[String]) -> f64 {
    if truth.is_empty() {
        return 1.0;
    }
    let got: HashSet<&str> = returned.iter().map(|s| s.as_str()).collect();
    let hit = truth.iter().filter(|t| got.contains(t.as_str())).count();
    hit as f64 / truth.len() as f64
}

/// 1-based rank of `target` within `returned`, or `None` if absent.
pub fn rank_of(returned: &[String], target: &str) -> Option<usize> {
    returned.iter().position(|w| w == target).map(|i| i + 1)
}
