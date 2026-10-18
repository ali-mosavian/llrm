//! `SpillPlacement`: where a split value lives in a register and where on the
//! stack. Translated from llvm/lib/CodeGen/SpillPlacement.cpp and
//! llvm/include/llvm/CodeGen/SpillPlacement.h at d1106deb71cc.
//!
//! # How it works
//! Each edge bundle is a node of a Hopfield network. Blocks that use the value
//! bias the bundles at their borders by their frequency (`PrefReg`,
//! `PrefSpill`, `MustSpill`); blocks it only passes through link their entry
//! and exit bundles. `iterate` updates the nodes to a fixed point, bounded at
//! ten updates per bundle; `finish` leaves the bundles that settled in a
//! register in [`SpillPlacement::active_bundles`].
//!
//! DEVIATE D-SP1: `prepare(BitVector &RegBundles)` keeps a pointer into the
//! caller's bit vector; here the bit set is owned by `SpillPlacement` and read
//! through `active_bundles`. The fact: Rust cannot hold a borrow of the
//! caller's vector across calls. The bits are the same.
//!
//! # Example
//! A loop body that uses the value (`PrefReg` on both borders, frequency 8)
//! inside a preheader that does not (frequency 1): the body's bundle settles
//! in a register, the preheader's stays out.
//!
//! # Used by
//! `SplitKit` and `RAGreedy::tryRegionSplit`.

use std::rc::Rc;

use llrm_mir::blockfrequency::frequency::BlockFrequency;

use super::edgebundles::EdgeBundles;

/// `SpillPlacement::BorderConstraint`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/SpillPlacement.h#L82>).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BorderConstraint {
    /// Block doesn't care / variable not live.
    DontCare,
    /// Block entry/exit prefers a register.
    PrefReg,
    /// Block entry/exit prefers a stack slot.
    PrefSpill,
    /// Block entry prefers both register and stack.
    PrefBoth,
    /// A register is impossible, variable must be spilled.
    MustSpill,
}

/// `SpillPlacement::BlockConstraint`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/SpillPlacement.h#L90>).
#[derive(Clone, Copy, Debug)]
pub struct BlockConstraint {
    /// Basic block number.
    pub number: u32,
    /// Constraint on block entry.
    pub entry: BorderConstraint,
    /// Constraint on block exit.
    pub exit: BorderConstraint,
    pub changes_value: bool,
}

/// `SpillPlacement::Node`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L72>).
#[derive(Clone, Default)]
struct Node {
    /// Sum of blocks that prefer a spill.
    bias_n: BlockFrequency,
    /// Sum of blocks that prefer a register.
    bias_p: BlockFrequency,
    /// -1, 0 or 1; positive means the variable goes in a register through this
    /// bundle.
    value: i32,
    /// (weight, bundle) for all transparent blocks connecting to other bundles.
    links: Vec<(BlockFrequency, u32)>,
    /// Sum of the weights of all links + threshold.
    sum_link_weights: BlockFrequency,
}

impl Node {
    /// `preferReg`: undecided nodes go on the stack.
    fn prefer_reg(&self) -> bool {
        self.value > 0
    }

    /// `mustSpill`.
    fn must_spill(&self) -> bool {
        self.bias_n >= self.bias_p + self.sum_link_weights
    }

    /// `clear`.
    fn clear(
        &mut self,
        threshold: BlockFrequency,
    ) {
        self.bias_n = BlockFrequency(0);
        self.bias_p = BlockFrequency(0);
        self.value = 0;
        self.sum_link_weights = threshold;
        self.links.clear();
    }

    /// `addLink`.
    fn add_link(
        &mut self,
        b: u32,
        w: BlockFrequency,
    ) {
        self.sum_link_weights = self.sum_link_weights + w;
        // There can be multiple links to the same bundle, add them up.
        for l in &mut self.links {
            if l.1 == b {
                l.0 = l.0 + w;
                return;
            }
        }
        // This must be the first link to b.
        self.links.push((w, b));
    }

    /// `addBias`.
    fn add_bias(
        &mut self,
        freq: BlockFrequency,
        direction: BorderConstraint,
    ) {
        match direction {
            BorderConstraint::PrefReg => self.bias_p = self.bias_p + freq,
            BorderConstraint::PrefSpill => self.bias_n = self.bias_n + freq,
            BorderConstraint::MustSpill => self.bias_n = BlockFrequency::max(),
            BorderConstraint::DontCare | BorderConstraint::PrefBoth => {}
        }
    }
}

/// `SparseSet<unsigned>` over bundle numbers: insertion order, no duplicates.
#[derive(Default)]
struct SparseSet {
    dense: Vec<u32>,
    sparse: Vec<u32>,
}

impl SparseSet {
    fn set_universe(
        &mut self,
        n: usize,
    ) {
        self.dense.clear();
        self.sparse.clear();
        self.sparse.resize(n, 0);
    }

    fn insert(
        &mut self,
        n: u32,
    ) {
        let at = self.sparse[n as usize] as usize;
        if at < self.dense.len() && self.dense[at] == n {
            return;
        }
        self.sparse[n as usize] = self.dense.len() as u32;
        self.dense.push(n);
    }

    fn pop_back_val(&mut self) -> Option<u32> {
        self.dense.pop()
    }

    fn clear(&mut self) {
        self.dense.clear();
    }

    fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }
}

/// `BitVector` of the active bundles.
#[derive(Default)]
struct Bits {
    words: Vec<u64>,
}

impl Bits {
    fn reset_all(
        &mut self,
        n: usize,
    ) {
        self.words.clear();
        self.words.resize(n.div_ceil(64), 0);
    }

    fn test(
        &self,
        n: u32,
    ) -> bool {
        self.words[n as usize / 64] >> (n % 64) & 1 != 0
    }

    fn set(
        &mut self,
        n: u32,
    ) {
        self.words[n as usize / 64] |= 1 << (n % 64);
    }

    fn reset(
        &mut self,
        n: u32,
    ) {
        self.words[n as usize / 64] &= !(1 << (n % 64));
    }

    /// `set_bits()`, ascending.
    fn set_bits(&self) -> Vec<u32> {
        let mut out = Vec::new();
        for (w, &word) in self.words.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                out.push(w as u32 * 64 + bits.trailing_zeros());
                bits &= bits - 1;
            }
        }
        out
    }
}

/// `SpillPlacement`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/SpillPlacement.h#L36>).
pub struct SpillPlacement {
    bundles: Rc<EdgeBundles>,
    /// `MBFI->getEntryFreq()`.
    entry_freq: BlockFrequency,
    nodes: Vec<Node>,
    active_nodes: Bits,
    recent_positive: Vec<u32>,
    block_frequencies: Vec<BlockFrequency>,
    threshold: BlockFrequency,
    todo_list: SparseSet,
}

impl SpillPlacement {
    /// `SpillPlacement::run`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L232>):
    /// `block_freqs[n]` is `MBFI->getBlockFreq` of block `n`.
    pub fn new(
        bundles: Rc<EdgeBundles>,
        entry_freq: BlockFrequency,
        block_freqs: Vec<BlockFrequency>,
    ) -> Self {
        let nb = bundles.get_num_bundles() as usize;
        let mut todo_list = SparseSet::default();
        todo_list.set_universe(nb);
        let mut s = Self {
            bundles,
            entry_freq,
            nodes: vec![Node::default(); nb],
            active_nodes: Bits::default(),
            recent_positive: Vec::new(),
            block_frequencies: block_freqs,
            threshold: BlockFrequency(1),
            todo_list,
        };
        s.set_threshold(entry_freq);
        s
    }

    /// `activate`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L253>):
    /// mark node `n` as active if it wasn't already.
    fn activate(
        &mut self,
        n: u32,
    ) {
        self.todo_list.insert(n);
        if self.active_nodes.test(n) {
            return;
        }
        self.active_nodes.set(n);
        self.nodes[n as usize].clear(self.threshold);

        // Very large bundles usually come from big switches, indirect
        // branches, landing pads, or loops with many 'continue' statements.
        // Give a small negative bias to large bundles such that a substantial
        // fraction of the connected blocks need to be interested before we
        // consider expanding the region through the bundle.
        if self.bundles.get_blocks(n).len() > 100 {
            self.nodes[n as usize].bias_p = BlockFrequency(0);
            self.nodes[n as usize].bias_n = BlockFrequency(self.entry_freq.get_frequency() >> 4);
        }
    }

    /// `setThreshold`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L282>).
    /// Since the threshold bounds the open interval (-Threshold; Threshold),
    /// 1 is the minimum.
    fn set_threshold(
        &mut self,
        entry: BlockFrequency,
    ) {
        // 2 is a good threshold when Entry == 2^14; divide by 2^13, rounding.
        let freq = entry.get_frequency();
        let scaled = (freq >> 13) + (freq & (1 << 12) != 0) as u64;
        self.threshold = BlockFrequency(scaled.max(1));
    }

    /// `addConstraints`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L292>).
    pub fn add_constraints(
        &mut self,
        live_blocks: &[BlockConstraint],
    ) {
        for lb in live_blocks {
            let freq = self.block_frequencies[lb.number as usize];
            // Live-in to block?
            if lb.entry != BorderConstraint::DontCare {
                let ib = self.bundles.get_bundle(lb.number, false);
                self.activate(ib);
                self.nodes[ib as usize].add_bias(freq, lb.entry);
            }
            // Live-out from block?
            if lb.exit != BorderConstraint::DontCare {
                let ob = self.bundles.get_bundle(lb.number, true);
                self.activate(ob);
                self.nodes[ob as usize].add_bias(freq, lb.exit);
            }
        }
    }

    /// `addPrefSpill`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L313>).
    pub fn add_pref_spill(
        &mut self,
        blocks: &[u32],
        strong: bool,
    ) {
        for &b in blocks {
            let mut freq = self.block_frequencies[b as usize];
            if strong {
                freq = freq + freq;
            }
            let ib = self.bundles.get_bundle(b, false);
            let ob = self.bundles.get_bundle(b, true);
            self.activate(ib);
            self.activate(ob);
            self.nodes[ib as usize].add_bias(freq, BorderConstraint::PrefSpill);
            self.nodes[ob as usize].add_bias(freq, BorderConstraint::PrefSpill);
        }
    }

    /// `addLinks`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L327>).
    pub fn add_links(
        &mut self,
        links: &[u32],
    ) {
        for &number in links {
            let ib = self.bundles.get_bundle(number, false);
            let ob = self.bundles.get_bundle(number, true);
            // Ignore self-loops.
            if ib == ob {
                continue;
            }
            self.activate(ib);
            self.activate(ob);
            let freq = self.block_frequencies[number as usize];
            self.nodes[ib as usize].add_link(ob, freq);
            self.nodes[ob as usize].add_link(ib, freq);
        }
    }

    /// `scanActiveBundles`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L343>).
    pub fn scan_active_bundles(&mut self) -> bool {
        self.recent_positive.clear();
        for n in self.active_nodes.set_bits() {
            self.update(n);
            // A node that must spill, or a node without any links is not going
            // to change its value ever again, so exclude it from iterations.
            if self.nodes[n as usize].must_spill() {
                continue;
            }
            if self.nodes[n as usize].prefer_reg() {
                self.recent_positive.push(n);
            }
        }
        !self.recent_positive.is_empty()
    }

    /// `update` (SpillPlacement.cpp#L357) with `Node::update`
    /// (SpillPlacement.cpp#L151): recompute the value of `n`; on a change of
    /// preference queue the neighbours that disagree.
    fn update(
        &mut self,
        n: u32,
    ) -> bool {
        let (changed, dissenting) = {
            let node = &self.nodes[n as usize];
            // Compute the weighted sum of inputs.
            let mut sum_n = node.bias_n;
            let mut sum_p = node.bias_p;
            for l in &node.links {
                match self.nodes[l.1 as usize].value {
                    -1 => sum_n = sum_n + l.0,
                    1 => sum_p = sum_p + l.0,
                    _ => {}
                }
            }
            // A dead zone around 0 avoids arbitrary bias when all links are 0
            // and tames rounding errors when the links nominally sum to 0.
            let before = node.prefer_reg();
            let value = if sum_n >= sum_p + self.threshold {
                -1
            } else if sum_p >= sum_n + self.threshold {
                1
            } else {
                0
            };
            (before != (value > 0), value)
        };
        self.nodes[n as usize].value = dissenting;
        if !changed {
            return false;
        }
        // `getDissentingNeighbors`: neighbours that already have the same value
        // are not going to change because of this node changing.
        for i in 0..self.nodes[n as usize].links.len() {
            let m = self.nodes[n as usize].links[i].1;
            if self.nodes[n as usize].value != self.nodes[m as usize].value {
                self.todo_list.insert(m);
            }
        }
        true
    }

    /// `iterate`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L366>):
    /// update the nodes until stability or the maximum number of iterations.
    pub fn iterate(&mut self) {
        // The nodes in the todo list were augmented by `add_constraints`,
        // `add_links` and co.; update the network energy from that frontier.
        self.recent_positive.clear();
        let mut limit = self.bundles.get_num_bundles() * 10;
        while limit > 0 && !self.todo_list.is_empty() {
            limit -= 1;
            let n = self.todo_list.pop_back_val().unwrap();
            if !self.update(n) {
                continue;
            }
            if self.nodes[n as usize].prefer_reg() {
                self.recent_positive.push(n);
            }
        }
    }

    /// `prepare`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L385>).
    pub fn prepare(&mut self) {
        self.recent_positive.clear();
        self.todo_list.clear();
        self.active_nodes.reset_all(self.bundles.get_num_bundles() as usize);
    }

    /// `finish`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SpillPlacement.cpp#L394>):
    /// write the preferences back to the active bundles; true if all of them
    /// prefer a register.
    pub fn finish(&mut self) -> bool {
        let mut perfect = true;
        for n in self.active_nodes.set_bits() {
            if !self.nodes[n as usize].prefer_reg() {
                self.active_nodes.reset(n);
                perfect = false;
            }
        }
        perfect
    }

    /// The bundles `finish` left in a register (D-SP1), ascending.
    pub fn active_bundles(&self) -> Vec<u32> {
        self.active_nodes.set_bits()
    }

    /// `getRecentPositive`.
    pub fn get_recent_positive(&self) -> &[u32] {
        &self.recent_positive
    }

    /// `getBlockFrequency`.
    pub fn get_block_frequency(
        &self,
        number: u32,
    ) -> BlockFrequency {
        self.block_frequencies[number as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REG: BorderConstraint = BorderConstraint::PrefReg;
    const DC: BorderConstraint = BorderConstraint::DontCare;

    fn bc(
        number: u32,
        entry: BorderConstraint,
        exit: BorderConstraint,
    ) -> BlockConstraint {
        BlockConstraint { number, entry, exit, changes_value: false }
    }

    /// 0 -> 1 -> 2 -> 3 with 1 <-> 2 a loop: 0 preheader, 1 header, 2 latch.
    fn chain(freqs: [u64; 4]) -> SpillPlacement {
        let succs = vec![vec![1], vec![2], vec![1, 3], vec![]];
        let b = Rc::new(EdgeBundles::new(&succs));
        SpillPlacement::new(b, BlockFrequency(freqs[0]), freqs.map(BlockFrequency).to_vec())
    }

    #[test]
    fn threshold_scales_with_entry_and_is_at_least_one() {
        let mut s = chain([1 << 14, 1, 1, 1]);
        assert_eq!(s.threshold, BlockFrequency(2));
        s.set_threshold(BlockFrequency(3));
        assert_eq!(s.threshold, BlockFrequency(1));
        s.set_threshold(BlockFrequency((1 << 13) + (1 << 12)));
        assert_eq!(s.threshold, BlockFrequency(2));
    }

    #[test]
    fn the_heavier_side_of_a_shared_bundle_decides() {
        // Blocks 0, 1 and 3 share the bundle of 2's exit.
        let mut s = chain([1 << 14, 1 << 14, 1 << 14, 1 << 20]);
        s.prepare();
        s.add_constraints(&[bc(1, REG, DC), bc(3, BorderConstraint::PrefSpill, DC)]);
        s.iterate();
        let shared = s.bundles.get_bundle(3, false);
        assert_eq!(shared, s.bundles.get_bundle(1, false));
        assert!(!s.finish());
        assert!(!s.active_bundles().contains(&shared));

        let mut s = chain([1 << 14, 1 << 20, 1 << 14, 1 << 14]);
        s.prepare();
        s.add_constraints(&[bc(1, REG, DC), bc(3, BorderConstraint::PrefSpill, DC)]);
        s.iterate();
        assert!(s.finish());
        assert!(s.active_bundles().contains(&shared));
    }

    #[test]
    fn must_spill_overrides_any_register_preference() {
        let mut s = chain([1 << 14, 1 << 14, 1 << 14, 1 << 14]);
        s.prepare();
        s.add_constraints(&[bc(1, REG, REG), bc(2, BorderConstraint::MustSpill, DC)]);
        s.iterate();
        let bundle = s.bundles.get_bundle(2, false);
        assert!(s.nodes[bundle as usize].must_spill());
        s.finish();
        assert!(!s.active_bundles().contains(&bundle));
    }

    #[test]
    fn links_to_the_same_bundle_add_up() {
        let mut n = Node::default();
        n.add_link(3, BlockFrequency(5));
        n.add_link(3, BlockFrequency(7));
        assert_eq!(n.links, vec![(BlockFrequency(12), 3)]);
        assert_eq!(n.sum_link_weights, BlockFrequency(12));
    }

    #[test]
    fn a_transparent_block_carries_the_preference_across() {
        // 0 -> 1 -> 2: block 1 only passes the value through.
        let succs = vec![vec![1], vec![2], vec![]];
        let b = Rc::new(EdgeBundles::new(&succs));
        let f = [1 << 14, 1 << 14, 1 << 14].map(BlockFrequency).to_vec();
        let mut s = SpillPlacement::new(b, f[0], f);
        s.prepare();
        s.add_constraints(&[bc(0, DC, REG), bc(2, REG, DC)]);
        s.add_links(&[1]);
        s.iterate();
        assert!(s.scan_active_bundles());
        assert!(s.finish());
        assert_eq!(s.active_bundles().len(), 2);
    }

    #[test]
    fn a_large_bundle_needs_a_pull_before_it_takes_a_register() {
        // One switch block with 101 successors: its exit bundle has 102 blocks.
        let mut succs = vec![(1..=101).collect::<Vec<usize>>()];
        succs.extend((0..101).map(|_| Vec::new()));
        let b = Rc::new(EdgeBundles::new(&succs));
        let f = vec![BlockFrequency(1 << 14); 102];
        let mut s = SpillPlacement::new(b, f[0], f);
        s.prepare();
        let big = s.bundles.get_bundle(0, true);
        s.activate(big);
        assert_eq!(s.nodes[big as usize].bias_n, BlockFrequency(1 << 10));
        s.add_constraints(&[bc(1, REG, DC)]);
        s.iterate();
        s.finish();
        // One block's pull (2^14) outweighs the 2^10 bias.
        assert!(s.active_bundles().contains(&big));
    }
}
