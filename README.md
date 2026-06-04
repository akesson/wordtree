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
| sv   | Index of (2 chars) ut              | 31.95 ns        |
| sv   | Index of (14 chars) rekommendation | 55.97 ns        |
| en   | Index of (2 chars) on              | 50.39 ns        |
| en   | Index of (14 chars) alphanumerical | 87.26 ns        |

## Find path for word

| lang | Path test                      | median (M4 Pro) |
| ---- | ------------------------------ | --------------- |
| sv   | Path (2 chars) ut              | 41.80 ns        |
| sv   | Path (14 chars) rekommendation | 80.18 ns        |
| en   | Path (2 chars) on              | 54.85 ns        |
| en   | Path (14 chars) alphanumerical | 113.18 ns       |

## Suggestions

| lang | Suggestions test                      | median (M4 Pro) |
| ---- | ------------------------------------- | --------------- |
| sv   | Suggestions (2 chars) u\_             | 160.00 us       |
| sv   | Suggestions (14 chars) rekommendat_on | 362.16 us       |
| en   | Suggestions (2 chars) o\_             | 725.12 us       |
| en   | Suggestions (14 chars) alphanumeri_al | 1125.3 us       |

## Generation

Building the whole tree from the bundled TSV with `Tree::from_tsv`.

| lang | Generation test | median (M4 Pro) |
| ---- | --------------- | --------------- |
| sv   | tree            | 145.17 ms       |
| en   | tree            | 907.16 ms       |

# Information about the data

> Like the benchmarks above, these statistics describe the original private
> dataset, not the `benches/data/*.tsv.zst` files bundled here. The bundled
> lists hold ~638k English and ~113k Swedish words (each mapping to a distinct
> expression), so the node, expression and distribution counts below do not
> match what this repo would produce.

## English

| what              | result                 |
| ----------------- | ---------------------- |
| Node count        | avg: 0.9999, max: 161, |
| Total nodes       | 1012273                |
| Total exprs       | 299528                 |
| max_rel_child_pos | 141076                 |
| max_source_count  | 853                    |

<br>

### Node child count distribution

first = 0, last = 49 and more.

[247882, 649483, 69053, 21656, 9174, 4719, 2777, 1812, 1258, 887, 693, 540, 458, 348, 267, 208, 209, 148, 131, 101, 85, 60, 50, 43, 40, 42, 30, 19, 28, 8, 13, 10, 3, 7, 2, 1, 0, 1, 1, 26]

<br>

### Expr source count distribution

0: 0-9, 1: 10-19 etc...

[216070, 28829, 12598, 7414, 5177, 3747, 3120, 2567, 2269, 2064, 1666, 1400, 1254, 1109, 974, 852, 758, 663, 598, 558, 508, 441, 380, 378, 278, 244, 260, 226, 222, 213, 198, 186, 174, 157, 127, 120, 122, 111, 104, 80, 67, 56, 111, 57, 64, 74, 57, 64, 49, 46, 50, 45, 46, 41, 30, 38, 25, 35, 25, 23, 23, 15, 18, 13, 19, 23, 18, 14, 24, 6, 14, 11, 15, 13, 23, 19, 8, 7, 6, 11, 3, 0, 0, 3, 0, 3, 0, 0, 0, 0]

## Swedish

| what              | result               |
| ----------------- | -------------------- |
| Node count        | avg: 0.9999, max: 28 |
| Total nodes       | 19903                |
| Total exprs       | 4649                 |
| max_rel_child_pos | 2444                 |
| max_source_count  | 45                   |

<br>

### Node child count distribution

first = 0, last = 49 and more

[3739, 14297, 1234, 301, 114, 56, 49, 27, 21, 12, 15, 7, 3, 5, 4, 5, 3, 0, 2, 2, 1, 2, 1, 2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]

<br>

### Expr source count distribution

0: 0-9, 1: 10-19 etc...

[3836, 530, 202, 66, 15]

Position corresponds to count.

[0, 2216, 571, 308, 191, 115, 145, 102, 104, 84, 83, 74, 73, 51, 39, 50, 47, 45, 29, 39, 24, 20, 23, 41, 24, 19, 16, 12, 8, 15, 9, 7, 10, 11, 7, 3, 1, 8, 6, 4, 4, 0, 2, 1, 3, 5, 0, 0, 0, 0]
