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
//! DEVIATE: no target combines (`WebAssemblyTargetLowering::PerformDAGCombine`)
//! and no `PreprocessISelDAG`. Reason: the slice does not reach them. Exit: W6.
//!
//! # See also
//! `SelectionDAGISel::CodeGenAndEmitDAG` and `WebAssemblyISelDAGToDAG.cpp`
//! (L425) at the pin d1106deb71cc.

use std::collections::BTreeMap;

use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use llrm_codegen::backend::selectiondag::dag_select::do_instruction_selection_with;
use llrm_codegen::backend::selectiondag::dagisel_adapter::{DagisAdapter, PinRegisters};
use llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer;
use llrm_codegen::backend::selectiondag::legalize_types::TypeLegalizer;
use llrm_codegen::backend::selectiondag::schedule::{Preference, SchedInfo, Unit, schedule};
use llrm_codegen::backend::selectiondag::{MemInfo, NodeData, SDValue, SelectionDAG};
use llrm_dagisel::dag::{Dag, SDValue as PinValue};
use llrm_dagisel::interp::{Outcome, Selector};

use super::call::{CALL, RET_CALL};
use crate::dagisel::generated::ids::{isd, opcode as op, reg as pin};
use crate::dagisel::generated::instrinfo::{self, flag};
use crate::dagisel::generated::{wasm_wasm32, wasm_wasm64};
use crate::dagisel::hooks::{Cpu, WasmHooks};
use crate::dagisel::lowering::{WasmLowering, target_lowering};
use crate::dagisel::post_ids;
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
pub struct WasmSched;

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

    fn call_frame_setup(&self) -> Option<u32> {
        Some(u32::from(op::ADJCALLSTACKDOWN))
    }

    fn call_frame_destroy(&self) -> Option<u32> {
        Some(u32::from(op::ADJCALLSTACKUP))
    }

    /// `createDefaultScheduler` at `-O0` (`OptLevel == None`) is the
    /// source-order list scheduler.
    fn preference(&self) -> Preference {
        Preference::Source
    }
}

/// Why a block did not get through.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    Legalize(String),
    CannotSelect(String),
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
            if Dag::opcode(adapter, inner.node)
                == llrm_codegen::backend::selectiondag::dagisel_adapter::pin_opcode(
                    llrm_codegen::backend::selectiondag::isd::ISD::TargetGlobalAddress,
                )
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

/// `CodeGenAndEmitDAG` up to the end of scheduling.
pub fn select_block(
    dag: &mut SelectionDAG,
    cpu: Cpu,
    tables: &[Vec<u32>],
) -> Result<Scheduled, Refusal> {
    let tli = target_lowering(cpu);
    let mut lowering = WasmLowering { tables };
    DagCombiner::new(dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).with_opt_none().run();
    let legalized = TypeLegalizer::new(dag, &tli).with_hooks(&mut lowering).run();
    if legalized.is_err() && llrm_codegen::support::env_var("LLRM_DAGDUMP").is_some() {
        eprintln!("{}", dag.dump());
    }
    legalized.map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    DagCombiner::new(dag, &mut NoCombines, CombineLevel::AfterLegalizeTypes).with_tli(&tli).with_opt_none().run();
    DagLegalizer::new(dag, &tli, &mut lowering).run().map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    DagCombiner::new(dag, &mut NoCombines, CombineLevel::AfterLegalizeDag).with_tli(&tli).with_opt_none().run();

    // `LLRM_DAGDUMP=all`: each block's DAG as selection sees it, as the x86 driver does.
    if llrm_codegen::support::env_var("LLRM_DAGDUMP").is_some_and(|mode| mode == "all") {
        eprintln!("{}", dag.dump());
    }
    let selector = Selector::new(match cpu {
        Cpu::Wasm32 => wasm_wasm32::TABLE,
        Cpu::Wasm64 => wasm_wasm64::TABLE,
    });
    let mut hooks = WasmHooks::new(cpu, true);
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
    Ok(Scheduled { units: schedule(dag, root, &WasmSched), mem_refs, accesses })
}
