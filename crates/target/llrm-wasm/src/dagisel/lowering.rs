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
use llrm_codegen::backend::selectiondag::{
    Action, LoadExt, MemBase, MemInfo, NodeData, SDValue, SelectionDAG, TargetLowering,
};
use llrm_lir::registers::RegId;

use super::generated::ids::isd as ids;
use super::hooks::Cpu;
use crate::dag::call::WasmCalling;
use crate::features::Features;
use crate::reg;

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
pub fn target_lowering(
    cpu: Cpu,
    features: Features,
) -> TargetLowering {
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
    if !features.has("sign-ext") {
        // Sign extends are legal only when extending a vector extract.
        set(
            &[SignExtendInReg],
            &[MVT::I8, MVT::I16, MVT::I32],
            if features.has("simd128") { Action::Custom } else { Action::Expand },
        );
    }
    set(&[StackSave, StackRestore], &[MVT::Other], Action::Expand);
    // The pin asks for `STACKSAVE` at `MVT::Other`; here the node's value type
    // is asked.
    set(&[StackSave], &[pointer(cpu)], Action::Expand);
    set(&[DynamicStackAlloc], &[pointer(cpu)], Action::Expand);
    set(&[FrameIndex], &words, Action::Custom);
    set(&[BrCC, SelectCC], &words, Action::Expand);
    // No floating-point truncating stores (WebAssemblyISelLowering.cpp#L405).
    tl.set_trunc_store_action(MVT::F64, MVT::F32, Action::Expand);
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
        tl.set_operation_action(LLRint, vt, Action::LibCall);
        tl.set_libcall_name(LLRint, vt, if vt == MVT::F32 { "llrintf" } else { "llrint" });
        // `TargetLoweringBase`: the transcendentals default to `Expand`, a call
        // to the C routine of that type.
        for (op, name) in [
            (FSin, "sin"),
            (FCos, "cos"),
            (FAtan, "atan"),
            (FLog, "log"),
            (FLog2, "log2"),
            (FExp, "exp"),
            (FExp2, "exp2"),
            (FRem, "fmod"),
        ] {
            tl.set_operation_action(op, vt, if op == FRem { Action::LibCall } else { Action::Expand });
            let name = if vt == MVT::F32 { f32_name(name) } else { name };
            tl.set_libcall_name(op, vt, name);
        }
    }
    // `RuntimeLibcalls`: the `i128` shifts are the compiler-rt routines.
    for (op, name) in [(Shl, "__ashlti3"), (Srl, "__lshrti3"), (Sra, "__ashrti3")] {
        tl.set_libcall_name(op, MVT::I128, name);
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
        "log2" => "log2f",
        "exp" => "expf",
        "exp2" => "exp2f",
        "fmod" => "fmodf",
        other => unreachable!("no single-precision name for {other}"),
    }
}

/// The custom lowerings. `tables` are the function's jump tables, as block ids:
/// `MJTI`.
pub struct WasmLowering<'a> {
    pub tables: &'a [Vec<u32>],
    pub cpu: Cpu,
}

impl Hooks for WasmLowering<'_> {
    /// `setStackPointerRegisterToSaveRestore(SP32 / SP64)` and the 16-byte
    /// stack alignment: the shared expansion of `DYNAMIC_STACKALLOC`,
    /// `STACKSAVE` and `STACKRESTORE` copies from and to that register.
    fn stack_pointer(&self) -> Option<(RegId, u64)> {
        let register = if self.cpu == Cpu::Wasm64 { reg::SP64 } else { reg::SP32 };
        Some((register, 16))
    }

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
        let pointer = EVT::I32;
        let out = |value: SDValue, arg_vt: EVT, split: bool, sret: bool| Out {
            value,
            ext: Ext::Zero,
            pointer: sret,
            arg_vt,
            split,
            variadic: false,
            sret,
            byval: None,
        };
        let mut outs: Vec<Out> = Vec::new();
        // `CanLowerReturn` is false for an `i128` result: it comes back
        // through a frame object whose address is the first argument.
        let cell = (ret == EVT::Simple(MVT::I128)).then(|| {
            let index = dag.create_stack_temporary(16, 16);
            (index, dag.get_frame_index(pointer, index))
        });
        if let Some((_, address)) = cell {
            outs.push(out(address, pointer, false, true));
        }
        for (at, &value) in args.iter().enumerate() {
            let mut value = value;
            // DEVIATE: the shift routines take an `i32` amount, the pin's
            // `getScalarShiftAmountTy` (pointer width); our DAG's amounts are
            // `i8` or the half type. Exit: wasm declares `i32` shift amounts.
            if at == 1 && ["__ashlti3", "__lshrti3", "__ashrti3"].contains(&name) {
                value = dag.get_zext_or_trunc(value, pointer);
            }
            let vt = dag.value_type(value);
            if vt == EVT::Simple(MVT::I128) {
                // An `i128` argument is two `i64`, the low part first.
                let low = dag.get_node(ISD::Truncate, EVT::I64, &[value]);
                let sixty_four = dag.get_constant(64, pointer);
                let shifted = dag.get_node(ISD::Srl, vt, &[value, sixty_four]);
                let high = dag.get_node(ISD::Truncate, EVT::I64, &[shifted]);
                outs.push(out(low, vt, true, false));
                outs.push(out(high, vt, true, false));
            } else {
                outs.push(out(value, vt, false, false));
            }
        }
        let callee = dag.get_external_symbol(EVT::I32, name);
        let root = dag.entry_node();
        let results: Vec<In> = if cell.is_some() {
            Vec::new()
        } else {
            vec![In { vt: ret, pointer: false, arg_vt: ret, split: false, sret: false, ext: Ext::Any, byval: None }]
        };
        let (chain, values) =
            WasmCalling { pointer: EVT::I32, bulk_memory_opt: true }.lower_call(dag, root, callee, &outs, &results);
        if let Some((index, address)) = cell {
            let info = MemInfo {
                mem_vt: ret,
                align: 16,
                volatile: false,
                base: MemBase::Stack(index),
                ext: None,
                space: 0,
                ordering: None,
            };
            return Some(dag.get_load(ret, chain, address, info));
        }
        values.first().copied()
    }

    /// `makeLibCall` for an `i128` result the caller wants as its two `i64`
    /// halves, low first (`forceExpandWideMUL`'s `__multi3`): the result comes
    /// back through a frame object whose address is the first argument, and
    /// each half is loaded from it.
    fn make_lib_call_parts(
        &mut self,
        dag: &mut SelectionDAG,
        name: &'static str,
        args: &[SDValue],
        rets: &[EVT],
        _signed: bool,
    ) -> Option<Vec<SDValue>> {
        if rets != [EVT::I64, EVT::I64] || args.iter().any(|&arg| dag.value_type(arg) != EVT::I64) {
            return None;
        }
        let pointer = EVT::I32;
        let out = |value: SDValue, arg_vt: EVT, split: bool, sret: bool| Out {
            value,
            ext: Ext::Zero,
            pointer: sret,
            arg_vt,
            split,
            variadic: false,
            sret,
            byval: None,
        };
        let index = dag.create_stack_temporary(16, 16);
        let address = dag.get_frame_index(pointer, index);
        let mut outs = vec![out(address, pointer, false, true)];
        outs.extend(args.iter().map(|&value| out(value, EVT::I64, false, false)));
        let callee = dag.get_external_symbol(EVT::I32, name);
        let root = dag.entry_node();
        let (chain, _) =
            WasmCalling { pointer: EVT::I32, bulk_memory_opt: true }.lower_call(dag, root, callee, &outs, &[]);
        let mut halves = Vec::with_capacity(2);
        for (at, offset) in [0i64, 8].into_iter().enumerate() {
            let info = MemInfo {
                mem_vt: EVT::I64,
                align: if at == 0 { 16 } else { 8 },
                volatile: false,
                base: MemBase::Stack(index),
                ext: None,
                space: 0,
                ordering: None,
            };
            let at_offset = dag.get_object_ptr_offset(address, offset);
            halves.push(dag.get_load(EVT::I64, chain, at_offset, info));
        }
        Some(halves)
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
