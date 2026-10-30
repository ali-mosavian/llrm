//! `reduceLoadWidth` for a constant shift or mask of a load: `srl (load p), 8`
//! read through an `and 255` is a byte load at `p + 1`, `and (load p), 255` a
//! byte `zextload`.
//!
//! DEVIATE SD249: only the `srl`, `sra` and `and` openings (the `truncate`
//! one is [`DagCombiner::reduce_load_width`] in `dag_combiner.rs`); no
//! look-through `freeze`, no `shl` left shift, and `isLegalNarrowLdSt` takes
//! every x86 access as allowed (unaligned loads are legal there). Owner
//! sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::reduceLoadWidth`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L17095)).

use super::actions::{Action, LoadExt};
use super::dag::{MemBase, MemInfo, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

/// `APInt::isMask`: a run of ones from bit 0.
fn is_mask(value: u128) -> bool {
    value != 0 && value & (value + 1) == 0
}

/// `APInt::isShiftedMask`: one run of ones; its first bit and length.
fn shifted_mask(value: u128) -> Option<(u32, u32)> {
    let first = value.trailing_zeros();
    (value != 0 && is_mask(value >> first)).then(|| (first, (value >> first).count_ones()))
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn reduce_load_width_of(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let vt = self.dag.value_type(node);
        if !vt.is_integer() || vt.size_in_bits() > 64 {
            return None;
        }
        let mut n0 = self.dag.operand(node, 0);
        // Look through a freeze between the `and` and the load; it is put back on the narrowed
        // load. DEVIATE: only a freeze with one use. Exit: x86 judged.
        let mut frozen = None;
        if op == ISD::And
            && self.hooks.reduce_load_width_through_freeze()
            && self.dag.opcode(n0) == ISD::Freeze
            && self.dag.has_one_use(n0.node)
        {
            frozen = Some(n0);
            n0 = self.dag.operand(n0, 0);
        }
        let mut ext_type: Option<LoadExt>;
        let mut ext_vt: EVT;
        let mut shift = 0u32;
        let mut shifted_offset = 0u32;
        match op {
            ISD::Srl | ISD::Sra => {
                let (load, amount) = (n0, self.dag.constant(self.dag.operand(node, 1))?);
                let info = self.load_info(load)?;
                let width = info.mem_vt.size_in_bits();
                if u128::from(width) <= amount {
                    return None;
                }
                shift = amount as u32;
                let want = if op == ISD::Srl { LoadExt::ZExtLoad } else { LoadExt::SExtLoad };
                ext_type = Some(want);
                ext_vt = EVT::integer(width - shift);
                if matches!(info.ext, Some(LoadExt::SExtLoad | LoadExt::ZExtLoad)) && info.ext != ext_type {
                    return None;
                }
            }
            ISD::And => {
                let mask = self
                    .dag
                    .constant(self.dag.operand(node, 1))
                    .filter(|_| !self.dag.is_opaque(self.dag.operand(node, 1)))?;
                self.load_info(n0)?;
                let active;
                if is_mask(mask) {
                    active = mask.count_ones();
                } else {
                    let (first, length) = shifted_mask(mask)?;
                    shift = first;
                    shifted_offset = first;
                    active = length;
                }
                ext_type = Some(LoadExt::ZExtLoad);
                ext_vt = EVT::integer(active);
            }
            _ => return None,
        }
        // `Opc == SRL || N0 is SRL`: the shift is the node itself here.
        let mut load = n0;
        if op == ISD::Srl {
            if self.dag.users(node).count() > 1 {
                return None;
            }
            let info = self.load_info(load)?;
            let width = info.mem_vt.size_in_bits();
            if shift >= width || info.ext == Some(LoadExt::SExtLoad) {
                return None;
            }
            if ext_vt.size_in_bits() > width - shift {
                if ext_type == Some(LoadExt::SExtLoad) {
                    return None;
                }
                ext_type = Some(LoadExt::ZExtLoad);
                ext_vt = EVT::integer(width - shift);
            }
            // The shift read only through a masking `and`: the load takes the
            // mask's width.
            let user = self.dag.users(node).next().map(|u| SDValue { node: u, res: 0 });
            if let Some(user) = user.filter(|&u| self.dag.opcode(u) == ISD::And) {
                if let Some(m) = self.dag.constant(self.dag.operand(user, 1)) {
                    if is_mask(m) {
                        let masked = EVT::integer(m.count_ones());
                        if ext_vt.size_in_bits() > masked.size_in_bits() && self.load_legal(vt, masked, ext_type) {
                            ext_vt = masked;
                        }
                    } else if ext_type == Some(LoadExt::ZExtLoad) {
                        if let Some((offset, active)) = shifted_mask(m) {
                            let masked = EVT::integer(active);
                            if u64::from(offset + shift) < u64::from(vt.size_in_bits())
                                && offset + active <= ext_vt.size_in_bits()
                                && self.load_legal(vt, masked, ext_type)
                            {
                                ext_vt = masked;
                                shift += offset;
                                shifted_offset = offset;
                            }
                        }
                    }
                }
            }
            load = self.dag.operand(node, 0);
        }
        let info = self.load_info(load)?;
        if !self.legal_narrow(load, &info, ext_type, ext_vt, shift) {
            return None;
        }
        let offset = i64::from(shift / 8);
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let ptr = if offset == 0 { ptr } else { self.dag.get_object_ptr_offset(ptr, offset) };
        let base = match info.base {
            MemBase::Stack(at) => MemBase::Stack(at + offset as i32),
            MemBase::CallArgument(at) => MemBase::CallArgument(at + offset as i32),
            other => other,
        };
        let align = if offset == 0 { info.align } else { info.align.min(1 << (offset as u32).trailing_zeros()) };
        let narrow = MemInfo { mem_vt: ext_vt, align, base, ext: None, ..info };
        let new = match ext_type {
            Some(ext) if ext_vt != vt => self.dag.get_ext_load(ext, vt, chain, ptr, narrow),
            _ => self.dag.get_load(vt, chain, ptr, narrow),
        };
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: new.node, res: 1 });
        let mut result = new;
        if frozen.is_some() {
            result = self.dag.get_node(ISD::Freeze, vt, &[result]);
            let kind = match ext_type {
                Some(LoadExt::ZExtLoad) => Some(ISD::AssertZext),
                Some(LoadExt::SExtLoad) => Some(ISD::AssertSext),
                _ => None,
            };
            if let Some(kind) = kind {
                let asserted = self.dag.get_value_type_node(ext_vt);
                result = self.dag.get_node(kind, vt, &[result, asserted]);
            }
        }
        if shifted_offset != 0 {
            let by = self.dag.get_constant(u128::from(shifted_offset), vt);
            result = self.dag.get_node(ISD::Shl, vt, &[result, by]);
            self.replace_value(node, result);
        }
        Some(result)
    }

    fn load_info(
        &self,
        load: SDValue,
    ) -> Option<MemInfo> {
        if self.dag.opcode(load) != ISD::Load || load.res != 0 {
            return None;
        }
        match &self.dag.node(load).data {
            super::dag::NodeData::Mem(info) => Some(info.clone()),
            _ => None,
        }
    }

    /// `TLI.isLoadLegal(vt, mem, ..., ext)`.
    fn load_legal(
        &self,
        vt: EVT,
        mem: EVT,
        ext: Option<LoadExt>,
    ) -> bool {
        let (EVT::Simple(vt), EVT::Simple(mem)) = (vt, mem) else { return false };
        match ext {
            None => true,
            Some(ext) => self.tli.is_some_and(|tli| tli.load_ext_action(ext, vt, mem) == Action::Legal),
        }
    }

    /// `isLegalNarrowLdSt` for a load.
    fn legal_narrow(
        &self,
        load: SDValue,
        info: &MemInfo,
        ext: Option<LoadExt>,
        mem: EVT,
        shift: u32,
    ) -> bool {
        let bits = mem.size_in_bits();
        if shift % 8 != 0 || bits < 8 || !bits.is_power_of_two() || info.volatile {
            return false;
        }
        if info.mem_vt.size_in_bits() < bits {
            return false;
        }
        // Only the value result is read, and nothing else reads the load.
        if self.dag.users(SDValue { node: load.node, res: 0 }).count() != 1 || self.dag.node(load).values.len() > 2 {
            return false;
        }
        if info.ext.is_some() && info.mem_vt.size_in_bits() < bits + shift {
            return false;
        }
        if self.level >= CombineLevel::AfterLegalizeVectorOps
            && !self.load_legal(self.dag.value_type(load), mem, ext.or(Some(LoadExt::ExtLoad)))
        {
            return false;
        }
        true
    }
}
