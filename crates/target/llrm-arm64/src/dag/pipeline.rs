//! The shared DAG stages run for arm64: combine, legalize, select with the
//! AArch64 table, schedule.
//!
//! DEVIATE SD525: no target combines (`PerformDAGCombine` of AArch64 is not
//! translated) and no `PreprocessISelDAG`. A Custom operation is refused (the
//! legalizer would otherwise expand it). Exit: B3.

use llrm_codegen::backend::selectiondag::NodeData;
use llrm_codegen::backend::selectiondag::dag_combiner::CombineHooks;
use llrm_codegen::backend::selectiondag::dag_combiner::CombineLevel;
use llrm_codegen::backend::selectiondag::dag_select::{Refusal, do_instruction_selection_with, legalized_with};
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks as LegalizeHooks;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::{SchedInfo, Unit, schedule};
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_dagisel::addr::AddrDag;
use llrm_dagisel::dag::SDValue as PinValue;
use llrm_dagisel::generated::aarch64::aarch64_apple_m1 as table;
use llrm_dagisel::generated::aarch64::ids::isd as aarch64_isd;
use llrm_dagisel::generated::aarch64::ids::opcode as aarch64_ids;
use llrm_dagisel::generated::aarch64::instrinfo::{self, flag};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::Outcome;
use llrm_dagisel::interp::Selector;
use llrm_lir::registers::RegId;
use llrm_sdag::actions::{Aarch64Features, TargetLowering};

use super::globals::Globals;

/// Combine hooks and legalizer hooks of arm64: none. A Custom operation the
/// table asks for is recorded, and the block refused after legalization.
pub struct Aarch64Lowering {
    pub custom: Option<String>,
    globals: Globals,
    /// AAPCS64 of Linux rather than Darwin's: where a library call's
    /// arguments go.
    linux: bool,
    /// `optsize`: `performMulCombine` leaves a multiply a `madd` takes
    /// (DEVIATE MUL-MADD-OS, see `combine::combine_mul`).
    opt_for_size: bool,
}

impl CombineHooks for Aarch64Lowering {
    /// `AArch64TargetLowering::isTruncateFree(EVT, EVT)`
    /// (AArch64ISelLowering.cpp#L18949 at d1106deb71cc): any narrower scalar
    /// integer.
    fn is_truncate_free(
        &self,
        from: EVT,
        to: EVT,
    ) -> bool {
        !from.is_vector()
            && !to.is_vector()
            && from.is_integer()
            && to.is_integer()
            && from.size_in_bits() > to.size_in_bits()
    }

    /// `AArch64TargetLowering::isZExtFree(EVT, EVT)`
    /// (AArch64ISelLowering.cpp#L18991 at d1106deb71cc): `i32` to `i64`.
    fn is_zext_free(
        &self,
        from: EVT,
        to: EVT,
    ) -> bool {
        !from.is_vector() && !to.is_vector() && from.size_in_bits() == 32 && to.size_in_bits() == 64
    }

    fn has_and_not(
        &self,
        vt: EVT,
        _constant: bool,
    ) -> bool {
        // `AArch64TargetLowering::hasAndNot`: `bic` takes any scalar integer.
        vt.is_integer()
    }

    fn is_indexed_legal(
        &self,
        _is_load: bool,
        vt: EVT,
        _post: bool,
    ) -> bool {
        // `setIndexedLoadAction`/`setIndexedStoreAction` of `PRE_INC` and
        // `POST_INC` (the other modes are never made) for i8 to i64 and
        // f32/f64.
        use llrm_codegen::backend::selectiondag::mvt::MVT;
        matches!(
            vt,
            EVT::Simple(MVT::I8 | MVT::I16 | MVT::I32 | MVT::I64 | MVT::F32 | MVT::F64)
        )
    }

    fn indexed_address_parts(
        &self,
        dag: &SelectionDAG,
        access: SDValue,
        address: SDValue,
        post: bool,
    ) -> Option<(SDValue, i64)> {
        // `getPreIndexedAddressParts`/`getPostIndexedAddressParts` over
        // `getIndexedAddressParts`.
        let NodeData::Mem(info) = dag.node(access).data else { return None };
        if info.volatile || !matches!(dag.opcode(address), ISD::Add | ISD::Sub) {
            return None;
        }
        let base = dag.operand(address, 0);
        let rhs = dag.operand(address, 1);
        dag.constant(rhs)?;
        let bits = dag.value_type(rhs).size_in_bits();
        let raw = dag.constant(rhs)?;
        let shift = 128 - bits;
        let mut offset = (((raw << shift) as i128) >> shift) as i64;
        if dag.opcode(address) == ISD::Sub {
            offset = offset.wrapping_neg();
        }
        // "All of the indexed addressing mode instructions take a signed 9 bit
        // immediate offset."
        if !(-256..256).contains(&offset) {
            return None;
        }
        if post {
            // "Post-indexing updates the base, so it's not a valid transform if
            // that's not the same as the load's pointer."
            let ptr = dag.operand(access, if dag.opcode(access) == ISD::Load { 1 } else { 2 });
            if ptr != base {
                return None;
            }
        }
        Some((base, offset))
    }

    fn is_legal_addressing_mode(
        &self,
        vt: EVT,
        base_offs: i64,
        scale: i64,
    ) -> bool {
        // `AArch64TargetLowering::isLegalAddressingMode`, no global.
        let size = i64::from(vt.size_in_bits().div_ceil(8));
        if scale == 0 {
            // [reg + imm]: the unscaled signed 9 bits, or the scaled
            // unsigned 12.
            return (-256..256).contains(&base_offs)
                || (size > 0 && base_offs % size == 0 && (0..4096).contains(&(base_offs / size)));
        }
        // [reg + reg] and [reg + reg * size].
        base_offs == 0 && (scale == 1 || scale == size)
    }

    fn canonicalize_ge_le(&self) -> bool {
        true
    }

    fn shift_amount_type(
        &self,
        _shifted: EVT,
    ) -> EVT {
        EVT::I64
    }

    fn widen_multiply_high_any_width(&self) -> bool {
        true
    }

    fn sub_algebra_folds(&self) -> bool {
        true
    }

    fn widened_unsigned_divide_type(
        &self,
        vt: EVT,
    ) -> Option<EVT> {
        (vt == EVT::I32).then_some(EVT::I64)
    }

    fn zero_extend_i32_to_i64_free(&self) -> bool {
        true
    }

    fn fold_xor_of_and_with_mask(&self) -> bool {
        true
    }

    fn fold_sext_in_reg_of_load(&self) -> bool {
        true
    }

    fn merge_stores_via_trunc_store(&self) -> bool {
        true
    }

    fn convert_select_of_constants_to_math(
        &self,
        _vt: EVT,
    ) -> bool {
        false
    }

    fn reduce_load_width_through_freeze(&self) -> bool {
        true
    }

    fn fold_or_absorption(&self) -> bool {
        true
    }

    fn fold_sra_of_trunc_of_shift(&self) -> bool {
        true
    }

    fn store_fp_constant_as_i64(&self) -> bool {
        true
    }

    fn should_transform_signed_truncation_check(
        &self,
        vt: EVT,
        kept_bits: u32,
    ) -> bool {
        let ok = |bits: u32| matches!(bits, 8 | 16 | 32 | 64);
        vt.is_integer() && ok(vt.size_in_bits()) && ok(kept_bits)
    }

    /// `AArch64TargetLowering::isTruncateFree(EVT, EVT)` (an integer narrowing)
    /// and `isZExtFree(EVT, EVT)` (32 to 64 bits only, as a `w` write clears
    /// the upper half):
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L17540>
    fn free_narrowing(
        &self,
        wide: EVT,
        narrow: EVT,
    ) -> bool {
        narrow.is_integer() && wide.is_integer() && narrow.size_in_bits() == 32 && wide.size_in_bits() == 64
    }

    /// `AArch64TargetLowering::isNarrowingProfitable`: `isTypeLegal(DestVT) &&
    /// DestVT.isVector()`. Vectors are deferred, so no scalar narrows.
    fn narrowing_profitable(
        &self,
        _wide: EVT,
        _narrow: EVT,
    ) -> bool {
        false
    }

    fn reassociate_compare_pairs(&self) -> bool {
        true
    }

    fn select_to_select_cc(
        &self,
        vt: EVT,
    ) -> bool {
        vt == EVT::I32 || vt == EVT::I64 || (vt.is_floating_point() && vt.size_in_bits() >= 32)
    }

    fn fold_mul_of_add_with_constant(&self) -> bool {
        true
    }

    /// `AArch64TargetLowering::isMulAddWithConstProfitable`: not when `c1` is
    /// an add immediate and `c1*c2` is not but takes more than one
    /// instruction to build.
    fn is_mul_add_with_const_profitable(
        &self,
        vt: EVT,
        c1: u128,
        c2: u128,
    ) -> bool {
        let bits = vt.size_in_bits();
        if bits > 64 {
            return true;
        }
        let sign_extend = |value: u128| (((value as u64) << (64 - bits)) as i64) >> (64 - bits);
        let c1_c2 = c1.wrapping_mul(c2);
        let (c1, product) = (sign_extend(c1), sign_extend(c1_c2));
        let legal = |imm: i64| imm != i64::MIN && crate::tti::is_legal_arith_immediate(imm.unsigned_abs());
        if !legal(c1) || legal(product) {
            return true;
        }
        let width = if bits <= 32 { 32 } else { 64 };
        let unsigned = (c1_c2 & (u128::MAX >> (128 - bits))) as u64;
        crate::expand_imm::mov_imm(unsigned, width, crate::mc::inst::Gpr(0)).len() <= 1
    }

    fn convert_setcc_logic_to_bitwise_logic(&self) -> bool {
        false
    }

    fn push_freeze_through_operands(&self) -> bool {
        true
    }

    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        matches!(
            op,
            ISD::Mul | ISD::Add | ISD::Sub | ISD::And | ISD::Or | ISD::SetCC | ISD::GlobalAddress | ISD::Truncate
        )
    }

    fn target_shrink_demanded_constant(
        &mut self,
        dag: &mut SelectionDAG,
        op: SDValue,
        demanded: u128,
        level: CombineLevel,
    ) -> Option<Option<SDValue>> {
        super::combine::shrink_demanded_constant(dag, op, demanded, level)
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        level: CombineLevel,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::Mul => super::combine::combine_mul(dag, node, level, self.opt_for_size),
            ISD::GlobalAddress => self.global_address_combine(dag, node),
            ISD::Truncate => super::vector::truncate_combine(dag, node),
            ISD::Target(aarch64_isd::DUP) => {
                super::vector::dup_combine(dag, node, level == CombineLevel::AfterLegalizeDag)
            }
            ISD::Target(aarch64_isd::SMULL | aarch64_isd::UMULL) => super::vector::mull_combine(
                dag,
                node,
                matches!(
                    level,
                    CombineLevel::BeforeLegalizeTypes | CombineLevel::AfterLegalizeTypes
                ),
            ),
            ISD::Add => super::combine::add_combine_for_shifted_operands(dag, node),
            ISD::Sub => super::combine::conditional_sub(dag, node),
            ISD::SetCC => super::combine::or_xor_chain(dag, node),
            ISD::And | ISD::Or if level == CombineLevel::AfterLegalizeDag => {
                super::ccmp::combine_and_or_csel(dag, node)
            }
            _ => None,
        }
    }
}

impl LegalizeHooks for Aarch64Lowering {
    fn stack_pointer(&self) -> Option<(llrm_lir::registers::RegId, u64)> {
        Some((crate::reg::SP, 16))
    }

    /// `makeLibCall` through `LowerCall`: the routine is a target external
    /// symbol, the chain the entry node (the result is the value; nothing
    /// orders the call after a store).
    fn make_lib_call(
        &mut self,
        dag: &mut SelectionDAG,
        name: &'static str,
        args: &[SDValue],
        ret: EVT,
        signed: bool,
    ) -> Option<SDValue> {
        use llrm_codegen::backend::selectiondag::target_calling::{Ext, In, Out, TargetCalling};
        let ext = if signed { Ext::Sign } else { Ext::Zero };
        // `getCopyToParts` of an `i128`: its two halves, which AAPCS64 passes
        // in a register pair, low half first (`split`).
        let quad = EVT::integer(128);
        let mut outs: Vec<Out> = Vec::new();
        for &value in args {
            let arg_vt = dag.value_type(value);
            let pieces = if arg_vt == quad {
                let (zero, one) = (dag.get_constant(0, EVT::I64), dag.get_constant(1, EVT::I64));
                vec![
                    dag.get_node(ISD::ExtractElement, EVT::I64, &[value, zero]),
                    dag.get_node(ISD::ExtractElement, EVT::I64, &[value, one]),
                ]
            } else {
                vec![value]
            };
            let split = pieces.len() > 1;
            outs.extend(pieces.into_iter().map(|value| Out {
                value,
                ext,
                pointer: false,
                arg_vt,
                split,
                variadic: false,
                sret: false,
                byval: None,
            }));
        }
        let ins: Vec<In> = if ret == quad {
            vec![
                In { vt: EVT::I64, pointer: false, arg_vt: ret, split: true, sret: false, ext: Ext::Any, byval: None };
                2
            ]
        } else {
            vec![In { vt: ret, pointer: false, arg_vt: ret, split: false, sret: false, ext: Ext::Any, byval: None }]
        };
        let calling = super::call::Aarch64Calling { linux: self.linux };
        calling.accepts(dag, llrm_codegen::backend::selectiondag::target_calling::Site::Call, &outs, &ins).ok()?;
        let callee = dag.get_external_symbol(EVT::I64, name);
        let root = dag.entry_node();
        let (_chain, values) = calling.lower_call(dag, root, callee, &outs, &ins);
        if ret == quad {
            // `getCopyFromParts`: the halves joined.
            return Some(dag.get_node(ISD::BuildPair, quad, &[*values.first()?, *values.get(1)?]));
        }
        values.first().copied()
    }

    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        // `f128`: the routine of its conversion or compare (`LowerF128Call`)
        {
            let mut call = |dag: &mut SelectionDAG, name: &'static str, args: &[SDValue], ret: EVT| {
                self.make_lib_call(dag, name, args, ret, false)
            };
            let handled =
                super::f128::convert(dag, node, &mut call).or_else(|| super::f128::compare(dag, node, &mut call));
            if let Some(result) = handled {
                if result.is_none() {
                    self.custom.get_or_insert_with(|| format!("custom lowering of f128 {:?}", dag.node(node).opcode));
                }
                return result;
            }
        }
        let lowered = match dag.opcode(node) {
            ISD::GlobalAddress => self.global_address(dag, node),
            ISD::ConstantPool => super::lowering::lower_constant_pool(dag, node),
            ISD::JumpTable => super::lowering::lower_jump_table(dag, node),
            ISD::BrJT => super::lowering::lower_br_jt(dag, node),
            ISD::BrInd => super::lowering::lower_br_ind(dag, node),
            ISD::SetCC => super::lowering::lower_setcc(dag, node),
            ISD::Select => super::lowering::lower_select(dag, node),
            ISD::SelectCC => super::lowering::lower_select_cc_node(dag, node),
            ISD::Xor => super::lowering::lower_xor(dag, node),
            ISD::SAddO | ISD::UAddO | ISD::SSubO | ISD::USubO | ISD::SMulO | ISD::UMulO => {
                super::lowering::lower_xalu(dag, node)
            }
            ISD::Abs | ISD::AbsMinPoison => super::lowering::lower_abs(dag, node),
            // Declined for a type the pin's lowering does not take.
            ISD::UAddOCarry | ISD::USubOCarry | ISD::SAddOCarry | ISD::SSubOCarry => {
                return super::lowering::lower_add_sub_carry(dag, node);
            }
            ISD::SetCCCarry => return super::lowering::lower_setcc_carry(dag, node),
            // `LowerShiftParts`
            // ([L13980](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L13980)).
            ISD::ShlParts | ISD::SrlParts | ISD::SraParts => {
                let (lo, hi) =
                    llrm_codegen::backend::selectiondag::expand_funnel::expand_shift_parts(dag, node, EVT::I32);
                let vt = dag.value_type(lo);
                Some(dag.get_node_multi(ISD::MergeValues, &[vt, vt], &[lo, hi]))
            }
            // `LowerLOAD` and `LowerSTORE`: an `i128` access is the pin's own
            // only when it is volatile or atomic
            // (`ldxp`/`stxp`, `LowerStore128`); every other is declined, and
            // the legalizer splits it.
            ISD::Load | ISD::Store => match dag.node(node).data {
                NodeData::Mem(info) if info.volatile || info.ordering.is_some() => None,
                _ => return None,
            },
            ISD::VAArg => super::lowering::lower_vaarg(dag, node),
            // `LowerINSERT_VECTOR_ELT`: a constant lane in range is as it is
            // (`INS`); any other index is declined.
            ISD::InsertVectorElt => {
                let lanes = u128::from(dag.value_type(node).lanes());
                dag.constant(dag.operand(node, 2)).filter(|&lane| lane < lanes).map(|_| node)
            }
            ISD::BuildVector => match super::vector::lower_build_vector(dag, node) {
                Some(done) => Some(done),
                // The pin's `SDValue()` for a constant that is no splat: the
                // legalizer loads it from the constant pool
                // (`ExpandBUILD_VECTOR`).
                None if super::vector::is_constant_build_vector(dag, node) => return None,
                None => None,
            },
            ISD::Mul if dag.value_type(node).is_vector() => super::vector::lower_vector_mul(dag, node),
            ISD::VectorShuffle => super::vector::lower_vector_shuffle(dag, node),
            ISD::Shl | ISD::Srl | ISD::Sra => super::vector::lower_vector_shift(dag, node),
            // Declined: the legalizer expands it, as the pin's `SDValue()`.
            ISD::FShl | ISD::FShr => return super::lowering::lower_funnel_shift(dag, node),
            ISD::BrCond => super::lowering::lower_brcond(dag, node),
            ISD::BrCC => super::lowering::lower_br_cc_node(dag, node),
            ISD::SIntToFp | ISD::UIntToFp if dag.value_type(node).is_vector() => {
                super::vector::lower_vector_int_to_fp(dag, node)
            }
            ISD::FpToSInt | ISD::FpToUInt if dag.value_type(node).is_vector() => {
                super::vector::lower_vector_fp_to_int(dag, node)
            }
            ISD::SIntToFp | ISD::UIntToFp => super::lowering::lower_int_to_fp(dag, node),
            ISD::FpToSInt | ISD::FpToUInt => super::lowering::lower_fp_to_int(dag, node),
            ISD::FpExtend => super::lowering::lower_fp_extend(dag, node),
            ISD::FpRound => super::lowering::lower_fp_round(dag, node),
            ISD::AtomicLoadAnd => super::lowering::lower_atomic_load_and(dag, node),
            // `LowerDYNAMIC_STACKALLOC` answers none off Windows without an
            // inline probe: the legalizer expands it, with the others.
            ISD::DynamicStackAlloc | ISD::StackSave | ISD::StackRestore => return None,
            _ => None,
        };
        if lowered.is_some() {
            return lowered;
        }
        self.custom.get_or_insert_with(|| format!("custom lowering of {:?}", dag.node(node).opcode));
        None
    }
}

impl Aarch64Lowering {
    /// `performGlobalAddressCombine`: when every user adds a constant, fold the
    /// smallest into the symbol's offset, within the object and below 2^20.
    fn global_address_combine(
        &self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        let NodeData::Global { symbol, offset } = dag.node(node).data else { return None };
        let global = self.globals.get(symbol)?;
        if global.got {
            return None;
        }
        let mut min_offset = u64::MAX;
        let users: Vec<u32> = dag.users(node).collect();
        for user in users {
            let user = SDValue { node: user, res: 0 };
            if dag.opcode(user) != ISD::Add {
                return None;
            }
            let constant = dag.constant(dag.operand(user, 0)).or_else(|| dag.constant(dag.operand(user, 1)))?;
            min_offset = min_offset.min(constant as u64);
        }
        let new_offset = min_offset.wrapping_add(offset as u64);
        // Larger than the current offset, or the two forms trade places for
        // ever.
        if new_offset <= offset as u64 || new_offset >= 1 << 20 || new_offset > global.size? {
            return None;
        }
        let vt = dag.value_type(node);
        let folded = dag.get_global_address(vt, symbol, new_offset as i64);
        let by = dag.get_constant(u128::from(min_offset), vt);
        Some(dag.get_node(ISD::Sub, vt, &[folded, by]))
    }

    /// `LowerGlobalAddress` for the small code model: a GOT load for a symbol
    /// from another object, else `ADRP` and `ADDlow` of the symbol.
    fn global_address(
        &self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        let data = dag.node(node);
        let (ISD::GlobalAddress, NodeData::Global { symbol, offset }) = (data.opcode, data.data) else { return None };
        let vt = dag.value_type(node);
        if self.globals.get(symbol)?.got {
            let target = dag.get_target_global_address(vt, symbol, 0);
            let loaded = dag.get_node(ISD::Target(aarch64_isd::LOADgot), vt, &[target]);
            return Some(if offset == 0 {
                loaded
            } else {
                let by = dag.get_constant(offset as u128, vt);
                dag.get_node(ISD::Add, vt, &[loaded, by])
            });
        }
        let target = dag.get_target_global_address(vt, symbol, offset);
        let page = dag.get_node(ISD::Target(aarch64_isd::ADRP), vt, &[target]);
        Some(dag.get_node(ISD::Target(aarch64_isd::ADDlow), vt, &[page, target]))
    }
}

/// What the scheduler reads of the AArch64 instruction descriptions.
pub struct Aarch64Sched;

impl SchedInfo for Aarch64Sched {
    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters::pin_register(&*super::ids::IDS, id)
    }

    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        instrinfo::desc(opcode as u16).map_or(0, |desc| usize::from(desc.num_defs))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        instrinfo::desc(opcode as u16).map_or(&[], |desc| desc.implicit_defs)
    }

    fn is_call(
        &self,
        opcode: u32,
    ) -> bool {
        instrinfo::desc(opcode as u16).is_some_and(|desc| desc.has(flag::CALL))
    }

    fn call_frame_setup(&self) -> Option<u32> {
        Some(u32::from(aarch64_ids::ADJCALLSTACKDOWN))
    }

    fn call_frame_destroy(&self) -> Option<u32> {
        Some(u32::from(aarch64_ids::ADJCALLSTACKUP))
    }

    fn source_order(&self) -> bool {
        true
    }

    /// NZCV is copied only through a GPR.
    fn has_cross_copy_class(
        &self,
        reg: u16,
    ) -> bool {
        reg == llrm_dagisel::generated::aarch64::ids::reg::NZCV
    }
}

/// The subtarget features of `apple-m1` that the action table reads
/// (`dagisel.toml` `cpus`).
const APPLE_M1: Aarch64Features = Aarch64Features { cssc: false, lse: true, outline_atomics: false, lse2: true };

thread_local! {
    /// The action tables built so far, by the subtarget features and
    /// `shouldOptForSize` they were built for: the pin builds
    /// `AArch64TargetLowering` once per subtarget, not per block (2.3 M of
    /// rectwo's isel in 17 blocks). Two subtargets in one process get two.
    static ACTIONS: std::cell::RefCell<Vec<(([bool; 4], bool), std::rc::Rc<TargetLowering>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The action table of `features`, built on first ask on this thread.
fn actions(
    features: Aarch64Features,
    opt_for_size: bool,
) -> std::rc::Rc<TargetLowering> {
    let key = ([features.cssc, features.lse, features.outline_atomics, features.lse2], opt_for_size);
    if let Some(found) =
        ACTIONS.with(|all| all.borrow().iter().find(|(one, _)| *one == key).map(|(_, tli)| tli.clone()))
    {
        return found;
    }
    let mut tli = TargetLowering::aarch64(features);
    tli.set_opt_for_size(opt_for_size);
    tli.set_fp_imm_predicate(super::lowering::fp_imm_legal);
    BUILT.with(|built| built.set(built.get() + 1));
    let tli = std::rc::Rc::new(tli);
    ACTIONS.with(|all| all.borrow_mut().push((key, tli.clone())));
    tli
}

/// How many action tables this thread has built: one per subtarget and size
/// setting, however many blocks were selected.
pub fn action_tables_built() -> usize {
    BUILT.with(std::cell::Cell::get)
}

/// `CodeGenAndEmitDAG` up to the end of scheduling: the units, in order.
pub fn select_block(
    dag: &mut SelectionDAG,
    globals: &Globals,
    info: &mut llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo,
    opt_for_size: bool,
    linux: bool,
) -> Result<Vec<Unit>, Refusal> {
    llrm_codegen::backend::selectiondag::live_out::assert_live_out_copies(dag, info);
    let mut lowering = Aarch64Lowering { custom: None, globals: globals.clone(), linux, opt_for_size };
    // The scratch objects the legalizer asks for (a float's sign byte through
    // memory) are frame objects numbered after the function's.
    dag.first_temporary = info.next_stack_object();
    let legal = legalized_with(dag, &actions(APPLE_M1, opt_for_size), &mut lowering, None, false);
    // The reason the legalizer stopped is the Custom operation it could not
    // lower.
    if let Some(why) = lowering.custom {
        return Err(Refusal::Legalize(why));
    }
    legal?;
    for (size, align) in std::mem::take(&mut dag.temporaries) {
        info.new_stack_object(size, align);
    }
    // `ComputeLiveOutVRegInfo`: after the legalizer, before the assertions go.
    llrm_codegen::backend::selectiondag::live_out::compute_live_out_vreg_info(dag, info);
    super::combine::shift_amounts_mod(dag);
    let selector = Selector::new(table::TABLE);
    let mut hooks = super::hooks::Aarch64Hooks::new(globals.clone(), opt_for_size, linux);
    do_instruction_selection_with(dag, &*super::ids::IDS, |adapter, node| {
        select_frame_index(adapter, node)
            .or_else(|| select_indexed_load(adapter, node))
            .or_else(|| super::bitfield::select_bitfield_extract(adapter, node))
            .or_else(|| super::bitfield::select_bitfield_insert_in_zero(adapter, node))
            .or_else(|| super::bitfield::select_bitfield_insert(adapter, node))
            .or_else(|| select_zero(adapter, node))
            .unwrap_or_else(|| selector.select(adapter, &mut hooks, node))
    })?;
    let root: SDValue = dag.get_root();
    Ok(schedule(dag, root, &Aarch64Sched))
}

/// `AArch64DAGToDAGISel::Select`, `case ISD::Constant`: zero is `WZR`/`XZR`,
/// which the coalescer propagates into the user. `None` for any other node.
fn select_zero(
    dag: &mut impl AddrDag,
    node: u32,
) -> Option<Outcome> {
    use llrm_dagisel::generated::aarch64::ids::reg;
    if dag.opcode(node) != isd::Constant {
        return None;
    }
    let value = PinValue::new(node, 0);
    let vt = dag.value_type(value);
    let register = match vt {
        mvt::i32 => reg::WZR,
        mvt::i64 => reg::XZR,
        _ => return None,
    };
    if dag.constant(value) != Some(0) {
        return None;
    }
    let zero = dag.register(register, vt);
    dag.replace_node(node, zero.node);
    Some(Outcome::Selected)
}

/// `AArch64DAGToDAGISel::Select`, `case ISD::FrameIndex`: `ADDXri TFI, 0, 0`.
/// `None` for any other node.
fn select_frame_index(
    dag: &mut impl AddrDag,
    node: u32,
) -> Option<Outcome> {
    if dag.opcode(node) != isd::FrameIndex {
        return None;
    }
    let index = dag.frame_index(PinValue::new(node, 0))?;
    let frame = dag.target_frame_index(index, mvt::i64);
    let (offset, shifter) = (dag.target_constant(0, mvt::i32), dag.target_constant(0, mvt::i32));
    dag.morph_node_to(node, aarch64_ids::ADDXri, &[mvt::i64], &[frame, offset, shifter]);
    Some(Outcome::Selected)
}

/// `AArch64DAGToDAGISel::tryIndexedLoad`: a pre- or post-indexed scalar load is
/// `LDR*pre`/`LDR*post` (base, offset, chain; results: the new base, the value,
/// the chain). `None` for any other node.
fn select_indexed_load(
    dag: &mut impl AddrDag,
    node: u32,
) -> Option<Outcome> {
    use llrm_dagisel::dag::LoadExt;
    if dag.opcode(node) != isd::LOAD || !dag.is_indexed(node) {
        return None;
    }
    let pre = !dag.is_post_indexed(node);
    let (vt, dst) = (dag.memory_vt(node), dag.value_type(PinValue::new(node, 0)));
    let ext = dag.load_ext_type(node)?;
    let offset = dag.constant(dag.operand(node, 2))?;
    let pick = |p: u16, q: u16| if pre { p } else { q };
    let mut dst = dst;
    let mut insert_to_64 = false;
    let opcode = if vt == mvt::i64 {
        pick(aarch64_ids::LDRXpre, aarch64_ids::LDRXpost)
    } else if vt == mvt::i32 {
        match ext {
            LoadExt::NonExt => pick(aarch64_ids::LDRWpre, aarch64_ids::LDRWpost),
            LoadExt::SExt => pick(aarch64_ids::LDRSWpre, aarch64_ids::LDRSWpost),
            _ => {
                insert_to_64 = true;
                dst = mvt::i32;
                pick(aarch64_ids::LDRWpre, aarch64_ids::LDRWpost)
            }
        }
    } else if vt == mvt::i16 {
        if ext == LoadExt::SExt {
            if dst == mvt::i64 {
                pick(aarch64_ids::LDRSHXpre, aarch64_ids::LDRSHXpost)
            } else {
                pick(aarch64_ids::LDRSHWpre, aarch64_ids::LDRSHWpost)
            }
        } else {
            insert_to_64 = dst == mvt::i64;
            dst = mvt::i32;
            pick(aarch64_ids::LDRHHpre, aarch64_ids::LDRHHpost)
        }
    } else if vt == mvt::i8 {
        if ext == LoadExt::SExt {
            if dst == mvt::i64 {
                pick(aarch64_ids::LDRSBXpre, aarch64_ids::LDRSBXpost)
            } else {
                pick(aarch64_ids::LDRSBWpre, aarch64_ids::LDRSBWpost)
            }
        } else {
            insert_to_64 = dst == mvt::i64;
            dst = mvt::i32;
            pick(aarch64_ids::LDRBBpre, aarch64_ids::LDRBBpost)
        }
    } else if vt == mvt::f16 {
        pick(aarch64_ids::LDRHpre, aarch64_ids::LDRHpost)
    } else if vt == mvt::f32 {
        pick(aarch64_ids::LDRSpre, aarch64_ids::LDRSpost)
    } else if vt == mvt::f64 {
        pick(aarch64_ids::LDRDpre, aarch64_ids::LDRDpost)
    } else {
        return None;
    };
    let (chain, base) = (dag.operand(node, 0), dag.operand(node, 1));
    let offset = dag.target_constant(offset, mvt::i64);
    let load = dag.machine_node(opcode, &[mvt::i64, dst, mvt::Other], &[base, offset, chain]);
    let mut value = PinValue::new(load, 1);
    if insert_to_64 {
        let zero = dag.target_constant(0, mvt::i64);
        let sub = dag.target_constant(i64::from(llrm_dagisel::generated::aarch64::ids::reg::sub_32), mvt::i32);
        let wide = dag.machine_node(
            llrm_dagisel::generated::ids::target_opcode::SUBREG_TO_REG,
            &[mvt::i64],
            &[zero, value, sub],
        );
        value = PinValue::new(wide, 0);
    }
    dag.replace_uses(PinValue::new(node, 0), value);
    dag.replace_uses(PinValue::new(node, 1), PinValue::new(load, 0));
    dag.replace_uses(PinValue::new(node, 2), PinValue::new(load, 2));
    dag.remove_dead_node(node);
    Some(Outcome::Selected)
}
