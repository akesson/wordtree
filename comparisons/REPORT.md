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
| wordtree (suggestions, default) | 5.4 | 87% |
| wordtree (corrections, exhaustive) | 5.6 | 100% |
| symspell | 5.6 | 100% |
| fst-lev | 5.2 | 85% |

**Autocomplete — top-5 agreement with the frequency oracle (25 prefixes)**

| engine | recall@5 |
| ------ | -------: |
| wordtree (suggestions) | 64% |
| wordtree (completions) | 80% |
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
| wordtree (suggestions, default) | 4.9 | 98% |
| wordtree (corrections, exhaustive) | 2.3 | 100% |
| symspell | 2.3 | 100% |
| fst-lev | 2.1 | 78% |

**Autocomplete — top-5 agreement with the frequency oracle (25 prefixes)**

| engine | recall@5 |
| ------ | -------: |
| wordtree (suggestions) | 68% |
| wordtree (completions) | 96% |
| pruning-trie | 93% |

### Finding A — wordtree corrects every single-edit typo, indels included

wordtree corrects all four single-character edit kinds at edit distance 1:
substitution and transposition (100%) **and** the length-changing edits,
deletion and insertion. It matches symspell's per-kind recall exactly — 100%
across the board on both en and sv. (Per-kind recall is about the *intended* word;
how much of the *full* DL≤1 set comes back is a separate, configurable axis — see
Finding E.)

Verified two ways: the per-kind table above, and an isolated three-word
reproduction in [`tests/correctness.rs`](tests/correctness.rs) of the README's
own example — `suggestions("aple", …)` now returns "apple".

Mechanism: `dist_search` (`src/search/distsearch.rs`) carries an incremental
Damerau-Levenshtein row down the trie, stored as a narrow diagonal **band**
(`src/editdist/dlrow.rs`): the band cell for column `n` is the distance to each
node's word and the band minimum prunes whole subtrees once they are out of range,
so all four edit kinds are handled uniformly while visiting only ~2–3% of the tree.
Banding does not change which nodes are visited or the corrections returned — at
edit distance 1 the results are bit-identical to a full-row walk — only the per-node
cost (§3). (Earlier versions used a 4-window state machine that, driven over a
*branching* trie, mis-scored mid-word indels and pruned them away — delete recall
~6–10%, insert 0%; the row-based walk replaced it.)

### Finding B — fst's Levenshtein has no transposition

fst's `Levenshtein` automaton is **plain** Levenshtein, so it scores a
transposition as distance 2 and misses it at max distance 1 (transpose recall
**0%**). wordtree and symspell are Damerau (transposition = 1). Building
transposition-tolerant fuzzy search on fst needs distance 2 (a much larger
automaton) or a custom Damerau automaton.

### Finding E — the correction budget is a knob, and completions no longer pin the typed word

Two quality levers, both visible in the tables above.

**The spelling cap is configurable** ([`Caps`](../src/search/mod.rs)). The default
keeps the list short — up to 20 corrections for a typo, but only **2** when the
query is *itself* a valid word/prefix (so completions dominate when you have likely
typed what you meant). That second cap is what bounds English's full-set recall at
87%: deletion typos frequently spell *another real word* ("abut", "brad", "brin",
"cary"), landing on the found-prefix arm — raising the *typo* cap alone barely moves
it (≈86→87%). It is a deliberate as-you-type choice, not a ceiling:
`corrections_with(q, f, Caps::uniform(n))` opens the cap and returns the **complete
DL≤1 set — 100%, the same 5.6 / 2.3 average as symspell** (the "exhaustive" rows
above). So recall is no longer the trade-off; latency still is (§3.2).

**`completions()` ranks the exact word by frequency, not on top.** Many common
prefixes are themselves zero-frequency stub words ("co", "re", "un" …); pinning the
typed word at slot 1 wasted a top-5 slot on a word the user already typed. Ranking
it by its own frequency — the way the pruning trie and the oracle do — drops the
stub and recovers a real completion, lifting completions recall@5 to **80% (en) /
96% (sv), now ahead of the pruning trie (74 / 93)**.

### Reading these tables

By default wordtree is a **small, frequency-ranked, as-you-type suggester** — its
correction budget is a configurable `Caps` (default: 20 for a typo, 2 for a query
that is itself a valid word; Finding E), giving 87% / 98% recall of the full DL≤1
set (en / sv). When you need the *complete* set, `corrections_with(…, Caps::uniform)`
**matches symspell's 100%** (the "exhaustive" rows); symspell is still far faster
(§3.2), but recall is no longer the wordtree trade-off it once was. For autocomplete
the autocomplete-only `completions()` call tracks the frequency oracle far better
than the combined `suggestions()` — 96% vs 68% recall@5 on sv — because it spends
the whole result budget on extensions instead of sharing it with spelling
corrections, and after Finding E it now edges past the purpose-built pruning trie.

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

Criterion medians. Lookup is in **nanoseconds**; correction/autocomplete in
**microseconds / milliseconds**. The first three tables are *apples-to-apples*:
each races wordtree's matching **single-job** call against the rival specialist for
that job. The fourth (§3.4) is wordtree's combined `suggestions()` call, which has
no single-engine rival — it is raced instead against the two-specialist stack you
would otherwise assemble to get the same fuzzy-autocomplete behaviour.

### 3.1 Exact lookup (ns)

| case | wordtree | fst | boomphf | hashmap | sorted-vec |
| ---- | -------: | --: | ------: | ------: | ---------: |
| en short `on` | 74.4 | 15.2 | 14.1 | **7.6** | 68.3 |
| en long `alphanumerical` | 111.1 | 94.8 | 26.1 | **8.7** | 85.7 |
| sv short `ut` | 70.1 | 18.4 | 13.7 | **7.9** | 56.3 |
| sv long `rekommendation` | 95.7 | 86.9 | 21.5 | **9.1** | 70.8 |

HashMap wins outright (~8 ns, flat). boomphf is next (~14–26 ns; it does a
membership check plus a fetch). wordtree is the **slowest** here (~70–111 ns,
~8–13× HashMap): `index_of` linearly scans each node's siblings. fst sits in the
middle. All are tens of ns — fine in absolute terms, but exact lookup is not a
reason to pick wordtree. (The 8-byte node — down from 12 — trimmed these ~10–20%
versus earlier runs by fitting more siblings per cache line.)

### 3.2 Spelling correction (edit distance 1) — `corrections()` vs the correctors

A *substitution* typo and a *deletion* typo — both now corrected by every engine
(Finding A). `wordtree (corrections)` is the correction-only `corrections()` call;
brute force is the full DL≤1 scan. (The combined `suggestions()` figure lives in
§3.4; on a typo it is within a few percent of `corrections()`, since the input has
no exact prefix for the autocomplete sweep to extend.)

| case | wordtree (corrections) | symspell | fst-lev | brute force |
| ---- | ---------------------: | -------: | ------: | ----------: |
| en sub `abxut` | 44.8 µs | **1.4 µs** | 125.0 µs | 100.3 ms |
| en del `abut` | 49.8 µs | **8.5 µs** | 127.2 µs | 90.7 ms |
| sv sub `apxil` | 23.9 µs | **0.95 µs** | 77.6 µs | 18.9 ms |
| sv del `apil` | 25.5 µs | **2.4 µs** | 70.4 µs | 16.9 ms |

### 3.3 Autocomplete (prefix top-5) — `completions()` vs the pruning trie

`completions()` is the autocomplete-only call — the direct counterpart to the
pruning trie's top-k, skipping the edit-distance walk. (The combined `suggestions()`
runs the walk anyway, so its autocomplete cost belongs in §3.4, not here.)

Six of the most common prefixes per language (Criterion medians, ascending by
`completions()` time):

| prefix | wordtree (completions) | pruning-trie | ratio |
| ------ | ---------------------: | -----------: | ----: |
| en `de` | 2.3 µs | **1.2 µs** | 2.0× |
| en `co` | 3.0 µs | **1.2 µs** | 2.5× |
| en `pr` | 3.5 µs | **1.2 µs** | 2.9× |
| en `re` | 3.7 µs | **1.3 µs** | 2.9× |
| en `in` | 5.6 µs | **1.4 µs** | 4.0× |
| en `un` | 7.3 µs | **1.5 µs** | 4.9× |
| sv `ma` | 1.1 µs | **1.1 µs** | 1.1× |
| sv `st` | 1.4 µs | **0.9 µs** | 1.5× |
| sv `ka` | 1.6 µs | **1.3 µs** | 1.2× |
| sv `sk` | 1.6 µs | **1.2 µs** | 1.4× |
| sv `ko` | 1.7 µs | **1.3 µs** | 1.3× |
| sv `in` | 3.4 µs | **1.4 µs** | 2.5× |

The ratio is **not** flat. The pruning trie holds ~0.9–1.5 µs across every prefix,
while `completions()` scales with the prefix's fan-out — near-parity on a small
subtree (sv `ma`, 1.1×) up to ~5× on the highest-frequency English prefixes (`un`,
`in`), which sweep tens of thousands of descendants. wordtree pays a per-kept-word
rank lookup into its off-node `values` table (plus the small percentile sort from
Finding E) on every swept word, and prunes a little less aggressively; the pruning
trie visits fewer. So the autocomplete latency gap is **~1.1–2.5× (sv) / ~2–5×
(en), widening with prefix popularity** — wordtree's edge is recall (Finding E),
not speed.

### 3.4 The combined call — one structure vs a two-specialist stack

`suggestions()` does both jobs in **one call against one mmap**, so its fair rival is
not any single engine but the **completer + corrector pair** you would otherwise wire
together — and run *both* on every keystroke to get fuzzy autocomplete as you type.
On raw latency the combined call loses to a symspell-based stack and beats an
fst-based one; the axis it actually wins is **footprint and structure count**.

| metric (en) | wordtree `suggestions()` | pruning-trie + fst-lev | pruning-trie + symspell |
| ----------- | -----------------------: | ---------------------: | ----------------------: |
| structures to build/ship | **1** | 2 | 2 |
| live heap | **21.1 MiB** | 89.1 MiB | 379.5 MiB |
| zero-copy mmap | **yes** | partial (fst only) | no |
| latency / keystroke † | 43.1 µs | ≈126 µs | ≈2.6 µs |

sv has the same shape: live heap **4.35 MiB** vs 18.8 (fst stack) / 78.4 (symspell
stack); `suggestions()` 22.4 µs vs ≈79 / ≈2.2.

So against an **fst** stack the single wordtree call is ~3× *faster* and ~4× smaller
in one mmap; against a **symspell** stack it is ~16× slower on latency but ~18×
smaller and a single mmap (symspell has no compact serialization, so half that stack
cannot be mmapped). The combination wins on bytes and on "one file, one call" — not
on speed.

> † Per-keystroke latency for the stacks = one completer call + one corrector call,
> since fuzzy-as-you-type runs both. Completer = pruning-trie's measured top-k
> (~1.2 µs en / ~1.3 µs sv, effectively flat); corrector = the symspell / fst-lev
> figures from §3.2. The corrector dominates by ~100×, so the sum is an
> order-of-magnitude figure, not a same-input measurement. Heap figures are the
> live heaps from §2, summed.

### Finding D — correction trails symspell; autocomplete is close to the pruning trie; the combined call trades speed for one mmap

**Spelling correction (§3.2).** symspell is the clear winner at ~1–9 µs. wordtree's
`corrections()` (≈24 µs sv, ≈45 µs en) is **~25–31× slower than symspell** on
substitutions; against fst-lev (≈70–127 µs) it is **~2.5–3.2× faster** while also
correcting transpositions (fst-lev is plain Levenshtein).

**Autocomplete (§3.3).** Across six common prefixes the autocomplete-only
`completions()` runs **2.3–7.3 µs (en) / 1.1–3.4 µs (sv)** while the pruning trie
stays flat at ~0.9–1.5 µs, so the latency gap is **~2–5× (en) / ~1.1–2.5× (sv)**,
widening with the prefix's fan-out. wordtree's edge here is recall (Finding E), not
speed.

**The combined call (§3.4).** `suggestions()` runs the edit-distance walk on *every*
call, even for a clean prefix, so it is structurally slower than either specialist
alone — that is the price of one call doing both jobs. Its win is not latency but
delivering fuzzy autocomplete from a single 21 MiB mmap instead of a two-structure
stack (89–380 MiB).

The split is structural — the same mechanism behind Finding A: `corrections()` walks the
trie computing one **banded** DP row per visited node (a `2K+1`-cell diagonal window — 3
cells at K=1 — in [`src/editdist/dlrow.rs`](../src/editdist/dlrow.rs)), whereas symspell
does a handful of hash lookups against precomputed deletes; `completions()`, like the
pruning trie, is just a pruned top-k descent. The band makes each visited node cost O(K)
rather than O(query length), so the walk's cost tracks the number of nodes it visits, not
how long the query is — longer queries benefit most (the 4–5-char typos above roughly
halved versus a full-row walk; a 14-char query drops ~80%). wordtree's extra cost over
the pruning trie — a per-kept-word rank lookup into its side `values` table (the price
of storing frequency off-node to keep the node 8 bytes, §2), plus slightly looser
pruning — is small on shallow prefixes but scales with the swept subtree, reaching ~5×
on the highest-fan-out English prefixes (§3.3).

---

## Bottom line

On **every individual axis, a specialist beats wordtree**:

- **Exact lookup:** HashMap and boomphf are fastest; wordtree is the slowest
  (~8–13× HashMap), though still tens of ns. Not a reason to choose wordtree.
- **Size:** fst does exact lookup + spelling correction in ~3× less space; wordtree is
  the smallest of the naive key-storing structures. wordtree's bytes buy inline
  frequency + folders + zero-copy mmap.
- **Spelling correction:** wordtree corrects all four single-edit kinds (Finding A),
  and with `Caps::uniform` its `corrections()` returns the *complete* DL≤1 set —
  100%, matching symspell (Finding E). symspell now wins decisively on **speed**
  (~25–31× faster on substitutions), not recall. wordtree is ~2.5–3.2× faster than
  fst-lev, and unlike fst's plain-Levenshtein automaton it also corrects
  transpositions.
- **Autocomplete:** the pruning trie is consistently **faster** on latency — flat at
  ~1 µs, while the autocomplete-only `completions()` scales with prefix fan-out to ~2–5×
  that on popular English prefixes (§3.3) — but after Finding E wordtree is slightly
  **ahead on recall@5** (80% vs 74% en, 96% vs 93% sv). The combined `suggestions()` is
  far slower still because it also runs the correction walk.

**The case for wordtree is the *combination*, not any single axis**: a browsable index +
frequency + typo-tolerant autocomplete in **one** structure that loads by zero-copy mmap
with no parse/build step (`live == serialized`), returning a frequency-ranked,
single-edit-tolerant list (substitutions, transpositions and indels) — short by default,
or the complete DL≤1 set on request (`Caps`). If you need all three jobs from one
mmappable file, it is a reasonable single dependency. If you need any one job on its own —
or low correction latency, or minimum size — reach for the specialist.

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
