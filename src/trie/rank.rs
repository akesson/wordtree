//! Succinct rank over the per-node "is a word" bitvector, plus the codec for the
//! side `values` table it indexes into.
//!
//! Word nodes are a sparse minority (~28% en), so their `percentile`/`expr_index`
//! are stored once per *word*, in node-position order, instead of one slot per
//! *node*. A word node at position `i` finds its entry at slot `rank(i)` — the
//! number of word nodes strictly before it. `rank` is answered in O(1) from a
//! cumulative index: one count per 64-bit word plus a single masked popcount of
//! the partial word. Everything is stored as raw little-endian `Vec<u8>` so the
//! whole structure stays zero-copy across rkyv's `Vec<u8>`/`ArchivedVec<u8>`.

/// Rank block size in bits: one cumulative `u32` per 64-bit word, so every rank
/// query is a single masked `u64` popcount. The index costs 4 bytes per 8 bytes
/// of bitvector (~0.14 MiB en) — negligible against the structure, and it keeps
/// the per-word-node rank in the autocomplete sweep cheap.
const BLOCK_BITS: usize = 64;
const BLOCK_BYTES: usize = BLOCK_BITS / 8;

/// Bytes for an `n`-bit bitvector, padded to whole `u64` words so the rank query
/// can always read a full 8 bytes.
#[inline]
pub fn word_bits_bytes(n: usize) -> usize {
    n.div_ceil(BLOCK_BITS) * BLOCK_BYTES
}

#[inline]
fn read_u64(bits: &[u8], byte: usize) -> u64 {
    u64::from_le_bytes([
        bits[byte],
        bits[byte + 1],
        bits[byte + 2],
        bits[byte + 3],
        bits[byte + 4],
        bits[byte + 5],
        bits[byte + 6],
        bits[byte + 7],
    ])
}

/// Set bit `i` (caller guarantees `i / 8 < bits.len()`).
#[inline]
pub fn set_bit(bits: &mut [u8], i: usize) {
    bits[i / 8] |= 1 << (i % 8);
}

/// Read bit `i`.
#[inline]
pub fn get_bit(bits: &[u8], i: usize) -> bool {
    (bits[i / 8] >> (i % 8)) & 1 != 0
}

/// Build the cumulative rank index: one little-endian `u32` per 64-bit word,
/// holding the number of set bits strictly *before* that word.
pub fn build_index(word_bits: &[u8], n: usize) -> Vec<u8> {
    let blocks = n.div_ceil(BLOCK_BITS);
    let mut out = Vec::with_capacity(blocks * 4);
    let mut cum: u32 = 0;
    for b in 0..blocks {
        out.extend_from_slice(&cum.to_le_bytes());
        cum += read_u64(word_bits, b * BLOCK_BYTES).count_ones();
    }
    out
}

/// Number of set bits strictly before bit `i` — the `values` slot for a word
/// node at position `i`. One index lookup plus one masked popcount.
#[inline]
pub fn rank(word_bits: &[u8], index: &[u8], i: usize) -> usize {
    let block = i / BLOCK_BITS;
    let o = block * 4;
    let base = u32::from_le_bytes([index[o], index[o + 1], index[o + 2], index[o + 3]]) as usize;
    let bit = i % BLOCK_BITS;
    let mask = (1u64 << bit) - 1; // bit ∈ [0, 63]; bit == 0 → mask 0
    base + (read_u64(word_bits, block * BLOCK_BYTES) & mask).count_ones() as usize
}

/// Bytes per `values` entry: `u16` percentile (LE) followed by `u24` expr_index (LE).
pub const VALUE_BYTES: usize = 5;

/// Largest expr_index that fits the 24-bit slot.
pub const MAX_EXPR_INDEX: u32 = (1 << 24) - 1;

/// Append one word entry. Panics if `expr_index` does not fit 24 bits.
#[inline]
pub fn push_value(values: &mut Vec<u8>, percentile: u16, expr_index: u32) {
    assert!(
        expr_index <= MAX_EXPR_INDEX,
        "expr_index must fit in 24 bits, was {expr_index}"
    );
    values.extend_from_slice(&percentile.to_le_bytes());
    values.extend_from_slice(&expr_index.to_le_bytes()[0..3]);
}

/// Read the `(percentile, expr_index)` entry at `slot`.
#[inline]
pub fn read_value(values: &[u8], slot: usize) -> (u16, u32) {
    let o = slot * VALUE_BYTES;
    let percentile = u16::from_le_bytes([values[o], values[o + 1]]);
    let expr_index = u32::from_le_bytes([values[o + 2], values[o + 3], values[o + 4], 0]);
    (percentile, expr_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rank over an explicit bit pattern matches a naive prefix popcount, across
    /// and within block boundaries.
    #[test]
    fn rank_matches_naive() {
        // 600 bits so we span several 64-bit blocks; set every 7th bit.
        let n = 600;
        let mut bits = vec![0u8; word_bits_bytes(n)];
        let mut set = Vec::new();
        for i in (0..n).step_by(7) {
            set_bit(&mut bits, i);
            set.push(i);
        }
        let index = build_index(&bits, n);

        for i in 0..n {
            let naive = set.iter().filter(|&&s| s < i).count();
            assert_eq!(rank(&bits, &index, i), naive, "rank mismatch at {i}");
            assert_eq!(get_bit(&bits, i), set.contains(&i), "bit mismatch at {i}");
        }
        // Each set bit's slot is its ordinal among set bits.
        for (slot, &i) in set.iter().enumerate() {
            assert_eq!(rank(&bits, &index, i), slot);
        }
    }

    #[test]
    fn value_roundtrip() {
        let mut values = Vec::new();
        push_value(&mut values, 0, 0);
        push_value(&mut values, 1000, MAX_EXPR_INDEX);
        push_value(&mut values, 512, 123_456);
        assert_eq!(read_value(&values, 0), (0, 0));
        assert_eq!(read_value(&values, 1), (1000, MAX_EXPR_INDEX));
        assert_eq!(read_value(&values, 2), (512, 123_456));
    }
}
