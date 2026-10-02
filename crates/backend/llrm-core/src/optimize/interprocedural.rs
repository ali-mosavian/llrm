//! The whole-module step a frontend runs once every body has reached its own
//! fixed point: inline, carry constants across direct calls, drop dead pure
//! calls and the tails of terminal ones, and send each changed body back
//! through its pipeline until no body changes.
//!
//! A frontend hands in each procedure's facts and its pipeline; nothing here
//! names a language or a machine.

use std::collections::BTreeSet;

use crate::model::mir::{Const, MemRef};
use crate::support::hash::IndexMap;

/// What the whole-module step needs to know about one procedure.
pub struct Procedure<'a> {
    pub name: &'a str,
    /// call site -> callee name
    pub calls: &'a IndexMap<i64, String>,
    /// Formal entry cells, in the order a call's pushes bind them, last first.
    pub parameters: &'a [MemRef],
    /// Scalar actuals per call site known before MIR, where the frontend has them.
    pub constants: &'a IndexMap<i64, Vec<Option<Const>>>,
    /// call site -> the ARG operations that feed it
    pub arguments: &'a IndexMap<i64, BTreeSet<i64>>,
}
