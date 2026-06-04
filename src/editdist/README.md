# Edit distance

Spelling suggestions are found by an **incremental Damerau-Levenshtein distance
carried down the trie**: as the search walks the tree, every node keeps one
dynamic-programming row recording the edit distance between the query and the
word spelled by the path from the root to that node. This corrects any single
edit — substitution, transposition, insertion or deletion — at edit distance ≤ 1,
and extends to larger distances by raising a single constant.

The implementation is [`dlrow.rs`](dlrow.rs); the trie walk that drives it is
[`../search/distsearch.rs`](../search/distsearch.rs).

## The row

For a query of length `n`, each node stores a row of `n + 1` values where

```
row[j] = edit_distance(query[0..j], word_spelled_to_this_node)
```

The row for a node is computed from its parent's row (`prev`) and its
grandparent's row (`prev_prev`, needed only for transposition), given the node's
character `ch` and the parent's character:

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
  When the node is a complete word and `row[n] ≤ MAX_DIST`, it is a spelling
  correction.
- **`min(row)`** is the smallest value in the row — the distance to the closest
  *prefix*, and a lower bound on the distance to every word below this node. Once
  `min(row) > MAX_DIST` the entire subtree is pruned, so the walk descends at most
  `MAX_DIST` levels past the query length and touches only a small fraction of the
  tree.

## Notes

- The recurrence is the *optimal string alignment* form of Damerau-Levenshtein
  (adjacent transpositions are not themselves re-edited). It is exact for
  distances up to one — the only range the suggester uses — and only differs from
  unrestricted Damerau-Levenshtein at distance two or more.
- `MAX_DIST` (in [`dlrow.rs`](dlrow.rs)) is the single knob: set it to `2` (and,
  if desired, restrict the row to a band around the diagonal) to offer
  distance-2 suggestions.
- A character is one edit regardless of how many bytes it occupies, because the
  query and the trie are compared as `char`s, not bytes.
