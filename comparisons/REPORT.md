# wordtree vs. alternatives — a reproducible comparative study

> Everything here is reproducible from this repo:
> ```
> cargo run -p comparisons --bin quality --release   # suggestion quality tables
> cargo run -p comparisons --bin size    --release   # size + RAM table
> cargo bench -p comparisons                         # latency (criterion)
> cargo test  -p comparisons                         # correctness gate + verified findings
> ```
> Measured on an Apple M-series laptop, Rust 1.96, against the word lists bundled
> in `../benches/data/` (the same data the wordtree benches use).

## What this measures

wordtree folds three jobs into *one* compact, memory-mappable structure: a
browsable folder index (`path_of`), exact word→`expr_index` lookup (`index_of`),
and frequency-aware fuzzy suggestions (`suggestions`). Almost no single
alternative does all three, so each job is measured against the best specialist
*for that job*, on the same data. The question is not whether wordtree wins a
single axis (it generally does not) but what folding three jobs into one
mmap-able file costs on each.

## Engines compared

| engine | job(s) | notes |
| --- | --- | --- |
| **wordtree** | lookup + path + suggest + autocomplete | the subject; zero-copy via rkyv; `suggestions` returns both fuzzy corrections and prefix completions |
| **[fst](https://crates.io/crates/fst)** (BurntSushi) | lookup + fuzzy | ordered FSA/transducer, mmap, Levenshtein automaton |
| **[symspell](https://crates.io/crates/symspell)** | suggest | Symmetric-Delete spelling correction, frequency-ranked |
| **[pruning_radix_trie](https://crates.io/crates/pruning_radix_trie)** | autocomplete | Wolf Garbe's freq-ranked prefix trie (wordtree's pruning is modelled on it) |
| **[boomphf](https://crates.io/crates/boomphf)** | lookup | minimal perfect hash map (`BoomHashMap`, stores keys) |
| **HashMap / sorted-Vec** | lookup | naive baselines |
| **brute force** ([strsim](https://crates.io/crates/strsim)) | suggest | DL≤1 scan; the oracle + a baseline |

## Method & fairness

- **One canonical word list per language** feeds every engine, so a
  [correctness gate](tests/correctness.rs) asserts they all resolve a word to the
  *same* `expr_index` before any timing is trusted (it does).
- **Parsing.** Some bundled words contain a bare `"` (`domino"`, `"├─┤`). csv's
  default quoting would treat that as a field quote and swallow whole spans, so
  the loader sets `quoting(false)`; every row then parses and the lists are fully
  unique — **en 638,545 words, sv 113,220** (word count == expr count). Tokens
  are kept verbatim, including non-words like `!Bang!` and `%SYSTEMROOT%`. fst and
  boomphf require unique keys, which holds here.
- **The fuzzy task is normalised**: "given a query string, return dictionary
  words within Damerau-Levenshtein ≤1, frequency-ranked." Every engine gets the
  *same literal typo string* (no `_` placeholder), and symspell/fst are pinned to
  edit distance 1 to match wordtree.
- Typo queries are generated deterministically: take the highest-frequency clean
  (a–z) words and apply one edit at the midpoint — delete / insert / substitute /
  transpose — so recall can be broken down by edit kind.

---

## 1. Suggestion quality

The most revealing axis, and the source of two findings worth knowing. Recall =
did the engine return the intended word (the one the typo was derived from). The
brute-force DL≤1 set is the oracle (100% by definition).

### en

**Typo correction — recall of the intended word, by edit kind**

| engine | substitute | transpose | delete | insert | overall |
| ------ | ---------: | --------: | -----: | -----: | ------: |
| wordtree | 100% | 100% | 96% | 100% | 99% |
| symspell | 100% | 100% | 100% | 100% | 100% |
| fst-lev | 100% | 0% | 100% | 100% | 75% |

**Suggestion-set shape (avg over 200 typo queries)**

| engine | avg results returned | avg recall of full DL≤1 set |
| ------ | -------------------: | --------------------------: |
| wordtree | 4.9 | 78% |
| symspell | 5.6 | 100% |
| fst-lev | 5.2 | 85% |

**Autocomplete — top-5 agreement with the frequency oracle (25 prefixes)**

| engine | recall@5 |
| ------ | -------: |
| wordtree | 63% |
| pruning-trie | 74% |

### sv

**Typo correction — recall of the intended word, by edit kind**

| engine | substitute | transpose | delete | insert | overall |
| ------ | ---------: | --------: | -----: | -----: | ------: |
| wordtree | 100% | 100% | 100% | 100% | 100% |
| symspell | 100% | 100% | 100% | 100% | 100% |
| fst-lev | 100% | 0% | 100% | 100% | 75% |

**Suggestion-set shape (avg over 200 typo queries)**

| engine | avg results returned | avg recall of full DL≤1 set |
| ------ | -------------------: | --------------------------: |
| wordtree | 4.5 | 92% |
| symspell | 2.3 | 100% |
| fst-lev | 2.1 | 78% |

**Autocomplete — top-5 agreement with the frequency oracle (25 prefixes)**

| engine | recall@5 |
| ------ | -------: |
| wordtree | 68% |
| pruning-trie | 93% |

### Finding A — wordtree corrects every single-edit typo, indels included

wordtree corrects all four single-character edit kinds at edit distance 1:
substitution and transposition (100%) **and** the length-changing edits,
deletion and insertion. On sv it matches symspell's per-kind recall exactly
(100% across the board); on en it reaches 99% overall, the only gap being a few
deletions (96%) where the intended word is crowded out of the top-k by
higher-frequency neighbours within distance 1 — a ranking/cap effect, not a
missing capability.

Verified two ways: the per-kind table above, and an isolated three-word
reproduction in [`tests/correctness.rs`](tests/correctness.rs) of the README's
own example — `suggestions("aple", …)` now returns "apple".

Mechanism: `dist_search` (`src/search/distsearch.rs`) carries an incremental
Damerau-Levenshtein row down the trie (`src/editdist/dlrow.rs`): `row[n]` is the
distance to each node's word and `min(row)` prunes whole subtrees once they are
out of range, so all four edit kinds are handled uniformly while visiting only
~2–3% of the tree. (Earlier versions used a 4-window state machine that, driven
over a *branching* trie, mis-scored mid-word indels and pruned them away — delete
recall ~6–10%, insert 0%; the row-based walk replaced it.)

### Finding B — fst's Levenshtein has no transposition

fst's `Levenshtein` automaton is **plain** Levenshtein, so it scores a
transposition as distance 2 and misses it at max distance 1 (transpose recall
**0%**). wordtree and symspell are Damerau (transposition = 1). Building
transposition-tolerant fuzzy search on fst needs distance 2 (a much larger
automaton) or a custom Damerau automaton.

### Reading these tables

wordtree is a **small, frequency-ranked, as-you-type suggester** (≈3–4 results,
capped at ~3 spelling corrections in `src/search/mod.rs`), not an exhaustive
corrector. symspell is **exhaustive** (100% recall of the DL≤1 set) and the right
tool when you must find every correction, indels included. For autocomplete the
pruning trie tracks the frequency oracle better than wordtree's extension
suggestions (93% vs 70% on sv) — it is purpose-built for top-k prefix completion.

---

## 2. Size & memory

`live heap` = bytes the structure holds after construction (measured with a
counting global allocator, one engine per process). `peak build` = high-water
mark during construction. `serialized` = on-disk / mmappable bytes where the
engine has one.

| lang | engine | unique words | live heap (MiB) | peak build (MiB) | serialized (MiB) |
| ---- | ------ | -----------: | --------------: | ---------------: | ---------------: |
| en | wordtree | 638545 | 26.48 | 236.96 | 26.48 |
| en | fst | 638545 | 10.00 | 30.51 | 6.70 |
| en | boomphf | 638545 | 33.92 | 68.71 | - |
| en | hashmap | 638545 | 38.66 | 38.66 | - |
| en | sorted-vec | 638545 | 25.14 | 34.89 | - |
| en | symspell | 638545 | 300.42 | 301.66 | - |
| en | pruning-trie | 638545 | 79.07 | 79.07 | - |
| sv | wordtree | 113220 | 5.58 | 35.15 | 5.58 |
| sv | fst | 113220 | 1.25 | 7.35 | 1.19 |
| sv | boomphf | 113220 | 4.58 | 9.26 | - |
| sv | hashmap | 113220 | 5.16 | 5.16 | - |
| sv | sorted-vec | 113220 | 4.49 | 7.94 | - |
| sv | symspell | 113220 | 60.86 | 71.60 | - |
| sv | pruning-trie | 113220 | 17.52 | 17.52 | - |

(wordtree stores a 12-byte node per trie node — 2,313,796 nodes en / 487,354 sv —
which is exactly the 26.48 / 5.58 MiB live-and-serialized figures.)

### Finding C — wordtree is *not* the compact one; fst is ~4× smaller

fst is the clear size winner (6.7 MiB serialized / 10 MiB live en) — it minimises
shared **prefixes and suffixes** (DAWG-like). The key-storing structures cluster
together: sorted-Vec 25, **wordtree 26**, boomphf 34, HashMap 39 MiB (en). So
wordtree is competitive with the naive key-storing maps and ~4× *larger* than fst,
which does exact lookup *and* fuzzy in that smaller space. wordtree shares
prefixes only and stores a 12-byte node per character, so its "size-optimised"
framing holds against a naive trie but **not** against an FSA. pruning-trie (79)
and symspell (300) are in another league entirely.

What wordtree's bytes buy that fst's don't: inline `percentile` (frequency) and
folder structure on every node, and **zero-copy mmap** — its serialized form *is*
its in-memory form (`live == serialized`, 26.48 MiB both), so loading is an mmap
with no parse or rebuild. fst is also mmap-able; HashMap/sorted-Vec/symspell are
not (they must be rebuilt at startup).

Two more honest notes:
- **wordtree's build peaks at ~9× its final size** (237 MiB to produce 26 MiB),
  via the `ego-tree` builder — relevant if you generate trees on a constrained
  device.
- **symspell trades memory for recall**: 300 MiB (en) for its delete-dictionary,
  ~11× wordtree and ~45× fst, with no compact serialization.
- **boomphf** (`BoomHashMap`) stores the keys, so it can reject non-members; its
  34 MiB therefore includes the keys. It is lookup-only (no prefix/fuzzy/folders).

---

## 3. Latency

Criterion medians. Lookup is in **nanoseconds**; suggestions/autocomplete in
**microseconds / milliseconds**.

### Exact lookup (ns)

| case | wordtree | fst | boomphf | hashmap | sorted-vec |
| ---- | -------: | --: | ------: | ------: | ---------: |
| en short `on` | 90.8 | 15.2 | 14.1 | **7.5** | 65.7 |
| en long `alphanumerical` | 121.5 | 94.3 | 25.8 | **8.5** | 80.5 |
| sv short `ut` | 77.9 | 18.0 | 13.7 | **8.3** | 54.0 |
| sv long `rekommendation` | 101.3 | 84.5 | 21.4 | **8.6** | 67.4 |

HashMap wins outright (~8 ns, flat). boomphf is next (~14–26 ns; it does a
membership check plus a fetch). wordtree is the **slowest** here (~80–120 ns,
~10–14× HashMap): `index_of` linearly scans each node's siblings. fst sits in the
middle. All are tens of ns — fine in absolute terms, but exact lookup is not a
reason to pick wordtree.

### Fuzzy suggestions (edit distance 1)

A *substitution* typo and a *deletion* typo — both now corrected by every engine
(Finding A). Brute force is the full DL≤1 scan.

| case | wordtree | symspell | fst-lev | brute force |
| ---- | -------: | -------: | ------: | ----------: |
| en sub `abxut` | 96.9 µs | **1.5 µs** | 125.6 µs | 97.7 ms |
| en del `abut` | 90.3 µs | **8.1 µs** | 127.0 µs | 86.9 ms |
| sv sub `apxil` | 50.6 µs | **1.0 µs** | 74.3 µs | 18.2 ms |
| sv del `apil` | 48.0 µs | **2.4 µs** | 70.7 µs | 16.7 ms |

### Autocomplete (prefix top-5)

| case | wordtree | pruning-trie |
| ---- | -------: | -----------: |
| en `co` | 62.6 µs | **1.3 µs** |
| sv `ko` | 32.0 µs | **1.3 µs** |

### Finding D — suggestion latency: well behind symspell, ahead of fst-lev

symspell is the clear winner at ~1–8 µs. wordtree (≈50 µs sv, ≈95 µs en) is
**~50–65× slower than symspell** on substitutions; symspell also returns the
*complete* DL≤1 set whereas wordtree returns a short, frequency-capped list, so
for exhaustive spell-checking symspell wins outright.

Against fst-lev (≈70–130 µs) wordtree is **~1.3–1.5× faster** on these queries
while correcting the same edit kinds *plus* transpositions (fst-lev is plain
Levenshtein). For autocomplete, pruning_radix_trie (≈1.3 µs) is **~25–50× faster**
and more accurate.

The residual cost is structural: `suggestions` runs a breadth-first edit-distance
walk — one small dynamic-programming row per visited node — plus a frequency
search over the frontier on every call, whereas symspell does a handful of hash
lookups against precomputed deletes and the pruning trie does a pruned top-k
descent.

---

## Bottom line

On **every individual axis, a specialist beats wordtree**:

- **Exact lookup:** HashMap and boomphf are fastest; wordtree is the slowest
  (~10–14× HashMap), though still tens of ns. Not a reason to choose wordtree.
- **Size:** fst does lookup+fuzzy in ~4× less space; wordtree is mid-pack with the
  naive key-storing maps. wordtree's bytes buy inline frequency + folders +
  zero-copy mmap.
- **Fuzzy suggestions:** wordtree now corrects all four single-edit kinds
  (Finding A), but symspell is still ~50–65× faster and returns the *complete*
  DL≤1 set, whereas wordtree returns a short, frequency-capped list — so for
  exhaustive spell-checking symspell wins. wordtree is ~1.3–1.5× faster than
  fst-lev, and unlike fst's plain-Levenshtein automaton it also corrects
  transpositions.
- **Autocomplete:** pruning_radix_trie is ~25–50× faster and tracks the frequency
  oracle better.

**The case for wordtree is the *combination*, not any single axis**: a browsable
folder index + frequency + as-you-type suggestions in **one** structure that
loads by zero-copy mmap with no parse/build step (`live == serialized`), returning
a deliberately short, frequency-ranked, single-edit-tolerant list (substitutions,
transpositions and indels). If you need all three jobs from one mmappable file and
can live with a small frequency-capped suggestion set rather than the exhaustive
DL≤1 set, it is a reasonable single dependency. If you need any one job on its
own — or low suggestion latency, or minimum size — reach for the specialist.

### Caveats & honesty notes

- All engines run on the **same** lists, parsed with `quoting(false)`; the lists
  are already unique, so the keep-first dedup applied uniformly is a no-op.
- **boomphf** uses `BoomHashMap` (stores keys), so it rejects non-members and its
  size includes the keys; it is lookup-only. (A bare `Mphf` would be far smaller
  but could not reject non-members.)
- **fst-lev** ranking uses a side percentile table (counted in fairness, not in
  the reported fst size) and is plain Levenshtein (no transposition).
- **symspell** has no compact serialization; its size is pure RAM.
- **path_of** (browsable folders) has no library rival and so is not raced here —
  it is a wordtree differentiator, not a contest.
- Numbers are from one M-series machine; treat them as ratios, not absolutes.
  Re-run with the commands at the top.
