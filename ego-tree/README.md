# ego-tree (vendored)

Vec-backed ID-tree, used by the wordtree builder as its intermediate mutable
tree before the compact node array is written out.

This is a vendored fork of [ego-tree](https://github.com/programble/ego-tree)
0.6.2 by Curtis McEnroe, updated to the Rust 2024 edition and extended with a
few helpers the builder needs (`depth_first` / `depth_first_fold` traversal,
`child_ids`, `first_and_last_child`, `next_sibling`, `sort_by_key`, and
by-reference value accessors). It is not published; the original crate lives on
[crates.io](https://crates.io/crates/ego-tree).

Licensed under [ISC](LICENSE), separately from the rest of this repository.
