//! A target's calls, returns and formal arguments as the pin's
//! `TargetLowering::LowerCall`, `LowerReturn` and `LowerFormalArguments`.
//!
//! # How it works
//! The builder splits each value into legal register parts
//! (`getCopyToParts`: halves until the type is legal), hands them over as
//! [`Out`]s and [`In`]s, and joins the parts the target returns
//! (`getCopyFromParts`). Where each part goes is the target's.
//!
//! A call in tail position is the target's `lower_tail_call` (`TC_RETURN`), or
//! an ordinary call where it answers none.
//!
//! DEVIATE: integers and pointers only; no byval, sret, floats or aggregates.
//! Reason: the targets using the hook need none yet. Exit: with the
//! first target that does.

use std::fmt::Debug;

use llrm_lir::registers::RegId;

use super::dag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::EVT;

/// How a value is widened to the location its convention gives it:
/// `CCValAssign`'s `AExt`, `SExt` and `ZExt`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ext {
    Any,
    Sign,
    Zero,
}

impl Ext {
    pub fn node(self) -> ISD {
        match self {
            Ext::Any => ISD::AnyExtend,
            Ext::Sign => ISD::SignExtend,
            Ext::Zero => ISD::ZeroExtend,
        }
    }
}

/// `ISD::ArgFlagsTy`'s `ByValSize` and `ByValAlign`: an aggregate passed by
/// value, in the bytes of the argument area.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByVal {
    pub size: u32,
    pub align: u32,
}

/// An `ISD::OutputArg` with its value: a part of an argument of a call or of
/// a returned value.
#[derive(Clone, Copy, Debug)]
pub struct Out {
    pub value: SDValue,
    pub ext: Ext,
    /// `ArgFlags.isPointer()`.
    pub pointer: bool,
    /// `OutputArg::ArgVT`: the type of the whole value this is a part of.
    pub arg_vt: EVT,
    /// `ArgFlags.isSplit()`: one of several parts of a value.
    pub split: bool,
    /// `ArgFlags.isSRet()`: the pointer to the result a callee writes.
    pub sret: bool,
    /// The argument is one of a call's variadic ones
    /// (`CallLoweringInfo::IsVarArg` past `NumFixedArgs`).
    pub variadic: bool,
    /// `ArgFlags.isByVal()`: `value` is the address of the bytes to copy into
    /// the argument area (a target whose `byval_in_area` says so).
    pub byval: Option<ByVal>,
}

/// An `ISD::InputArg`: a part of a formal argument or of a call's result.
#[derive(Clone, Copy, Debug)]
pub struct In {
    pub vt: EVT,
    pub pointer: bool,
    /// `InputArg::ArgVT`: the type of the whole value this is a part of.
    pub arg_vt: EVT,
    /// `ArgFlags.isSplit()`.
    pub split: bool,
    /// `ArgFlags.isSRet()`.
    pub sret: bool,
    /// `ArgFlags.isSExt()` / `isZExt()`: how a part narrower than its register
    /// is extended.
    pub ext: Ext,
    /// `ArgFlags.isByVal()`: the part is the address of the bytes the caller
    /// copied into the argument area (a target whose `byval_in_area` says so).
    pub byval: Option<ByVal>,
}

/// `MachineFrameInfo::CreateFixedObject`: size, offset from the first
/// argument, immutable; the frame index.
pub type FixedObject<'a> = &'a mut dyn FnMut(u32, i64, bool) -> i32;

/// `CreateStackObject`: size and alignment; the frame index.
pub type StackObject<'a> = &'a mut dyn FnMut(u32, u32) -> i32;

/// A value `va_start` writes into the `va_list`.
#[derive(Clone, Copy, Debug)]
pub enum VaWord {
    /// The address of a frame object plus an offset.
    Frame {
        index: i32,
        offset: i64,
    },
    Constant(i64),
}

/// What a variadic function's entry made for `va_start`
/// (`SaveVarArgRegisters`): the chain after the register stores, and the words
/// of the list as (byte offset in the list, width in bytes, value). `size` is
/// the list's size, which `va_copy` copies.
#[derive(Clone, Debug)]
pub struct VariadicArea {
    pub chain: SDValue,
    pub words: Vec<(u32, u32, VaWord)>,
    pub size: u32,
}

/// A live-in register: the virtual register the function receives `reg` in.
pub type LiveIn<'a> = &'a mut dyn FnMut(RegId) -> u32;

/// Where a [`TargetCalling::accepts`] question comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Site {
    Arguments,
    Call,
    Return,
}

/// A function the target lowers as an `INTRINSIC_*` node of its own
/// (`llvm.wasm.throw`): the number the target gives it, and whether it has
/// side effects or reads memory (a chain, as `visitTargetIntrinsic`'s
/// `HasChain`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetIntrinsic {
    pub id: u32,
    pub chained: bool,
}

/// The target-independent `LowerVASTART` of the pin's stack-walking targets (M68k's `LowerVASTART`, AVR's, MSP430's):
/// store the address of the first variadic argument's fixed object (`VarArgsFrameIndex`) through the list.
/// `pointer` is the target's pointer type, `align` the list's alignment in bytes. `None` where the function has no
/// variadic frame index.
pub fn lower_vastart_frame_index(
    dag: &mut SelectionDAG,
    node: SDValue,
    pointer: EVT,
    align: u32,
) -> Option<SDValue> {
    let index = dag.var_args_frame_index?;
    let (chain, list) = (dag.operand(node, 0), dag.operand(node, 1));
    let first = dag.get_frame_index(pointer, index);
    let base = match dag.node(list).data {
        NodeData::FrameIndex(slot) => MemBase::Stack(slot),
        _ => MemBase::Unknown,
    };
    let info = MemInfo { mem_vt: pointer, align, volatile: false, base, ext: None, space: 0, ordering: None };
    Some(dag.get_store(chain, first, list, info))
}

pub trait TargetCalling: Debug + Sync {
    /// The intrinsic this name is for the target; none: an ordinary call.
    fn intrinsic(
        &self,
        _name: &str,
    ) -> Option<TargetIntrinsic> {
        None
    }

    /// Whether a `byval` argument is passed as the address of a copy the
    /// caller makes in its own frame, the callee reading through the pointer.
    /// A target that puts the bytes in the argument area says no, and a call
    /// with one is refused.
    fn byval_by_reference(&self) -> bool {
        false
    }

    /// Whether a `byval` argument's bytes are copied into the argument area
    /// (`CCPassByVal`), the callee reading them from there; a `byval` part
    /// then carries its [`ByVal`] and the hook places and copies it.
    fn byval_in_area(&self) -> bool {
        false
    }

    /// Whether a caller with a `byval` parameter makes no tail call (the pin's `isEligibleForTailCallOptimization`:
    /// the parameter is a pointer into the argument area a tail call would reuse).
    fn byval_parameter_blocks_tail_call(&self) -> bool {
        false
    }

    /// Whether the hook places a call in calling convention `cc` (LLVM's
    /// numbers: 0 `ccc`, 8 `fastcc`, 9 `coldcc`): a call in another is refused,
    /// as the pin's `CCAssignFnForCall` rejects what it has no table for.
    fn supports_convention(
        &self,
        _cc: u32,
    ) -> bool {
        true
    }

    /// Whether a direct call of an `llvm.*` function that no intrinsic, atomic
    /// or target hook lowered is emitted as a call to a symbol of that name
    /// (a runtime routine of the target's own). A target with none says no and
    /// the call is refused by name: the symbol would fail only at link, or
    /// bind to nothing meant. The same refusal, with the same wording, as
    /// `BuilderTarget::refuse_unknown_intrinsics` (#8452) for a flow without
    /// this hook; 6502 converges on that field with the shared flow.
    fn calls_unlowered_intrinsic(
        &self,
        _name: &str,
    ) -> bool {
        true
    }

    /// `CanLowerReturn`: whether a result of these parts leaves in registers.
    /// When not, the function returns it through a hidden pointer, the first
    /// parameter (`sret` demotion).
    fn can_lower_return(
        &self,
        _parts: &[EVT],
    ) -> bool {
        true
    }

    /// Whether a function with an `sret` parameter returns that pointer in the
    /// result register as well (the pin's `SRetReturnReg`; M68k copies it to
    /// `d0`), as an extra integer part of `lower_return`'s `outs`.
    fn returns_sret_pointer(&self) -> bool {
        false
    }

    /// Whether the target places these parts (`outs` the values sent, `ins` the
    /// values received); the builder refuses with the reason when it does not.
    fn accepts(
        &self,
        _dag: &SelectionDAG,
        _site: Site,
        _outs: &[Out],
        _ins: &[In],
    ) -> Result<(), &'static str> {
        Ok(())
    }

    /// Whether an argument with this attribute (`sret`, `byval`, ...) is placed
    /// like any other: the builder refuses the ones the hook would have to
    /// move.
    fn passes_in_place(
        &self,
        _attribute: &str,
    ) -> bool {
        false
    }

    /// `LowerCall`: the call sequence and the call; returns the chain and the
    /// result parts.
    fn lower_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
    ) -> (SDValue, Vec<SDValue>);

    /// `LowerCall` of a call in tail position (`IsTailCall`): the `TC_RETURN`
    /// node that ends the block, or none where the target does not take the
    /// call as a tail call (`isEligibleForTailCallOptimization`), which is
    /// then an ordinary call.
    fn lower_tail_call(
        &self,
        _dag: &mut SelectionDAG,
        _chain: SDValue,
        _callee: SDValue,
        _outs: &[Out],
    ) -> Option<SDValue> {
        None
    }

    /// `LowerCall` of a target that makes frame objects of its own
    /// (`MachineFrameInfo::CreateStackObject`: size, alignment, the frame
    /// index) or needs to know the callee is variadic with no variadic
    /// argument passed (`CallLoweringInfo::IsVarArg`).
    fn lower_call_with_frame(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
        _variadic: bool,
        _frame: &mut dyn FnMut(u32, u32) -> i32,
    ) -> (SDValue, Vec<SDValue>) {
        self.lower_call(dag, chain, callee, outs, ins)
    }

    /// `LowerFormalArguments`: the value of each part.
    fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        fixed: FixedObject,
        live_in: LiveIn,
    ) -> Vec<SDValue>;

    /// `LowerFormalArguments` of a variadic function whose `va_list` holds
    /// words over register save areas or the stack arguments: the stores that
    /// save the unnamed argument registers and the words `va_start` writes.
    fn variadic_area(
        &self,
        _dag: &mut SelectionDAG,
        _chain: SDValue,
        _ins: &[In],
        _fixed: FixedObject,
        _stack: StackObject,
        _live_in: LiveIn,
    ) -> Option<VariadicArea> {
        None
    }

    /// `LowerFormalArguments` of a variadic function: the pointer to the buffer
    /// its variadic arguments are in, an extra parameter after the `fixed`
    /// ones; none for a target that keeps them elsewhere.
    fn variadic_buffer(
        &self,
        _dag: &mut SelectionDAG,
        _fixed: usize,
    ) -> Option<SDValue> {
        None
    }

    /// `LowerFormalArguments` of a variadic function: the offset in the stack
    /// arguments of the first variadic one (`CreateFixedObject(1, StackSize,
    /// true)`), which `va_start` stores the address of; none for a target that
    /// keeps them elsewhere.
    fn variadic_frame_offset(
        &self,
        _fixed: &[In],
    ) -> Option<i64> {
        None
    }

    /// `SelectionDAGTargetInfo::EmitTargetCodeForMemcpy` (and `Memmove`, which
    /// the pin routes to it on wasm): the node that copies `size` bytes, or
    /// none to leave it to the inline expansion and the library call.
    fn emit_memcpy(
        &self,
        _dag: &mut SelectionDAG,
        _chain: SDValue,
        _dst: SDValue,
        _src: SDValue,
        _size: SDValue,
    ) -> Option<SDValue> {
        None
    }

    /// [`Self::emit_memcpy`] for a function tuned for size or not
    /// (`hasOptSize`, which the pin's hook reads off the function): a
    /// target that prices the two apart overrides this.
    #[allow(clippy::too_many_arguments)]
    fn emit_memcpy_sized(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
        _opt_size: bool,
    ) -> Option<SDValue> {
        self.emit_memcpy(dag, chain, dst, src, size)
    }

    /// `SelectionDAGTargetInfo::EmitTargetCodeForMemmove`; a target that has
    /// no separate one routes it to [`Self::emit_memcpy`].
    fn emit_memmove(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
    ) -> Option<SDValue> {
        self.emit_memcpy(dag, chain, dst, src, size)
    }

    /// `SelectionDAGTargetInfo::EmitTargetCodeForMemset`.
    fn emit_memset(
        &self,
        _dag: &mut SelectionDAG,
        _chain: SDValue,
        _dst: SDValue,
        _value: SDValue,
        _size: SDValue,
    ) -> Option<SDValue> {
        None
    }

    /// `LowerReturn`: the return node.
    fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue;
}
