//! The shared DAG stages run for arm64: combine, legalize, select with the
//! AArch64 table, schedule.
//!
//! DEVIATE: no target combines (`PerformDAGCombine` of AArch64 is not
//! translated) and no `PreprocessISelDAG`. A Custom operation is refused (the
//! legalizer would otherwise expand it). Reason: the first functions need none.
//! Exit: B3.

use llrm_codegen::backend::selectiondag::dag_combiner::CombineHooks;
use llrm_codegen::backend::selectiondag::dag_combiner::CombineLevel;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::dag_select::{legalized_with, do_instruction_selection_with, Refusal};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks as LegalizeHooks;
use llrm_codegen::backend::selectiondag::schedule::{SchedInfo, Unit, schedule};
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_dagisel::generated::aarch64::aarch64_apple_m1 as table;
use llrm_dagisel::generated::aarch64::instrinfo::{self, flag};
use llrm_dagisel::interp::Selector;
use llrm_lir::registers::RegId;
use llrm_sdag::actions::{Aarch64Features, TargetLowering};

use llrm_dagisel::dag::SDValue as PinValue;
use llrm_dagisel::generated::aarch64::ids::opcode as aarch64_ids;
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::Outcome;
use llrm_dagisel::x86_addr::AddrDag;

use llrm_dagisel::generated::aarch64::ids::isd as aarch64_isd;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::NodeData;

use super::globals::Globals;
use super::ids::Aarch64Ids;

/// Combine hooks and legalizer hooks of arm64: none. A Custom operation the
/// table asks for is recorded, and the block refused after legalization.
pub struct Aarch64Lowering {
    pub custom: Option<String>,
    globals: Globals,
}

impl CombineHooks for Aarch64Lowering {
    fn is_indexed_legal(
        &self,
        _is_load: bool,
        vt: EVT,
        _post: bool,
    ) -> bool {
        // `setIndexedLoadAction`/`setIndexedStoreAction` of `PRE_INC` and `POST_INC` (the other modes are
        // never made) for i8 to i64 and f32/f64.
        use llrm_codegen::backend::selectiondag::mvt::MVT;
        matches!(vt, EVT::Simple(MVT::I8 | MVT::I16 | MVT::I32 | MVT::I64 | MVT::F32 | MVT::F64))
    }

    fn indexed_address_parts(
        &self,
        dag: &SelectionDAG,
        access: SDValue,
        address: SDValue,
        post: bool,
    ) -> Option<(SDValue, i64)> {
        // `getPreIndexedAddressParts`/`getPostIndexedAddressParts` over `getIndexedAddressParts`.
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
        // "All of the indexed addressing mode instructions take a signed 9 bit immediate offset."
        if !(-256..256).contains(&offset) {
            return None;
        }
        if post {
            // "Post-indexing updates the base, so it's not a valid transform if that's not the same as the
            // load's pointer."
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
            // [reg + imm]: the unscaled signed 9 bits, or the scaled unsigned 12.
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

    fn simplify_demanded_add_sub_mul(&self) -> bool {
        true
    }

    fn reassociate_compare_pairs(&self) -> bool {
        true
    }

    fn select_to_select_cc(&self, vt: EVT) -> bool {
        vt == EVT::I32 || vt == EVT::I64 || (vt.is_floating_point() && vt.size_in_bits() >= 32)
    }

    fn fold_mul_of_add_with_constant(&self) -> bool {
        true
    }

    /// `AArch64TargetLowering::isMulAddWithConstProfitable`: not when `c1` is an add immediate
    /// and `c1*c2` is not but takes more than one instruction to build.
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
        matches!(op, ISD::Mul | ISD::Sub | ISD::And | ISD::Or | ISD::GlobalAddress | ISD::Shl | ISD::Srl | ISD::Sra | ISD::Rotr)
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
            ISD::Mul => super::combine::combine_mul(dag, node, level),
            ISD::GlobalAddress => self.global_address_combine(dag, node),
            ISD::Sub => super::combine::conditional_sub(dag, node),
            ISD::Shl | ISD::Srl | ISD::Sra | ISD::Rotr if level == CombineLevel::AfterLegalizeDag => {
                super::combine::shift_amount_mod(dag, node)
            }
            ISD::And | ISD::Or if level == CombineLevel::AfterLegalizeDag => super::ccmp::combine_and_or_csel(dag, node),
            _ => None,
        }
    }
}

impl LegalizeHooks for Aarch64Lowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        let lowered = match dag.opcode(node) {
            ISD::GlobalAddress => self.global_address(dag, node),
            ISD::SetCC => super::lowering::lower_setcc(dag, node),
            ISD::Select => super::lowering::lower_select(dag, node),
            ISD::SelectCC => super::lowering::lower_select_cc_node(dag, node),
            ISD::Xor => super::lowering::lower_xor(dag, node),
            ISD::BrCond => super::lowering::lower_brcond(dag, node),
            ISD::BrCC => super::lowering::lower_br_cc_node(dag, node),
            ISD::SIntToFp | ISD::UIntToFp => super::lowering::lower_int_to_fp(dag, node),
            ISD::FpToSInt | ISD::FpToUInt => super::lowering::lower_fp_to_int(dag, node),
            ISD::FpExtend => super::lowering::lower_fp_extend(dag, node),
            ISD::FpRound => super::lowering::lower_fp_round(dag, node),
            ISD::AtomicLoadAnd => super::lowering::lower_atomic_load_and(dag, node),
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
    /// `performGlobalAddressCombine`: when every user adds a constant, fold the smallest into
    /// the symbol's offset, within the object and below 2^20.
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
        // Larger than the current offset, or the two forms trade places for ever.
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
        llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters::pin_register(&Aarch64Ids::new(), id)
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

/// The subtarget features of `apple-m1` that the action table reads (`dagisel.toml` `cpus`).
const APPLE_M1: Aarch64Features = Aarch64Features { cssc: false, lse: true, outline_atomics: false, lse2: true };

/// `CodeGenAndEmitDAG` up to the end of scheduling: the units, in order.
pub fn select_block(
    dag: &mut SelectionDAG,
    globals: &Globals,
    info: &mut llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo,
) -> Result<Vec<Unit>, Refusal> {
    let mut tli = TargetLowering::aarch64(APPLE_M1);
    tli.set_fp_imm_predicate(super::lowering::fp_imm_legal);
    llrm_codegen::backend::selectiondag::live_out::assert_live_out_copies(dag, info);
    let mut lowering = Aarch64Lowering { custom: None, globals: globals.clone() };
    let legal = legalized_with(dag, &tli, &mut lowering, None);
    // The reason the legalizer stopped is the Custom operation it could not lower.
    if let Some(why) = lowering.custom {
        return Err(Refusal::Legalize(why));
    }
    legal?;
    // `ComputeLiveOutVRegInfo`: after the legalizer, before the assertions go.
    llrm_codegen::backend::selectiondag::live_out::compute_live_out_vreg_info(dag, info);
    llrm_codegen::backend::selectiondag::dag_select::remove_assertions(dag);
    let selector = Selector::new(table::TABLE);
    let mut hooks = super::hooks::Aarch64Hooks::new(globals.clone());
    do_instruction_selection_with(dag, &Aarch64Ids::new(), |adapter, node| {
        select_frame_index(adapter, node)
            .or_else(|| select_indexed_load(adapter, node))
            .or_else(|| select_zero(adapter, node))
            .unwrap_or_else(|| selector.select(adapter, &mut hooks, node))
    })?;
    let root: SDValue = dag.get_root();
    Ok(schedule(dag, root, &Aarch64Sched))
}

/// `AArch64DAGToDAGISel::Select`, `case ISD::Constant`: zero is `WZR`/`XZR`, which the
/// coalescer propagates into the user. `None` for any other node.
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

/// `AArch64DAGToDAGISel::tryIndexedLoad`: a pre- or post-indexed scalar load is `LDR*pre`/`LDR*post`
/// (base, offset, chain; results: the new base, the value, the chain). `None` for any other node.
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
        let wide = dag.machine_node(llrm_dagisel::generated::ids::target_opcode::SUBREG_TO_REG, &[mvt::i64], &[zero, value, sub]);
        value = PinValue::new(wide, 0);
    }
    dag.replace_uses(PinValue::new(node, 0), value);
    dag.replace_uses(PinValue::new(node, 1), PinValue::new(load, 0));
    dag.replace_uses(PinValue::new(node, 2), PinValue::new(load, 2));
    dag.remove_dead_node(node);
    Some(Outcome::Selected)
}
