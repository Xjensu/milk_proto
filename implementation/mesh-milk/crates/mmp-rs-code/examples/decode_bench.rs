//! Host-side Reed--Solomon decode benchmark.
//!
//! This executable measures the current codec on the machine running it.  It
//! is deliberately not a proxy for nRF52/ESP32 flash, RAM, or cycle counts:
//! the OQ-7 target-silicon gate still needs a linked firmware image and a
//! board-level measurement.

use std::env;
use std::hint::black_box;
use std::time::Instant;

use mmp_rs_code::{decode, encode};

const DEFAULT_ITERATIONS: usize = 5;
const SYMBOL_SIZE: usize = 2_048;

struct Profile {
    name: &'static str,
    k: usize,
    n: usize,
}

fn iterations() -> usize {
    env::var("MMP_RS_BENCH_ITERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value: &usize| *value > 0)
        .unwrap_or(DEFAULT_ITERATIONS)
}

fn checksum(symbols: &[Vec<u8>]) -> u64 {
    symbols.iter().flatten().fold(0u64, |acc, byte| {
        acc.wrapping_mul(1_099_511_628_211)
            .wrapping_add(u64::from(*byte))
    })
}

/// Return a stable, non-cryptographic fingerprint of the exact symbols fed to
/// `decode`. Including the symbol index makes a reordered sample visible in
/// benchmark evidence even when the profile dimensions are unchanged.
fn input_fingerprint(received: &[(u8, Vec<u8>)]) -> u64 {
    received.iter().fold(0u64, |acc, (index, symbol)| {
        let with_index = acc
            .wrapping_mul(1_099_511_628_211)
            .wrapping_add(u64::from(*index));
        let with_length = with_index
            .wrapping_mul(1_099_511_628_211)
            .wrapping_add(symbol.len() as u64);
        symbol.iter().fold(with_length, |inner, byte| {
            inner
                .wrapping_mul(1_099_511_628_211)
                .wrapping_add(u64::from(*byte))
        })
    })
}

fn data_symbols(k: usize) -> Vec<Vec<u8>> {
    (0..k)
        .map(|index| {
            (0..SYMBOL_SIZE)
                .map(|offset| {
                    // A deterministic, non-constant pattern exercises every
                    // byte while keeping the run reproducible across hosts.
                    (index as u8)
                        .wrapping_mul(31)
                        .wrapping_add((offset as u8).wrapping_mul(17))
                        .wrapping_add((offset >> 8) as u8)
                })
                .collect()
        })
        .collect()
}

fn run(profile: Profile, iterations: usize) {
    let data = data_symbols(profile.k);
    let expected = checksum(&data);
    let encoded = encode(&data, profile.n);
    // Keep every parity symbol and fill the remaining slots with data after
    // a missing prefix.  File stripes have only eight parity symbols, so a
    // parity-only sample would not contain the `k` symbols required for a
    // decode.  This mixed sample still exercises interpolation for the
    // missing prefix while reflecting the documented `k=32, n=40` profile.
    let parity_count = profile.n - profile.k;
    let mut received: Vec<(u8, Vec<u8>)> = (profile.k..profile.n)
        .map(|index| (index as u8, encoded[index].clone()))
        .collect();
    received.extend((parity_count..profile.k).map(|index| (index as u8, encoded[index].clone())));
    let fingerprint = input_fingerprint(&received);

    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        let recovered = decode(black_box(&received), profile.k)
            .expect("benchmark input must contain k distinct symbols");
        let elapsed = started.elapsed().as_nanos();
        assert_eq!(checksum(&recovered), expected, "decode correctness failure");
        black_box(recovered);
        samples.push(elapsed);
    }

    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    println!(
        "profile={} k={} n={} symbol_bytes={} iterations={} input_fingerprint=0x{fingerprint:016x} min_ns={} median_ns={} max_ns={}",
        profile.name,
        profile.k,
        profile.n,
        SYMBOL_SIZE,
        iterations,
        samples[0],
        median,
        samples[samples.len() - 1]
    );
}

#[cfg(test)]
mod tests {
    use super::input_fingerprint;

    #[test]
    fn input_fingerprint_is_stable_and_order_sensitive() {
        let first = vec![(0, vec![1, 2]), (3, vec![4, 5])];
        let same = vec![(0, vec![1, 2]), (3, vec![4, 5])];
        let reordered = vec![(3, vec![4, 5]), (0, vec![1, 2])];

        assert_eq!(input_fingerprint(&first), input_fingerprint(&same));
        assert_ne!(input_fingerprint(&first), input_fingerprint(&reordered));
    }
}

fn main() {
    let iterations = iterations();
    println!(
        "host_only=true note=timings_are_not_embedded_acceptance_evidence iterations={iterations}"
    );
    run(
        Profile {
            name: "file-stripe",
            k: 32,
            n: 40,
        },
        iterations,
    );
    run(
        Profile {
            name: "max-k",
            k: 64,
            n: 72,
        },
        iterations,
    );
}
