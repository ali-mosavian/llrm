//! A target's calls, returns and formal arguments as the pin's
//! `TargetLowering::LowerCall`, `LowerReturn` and `LowerFormalArguments`.
//!
//! # How it works
//! The builder splits each value into legal register parts
//! (`getCopyToParts`: halves until the type is legal), hands them over as
//! [`Out`]s and [`In`]s, and joins the parts the target returns
//! (`getCopyFromParts`). Where each part goes is the target's.
//!
//! DEVIATE: integers and pointers only; no byval, sret, tail calls, floats or
//! aggregates. Reason: the targets using the hook need none yet. Exit: with the
//! first target that does.

use std::fmt::Debug;

use llrm_lir::registers::RegId;

use super::dag::{SDValue, SelectionDAG};
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
    /// The argument is one of a call's variadic ones (`CallLoweringInfo::IsVarArg` past `NumFixedArgs`).
    pub variadic: bool,
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
}

/// `MachineFrameInfo::CreateFixedObject`: size, offset from the first
/// argument, immutable; the frame index.
pub type FixedObject<'a> = &'a mut dyn FnMut(u32, i64, bool) -> i32;

/// A live-in register: the virtual register the function receives `reg` in.
pub type LiveIn<'a> = &'a mut dyn FnMut(RegId) -> u32;

/// Where a [`TargetCalling::accepts`] question comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Site {
    Arguments,
    Call,
    Return,
}

pub trait TargetCalling: Debug + Sync {
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

    /// `LowerFormalArguments`: the value of each part.
    fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        fixed: FixedObject,
        live_in: LiveIn,
    ) -> Vec<SDValue>;

    /// `LowerReturn`: the return node.
    fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue;
}
