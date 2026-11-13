//! Vectors no register holds are split in two (`TypeSplitVector`): the pin's
//! `SplitVectorResult` and `SplitVectorOperand`.
//!
//! # How it works
//! A value of an illegal vector type (`v4i64`) is carried as two values of its
//! half (`v2i64`), kept in `TypeLegalizer::split` (the pin's `SplitVectors`).
//! A node that makes such a value makes its halves
//! ([`TypeLegalizer::split_vector_result`]); a node that only reads one reads
//! the halves ([`TypeLegalizer::split_vector_operand`]): a store becomes two, a
//! conversion to a legal type is made on each half and the results are
//! concatenated.
//!
//! # Translated
//! `SplitVecRes_BUILD_VECTOR`, `SplitVecRes_INSERT_VECTOR_ELT` (a constant
//! index), `SplitVecRes_LOAD`, `SplitVecRes_BinOp`, `SplitVecRes_UnaryOp`,
//! `SplitVecRes_TernaryOp` (`fma`), `SplitVecOp_STORE`, `SplitVecOp_UnaryOp`,
//! `SplitVecOp_EXTRACT_VECTOR_ELT` (a constant index), `SplitVecOp_VECREDUCE`,
//! `SplitVecRes_CONCAT_VECTORS`, `SplitVecRes_EXTRACT_SUBVECTOR`,
//! `SplitVecOp_EXTRACT_SUBVECTOR` (a piece inside one half, or the high half),
//! `SplitVecRes_ExtendOp`, all of
//! [LegalizeVectorTypes.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeVectorTypes.cpp).
//!
//! DEVIATE SV1 (owner llrm-arm64-vec, exit 2026-11-27): an `insertelement` or
//! `extractelement` at a variable index, an extending load, a truncating store
//! and the other nodes of `SplitVectorResult`'s switch are refused by name
//! ([`LegalizeError::Unsupported`]); the pin spills the vector to the stack
//! for the variable index.

use llrm_sdag::lowering::TypeAction;

use super::{LegalizeError, SDValue, TypeLegalizer};
use crate::backend::selectiondag::dag::{MemInfo, NodeData};
use crate::backend::selectiondag::isd::ISD;
use crate::backend::selectiondag::mvt::{EVT, MVT};

/// `GetSplitDestVTs`: the two halves of a vector type.
fn split_dest(vt: EVT) -> Option<(EVT, EVT)> {
    let EVT::Simple(mvt) = vt else { return None };
    let half = MVT::vector(mvt.element()?, mvt.lanes()? / 2)?;
    Some((EVT::Simple(half), EVT::Simple(half)))
}

/// The nodes of one result type and operands of that type: `SplitVecRes_BinOp`
/// and `SplitVecRes_TernaryOp`.
fn is_elementwise(op: ISD) -> bool {
    matches!(
        op,
        ISD::Add
            | ISD::Sub
            | ISD::Mul
            | ISD::And
            | ISD::Or
            | ISD::Xor
            | ISD::Shl
            | ISD::Srl
            | ISD::Sra
            | ISD::SMin
            | ISD::SMax
            | ISD::UMin
            | ISD::UMax
            | ISD::FAdd
            | ISD::FSub
            | ISD::FMul
            | ISD::FDiv
            | ISD::FMA
            | ISD::FNeg
            | ISD::FAbs
    )
}

/// `SplitVecRes_ExtendOp`'s nodes.
fn is_extend(op: ISD) -> bool {
    matches!(op, ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend)
}

/// The conversions whose result type differs from the operand's
/// (`SplitVecRes_UnaryOp`, `SplitVecOp_UnaryOp`).
fn is_conversion(op: ISD) -> bool {
    matches!(
        op,
        ISD::SIntToFp | ISD::UIntToFp | ISD::FpToSInt | ISD::FpToUInt | ISD::FpExtend | ISD::FpRound
    )
}

impl TypeLegalizer<'_> {
    /// `SelectionDAG::SplitVector`: the low half at lane 0 and the high half
    /// after it, both `EXTRACT_SUBVECTOR`.
    fn split_vector_of(
        &mut self,
        value: SDValue,
    ) -> Option<(SDValue, SDValue)> {
        let (lo_vt, hi_vt) = split_dest(self.dag.value_type(value))?;
        let zero = self.dag.get_constant(0, EVT::I64);
        let high = self.dag.get_constant(u128::from(lo_vt.lanes()), EVT::I64);
        Some((
            self.dag.get_node(ISD::ExtractSubvector, lo_vt, &[value, zero]),
            self.dag.get_node(ISD::ExtractSubvector, hi_vt, &[value, high]),
        ))
    }

    /// `SplitVecRes_ExtendOp`: an extension by more than a doubling of the
    /// lane, of a legal source whose half is no type, is made in two steps
    /// first, so that the halves are legal.
    fn split_extend(
        &mut self,
        value: SDValue,
        lo_vt: EVT,
        hi_vt: EVT,
    ) -> Option<(SDValue, SDValue)> {
        let op = self.dag.opcode(value);
        let source = self.dag.operand(value, 0);
        let (src_vt, dest_vt) = (self.dag.value_type(source), self.dag.value_type(value));
        let flags = self.dag.node(value).flags;
        let (src_bits, dest_bits) = (src_vt.element().size_in_bits(), dest_vt.element().size_in_bits());
        if src_vt.lanes() % 2 == 0 && src_bits * 2 < dest_bits {
            let EVT::Simple(src_mvt) = src_vt else { return None };
            let new_src = MVT::vector(MVT::integer(src_bits * 2)?, src_mvt.lanes()?).map(EVT::Simple)?;
            // A half with no `MVT` (`v4i8`) is not a legal type.
            let split_src = MVT::vector(src_mvt.element()?, src_mvt.lanes()? / 2).map(EVT::Simple);
            let (split_lo, _) = split_dest(new_src)?;
            let legal = |this: &Self, vt: EVT| this.action(vt) == TypeAction::Legal;
            if legal(self, src_vt)
                && !split_src.is_some_and(|half| legal(self, half))
                && legal(self, new_src)
                && legal(self, split_lo)
            {
                let widened = self.dag.get_node_flags(op, new_src, &[source], flags);
                let (lo, hi) = self.split_vector_of(widened)?;
                return Some((
                    self.dag.get_node_flags(op, lo_vt, &[lo], flags),
                    self.dag.get_node_flags(op, hi_vt, &[hi], flags),
                ));
            }
        }
        // The generic split (`SplitVecRes_UnaryOp`).
        let (lo, hi) = if self.action(src_vt) == TypeAction::SplitVector {
            self.split_vector(source)
        } else {
            self.split_vector_of(source)?
        };
        Some((self.dag.get_node_flags(op, lo_vt, &[lo], flags), self.dag.get_node_flags(op, hi_vt, &[hi], flags)))
    }

    /// `GetSplitVector`.
    fn split_vector(
        &self,
        value: SDValue,
    ) -> (SDValue, SDValue) {
        let value = self.remap(value);
        *self.split.get(&value).expect("operand split before its user")
    }

    /// `SetSplitVector`.
    fn set_split_vector(
        &mut self,
        value: SDValue,
        lo: SDValue,
        hi: SDValue,
    ) {
        let value = self.remap(value);
        let (lo, hi) = (self.analyze_new_value(lo), self.analyze_new_value(hi));
        self.split.insert(value, (lo, hi));
    }

    /// `SplitVectorResult`.
    pub(super) fn split_vector_result(
        &mut self,
        value: SDValue,
    ) -> Result<(), LegalizeError> {
        let vt = self.dag.value_type(value);
        let op = self.dag.opcode(value);
        let unsupported = |what| LegalizeError::Unsupported { op, ty: vt, what };
        let (lo_vt, hi_vt) = split_dest(vt).ok_or_else(|| unsupported("split result of a type with no half"))?;
        let flags = self.dag.node(value).flags;
        let (lo, hi) = match op {
            ISD::Undef | ISD::Poison => (self.dag.get_undef(lo_vt), self.dag.get_undef(hi_vt)),
            // `SplitVecRes_BUILD_VECTOR`: the first lanes, then the rest.
            ISD::BuildVector => {
                let ops = self.dag.node(value).ops.clone();
                let lanes = lo_vt.lanes() as usize;
                (self.dag.get_build_vector(lo_vt, &ops[..lanes]), self.dag.get_build_vector(hi_vt, &ops[lanes..]))
            }
            // `SplitVecRes_INSERT_VECTOR_ELT`, a constant index: into the half
            // it lands in.
            ISD::InsertVectorElt => {
                let (vector, element, index) =
                    (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
                let Some(at) = self.dag.constant(index) else {
                    return Err(unsupported("split insert at a variable index"));
                };
                let (lo, hi) = self.split_vector(vector);
                let lanes = u128::from(lo_vt.lanes());
                if at < lanes {
                    (self.dag.get_node(ISD::InsertVectorElt, lo_vt, &[lo, element, index]), hi)
                } else {
                    let index = self.dag.get_constant(at - lanes, self.dag.value_type(index));
                    (lo, self.dag.get_node(ISD::InsertVectorElt, hi_vt, &[hi, element, index]))
                }
            }
            ISD::Load => self.split_load(value, lo_vt, hi_vt).ok_or_else(|| unsupported("split extending load"))?,
            // `SplitVecRes_BinOp`, `SplitVecRes_TernaryOp`: each operand's
            // halves.
            op if is_elementwise(op) => {
                let ops = self.dag.node(value).ops.clone();
                let (mut los, mut his) = (Vec::new(), Vec::new());
                for operand in ops {
                    if self.dag.value_type(operand) != vt {
                        return Err(unsupported("split operation with an operand of another type"));
                    }
                    let (lo, hi) = self.split_vector(operand);
                    los.push(lo);
                    his.push(hi);
                }
                (self.dag.get_node_flags(op, lo_vt, &los, flags), self.dag.get_node_flags(op, hi_vt, &his, flags))
            }
            // `SplitVecRes_UnaryOp`: the result type may differ from the
            // operand's, and the operand splits too.
            op if is_conversion(op) => {
                let ops = self.dag.node(value).ops.clone();
                if self.action(self.dag.value_type(ops[0])) != TypeAction::SplitVector {
                    return Err(unsupported("split conversion of an operand that does not split"));
                }
                let (lo, hi) = self.split_vector(ops[0]);
                let (mut los, mut his) = (vec![lo], vec![hi]);
                los.extend_from_slice(&ops[1..]);
                his.extend_from_slice(&ops[1..]);
                (self.dag.get_node_flags(op, lo_vt, &los, flags), self.dag.get_node_flags(op, hi_vt, &his, flags))
            }
            // `SplitVecRes_CONCAT_VECTORS`: the first operands, then the rest.
            ISD::ConcatVectors => {
                let ops = self.dag.node(value).ops.clone();
                if ops.len() % 2 != 0 {
                    return Err(unsupported("split concatenation of an odd count of vectors"));
                }
                let half = ops.len() / 2;
                if half == 1 {
                    (ops[0], ops[1])
                } else {
                    (
                        self.dag.get_node(ISD::ConcatVectors, lo_vt, &ops[..half]),
                        self.dag.get_node(ISD::ConcatVectors, hi_vt, &ops[half..]),
                    )
                }
            }
            // `SplitVecRes_EXTRACT_SUBVECTOR`: each half of the piece, from the
            // same vector.
            ISD::ExtractSubvector => {
                let (vector, index) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let at = self.dag.constant(index).ok_or_else(|| unsupported("split extract at a variable index"))?;
                let high = self.dag.get_constant(at + u128::from(lo_vt.lanes()), self.dag.value_type(index));
                (
                    self.dag.get_node(ISD::ExtractSubvector, lo_vt, &[vector, index]),
                    self.dag.get_node(ISD::ExtractSubvector, hi_vt, &[vector, high]),
                )
            }
            // `SplitVecRes_ExtendOp`.
            op if is_extend(op) => {
                self.split_extend(value, lo_vt, hi_vt).ok_or_else(|| unsupported("split extension"))?
            }
            _ => return Err(unsupported("split result")),
        };
        self.set_split_vector(value, lo, hi);
        Ok(())
    }

    /// `SplitVecRes_LOAD`: the first half at the address, the second one half
    /// on; their chains joined. `None` for an extending load.
    fn split_load(
        &mut self,
        load: SDValue,
        lo_vt: EVT,
        hi_vt: EVT,
    ) -> Option<(SDValue, SDValue)> {
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let NodeData::Mem(info) = self.dag.node(load).data else { unreachable!("a load has a memory operand") };
        if info.ext.is_some() {
            return None;
        }
        let step = u128::from(lo_vt.size_in_bits() / 8);
        let high_ptr = self.address_plus(ptr, step);
        let lo_info = MemInfo { mem_vt: lo_vt, ..info };
        let hi_info = MemInfo { mem_vt: hi_vt, align: info.align.min(step as u32), ..info };
        let (lo, hi) =
            (self.dag.get_load(lo_vt, chain, ptr, lo_info), self.dag.get_load(hi_vt, chain, high_ptr, hi_info));
        let chains = self.dag.get_token_factor(&[SDValue { node: lo.node, res: 1 }, SDValue { node: hi.node, res: 1 }]);
        self.replace_value_with(SDValue { node: load.node, res: 1 }, chains);
        Some((lo, hi))
    }

    /// `SplitVectorOperand`: an operand `at` of node `n` is a vector to split.
    /// True if `n` was updated in place and must be looked at again.
    pub(super) fn split_vector_operand(
        &mut self,
        n: u32,
        at: usize,
    ) -> Result<bool, LegalizeError> {
        let node = SDValue { node: n, res: 0 };
        let op = self.dag.opcode(node);
        let operand = self.dag.operand(node, at);
        let unsupported = |what| LegalizeError::Unsupported { op, ty: self.dag.value_type(operand), what };
        let result = match op {
            // `SplitVecOp_STORE`, a plain store: the first half, then the
            // second one half on.
            ISD::Store if at == 1 => {
                let (chain, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 2));
                let NodeData::Mem(info) = self.dag.node(node).data else {
                    unreachable!("a store has a memory operand")
                };
                if info.mem_vt != self.dag.value_type(operand) {
                    return Err(unsupported("split truncating store"));
                }
                let (lo, hi) = self.split_vector(operand);
                let (lo_vt, hi_vt) = (self.dag.value_type(lo), self.dag.value_type(hi));
                let step = u128::from(lo_vt.size_in_bits() / 8);
                let high_ptr = self.address_plus(ptr, step);
                let lo_info = MemInfo { mem_vt: lo_vt, ..info };
                let hi_info = MemInfo { mem_vt: hi_vt, align: info.align.min(step as u32), ..info };
                let low_store = self.dag.get_store(chain, lo, ptr, lo_info);
                let high_store = self.dag.get_store(chain, hi, high_ptr, hi_info);
                self.dag.get_token_factor(&[low_store, high_store])
            }
            // `SplitVecOp_UnaryOp`: the result is legal, the operand splits:
            // each half converted to half the result, then joined.
            op if at == 0 && is_conversion(op) => {
                let result_vt = self.dag.value_type(node);
                let (lo, hi) = self.split_vector(operand);
                let lanes = self.dag.value_type(lo).lanes();
                let EVT::Simple(element) = result_vt.element() else {
                    return Err(unsupported("convert to a non-simple lane"));
                };
                let out = MVT::vector(element, lanes)
                    .map(EVT::Simple)
                    .ok_or_else(|| unsupported("split conversion with no half result type"))?;
                let flags = self.dag.node(node).flags;
                let mut lo_ops = vec![lo];
                let mut hi_ops = vec![hi];
                let rest = self.dag.node(node).ops[1..].to_vec();
                lo_ops.extend_from_slice(&rest);
                hi_ops.extend_from_slice(&rest);
                let (lo, hi) = (
                    self.dag.get_node_flags(op, out, &lo_ops, flags),
                    self.dag.get_node_flags(op, out, &hi_ops, flags),
                );
                self.dag.get_node(ISD::ConcatVectors, result_vt, &[lo, hi])
            }
            // `SplitVecOp_VECREDUCE`: the halves combined by the scalar
            // operation, then reduced.
            op if op.vec_reduce_base_opcode().is_some() => {
                let base = op.vec_reduce_base_opcode().expect("a reduction");
                let (lo, hi) = self.split_vector(operand);
                let half = self.dag.value_type(lo);
                let flags = self.dag.node(node).flags;
                let partial = self.dag.get_node_flags(base, half, &[lo, hi], flags);
                self.dag.get_node_flags(op, self.dag.value_type(node), &[partial], flags)
            }
            // `SplitVecOp_EXTRACT_VECTOR_ELT`, a constant index.
            ISD::ExtractElement if at == 0 => {
                let index = self.dag.operand(node, 1);
                let Some(lane) = self.dag.constant(index) else {
                    return Err(unsupported("split extract at a variable index"));
                };
                let (lo, hi) = self.split_vector(operand);
                let lanes = u128::from(self.dag.value_type(lo).lanes());
                let vt = self.dag.value_type(node);
                if lane < lanes {
                    self.dag.get_node(ISD::ExtractElement, vt, &[lo, index])
                } else {
                    let index = self.dag.get_constant(lane - lanes, self.dag.value_type(index));
                    self.dag.get_node(ISD::ExtractElement, vt, &[hi, index])
                }
            }
            // `SplitVecOp_EXTRACT_SUBVECTOR`: the piece is in the low half, or
            // it is a whole piece of the high half.
            ISD::ExtractSubvector if at == 0 => {
                let index = self.dag.operand(node, 1);
                let at_lane =
                    self.dag.constant(index).ok_or_else(|| unsupported("split extract at a variable index"))?;
                let (lo, hi) = self.split_vector(operand);
                let result_vt = self.dag.value_type(node);
                let (lo_lanes, lanes) = (u128::from(self.dag.value_type(lo).lanes()), u128::from(result_vt.lanes()));
                if at_lane + lanes <= lo_lanes {
                    self.dag.get_node(ISD::ExtractSubvector, result_vt, &[lo, index])
                } else if at_lane >= lo_lanes && (at_lane - lo_lanes) % lanes == 0 {
                    let index = self.dag.get_constant(at_lane - lo_lanes, self.dag.value_type(index));
                    self.dag.get_node(ISD::ExtractSubvector, result_vt, &[hi, index])
                } else {
                    return Err(unsupported("split extract across the halves"));
                }
            }
            _ => return Err(unsupported("split operand")),
        };
        if result.node == n {
            return Ok(true);
        }
        self.replace_value_with(node, result);
        Ok(false)
    }
}
