# Edit distance

Spelling suggestions are found by an **incremental Damerau-Levenshtein distance
carried down the trie**: as the search walks the tree, every node keeps a small
dynamic-programming row recording the edit distance between the query and the
word spelled by the path from the root to that node. This corrects any single
edit — substitution, transposition, insertion or deletion — at edit distance ≤ 1,
and extends to larger distances by widening the stored band (see below).

The implementation is [`dlrow.rs`](dlrow.rs); the trie walk that drives it is
[`../search/distsearch.rs`](../search/distsearch.rs).

## The row

Conceptually, for a query of length `n`, each node corresponds to a row of
`n + 1` values where

```
row[j] = edit_distance(query[0..j], word_spelled_to_this_node)
```

computed from its parent's row (`prev`) and its grandparent's row (`prev_prev`,
needed only for transposition), given the node's character `ch` and the parent's:

```
row[0] = prev[0] + 1                                  // = depth in the trie
for j in 1..=n:
    cost   = if query[j-1] == ch { 0 } else { 1 }
    row[j] = min(prev[j]   + 1,                        // deletion  (word longer)
                 row[j-1]  + 1,                         // insertion (word shorter)
                 prev[j-1] + cost)                      // match / substitution
    if query[j-1] == parent_char and ch == query[j-2]:
        row[j] = min(row[j], prev_prev[j-2] + 1)        // transposition
```

The root's notional parent is the empty word, whose row is the deletion ladder
`[0, 1, 2, …, n]`.

Two quantities drive the search:

- **`row[n]`** is the distance between the *whole* query and this node's word.
  When the node is a complete word and `row[n] ≤ K` (the max edit distance), it is
  a spelling correction.
- **`min(row)`** is the smallest value in the row — the distance to the closest
  *prefix*, and a lower bound on the distance to every word below this node. Once
  `min(row) > K` the entire subtree is pruned, so the walk descends at most `K`
  levels past the query length and touches only a small fraction of the tree.

## The band — what is actually stored

The full row is never materialised. `row[j]` is at least `|j − depth|` (you need
that many indels just to fix the length difference), so any cell with
`|j − depth| > K` is already `> K` and can never be a kept correction nor lower a
surviving `min`. Only the `2K + 1` cells with `|j − depth| ≤ K` — a diagonal
**band** around `j = depth` — are kept, as a fixed `[u8; W]` array whose width
`W = 2K + 1` is a compile-time constant (`W = 3` for the default `K = 1`).

Local index `o ∈ 0..W` maps to query column `j = depth + o − K`; the diagonal
(`j = depth`) sits at `o = K`. A cell whose column falls outside `[0, n]`, or
whose recurrence has no in-band predecessor, holds an out-of-band sentinel
(`OOB`, larger than any kept distance). The band shift turns every neighbour into
a *constant* local offset, so the recurrence carries no per-cell column
arithmetic:

```
cur[o] = min(prev[o+1] + 1,        // deletion      (parent column j)
             prev[o]   + cost,     // match / sub   (parent column j-1)
             cur[o-1]  + 1,        // insertion     (current column j-1)
             pp[o]     + 1)        // transposition (grandparent column j-2)
```

In [`dlrow.rs`](dlrow.rs): `base_band` builds the depth-0 band, `fill_band` writes
each node's band from its parent (`prev`) and grandparent (`pp`), `band_dist`
reads the full-query distance (the column-`n` cell), and `row_min` is the band
minimum that drives pruning.

By Ukkonen's argument the optimal alignment to any cell whose true distance is
`≤ K` stays inside the band, so **every in-range value the search acts on is
computed exactly**, and the kept corrections, their order, and the set of visited
nodes are *bit-identical* to a full-row walk. Out-of-range cells may be
over-estimated, but they stay `> K`, so every keep/prune decision is unchanged.
Banding only changes the per-node cost — `O(K)` instead of `O(n)` — which is why
longer queries gain the most (a 14-char fuzzy query is ~80% faster than the
full-row walk; short typos roughly halve).

## Notes

- The recurrence is the *optimal string alignment* form of Damerau-Levenshtein
  (adjacent transpositions are not re-edited). It is exact for distances up to
  one — the only range the suggester uses — and only differs from unrestricted
  Damerau-Levenshtein at distance two or more.
- The band width `W` is a `const` generic on the searchers (`dist_search` /
  `dist_walk`), pinned by the public API to `BAND` (`= 2·MAX_DIST + 1 = 3`). To
  offer distance-2 suggestions, instantiate the search with a wider band
  (`W = 5`). Stable Rust cannot size `[u8; 2*K + 1]` from a `K` parameter, so the
  width `W` *itself* is the const generic, with `K = (W − 1) / 2` derived as a
  value. (Note that a *usefully* deeper distance also wants corrections ranked by
  edit distance, not by frequency alone — a search-side change, not a band one.)
- A character is one edit regardless of how many bytes it occupies, because the
  query and the trie are compared as `char`s, not bytes.
