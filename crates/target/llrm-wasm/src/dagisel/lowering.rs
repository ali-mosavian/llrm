//! `WebAssemblyTargetLowering`: the action table and the custom lowering of
//! addresses, for the integer slice.
//!
//! # How it works
//! `i32` and `i64` are the legal integers; `i8` and `i16` promote. A global
//! address becomes `WebAssemblyISD::Wrapper (TargetGlobalAddress)` and a frame
//! index its target form, which the selector's patterns and
//! [`super::addr`] match.
//!
//! DEVIATE: not translated yet are the floating-point rows, the SIMD, reference
//! type and atomic rows, `LOAD`/`STORE` `Custom` (only address spaces 1 and the
//! reference types use it), `VAARG` (no such ISD opcode yet), `DEBUGTRAP`,
//! `ExternalSymbol`, `JumpTable`, `BR_JT`, `VASTART`, `CopyToReg` and the
//! position-independent path of `LowerGlobalAddress`. Reason: each needs DAG
//! nodes or target state the straight-line slice does not reach, and a `Custom`
//! row whose lowering returns nothing would expand the node. Exit: W4 onwards.
//!
//! # See also
//! `llvm/lib/Target/WebAssembly/WebAssemblyISelLowering.cpp`, pin
//! d1106deb71cc: the constructor (L43), `LowerFrameIndex` (L2027),
//! `LowerGlobalAddress` (L2117).

use llrm_codegen::backend::selectiondag::TargetLoweringInfo;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::mvt::MVT;
use llrm_codegen::backend::selectiondag::{Action, LoadExt, NodeData, SDValue, SelectionDAG, TargetLowering};

use super::generated::ids::isd as ids;
use super::hooks::Cpu;

/// The pin's `WebAssemblyISD` numbers, as the matcher table names them.
pub mod wasmisd {
    use super::{ISD, ids};

    pub const WRAPPER: ISD = ISD::Target(ids::Wrapper);
}

/// The pointer type: `Subtarget->hasAddr64() ? MVT::i64 : MVT::i32`.
fn pointer(cpu: Cpu) -> MVT {
    match cpu {
        Cpu::Wasm32 => MVT::I32,
        Cpu::Wasm64 => MVT::I64,
    }
}

/// The constructor of `WebAssemblyTargetLowering`, in the pin's order.
pub fn target_lowering(cpu: Cpu) -> TargetLowering {
    use ISD::*;
    let words = [MVT::I32, MVT::I64];
    let mut tl = TargetLowering::empty(TargetLoweringInfo::new(&words, false));
    tl.init_actions();
    let mut set = |ops: &[ISD], vts: &[MVT], action: Action| {
        for &op in ops {
            for &vt in vts {
                tl.set_operation_action(op, vt, action);
            }
        }
    };
    set(&[GlobalAddress], &[pointer(cpu)], Action::Custom);
    set(&[VACopy, VAEnd], &[MVT::Other], Action::Expand);
    set(
        &[
            BSwap, SMulLoHi, UMulLoHi, MulHS, MulHU, SDivRem, UDivRem, ShlParts, SraParts, SrlParts, AddC, AddE, SubC,
            SubE,
        ],
        &words,
        Action::Expand,
    );
    set(&[SignExtendInReg], &[MVT::I1], Action::Expand);
    set(&[StackSave, StackRestore], &[MVT::Other], Action::Expand);
    set(&[DynamicStackAlloc], &[pointer(cpu)], Action::Expand);
    set(&[FrameIndex], &words, Action::Custom);
    set(&[BrCC, SelectCC], &words, Action::Expand);
    for vt in [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
        for ext in [LoadExt::ExtLoad, LoadExt::ZExtLoad, LoadExt::SExtLoad] {
            tl.set_load_ext_action(ext, vt, MVT::I1, Action::Promote);
        }
    }
    tl.set_operation_action(BuildPair, MVT::I64, Action::Expand);
    tl.set_operation_action(Trap, MVT::Other, Action::Legal);
    tl
}

/// The custom lowerings.
pub struct WasmLowering;

impl Hooks for WasmLowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::GlobalAddress => Some(lower_global_address(dag, node)),
            ISD::FrameIndex => Some(lower_frame_index(dag, node)),
            _ => None,
        }
    }
}

/// `LowerGlobalAddress`, the non-position-independent path.
fn lower_global_address(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let NodeData::Global { symbol, offset } = dag.node(node).data else {
        unreachable!("a GlobalAddress node carries its symbol")
    };
    let address = dag.get_target_global_address(vt, symbol, offset);
    dag.get_node(wasmisd::WRAPPER, vt, &[address])
}

/// `LowerFrameIndex`.
fn lower_frame_index(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let NodeData::FrameIndex(index) = dag.node(node).data else { unreachable!("a FrameIndex node carries its index") };
    let vt = dag.value_type(node);
    dag.get_target_frame_index(vt, index)
}
