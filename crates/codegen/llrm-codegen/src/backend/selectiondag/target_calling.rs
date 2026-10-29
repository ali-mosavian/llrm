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
}

/// An `ISD::InputArg`: a part of a formal argument or of a call's result.
#[derive(Clone, Copy, Debug)]
pub struct In {
    pub vt: EVT,
    pub pointer: bool,
}

/// `MachineFrameInfo::CreateFixedObject`: size, offset from the first
/// argument, immutable; the frame index.
pub type FixedObject<'a> = &'a mut dyn FnMut(u32, i64, bool) -> i32;

/// A live-in register: the virtual register the function receives `reg` in.
pub type LiveIn<'a> = &'a mut dyn FnMut(RegId) -> u32;

pub trait TargetCalling: Debug + Sync {
    /// Whether a `byval` argument is passed as the address of a copy the
    /// caller makes in its own frame, the callee reading through the pointer.
    /// A target that puts the bytes in the argument area says no, and a call
    /// with one is refused.
    fn byval_by_reference(&self) -> bool {
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
