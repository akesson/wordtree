# wordtree vs. alternatives — a reproducible comparative study

> Everything here is reproducible from this repo:
> ```
> cargo run -p comparisons --bin quality --release   # autocomplete + correction quality tables
> cargo run -p comparisons --bin size    --release   # size + RAM table
> cargo bench -p comparisons                         # latency (criterion)
> cargo test  -p comparisons                         # correctness gate + verified findings
> ```
> Measured on an Apple M-series laptop, Rust 1.96, against the word lists bundled
> in `../benches/data/` (the same data the wordtree benches use).

## What this measures

wordtree folds three jobs into *one* compact, memory-mappable structure: a
**browsable index** (`path_of`), **exact lookup** (`index_of`), and **typo-tolerant
autocomplete** (`suggestions`) — autocomplete that also corrects typos. That third job
has two halves, each with its own specialist, so it is measured as two tasks:
**autocomplete** (`completions`, prefix completion) and **spelling correction**
(`corrections`, fuzzy matching). Almost no single alternative does all three jobs, so
each is measured against the best specialist *for that task*, on the same data. The
question is not whether wordtree wins a single axis (it generally does not) but what
folding everything into one mmap-able file costs on each.

## Engines compared

| engine | job(s) | notes |
| --- | --- | --- |
| **wordtree** | exact lookup + browsable index + typo-tolerant autocomplete | the subject; zero-copy via rkyv; `suggestions()` merges autocomplete + spelling correction, or call `completions()` / `corrections()` for one half |
| **[fst](https://crates.io/crates/fst)** (BurntSushi) | exact lookup + spelling correction | ordered FSA/transducer, mmap, Levenshtein automaton |
| **[symspell](https://crates.io/crates/symspell)** | spelling correction | Symmetric-Delete spelling correction, frequency-ranked |
| **[pruning_radix_trie](https://crates.io/crates/pruning_radix_trie)** | autocomplete | Wolf Garbe's freq-ranked prefix trie (wordtree's pruning is modelled on it) |
| **[boomphf](https://crates.io/crates/boomphf)** | exact lookup | minimal perfect hash map (`BoomHashMap`, stores keys) |
| **HashMap / sorted-Vec** | exact lookup | naive baselines |
| **brute force** ([strsim](https://crates.io/crates/strsim)) | spelling correction | DL≤1 scan; the oracle + a baseline |

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
- **The spelling-correction task is normalised**: "given a query string, return
  dictionary words within Damerau-Levenshtein ≤1, frequency-ranked." Every engine gets the
  *same literal typo string* (no `_` placeholder), and symspell/fst are pinned to
  edit distance 1 to match wordtree.
- Typo queries are generated deterministically: take the highest-frequency clean
  (a–z) words and apply one edit at the midpoint — delete / insert / substitute /
  transpose — so recall can be broken down by edit kind.

---

## 1. Autocomplete & correction quality

The most revealing axis, and the source of two findings worth knowing. Recall =
did the engine return the intended word (the one the typo was derived from). The
brute-force DL≤1 set is the oracle (100% by definition).

### en

**Spelling correction — recall of the intended word, by edit kind**

| engine | substitute | transpose | delete | insert | overall |
| ------ | ---------: | --------: | -----: | -----: | ------: |
| wordtree | 100% | 100% | 100% | 100% | 100% |
| symspell | 100% | 100% | 100% | 100% | 100% |
| fst-lev | 100% | 0% | 100% | 100% | 75% |

**Correction-set shape (avg over 200 typo queries)**

| engine | avg results returned | avg recall of full DL≤1 set |
| ------ | -------------------: | --------------------------: |
| wordtree | 4.8 | 78% |
| symspell | 5.6 | 100% |
| fst-lev | 5.2 | 85% |

**Autocomplete — top-5 agreement with the frequency oracle (25 prefixes)**

| engine | recall@5 |
| ------ | -------: |
| wordtree (suggestions) | 64% |
| wordtree (completions) | 72% |
| pruning-trie | 74% |

### sv

**Spelling correction — recall of the intended word, by edit kind**

| engine | substitute | transpose | delete | insert | overall |
| ------ | ---------: | --------: | -----: | -----: | ------: |
| wordtree | 100% | 100% | 100% | 100% | 100% |
| symspell | 100% | 100% | 100% | 100% | 100% |
| fst-lev | 100% | 0% | 100% | 100% | 75% |

**Correction-set shape (avg over 200 typo queries)**

| engine | avg results returned | avg recall of full DL≤1 set |
| ------ | -------------------: | --------------------------: |
| wordtree | 4.5 | 92% |
| symspell | 2.3 | 100% |
| fst-lev | 2.1 | 78% |

**Autocomplete — top-5 agreement with the frequency oracle (25 prefixes)**

| engine | recall@5 |
| ------ | -------: |
| wordtree (suggestions) | 68% |
| wordtree (completions) | 85% |
| pruning-trie | 93% |

### Finding A — wordtree corrects every single-edit typo, indels included

wordtree corrects all four single-character edit kinds at edit distance 1:
substitution and transposition (100%) **and** the length-changing edits,
deletion and insertion. It matches symspell's per-kind recall exactly — 100%
across the board on both en and sv. (The result is still a small frequency-capped
top-k, so in principle a correct word could be crowded out by higher-frequency
neighbours within distance 1; on this sample that does not happen.)

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
autocomplete-only `completions()` call tracks the frequency oracle far better than
the combined `suggestions()` — 85% vs 68% recall@5 on sv — because it spends the
whole result budget on extensions instead of sharing it with spelling
corrections; the purpose-built pruning trie is still best (93%).

---

## 2. Size & memory

`live heap` = bytes the structure holds after construction (measured with a
counting global allocator, one engine per process). `peak build` = high-water
mark during construction. `serialized` = on-disk / mmappable bytes where the
engine has one.

| lang | engine | unique words | live heap (MiB) | peak build (MiB) | serialized (MiB) |
| ---- | ------ | -----------: | --------------: | ---------------: | ---------------: |
| en | wordtree | 638545 | 21.11 | 223.58 | 21.11 |
| en | fst | 638545 | 10.00 | 30.51 | 6.70 |
| en | boomphf | 638545 | 33.92 | 68.71 | - |
| en | hashmap | 638545 | 38.66 | 38.66 | - |
| en | sorted-vec | 638545 | 25.14 | 34.89 | - |
| en | symspell | 638545 | 300.42 | 301.66 | - |
| en | pruning-trie | 638545 | 79.07 | 79.07 | - |
| sv | wordtree | 113220 | 4.35 | 32.49 | 4.35 |
| sv | fst | 113220 | 1.25 | 7.35 | 1.19 |
| sv | boomphf | 113220 | 4.58 | 9.26 | - |
| sv | hashmap | 113220 | 5.16 | 5.16 | - |
| sv | sorted-vec | 113220 | 4.49 | 7.94 | - |
| sv | symspell | 113220 | 60.86 | 71.60 | - |
| sv | pruning-trie | 113220 | 17.52 | 17.52 | - |

(wordtree stores an 8-byte node per trie node — 2,313,796 nodes en / 487,354 sv —
plus compact per-word side tables: a word-bitvector, a rank index, and a 5-byte
`percentile`+`expr_index` entry per word. en: 17.65 MiB nodes + 3.46 MiB side =
21.11 MiB; sv: 3.72 + 0.63 = 4.35 MiB — both the live-and-serialized figures.)

### Finding C — wordtree is *not* the compact one; fst is ~3× smaller

fst is the clear size winner (6.7 MiB serialized / 10 MiB live en) — it minimises
shared **prefixes and suffixes** (DAWG-like). The key-storing structures cluster
together: **wordtree 21**, sorted-Vec 25, boomphf 34, HashMap 39 MiB (en). So
wordtree is the smallest of the naive key-storing structures yet still ~3× *larger*
than fst, which does exact lookup *and* spelling correction in that smaller space.
wordtree shares prefixes only and stores an 8-byte node per character plus a
per-word side table, so its "size-optimised" framing holds against a naive trie
but **not** against an FSA. pruning-trie (79) and symspell (300) are in another
league entirely.

What wordtree's bytes buy that fst's don't: inline `max_child_percentile` and
folder structure on every node, the per-word frequency + word→`expr_index`
mapping, and **zero-copy mmap** — its serialized form *is* its in-memory form
(`live == serialized`, 21.11 MiB both), so loading is an mmap with no parse or
rebuild. fst is also mmap-able; HashMap/sorted-Vec/symspell are not (they must be
rebuilt at startup).

Two more honest notes:
- **wordtree's build peaks at ~11× its final size** (224 MiB to produce 21 MiB),
  via the `ego-tree` builder — relevant if you generate trees on a constrained
  device.
- **symspell trades memory for recall**: 300 MiB (en) for its delete-dictionary,
  ~14× wordtree and ~45× fst, with no compact serialization.
- **boomphf** (`BoomHashMap`) stores the keys, so it can reject non-members; its
  34 MiB therefore includes the keys. It is exact-lookup-only (no autocomplete/correction/folders).

---

## 3. Latency

Criterion medians. Lookup is in **nanoseconds**; suggestions/autocomplete in
**microseconds / milliseconds**.

### Exact lookup (ns)

| case | wordtree | fst | boomphf | hashmap | sorted-vec |
| ---- | -------: | --: | ------: | ------: | ---------: |
| en short `on` | 72.0 | 15.3 | 13.5 | **7.5** | 65.8 |
| en long `alphanumerical` | 108.9 | 94.4 | 25.7 | **8.6** | 81.2 |
| sv short `ut` | 67.7 | 18.1 | 13.1 | **8.7** | 55.3 |
| sv long `rekommendation` | 95.4 | 85.4 | 21.4 | **8.6** | 69.0 |

HashMap wins outright (~8 ns, flat). boomphf is next (~13–26 ns; it does a
membership check plus a fetch). wordtree is the **slowest** here (~67–109 ns,
~8–13× HashMap): `index_of` linearly scans each node's siblings. fst sits in the
middle. All are tens of ns — fine in absolute terms, but exact lookup is not a
reason to pick wordtree. (The 8-byte node — down from 12 — trimmed these ~10–20%
versus earlier runs by fitting more siblings per cache line.)

### Spelling correction (edit distance 1)

A *substitution* typo and a *deletion* typo — both now corrected by every engine
(Finding A). `wordtree (corrections)` is the correction-only `corrections()` call; brute
force is the full DL≤1 scan.

| case | wordtree (suggestions) | wordtree (corrections) | symspell | fst-lev | brute force |
| ---- | ---------------------: | ---------------------: | -------: | ------: | ----------: |
| en sub `abxut` | 95.0 µs | 94.0 µs | **1.5 µs** | 126.8 µs | 103.2 ms |
| en del `abut` | 85.5 µs | 86.1 µs | **8.3 µs** | 128.7 µs | 92.1 ms |
| sv sub `apxil` | 49.5 µs | 49.2 µs | **1.0 µs** | 78.5 µs | 19.2 ms |
| sv del `apil` | 45.7 µs | 44.6 µs | **2.4 µs** | 71.1 µs | 17.2 ms |

`corrections()` (correction-only) costs essentially the same as the combined
`suggestions()` here — the Damerau walk is the cost, and a typo has no exact prefix
to complete, so the autocomplete sweep adds nothing.

### Autocomplete (prefix top-5)

`suggestions()` also runs the edit-distance walk; `completions()` is the autocomplete-only
call — the direct counterpart to the pruning trie's top-k.

| case | wordtree (suggestions) | wordtree (completions) | pruning-trie |
| ---- | ---------------------: | ---------------------: | -----------: |
| en `co` | 56.4 µs | 2.6 µs | **1.2 µs** |
| sv `ko` | 29.1 µs | 1.5 µs | **1.3 µs** |

### Finding D — correction latency trails symspell; autocomplete is close to the pruning trie

**Spelling correction.** symspell is the clear winner at ~1–8 µs. wordtree (≈49 µs sv,
≈95 µs en) is **~50–65× slower than symspell** on substitutions; against fst-lev
(≈70–129 µs) it is **~1.3–1.6× faster** while also correcting transpositions (fst-lev is
plain Levenshtein). The correction-only `corrections()` call costs essentially the same
as the combined `suggestions()` (94 vs 95 µs en, 49 vs 50 µs sv): the Damerau walk is the
dominant cost, and a typo has no exact prefix for the autocomplete sweep to extend.

**Autocomplete.** `suggestions()` runs the edit-distance walk on every call, even for a
clean prefix that only needs completion, so it is the wrong call to race against a pure
top-k completer. The autocomplete-only `completions()` call is the direct equivalent: it
skips the walk and lands at **~2.6 µs (en) / ~1.5 µs (sv)** — close to the pruning trie
(~1.2–1.3 µs): within ~1.15× on sv, ~2× on en. The higher `suggestions()` figure is the
bundled correction work, not the autocomplete.

The split is structural — the same mechanism behind Finding A: `corrections()` walks the
trie one DP row per visited node, whereas symspell does a handful of hash lookups against
precomputed deletes; `completions()`, like the pruning trie, is just a pruned top-k
descent. wordtree's small extra cost over the pruning trie is the per-kept-word rank
lookup into its side `values` table — the price of storing frequency off-node to keep the
node 8 bytes (§2).

---

## Bottom line

On **every individual axis, a specialist beats wordtree**:

- **Exact lookup:** HashMap and boomphf are fastest; wordtree is the slowest
  (~8–13× HashMap), though still tens of ns. Not a reason to choose wordtree.
- **Size:** fst does exact lookup + spelling correction in ~3× less space; wordtree is
  the smallest of the naive key-storing structures. wordtree's bytes buy inline
  frequency + folders + zero-copy mmap.
- **Spelling correction:** wordtree now corrects all four single-edit kinds
  (Finding A), but symspell is still ~50–65× faster and returns the *complete*
  DL≤1 set, whereas wordtree returns a short, frequency-capped list — so for
  exhaustive spell-checking symspell wins. wordtree is ~1.3–1.6× faster than
  fst-lev, and unlike fst's plain-Levenshtein automaton it also corrects
  transpositions.
- **Autocomplete:** with the autocomplete-only `completions()` call wordtree is
  close to pruning_radix_trie (~1.5–2.6 µs; within ~1.15× on sv, ~2× on en). The
  pruning trie still tracks the frequency oracle better (recall@5 93% vs 85% sv).
  The combined `suggestions()` is far slower here only because it also runs the
  correction walk.

**The case for wordtree is the *combination*, not any single axis**: a browsable index +
frequency + typo-tolerant autocomplete in **one** structure that loads by zero-copy mmap
with no parse/build step (`live == serialized`), returning a deliberately short,
frequency-ranked, single-edit-tolerant list (substitutions, transpositions and indels).
If you need all three jobs from one mmappable file and can live with that short set rather
than the exhaustive DL≤1 set, it is a reasonable single dependency. If you need any one
job on its own — or low correction latency, or minimum size — reach for the specialist.

### Caveats & honesty notes

- All engines run on the **same** lists, parsed with `quoting(false)`; the lists
  are already unique, so the keep-first dedup applied uniformly is a no-op.
- **boomphf** uses `BoomHashMap` (stores keys), so it rejects non-members and its
  size includes the keys; it is exact-lookup-only. (A bare `Mphf` would be far smaller
  but could not reject non-members.)
- **fst-lev** ranking uses a side percentile table (counted in fairness, not in
  the reported fst size) and is plain Levenshtein (no transposition).
- **symspell** has no compact serialization; its size is pure RAM.
- **path_of** (browsable folders) has no library rival and so is not raced here —
  it is a wordtree differentiator, not a contest.
- Numbers are from one M-series machine; treat them as ratios, not absolutes.
  Re-run with the commands at the top.
