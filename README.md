# wordtree

> The `wordtree` crate was extracted from a larger private project to accompany an
> article.

Everything a word-list UI needs — typo-tolerant as-you-type suggestions, exact
lookup, and a browsable A–Z index — served from one small file that loads by
memory-mapping, with no parsing or index-building at startup.

Type `appl` into a search box backed by wordtree and you get `apple`, `apply`
and `application` back; mistype it as `aple` and `apple` still tops the list.
Completions and one-edit spelling fixes come merged in a single frequency-ranked
list, in tens of microseconds per keystroke against a 638,000-word dictionary.
That is the crate's job — shipping a large word list inside an app (a dictionary
or translation app, a keyboard, a command palette) and answering three questions
about it:

- **What is the user trying to type?** `suggestions()` completes the prefix *and*
  forgives one mistake — a wrong, missing, extra or swapped character — ranking
  results by word frequency so common words surface first. `completions()` and
  `corrections()` give either half alone, and the `Caps` budget turns the short
  as-you-type list into an exhaustive spell-check when you need every candidate.
- **What does this word point to?** Every word carries a numeric index you assign
  (`index_of("apple") → 1`), so a match lands directly in your own table of
  definitions, translations or products — no second lookup structure.
- **What's in the list?** Words are grouped into folders of ~100 (`path_of`), so a
  UI can offer a drill-down index for browsing, not just a search box.

It also ships well: the whole structure — 638k English words in 21 MiB, 113k
Swedish in 4.4 MiB — serialises with [`rkyv`](https://rkyv.org), and its on-disk
form *is* its in-memory form. Loading is an mmap; there is nothing to parse or
rebuild, which matters on phones and other memory-tight, cold-start-sensitive
targets.

The [deep dive](#word-tree) below explains the data structure and edit-distance
design; [`comparisons/REPORT.md`](comparisons/REPORT.md) races it honestly
against the specialist crates (fst, symspell, pruning_radix_trie) — each beats
wordtree on its own axis; wordtree's case is all three jobs from one file.

## Usage

```rust
use wordtree::{Builder, Caps, TreeFn};

// Build a tree from (word, percentile, expr_index) entries.
let mut builder = Builder::new();
builder.add_word("apple", 99, 1);
builder.add_word("apply", 80, 2);
builder.add_word("apricot", 50, 3);
builder.organize_into_folders(100); // ~100 words per browsable folder
let tree = builder.to_tree();

// Exact word -> expression-index lookup.
assert_eq!(tree.index_of("apple"), Some(1));

// Browsable folder path for a word.
let _path = tree.path_of("apricot");

// Typo-tolerant autocomplete for a (mis)typed query; the closure filters
// which expression indices are acceptable candidates. "aple" is a deletion typo
// of "apple"; length-changing edits (indels) within edit distance 1 are
// corrected, so "apple" (expr_index 1) is among the suggestions.
let suggestions = tree.suggestions("aple", |_expr_index| true);
assert!(suggestions.iter().any(|s| s.expr_index == 1));

// Need only one half? `completions()` is autocomplete only (no edit-distance walk,
// much cheaper); `corrections()` is spelling correction only (no prefix completion).
let _completions = tree.completions("ap", |_| true);   // words extending "ap"
let _corrections = tree.corrections("aple", |_| true); // spelling corrections only

// The default list is deliberately short (good as-you-type behaviour). For
// exhaustive spell-check — the *complete* set of words within edit distance 1,
// including when the query is itself a valid word — widen the per-query budget
// with `Caps`. `Caps::uniform(64)` recalls 100% of the brute-force DL≤1 set.
let _all = tree.corrections_with("aple", |_| true, Caps::uniform(64));
```

`to_tree()` returns a `Tree` that implements `TreeFn`. After `rkyv`-serialising it, the
same query methods are available zero-copy on `ArchivedTree`. The per-query suggestion
budget is configurable via `Caps` — see [`comparisons/REPORT.md`](comparisons/REPORT.md)
§1 for the recall trade-off behind the default.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. The vendored [`ego-tree`](ego-tree/) crate is licensed separately under
ISC (see [ego-tree/LICENSE](ego-tree/LICENSE)).

## Data

The bundled benchmark and test word lists are derived from PanLex and Wiktionary — see
[DATA-LICENSE](DATA-LICENSE) for attribution and terms.

---

## Contents

- [Word Tree](#word-tree)
  - [Uses](#uses)
  - [Node layout](#node-layout)
  - [Edit distance](#edit-distance)
  - [Benchmarks](#benchmarks)

# Word Tree

A custom trie that stores words as a tree of chars, size-optimised into a flat
array of fixed-width nodes. Each node is followed by all its siblings, so the
tree is laid out width-first. Some nodes are marked as folders such that, where
possible, at most ~100 words land in each folder.

## Uses

The three jobs above map onto the structure: folder-marked nodes drive the browsable
index, the char-path to a node yields its expression index, and a frequency-ranked walk
produces typo-tolerant autocomplete.

The design targets alphabetic scripts (tens of distinct characters per language);
logographic scripts (thousands of distinct characters) are out of scope.

## Node layout

Each node is a fixed 8-byte (64-bit) record, so a tree read from disk needs no
type-casting or transformation (zero-copy mmap):

| field                | bits   | comment                                                              |
| -------------------- | ------ | -------------------------------------------------------------------- |
| first_child_pos      | 24     | relative position of the first child                                 |
| node_char            | 24     | UTF-32 codepoint (low 3 bytes used)                                  |
| is_folder            | 1      |                                                                      |
| is_last_sibling      | 1      |                                                                      |
| max_child_percentile | 10     | max percentile in the subtree — drives the pruning walk (below)      |
| (spare)              | 4      |                                                                      |
| **TOT**              | **64** | = 8 bytes                                                            |

`max_child_percentile` powers the [pruning-radix-trie](https://towardsdatascience.com/the-pruning-radix-trie-a-radix-trie-on-steroids-412807f77abc)
walk: a subtree whose best percentile can't beat the current top-k is skipped. It stays
inline because it is read on *every* node during the walk.

### Per-word side tables

A word's own `percentile` (frequency, 0–1000) and 24-bit `expr_index` only exist on
nodes that terminate a word — about a quarter of all nodes — so storing them inline
would waste 5 bytes on every internal node. Instead they live in three side arrays,
also part of the zero-copy mmap:

| table        | size                          | role                                                      |
| ------------ | ----------------------------- | --------------------------------------------------------- |
| `word_bits`  | 1 bit / node                  | marks which nodes are words                                |
| `rank_index` | 1× u32 / 64-bit word          | cumulative word count — answers `rank(node) → slot` in O(1)|
| `values`     | 5 bytes / **word** (u16 + u24)| the `(percentile, expr_index)` pair for each word         |

A word node at position `i` finds its value at `values[rank(i)]` — the number of word
nodes before it. The bit probe is on the hot browse/descent path; the rank query only
fires when a per-word value is actually consumed (an exact `index_of`, or a kept
suggestion). Storing the pair inline would need 12-byte nodes (~26.5 MiB for English);
the 8-byte nodes plus side tables come to ~21.1 MiB, ~20% smaller with no loss of
function — see [`comparisons/REPORT.md`](comparisons/REPORT.md) §2.

## Edit distance

An edit distance of 1 — counting transposition (swapping two adjacent chars) alongside substitution, insertion and deletion — is sufficient for as-you-type correction.

It is evaluated incrementally as the trie is walked: one small dynamic-programming
Damerau-Levenshtein row per node — stored as a fixed `2K+1`-cell diagonal *band*
(3 cells at edit distance `K=1`) rather than the full row — with a subtree pruned as
soon as its band minimum is out of range. This handles all four edit kinds uniformly
and visits well under 1% of the tree, without a full edit-distance matrix. The
band keeps each visited node at `O(K)` work instead of `O(query length)`; results are
identical to a full-row walk. See [src/editdist/README.md](src/editdist/README.md)

## Benchmarks

Building the full tree from the bundled TSV takes roughly 435 ms (English, ~638k words)
and 71 ms (Swedish, ~113k words) on an Apple M4 Pro. For comparative latency, size and
suggestion-quality benchmarks against specialist crates (fst, symspell,
pruning_radix_trie, boomphf), see [`comparisons/REPORT.md`](comparisons/REPORT.md).
