//! `ZERO_EXTEND_VECTOR_INREG`: the node a shuffle that zero-fills lanes is
//! (`combineShuffleToZeroExtendVectorInReg`), and what folds into it.
//!
//! `combineShuffleToZeroExtendVectorInReg`
//! ([DAGCombiner.cpp#L28913](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L28913)),
//! `canCombineShuffleToExtendVectorInreg`
//! ([L28837](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L28837)),
//! `visitEXTEND_VECTOR_INREG` and `foldExtendVectorInregToExtendOfSubvector`
//! ([L17596](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L17596)).
//!
//! # How it works
//! A shuffle's lanes that read a known-zero lane of an operand are marked
//! zero; the mask is widened to the widest lanes it keeps; if each chunk of
//! `Scale` lanes is `{n, zero, ..}` the shuffle is a `ZERO_EXTEND_VECTOR_INREG`
//! of the operand to lanes `Scale` times wider.
//!
//! ```text
//! shuffle<0,z,z,z,1,z,z,z, ..> x:v16i8, zeros  ->  bitcast (zext_inreg:v4i32 x)
//! zext_inreg (concat x, undef, ..)              ->  zext x
//! ```
//!
//! DEVIATE ZXI1 (owner w11, exit 2026-11-27): big-endian targets (the pin
//! has none either), the known-zero lanes of a bitcast or a load,
//! `SimplifyDemandedVectorElts` and the *any*/*sign* forms are not made.
//!
//! # Used by
//! `visit_vector_shuffle`; `DagCombiner::visit` for the node.

use llrm_sdag::mvt::{EVT, MVT};

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::known_bits::known_bits;

/// A shuffle lane that reads a lane known to be zero.
const ZEROABLE: i32 = -2;

/// `widenShuffleMaskElts(Scale, ...)`: the mask over lanes `scale` times wider.
fn widen_shuffle_mask_elts(
    scale: usize,
    mask: &[i32],
) -> Option<Vec<i32>> {
    if mask.len() % scale != 0 {
        return None;
    }
    let mut wide = Vec::with_capacity(mask.len() / scale);
    for slice in mask.chunks(scale) {
        let front = slice[0];
        if front < 0 {
            // "Negative values (undef or other "sentinel" values) must be
            // equal across the entire slice."
            if slice.iter().any(|&at| at != front) {
                return None;
            }
            wide.push(front);
        } else {
            // "A positive mask element must be cleanly divisible", and "the
            // elements of the slice must be consecutive".
            if front as usize % scale != 0 || slice.iter().enumerate().any(|(at, &lane)| lane != front + at as i32) {
                return None;
            }
            wide.push(front / scale as i32);
        }
    }
    Some(wide)
}

/// `getShuffleMaskWithWidestElts`.
fn shuffle_mask_with_widest_elts(mask: &[i32]) -> Vec<i32> {
    let mut current = mask.to_vec();
    for scale in 2..=mask.len() {
        while let Some(wide) = widen_shuffle_mask_elts(scale, &current) {
            current = wide;
        }
    }
    current
}

/// `ShuffleVectorSDNode::commuteMask`.
fn commute_mask(mask: &mut [i32]) {
    let lanes = mask.len() as i32;
    for at in mask.iter_mut().filter(|at| **at >= 0) {
        *at = if *at < lanes { *at + lanes } else { *at - lanes };
    }
}

/// A vector of `lanes` integers of `bits`, when there is such a type.
fn integer_vector(
    bits: u32,
    lanes: u32,
) -> Option<EVT> {
    let element = MVT::integer(bits)?;
    Some(EVT::vector(element, lanes))
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `computeVectorKnownZeroElements`, for the lanes of `value` in `demanded`
    /// (bit `n` is lane `n`): the lanes known to be zero.
    fn known_zero_lanes(
        &self,
        value: SDValue,
        demanded: u128,
    ) -> u128 {
        (0..self.dag.value_type(value).lanes())
            .filter(|&lane| demanded >> lane & 1 == 1)
            .fold(0, |zero, lane| zero | u128::from(self.lane_is_zero(value, lane, 0)) << lane)
    }

    /// `computeKnownBits(value, DemandedElts = lane)` is all zeros, for the
    /// vector nodes that name their lanes.
    fn lane_is_zero(
        &self,
        value: SDValue,
        lane: u32,
        depth: u32,
    ) -> bool {
        if depth > 6 {
            return false;
        }
        let scalar_is_zero = |scalar: SDValue| {
            let bits = self.dag.value_type(value).element().size_in_bits();
            let mask = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
            !self.dag.is_undef(scalar) && known_bits(self.dag, scalar).zero & mask == mask
        };
        match self.dag.opcode(value) {
            ISD::BuildVector => scalar_is_zero(self.dag.operand(value, lane as usize)),
            ISD::ConcatVectors => {
                let part = self.dag.operand(value, 0);
                let width = self.dag.value_type(part).lanes();
                self.lane_is_zero(self.dag.operand(value, (lane / width) as usize), lane % width, depth + 1)
            }
            ISD::InsertVectorElt => match self.dag.constant(self.dag.operand(value, 2)) {
                Some(at) if at == u128::from(lane) => scalar_is_zero(self.dag.operand(value, 1)),
                Some(_) => self.lane_is_zero(self.dag.operand(value, 0), lane, depth + 1),
                None => false,
            },
            _ => false,
        }
    }

    /// `canCombineShuffleToExtendVectorInreg`: the type `Match` finds a
    /// `Scale` for, `lanes` of `bits` being the shuffle's.
    fn can_combine_shuffle_to_extend_vector_inreg(
        &self,
        lane_bits: u32,
        lanes: u32,
        matches: impl Fn(usize) -> bool,
    ) -> Option<EVT> {
        let tli = self.tli?;
        // "Attempt to match a '*_extend_vector_inreg' shuffle, we just search
        // for power-of-2 extensions as they are the most likely."
        let mut scale = 2;
        while scale < lanes {
            if lanes % scale == 0 {
                let out = integer_vector(lane_bits * scale, lanes / scale);
                if let Some(out) = out
                    && tli.types.is_type_legal(out)
                    && (!self.legal_operations() || tli.is_operation_legal_or_custom(ISD::ZeroExtendVectorInreg, out))
                    && matches(scale as usize)
                {
                    return Some(out);
                }
            }
            scale *= 2;
        }
        None
    }

    /// `combineShuffleToZeroExtendVectorInReg`.
    pub(super) fn combine_shuffle_to_zero_extend_vector_inreg(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        // "Perform this really late, because it could eliminate knowledge of
        // undef elements created by this shuffle."
        if self.level >= CombineLevel::AfterLegalizeTypes
            || !vt.element().is_integer()
            || self.dag.config().big_endian
            || self.tli.is_none()
        {
            return None;
        }
        let lanes = vt.lanes() as usize;
        let mask = self.dag.shuffle_mask(node).to_vec();
        let operands = [self.dag.operand(node, 0), self.dag.operand(node, 1)];
        // "Which elements of which operand does this shuffle demand?"
        let mut demanded = [0u128; 2];
        for &at in mask.iter().filter(|&&at| at >= 0) {
            let (operand, lane) = if (at as usize) < lanes { (0, at as usize) } else { (1, at as usize - lanes) };
            demanded[operand] |= 1 << lane;
        }
        let zero = [self.known_zero_lanes(operands[0], demanded[0]), self.known_zero_lanes(operands[1], demanded[1])];
        // "Manifest zeroable element knowledge in the shuffle mask."
        let mut mask = mask;
        let mut zeroable = false;
        for at in mask.iter_mut().filter(|at| **at >= 0) {
            let (operand, lane) = if (*at as usize) < lanes { (0, *at as usize) } else { (1, *at as usize - lanes) };
            if zero[operand] >> lane & 1 == 1 {
                *at = ZEROABLE;
                zeroable = true;
            }
        }
        // "Don't proceed unless we've refined at least one zeroable mask
        // indice ... Proceeding will lead to endless combine loops."
        if !zeroable {
            return None;
        }
        // "The shuffle may be more fine-grained than we want."
        let mut scaled = shuffle_mask_with_widest_elts(&mask);
        let prescale = (mask.len() / scaled.len()) as u32;
        let lane_bits = vt.element().size_in_bits() * prescale;
        let prescaled = integer_vector(lane_bits, scaled.len() as u32)?;
        let tli = self.tli?;
        if !tli.types.is_type_legal(prescaled) && tli.types.is_type_legal(vt) {
            return None;
        }
        // "shuffle<0,z,1,-1> == (v2i64 zero_extend_vector_inreg(v4i32)), but
        // not shuffle<z,z,1,-1> and not shuffle<0,z,z,-1>."
        let is_zero_extend = |scale: usize, mask: &[i32]| {
            mask.chunks(scale)
                .enumerate()
                .all(|(source, chunk)| chunk[0] == source as i32 && chunk[1..].iter().all(|&at| at == ZEROABLE))
        };
        for commuted in [false, true] {
            if commuted {
                commute_mask(&mut scaled);
            }
            let found = self.can_combine_shuffle_to_extend_vector_inreg(lane_bits, scaled.len() as u32, |scale| {
                is_zero_extend(scale, &scaled)
            });
            if let Some(out) = found {
                let operand = operands[usize::from(commuted)];
                let source = self.bitcast_to(prescaled, operand);
                let extended = self.dag.get_node(ISD::ZeroExtendVectorInreg, out, &[source]);
                return Some(self.bitcast_to(vt, extended));
            }
        }
        None
    }

    /// `getBitcast`: the value itself when the type is.
    fn bitcast_to(
        &mut self,
        vt: EVT,
        value: SDValue,
    ) -> SDValue {
        if self.dag.value_type(value) == vt { value } else { self.dag.get_node(ISD::BitCast, vt, &[value]) }
    }

    /// `visitEXTEND_VECTOR_INREG`, for the zero-extending node.
    pub(super) fn visit_zero_extend_vector_inreg(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let source = self.dag.operand(node, 0);
        // "{s/z}ext_vector_inreg(undef) = 0 because the top bits must be the
        // same."
        if self.dag.is_undef(source) {
            let zero = self.dag.get_constant(0, vt.element());
            return Some(self.dag.get_splat_build_vector(vt, zero));
        }
        // `foldExtendVectorInregToExtendOfSubvector`: "our operand must be a
        // one-use CONCAT_VECTORS" "extending exactly one of its operands".
        if !self.dag.has_one_use(source.node) || self.dag.opcode(source) != ISD::ConcatVectors {
            return None;
        }
        let first = self.dag.operand(source, 0);
        let (first_vt, source_vt) = (self.dag.value_type(first), self.dag.value_type(source));
        if first_vt.lanes() != vt.lanes() || first_vt.element() != source_vt.element() {
            return None;
        }
        if self.legal_operations() && !self.tli?.is_operation_legal(ISD::ZeroExtend, vt) {
            return None;
        }
        Some(self.dag.get_node(ISD::ZeroExtend, vt, &[first]))
    }
}
