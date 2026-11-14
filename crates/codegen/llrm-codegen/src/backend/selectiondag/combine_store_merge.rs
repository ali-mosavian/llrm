//! `mergeConsecutiveStores`: stores of constants to consecutive addresses on
//! one chain become one store of the joined constant, and stores of loads from
//! consecutive addresses become one wider load and store, of the widest legal
//! integer type that covers a run of them.
//!
//! DEVIATE SD217: only the stores of constants and of loads (no vector element
//! extracts as the source, no vector constant, a truncating store for a type
//! that promotes only where the target opts in
//! (`merge_stores_via_trunc_store`), no rotate of two loads in reverse order),
//! the base of an address is a frame index, a global or one node plus constant
//! offsets (`BaseIndexOffset::match` without the index register), and
//! `allowsMemoryAccess` is true for every scalar the target holds (x86 assumes
//! up to 8 bytes fast), and the argument stores of a constant to a call
//! sequence (chained off `CALLSEQ_START`) are not merged: they are pushes, and
//! a merged `pushd` is longer at m16. Exit: the vectors, the rotate. Owner w10.
//! Review 2026-11-13.
//!
//! # See also
//! LLVM's `DAGCombiner::mergeConsecutiveStores`
//! ([L24277](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L24277)),
//! `getStoreMergeCandidates`
//! ([L23456](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L23456)),
//! `tryStoreMergeOfConstants`
//! ([L23770](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L23770)),
//! `tryStoreMergeOfLoads`
//! ([L23969](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L23969)).

use super::actions::{Action, LoadExt, TargetLowering};
use super::dag::{MemBase, MemInfo, MemLocation, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::lowering::TypeAction;
use super::mvt::{EVT, MVT};

/// The most nodes the candidate search looks at (`MaxSearchNodes`).
const MAX_SEARCH_NODES: usize = 1024;

/// What an address is relative to: `BaseIndexOffset`'s base.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Base {
    Global(u32),
    Frame(i32),
    Node(u32),
}

/// `StoreSource`: what a store writes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Constant,
    /// A load of the stored type, a simple one with no other reader.
    Load {
        base: Base,
    },
}

/// A store `MemOpLink`: the store, and its byte offset from the base.
#[derive(Clone, Copy)]
struct Link {
    store: SDValue,
    offset: i64,
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `BaseIndexOffset::match`: the base of `ptr` and its constant offset.
    pub(super) fn match_address(
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
                // Only consider ORs which act as adds.
                (ISD::Or, _) => {
                    let Some(c) = self.dag.constant(self.dag.operand(at, 1)) else { break };
                    let bits = self.dag.value_type(at).size_in_bits();
                    if !super::known_bits::masked_value_is_zero(self.dag, self.dag.operand(at, 0), c) {
                        break;
                    }
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

    /// `getStoreSource`, for a store of `info`'s type: the constant, or the
    /// load that is the only reader of its value; none for the rest.
    fn store_source(
        &self,
        value: SDValue,
        info: MemInfo,
    ) -> Option<Source> {
        if self.dag.constant(value).is_some() {
            return Some(Source::Constant);
        }
        if self.dag.opcode(value) != ISD::Load || value.res != 0 {
            return None;
        }
        let NodeData::Mem(load) = self.dag.node(value).data else { return None };
        // An extending load is a source too (`getStoreSource` takes any
        // `LOAD`): a promoted `i16` store is a truncating store of one.
        if load.volatile || load.mem_vt != info.mem_vt || load.space != info.space || self.dag.users(value).count() != 1
        {
            return None;
        }
        Some(Source::Load { base: self.match_address(self.dag.operand(value, 1)).0 })
    }

    /// `getStoreMergeCandidates`: the stores that store as many bytes as `st`,
    /// of the same kind of source, to an address of the same base, with their
    /// offsets from `st`'s, and the root they hang from. The root is the chain
    /// of `st`, or the chain of the load that is that chain.
    fn store_merge_candidates(
        &self,
        st: SDValue,
        info: MemInfo,
        source: Source,
    ) -> (SDValue, Vec<Link>) {
        let ptr = self.dag.operand(st, 2);
        let (base, here) = self.match_address(ptr);
        let mut root = self.dag.operand(st, 0);
        // Deviation: the argument stores of a constant in a call sequence
        // become pushes, and a merged one is a `pushd` with an
        // operand-size prefix and a 32-bit immediate (`push 63; push 4`
        // grew by a byte at m16 -Os).
        if self.dag.opcode(root) == ISD::CallSeqStart && source == Source::Constant {
            return (root, Vec::new());
        }
        let through_load = self.dag.opcode(root) == ISD::Load && root.res == 1;
        if through_load {
            root = self.dag.operand(root, 0);
        }
        let mut found = Vec::new();
        let mut consider = |this: &Self, other: SDValue| {
            // A merged store is dead but still on the chain of the load it
            // read.
            if this.dag.opcode(other) != ISD::Store
                || (!this.dag.has_users(other) && other.node != this.dag.get_root().node)
            {
                return;
            }
            let NodeData::Mem(theirs) = this.dag.node(other).data else { return };
            if theirs.volatile
                || theirs.mem_vt != info.mem_vt
                || theirs.space != info.space
                || theirs.ext.is_some()
                || this.store_source(this.dag.operand(other, 1), theirs) != Some(source)
            {
                return;
            }
            let (their_base, there) = this.match_address(this.dag.operand(other, 2));
            if their_base == base {
                found.push(Link { store: other, offset: there - here });
            }
        };
        for user in self.dag.users(root).take(MAX_SEARCH_NODES) {
            let other = SDValue { node: user, res: 0 };
            if self.dag.operand(other, 0) != root {
                continue;
            }
            if through_load && self.dag.opcode(other) == ISD::Load {
                // "Walk down chain": the stores that read this load's chain.
                let chain = SDValue { node: user, res: 1 };
                for store in self.dag.users(chain).take(MAX_SEARCH_NODES) {
                    let store = SDValue { node: store, res: 0 };
                    if self.dag.operand(store, 0) == chain {
                        consider(self, store);
                    }
                }
            } else {
                consider(self, other);
            }
        }
        (root, found)
    }

    /// `checkMergeStoreCandidatesForDependencies`: none of the stores reads,
    /// through its operands, another of them (a merge would be a cycle).
    fn merge_candidates_are_independent(
        &self,
        stores: &[Link],
        root: SDValue,
    ) -> bool {
        let mut visited = crate::support::hash::HashSet::default();
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
        visited: &mut crate::support::hash::HashSet<u32>,
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

    /// `mergeConsecutiveStores`. True when stores
    /// were merged (the originals are then dead).
    pub(super) fn merge_consecutive_stores(
        &mut self,
        st: SDValue,
    ) -> bool {
        if self.opt_none {
            return false;
        }
        let Some(tli) = self.tli else { return false };
        let NodeData::Mem(info) = self.dag.node(st).data else { return false };
        let mem = info.mem_vt;
        // "Always perform this optimization before types are legal. If the
        // target prefers, also try this after legalization".
        if self.level >= CombineLevel::AfterLegalizeTypes && !self.hooks.merge_stores_after_legalization(mem) {
            return false;
        }
        let widest = [8u32, 16, 32, 64]
            .into_iter()
            .filter(|&bits| tli.types.is_type_legal(EVT::integer(bits)))
            .max()
            .unwrap_or(0);
        let widest_vector = if self.hooks.merge_stores_to_vectors() {
            tli.types.legal_vectors().iter().map(|vt| EVT::Simple(*vt).size_in_bits()).max().unwrap_or(0)
        } else {
            0
        };
        if info.volatile
            || !mem.is_integer()
            || mem.size_in_bits() % 8 != 0
            || mem.size_in_bits() * 2 > widest.max(widest_vector)
        {
            return false;
        }
        let Some(source) = self.store_source(self.dag.operand(st, 1), info) else { return false };
        let element = i64::from(mem.size_in_bits() / 8);
        let (root, mut stores) = self.store_merge_candidates(st, info, source);
        if stores.len() < 2 {
            return false;
        }
        stores.sort_by_key(|link| link.offset);
        let mut changed = false;
        while stores.len() > 1 {
            let run = Self::consecutive_stores(&mut stores, element);
            if run == 0 {
                return changed;
            }
            changed |= match source {
                Source::Constant => self.merge_constants(&mut stores, run, mem, root, widest),
                Source::Load { .. } => self.merge_loads(&mut stores, run, mem, root, widest),
            };
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
            let mut last_trunc = false;
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
                    last_trunc = false;
                    last_legal = i + 1;
                } else if self.hooks.merge_stores_via_trunc_store() && self.trunc_store_is_legal(tli, bits) {
                    last_trunc = true;
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
            changed |= self.merge_store_constants(&stores[..last_legal], mem, last_trunc);
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
        truncating: bool,
    ) -> bool {
        let element_bits = mem.size_in_bits();
        let total = element_bits * stores.len() as u32;
        let mask = super::fold::mask(element_bits);
        // Little endian: the lowest address is the lowest bits; big endian
        // the highest.
        let mut joined = 0u128;
        let little = self.hooks.little_endian();
        for at in 0..stores.len() {
            let link = &stores[if little { stores.len() - 1 - at } else { at }];
            joined =
                (joined << element_bits) | (self.dag.constant(self.dag.operand(link.store, 1)).unwrap_or(0) & mask);
        }
        let first = stores[0].store;
        let (chain, info) = self.merged_chain_and_info(stores, total);
        let stored = if truncating { self.promoted(total) } else { EVT::integer(total) };
        let value = self.dag.get_constant(joined, stored);
        let ptr = self.dag.operand(first, 2);
        let new = self.dag.get_store(chain, value, ptr, info);
        // `WorklistInserter`: what the DAG makes is queued as it is made.
        self.add_to_worklist(value.node);
        self.add_to_worklist(new.node);
        for link in stores {
            self.combine_store_to(link.store, new);
        }
        self.add_to_worklist(chain.node);
        true
    }

    /// The type an illegal integer of `bits` is promoted to.
    fn promoted(
        &self,
        bits: u32,
    ) -> EVT {
        self.tli.expect("a merge has a target").types.type_conversion(EVT::integer(bits)).1
    }

    /// `isTruncStoreLegal(promoted, StoreTy)` for a `StoreTy` of `bits` that
    /// the type legalizer promotes.
    fn trunc_store_is_legal(
        &self,
        tli: &TargetLowering,
        bits: u32,
    ) -> bool {
        let store_ty = EVT::integer(bits);
        if tli.types.type_action(store_ty) != TypeAction::PromoteInteger {
            return false;
        }
        match (self.promoted(bits), store_ty) {
            (promoted @ EVT::Simple(wide), EVT::Simple(narrow)) => {
                tli.types.is_type_legal(promoted) && tli.trunc_store_action(wide, narrow) == Action::Legal
            }
            _ => false,
        }
    }

    /// `isLoadLegal(promoted, StoreTy, ZEXTLOAD/SEXTLOAD/EXTLOAD)` for a
    /// `StoreTy` of `bits` that the type legalizer promotes.
    fn ext_loads_are_legal(
        &self,
        tli: &TargetLowering,
        bits: u32,
    ) -> bool {
        match (self.promoted(bits), EVT::integer(bits)) {
            (EVT::Simple(wide), EVT::Simple(narrow)) => {
                [LoadExt::ZExtLoad, LoadExt::SExtLoad, LoadExt::ExtLoad]
                    .into_iter()
                    .all(|ext| tli.load_ext_action(ext, wide, narrow) == Action::Legal)
            }
            _ => false,
        }
    }

    /// `getMergeStoreChains` and the `MemInfo` of the joined store of `total`
    /// bits: the stores' chains, each once, not the stores; the pointer info
    /// of the first when all name one object.
    fn merged_chain_and_info(
        &mut self,
        stores: &[Link],
        total: u32,
    ) -> (SDValue, MemInfo) {
        let NodeData::Mem(first_info) = self.dag.node(stores[0].store).data else { unreachable!("a store") };
        let merged: Vec<u32> = stores.iter().map(|link| link.store.node).collect();
        let mut chains = Vec::new();
        for link in stores {
            let chain = self.dag.operand(link.store, 0);
            if !merged.contains(&chain.node) && !chains.contains(&chain) {
                chains.push(chain);
            }
        }
        let chain = self.dag.get_token_factor(&chains);
        // `hasSameUnderlyingObj`.
        let same_object = match first_info.base {
            MemBase::Value(_) => self.stores_share_underlying_object(stores),
            base => stores.iter().all(|link| {
                matches!(
                    self.dag.node(link.store).data,
                    NodeData::Mem(info) if info.base == base && base != MemBase::Unknown
                )
            }),
        };
        let base = if same_object { self.joined_base(first_info.base, total) } else { MemBase::Unknown };
        (chain, MemInfo { mem_vt: EVT::integer(total), base, ..first_info })
    }

    /// `hasSameUnderlyingObj`: every store names an IR pointer, and all of
    /// them one underlying object.
    fn stores_share_underlying_object(
        &self,
        stores: &[Link],
    ) -> bool {
        let Some(oracle) = self.alias else { return false };
        let mut locations = Vec::with_capacity(stores.len());
        for link in stores {
            let NodeData::Mem(info) = self.dag.node(link.store).data else { return false };
            let MemBase::Value(id) = info.base else { return false };
            locations.push(self.dag.mem_locations[id.get() as usize - 1]);
        }
        oracle.same_underlying_object(&locations)
    }

    /// The pointer info of a joined access of `total` bits that reuses the
    /// first one's: its IR pointer, the joined size, no `!tbaa`
    /// (`AAMDNodes::concat` drops it). Another base is kept as it is.
    fn joined_base(
        &mut self,
        first: MemBase,
        total: u32,
    ) -> MemBase {
        let MemBase::Value(id) = first else { return first };
        let location =
            MemLocation { tbaa: None, bytes: total.div_ceil(8), ..self.dag.mem_locations[id.get() as usize - 1] };
        self.dag.mem_locations.push(location);
        MemBase::Value(std::num::NonZeroU32::new(self.dag.mem_locations.len() as u32).expect("one location pushed"))
    }

    fn stored_load(
        &self,
        link: &Link,
    ) -> SDValue {
        self.dag.operand(link.store, 1)
    }

    fn load_align(
        &self,
        load: SDValue,
    ) -> u32 {
        match self.dag.node(load).data {
            NodeData::Mem(info) => info.align,
            _ => 0,
        }
    }

    /// `EVT::getVectorVT(Context, MemVT, Elts)`: `lanes` cells of `cell`.
    fn vector_of(
        &self,
        cell: EVT,
        lanes: u32,
    ) -> Option<EVT> {
        let EVT::Simple(element) = cell else { return None };
        MVT::vector(element, lanes).map(EVT::Simple)
    }

    /// `TLI.isTypeLegal(Ty) && allowsMemoryAccess(Ty)` for the vector of
    /// `lanes` cells of `cell`: a legal vector type is held in a register
    /// and moved whole, aligned or not (x86 `movups`).
    fn legal_vector_of(
        &self,
        cell: EVT,
        lanes: u32,
    ) -> bool {
        let tli = self.tli.expect("a merge has a target");
        self.hooks.merge_stores_to_vectors()
            && self.vector_of(cell, lanes).is_some_and(|vt| tli.types.is_type_legal(vt))
    }

    /// `tryStoreMergeOfLoads`: the stores of loads from consecutive addresses
    /// off one chain, as many as one legal integer type holds, become a wide
    /// load and a wide store.
    fn merge_loads(
        &mut self,
        stores: &mut Vec<Link>,
        mut consecutive: usize,
        mem: EVT,
        root: SDValue,
        widest: u32,
    ) -> bool {
        let tli = self.tli.expect("a merge has a target");
        let element_bits = mem.size_in_bits();
        let element = i64::from(element_bits / 8);
        let mut changed = false;
        while consecutive >= 2 {
            let mut truncating = false;
            let first_load = self.stored_load(&stores[0]);
            let first_align = self.load_align(first_load);
            let first_store_align = self.store_align(stores[0].store);
            let load_chain = self.dag.operand(first_load, 0);
            let start = self.match_address(self.dag.operand(first_load, 1)).1;
            let (mut last_consecutive, mut last_legal, mut last_vector) = (1, 1, 1);
            for i in 1..consecutive {
                let load = self.stored_load(&stores[i]);
                // "All loads must share the same chain."
                if self.dag.operand(load, 0) != load_chain {
                    break;
                }
                if self.match_address(self.dag.operand(load, 1)).1 - start != element * i as i64 {
                    break;
                }
                last_consecutive = i + 1;
                // "Find a legal type for the vector store."
                if self.legal_vector_of(mem, i as u32 + 1) {
                    last_vector = i + 1;
                }
                let bits = (i as u32 + 1) * element_bits;
                if bits > widest {
                    continue;
                }
                if tli.types.is_type_legal(EVT::integer(bits)) {
                    truncating = false;
                    last_legal = i + 1;
                } else if self.hooks.merge_stores_via_trunc_store()
                    && self.trunc_store_is_legal(tli, bits)
                    && self.ext_loads_are_legal(tli, bits)
                {
                    // "Or check whether a truncstore and extload is legal."
                    truncating = true;
                    last_legal = i + 1;
                }
            }
            // "Only use vector types if the vector type is larger than the
            // integer type. If they are the same, use integers."
            let vector = last_vector > last_legal;
            let count = last_consecutive.min(last_legal.max(last_vector));
            if count < 2 {
                // "Drop as many candidates as we can here."
                let mut skip = 1;
                while skip < consecutive
                    && self.load_align(self.stored_load(&stores[skip])) <= first_align
                    && self.store_align(stores[skip].store) <= first_store_align
                {
                    skip += 1;
                }
                stores.drain(..skip);
                consecutive -= skip;
                continue;
            }
            if !self.merge_candidates_are_independent(&stores[..count], root) {
                stores.drain(..count);
                consecutive -= count;
                continue;
            }
            changed |= self.merge_store_loads(&stores[..count], truncating, vector);
            stores.drain(..count);
            consecutive -= count;
        }
        changed
    }

    /// The joined load and store, the loads' chain readers moved to the new
    /// load.
    fn merge_store_loads(
        &mut self,
        stores: &[Link],
        truncating: bool,
        vector: bool,
    ) -> bool {
        let first_load = self.stored_load(&stores[0]);
        let NodeData::Mem(load_info) = self.dag.node(first_load).data else { return false };
        let total = load_info.mem_vt.size_in_bits() * stores.len() as u32;
        let (chain, store_info) = self.merged_chain_and_info(stores, total);
        // The joined load reuses the first load's pointer info as it is
        // (`FirstLoad->getPointerInfo()`).
        let same_object = matches!(load_info.base, MemBase::Value(_))
            || stores.iter().all(|link| {
                matches!(
                    self.dag.node(self.stored_load(link)).data,
                    NodeData::Mem(info) if info.base == load_info.base && info.base != MemBase::Unknown
                )
            });
        let joint = if vector {
            self.vector_of(load_info.mem_vt, stores.len() as u32).expect("a vector run is of a legal type")
        } else {
            EVT::integer(total)
        };
        let base = if same_object { self.joined_base(load_info.base, total) } else { MemBase::Unknown };
        let wide = MemInfo { mem_vt: joint, ext: None, base, ..load_info };
        let (chain_in, ptr) = (self.dag.operand(first_load, 0), self.dag.operand(first_load, 1));
        // The truncstore/extload case: an `EXTLOAD` into the promoted type
        // and a truncating store of it.
        let new_load = if truncating {
            self.dag.get_ext_load(LoadExt::ExtLoad, self.promoted(total), chain_in, ptr, wide)
        } else {
            self.dag.get_load(joint, chain_in, ptr, wide)
        };
        let new_store = self.dag.get_store(chain, new_load, self.dag.operand(stores[0].store, 2), store_info);
        for link in stores {
            let load = self.stored_load(link);
            self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: new_load.node, res: 1 });
        }
        for link in stores {
            self.replace_value(link.store, new_store);
        }
        self.add_to_worklist(chain.node);
        true
    }
}
