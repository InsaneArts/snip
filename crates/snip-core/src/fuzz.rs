//! A small fuzzer for Snip's parsers (M5: "fuzzing of every parser is
//! clean"): inputs a parser could meet, made from samples of what it reads,
//! bent out of shape, and random bytes. Each parser's tests feed it
//! thousands; none may panic. The same inputs every run (FUZZ_SEED changes
//! them, FUZZ_ITERS how many), so a failure comes back.

use std::panic::{catch_unwind, AssertUnwindSafe};

/// Feed `parse` `n` inputs made from `seeds` (FUZZ_ITERS: as many as it
/// says); a panic fails, naming the input that made it.
pub fn run(seeds: &[&[u8]], n: usize, parse: impl Fn(&[u8])) {
    let n = std::env::var("FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(n);
    let seed = std::env::var("FUZZ_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0x5eed_5eed_5eed_5eed);
    let mut rng = Rng(seed);
    // quiet: the panic is reported below, with its input
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut failed = None;
    for i in 0..n {
        let input = rng.input(seeds);
        if let Err(e) = catch_unwind(AssertUnwindSafe(|| parse(&input))) {
            let why = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            failed = Some((i, input, why));
            break;
        }
    }
    std::panic::set_hook(hook);
    if let Some((i, input, why)) = failed {
        panic!("input {i} panicked ({why}): {} bytes, {:?}…", input.len(), String::from_utf8_lossy(&input[..input.len().min(160)]));
    }
}

/// xorshift64*: small, and the same everywhere.
struct Rng(u64);

/// What breaks text parsers: their punctuation, odd numbers and characters.
const TOKENS: &[&str] = &[
    "{", "}", "[", "]", "\"", ",", ":", "=", "\\", "\n", "\0", "-", "0", "-1", "1e999", "NaN", "null", "true", "18446744073709551616",
    "-9223372036854775809", "é", "\u{ffff}", "#", "\t", " ", "0x", "%", "..", "/",
];

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }

    fn input(&mut self, seeds: &[&[u8]]) -> Vec<u8> {
        // a tenth: random bytes
        if seeds.is_empty() || self.below(10) == 0 {
            return (0..self.below(257)).map(|_| self.next() as u8).collect();
        }
        let mut b = seeds[self.below(seeds.len())].to_vec();
        for _ in 0..1 + self.below(8) {
            self.mutate(&mut b, seeds);
        }
        b
    }

    fn mutate(&mut self, b: &mut Vec<u8>, seeds: &[&[u8]]) {
        let at = self.below(b.len() + 1);
        let last = b.len().saturating_sub(1);
        match self.below(9) {
            0 if !b.is_empty() => b[at.min(last)] ^= 1 << self.below(8),
            1 if !b.is_empty() => b[at.min(last)] = self.next() as u8,
            2 => {
                let bytes: Vec<u8> = (0..1 + self.below(16)).map(|_| self.next() as u8).collect();
                b.splice(at..at, bytes);
            }
            3 => {
                let end = (at + 1 + self.below(32)).min(b.len());
                b.drain(at.min(end)..end);
            }
            4 => {
                let end = (at + 1 + self.below(64)).min(b.len());
                let copy = b[at.min(end)..end].to_vec();
                b.splice(at..at, copy);
            }
            5 => b.truncate(at),
            6 => {
                let other = seeds[self.below(seeds.len())];
                let from = self.below(other.len() + 1);
                let end = (from + self.below(64)).min(other.len());
                b.splice(at..at, other[from..end].iter().copied());
            }
            7 => {
                // a boundary value, little-endian, over what's there
                let v: u64 = [0, u64::MAX, 0x7f, 0x80, 0xff, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff, 0x7fff_ffff_ffff_ffff][self.below(9)];
                let width = [1, 2, 4, 8][self.below(4)];
                for (i, byte) in v.to_le_bytes()[..width].iter().enumerate() {
                    if let Some(x) = b.get_mut(at + i) {
                        *x = *byte;
                    }
                }
            }
            _ => {
                let t = TOKENS[self.below(TOKENS.len())].as_bytes();
                b.splice(at..at, t.iter().copied());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_is_found_and_named() {
        // a parser that panics on a byte it doesn't expect
        let caught = catch_unwind(|| run(&[b"abc"], 10_000, |b| assert!(!b.contains(&b'{'), "a brace")));
        assert!(caught.is_err(), "the fuzzer found the brace");
        // and one that doesn't panic passes
        run(&[b"abc"], 1000, |b| {
            let _ = String::from_utf8_lossy(b).len();
        });
    }
}
