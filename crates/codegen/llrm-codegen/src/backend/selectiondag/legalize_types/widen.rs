//! Vectors a register class holds with more lanes are widened
//! (`TypeWidenVector`): the pin's `WidenVectorResult` and
//! `WidenVectorOperand`.
//!
//! # How it works
//! A value of an illegal vector type (`v8i8`) is carried as one value of the
//! wider legal type (`v16i8`), its extra lanes undefined, kept in
//! `TypeLegalizer::widened` (the pin's `WidenedVectors`). A node that makes
//! such a value makes the wide one ([`TypeLegalizer::widen_vector_result`]); a
//! node that only reads one reads the wide one
//! ([`TypeLegalizer::widen_vector_operand`]).
//!
//! Memory is never touched past the vector: a load or store is made of the
//! widest pieces that fit it (`findMemType`), an `i64` for a `v8i8`.
//!
//! # Translated
//! `WidenVecRes_BUILD_VECTOR`, `WidenVecRes_LOAD` (`GenWidenVectorLoads`),
//! `WidenVecRes_Binary`, `WidenVecRes_INSERT_VECTOR_ELT`,
//! `WidenVecRes_SCALAR_TO_VECTOR`, `WidenVecOp_STORE`
//! (`GenWidenVectorStores`), `WidenVecOp_EXTRACT_VECTOR_ELT`,
//! `WidenVecOp_EXTEND`, `WidenVecRes_EXTRACT_SUBVECTOR`, `WidenVecRes_BITCAST`,
//! `WidenVecOp_CONCAT_VECTORS`, `WidenVecOp_BITCAST` (not their stack-slot
//! fallbacks), all of
//! [LegalizeVectorTypes.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeVectorTypes.cpp).
//!
//! DEVIATE WV1 (owner llrm-target-x86-m64-b, exit 2026-11-30): every other
//! node of `WidenVectorResult`'s and `WidenVectorOperand`'s switch, an
//! integer division (`WidenVecRes_BinaryCanTrap`), an extending load and a
//! load or store in vector pieces are refused by name
//! ([`LegalizeError::Unsupported`]).

use llrm_sdag::lowering::TypeAction;

use super::{LegalizeError, SDValue, TypeLegalizer};
use crate::backend::selectiondag::SelectionDAG;
use crate::backend::selectiondag::dag::{MemInfo, NodeData};
use crate::backend::selectiondag::isd::ISD;
use crate::backend::selectiondag::mvt::{EVT, MVT};

/// The integer types a memory piece may be, widest first
/// (`reverse(MVT::integer_valuetypes())`).
const INTEGER_PIECES: [MVT; 5] = [MVT::I128, MVT::I64, MVT::I32, MVT::I16, MVT::I8];

/// `commonAlignment`: the alignment of the address `offset` bytes past one
/// aligned to `align`.
fn common_alignment(
    align: u32,
    offset: u128,
) -> u32 {
    if offset == 0 { align } else { align.min((offset & offset.wrapping_neg()).min(u128::from(u32::MAX)) as u32) }
}

impl TypeLegalizer<'_> {
    /// `GetWidenedVector`.
    fn widened_vector(
        &self,
        value: SDValue,
    ) -> SDValue {
        let value = self.remap(value);
        *self.widened.get(&value).expect("operand widened before its user")
    }

    /// `SetWidenedVector`.
    fn set_widened_vector(
        &mut self,
        value: SDValue,
        result: SDValue,
    ) {
        let value = self.remap(value);
        let result = self.analyze_new_value(result);
        self.widened.insert(value, result);
    }

    /// The operand of a node that widens, in the type of the result: widened
    /// where it is an illegal vector, itself where it already has that type.
    fn widen_operand(
        &self,
        operand: SDValue,
    ) -> SDValue {
        if self.action(self.dag.value_type(operand)) == TypeAction::WidenVector {
            self.widened_vector(operand)
        } else {
            operand
        }
    }

    /// `findMemType`: the widest type a load or store of `width` bits of a
    /// vector widened to `widen_vt` may use. `align` (bytes, 0 for a volatile
    /// or atomic access) lets a load read past `width` up to `widen_ex` bits
    /// when the access is aligned so that it cannot fault.
    fn find_mem_type(
        &self,
        width: u32,
        widen_vt: EVT,
        align: u32,
        widen_ex: u32,
    ) -> Option<EVT> {
        let widen_elt = widen_vt.element();
        let widen_width = widen_vt.size_in_bits();
        let widen_elt_width = widen_elt.size_in_bits();
        let align_in_bits = align * 8;
        let usable =
            |this: &Self, mem_vt: EVT| matches!(
                this.action(mem_vt),
                TypeAction::Legal | TypeAction::PromoteInteger
            );
        let fits = |mem_width: u32| {
            widen_width % mem_width == 0
                && (widen_width / mem_width).is_power_of_two()
                && (mem_width <= width || (align != 0 && mem_width <= align_in_bits && mem_width <= width + widen_ex))
        };

        let mut ret = widen_elt;
        if width == widen_elt_width {
            return Some(ret);
        }
        for mem in INTEGER_PIECES {
            let mem_vt = EVT::Simple(mem);
            let mem_width = mem_vt.size_in_bits();
            if mem_width <= widen_elt_width {
                break;
            }
            if usable(self, mem_vt) && fits(mem_width) {
                if mem_width == widen_width {
                    return Some(mem_vt);
                }
                ret = mem_vt;
                break;
            }
        }
        // The widest vector of the same element first.
        let mut vectors: Vec<MVT> = MVT::vectors().filter(|vt| EVT::Simple(*vt).element() == widen_elt).collect();
        vectors.sort_by_key(|vt| std::cmp::Reverse(vt.lanes()));
        for mem in vectors {
            let mem_vt = EVT::Simple(mem);
            let mem_width = mem_vt.size_in_bits();
            if usable(self, mem_vt) && fits(mem_width) && (ret.size_in_bits() < mem_width || mem_vt == widen_vt) {
                return Some(mem_vt);
            }
        }
        Some(ret)
    }

    /// `BuildVectorFromScalar`, for scalar loads only: the first becomes lane
    /// 0 of a vector of its type, each next one is inserted after it, then the
    /// whole is cast to `vec_vt`.
    fn build_vector_from_scalars(
        &mut self,
        vec_vt: EVT,
        scalars: &[SDValue],
    ) -> Option<SDValue> {
        let width = vec_vt.size_in_bits();
        let mut ty = self.dag.value_type(scalars[0]);
        let vector_of = |ty: EVT| match ty {
            EVT::Simple(lane) => MVT::vector(lane, width / ty.size_in_bits()).map(EVT::Simple),
            EVT::Int(_) | EVT::Vector(..) => None,
        };
        let mut vec = self.dag.get_node(ISD::ScalarToVector, vector_of(ty)?, &[scalars[0]]);
        let mut at = 1_u32;
        for &scalar in &scalars[1..] {
            let next = self.dag.value_type(scalar);
            if next != ty {
                vec = self.dag.get_node(ISD::BitCast, vector_of(next)?, &[vec]);
                at = at * ty.size_in_bits() / next.size_in_bits();
                ty = next;
            }
            let index = self.dag.get_constant(u128::from(at), EVT::I64);
            vec = self.dag.get_node(ISD::InsertVectorElt, self.dag.value_type(vec), &[vec, scalar, index]);
            at += 1;
        }
        Some(self.dag.get_node(ISD::BitCast, vec_vt, &[vec]))
    }

    /// `GenWidenVectorLoads`: the loads that read exactly the vector's bytes,
    /// built into the widened value; `None` for a load in vector pieces.
    /// Pushes the chain of each load on `chains`.
    fn gen_widen_vector_loads(
        &mut self,
        load: SDValue,
        chains: &mut Vec<SDValue>,
    ) -> Option<SDValue> {
        let widen_vt = self.tli.types.type_conversion(self.dag.value_type(load)).1;
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let NodeData::Mem(info) = self.dag.node(load).data else { unreachable!("a load has a memory operand") };
        let ld_width = info.mem_vt.size_in_bits();
        let width_diff = widen_vt.size_in_bits() - ld_width;
        // A wider load is allowed where it cannot fault, of a simple access.
        let simple = !info.volatile && info.ordering.is_none();
        let ld_align = if simple { info.align } else { 0 };

        let first_vt = self.find_mem_type(ld_width, widen_vt, ld_align, width_diff)?;
        let first_width = first_vt.size_in_bits();
        let mut mem_vts = Vec::new();
        if ld_width > first_width {
            let (mut new_vt, mut remaining, mut new_width) = (first_vt, ld_width, first_width);
            loop {
                remaining -= new_width;
                if remaining < new_width {
                    new_vt = self.find_mem_type(remaining, widen_vt, ld_align, width_diff)?;
                    new_width = new_vt.size_in_bits();
                }
                mem_vts.push(new_vt);
                if remaining <= new_width {
                    break;
                }
            }
        }

        let first = self.dag.get_load(first_vt, chain, ptr, MemInfo { mem_vt: first_vt, ..info });
        chains.push(SDValue { node: first.node, res: 1 });
        if mem_vts.is_empty() {
            // `coerceLoadedValue`
            if first_vt.is_vector() {
                debug_assert_eq!(first_vt, widen_vt, "the first type is the widened one");
                return Some(first);
            }
            let EVT::Simple(lane) = first_vt else { return None };
            let vec = MVT::vector(lane, widen_vt.size_in_bits() / first_width)?;
            let vec = self.dag.get_node(ISD::ScalarToVector, EVT::Simple(vec), &[first]);
            return Some(self.dag.get_node(ISD::BitCast, widen_vt, &[vec]));
        }
        if first_vt.is_vector() || mem_vts.iter().any(|vt| vt.is_vector()) {
            return None;
        }

        let mut scalars = vec![first];
        let mut offset = u128::from(first_width / 8);
        let mut ptr = self.address_plus(ptr, offset);
        for mem_vt in mem_vts {
            let piece = self
                .dag
                .get_load(mem_vt, chain, ptr, MemInfo { mem_vt, align: common_alignment(info.align, offset), ..info });
            chains.push(SDValue { node: piece.node, res: 1 });
            scalars.push(piece);
            let step = u128::from(mem_vt.size_in_bits() / 8);
            offset += step;
            ptr = self.address_plus(ptr, step);
        }
        self.build_vector_from_scalars(widen_vt, &scalars)
    }

    /// `GenWidenVectorStores`: the stores that write exactly the vector's
    /// bytes from the widened value; `None` for a store in vector pieces.
    fn gen_widen_vector_stores(
        &mut self,
        store: SDValue,
        chains: &mut Vec<SDValue>,
    ) -> Option<()> {
        let (chain, value, ptr) = (self.dag.operand(store, 0), self.dag.operand(store, 1), self.dag.operand(store, 2));
        let NodeData::Mem(info) = self.dag.node(store).data else { unreachable!("a store has a memory operand") };
        let val_op = self.widened_vector(value);
        let val_vt = self.dag.value_type(val_op);
        let val_width = val_vt.size_in_bits();
        let mut remaining = info.mem_vt.size_in_bits();

        // How to widen the store: each piece type and how many of it.
        let mut pieces: Vec<(EVT, u32)> = Vec::new();
        while remaining != 0 {
            let new_vt = self.find_mem_type(remaining, val_vt, 0, 0)?;
            if new_vt.is_vector() {
                return None;
            }
            let new_width = new_vt.size_in_bits();
            let mut count = 0;
            loop {
                remaining -= new_width;
                count += 1;
                if remaining == 0 || remaining < new_width {
                    break;
                }
            }
            pieces.push((new_vt, count));
        }

        let mut offset = 0_u128;
        let mut ptr = ptr;
        let mut at = 0_u32;
        for (new_vt, count) in pieces {
            let new_width = new_vt.size_in_bits();
            let EVT::Simple(lane) = new_vt else { return None };
            // Cast the vector to the scalar type that is stored.
            let vec_vt = EVT::Simple(MVT::vector(lane, val_width / new_width)?);
            let vec = self.dag.get_node(ISD::BitCast, vec_vt, &[val_op]);
            // The index, relative to the new vector type.
            at = at * val_vt.element().size_in_bits() / new_width;
            for _ in 0..count {
                let index = self.dag.get_constant(u128::from(at), EVT::I64);
                at += 1;
                let part = self.dag.get_node(ISD::ExtractElement, new_vt, &[vec, index]);
                let piece = self
                    .dag
                    .get_store(
                        chain,
                        part,
                        ptr,
                        MemInfo { mem_vt: new_vt, align: common_alignment(info.align, offset), ..info },
                    );
                chains.push(piece);
                let step = u128::from(new_width / 8);
                offset += step;
                ptr = self.address_plus(ptr, step);
            }
            at = at * new_width / val_vt.element().size_in_bits();
        }
        Some(())
    }

    /// `WidenVecRes_LOAD`.
    fn widen_load(
        &mut self,
        load: SDValue,
    ) -> Option<SDValue> {
        let NodeData::Mem(info) = self.dag.node(load).data else { unreachable!("a load has a memory operand") };
        if info.ext.is_some() || info.mem_vt.element().size_in_bits() % 8 != 0 {
            return None;
        }
        let mut chains = Vec::new();
        let result = self.gen_widen_vector_loads(load, &mut chains)?;
        let chain = if chains.len() == 1 { chains[0] } else { self.dag.get_token_factor(&chains) };
        self.replace_value_with(SDValue { node: load.node, res: 1 }, chain);
        Some(result)
    }

    /// `WidenVectorResult`.
    pub(super) fn widen_vector_result(
        &mut self,
        value: SDValue,
    ) -> Result<(), LegalizeError> {
        let vt = self.dag.value_type(value);
        let op = self.dag.opcode(value);
        let unsupported = |what| LegalizeError::Unsupported { op, ty: vt, what };
        let widen_vt = self.tli.types.type_conversion(vt).1;
        let flags = self.dag.node(value).flags;
        let result = match op {
            ISD::Undef | ISD::Poison => self.dag.get_undef(widen_vt),
            // `WidenVecRes_BUILD_VECTOR`: the lanes, then undefined ones.
            ISD::BuildVector => {
                let mut ops = self.dag.node(value).ops.clone();
                let undef = self.dag.get_undef(vt.element());
                ops.resize(widen_vt.lanes() as usize, undef);
                self.dag.get_build_vector(widen_vt, &ops)
            }
            ISD::ScalarToVector => {
                let element = self.dag.operand(value, 0);
                self.dag.get_node(ISD::ScalarToVector, widen_vt, &[element])
            }
            // `WidenVecRes_INSERT_VECTOR_ELT`.
            ISD::InsertVectorElt => {
                let (vector, element, index) =
                    (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
                let vector = self.widened_vector(vector);
                self.dag.get_node(ISD::InsertVectorElt, widen_vt, &[vector, element, index])
            }
            ISD::Load => self.widen_load(value).ok_or_else(|| unsupported("widen load"))?,
            // `WidenVecRes_BinaryCanTrap`: the undefined lanes of a division
            // would trap.
            ISD::UDiv | ISD::SDiv | ISD::URem | ISD::SRem => return Err(unsupported("widen integer division")),
            // `WidenVecRes_Binary`, `WidenVecRes_Ternary`: each operand
            // widened.
            op if super::vector::is_elementwise(op) => {
                let ops = self.dag.node(value).ops.clone();
                if ops.iter().any(|operand| self.dag.value_type(*operand) != vt) {
                    return Err(unsupported("widen operation with an operand of another type"));
                }
                let wide: Vec<SDValue> = ops.into_iter().map(|operand| self.widened_vector(operand)).collect();
                self.dag.get_node_flags(op, widen_vt, &wide, flags)
            }
            // `WidenVecRes_EXTRACT_SUBVECTOR`: the input itself, or a subvector
            // of it as wide as the result; otherwise the lanes one by one, then
            // undefined ones.
            ISD::ExtractSubvector => {
                let (source, index) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let source = self.widen_operand(source);
                let source_vt = self.dag.value_type(source);
                let start = self.dag.constant(index).ok_or_else(|| unsupported("widen extract at a variable index"))?;
                let (wide_lanes, source_lanes) = (u128::from(widen_vt.lanes()), u128::from(source_vt.lanes()));
                if start == 0 && source_vt == widen_vt {
                    source
                } else if start % wide_lanes == 0 && start + wide_lanes < source_lanes {
                    self.dag.get_node(ISD::ExtractSubvector, widen_vt, &[source, index])
                } else {
                    let lane_vt = vt.element();
                    let mut ops = Vec::with_capacity(widen_vt.lanes() as usize);
                    for lane in 0..u128::from(vt.lanes()) {
                        let at = self.dag.get_constant(start + lane, EVT::I64);
                        ops.push(self.dag.get_node(ISD::ExtractElement, lane_vt, &[source, at]));
                    }
                    let undef = self.dag.get_undef(lane_vt);
                    ops.resize(widen_vt.lanes() as usize, undef);
                    self.dag.get_build_vector(widen_vt, &ops)
                }
            }
            // `WidenVecRes_SETCC`: both operands widened to the result's lane
            // count (those that widen already, else padded with undefined
            // lanes). A split input is not translated.
            ISD::SetCC => {
                let ops = self.dag.node(value).ops.clone();
                let input_vt = self.dag.value_type(ops[0]);
                let widen_in_vt = match input_vt.element() {
                    EVT::Simple(lane) => MVT::vector(lane, widen_vt.lanes()).map(EVT::Simple),
                    _ => None,
                }
                .ok_or_else(|| unsupported("widen compare of an extended vector"))?;
                let (lhs, rhs) = match self.action(input_vt) {
                    TypeAction::SplitVector => return Err(unsupported("widen compare of a split vector")),
                    TypeAction::WidenVector => (self.widened_vector(ops[0]), self.widened_vector(ops[1])),
                    _ => {
                        // `INSERT_SUBVECTOR` into undefined lanes: a
                        // concatenation, there being no such node here.
                        let (wide, narrow) = (widen_vt.lanes(), input_vt.lanes());
                        if wide % narrow != 0 {
                            return Err(unsupported("widen compare by a lane count that is not a multiple"));
                        }
                        let poison = self.dag.get_poison(input_vt);
                        let pad = |dag: &mut SelectionDAG, operand| {
                            let mut parts = vec![poison; (wide / narrow) as usize];
                            parts[0] = operand;
                            dag.get_node(ISD::ConcatVectors, widen_in_vt, &parts)
                        };
                        (pad(self.dag, ops[0]), pad(self.dag, ops[1]))
                    }
                };
                if self.dag.value_type(lhs) != widen_in_vt || self.dag.value_type(rhs) != widen_in_vt {
                    return Err(unsupported("widen compare whose input did not widen to the result's lanes"));
                }
                let mut operands = vec![lhs, rhs];
                operands.extend_from_slice(&ops[2..]);
                self.dag.get_node_flags(ISD::SetCC, widen_vt, &operands, flags)
            }
            // `WidenVecRes_Select`: the mask at the select's lane width where
            // `WidenVSELECTMask` has one, else the condition widened like the
            // arms.
            ISD::VSelect => {
                let ops = self.dag.node(value).ops.clone();
                let cond = match self.widen_vselect_mask(value) {
                    Some(mask) => mask,
                    None => {
                        let cond = self.widen_operand(ops[0]);
                        if self.dag.value_type(cond).lanes() != widen_vt.lanes() {
                            return Err(unsupported("widen select with a condition of another width"));
                        }
                        cond
                    }
                };
                let (on_true, on_false) = (self.widened_vector(ops[1]), self.widened_vector(ops[2]));
                self.dag.get_node_flags(ISD::VSelect, widen_vt, &[cond, on_true, on_false], flags)
            }
            // `WidenVecRes_Convert`: the conversion of the input widened to the
            // result's lane count where that is a legal vector (padded with
            // undefined lanes, or cut down), else the lanes one by one and
            // undefined ones.
            op if super::vector::is_extend(op) || super::vector::is_conversion(op) => {
                let ops = self.dag.node(value).ops.clone();
                let (input, rest) = (ops[0], &ops[1..]);
                let wide_input = self.widen_operand(input);
                let input_vt = self.dag.value_type(wide_input);
                let in_widen_vt = match self.dag.value_type(input).element() {
                    EVT::Simple(lane) => MVT::vector(lane, widen_vt.lanes()).map(EVT::Simple),
                    _ => None,
                };
                let mut sized = None;
                if let Some(in_widen_vt) = in_widen_vt {
                    if input_vt == in_widen_vt {
                        sized = Some(wide_input);
                    } else if self.tli.types.is_type_legal(in_widen_vt) {
                        let (wide, narrow) = (widen_vt.lanes(), input_vt.lanes());
                        if wide % narrow == 0 {
                            let undef = self.dag.get_undef(input_vt);
                            let mut parts = vec![undef; (wide / narrow) as usize];
                            parts[0] = wide_input;
                            sized = Some(self.dag.get_node(ISD::ConcatVectors, in_widen_vt, &parts));
                        } else if narrow % wide == 0 {
                            let zero = self.dag.get_constant(0, EVT::I64);
                            sized = Some(self.dag.get_node(ISD::ExtractSubvector, in_widen_vt, &[wide_input, zero]));
                        }
                    }
                }
                let convert = |dag: &mut SelectionDAG, ty: EVT, from: SDValue| {
                    let mut operands = vec![from];
                    operands.extend_from_slice(rest);
                    dag.get_node_flags(op, ty, &operands, flags)
                };
                if let Some(sized) = sized {
                    convert(self.dag, widen_vt, sized)
                } else {
                    // The pin's "nasty scalar code": only the result's own
                    // lanes are converted.
                    let (from_lane, lane_vt) = (input_vt.element(), widen_vt.element());
                    let undef = self.dag.get_undef(lane_vt);
                    let mut lanes = vec![undef; widen_vt.lanes() as usize];
                    for (at, lane) in lanes.iter_mut().enumerate().take(vt.lanes() as usize) {
                        let index = self.dag.get_constant(at as u128, EVT::I64);
                        let element = self.dag.get_node(ISD::ExtractElement, from_lane, &[wide_input, index]);
                        *lane = convert(self.dag, lane_vt, element);
                    }
                    self.dag.get_build_vector(widen_vt, &lanes)
                }
            }
            // `WidenVecRes_BITCAST`, an input that is legal or widens: the same
            // size is a bitcast; a smaller one is placed in lane 0 of a legal
            // vector of its own lanes, which is bitcast.
            ISD::BitCast => {
                let input = self.widen_operand(self.dag.operand(value, 0));
                let input_vt = self.dag.value_type(input);
                let (wide_bits, input_bits) = (widen_vt.size_in_bits(), input_vt.size_in_bits());
                if wide_bits == input_bits {
                    self.dag.get_node(ISD::BitCast, widen_vt, &[input])
                } else {
                    let lane_vt = if input_vt.is_vector() { input_vt.element() } else { input_vt };
                    let lane_bits = lane_vt.size_in_bits();
                    let new_vt = match lane_vt {
                        EVT::Simple(lane) if wide_bits % lane_bits == 0 && wide_bits % input_bits == 0 => {
                            MVT::vector(lane, wide_bits / lane_bits).map(EVT::Simple)
                        }
                        _ => None,
                    }
                    .filter(|&new_vt| self.tli.types.is_type_legal(new_vt) && self.tli.types.is_type_legal(lane_vt))
                    .ok_or_else(|| unsupported("widen bitcast through memory"))?;
                    let wide = if input_vt.is_vector() {
                        let undef = self.dag.get_undef(input_vt);
                        let mut parts = vec![undef; (wide_bits / input_bits) as usize];
                        parts[0] = input;
                        self.dag.get_node(ISD::ConcatVectors, new_vt, &parts)
                    } else {
                        self.dag.get_node(ISD::ScalarToVector, new_vt, &[input])
                    };
                    self.dag.get_node(ISD::BitCast, widen_vt, &[wide])
                }
            }
            _ => return Err(unsupported("widen result")),
        };
        self.set_widened_vector(value, result);
        Ok(())
    }

    /// `WidenVectorOperand`: an operand `at` of node `n` is a vector to
    /// widen. True if `n` was updated in place and must be looked at again.
    pub(super) fn widen_vector_operand(
        &mut self,
        n: u32,
        at: usize,
    ) -> Result<bool, LegalizeError> {
        let node = SDValue { node: n, res: 0 };
        let op = self.dag.opcode(node);
        let operand = self.dag.operand(node, at);
        let operand_vt = self.dag.value_type(operand);
        let unsupported = |what| LegalizeError::Unsupported { op, ty: operand_vt, what };
        let result = match op {
            // `WidenVecOp_STORE`, a plain store.
            ISD::Store if at == 1 => {
                let NodeData::Mem(info) = self.dag.node(node).data else {
                    unreachable!("a store has a memory operand")
                };
                if info.mem_vt != self.dag.value_type(operand) || info.mem_vt.element().size_in_bits() % 8 != 0 {
                    return Err(unsupported("widen truncating store"));
                }
                let mut chains = Vec::new();
                self.gen_widen_vector_stores(node, &mut chains).ok_or_else(|| unsupported("widen store"))?;
                if chains.len() == 1 { chains[0] } else { self.dag.get_token_factor(&chains) }
            }
            // `WidenVecOp_EXTRACT_VECTOR_ELT`: the lane of the wide vector.
            ISD::ExtractElement if at == 0 => {
                let index = self.dag.operand(node, 1);
                let wide = self.widened_vector(operand);
                self.dag.get_node(ISD::ExtractElement, self.dag.value_type(node), &[wide, index])
            }
            // `WidenVecOp_EXTEND`: the wide source extended whole, to as many
            // lanes as it has, then the low ones kept. The pin's
            // `*_EXTEND_VECTOR_INREG` node is that extension of the low lanes;
            // here the extension is the one the target lowers, split over the
            // registers its result takes, and the dead high part goes.
            ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend if at == 0 => {
                let wide = self.widened_vector(operand);
                let result_vt = self.dag.value_type(node);
                let EVT::Simple(lane) = result_vt.element() else {
                    return Err(unsupported("widen extension to a non-simple lane"));
                };
                let wide_result = MVT::vector(lane, self.dag.value_type(wide).lanes())
                    .map(EVT::Simple)
                    .ok_or_else(|| unsupported("widen extension with no wide result type"))?;
                // Any bits will do for an `any_extend`; the target lowers the
                // zero extension.
                let extend = if op == ISD::AnyExtend { ISD::ZeroExtend } else { op };
                let extended = self.dag.get_node_flags(extend, wide_result, &[wide], self.dag.node(node).flags);
                let zero = self.dag.get_constant(0, EVT::I64);
                self.dag.get_node(ISD::ExtractSubvector, result_vt, &[extended, zero])
            }
            // `WidenVecOp_CONCAT_VECTORS`: the first operand widened when the
            // rest are undefined, else every lane of every operand, one by one.
            ISD::ConcatVectors => {
                let ops = self.dag.node(node).ops.clone();
                let result_vt = self.dag.value_type(node);
                let first_vt = self.dag.value_type(ops[0]);
                if result_vt == self.tli.types.type_conversion(first_vt).1
                    && ops[1..].iter().all(|operand| self.dag.is_undef(*operand))
                {
                    self.widened_vector(ops[0])
                } else {
                    let lane_vt = result_vt.element();
                    let mut lanes = Vec::with_capacity(result_vt.lanes() as usize);
                    for operand in &ops {
                        let wide = self.widened_vector(*operand);
                        for lane in 0..u128::from(first_vt.lanes()) {
                            let at = self.dag.get_constant(lane, EVT::I64);
                            lanes.push(self.dag.get_node(ISD::ExtractElement, lane_vt, &[wide, at]));
                        }
                    }
                    self.dag.get_build_vector(result_vt, &lanes)
                }
            }
            // `WidenVecOp_BITCAST`, to a scalar that divides the wide vector:
            // lane 0 of the wide vector seen as lanes of that scalar.
            ISD::BitCast if at == 0 && !self.dag.value_type(node).is_vector() => {
                let wide = self.widened_vector(operand);
                let (vt, wide_bits) = (self.dag.value_type(node), self.dag.value_type(wide).size_in_bits());
                let new_vt = match vt {
                    EVT::Simple(lane) if wide_bits % vt.size_in_bits() == 0 => {
                        MVT::vector(lane, wide_bits / vt.size_in_bits()).map(EVT::Simple)
                    }
                    _ => None,
                }
                .filter(|&new_vt| self.tli.types.is_type_legal(new_vt))
                .ok_or_else(|| unsupported("widen bitcast through memory"))?;
                let cast = self.dag.get_node(ISD::BitCast, new_vt, &[wide]);
                let zero = self.dag.get_constant(0, EVT::I64);
                self.dag.get_node(ISD::ExtractElement, vt, &[cast, zero])
            }
            // `WidenVecOp_VECREDUCE`: the wide vector with the lanes past the
            // original ones set to the operation's neutral element, then
            // reduced.
            op if op.vec_reduce_base_opcode().is_some() => {
                let wide = self.widened_vector(operand);
                let (wide_vt, lane_vt) = (self.dag.value_type(wide), operand_vt.element());
                let bits = lane_vt.size_in_bits();
                let neutral = match op.vec_reduce_base_opcode() {
                    Some(ISD::And | ISD::UMin) => u128::MAX,
                    Some(ISD::Mul) => 1,
                    Some(ISD::SMax) => 1u128 << (bits - 1),
                    Some(ISD::SMin) => (1u128 << (bits - 1)) - 1,
                    _ => 0,
                } & (u128::MAX >> (128 - bits));
                let neutral = self.dag.get_constant(neutral, lane_vt);
                let mut padded = wide;
                for lane in operand_vt.lanes()..wide_vt.lanes() {
                    let index = self.dag.get_constant(u128::from(lane), EVT::I64);
                    padded = self.dag.get_node(ISD::InsertVectorElt, wide_vt, &[padded, neutral, index]);
                }
                let flags = self.dag.node(node).flags;
                self.dag.get_node_flags(op, self.dag.value_type(node), &[padded], flags)
            }
            _ => return Err(unsupported("widen operand")),
        };
        if result.node == n {
            return Ok(true);
        }
        let results = self.dag.node(node).values.len();
        if results == 1 {
            self.replace_value_with(node, result);
        } else {
            for res in 0..results as u8 {
                self.replace_value_with(SDValue { node: n, res }, SDValue { node: result.node, res });
            }
        }
        Ok(false)
    }
}
