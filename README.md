# wordtree

> **Status: unmaintained showcase.** This repository is an open-source snapshot of the
> `wordtree` crate, extracted from a larger private project to accompany an article.
> It is not actively maintained — issues and pull requests may go unanswered.

A compact, cache-friendly trie for storing large word lists with three jobs in mind:

- **Browsable indexes** — words are grouped into folders (≈100 per folder) for navigation.
- **Fast lookup** — resolve a word to its expression index in `O(word length)`.
- **Spelling suggestions** — frequency-aware fuzzy matching via a Damerau-Levenshtein
  state machine optimised for edit distance ≤ 1.

The tree is stored as a width-first array of 12-byte nodes (~88 bits each) and can be
serialised with [`rkyv`](https://rkyv.org) for zero-copy, memory-mapped access. See the
[deep dive](#word-tree) below for the data structure, edit-distance design, and benchmarks.

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

// Fast word -> expression-index lookup.
assert_eq!(tree.index_of("apple"), Some(1));

// Browsable folder path for a word.
let _path = tree.path_of("apricot");

// Frequency-aware suggestions for a (mis)typed query; the closure filters
// which expression indices are acceptable candidates.
let _suggestions = tree.suggestions("aple", |_expr_index| true);
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
  - [Data storage needs](#data-storage-needs)
  - [Data structure](#data-structure)
  - [Edit distance](#edit-distance)
- [Benchmarking](#benchmarking)
  - [Find word index](#find-word-index)
  - [Find path for word](#find-path-for-word)
  - [Suggestions](#suggestions)
  - [Generation](#generation)
- [Information about the data](#information-about-the-data)
  - [English](#english)
    - [Node child count distribution](#node-child-count-distribution)
    - [Expr source count distribution](#expr-source-count-distribution)
  - [Swedish](#swedish)
    - [Node child count distribution](#node-child-count-distribution-1)
    - [Expr source count distribution](#expr-source-count-distribution-1)

# Word Tree

This is a custom trie for storing words as a tree of chars.
It is heavily size optimized and uses an array of Nodes.

A node is followed by all it's siblings thus the tree is stored in a width-first manner.

The tree has some nodes marked as folders in a manner that,
if possible, a maximum of X words is stored per folder.
X is normally 100.

## Uses

The tree is:

1. the base for providing a user-browsable index
   where each index page contains folders and a maximum of
   100 words.

2. used for finding the expr index of a word.

3. use for proposing words with similar spelling to a given string.

Note that normally expressions with spaces should not be included,
because they are never proposed and only kept for translations of
other non-spaced expressions.

## Languages

Alphabetic languages have in general up to 40 different characters plus some special ones (-, +, etc).
It can be thought of as up to 100 different chars.

As-you-type proposals handling mis-spelt strings are of value.

Logographic languages (chinese, etc) have thousands of frequently used logograms and in total tens of thousands.
These languages are often used with special entry forms. They are currently not the focus of the word-tree.

## Data storage needs

| info                | bits needed | comment                                                              |
| ------------------- | ----------- | -------------------------------------------------------------------- |
| first_rel_child_pos | 18          | relative position of first child (for english the max val is 141076) |
| node_char           | 24          | only three of the bytes are used in utf32                            |
| is_folder           | 1           |
| is_last_child       | 1           |
| source_count        | 7           | source count of current word                                         |
| max_source_count    | 7           | max_source_count of the children                                     |
| expr_index          | 20          |
| TOT                 | 84          |

The _max_source_count_ is used for facilitating the [pruning radix tree](https://towardsdatascience.com/the-pruning-radix-trie-a-radix-trie-on-steroids-412807f77abc) algorightm.

## Data structure

Possible space optimisation that stores data in a byte array, with the further advantage
that if read from disk, there is no need to type-cast or transform the data.

| info             | bits |
| ---------------- | ---- |
| first_child_pos  | 24   |
| node_char        | 24   |
| is_folder        | 1    |
| is_last_child    | 1    |
| source_count     | 7    |
| max_source_count | 7    |
| expr_index       | 24   |
| TOT              | 88   |

## Edit distance

An edit distance of 1 should be sufficient, if transposition is accounted for (i.e. swapping two chars when writing).

This enables a state-based edit distance evaluation, removing any need for costly matrix-based calculations. See [src/editdist/README.md](src/editdist/README.md)

# Benchmarking

> Measured on an Apple M4 Pro against the word lists bundled in `benches/data/`
> (~638k English, ~113k Swedish words). Run `cargo bench` to reproduce. Lookup
> (`index` / `path`) is `O(word length)` and cheap; suggestions scale with tree
> size, so they are markedly more expensive on these full word lists.

## Find word index

| lang | Index test                         | median (M4 Pro) |
| ---- | ---------------------------------- | --------------- |
| sv   | Index of (2 chars) ut              | 80.23 ns        |
| sv   | Index of (14 chars) rekommendation | 104.06 ns       |
| en   | Index of (2 chars) on              | 85.14 ns        |
| en   | Index of (14 chars) alphanumerical | 123.42 ns       |

## Find path for word

| lang | Path test                      | median (M4 Pro) |
| ---- | ------------------------------ | --------------- |
| sv   | Path (2 chars) ut              | 87.98 ns        |
| sv   | Path (14 chars) rekommendation | 130.16 ns       |
| en   | Path (2 chars) on              | 89.77 ns        |
| en   | Path (14 chars) alphanumerical | 149.14 ns       |

## Suggestions

| lang | Suggestions test                      | median (M4 Pro) |
| ---- | ------------------------------------- | --------------- |
| sv   | Suggestions (2 chars) u\_             | 19.91 us        |
| sv   | Suggestions (14 chars) rekommendat_on | 38.69 us        |
| en   | Suggestions (2 chars) o\_             | 49.59 us        |
| en   | Suggestions (14 chars) alphanumeri_al | 76.97 us        |

## Generation

Building the whole tree from the bundled TSV with `Tree::from_tsv`.

| lang | Generation test | median (M4 Pro) |
| ---- | --------------- | --------------- |
| sv   | tree            | 63.88 ms        |
| en   | tree            | 384.31 ms       |

# Information about the data

> Generated from the bundled `benches/data/*.tsv.zst` files by
> `cargo run --release --example stats`. Re-run it to regenerate these figures
> if the data changes.

## English

| what              | result             |
| ----------------- | ------------------ |
| Node count        | avg: 0.9999, max: 114 |
| Total nodes       | 2313796            |
| Total exprs       | 638545             |
| max_rel_child_pos | 312175             |
| max_source_count  | 1000               |
| max depth         | 182                |

<br>

### Node child count distribution

Index = child count; the last bucket (39) is "39 or more".

[536639, 1544069, 137393, 42752, 19027, 10111, 6180, 4034, 2769, 2052, 1541, 1214, 978, 822, 652, 539, 464, 374, 363, 316, 265, 225, 162, 137, 121, 85, 88, 77, 58, 32, 37, 29, 30, 18, 15, 13, 8, 16, 9, 82]

<br>

### Expr source count distribution

Bucket `i` covers source counts `i*10 .. i*10+9` (bucket 0 is dominated by the interior trie nodes, whose source count is 0).

[1710172, 6373, 6367, 6360, 6371, 6354, 6374, 6333, 6338, 6361, 6287, 6272, 6354, 6327, 6291, 6272, 6348, 6364, 6254, 6247, 6237, 6366, 6371, 6298, 6189, 6159, 6264, 6347, 6356, 6334, 6117, 6087, 6164, 6329, 6340, 6327, 6257, 5946, 6012, 6273, 6270, 6276, 6236, 5686, 5889, 6111, 6119, 6096, 5541, 5448, 5653, 5655, 4777, 5245, 4869, 5687, 6355, 6322, 6306, 6317, 6250, 6261, 6219, 6196, 6174, 6081, 5893, 5101, 5489, 6322, 6316, 6295, 6242, 6164, 6046, 5460, 6010, 6289, 6197, 5594, 6211, 6196, 5792, 6245, 5882, 5963, 6073, 6034, 6049, 5981, 6067, 6046, 6043, 6122, 6021, 6088, 6067, 6092, 6071, 6103, 1]

## Swedish

| what              | result            |
| ----------------- | ----------------- |
| Node count        | avg: 0.9996, max: 67 |
| Total nodes       | 487354            |
| Total exprs       | 113220            |
| max_rel_child_pos | 62385             |
| max_source_count  | 1000              |
| max depth         | 85                |

<br>

### Node child count distribution

Index = child count; the last bucket (39) is "39 or more".

[94984, 351914, 24406, 6907, 3014, 1656, 1039, 717, 539, 410, 333, 242, 190, 176, 142, 102, 113, 98, 83, 68, 43, 32, 29, 28, 14, 11, 14, 10, 7, 2, 4, 0, 1, 0, 2, 1, 2, 0, 0, 21]

<br>

### Expr source count distribution

Bucket `i` covers source counts `i*10 .. i*10+9` (bucket 0 is dominated by the interior trie nodes, whose source count is 0).

[378463, 1139, 1139, 1138, 1139, 1138, 1136, 1136, 1139, 1135, 1138, 1135, 1137, 1136, 1136, 1133, 1136, 1140, 1135, 1129, 1136, 1134, 1139, 1125, 1125, 1130, 1122, 1136, 1139, 1137, 1121, 1105, 1126, 1114, 1135, 1138, 1139, 1108, 1095, 1114, 1115, 1136, 1138, 1138, 1120, 1083, 1077, 1118, 1133, 1137, 1132, 1080, 1061, 1107, 1134, 1135, 1112, 973, 1101, 1112, 1083, 920, 1047, 930, 933, 1137, 1133, 1123, 1116, 1112, 1108, 1093, 1094, 1079, 983, 988, 1128, 1115, 1110, 1081, 977, 1126, 1106, 989, 1109, 1102, 1065, 1058, 1104, 1073, 1075, 1058, 1077, 1050, 1056, 1068, 1046, 1033, 1081, 1070, 1]
