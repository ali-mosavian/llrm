//! `WebAssemblyTargetLowering`: the action table and the custom lowering of
//! addresses, for the integer slice.
//!
//! # How it works
//! `i32` and `i64` are the legal integers; `i8` and `i16` promote. A global
//! address becomes `WebAssemblyISD::Wrapper (TargetGlobalAddress)` and a frame
//! index its target form, which the selector's patterns and
//! [`super::addr`] match.
//!
//! DEVIATE W6: not translated are the SIMD, reference type and atomic rows,
//! `LOAD`/`STORE` `Custom` (only address spaces 1 and the reference types use
//! it), `VAARG` (no such ISD opcode), `DEBUGTRAP`, `ExternalSymbol`,
//! `VASTART`, `CopyToReg` and the position-independent path of
//! `LowerGlobalAddress`. Reason: QB reaches none of them, and a `Custom` row
//! whose lowering returns nothing would expand the node. Exit: with the rows.
//!
//! # See also
//! `llvm/lib/Target/WebAssembly/WebAssemblyISelLowering.cpp`, pin
//! d1106deb71cc: the constructor (L43), `LowerFrameIndex` (L2027),
//! `LowerGlobalAddress` (L2117).

use llrm_codegen::backend::selectiondag::TargetLoweringInfo;
use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::target_calling::{Ext, In, Out, TargetCalling};
use llrm_codegen::backend::selectiondag::{Action, LoadExt, NodeData, SDValue, SelectionDAG, TargetLowering};

use super::generated::ids::isd as ids;
use super::hooks::Cpu;
use crate::dag::call::WasmCalling;

/// The pin's `WebAssemblyISD` numbers, as the matcher table names them.
pub mod wasmisd {
    use super::{ISD, ids};

    pub const WRAPPER: ISD = ISD::Target(ids::Wrapper);
    pub const BR_TABLE: ISD = ISD::Target(ids::BR_TABLE);
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
    let mut tl = TargetLowering::empty(
        // `getSetCCResultType` is `i32`; a shift's amount has the type of the
        // value shifted (`getScalarShiftAmountTy`, up to 64 bits).
        TargetLoweringInfo::new(&[MVT::I32, MVT::I64], false).with_shift_by_value_type().with_set_cc_result(MVT::I32),
    );
    tl.init_actions();
    let mut set = |ops: &[ISD], vts: &[MVT], action: Action| {
        for &op in ops {
            for &vt in vts {
                tl.set_operation_action(op, vt, action);
            }
        }
    };
    set(&[GlobalAddress, ExternalSymbol], &[pointer(cpu)], Action::Custom);
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
    for vt in [MVT::F32, MVT::F64] {
        // Don't expand the floating-point types to constant pools.
        tl.set_operation_action(ConstantFP, vt, Action::Legal);
        // Expand floating-point comparisons.
        for cc in [
            CondCode::SetO,
            CondCode::SetUo,
            CondCode::SetUEq,
            CondCode::SetONe,
            CondCode::SetULt,
            CondCode::SetULe,
            CondCode::SetUGt,
            CondCode::SetUGe,
        ] {
            tl.set_cond_code_action(cc, vt, Action::Expand);
        }
        // Expand floating-point library function operators; the scalar
        // remainder is a libcall.
        for op in [FSin, FCos] {
            tl.set_operation_action(op, vt, Action::Expand);
        }
        tl.set_operation_action(FRem, vt, Action::LibCall);
        tl.set_operation_action(LRint, vt, Action::LibCall);
        tl.set_libcall_name(LRint, vt, if vt == MVT::F32 { "lrintf" } else { "lrint" });
        // `TargetLoweringBase`: the transcendentals default to `Expand`, a call
        // to the C routine of that type.
        for (op, name) in [(FSin, "sin"), (FCos, "cos"), (FAtan, "atan"), (FLog, "log"), (FExp, "exp"), (FRem, "fmod")]
        {
            tl.set_operation_action(op, vt, if op == FRem { Action::LibCall } else { Action::Expand });
            let name = if vt == MVT::F32 { f32_name(name) } else { name };
            tl.set_libcall_name(op, vt, name);
        }
    }
    tl.set_operation_action(BrJT, MVT::Other, Action::Custom);
    tl.set_operation_action(BuildPair, MVT::I64, Action::Expand);
    tl.set_operation_action(Trap, MVT::Other, Action::Legal);
    tl
}

/// The single-precision routine of a C math function: `RTLIB`'s `sinf`.
fn f32_name(name: &'static str) -> &'static str {
    match name {
        "sin" => "sinf",
        "cos" => "cosf",
        "atan" => "atanf",
        "log" => "logf",
        "exp" => "expf",
        "fmod" => "fmodf",
        other => unreachable!("no single-precision name for {other}"),
    }
}

/// The custom lowerings. `tables` are the function's jump tables, as block ids:
/// `MJTI`.
pub struct WasmLowering<'a> {
    pub tables: &'a [Vec<u32>],
}

impl Hooks for WasmLowering<'_> {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::GlobalAddress => Some(lower_global_address(dag, node)),
            ISD::ExternalSymbol => Some(lower_external_symbol(dag, node)),
            ISD::FrameIndex => Some(lower_frame_index(dag, node)),
            ISD::BrJT => Some(lower_br_jt(dag, node, self.tables)),
            _ => None,
        }
    }

    /// `makeLibCall` through `LowerCall`: the routine is an external symbol,
    /// the chain the entry node.
    fn make_lib_call(
        &mut self,
        dag: &mut SelectionDAG,
        name: &'static str,
        args: &[SDValue],
        ret: EVT,
        _signed: bool,
    ) -> Option<SDValue> {
        let outs: Vec<Out> = args
            .iter()
            .map(|&value| Out {
                value,
                ext: Ext::Zero,
                pointer: false,
                arg_vt: dag.value_type(value),
                split: false,
                variadic: false,
            })
            .collect();
        let callee = dag.get_external_symbol(EVT::I32, name);
        let root = dag.entry_node();
        let results = [In { vt: ret, pointer: false, arg_vt: ret, split: false }];
        let (_chain, values) = WasmCalling { pointer: EVT::I32 }.lower_call(dag, root, callee, &outs, &results);
        values.first().copied()
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

/// `LowerExternalSymbol`, the non-position-independent path.
fn lower_external_symbol(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let NodeData::Symbol(id) = dag.node(node).data else { unreachable!("an ExternalSymbol node carries its symbol") };
    let name = dag.symbol_names[id as usize].clone();
    let address = dag.get_target_external_symbol(vt, &name);
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

/// `LowerBR_JT`: `BR_TABLE chain, index, case blocks.., dummy default`. The
/// default is the first case until `FixBrTableDefaults` finds the range
/// check's.
fn lower_br_jt(
    dag: &mut SelectionDAG,
    node: SDValue,
    tables: &[Vec<u32>],
) -> SDValue {
    let (chain, table, index) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let NodeData::JumpTable(at) = dag.node(table).data else { unreachable!("a BR_JT names a JumpTable") };
    let blocks = &tables[at as usize];
    let mut ops = vec![chain, index];
    ops.extend(blocks.iter().map(|&block| dag.get_basic_block(block)));
    ops.push(dag.get_basic_block(blocks[0]));
    dag.get_node(wasmisd::BR_TABLE, MVT::Other.into(), &ops)
}
