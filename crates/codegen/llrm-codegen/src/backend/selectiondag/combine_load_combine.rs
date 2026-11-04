//! `MatchLoadCombine`: a wide value built from narrow loads of adjacent bytes
//! by shifts and ors becomes one load, with a `bswap` where the bytes are in
//! the opposite order.
//!
//! DEVIATE SD2xx: only before legalization, and `allowsMemoryAccess` is true
//! and fast for every width taken (x86: up to 8 bytes), as `mergeTruncStores`
//! has it. The vector loads and `extract_vector_elt` providers are not
//! translated. Owner: w2. Exit: a target answer and vector DAG nodes,
//! 2026-11-28.
//!
//! # See also
//! LLVM's `DAGCombiner::MatchLoadCombine`
//! ([L10198](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L10198))
//! and `calculateByteProvider`
//! ([L9742](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L9742)).

use llrm_sdag::actions::LoadExt;

use super::combine_store_merge::Base;
use super::dag::{MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

/// `SDByteProvider`: where a byte of a value comes from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ByteProvider {
    /// A byte known to be zero.
    Zero,
    /// Byte `offset` of `load`.
    Src { load: SDValue, offset: u32 },
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `calculateByteProvider`: what provides byte `index` of `op`. `mask`
    /// collects the partial masks of the ands passed through, by the byte
    /// `starting` they were asked for.
    fn calculate_byte_provider(
        &self,
        op: SDValue,
        index: u32,
        depth: u32,
        starting: usize,
        mask: &mut [u8],
    ) -> Option<ByteProvider> {
        // The typical i64 by i8 pattern takes recursion up to 8 calls deep.
        if depth == 10 {
            return None;
        }
        // Only a single-use operand is the pattern's own.
        if depth != 0 && !self.dag.has_one_use(op.node) {
            return None;
        }
        let bits = self.dag.value_type(op).size_in_bits();
        if bits % 8 != 0 {
            return None;
        }
        match self.dag.opcode(op) {
            ISD::Or => {
                let lhs = self.calculate_byte_provider(self.dag.operand(op, 0), index, depth + 1, starting, mask)?;
                let rhs = self.calculate_byte_provider(self.dag.operand(op, 1), index, depth + 1, starting, mask)?;
                match (lhs, rhs) {
                    (ByteProvider::Zero, other) | (other, ByteProvider::Zero) => Some(other),
                    _ => None,
                }
            }
            ISD::Shl => {
                let amount = self.dag.constant(self.dag.operand(op, 1))?;
                if amount % 8 != 0 {
                    return None;
                }
                // A shift past the index provides nothing; else the index
                // moves down by the shift.
                let bytes = (amount / 8) as u32;
                if index < bytes {
                    return Some(ByteProvider::Zero);
                }
                self.calculate_byte_provider(
                    self.dag.operand(op, 0),
                    index - bytes,
                    depth + 1,
                    index as usize,
                    mask,
                )
            }
            ISD::AnyExtend | ISD::SignExtend | ISD::ZeroExtend => {
                let narrow = self.dag.operand(op, 0);
                let narrow_bits = self.dag.value_type(narrow).size_in_bits();
                if narrow_bits % 8 != 0 {
                    return None;
                }
                if index >= narrow_bits / 8 {
                    return (self.dag.opcode(op) == ISD::ZeroExtend).then_some(ByteProvider::Zero);
                }
                self.calculate_byte_provider(narrow, index, depth + 1, starting, mask)
            }
            ISD::BSwap => {
                self.calculate_byte_provider(self.dag.operand(op, 0), bits / 8 - index - 1, depth + 1, starting, mask)
            }
            ISD::And => {
                // Constants are canonicalized to the right of an and.
                let byte = (self.dag.constant(self.dag.operand(op, 1))? >> (index * 8)) as u8;
                if byte == 0 {
                    return Some(ByteProvider::Zero);
                }
                let result = self.calculate_byte_provider(self.dag.operand(op, 0), index, depth + 1, starting, mask)?;
                // A zero result may be dropped by the or in favour of its
                // other side, so only a byte that is provided is masked.
                if byte != 0xFF && result != ByteProvider::Zero {
                    mask[starting] &= byte;
                }
                Some(result)
            }
            ISD::Load => {
                let NodeData::Mem(info) = self.dag.node(op).data else { return None };
                if op.res != 0 || info.volatile || info.ordering.is_some() {
                    return None;
                }
                let narrow_bits = info.mem_vt.size_in_bits();
                if narrow_bits % 8 != 0 {
                    return None;
                }
                // A load that does not reach the byte provides it only as a
                // zero extension.
                if index >= narrow_bits / 8 {
                    return (info.ext == Some(LoadExt::ZExtLoad)).then_some(ByteProvider::Zero);
                }
                Some(ByteProvider::Src { load: op, offset: index })
            }
            _ => None,
        }
    }

    /// `MatchLoadCombine`: the load (and `bswap`) that replaces the or `n`, or
    /// none.
    pub(super) fn match_load_combine(
        &mut self,
        n: SDValue,
    ) -> Option<SDValue> {
        if self.legal_operations() {
            return None;
        }
        // Handles simple types only.
        let vt = self.dag.value_type(n);
        if vt != EVT::I16 && vt != EVT::I32 && vt != EVT::I64 {
            return None;
        }
        let width = (vt.size_in_bits() / 8) as usize;
        // The target is little endian: a load's byte `i` is at offset `i`.
        let mut base: Option<(Base, i64)> = None;
        let mut chain: Option<SDValue> = None;
        let mut loads: Vec<SDValue> = Vec::new();
        let mut first: Option<(SDValue, u32)> = None;
        let mut first_offset = i64::MAX;
        let mut offsets = vec![0i64; width];
        let mut masks = vec![0xFFu8; width];
        let mut zero_extended = 0usize;
        // Every byte of the or comes from the same base address.
        for i in (0..width).rev() {
            let provider = self.calculate_byte_provider(n, i as u32, 0, i, &mut masks)?;
            let ByteProvider::Src { load, offset } = provider else {
                // The most significant bytes may be zero: the load is
                // zero-extended.
                zero_extended += 1;
                if zero_extended != width - i {
                    return None;
                }
                continue;
            };
            // All the loads share one chain.
            let load_chain = self.dag.operand(load, 0);
            match chain {
                None => chain = Some(load_chain),
                Some(seen) if seen != load_chain => return None,
                Some(_) => {}
            }
            let (this_base, this_at) = self.match_address(self.dag.operand(load, 1));
            let from_base = match base {
                None => {
                    base = Some((this_base, this_at));
                    0
                }
                Some((first_base, first_at)) if first_base == this_base => this_at - first_at,
                Some(_) => return None,
            };
            let at = from_base + i64::from(offset);
            offsets[i] = at;
            // Remember the first byte load.
            if at < first_offset {
                first = Some((load, offset));
                first_offset = at;
            }
            if !loads.contains(&load) {
                loads.push(load);
            }
        }
        let (first_load, first_byte) = first?;
        let mem_bits = ((width - zero_extended) * 8) as u32;
        let mem_vt = EVT::integer(mem_bits);
        // The bytes match either a big or a little endian load.
        let kept = &offsets[..width - zero_extended];
        let little = kept.iter().enumerate().all(|(i, &at)| at - first_offset == i as i64);
        let big = kept.iter().enumerate().all(|(i, &at)| at - first_offset == (kept.len() - i - 1) as i64);
        let need_bswap = match (kept.len() >= 2, little, big) {
            (true, true, _) => false,
            (true, false, true) => true,
            _ => return None,
        };
        // The first byte is at offset zero of the first load, so the value
        // loads from its address.
        if first_byte != 0 {
            return None;
        }
        let need_zext = zero_extended > 0;
        let NodeData::Mem(info) = self.dag.node(first_load).data else { return None };
        let ptr = self.dag.operand(first_load, 1);
        let chain = chain?;
        let mut new = if need_zext {
            self.dag.get_ext_load(LoadExt::ZExtLoad, vt, chain, ptr, MemInfo { mem_vt, ext: None, ..info })
        } else {
            self.dag.get_load(vt, chain, ptr, MemInfo { mem_vt, ext: None, ..info })
        };
        // The old loads' chain users move to the new load.
        for old in loads {
            self.make_equivalent_memory_ordering(
                SDValue { node: old.node, res: 1 },
                SDValue { node: new.node, res: 1 },
            );
        }
        // Bytes an and kept only partly, in the order of the result.
        let partial = masks.iter().any(|&mask| mask != 0xFF);
        let combined = masks.iter().enumerate().fold(0u128, |all, (i, &mask)| all | u128::from(mask) << (i * 8));
        if need_bswap {
            if need_zext {
                let shift = self.dag.get_constant((zero_extended * 8) as u128, vt);
                new = self.dag.get_node(ISD::Shl, vt, &[new, shift]);
            }
            new = self.dag.get_node(ISD::BSwap, vt, &[new]);
        }
        if partial {
            let all = self.dag.get_constant(combined, vt);
            new = self.dag.get_node(ISD::And, vt, &[new, all]);
        }
        Some(new)
    }
}
