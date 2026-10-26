//! `mergeConsecutiveStores`, the constant half: stores of constants to
//! consecutive addresses on one chain become one store of the joined constant,
//! of the widest legal integer type that covers a run of them.
//!
//! DEVIATE SD217: only the stores of constants (no loads or vector element
//! extracts as the source, no vector store, no truncating store for a type that
//! promotes), the base of an address is a frame index, a global or one node
//! plus constant offsets (`BaseIndexOffset::match` without the index
//! register), and `allowsMemoryAccess` is true for every scalar the target
//! holds (x86 assumes up to 8 bytes fast). Exit: the load source with the
//! alias queries, the vectors. Owner w10. Review 2026-11-13.
//!
//! # See also
//! LLVM's `DAGCombiner::mergeConsecutiveStores`
//! ([L24277](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L24277)),
//! `getStoreMergeCandidates`
//! ([L23456](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L23456)),
//! `tryStoreMergeOfConstants`
//! ([L23770](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L23770)).

use super::dag::{MemBase, MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

/// The most nodes the candidate search looks at (`MaxSearchNodes`).
const MAX_SEARCH_NODES: usize = 1024;

/// What an address is relative to: `BaseIndexOffset`'s base.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Base {
    Global(u32),
    Frame(i32),
    Node(u32),
}

/// A store `MemOpLink`: the store, and its byte offset from the base.
#[derive(Clone, Copy)]
struct Link {
    store: SDValue,
    offset: i64,
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `BaseIndexOffset::match`: the base of `ptr` and its constant offset.
    fn match_address(
        &self,
        ptr: SDValue,
    ) -> (Base, i64) {
        let mut at = ptr;
        let mut offset = 0i64;
        loop {
            match (self.dag.opcode(at), self.dag.node(at).data) {
                (ISD::Add, _) => {
                    let Some(c) = self.dag.constant(self.dag.operand(at, 1)) else { break };
                    let bits = self.dag.value_type(at).size_in_bits();
                    offset = offset.wrapping_add(((c << (128 - bits)) as i128 >> (128 - bits)) as i64);
                    at = self.dag.operand(at, 0);
                }
                (ISD::GlobalAddress, NodeData::Global { symbol, offset: at_offset }) => {
                    return (Base::Global(symbol), offset.wrapping_add(at_offset));
                }
                (ISD::FrameIndex, NodeData::FrameIndex(index)) => return (Base::Frame(index), offset),
                _ => break,
            }
        }
        (Base::Node(at.node), offset)
    }

    /// `getStoreMergeCandidates`, for a constant source: the stores on the
    /// same chain node as `st` that store as many bytes as `st` to an address
    /// of the same base, with their offsets from `st`'s.
    fn store_merge_candidates(
        &self,
        st: SDValue,
        info: MemInfo,
    ) -> Vec<Link> {
        let ptr = self.dag.operand(st, 2);
        let (base, here) = self.match_address(ptr);
        let root = self.dag.operand(st, 0);
        let mut found = Vec::new();
        for user in self.dag.users(root).take(MAX_SEARCH_NODES) {
            let other = SDValue { node: user, res: 0 };
            if self.dag.opcode(other) != ISD::Store || self.dag.operand(other, 0) != root {
                continue;
            }
            let NodeData::Mem(theirs) = self.dag.node(other).data else { continue };
            if theirs.volatile
                || theirs.mem_vt != info.mem_vt
                || theirs.space != info.space
                || theirs.ext.is_some()
                || self.dag.constant(self.dag.operand(other, 1)).is_none()
            {
                continue;
            }
            let (their_base, there) = self.match_address(self.dag.operand(other, 2));
            if their_base == base {
                found.push(Link { store: other, offset: there - here });
            }
        }
        found
    }

    /// `checkMergeStoreCandidatesForDependencies`: none of the stores reads,
    /// through its operands, another of them (a merge would be a cycle).
    fn merge_candidates_are_independent(
        &self,
        stores: &[Link],
        root: SDValue,
    ) -> bool {
        let mut visited = std::collections::HashSet::new();
        let mut worklist = vec![root.node];
        while let Some(node) = worklist.pop() {
            if !visited.insert(node) {
                continue;
            }
            if self.dag.opcode(SDValue { node, res: 0 }) == ISD::TokenFactor {
                worklist.extend(self.dag.node(SDValue { node, res: 0 }).ops.iter().map(|op| op.node));
            }
        }
        let max = MAX_SEARCH_NODES + visited.len();
        for link in stores {
            worklist.extend(self.dag.node(link.store).ops.iter().map(|op| op.node));
        }
        for link in stores {
            if self.has_predecessor(link.store.node, &mut visited, &mut worklist, max) {
                return false;
            }
        }
        true
    }

    /// `SDNode::hasPredecessorHelper`: whether `target` is reached by walking
    /// the operands of `worklist`, `visited` carried between calls.
    fn has_predecessor(
        &self,
        target: u32,
        visited: &mut std::collections::HashSet<u32>,
        worklist: &mut Vec<u32>,
        max: usize,
    ) -> bool {
        if visited.contains(&target) {
            return true;
        }
        while let Some(node) = worklist.pop() {
            if node == target {
                return true;
            }
            if visited.insert(node) {
                worklist.extend(self.dag.node(SDValue { node, res: 0 }).ops.iter().map(|op| op.node));
            }
            if visited.len() >= max {
                return true;
            }
        }
        false
    }

    /// `mergeConsecutiveStores` for stores of constants. True when stores
    /// were merged (the originals are then dead).
    pub(super) fn merge_consecutive_stores(
        &mut self,
        st: SDValue,
    ) -> bool {
        // "Always perform this optimization before types are legal"; no
        // x86 type asks to merge after (`mergeStoresAfterLegalization`).
        if self.level >= CombineLevel::AfterLegalizeTypes {
            return false;
        }
        let Some(tli) = self.tli else { return false };
        let NodeData::Mem(info) = self.dag.node(st).data else { return false };
        let mem = info.mem_vt;
        let widest = [8u32, 16, 32, 64]
            .into_iter()
            .filter(|&bits| tli.types.is_type_legal(EVT::integer(bits)))
            .max()
            .unwrap_or(0);
        if info.volatile
            || !mem.is_integer()
            || mem.size_in_bits() % 8 != 0
            || mem.size_in_bits() * 2 > widest
            || self.dag.constant(self.dag.operand(st, 1)).is_none()
        {
            return false;
        }
        let element = i64::from(mem.size_in_bits() / 8);
        let mut stores = self.store_merge_candidates(st, info);
        if stores.len() < 2 {
            return false;
        }
        stores.sort_by_key(|link| link.offset);
        let root = self.dag.operand(st, 0);
        let mut changed = false;
        while stores.len() > 1 {
            let run = Self::consecutive_stores(&mut stores, element);
            if run == 0 {
                return changed;
            }
            changed |= self.merge_constants(&mut stores, run, mem, root, widest);
        }
        changed
    }

    /// `getConsecutiveStores`: drops the stores before the first run of two
    /// or more at `element` strides, and returns the length of that run.
    fn consecutive_stores(
        stores: &mut Vec<Link>,
        element: i64,
    ) -> usize {
        loop {
            let mut start = 0;
            while start + 1 < stores.len() && stores[start].offset + element != stores[start + 1].offset {
                start += 1;
            }
            if start + 1 >= stores.len() {
                return 0;
            }
            stores.drain(..start);
            let first = stores[0].offset;
            let mut run = 1;
            for (i, link) in stores.iter().enumerate().skip(1) {
                if link.offset - first != element * i as i64 {
                    break;
                }
                run = i + 1;
            }
            if run > 1 {
                return run;
            }
            stores.remove(0);
        }
    }

    /// `tryStoreMergeOfConstants`.
    fn merge_constants(
        &mut self,
        stores: &mut Vec<Link>,
        mut consecutive: usize,
        mem: EVT,
        root: SDValue,
        widest: u32,
    ) -> bool {
        let tli = self.tli.expect("a merge has a target");
        let element_bits = mem.size_in_bits();
        let mut changed = false;
        while consecutive >= 2 {
            let first = stores[0].store;
            let NodeData::Mem(first_info) = self.dag.node(first).data else { return changed };
            let mut last_legal = 1;
            let mut non_zero = false;
            let mut first_zero_after_non_zero = consecutive;
            for i in 0..consecutive {
                let zero = self.dag.constant(self.dag.operand(stores[i].store, 1)) == Some(0);
                if zero && non_zero && first_zero_after_non_zero == consecutive {
                    first_zero_after_non_zero = i;
                }
                non_zero |= !zero;
                let bits = (i as u32 + 1) * element_bits;
                if bits > widest {
                    break;
                }
                if tli.types.is_type_legal(EVT::integer(bits)) {
                    last_legal = i + 1;
                }
            }
            if last_legal < 2 {
                // "Drop as many candidates as we can here."
                let mut skip = 1;
                while skip < consecutive
                    && skip < first_zero_after_non_zero
                    && self.store_align(stores[skip].store) <= first_info.align
                {
                    skip += 1;
                }
                stores.drain(..skip);
                consecutive -= skip;
                continue;
            }
            if !self.merge_candidates_are_independent(&stores[..last_legal], root) {
                stores.drain(..last_legal);
                consecutive -= last_legal;
                continue;
            }
            changed |= self.merge_store_constants(&stores[..last_legal], mem);
            stores.drain(..last_legal);
            consecutive -= last_legal;
        }
        changed
    }

    fn store_align(
        &self,
        store: SDValue,
    ) -> u32 {
        match self.dag.node(store).data {
            NodeData::Mem(info) => info.align,
            _ => 0,
        }
    }

    /// `mergeStoresOfConstantsOrVecElts`, the integer path.
    fn merge_store_constants(
        &mut self,
        stores: &[Link],
        mem: EVT,
    ) -> bool {
        let element_bits = mem.size_in_bits();
        let total = element_bits * stores.len() as u32;
        let mask = super::fold::mask(element_bits);
        // Little endian: the lowest address is the lowest bits.
        let mut joined = 0u128;
        for link in stores.iter().rev() {
            joined =
                (joined << element_bits) | (self.dag.constant(self.dag.operand(link.store, 1)).unwrap_or(0) & mask);
        }
        let first = stores[0].store;
        let NodeData::Mem(first_info) = self.dag.node(first).data else { return false };
        // `getMergeStoreChains`: the stores' chains, each once, not the stores.
        let merged: Vec<u32> = stores.iter().map(|link| link.store.node).collect();
        let mut chains = Vec::new();
        for link in stores {
            let chain = self.dag.operand(link.store, 0);
            if !merged.contains(&chain.node) && !chains.contains(&chain) {
                chains.push(chain);
            }
        }
        let chain = self.dag.get_token_factor(&chains);
        let same_object = stores
            .iter()
            .all(
                |link| match self.dag.node(link.store).data {
                    NodeData::Mem(info) => info.base == first_info.base && info.base != MemBase::Unknown,
                    _ => false,
                },
            );
        let info = MemInfo {
            mem_vt: EVT::integer(total),
            base: if same_object { first_info.base } else { MemBase::Unknown },
            ..first_info
        };
        let value = self.dag.get_constant(joined, EVT::integer(total));
        let ptr = self.dag.operand(first, 2);
        let new = self.dag.get_store(chain, value, ptr, info);
        for link in stores {
            self.replace_value(link.store, new);
        }
        self.add_to_worklist(chain.node);
        true
    }
}
