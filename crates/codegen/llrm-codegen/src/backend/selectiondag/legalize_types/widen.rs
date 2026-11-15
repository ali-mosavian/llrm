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
//! `WidenVecOp_EXTEND`, all of
//! [LegalizeVectorTypes.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeVectorTypes.cpp).
//!
//! DEVIATE WV1 (owner llrm-target-x86-m64-b, exit 2026-11-30): every other
//! node of `WidenVectorResult`'s and `WidenVectorOperand`'s switch, an
//! integer division (`WidenVecRes_BinaryCanTrap`), an extending load and a
//! load or store in vector pieces are refused by name
//! ([`LegalizeError::Unsupported`]).

use llrm_sdag::lowering::TypeAction;

use super::{LegalizeError, SDValue, TypeLegalizer};
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
            EVT::Int(_) => None,
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
