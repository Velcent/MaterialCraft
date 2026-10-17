//! Length-limited Huffman code construction and canonical code assignment.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Optimal-ish code lengths for `freq`, none longer than `limit`. Symbols with zero frequency get length 0.
/// At least two symbols always get a code, so the result is a complete prefix code (which strict decoders such as
/// zlib require); with fewer than two used symbols, unused ones are borrowed.
pub(super) fn code_lengths(freq: &[u32], limit: u8) -> Vec<u8> {
    let n = freq.len();
    let mut lens = vec![0u8; n];
    let mut used: Vec<usize> = (0..n).filter(|&s| freq[s] > 0).collect();
    if used.len() < 2 {
        // Pad with the lowest unused symbols so the code is complete.
        for s in 0..n {
            if used.len() >= 2 {
                break;
            }
            if !used.contains(&s) {
                used.push(s);
            }
        }
        for &s in &used {
            lens[s] = 1;
        }
        return lens;
    }

    // Plain Huffman via a min-heap; nodes 0..used.len() are leaves.
    let leaves = used.len();
    let mut parent = vec![usize::MAX; 2 * leaves - 1];
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = used.iter().enumerate().map(|(i, &s)| Reverse((freq[s] as u64, i))).collect();
    let mut next = leaves;
    while heap.len() > 1 {
        let (Some(Reverse((fa, a))), Some(Reverse((fb, b)))) = (heap.pop(), heap.pop()) else {
            break;
        };
        parent[a] = next;
        parent[b] = next;
        heap.push(Reverse((fa + fb, next)));
        next += 1;
    }
    let root = next - 1;
    let mut depth = vec![0u32; 2 * leaves - 1];
    for node in (0..root).rev() {
        depth[node] = depth[parent[node]] + 1;
    }

    // Histogram of lengths, then push overlong codes down to `limit` while keeping the Kraft sum exact.
    let limit = limit as usize;
    let mut count = vec![0u32; limit + 1];
    for &d in &depth[..leaves] {
        count[(d as usize).min(limit)] += 1;
    }
    let mut kraft: u64 = (1..=limit).map(|l| (count[l] as u64) << (limit - l)).sum();
    while kraft > 1u64 << limit {
        count[limit] -= 1;
        for l in (1..limit).rev() {
            if count[l] > 0 {
                count[l] -= 1;
                count[l + 1] += 2;
                break;
            }
        }
        kraft -= 1;
    }

    // Hand out lengths: the most frequent symbols get the shortest codes.
    used.sort_by_key(|&s| Reverse(freq[s]));
    let mut it = used.into_iter();
    for (l, &c) in count.iter().enumerate().skip(1) {
        for _ in 0..c {
            if let Some(s) = it.next() {
                lens[s] = l as u8;
            }
        }
    }
    lens
}

/// Canonical codes for `lens`, bit-reversed so they can be written LSB-first.
pub(super) fn canonical_codes(lens: &[u8]) -> Vec<u16> {
    let mut count = [0u16; 16];
    for &l in lens {
        count[l as usize] += 1;
    }
    count[0] = 0;
    let mut next = [0u16; 16];
    let mut code = 0u16;
    for bits in 1..16 {
        code = (code + count[bits - 1]) << 1;
        next[bits] = code;
    }
    lens.iter()
        .map(|&l| {
            if l == 0 {
                return 0;
            }
            let c = next[l as usize];
            next[l as usize] += 1;
            c.reverse_bits() >> (16 - l as u32)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kraft(lens: &[u8], limit: u8) -> u64 {
        lens.iter().filter(|&&l| l > 0).map(|&l| 1u64 << (limit - l)).sum()
    }

    #[test]
    fn complete_and_limited() {
        // Fibonacci frequencies produce a maximally deep tree.
        let mut f = vec![1u32, 1];
        while f.len() < 30 {
            let k = f.len();
            f.push(f[k - 1] + f[k - 2]);
        }
        for limit in [7u8, 15] {
            let l = code_lengths(&f, limit);
            assert!(l.iter().all(|&x| x >= 1 && x <= limit));
            assert_eq!(kraft(&l, limit), 1 << limit);
        }
        assert_eq!(code_lengths(&[0, 0, 5, 0], 15), vec![1, 0, 1, 0]);
        assert_eq!(code_lengths(&[0, 0, 0], 15), vec![1, 1, 0]);
        assert_eq!(code_lengths(&[3, 3, 3, 3], 15), vec![2, 2, 2, 2]);
    }

    #[test]
    fn canonical_matches_rfc_example() {
        // RFC 1951 3.2.2: lengths (3,3,3,3,3,2,4,4) -> 010 011 100 101 110 00 1110 1111.
        let codes = canonical_codes(&[3, 3, 3, 3, 3, 2, 4, 4]);
        let expect = [0b010u16, 0b011, 0b100, 0b101, 0b110, 0b00, 0b1110, 0b1111];
        let lens = [3u32, 3, 3, 3, 3, 2, 4, 4];
        for i in 0..8 {
            assert_eq!(codes[i], expect[i].reverse_bits() >> (16 - lens[i]));
        }
    }
}
