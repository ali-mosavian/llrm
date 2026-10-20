//! `EdgeBundles`: the equivalence classes of block borders. Translated from
//! llvm/lib/CodeGen/EdgeBundles.cpp at d1106deb71cc, over
//! [`crate::liveness::eqclasses::IntEqClasses`].
//!
//! # How it works
//! Block `n` has an entry node `2n` and an exit node `2n + 1`. A block's exit
//! is joined with the entry of each successor, and the classes are numbered in
//! the order of their lowest member.
//!
//! # Example
//! A diamond `0 -> {1, 2} -> 3` has bundles `{exit 0, entry 1, entry 2}` = 1
//! and `{exit 1, exit 2, entry 3}` = 2 besides entry 0 = 0 and exit 3 = 3.
//!
//! # Used by
//! [`super::spillplacement`].

use crate::liveness::eqclasses::IntEqClasses;

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/EdgeBundles.h#L26>
pub struct EdgeBundles {
    /// `EC`, compressed: node -> bundle.
    ec: IntEqClasses,
    /// `Blocks`: bundle -> block numbers.
    blocks: Vec<Vec<u32>>,
}

impl EdgeBundles {
    /// `EdgeBundles::init`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/EdgeBundles.cpp#L57>);
    /// `succs[n]` are the successors of block `n`.
    pub fn new(succs: &[Vec<usize>]) -> Self {
        let nblocks = succs.len();
        let mut ec = IntEqClasses::default();
        ec.grow(2 * nblocks);
        for (n, ss) in succs.iter().enumerate() {
            let out_e = 2 * n + 1;
            // Join the outgoing bundle with the ingoing bundles of all
            // successors.
            for &s in ss {
                ec.join(out_e as u32, 2 * s as u32);
            }
        }
        ec.compress();
        // Compute the reverse mapping.
        let mut blocks: Vec<Vec<u32>> = vec![Vec::new(); ec.get_num_classes() as usize];
        for i in 0..nblocks as u32 {
            let b0 = ec.get(2 * i);
            let b1 = ec.get(2 * i + 1);
            blocks[b0 as usize].push(i);
            if b1 != b0 {
                blocks[b1 as usize].push(i);
            }
        }
        Self { ec, blocks }
    }

    /// `getBundle`: the ingoing (`out` false) or outgoing bundle of block `n`.
    pub fn get_bundle(
        &self,
        n: u32,
        out: bool,
    ) -> u32 {
        self.ec.get(2 * n + out as u32)
    }

    /// `getNumBundles`.
    pub fn get_num_bundles(&self) -> u32 {
        self.ec.get_num_classes()
    }

    /// `getBlocks`: the blocks connected to `bundle`.
    pub fn get_blocks(
        &self,
        bundle: u32,
    ) -> &[u32] {
        &self.blocks[bundle as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diamond_bundles_are_numbered_by_lowest_member() {
        let e = EdgeBundles::new(&[vec![1, 2], vec![3], vec![3], vec![]]);
        assert_eq!(e.get_num_bundles(), 4);
        assert_eq!(e.get_bundle(0, false), 0);
        let b = e.get_bundle(0, true);
        assert_eq!((e.get_bundle(1, false), e.get_bundle(2, false)), (b, b));
        let c = e.get_bundle(1, true);
        assert_eq!((e.get_bundle(2, true), e.get_bundle(3, false)), (c, c));
        assert_eq!(e.get_blocks(b), &[0, 1, 2]);
        assert_eq!(e.get_blocks(c), &[1, 2, 3]);
    }

    #[test]
    fn a_self_loop_lists_its_block_once() {
        let e = EdgeBundles::new(&[vec![0]]);
        assert_eq!(e.get_num_bundles(), 1);
        assert_eq!(e.get_blocks(0), &[0]);
    }
}
