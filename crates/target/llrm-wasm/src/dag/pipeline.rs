//! The shared DAG stages run for wasm: the pin's `CodeGenAndEmitDAG` order
//! (combine, legalize types, combine, legalize the DAG, combine, select,
//! schedule) over a built block, with wasm's tables and hooks.
//!
//! # How it works
//! `WebAssemblyDAGToDAGISel::Select` handles `CALL` itself: it splits the node
//! into `CALL_PARAMS` (operands, glue result) and `CALL_RESULTS` (the results),
//! which a custom inserter joins after selection. The same split is made here
//! ([`select_call`]); everything else goes through the matcher table.
//!
//! The pin runs no scheduler preference of its own at `-O0`:
//! `createDefaultScheduler` gives `source`.
//!
//! DEVIATE W4: no target combines
//! (`WebAssemblyTargetLowering::PerformDAGCombine`) and no `PreprocessISelDAG`.
//! Reason: they serve SIMD, atomics and `BUILD_VECTOR`, which QB does not
//! produce. Exit: with those rows, no later than 2026-11-20.
//!
//! # See also
//! `SelectionDAGISel::CodeGenAndEmitDAG` and `WebAssemblyISelDAGToDAG.cpp`
//! (L425) at the pin d1106deb71cc.

use std::collections::BTreeMap;

use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::chain_combine::AliasOracle;
use llrm_codegen::backend::selectiondag::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use llrm_codegen::backend::selectiondag::dag_select::do_instruction_selection_with;
use llrm_codegen::backend::selectiondag::dagisel_adapter::{DagisAdapter, PinRegisters};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer;
use llrm_codegen::backend::selectiondag::legalize_types::TypeLegalizer;
use llrm_codegen::backend::selectiondag::live_out;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::{SchedInfo, Unit, schedule};
use llrm_codegen::backend::selectiondag::{MemInfo, NodeData, SDValue, SelectionDAG, TargetLowering};
use llrm_dagisel::dag::{Dag, SDValue as PinValue};
use llrm_dagisel::interp::{Outcome, Selector};

use super::call::{CALL, RET_CALL};
use crate::dagisel::generated::ids::{isd, reg as pin};
use crate::dagisel::generated::instrinfo::{self, flag};
use crate::dagisel::generated::{wasm_wasm32, wasm_wasm64};
use crate::dagisel::hooks::{Cpu, WasmHooks};
use crate::dagisel::lowering::{WasmLowering, target_lowering};
use crate::dagisel::post_ids;
use crate::features::Features;
use crate::reg;

/// Wasm's physical registers in the pin's numbering: the stack-pointer global.
pub struct WasmPins;

impl PinRegisters for WasmPins {
    fn phys_reg(
        &self,
        register: u16,
    ) -> Option<RegId> {
        match register {
            pin::SP32 => Some(reg::SP32),
            pin::SP64 => Some(reg::SP64),
            _ => None,
        }
    }

    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        match id {
            reg::SP32 => Some(pin::SP32),
            reg::SP64 => Some(pin::SP64),
            _ => None,
        }
    }
}

/// What the scheduler reads of the instruction descriptions.
pub struct WasmSched {
    /// Not `-O0`: `createDefaultScheduler` is source order at `-O0` only.
    pub optimize: bool,
}

impl SchedInfo for WasmSched {
    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        WasmPins.pin_register(id)
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

    /// `createDefaultScheduler` is the source-order list scheduler at `-O0`
    /// (`OptLevel == None`); above it, `WebAssemblyTargetLowering` asks for
    /// `Sched::RegPressure` (`setSchedulingPreference`) and the subtarget
    /// disables the machine scheduler (`enableMachineScheduler` is false), so
    /// the register-reduction scheduler (`createBURRListDAGScheduler`) runs.
    fn source_order(&self) -> bool {
        !self.optimize
    }
}

/// Why a block did not get through.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    Legalize(String),
    CannotSelect(String),
}

/// The combiner's target hooks: `SimplifySetCC` makes `x <= c` into `x < c +
/// 1`, as the pin does for wasm.
struct WasmCombines;

impl CombineHooks for WasmCombines {
    /// `visitFREEZE` pushes a freeze to the operands the pin cannot prove
    /// defined, so the freeze `narrowSDivSRem` leaves on a byte does not hide
    /// its sign bits from `sext_inreg` (`div8_os` -O3 kept one per iteration).
    fn push_freeze_through_operands(&self) -> bool {
        true
    }

    /// `TargetLowering::convertSelectOfConstantsToMath`: wasm keeps the pin's
    /// default, so `(shl (select c, 1, 2), 2)` folds into one `select 4, 8`.
    fn convert_select_of_constants_to_math(
        &self,
        _vt: EVT,
    ) -> bool {
        false
    }

    /// `convertSetCCLogicToBitwiseLogic`: WebAssembly keeps the pin's default
    /// (false), so `(a != 7) | (b != 9)` stays two compares and an `or`.
    fn convert_setcc_logic_to_bitwise_logic(&self) -> bool {
        false
    }

    /// `TargetLowering::shouldNormalizeToSelectSequence`: wasm keeps the pin's
    /// default, true unless the select's type is split into registers, so
    /// `select (and c0, c1), x, y` is `select c0, (select c1, x, y), y`.
    fn normalize_to_select_sequence(
        &self,
        vt: EVT,
        _cond_vt: EVT,
    ) -> bool {
        !(vt.is_integer() && vt.size_in_bits() > 64)
    }

    fn canonicalize_ge_le(&self) -> bool {
        true
    }

    /// `getScalarShiftAmountTy`: wasm shifts by the type of the value shifted
    /// (the combiner's default is a byte).
    fn shift_amount_type(
        &self,
        shifted: EVT,
    ) -> EVT {
        shifted
    }

    fn fold_and_of_extload(&self) -> bool {
        true
    }

    /// `visitSUB`'s `C2-(A+C1)` is `(C2-C1)-A` (DAGCombiner.cpp#L4432) and the
    /// folds beside it: the pin has them on for every target.
    fn sub_algebra_folds(&self) -> bool {
        true
    }

    fn fold_sext_in_reg_of_load(&self) -> bool {
        true
    }

    fn rebuild_brcond_condition(&self) -> bool {
        true
    }

    fn fold_bin_op_into_select(&self) -> bool {
        true
    }

    /// `visitMUL`'s `(mul (add x, c1), c2)` is `(add (mul x, c2), c1*c2)`
    /// (DAGCombiner.cpp#L5080), for a single-use add where
    /// `isMulAddWithConstProfitable` holds, which is the pin's default for
    /// wasm.
    fn fold_mul_of_add_with_constant(&self) -> bool {
        true
    }

    /// `tryStoreMergeOfConstants`: two byte stores are one `i32.store16`, a
    /// truncating store of the promoted type (`isTruncStoreLegal(i32, i16)`).
    fn merge_stores_via_trunc_store(&self) -> bool {
        true
    }

    /// `replaceStoreOfFPConstant`: a `float`/`double` constant store, volatile
    /// too (`isNormalStore`), is an `i32`/`i64` constant store, as the pin does
    /// for wasm.
    fn store_fp_constant_as_i64(&self) -> bool {
        true
    }

    fn merge_stores_after_legalization(
        &self,
        _mem: EVT,
    ) -> bool {
        // The pin's default; EVT holds no vectors yet.
        true
    }
}

/// `WebAssemblyDAGToDAGISel::Select` for `CALL` and `RET_CALL`.
fn select_call(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
) -> Outcome {
    let operands = Dag::num_operands(adapter, node);
    let mut params = Vec::with_capacity(operands);
    for at in 1..operands {
        let mut operand = Dag::operand(adapter, node, at);
        // The callee loses its `Wrapper` when it is a symbol: only a computed
        // one is called indirectly.
        if at == 1 && Dag::opcode(adapter, operand.node) == isd::Wrapper {
            let inner = Dag::operand(adapter, operand.node, 0);
            use llrm_codegen::backend::selectiondag::isd::ISD::{
                TargetExternalSymbolNode as TARGET_EXTERNAL, TargetGlobalAddress as TARGET_GLOBAL,
            };
            let symbol = |isd| llrm_codegen::backend::selectiondag::dagisel_adapter::pin_opcode(isd);
            if [TARGET_GLOBAL, TARGET_EXTERNAL].into_iter().any(|kind| Dag::opcode(adapter, inner.node) == symbol(kind))
            {
                operand = inner;
            }
        }
        params.push(operand);
    }
    params.push(Dag::operand(adapter, node, 0));
    let glue = llrm_dagisel::generated::ids::mvt::Glue;
    let call_params = Dag::machine_node(adapter, post_ids::CALL_PARAMS, &[glue], &params);
    let results = if adapter.dag.opcode(SDValue { node, res: 0 })
        == llrm_codegen::backend::selectiondag::isd::ISD::Target(RET_CALL)
    {
        post_ids::RET_CALL_RESULTS
    } else {
        post_ids::CALL_RESULTS
    };
    let vts: Vec<_> =
        (0..Dag::num_values(adapter, node)).map(|at| Dag::value_type(adapter, PinValue::new(node, at as u8))).collect();
    let call_results = Dag::machine_node(adapter, results, &vts, &[PinValue::new(call_params, 0)]);
    Dag::replace_node(adapter, node, call_results);
    Outcome::Selected
}

/// A block after scheduling: the units in order, and the memory accesses each
/// machine node got (`setNodeMemRefs`), as the load and store nodes they came
/// from.
pub struct Scheduled {
    pub units: Vec<Unit>,
    pub mem_refs: BTreeMap<u32, Vec<u32>>,
    /// What each load and store node was before selection: `MorphNodeTo` turns
    /// a selected access into the instruction in place and drops its
    /// `MachineMemOperand`.
    pub accesses: BTreeMap<u32, MemInfo>,
}

/// One `DAGCombiner` run: `OptLevel == None` only at `-O0`.
fn combine(
    dag: &mut SelectionDAG,
    level: CombineLevel,
    tli: &TargetLowering,
    alias: &dyn AliasOracle,
    optimize: bool,
) {
    let mut hooks = WasmCombines;
    let combiner = DagCombiner::new(dag, &mut hooks, level).with_tli(tli).with_alias(alias);
    if optimize { combiner } else { combiner.with_opt_none() }.run();
}

/// `CodeGenAndEmitDAG` up to the end of scheduling.
pub fn select_block(
    dag: &mut SelectionDAG,
    cpu: Cpu,
    features: Features,
    info: &mut FunctionLoweringInfo,
    alias: &dyn AliasOracle,
    optimize: bool,
) -> Result<Scheduled, Refusal> {
    let tli = target_lowering(cpu, features);
    // The scratch objects the legalizers ask for (an `i128` libcall result's
    // memory) are frame objects numbered after the function's.
    dag.first_temporary = info.next_stack_object();
    let tables = &info.jump_tables;
    let mut lowering = WasmLowering { tables, cpu };
    if optimize {
        live_out::assert_live_out_copies(dag, info);
    }
    combine(dag, CombineLevel::BeforeLegalizeTypes, &tli, alias, optimize);
    let legalized = TypeLegalizer::new(dag, &tli).with_hooks(&mut lowering).run();
    if legalized.is_err() && llrm_codegen::support::env_var("LLRM_DAGDUMP").is_some() {
        eprintln!("{}", dag.dump());
    }
    legalized.map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    combine(dag, CombineLevel::AfterLegalizeTypes, &tli, alias, optimize);
    DagLegalizer::new(dag, &tli, &mut lowering).run().map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    for (size, align) in std::mem::take(&mut dag.temporaries) {
        info.new_stack_object(size, align);
    }
    combine(dag, CombineLevel::AfterLegalizeDag, &tli, alias, optimize);
    if optimize {
        live_out::compute_live_out_vreg_info(dag, info);
    }

    // `LLRM_DAGDUMP=all`: each block's DAG as selection sees it, as the x86
    // driver does.
    if llrm_codegen::support::env_var("LLRM_DAGDUMP").is_some_and(|mode| mode == "all") {
        eprintln!("{}", dag.dump());
    }
    let selector = Selector::new(match cpu {
        Cpu::Wasm32 => wasm_wasm32::TABLE,
        Cpu::Wasm64 => wasm_wasm64::TABLE,
    });
    let mut hooks = WasmHooks::new(cpu, features, true);
    let mut mem_refs = BTreeMap::new();
    let accesses = dag
        .nodes()
        .iter()
        .enumerate()
        .filter_map(|(at, node)| if let NodeData::Mem(info) = node.data { Some((at as u32, info)) } else { None })
        .collect();
    do_instruction_selection_with(dag, &WasmPins, |adapter, node| {
        let code = Dag::opcode(adapter, node);
        let outcome = if code == CALL || code == RET_CALL {
            select_call(adapter, node)
        } else {
            selector.select(adapter, &mut hooks, node)
        };
        mem_refs.append(&mut adapter.mem_refs);
        outcome
    })
    .map_err(|why| Refusal::CannotSelect(format!("{why:?}")))?;
    let root: SDValue = dag.get_root();
    Ok(Scheduled { units: schedule(dag, root, &WasmSched { optimize }), mem_refs, accesses })
}
