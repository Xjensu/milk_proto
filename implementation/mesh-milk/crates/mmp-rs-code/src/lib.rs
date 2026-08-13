//! mmp-rs-code — `no_std` GF(256) Reed-Solomon, k ≤ 64, table-driven
//! (doc 08 §8, OQ-7 bench target).
//!
//! Construction: evaluation code over GF(256) with the polynomial defined by
//! interpolating the k data symbols at points 0..k. Symbols 0..k therefore
//! equal the data verbatim (systematic — zero coding cost on the common
//! lossless path); any k received (index, symbol) pairs reconstruct the block
//! by Lagrange interpolation.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

pub const MAX_K: usize = 64;
pub const MAX_N: usize = 255;

/// GF(256) with the AES polynomial 0x11B, generator 3.
const fn build_tables() -> ([u8; 256], [u8; 256]) {
    let mut exp = [0u8; 256];
    let mut log = [0u8; 256];
    let mut x: u16 = 1;
    let mut i = 0;
    while i < 255 {
        exp[i] = x as u8;
        log[x as usize] = i as u8;
        // multiply by generator 3: x*2 ^ x
        let x2 = (x << 1) ^ (if x & 0x80 != 0 { 0x11B } else { 0 });
        x = (x2 ^ x) & 0xFF;
        i += 1;
    }
    exp[255] = exp[0];
    (exp, log)
}

const TABLES: ([u8; 256], [u8; 256]) = build_tables();

#[inline]
fn gmul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let (exp, log) = (&TABLES.0, &TABLES.1);
    let s = log[a as usize] as usize + log[b as usize] as usize;
    exp[s % 255]
}

#[inline]
fn ginv(a: u8) -> u8 {
    debug_assert!(a != 0);
    let (exp, log) = (&TABLES.0, &TABLES.1);
    exp[(255 - log[a as usize] as usize) % 255]
}

/// Lagrange basis weight `ℓ_j(at)` for interpolation points `xs`.
fn lagrange_weight(xs: &[u8], j: usize, at: u8) -> u8 {
    let mut num = 1u8;
    let mut den = 1u8;
    for (m, &xm) in xs.iter().enumerate() {
        if m == j {
            continue;
        }
        num = gmul(num, at ^ xm);
        den = gmul(den, xs[j] ^ xm);
    }
    gmul(num, ginv(den))
}

/// Encode `k = data.len()` equal-length data symbols into `n` symbols.
/// Symbols `0..k` are the data itself. Panics on violated caps (caller bug).
pub fn encode(data: &[Vec<u8>], n: usize) -> Vec<Vec<u8>> {
    let k = data.len();
    assert!((1..=MAX_K).contains(&k) && n >= k && n <= MAX_N);
    let sym_len = data[0].len();
    assert!(data.iter().all(|s| s.len() == sym_len));

    let xs: Vec<u8> = (0..k as u8).collect();
    let mut out: Vec<Vec<u8>> = data.to_vec();
    for idx in k..n {
        let at = idx as u8;
        // p(at) = Σ_j data_j · ℓ_j(at) — weights computed once per symbol.
        let weights: Vec<u8> = (0..k).map(|j| lagrange_weight(&xs, j, at)).collect();
        let mut sym = vec![0u8; sym_len];
        for (j, w) in weights.iter().enumerate() {
            if *w == 0 {
                continue;
            }
            for (o, d) in sym.iter_mut().zip(data[j].iter()) {
                *o ^= gmul(*w, *d);
            }
        }
        out.push(sym);
    }
    out
}

/// Reconstruct the k data symbols from any k received `(index, symbol)` pairs.
/// Returns `None` on out-of-range/duplicate indices or length mismatch. Symbol
/// indices are the zero-based positions in an encoded block, so the largest
/// valid index is `MAX_N - 1` (with `n <= MAX_N` in [`encode`]).
pub fn decode(received: &[(u8, Vec<u8>)], k: usize) -> Option<Vec<Vec<u8>>> {
    if received.len() < k || k == 0 || k > MAX_K {
        return None;
    }
    let take = &received[..k];
    let sym_len = take[0].1.len();
    let xs: Vec<u8> = take.iter().map(|(i, _)| *i).collect();
    for (i, x) in xs.iter().enumerate() {
        if (*x as usize) >= MAX_N || take[i].1.len() != sym_len || xs[..i].contains(x) {
            return None;
        }
    }
    // data_i = p(i) = Σ_j received_j · ℓ_j(i) over the received points.
    let mut out = Vec::with_capacity(k);
    for target in 0..k as u8 {
        if let Some(pos) = xs.iter().position(|&x| x == target) {
            out.push(take[pos].1.clone()); // systematic fast path
            continue;
        }
        let weights: Vec<u8> = (0..k).map(|j| lagrange_weight(&xs, j, target)).collect();
        let mut sym = vec![0u8; sym_len];
        for (j, w) in weights.iter().enumerate() {
            if *w == 0 {
                continue;
            }
            for (o, r) in sym.iter_mut().zip(take[j].1.iter()) {
                *o ^= gmul(*w, *r);
            }
        }
        out.push(sym);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systematic_and_recovers_from_any_k() {
        let data: Vec<Vec<u8>> = (0..8u8).map(|i| vec![i.wrapping_mul(7); 64]).collect();
        let symbols = encode(&data, 12);
        assert_eq!(&symbols[..8], &data[..]); // systematic

        // Drop symbols 0,3,5,7 — recover from a mixed set incl. parity.
        let received: Vec<(u8, Vec<u8>)> = [1u8, 2, 4, 6, 8, 9, 10, 11]
            .iter()
            .map(|&i| (i, symbols[i as usize].clone()))
            .collect();
        let recovered = decode(&received, 8).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn duplicate_index_rejected() {
        let data: Vec<Vec<u8>> = (0..4u8).map(|i| vec![i; 8]).collect();
        let symbols = encode(&data, 6);
        let received = vec![
            (0u8, symbols[0].clone()),
            (0u8, symbols[0].clone()),
            (1u8, symbols[1].clone()),
            (2u8, symbols[2].clone()),
        ];
        assert!(decode(&received, 4).is_none());
    }

    #[test]
    fn corrupted_parity_yields_wrong_data_not_panic() {
        // RS here is erasure-coding; corruption detection is the Merkle layer's
        // job (doc 08 §8). Verify we degrade without panicking.
        let data: Vec<Vec<u8>> = (0..4u8).map(|i| vec![i + 1; 16]).collect();
        let mut symbols = encode(&data, 8);
        symbols[5][0] ^= 0xff;
        let received: Vec<(u8, Vec<u8>)> = [4u8, 5, 6, 7]
            .iter()
            .map(|&i| (i, symbols[i as usize].clone()))
            .collect();
        let recovered = decode(&received, 4).unwrap();
        assert_ne!(recovered, data);
    }

    #[test]
    fn max_k_recovers_from_parity_only() {
        let data: Vec<Vec<u8>> = (0..MAX_K as u8).map(|i| vec![i; 8]).collect();
        let symbols = encode(&data, MAX_K * 2);
        let received: Vec<(u8, Vec<u8>)> = (MAX_K..MAX_K * 2)
            .map(|i| (i as u8, symbols[i].clone()))
            .collect();
        assert_eq!(decode(&received, MAX_K), Some(data));
    }

    #[test]
    fn index_at_max_n_is_rejected() {
        let data = vec![vec![1u8; 4], vec![2u8; 4]];
        let symbols = encode(&data, 3);
        let received = vec![(0u8, symbols[0].clone()), (MAX_N as u8, symbols[1].clone())];
        assert!(decode(&received, 2).is_none());
    }
}
