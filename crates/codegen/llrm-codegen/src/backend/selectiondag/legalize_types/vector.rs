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
//! `SplitVecOp_EXTRACT_VECTOR_ELT` (a constant index), all of
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

/// The conversions whose result type differs from the operand's
/// (`SplitVecRes_UnaryOp`, `SplitVecOp_UnaryOp`).
fn is_conversion(op: ISD) -> bool {
    matches!(
        op,
        ISD::SIntToFp | ISD::UIntToFp | ISD::FpToSInt | ISD::FpToUInt | ISD::FpExtend | ISD::FpRound | ISD::Truncate
    )
}

impl TypeLegalizer<'_> {
    /// `GetSplitVector`.
    fn split_vector(
        &self,
        value: SDValue,
    ) -> (SDValue, SDValue) {
        let value = self.remap(value);
        *self.split.get(&value).expect("operand split before its user")
    }

    /// The type `SplitVecRes_ExtendOp` extends the source to first, when it
    /// can: the lanes doubled, the source legal and its halves not, the
    /// doubled type and its halves legal.
    fn incremental_extend_type(
        &self,
        operand: SDValue,
        dest: EVT,
    ) -> Option<EVT> {
        let (EVT::Simple(src), EVT::Simple(dest)) = (self.dag.value_type(operand), dest) else { return None };
        let bits = |lane: MVT| EVT::Simple(lane).size_in_bits();
        let (element, lanes) = (src.element()?, src.lanes()?);
        if lanes % 2 != 0 || bits(element) * 2 >= bits(dest.element()?) {
            return None;
        }
        let legal = |vt: Option<MVT>| vt.is_some_and(|vt| self.tli.types.is_type_legal(EVT::Simple(vt)));
        let doubled = MVT::integer(bits(element) * 2).and_then(|wide| MVT::vector(wide, lanes));
        let doubled_half = doubled.and_then(|vt| MVT::vector(vt.element()?, lanes / 2));
        let src_half = MVT::vector(element, lanes / 2);
        (legal(Some(src)) && !legal(src_half) && legal(doubled) && legal(doubled_half))
            .then(|| EVT::Simple(doubled.unwrap()))
    }

    /// `SelectionDAG::SplitVectorOperand` for a legal vector: its two halves as
    /// `EXTRACT_SUBVECTOR`s.
    fn split_operand_by_hand(
        &mut self,
        operand: SDValue,
    ) -> Option<(SDValue, SDValue)> {
        let vt = self.dag.value_type(operand);
        let (half, _) = split_dest(vt)?;
        let lanes = u128::from(half.lanes());
        let (zero, mid) = (self.dag.get_constant(0, EVT::I64), self.dag.get_constant(lanes, EVT::I64));
        Some((
            self.dag.get_node(ISD::ExtractSubvector, half, &[operand, zero]),
            self.dag.get_node(ISD::ExtractSubvector, half, &[operand, mid]),
        ))
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
            // `SplitVecRes_ExtendOp`: an extension by more than one doubling of
            // a legal source whose halves are no type is made in two steps,
            // the first on the whole vector.
            op @ (ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend) => {
                let operand = self.dag.operand(value, 0);
                if let Some(widened) = self.incremental_extend_type(operand, vt) {
                    let step = self.dag.get_node_flags(op, widened, &[operand], flags);
                    let (lo, hi) =
                        self.split_operand_by_hand(step).ok_or_else(|| unsupported("split extend of a lone lane"))?;
                    (self.dag.get_node_flags(op, lo_vt, &[lo], flags), self.dag.get_node_flags(op, hi_vt, &[hi], flags))
                } else {
                    let (lo, hi) = if self.action(self.dag.value_type(operand)) == TypeAction::SplitVector {
                        self.split_vector(operand)
                    } else {
                        self.split_operand_by_hand(operand)
                            .ok_or_else(|| unsupported("split extend of an operand with no half"))?
                    };
                    (self.dag.get_node_flags(op, lo_vt, &[lo], flags), self.dag.get_node_flags(op, hi_vt, &[hi], flags))
                }
            }
            op if is_conversion(op) => {
                let ops = self.dag.node(value).ops.clone();
                let (lo, hi) = if self.action(self.dag.value_type(ops[0])) == TypeAction::SplitVector {
                    self.split_vector(ops[0])
                } else {
                    self.split_operand_by_hand(ops[0]).ok_or_else(|| unsupported("split conversion of a lone lane"))?
                };
                let (mut los, mut his) = (vec![lo], vec![hi]);
                los.extend_from_slice(&ops[1..]);
                his.extend_from_slice(&ops[1..]);
                (self.dag.get_node_flags(op, lo_vt, &los, flags), self.dag.get_node_flags(op, hi_vt, &his, flags))
            }
            // `ConcatVectors` of two halves is its operands.
            ISD::ConcatVectors => (self.dag.operand(value, 0), self.dag.operand(value, 1)),
            // `SplitVecRes_EXTRACT_SUBVECTOR`: each half is a subvector of the
            // same vector, the second one half on.
            ISD::ExtractSubvector => {
                let (vector, at) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let Some(start) = self.dag.constant(at) else { return Err(unsupported("split result")) };
                let mid = self.dag.get_constant(start + u128::from(lo_vt.lanes()), EVT::I64);
                (
                    self.dag.get_node(ISD::ExtractSubvector, lo_vt, &[vector, at]),
                    self.dag.get_node(ISD::ExtractSubvector, hi_vt, &[vector, mid]),
                )
            }
            // `SplitVecRes_VECTOR_SHUFFLE`: the four halves of the two inputs;
            // each result half is a shuffle of the (at most two) of them its
            // lanes read, or else a `BUILD_VECTOR` of extracted lanes.
            //
            // DEVIATE VL2 (owner llrm-arm64-vec, exit 2026-11-27): the pin's
            // `ProcessShuffleMasks` and its peeking through input shuffles and
            // `CONCAT_VECTORS` are not translated; a half that reads three or
            // four of the inputs is built lane by lane.
            ISD::VectorShuffle => {
                let (n0, n1) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let (a_lo, a_hi) = self.split_vector(n0);
                let (b_lo, b_hi) = self.split_vector(n1);
                let inputs = [a_lo, a_hi, b_lo, b_hi];
                let mask = self.dag.shuffle_mask(value).to_vec();
                let new_elts = lo_vt.lanes() as i32;
                let mut halves = Vec::new();
                for (half, vt) in [(0usize, lo_vt), (1, hi_vt)] {
                    let part = &mask[half * new_elts as usize..(half + 1) * new_elts as usize];
                    let mut used: Vec<i32> = Vec::new();
                    for &lane in part.iter().filter(|&&lane| lane >= 0) {
                        if !used.contains(&(lane / new_elts)) {
                            used.push(lane / new_elts);
                        }
                    }
                    let node = if used.len() <= 2 {
                        let slot = |source: i32| used.iter().position(|&seen| seen == source).expect("a used source") as i32;
                        let local: Vec<i32> = part
                            .iter()
                            .map(|&lane| if lane < 0 { -1 } else { slot(lane / new_elts) * new_elts + lane % new_elts })
                            .collect();
                        let first = used.first().map_or_else(|| self.dag.get_undef(vt), |&source| inputs[source as usize]);
                        let second = used.get(1).map_or_else(|| self.dag.get_undef(vt), |&source| inputs[source as usize]);
                        self.dag.get_vector_shuffle(vt, first, second, &local)
                    } else {
                        let element = vt.element();
                        let lanes: Vec<SDValue> = part
                            .iter()
                            .map(|&lane| {
                                if lane < 0 {
                                    return self.dag.get_undef(element);
                                }
                                let index = self.dag.get_constant((lane % new_elts) as u128, EVT::I64);
                                self.dag.get_node(ISD::ExtractElement, element, &[inputs[(lane / new_elts) as usize], index])
                            })
                            .collect();
                        self.dag.get_build_vector(vt, &lanes)
                    };
                    halves.push(node);
                }
                (halves[0], halves[1])
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

    /// The truncation whose result lane is under half its operand's: the pin
    /// does it in two steps (`SplitVecOp_TruncateHelper`).
    fn truncates_by_more_than_half(
        &self,
        node: SDValue,
        operand: SDValue,
    ) -> bool {
        let (result, source) = (self.dag.value_type(node), self.dag.value_type(operand));
        result.is_vector() && source.is_vector() && result.element().size_in_bits() * 2 < source.element().size_in_bits()
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
            // `SplitVecOp_TruncateHelper`: a truncation to less than half the
            // lane is two: each half to half the lane (a legal result
            // type), joined, then the rest.
            ISD::Truncate if at == 0 && self.truncates_by_more_than_half(node, operand) => {
                let result_vt = self.dag.value_type(node);
                let (lo, hi) = self.split_vector(operand);
                let lanes = self.dag.value_type(lo).lanes();
                let half_bits = self.dag.value_type(operand).element().size_in_bits() / 2;
                let half = MVT::integer(half_bits)
                    .and_then(|element| MVT::vector(element, lanes))
                    .map(EVT::Simple)
                    .ok_or_else(|| unsupported("split truncation with no half-lane type"))?;
                let inter = MVT::integer(half_bits)
                    .and_then(|element| MVT::vector(element, lanes * 2))
                    .map(EVT::Simple)
                    .ok_or_else(|| unsupported("split truncation with no half-lane type"))?;
                let (lo, hi) = (self.dag.get_node(ISD::Truncate, half, &[lo]), self.dag.get_node(ISD::Truncate, half, &[hi]));
                let joined = self.dag.get_node(ISD::ConcatVectors, inter, &[lo, hi]);
                self.dag.get_node(ISD::Truncate, result_vt, &[joined])
            }
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
            // `SplitVecOp_EXTRACT_SUBVECTOR`: the result is legal; it is in
            // the half the index names.
            ISD::ExtractSubvector if at == 0 => {
                let index = self.dag.operand(node, 1);
                let Some(start) = self.dag.constant(index) else {
                    return Err(unsupported("split extract of a subvector at a variable index"));
                };
                let (lo, hi) = self.split_vector(operand);
                let low_lanes = u128::from(self.dag.value_type(lo).lanes());
                let vt = self.dag.value_type(node);
                if start < low_lanes {
                    self.dag.get_node(ISD::ExtractSubvector, vt, &[lo, index])
                } else {
                    let rest = self.dag.get_constant(start - low_lanes, EVT::I64);
                    self.dag.get_node(ISD::ExtractSubvector, vt, &[hi, rest])
                }
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
            _ => return Err(unsupported("split operand")),
        };
        if result.node == n {
            return Ok(true);
        }
        self.replace_value_with(node, result);
        Ok(false)
    }
}
