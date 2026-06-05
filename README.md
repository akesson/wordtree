# wordtree

> **Status: unmaintained showcase.** This repository is an open-source snapshot of the
> `wordtree` crate, extracted from a larger private project to accompany an article.
> It is not actively maintained — issues and pull requests may go unanswered.

A compact, cache-friendly trie for storing large word lists with three jobs in mind:

- **Browsable index** — words are grouped into folders (≈100 per folder) for navigation.
- **Exact lookup** — resolve a word to its expression index in `O(word length)`.
- **Typo-tolerant autocomplete** — frequency-ranked as-you-type suggestions that combine
  *autocomplete* (extend a prefix) with *spelling correction* (fix a single edit —
  substitution, transposition, insertion or deletion — at Damerau-Levenshtein distance
  ≤ 1). Either half can be requested on its own.

The tree is stored as a width-first array of 8-byte (64-bit) nodes, with the sparse
per-word data (frequency and expression index) held in compact side tables, and can be
serialised with [`rkyv`](https://rkyv.org) for zero-copy, memory-mapped access. See the
[deep dive](#word-tree) below for the data structure and edit-distance design;
[`comparisons/REPORT.md`](comparisons/REPORT.md) benchmarks it against specialist crates.

## Usage

```rust
use wordtree::{Builder, TreeFn};

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
```

`to_tree()` returns a `Tree` that implements `TreeFn`. After `rkyv`-serialising it, the
same query methods are available zero-copy on `ArchivedTree`.

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
  - [Languages](#languages)
  - [Node layout](#node-layout)
  - [Edit distance](#edit-distance)
  - [Benchmarks](#benchmarks)

# Word Tree

This is a custom trie for storing words as a tree of chars.
It is heavily size optimized and uses an array of Nodes.

A node is followed by all its siblings, so the tree is stored in a width-first manner.

The tree has some nodes marked as folders in a manner that,
if possible, a maximum of X words is stored per folder.
X is normally 100.

## Uses

The three jobs above map onto the structure: folder-marked nodes drive the browsable
index, the char-path to a node yields its expression index, and a frequency-ranked walk
produces typo-tolerant autocomplete.

Expressions containing spaces are normally excluded — they are never suggested, and are
kept only to translate other non-spaced expressions.

## Languages

Alphabetic languages have in general up to 40 different characters plus some special ones (-, +, etc).
It can be thought of as up to 100 different chars.

As-you-type autocomplete that tolerates mis-spellings is of value.

Logographic languages (chinese, etc) have thousands of frequently used logograms and in total tens of thousands.
These languages are often used with special entry forms. They are currently not the focus of the word-tree.

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

A word's own `percentile` (frequency, 0–1000) and 24-bit `expr_index` only exist on the
~28% of nodes that terminate a word, so storing them inline would waste 5 bytes on every
internal node. Instead they live in three side arrays, also part of the zero-copy mmap:

| table        | size                          | role                                                      |
| ------------ | ----------------------------- | --------------------------------------------------------- |
| `word_bits`  | 1 bit / node                  | marks which nodes are words                                |
| `rank_index` | 1× u32 / 64-bit word          | cumulative word count — answers `rank(node) → slot` in O(1)|
| `values`     | 5 bytes / **word** (u16 + u24)| the `(percentile, expr_index)` pair for each word         |

A word node at position `i` finds its value at `values[rank(i)]` — the number of word
nodes before it. The bit probe is on the hot browse/descent path; the rank query only
fires when a per-word value is actually consumed (an exact `index_of`, or a kept
suggestion). On English this trims the structure from ~26.5 MiB (12-byte nodes) to
~21.1 MiB (8-byte nodes + side tables), a ~20% reduction with no loss of function — see
[`comparisons/REPORT.md`](comparisons/REPORT.md) §2.

## Edit distance

An edit distance of 1 — counting transposition (swapping two adjacent chars) alongside substitution, insertion and deletion — is sufficient for as-you-type correction.

It is evaluated incrementally as the trie is walked: one small dynamic-programming Damerau-Levenshtein row per node, with a subtree pruned as soon as its whole row is out of range. This handles all four edit kinds uniformly and visits only a tiny fraction of the tree, without a full edit-distance matrix. See [src/editdist/README.md](src/editdist/README.md)

## Benchmarks

Building the full tree from the bundled TSV takes roughly 405 ms (English, ~638k words)
and 68 ms (Swedish, ~113k words) on an Apple M4 Pro. For comparative latency, size and
suggestion-quality benchmarks against specialist crates (fst, symspell,
pruning_radix_trie, boomphf), see [`comparisons/REPORT.md`](comparisons/REPORT.md).
