//! The shared DAG stages run for arm64: combine, legalize, select with the
//! AArch64 table, schedule.
//!
//! DEVIATE: no target combines (`PerformDAGCombine` of AArch64 is not
//! translated) and no `PreprocessISelDAG`. A Custom operation is refused (the
//! legalizer would otherwise expand it). Reason: the first functions need none.
//! Exit: B3.

use llrm_codegen::backend::selectiondag::dag_combiner::CombineHooks;
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

impl CombineHooks for Aarch64Lowering {}

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
            ISD::VAStart => super::lowering::lower_va_start(dag, node),
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

/// `CodeGenAndEmitDAG` up to the end of scheduling: the units, in order.
pub fn select_block(
    dag: &mut SelectionDAG,
    globals: &Globals,
) -> Result<Vec<Unit>, Refusal> {
    let mut tli = TargetLowering::aarch64(Aarch64Features::default());
    tli.set_fp_imm_predicate(super::lowering::fp_imm_legal);
    let mut lowering = Aarch64Lowering { custom: None, globals: globals.clone() };
    let legal = legalized_with(dag, &tli, &mut lowering, None);
    // The reason the legalizer stopped is the Custom operation it could not lower.
    if let Some(why) = lowering.custom {
        return Err(Refusal::Legalize(why));
    }
    legal?;
    let selector = Selector::new(table::TABLE);
    let mut hooks = super::hooks::Aarch64Hooks::new(globals.clone());
    do_instruction_selection_with(dag, &Aarch64Ids::new(), |adapter, node| {
        select_frame_index(adapter, node)
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
