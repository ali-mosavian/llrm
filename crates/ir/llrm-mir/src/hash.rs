//! The crate's hashed collections on a fixed-key hasher: std's `RandomState`
//! reseeds every run, so a pass's work and any order reaching output would vary
//! (#992). The crate depends on nothing, so this is Fx written out here, not
//! `llrm_support::hash`'s: SipHash, which `DefaultHasher` is, was 1% of a
//! compile of d_faces on its own.

use std::hash::{BuildHasherDefault, Hasher};

/// The multiply-and-rotate hash of rustc's `FxHasher`: no keys, one multiply a
/// word.
#[derive(Clone, Copy, Default)]
pub struct Fx(u64);

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl Fx {
    fn add(
        &mut self,
        word: u64,
    ) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for Fx {
    fn write(
        &mut self,
        bytes: &[u8],
    ) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.add(u64::from_le_bytes(chunk.try_into().expect("eight bytes")));
        }
        for &byte in chunks.remainder() {
            self.add(u64::from(byte));
        }
    }

    fn write_u8(
        &mut self,
        word: u8,
    ) {
        self.add(u64::from(word));
    }

    fn write_u16(
        &mut self,
        word: u16,
    ) {
        self.add(u64::from(word));
    }

    fn write_u32(
        &mut self,
        word: u32,
    ) {
        self.add(u64::from(word));
    }

    fn write_u64(
        &mut self,
        word: u64,
    ) {
        self.add(word);
    }

    fn write_usize(
        &mut self,
        word: usize,
    ) {
        self.add(word as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

#[allow(clippy::disallowed_types)]
pub type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<Fx>>;
#[allow(clippy::disallowed_types)]
pub type HashSet<T> = std::collections::HashSet<T, BuildHasherDefault<Fx>>;
