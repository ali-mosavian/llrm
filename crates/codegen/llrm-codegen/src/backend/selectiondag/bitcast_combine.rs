//! `visitBITCAST`'s load fold: `(conv (load x))` is `(load (conv*)x)`, so a
//! float argument read as its bits is one integer load, not `fld; fstp; mov`.
//! And `foldBitcastedFPLogic` (a port): a sign-bit `xor`/`and`/`or` of a
//! float's bits, cast back, is `fneg`/`fabs`/`fneg (fabs)` (`__negdf2` on wasm:
//! `f64.neg`, 7 B, not reinterpret, const, xor, reinterpret, 20 B).
//!
//! DEVIATE SD548: `foldBitcastedFPLogic` takes the operand only when it is a
//! cast from the result type; the target's `isFNegFree`/`isFAbsFree` second
//! way (no target here answers true) and vector splat masks are not ported.
//! Owner softfloat. Exit: when a target answers `isFNegFree`.
//!
//! `visitBITCAST`'s constant fold (a port): a cast of a `build_vector` of
//! constants and undefs to another vector type is the `build_vector` of the
//! recast bits (`ConstantFoldBITCASTofBUILD_VECTOR`, `getConstantRawBits`,
//! `recastRawBits`). Type legalization casts the widened zero of a
//! `<2 x i8>` store to the stored scalar's vector; folded, the lane is a
//! constant and the stores of zeros merge (`mov dword [rdi+2], 0`, not
//! `xorps; movd; mov; mov`).
//!
//! DEVIATE SD251: only scalar casts of a plain load (no vector, `freeze`,
//! `AssertNoFPClass` or range metadata; `isLoadBitCastBeneficial` is true for
//! scalars on x86). Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitBITCAST`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L18314));
//! `foldBitcastedFPLogic`
//! ([L18247](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L18247)).

use super::dag::{MemBase, MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_bitcast(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if self.dag.is_undef(self.dag.operand(node, 0)) {
            let vt = self.dag.value_type(node);
            return Some(self.dag.get_undef(vt));
        }
        if let Some(constant) = self.fold_bitcast_of_constant_build_vector(node) {
            return Some(constant);
        }
        if let Some(fp) = self.fold_bitcasted_fp_logic(node) {
            return Some(fp);
        }
        let (vt, load) = (self.dag.value_type(node), self.dag.operand(node, 0));
        if self.dag.opcode(load) != ISD::Load || load.res != 0 {
            return None;
        }
        // DEVIATE SD256: a shared load is read again as the integer when every
        // user of the cast is a call argument's store.
        let shared = !self.dag.has_one_use(load.node);
        if shared && !self.only_call_argument_stores(node) {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data.clone() else { return None };
        if info.ext.is_some() || !matches!(vt, EVT::Simple(_)) {
            return None;
        }
        // "(LegalOperations || !LN0->isSimple()) && !isOperationLegal(LOAD,
        // VT)"
        let legal_operations = self.level >= CombineLevel::AfterLegalizeVectorOps;
        if (legal_operations || info.volatile) && !self.tli.is_some_and(|tli| tli.is_operation_legal(ISD::Load, vt)) {
            return None;
        }
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let cast = MemInfo { mem_vt: vt, ..info };
        let new = self.dag.get_load(vt, chain, ptr, cast);
        let old_chain = SDValue { node: load.node, res: 1 };
        if shared {
            // What was ordered after the float load is ordered after both
            // loads (`ReplaceAllUsesOfValueWith` of the old chain by a
            // `TokenFactor` that still reads it).
            let both = self.dag.get_token_factor(&[old_chain, SDValue { node: new.node, res: 1 }]);
            self.replace_value(old_chain, both);
            self.dag.update_node_operands(both.node, &[old_chain, SDValue { node: new.node, res: 1 }]);
        } else {
            self.replace_value(old_chain, SDValue { node: new.node, res: 1 });
        }
        Some(new)
    }

    /// "If the input is a BUILD_VECTOR with all constant elements, fold this
    /// now. Only do this before legalize types, unless both types are integer
    /// and the scalar type is legal. Only do this before legalize ops."
    fn fold_bitcast_of_constant_build_vector(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (vt, source) = (self.dag.value_type(node), self.dag.operand(node, 0));
        if !vt.is_vector() || self.dag.opcode(source) != ISD::BuildVector || !self.dag.has_one_use(source.node) {
            return None;
        }
        let source_vt = self.dag.value_type(source);
        let element = vt.element();
        if self.level >= CombineLevel::AfterLegalizeTypes {
            let legal_element = self.tli.is_some_and(|tli| tli.types.is_type_legal(element));
            if self.legal_operations() || !element.is_integer() || !source_vt.element().is_integer() || !legal_element {
                return None;
            }
        }
        let (source_bits, element_bits) = (source_vt.element().size_in_bits(), element.size_in_bits());
        if source_bits > 128 || element_bits > 128 || source_bits * source_vt.lanes() != element_bits * vt.lanes() {
            return None;
        }
        // `getConstantRawBits`: "Early-out if this contains anything but
        // Undef/Constant/ConstantFP".
        let mut raw = Vec::with_capacity(source_vt.lanes() as usize);
        for lane in 0..source_vt.lanes() as usize {
            let op = self.dag.operand(source, lane);
            raw.push(if self.dag.is_undef(op) {
                None
            } else {
                let bits = self.dag.constant(op).or_else(|| self.dag.fp_constant(op))?;
                Some(bits & super::fold::mask(source_bits))
            });
        }
        let recast = recast_raw_bits(self.dag.config().big_endian, source_bits, element_bits, &raw);
        let ops: Vec<SDValue> = recast
            .into_iter()
            .map(|bits| match bits {
                None => self.dag.get_undef(element),
                Some(bits) if element.is_floating_point() => self.dag.get_constant_fp(bits, element),
                Some(bits) => self.dag.get_constant(bits, element),
            })
            .collect();
        let folded = self.dag.get_build_vector(vt, &ops);
        for op in ops {
            self.add_to_worklist(op.node);
        }
        Some(folded)
    }

    /// Whether every user of `cast` stores it to a call's argument slot.
    fn only_call_argument_stores(
        &self,
        cast: SDValue,
    ) -> bool {
        let mut users = self.dag.users(cast).peekable();
        users.peek().is_some()
            && users.all(|user| {
                let user = SDValue { node: user, res: 0 };
                self.dag.opcode(user) == ISD::Store
                    && matches!(
                        self.dag.node(user).data,
                        NodeData::Mem(MemInfo { base: MemBase::CallArgument(_), .. })
                    )
            })
    }

    /// `foldBitcastedFPLogic`: `(bitcast fp (xor (bitcast int x) signmask))` is
    /// `(fneg x)`; `and` with the complement of the mask is `fabs`, `or` with
    /// the mask is `fneg (fabs x)`.
    fn fold_bitcasted_fp_logic(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let logic = self.dag.operand(node, 0);
        let width = self.dag.value_type(logic).size_in_bits();
        if !vt.is_floating_point() || vt.size_in_bits() != width || width > 64 {
            return None;
        }
        let sign = 1u128 << (width - 1);
        let all = (1u128 << width) - 1;
        let (fp_op, mask) = match self.dag.opcode(logic) {
            ISD::And => (ISD::FAbs, all & !sign),
            ISD::Xor => (ISD::FNeg, sign),
            ISD::Or => (ISD::FAbs, sign),
            _ => return None,
        };
        let legal_operations = self.level >= CombineLevel::AfterLegalizeVectorOps;
        if legal_operations && !self.tli.is_some_and(|tli| tli.is_operation_legal(fp_op, vt)) {
            return None;
        }
        if self.dag.constant(self.dag.operand(logic, 1))? & all != mask {
            return None;
        }
        let bits = self.dag.operand(logic, 0);
        if self.dag.opcode(bits) != ISD::BitCast || self.dag.value_type(self.dag.operand(bits, 0)) != vt {
            return None;
        }
        let fp = self.dag.get_node(fp_op, vt, &[self.dag.operand(bits, 0)]);
        Some(if self.dag.opcode(logic) == ISD::Or { self.dag.get_node(ISD::FNeg, vt, &[fp]) } else { fp })
    }
}

/// `BuildVectorSDNode::recastRawBits`: the lanes of `source_bits` bits
/// (`None`: undef) as lanes of `element_bits` bits. A lane is undef when all
/// its bits are; the undef bits of another are zeros.
fn recast_raw_bits(
    big_endian: bool,
    source_bits: u32,
    element_bits: u32,
    source: &[Option<u128>],
) -> Vec<Option<u128>> {
    let total = source_bits as usize * source.len();
    let at =
        |lane: usize, bits: u32| if big_endian { total - (lane + 1) * bits as usize } else { lane * bits as usize };
    let mut value = vec![false; total];
    let mut defined = vec![false; total];
    for (lane, bits) in source.iter().enumerate() {
        let from = at(lane, source_bits);
        if let Some(bits) = bits {
            for bit in 0..source_bits as usize {
                value[from + bit] = bits >> bit & 1 == 1;
                defined[from + bit] = true;
            }
        }
    }
    (0..total / element_bits as usize)
        .map(|lane| {
            let from = at(lane, element_bits);
            let span = from..from + element_bits as usize;
            span.clone()
                .any(|bit| defined[bit])
                .then(|| span.enumerate().fold(0_u128, |sum, (i, bit)| sum | u128::from(value[bit]) << i))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::recast_raw_bits;

    #[test]
    fn test_lanes_merge_with_lane_zero_low_on_little_endian_and_high_on_big() {
        let bytes = [Some(0x11), Some(0x22), Some(0x33), Some(0x44)];
        assert_eq!(recast_raw_bits(false, 8, 16, &bytes), [Some(0x2211), Some(0x4433)]);
        assert_eq!(recast_raw_bits(true, 8, 16, &bytes), [Some(0x1122), Some(0x3344)]);
        assert_eq!(recast_raw_bits(false, 8, 32, &bytes), [Some(0x4433_2211)]);
        assert_eq!(recast_raw_bits(true, 8, 32, &bytes), [Some(0x1122_3344)]);
    }

    #[test]
    fn test_a_lane_splits_low_part_first_on_little_endian_and_high_first_on_big() {
        let words = [Some(0xAABB_CCDD)];
        assert_eq!(recast_raw_bits(false, 32, 8, &words), [Some(0xDD), Some(0xCC), Some(0xBB), Some(0xAA)]);
        assert_eq!(recast_raw_bits(true, 32, 8, &words), [Some(0xAA), Some(0xBB), Some(0xCC), Some(0xDD)]);
    }

    /// `<2 x i8> zeroinitializer` widened to 16 lanes (the rest undef), read as
    /// `<8 x i16>`: lane 0 is the zero the store of the pair needs; the
    /// lanes of undef stay undef.
    #[test]
    fn test_an_all_undef_lane_stays_undef_and_a_half_undef_lane_reads_zero_there() {
        let mut widened = vec![None; 16];
        widened[0] = Some(0);
        widened[1] = Some(0);
        widened[2] = Some(0xFF);
        let words = recast_raw_bits(false, 8, 16, &widened);
        assert_eq!(words[0], Some(0));
        assert_eq!(words[1], Some(0x00FF), "the undef upper byte is zero");
        assert!(words[2..].iter().all(Option::is_none));
        let big = recast_raw_bits(true, 8, 16, &widened);
        assert_eq!((big[0], big[1]), (Some(0), Some(0xFF00)), "big endian: lane 0 is the high byte of word 0");
    }
}
