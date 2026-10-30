//! `WebAssemblyTargetLowering::LowerCall`, `LowerReturn` and `LowerFormalArguments`
//! as the DAG builder's `TargetCalling`.
//!
//! # How it works
//! Wasm passes no value in a register: a formal argument is an `ARGUMENT idx` node
//! (the local the parameter lands in), a call is one `CALL chain, callee, args..`
//! node with a result per returned part and the chain last, and a return is
//! `RETURN chain, value`.
//!
//! DEVIATE: integers and floats only, one returned part, no variadic call
//! (the pin stores them to a frame object), no byval, tail call, swift or
//! funcref call; `accepts` refuses what it can see. Reason: the straight-line and
//! plain-call slice. Exit: the varargs step (W10).
//!
//! # See also
//! `WebAssemblyISelLowering.cpp` at the pin d1106deb71cc: `LowerCall` (L1281),
//! `CanLowerReturn` (L1583), `LowerReturn` (L1591), `LowerFormalArguments` (L1621).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::target_calling::{FixedObject, In, LiveIn, Out, Site, TargetCalling};
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};

use crate::dagisel::generated::ids::isd;

/// `WebAssemblyISD::CALL`: the first number after the generated ones
/// (`GENERATED_OPCODE_END`, one past `WrapperREL`).
pub const CALL: u16 = isd::WrapperREL + 1;

/// `WebAssemblyISD::RET_CALL`.
pub const RET_CALL: u16 = CALL + 1;

#[derive(Debug)]
pub struct WasmCalling {
    /// The pointer type: `i32` for wasm32, `i64` for wasm64.
    pub pointer: EVT,
}

fn is_value(vt: EVT) -> bool {
    matches!(vt, EVT::I32 | EVT::I64 | EVT::Simple(MVT::F32 | MVT::F64))
}

impl TargetCalling for WasmCalling {
    fn accepts(
        &self,
        dag: &SelectionDAG,
        site: Site,
        outs: &[Out],
        ins: &[In],
    ) -> Result<(), &'static str> {
        if !outs.iter().all(|out| is_value(dag.value_type(out.value))) || !ins.iter().all(|part| is_value(part.vt)) {
            return Err("a wasm argument or result that is no i32, i64, f32 or f64");
        }
        if outs.iter().any(|out| out.variadic) {
            return Err("a wasm variadic call");
        }
        // `canLowerReturn`: without multivalue a function returns one value.
        match site {
            Site::Return if outs.len() > 1 => Err("a wasm return of more than one part"),
            Site::Call if ins.len() > 1 => Err("a wasm call result of more than one part"),
            _ => Ok(()),
        }
    }

    fn lower_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
    ) -> (SDValue, Vec<SDValue>) {
        // A direct callee is `Wrapper (TargetGlobalAddress)`; `Select` unwraps it again.
        let callee = match (dag.opcode(callee), dag.node(callee).data) {
            (ISD::GlobalAddress, NodeData::Global { symbol, offset }) => {
                let address = dag.get_target_global_address(self.pointer, symbol, offset);
                dag.get_node(ISD::Target(isd::Wrapper), self.pointer, &[address])
            }
            _ => callee,
        };
        let mut operands = vec![chain, callee];
        operands.extend(outs.iter().map(|out| out.value));
        let mut vts: Vec<EVT> = ins.iter().map(|part| part.vt).collect();
        vts.push(EVT::OTHER);
        let call = dag.get_node_multi(ISD::Target(CALL), &vts, &operands);
        let values = (0..ins.len() as u8).map(|res| SDValue { node: call.node, res }).collect();
        (SDValue { node: call.node, res: ins.len() as u8 }, values)
    }

    fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        _chain: SDValue,
        ins: &[In],
        _fixed: FixedObject,
        _live_in: LiveIn,
    ) -> Vec<SDValue> {
        ins.iter()
            .enumerate()
            .map(|(at, part)| {
                let index = dag.get_constant_with(at as u128, EVT::I32, true, false);
                dag.get_node(ISD::Target(isd::ARGUMENT), part.vt, &[index])
            })
            .collect()
    }

    fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue {
        let mut operands = vec![chain];
        operands.extend(outs.iter().map(|out| out.value));
        dag.get_node(ISD::Target(isd::RETURN), EVT::OTHER, &operands)
    }
}
