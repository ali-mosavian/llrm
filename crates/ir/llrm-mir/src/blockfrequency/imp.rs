//! `BlockFrequencyInfoImplBase` and `BlockFrequencyInfoImpl<BT>`, a
//! translation of llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h and
//! llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp at d1106deb71cc: mass flows
//! from the entry through each loop, packaged inner loops first, and the
//! loops' scales multiply it back out into frequencies.
//!
//! Generic over the graph ([`FlowGraph`]) and the edge probabilities
//! ([`EdgeProbabilities`]), LLVM's `BlockT` and `BranchProbabilityInfoT`.
//!
//! DEVIATE 3: `std::list<LoopData>` is an arena with list links: a loop is
//! its index, an iterator into the list is `Option<LoopId>` (`None` the end).
//! DEVIATE 4: no iterative inference (`UseIterativeBFIInference`, off by
//! default and only with profile data), no `print`, no profile counts.

use super::branchprobability::BranchProbability;
use super::frequency::BlockFrequency;
use super::scalednumber::Scaled64;
use crate::cycleinfo::{CycleInfo, CycleRef, FlowGraph};
use crate::hash::HashMap;
use crate::module::BlockId;

/// `BranchProbabilityInfoT::getEdgeProbability(Src, IndexInSuccessors)`.
pub trait EdgeProbabilities {
    fn get_edge_probability(
        &self,
        src: BlockId,
        index: usize,
    ) -> BranchProbability;
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L74>
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct BlockMass(u64);

impl BlockMass {
    fn get_full() -> Self {
        Self(u64::MAX)
    }

    fn is_full(self) -> bool {
        self.0 == u64::MAX
    }

    fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L68>
    fn to_scaled(self) -> Scaled64 {
        if self.is_full() {
            return Scaled64::new(1, 0);
        }
        Scaled64::new(self.0 + 1, -64)
    }

    fn add(
        self,
        x: Self,
    ) -> Self {
        Self(self.0.saturating_add(x.0))
    }

    fn sub(
        self,
        x: Self,
    ) -> Self {
        Self(self.0.saturating_sub(x.0))
    }

    fn mul(
        self,
        p: BranchProbability,
    ) -> Self {
        Self(p.scale(self.0))
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L190>
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub struct BlockNode(pub u32);

impl Default for BlockNode {
    fn default() -> Self {
        Self(u32::MAX)
    }
}

impl BlockNode {
    pub fn is_valid(self) -> bool {
        self.0 < u32::MAX
    }

    fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct FrequencyData {
    scaled: Scaled64,
    integer: u64,
}

type LoopId = usize;

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L227>
#[derive(Clone, Debug, Default)]
struct LoopData {
    parent: Option<LoopId>,
    is_packaged: bool,
    contains_irreducible: bool,
    is_irreducible: bool,
    exits: Vec<(BlockNode, BlockMass)>,
    nodes: Vec<BlockNode>,
    backedge_mass: BlockMass,
    mass: BlockMass,
    scale: Scaled64,
}

impl LoopData {
    fn is_header(
        &self,
        node: BlockNode,
    ) -> bool {
        node == self.nodes[0]
    }

    fn get_header(&self) -> BlockNode {
        self.nodes[0]
    }
}

/// `std::list<LoopData>` (DEVIATE 3).
#[derive(Clone, Debug, Default)]
struct Loops {
    data: Vec<LoopData>,
    next: Vec<Option<LoopId>>,
    prev: Vec<Option<LoopId>>,
    head: Option<LoopId>,
    tail: Option<LoopId>,
}

impl Loops {
    /// `emplace(Insert, ...)`: before `at`, or at the end.
    fn emplace(
        &mut self,
        at: Option<LoopId>,
        one: LoopData,
    ) -> LoopId {
        let id = self.data.len();
        self.data.push(one);
        let before = match at {
            Some(at) => self.prev[at],
            None => self.tail,
        };
        self.next.push(at);
        self.prev.push(before);
        match before {
            Some(before) => self.next[before] = Some(id),
            None => self.head = Some(id),
        }
        match at {
            Some(at) => self.prev[at] = Some(id),
            None => self.tail = Some(id),
        }
        id
    }

    fn in_order(&self) -> Vec<LoopId> {
        let mut out = Vec::with_capacity(self.data.len());
        let mut at = self.head;
        while let Some(one) = at {
            out.push(one);
            at = self.next[one];
        }
        out
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L271>
#[derive(Clone, Copy, Debug)]
struct WorkingData {
    node: BlockNode,
    loop_: Option<LoopId>,
    mass: BlockMass,
}

/// Where `WorkingData::getMass` points: the block's own mass, or a packaged
/// loop's it heads.
#[derive(Clone, Copy)]
enum MassAt {
    Working(usize),
    Loop(LoopId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DistType {
    Local,
    Exit,
    Backedge,
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L347>
#[derive(Clone, Copy, Debug)]
struct Weight {
    kind: DistType,
    target: BlockNode,
    amount: u64,
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L366>
#[derive(Debug, Default)]
struct Distribution {
    weights: Vec<Weight>,
    total: u64,
    did_overflow: bool,
}

impl Distribution {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L128>
    fn add(
        &mut self,
        node: BlockNode,
        amount: u64,
        kind: DistType,
    ) {
        assert!(amount != 0, "invalid weight of 0");
        let (new_total, is_overflow) = self.total.overflowing_add(amount);
        assert!(!(self.did_overflow && is_overflow), "unexpected repeated overflow");
        self.did_overflow |= is_overflow;
        self.total = new_total;
        self.weights.push(Weight { kind, target: node, amount });
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L223>
    fn normalize(&mut self) {
        if self.weights.is_empty() {
            return;
        }
        if self.weights.len() > 1 {
            combine_weights(&mut self.weights);
        }
        if self.weights.len() == 1 {
            self.total = 1;
            self.weights[0].amount = 1;
            return;
        }
        let mut shift = 0;
        if self.did_overflow {
            shift = 33;
        } else if self.total > u32::MAX as u64 {
            shift = 33 - self.total.leading_zeros() as i32;
        }
        if shift == 0 {
            debug_assert_eq!(self.total, self.weights.iter().map(|one| one.amount).sum::<u64>());
            return;
        }
        self.total = 0;
        for w in &mut self.weights {
            w.amount = shift_right_and_round(w.amount, shift).max(1);
            assert!(w.amount <= u32::MAX as u64);
            self.total += w.amount;
        }
        assert!(self.total <= u32::MAX as u64);
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L139>
fn combine_weight(
    w: &mut Weight,
    other: &Weight,
) {
    assert!(other.target.is_valid());
    if w.amount == 0 {
        *w = *other;
        return;
    }
    assert!(w.kind == other.kind);
    assert!(w.target == other.target);
    assert!(other.amount != 0, "Expected non-zero weight");
    w.amount = w.amount.saturating_add(other.amount);
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L157>
fn combine_weights_by_sorting(weights: &mut Vec<Weight>) {
    weights.sort_by_key(|one| one.target);
    let mut out: Vec<Weight> = Vec::with_capacity(weights.len());
    for w in weights.iter() {
        match out.last_mut() {
            Some(last) if last.target == w.target => combine_weight(last, w),
            _ => out.push(*w),
        }
    }
    *weights = out;
}

/// `NextPowerOf2`: the next power of two strictly greater than `n`.
fn next_power_of_2(n: u64) -> u64 {
    1 << (64 - n.leading_zeros())
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L176>
///
/// The order a `DenseMap<uint32_t, Weight>` iterates in, which the result
/// keeps: as many buckets as `DenseMap(NextPowerOf2(2 * N))` reserves, keys
/// hashed `k * 37` and probed linearly, read in bucket order. The reserve
/// leaves it too empty ever to grow.
fn combine_weights_by_hashing(weights: &mut Vec<Weight>) {
    let reserve = next_power_of_2(2 * weights.len() as u64);
    let buckets = next_power_of_2(reserve * 4 / 3 + 1) as usize;
    let mask = buckets - 1;
    let mut table: Vec<Option<Weight>> = vec![None; buckets];
    let mut size = 0;
    for w in weights.iter() {
        let key = w.target.0;
        let mut at = (key.wrapping_mul(37) as usize) & mask;
        loop {
            match &mut table[at] {
                slot @ None => {
                    let mut fresh = Weight { kind: DistType::Local, target: BlockNode::default(), amount: 0 };
                    combine_weight(&mut fresh, w);
                    *slot = Some(fresh);
                    size += 1;
                    break;
                }
                Some(found) if found.target.0 == key => {
                    combine_weight(found, w);
                    break;
                }
                Some(_) => at = (at + 1) & mask,
            }
        }
    }
    if weights.len() == size {
        return;
    }
    *weights = table.into_iter().flatten().collect();
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L197>
fn combine_weights(weights: &mut Vec<Weight>) {
    if weights.len() > 128 {
        combine_weights_by_hashing(weights);
        return;
    }
    combine_weights_by_sorting(weights);
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L208>
fn shift_right_and_round(
    n: u64,
    shift: i32,
) -> u64 {
    assert!((0..64).contains(&shift));
    if shift == 0 {
        return n;
    }
    (n >> shift) + (1 & n >> (shift - 1))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L100>
struct DitheringDistributer {
    rem_weight: u32,
    rem_mass: BlockMass,
}

impl DitheringDistributer {
    fn new(
        dist: &mut Distribution,
        mass: BlockMass,
    ) -> Self {
        dist.normalize();
        Self { rem_weight: dist.total as u32, rem_mass: mass }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L117>
    fn take_mass(
        &mut self,
        weight: u32,
    ) -> BlockMass {
        assert!(weight != 0, "invalid weight");
        assert!(weight <= self.rem_weight);
        let mass = self.rem_mass.mul(BranchProbability::new(weight, self.rem_weight));
        self.rem_weight -= weight;
        self.rem_mass = self.rem_mass.sub(mass);
        mass
    }
}

/// `bfi_detail::IrreducibleGraph`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L608>
struct IrreducibleGraph {
    start: BlockNode,
    start_irr: usize,
    nodes: Vec<(BlockNode, Vec<usize>)>,
    lookup: HashMap<u32, usize>,
}

/// Block frequencies of a graph, as `BlockFrequencyInfoImpl::calculate`
/// leaves them.
#[derive(Clone, Debug, Default)]
pub struct BlockFrequencyInfo {
    freqs: Vec<FrequencyData>,
    is_irr_loop_header: Vec<bool>,
    /// `Nodes`: each block number's node, invalid for a block not reached.
    nodes: Vec<BlockNode>,
}

/// The state of one `calculate`: `BlockFrequencyInfoImplBase`'s working
/// fields and `BlockFrequencyInfoImpl`'s.
struct Impl<'g, G: FlowGraph, P: EdgeProbabilities> {
    graph: &'g G,
    bpi: &'g P,
    ci: &'g CycleInfo,
    rpot: Vec<BlockId>,
    nodes: Vec<BlockNode>,
    freqs: Vec<FrequencyData>,
    is_irr_loop_header: Vec<bool>,
    working: Vec<WorkingData>,
    loops: Loops,
    top_contains_irreducible: bool,
}

impl BlockFrequencyInfo {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1010>
    pub fn calculate<G: FlowGraph, P: EdgeProbabilities>(
        graph: &G,
        bpi: &P,
        ci: &CycleInfo,
    ) -> Self {
        let mut one = Impl {
            graph,
            bpi,
            ci,
            rpot: Vec::new(),
            nodes: Vec::new(),
            freqs: Vec::new(),
            is_irr_loop_header: Vec::new(),
            working: Vec::new(),
            loops: Loops::default(),
            top_contains_irreducible: false,
        };
        if graph.entry().is_none() {
            return Self::default();
        }
        one.initialize_rpot();
        one.initialize_loops();
        one.compute_mass_in_loops();
        one.compute_mass_in_function();
        one.unwrap_loops();
        one.convert_floating_to_integer();
        Self { freqs: one.freqs, is_irr_loop_header: one.is_irr_loop_header, nodes: one.nodes }
    }

    fn get_node(
        &self,
        block: BlockId,
    ) -> BlockNode {
        self.nodes.get(block.0 as usize).copied().unwrap_or_default()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L539>
    pub fn get_block_freq(
        &self,
        block: BlockId,
    ) -> BlockFrequency {
        let node = self.get_node(block);
        if !node.is_valid() {
            return BlockFrequency(0);
        }
        BlockFrequency(self.freqs[node.index()].integer)
    }

    pub fn get_entry_freq(&self) -> BlockFrequency {
        assert!(!self.freqs.is_empty());
        BlockFrequency(self.freqs[0].integer)
    }

    pub fn get_floating_block_freq(
        &self,
        block: BlockId,
    ) -> Scaled64 {
        let node = self.get_node(block);
        if !node.is_valid() {
            return Scaled64::get_zero();
        }
        self.freqs[node.index()].scaled
    }

    pub fn is_irr_loop_header(
        &self,
        block: BlockId,
    ) -> bool {
        let node = self.get_node(block);
        node.is_valid() && self.is_irr_loop_header[node.index()]
    }
}

impl<G: FlowGraph, P: EdgeProbabilities> Impl<'_, G, P> {
    fn get_node(
        &self,
        block: BlockId,
    ) -> BlockNode {
        self.nodes.get(block.0 as usize).copied().unwrap_or_default()
    }

    fn get_block(
        &self,
        node: BlockNode,
    ) -> BlockId {
        self.rpot[node.index()]
    }

    fn loop_(
        &self,
        id: LoopId,
    ) -> &LoopData {
        &self.loops.data[id]
    }

    fn is_loop_header(
        &self,
        index: usize,
    ) -> bool {
        let w = self.working[index];
        w.loop_.is_some_and(|l| self.loop_(l).is_header(w.node))
    }

    fn get_containing_loop(
        &self,
        index: usize,
    ) -> Option<LoopId> {
        let w = self.working[index];
        let mut l = w.loop_;
        while let Some(one) = l
            && self.loop_(one).is_header(w.node)
        {
            l = self.loop_(one).parent;
        }
        l
    }

    fn get_packaged_loop(
        &self,
        index: usize,
    ) -> Option<LoopId> {
        let l = self.working[index].loop_?;
        if !self.loop_(l).is_packaged {
            return None;
        }
        let mut l = l;
        while let Some(parent) = self.loop_(l).parent
            && self.loop_(parent).is_packaged
        {
            l = parent;
        }
        Some(l)
    }

    fn get_resolved_node(
        &self,
        index: usize,
    ) -> BlockNode {
        match self.get_packaged_loop(index) {
            Some(l) => self.loop_(l).get_header(),
            None => self.working[index].node,
        }
    }

    fn mass_at(
        &self,
        index: usize,
    ) -> MassAt {
        let w = self.working[index];
        let mut at = MassAt::Working(index);
        let mut l = w.loop_;
        while let Some(one) = l
            && self.loop_(one).is_packaged
            && self.loop_(one).is_header(w.node)
        {
            at = MassAt::Loop(one);
            l = self.loop_(one).parent;
        }
        at
    }

    fn get_mass(
        &self,
        index: usize,
    ) -> BlockMass {
        match self.mass_at(index) {
            MassAt::Working(i) => self.working[i].mass,
            MassAt::Loop(l) => self.loop_(l).mass,
        }
    }

    fn set_mass(
        &mut self,
        index: usize,
        mass: BlockMass,
    ) {
        match self.mass_at(index) {
            MassAt::Working(i) => self.working[i].mass = mass,
            MassAt::Loop(l) => self.loops.data[l].mass = mass,
        }
    }

    fn is_packaged(
        &self,
        index: usize,
    ) -> bool {
        self.get_resolved_node(index) != self.working[index].node
    }

    fn is_a_package(
        &self,
        index: usize,
    ) -> bool {
        self.is_loop_header(index) && self.loop_(self.working[index].loop_.expect("a header")).is_packaged
    }

    /// `post_order(Entry)`, reversed.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1062>
    fn initialize_rpot(&mut self) {
        let entry = self.graph.entry().expect("an entry");
        let mut visited = vec![false; self.graph.max_number()];
        // (block, its children, the next one to visit).
        let mut stack: Vec<(BlockId, crate::blocklist::BlockList, usize)> = Vec::new();
        visited[entry.0 as usize] = true;
        stack.push((entry, self.graph.children(entry), 0));
        while let Some(top) = stack.last_mut() {
            if top.2 < top.1.len() {
                let child = top.1[top.2];
                top.2 += 1;
                if !visited[child.0 as usize] {
                    visited[child.0 as usize] = true;
                    let children = self.graph.children(child);
                    stack.push((child, children, 0));
                }
            } else {
                self.rpot.push(top.0);
                stack.pop();
            }
        }
        self.rpot.reverse();
        self.nodes = vec![BlockNode::default(); self.graph.max_number()];
        for (idx, block) in self.rpot.iter().enumerate() {
            self.nodes[block.0 as usize] = BlockNode(idx as u32);
        }
        self.working = (0..self.rpot.len())
            .map(|index| WorkingData { node: BlockNode(index as u32), loop_: None, mass: BlockMass::default() })
            .collect();
        self.freqs = vec![FrequencyData::default(); self.rpot.len()];
        self.is_irr_loop_header = vec![false; self.rpot.len()];
    }

    fn has_loop(
        &self,
        c: CycleRef,
    ) -> bool {
        let ci = self.ci;
        if !ci.is_reducible(c) {
            return false;
        }
        let mut a = ci.get_parent_cycle(c);
        while a.is_valid() {
            if !ci.is_reducible(a) && ci.is_entry(a, ci.get_header(c)) {
                return false;
            }
            a = ci.get_parent_cycle(a);
        }
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1087>
    fn initialize_loops(&mut self) {
        let ci = self.ci;
        let mut q: std::collections::VecDeque<(CycleRef, Option<LoopId>)> =
            ci.toplevel_cycles().map(|c| (c, None)).collect();
        if q.is_empty() {
            return;
        }
        while let Some((cycle, mut parent)) = q.pop_front() {
            if self.has_loop(cycle) {
                let header = self.get_node(ci.get_header(cycle));
                let id = self.loops.emplace(None, LoopData { parent, nodes: vec![header], ..LoopData::default() });
                self.working[header.index()].loop_ = Some(id);
                parent = Some(id);
            } else if !ci.is_reducible(cycle) {
                match parent {
                    Some(parent) => self.loops.data[parent].contains_irreducible = true,
                    None => self.top_contains_irreducible = true,
                }
            }
            for c in ci.children(cycle) {
                q.push_back((c, parent));
            }
        }
        for index in 0..self.rpot.len() {
            if self.is_loop_header(index) {
                if let Some(containing) = self.get_containing_loop(index) {
                    self.loops.data[containing].nodes.push(BlockNode(index as u32));
                }
                continue;
            }
            let mut cycle = ci.get_cycle(self.rpot[index]);
            while cycle.is_valid() && !self.has_loop(cycle) {
                cycle = ci.get_parent_cycle(cycle);
            }
            if !cycle.is_valid() {
                continue;
            }
            let header = self.get_node(ci.get_header(cycle));
            assert!(header.is_valid());
            assert!(self.is_loop_header(header.index()));
            let l = self.working[header.index()].loop_.expect("a header");
            self.working[index].loop_ = Some(l);
            self.loops.data[l].nodes.push(BlockNode(index as u32));
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1146>
    fn compute_mass_in_loops(&mut self) {
        let mut at = self.loops.tail;
        while let Some(l) = at {
            if self.loop_(l).contains_irreducible {
                let next = self.loops.next[l];
                self.compute_irreducible_mass(Some(l), next);
            }
            self.compute_mass_in_loop(l);
            at = self.loops.prev[l];
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1157>
    fn compute_mass_in_loop(
        &mut self,
        l: LoopId,
    ) {
        if self.loop_(l).is_irreducible {
            self.solve_irreducible_mass(l);
        } else {
            let header = self.loop_(l).get_header();
            self.set_mass(header.index(), BlockMass::get_full());
            self.propagate_mass_to_successors(Some(l), header);
            let members = self.loop_(l).nodes[1..].to_vec();
            for m in members {
                self.propagate_mass_to_successors(Some(l), m);
            }
        }
        self.compute_loop_scale(l);
        self.package_loop(l);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1176>
    fn get_succ_weights(
        &self,
        node: BlockNode,
        out: &mut Vec<(BlockNode, u64)>,
    ) {
        out.clear();
        if let Some(l) = self.get_packaged_loop(node.index()) {
            for &(target, mass) in &self.loop_(l).exits {
                out.push((self.get_resolved_node(target.index()), mass.0));
            }
            return;
        }
        let bb = self.get_block(node);
        for (index, child) in self.graph.children(bb).iter().enumerate() {
            let succ = self.get_node(*child);
            if !succ.is_valid() {
                continue;
            }
            let w = get_weight_from_branch_prob(self.bpi.get_edge_probability(bb, index)) as u64;
            out.push((self.get_resolved_node(succ.index()), w.max(1)));
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1197>
    fn solve_irreducible_mass(
        &mut self,
        l: LoopId,
    ) {
        let nodes = self.loop_(l).nodes.clone();
        let n = nodes.len();
        let mut p: Vec<(usize, usize, Scaled64)> = Vec::new();
        let mut ex: Vec<(usize, BlockNode, Scaled64)> = Vec::new();
        let mut succs = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            self.get_succ_weights(*node, &mut succs);
            let total: u64 = succs.iter().map(|one| one.1).sum();
            if total == 0 {
                continue;
            }
            let inv_total = Scaled64::get_inverse(total);
            for &(target, weight) in &succs {
                let pr = Scaled64::new(weight, 0) * inv_total;
                match nodes.binary_search(&target) {
                    Ok(at) => p.push((i, at, pr)),
                    Err(_) => ex.push((i, target, pr)),
                }
            }
        }
        let mut f = vec![Scaled64::get_zero(); n];
        let mut g = vec![Scaled64::get_zero(); n];
        let mut pinned = vec![false; n];
        let mut sum = Scaled64::get_zero();
        for i in 0..n {
            if let Some(w) = self.graph.irr_loop_header_weight(self.get_block(nodes[i])) {
                f[i] = Scaled64::new(w, 0);
                pinned[i] = true;
                sum += f[i];
            }
        }
        if sum.is_zero() {
            pinned.fill(false);
            f.fill(Scaled64::get_inverse(n as u64));
            sum = f.iter().fold(Scaled64::get_zero(), |all, one| all + *one);
        }
        const MAX_ITERATIONS: u32 = 16;
        for _ in 0..MAX_ITERATIONS {
            g.fill(Scaled64::get_zero());
            for &(i, j, pr) in &p {
                g[j] += f[i] * pr;
            }
            let mut new = Scaled64::get_zero();
            for i in 0..n {
                if pinned[i] {
                    g[i] = f[i];
                }
                new += g[i];
            }
            if new.is_zero() {
                break;
            }
            let ratio = new / sum;
            let mut delta = Scaled64::get_zero();
            for i in 0..n {
                let was = ratio * f[i];
                delta += if g[i] >= was { g[i] - was } else { was - g[i] };
                f[i] = g[i];
            }
            sum = new;
            if delta < new * Scaled64::new(1, -32) {
                break;
            }
        }
        if !sum.is_zero() {
            for x in &mut f {
                *x = *x / sum;
            }
        }
        for i in 0..n {
            self.set_mass(nodes[i].index(), BlockMass(f[i].scale_u64(u64::MAX)));
        }
        let mut total_exit = BlockMass::default();
        for &(i, succ, pr) in &ex {
            let m = (f[i] * pr).scale_u64(u64::MAX);
            self.loops.data[l].exits.push((succ, BlockMass(m)));
            total_exit = total_exit.add(BlockMass(m));
        }
        self.loops.data[l].backedge_mass = BlockMass::get_full().sub(total_exit);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1270>
    fn compute_mass_in_function(&mut self) {
        if self.top_contains_irreducible {
            let begin = self.loops.head;
            self.compute_irreducible_mass(None, begin);
        }
        assert!(!self.working.is_empty(), "no blocks in function");
        assert!(!self.is_loop_header(0), "entry block is a loop header");
        self.set_mass(0, BlockMass::get_full());
        for i in 0..self.rpot.len() {
            if self.is_packaged(i) {
                continue;
            }
            self.propagate_mass_to_successors(None, BlockNode(i as u32));
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1478>
    fn compute_irreducible_mass(
        &mut self,
        outer: Option<LoopId>,
        insert: Option<LoopId>,
    ) {
        let g = self.irreducible_graph(outer);
        for l in self.analyze_irreducible(&g, outer, insert) {
            self.compute_mass_in_loop(l);
        }
        let Some(outer) = outer else { return };
        assert!(self.loop_(outer).exits.is_empty(), "unexpected exits before distribution");
        assert!(self.loop_(outer).backedge_mass.is_empty(), "unexpected backedge mass before distribution");
        let nodes = std::mem::take(&mut self.loops.data[outer].nodes);
        let mut kept = vec![nodes[0]];
        kept.extend(nodes[1..].iter().copied().filter(|one| !self.is_packaged(one.index())));
        self.loops.data[outer].nodes = kept;
    }

    /// `IrreducibleGraph(BFI, OuterLoop, addBlockEdges)`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L668>
    fn irreducible_graph(
        &self,
        outer: Option<LoopId>,
    ) -> IrreducibleGraph {
        let mut g =
            IrreducibleGraph { start: BlockNode(0), start_irr: 0, nodes: Vec::new(), lookup: HashMap::default() };
        let edges_of: Vec<BlockNode> = match outer {
            Some(outer) => {
                let l = self.loop_(outer);
                g.start = l.get_header();
                for &n in &l.nodes {
                    self.add_node(&mut g, n);
                }
                l.nodes.clone()
            }
            None => {
                for index in 0..self.working.len() {
                    if !self.is_packaged(index) {
                        self.add_node(&mut g, BlockNode(index as u32));
                    }
                }
                (0..self.working.len() as u32).map(BlockNode).collect()
            }
        };
        for (at, one) in g.nodes.iter().enumerate() {
            g.lookup.insert(one.0.0, at);
        }
        for n in edges_of {
            self.add_edges(&mut g, n, outer);
        }
        g.start_irr = g.lookup[&g.start.0];
        g
    }

    fn add_node(
        &self,
        g: &mut IrreducibleGraph,
        node: BlockNode,
    ) {
        g.nodes.push((node, Vec::new()));
        assert!(self.get_mass(node.index()).is_empty(), "mass distributed before the region was packaged");
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L691>
    fn add_edges(
        &self,
        g: &mut IrreducibleGraph,
        node: BlockNode,
        outer: Option<LoopId>,
    ) {
        let Some(&irr) = g.lookup.get(&node.0) else { return };
        if self.is_a_package(node.index()) {
            let l = self.working[node.index()].loop_.expect("a package");
            for &(target, _) in &self.loop_(l).exits {
                self.add_edge(g, irr, target, outer);
            }
        } else {
            let bb = self.rpot[node.index()];
            for succ in self.graph.children(bb).iter() {
                self.add_edge(g, irr, self.get_node(*succ), outer);
            }
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L635>
    fn add_edge(
        &self,
        g: &mut IrreducibleGraph,
        irr: usize,
        succ: BlockNode,
        outer: Option<LoopId>,
    ) {
        if outer.is_some_and(|l| self.loop_(l).is_header(succ)) {
            return;
        }
        let Some(&at) = g.lookup.get(&succ.0) else { return };
        g.nodes[irr].1.push(at);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L697>
    fn analyze_irreducible(
        &mut self,
        g: &IrreducibleGraph,
        outer: Option<LoopId>,
        insert: Option<LoopId>,
    ) -> Vec<LoopId> {
        assert_eq!(outer.is_none(), insert == self.loops.head);
        let mut scc_id = vec![u32::MAX; g.nodes.len()];
        let mut sccs: Vec<Vec<usize>> = Vec::new();
        for (id, scc) in scc_order(g).into_iter().enumerate() {
            for &n in &scc {
                scc_id[n] = id as u32;
            }
            if scc.len() >= 2 {
                sccs.push(scc);
            }
        }
        let mut is_entry = vec![false; g.nodes.len()];
        let mut extra = vec![false; g.nodes.len()];
        for (u, (_, succs)) in g.nodes.iter().enumerate() {
            for &v in succs {
                if scc_id[u] != scc_id[v] {
                    is_entry[v] = true;
                }
            }
        }
        for (u, (u_node, succs)) in g.nodes.iter().enumerate() {
            if is_entry[u] {
                continue;
            }
            for &v in succs {
                if scc_id[v] == scc_id[u] && u_node >= &g.nodes[v].0 {
                    extra[v] = true;
                }
            }
        }
        sccs.iter().map(|scc| self.create_irreducible_loop(g, outer, insert, scc, &is_entry, &extra)).collect()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L662>
    fn create_irreducible_loop(
        &mut self,
        g: &IrreducibleGraph,
        outer: Option<LoopId>,
        insert: Option<LoopId>,
        scc: &[usize],
        is_entry: &[bool],
        extra: &[bool],
    ) -> LoopId {
        let mut members = Vec::with_capacity(scc.len());
        for &i in scc {
            members.push(g.nodes[i].0);
            if is_entry[i] || extra[i] {
                self.is_irr_loop_header[g.nodes[i].0.index()] = true;
            }
        }
        members.sort_unstable();
        let l = self
            .loops
            .emplace(insert, LoopData { parent: outer, is_irreducible: true, nodes: members, ..LoopData::default() });
        for n in self.loop_(l).nodes.clone() {
            if self.is_loop_header(n.index()) {
                let inner = self.working[n.index()].loop_.expect("a header");
                self.loops.data[inner].parent = Some(l);
            } else {
                self.working[n.index()].loop_ = Some(l);
            }
        }
        l
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1510>
    fn propagate_mass_to_successors(
        &mut self,
        outer: Option<LoopId>,
        node: BlockNode,
    ) {
        let mut dist = Distribution::default();
        if let Some(l) = self.get_packaged_loop(node.index()) {
            assert!(Some(l) != outer, "Cannot propagate mass in a packaged loop");
            self.add_loop_successors_to_dist(outer, l, &mut dist);
        } else {
            let bb = self.get_block(node);
            for (index, child) in self.graph.children(bb).iter().enumerate() {
                let weight = get_weight_from_branch_prob(self.bpi.get_edge_probability(bb, index));
                self.add_to_dist(&mut dist, outer, node, self.get_node(*child), weight as u64);
            }
        }
        self.distribute_mass(node, outer, &mut dist);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L313>
    fn add_to_dist(
        &self,
        dist: &mut Distribution,
        outer: Option<LoopId>,
        pred: BlockNode,
        succ: BlockNode,
        mut weight: u64,
    ) {
        if weight == 0 {
            weight = 1;
        }
        let resolved = self.get_resolved_node(succ.index());
        if outer.is_some_and(|l| self.loop_(l).is_header(resolved)) {
            dist.add(resolved, weight, DistType::Backedge);
            return;
        }
        if self.get_containing_loop(resolved.index()) != outer {
            dist.add(resolved, weight, DistType::Exit);
            return;
        }
        assert!(resolved >= pred, "unhandled irreducible control flow");
        dist.add(resolved, weight, DistType::Local);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L362>
    fn add_loop_successors_to_dist(
        &self,
        outer: Option<LoopId>,
        l: LoopId,
        dist: &mut Distribution,
    ) {
        let header = self.loop_(l).get_header();
        for &(target, mass) in &self.loop_(l).exits {
            self.add_to_dist(dist, outer, header, target, mass.0);
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L370>
    fn compute_loop_scale(
        &mut self,
        l: LoopId,
    ) {
        let infinite_loop_scale = Scaled64::new(1, 12);
        let exit_mass = BlockMass::get_full().sub(self.loop_(l).backedge_mass);
        self.loops.data[l].scale =
            if exit_mass.is_empty() { infinite_loop_scale } else { exit_mass.to_scaled().inverse() };
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L402>
    fn package_loop(
        &mut self,
        l: LoopId,
    ) {
        for m in self.loop_(l).nodes.clone() {
            if let Some(inner) = self.get_packaged_loop(m.index()) {
                self.loops.data[inner].exits.clear();
            }
        }
        self.loops.data[l].is_packaged = true;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L427>
    fn distribute_mass(
        &mut self,
        source: BlockNode,
        outer: Option<LoopId>,
        dist: &mut Distribution,
    ) {
        let mass = self.get_mass(source.index());
        let mut d = DitheringDistributer::new(dist, mass);
        for w in &dist.weights {
            let taken = d.take_mass(w.amount as u32);
            if w.kind == DistType::Local {
                let index = w.target.index();
                let now = self.get_mass(index).add(taken);
                self.set_mass(index, now);
                continue;
            }
            let outer = outer.expect("backedge or exit outside of loop");
            if w.kind == DistType::Backedge {
                let now = self.loop_(outer).backedge_mass.add(taken);
                self.loops.data[outer].backedge_mass = now;
                continue;
            }
            self.loops.data[outer].exits.push((w.target, taken));
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L524>
    fn unwrap_loops(&mut self) {
        for index in 0..self.working.len() {
            self.freqs[index].scaled = self.working[index].mass.to_scaled();
        }
        for l in self.loops.in_order() {
            self.unwrap_loop(l);
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L505>
    fn unwrap_loop(
        &mut self,
        l: LoopId,
    ) {
        let combined = self.loop_(l).scale * self.loop_(l).mass.to_scaled();
        self.loops.data[l].scale = combined;
        self.loops.data[l].is_packaged = false;
        for n in self.loop_(l).nodes.clone() {
            let index = n.index();
            if self.is_a_package(index) {
                let inner = self.get_packaged_loop(index).expect("a package");
                let new = combined * self.loop_(inner).scale;
                self.loops.data[inner].scale = new;
            } else {
                self.freqs[index].scaled = combined * self.freqs[index].scaled;
            }
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BlockFrequencyInfoImpl.cpp#L478>
    fn convert_floating_to_integer(&mut self) {
        let mut max = Scaled64::get_zero();
        for f in &self.freqs {
            if max < f.scaled {
                max = f.scaled;
            }
        }
        const MAX_BITS: i16 = 64;
        const SLACK: i16 = 10;
        let scaling_factor = Scaled64::new(1, MAX_BITS - SLACK) / max;
        for f in &mut self.freqs {
            let scaled = f.scaled * scaling_factor;
            f.integer = scaled.to_int().max(1);
        }
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/BlockFrequencyInfoImpl.h#L1506>
fn get_weight_from_branch_prob(prob: BranchProbability) -> u32 {
    prob.get_numerator()
}

/// `scc_iterator` over an `IrreducibleGraph` from its start: Tarjan's
/// algorithm, each SCC in the order the iterator yields it, its nodes in the
/// order it pops them.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/SCCIterator.h#L46>
fn scc_order(g: &IrreducibleGraph) -> Vec<Vec<usize>> {
    let mut visit_num = 0u32;
    let mut numbers: Vec<Option<u32>> = vec![None; g.nodes.len()];
    let mut scc_node_stack: Vec<usize> = Vec::new();
    // (node, next child, MinVisited).
    let mut visit_stack: Vec<(usize, usize, u32)> = Vec::new();
    let mut out = Vec::new();
    let mut visit_one = |n: usize, numbers: &mut Vec<Option<u32>>, scc: &mut Vec<usize>, stack: &mut Vec<_>| {
        visit_num += 1;
        numbers[n] = Some(visit_num);
        scc.push(n);
        stack.push((n, 0, visit_num));
    };
    visit_one(g.start_irr, &mut numbers, &mut scc_node_stack, &mut visit_stack);
    // GetNextSCC, once for each SCC.
    while !visit_stack.is_empty() {
        // DFSVisitChildren.
        while let Some(&(node, next, _)) = visit_stack.last()
            && next < g.nodes[node].1.len()
        {
            visit_stack.last_mut().expect("a frame").1 += 1;
            let child = g.nodes[node].1[next];
            match numbers[child] {
                None => visit_one(child, &mut numbers, &mut scc_node_stack, &mut visit_stack),
                Some(child_num) => {
                    let top = visit_stack.last_mut().expect("a frame");
                    if top.2 > child_num {
                        top.2 = child_num;
                    }
                }
            }
        }
        let (visiting, _, min_visit_num) = visit_stack.pop().expect("a frame");
        if let Some(top) = visit_stack.last_mut()
            && top.2 > min_visit_num
        {
            top.2 = min_visit_num;
        }
        if Some(min_visit_num) != numbers[visiting] {
            continue;
        }
        let mut scc = Vec::new();
        loop {
            let n = scc_node_stack.pop().expect("a node");
            numbers[n] = Some(u32::MAX);
            scc.push(n);
            if n == visiting {
                break;
            }
        }
        out.push(scc);
    }
    out
}
