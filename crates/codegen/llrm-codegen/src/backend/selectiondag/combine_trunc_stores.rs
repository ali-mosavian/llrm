//! `mergeTruncStores`: stores of the parts of one wide value to adjacent bytes
//! become one store of it, with a `bswap` (bytes in the opposite order) or a
//! rotate (two halves swapped).
//!
//! DEVIATE SD2xx: `allowsMemoryAccess` is true and fast for every width the
//! store merge takes (x86: up to 8 bytes), as `combine_store_merge` has it.
//! Owner: w2. Exit: a target answer, 2026-11-28.
//!
//! # See also
//! LLVM's `DAGCombiner::mergeTruncStores`
//! ([L9976](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L9976)).

use super::combine_store_merge::Base;
use super::dag::{MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `stripTruncAndExt`: `value` through its truncates and extends.
    fn strip_trunc_and_ext(
        &self,
        mut value: SDValue,
    ) -> SDValue {
        while matches!(
            self.dag.opcode(value),
            ISD::Truncate | ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend
        ) {
            value = self.dag.operand(value, 0);
        }
        value
    }

    /// `mergeTruncStores`: the store that replaces `st` and the stores chained
    /// below it, or none.
    pub(super) fn merge_trunc_stores(
        &mut self,
        st: SDValue,
    ) -> Option<SDValue> {
        // The matching looks for `store (trunc x)` patterns that appear early
        // but are likely to be replaced by truncating store nodes later.
        if self.legal_operations() || self.opt_none {
            return None;
        }
        // Simple stores of 1-4 bytes only.
        let NodeData::Mem(info) = self.dag.node(st).data else { return None };
        let mem = info.mem_vt;
        if !(mem == EVT::I8 || mem == EVT::I16 || mem == EVT::I32) || info.volatile || info.ordering.is_some() {
            return None;
        }
        // The stores on the chain, up to the widest (i64).
        let narrow = mem.size_in_bits();
        let max_stores = (64 / narrow) as usize;
        let mut chain = self.dag.operand(st, 0);
        let mut stores = vec![st];
        while self.dag.opcode(chain) == ISD::Store && chain.res == 0 {
            let NodeData::Mem(other) = self.dag.node(chain).data else { return None };
            // All the same size, each the only user of the next's chain.
            if other.mem_vt != mem || other.volatile || other.ordering.is_some() || !self.dag.has_one_use(chain.node) {
                return None;
            }
            stores.push(chain);
            chain = self.dag.operand(chain, 0);
            if max_stores < stores.len() {
                return None;
            }
        }
        if stores.len() < 2 {
            return None;
        }
        let count = stores.len() as u32;
        let wide_bits = count * narrow;
        if wide_bits != 16 && wide_bits != 32 && wide_bits != 64 {
            return None;
        }
        // Where each byte of the source goes.
        let mut source: Option<SDValue> = None;
        let mut offsets = vec![i64::MAX; stores.len()];
        let mut first_offset = i64::MAX;
        let mut first_store: Option<SDValue> = None;
        let mut base: Option<(Base, i64)> = None;
        for &store in &stores {
            let trunc = self.dag.operand(store, 1);
            if self.dag.opcode(trunc) != ISD::Truncate {
                return None;
            }
            let mut offset = 0i64;
            let mut wide = self.dag.operand(trunc, 0);
            if matches!(self.dag.opcode(wide), ISD::Srl | ISD::Sra)
                && let Some(amount) = self.dag.constant(self.dag.operand(wide, 1))
            {
                let amount = amount as u64;
                if amount % u64::from(narrow) != 0
                    || amount > u64::from(self.dag.value_type(wide).size_in_bits() - narrow)
                {
                    return None;
                }
                offset = (amount / u64::from(narrow)) as i64;
                wide = self.dag.operand(wide, 0);
            }
            match source {
                None => source = Some(wide),
                Some(seen) if seen != wide => {
                    if self.strip_trunc_and_ext(seen) != wide && self.strip_trunc_and_ext(wide) != seen {
                        return None;
                    }
                    if self.dag.value_type(wide).size_in_bits() > self.dag.value_type(seen).size_in_bits() {
                        source = Some(wide);
                    }
                    if self.dag.value_type(source.expect("set")).size_in_bits() < wide_bits {
                        return None;
                    }
                }
                Some(_) => {}
            }
            // The same base address.
            let (this_base, this_offset) = self.match_address(self.dag.operand(store, 2));
            let from_base = match base {
                None => {
                    base = Some((this_base, this_offset));
                    0
                }
                Some((first_base, first_at)) if first_base == this_base => this_offset - first_at,
                Some(_) => return None,
            };
            if from_base < first_offset {
                first_store = Some(store);
                first_offset = from_base;
            }
            if offset < 0 || offset >= i64::from(count) || offsets[offset as usize] != i64::MAX {
                return None;
            }
            offsets[offset as usize] = from_base;
        }
        let (mut source, first_store) = (source?, first_store?);
        let wide = EVT::integer(wide_bits);
        // Allowed and fast on the target: DEVIATE.
        let step = i64::from(narrow / 8);
        let little = |offsets: &[i64]| (0..stores.len()).all(|i| offsets[i] == i as i64 * step + first_offset);
        let big = |offsets: &[i64]| {
            let n = stores.len();
            (0..n).all(|i| offsets[n - 1 - i] == i as i64 * step + first_offset)
        };
        let (mut need_bswap, mut need_rotate) = (false, false);
        // The target is little endian.
        if !little(&offsets) {
            if narrow == 8 && big(&offsets) {
                need_bswap = true;
            } else if stores.len() == 2 && big(&offsets) {
                need_rotate = true;
            } else {
                return None;
            }
        }
        if wide != self.dag.value_type(source) {
            source = self.dag.get_node(ISD::Truncate, wide, &[source]);
        }
        if need_bswap {
            source = self.dag.get_node(ISD::BSwap, wide, &[source]);
        } else if need_rotate {
            let amount = self.dag.get_constant(u128::from(wide_bits / 2), wide);
            source = self.dag.get_node(ISD::Rotr, wide, &[source, amount]);
        }
        let NodeData::Mem(first_info) = self.dag.node(first_store).data else { return None };
        let new =
            self.dag.get_store(chain, source, self.dag.operand(first_store, 2), MemInfo { mem_vt: wide, ..first_info });
        // The other stores are left to the combines that remove dead ones.
        self.replace_value(st, new);
        Some(new)
    }
}
