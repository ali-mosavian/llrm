//! The frame plan of a procedure whose prologue already exists (native-code
//! input): where it reserves the frame, where it releases it, and which
//! registers hold values on entry.
//!
//! # How it works
//! A [`Plan`] holds the established reservation (`entry.floor`,
//! `entry.reserve_at`, the registers saved), the instructions that release the
//! frame (`releases`), `(instruction, register)` pairs for values that arrive
//! in registers (`registers`), the outgoing stack-argument cells (`outgoing`),
//! whether the procedure keeps a frame register (`framed`) and the stack depth
//! at a return (`return_depth`, not read anywhere).
//!
//! `pins` reads the plan against the body and gives each value held at one of
//! those instructions that register, so the allocator keeps it there.
//!
//! A plan changes the other frame phases:
//!
//! ```text
//!   frame::of     refuses a body that reaches below entry.floor; leaves the
//!                 outgoing cells out of the floor; keeps pins in native_pins
//!   Frame::slot   refuses a spill when the procedure is not framed
//!   prologue      reserves at entry.reserve_at and releases at `releases`,
//!                 not at the entry and the returns; shifts outgoing cells
//!   assemble      no second frame (relayout)
//! ```
//!
//! No caller builds a plan: `assemble` passes none to `frame::of`, so
//! `Frame::native` is always `None` in the compiler and the paths above are
//! not taken.
//!
//! # Example
//! No honest example: no caller builds a plan, so no LIR is ever shaped by it.
//! Read `frame::of` for where a plan would enter.
//!
//! # Used by
//! `frame` (`Frame::native`), `prologue`, `relayout`, `assemble` and
//! `flow::machine_with` (which adds `native_pins` to the allocator's pins).

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;

use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub floor: i64,
    pub reserve_at: i64,
    pub saved: Vec<RegId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    pub entry: Entry,
    pub releases: BTreeSet<i64>,
    pub registers: Vec<(i64, RegId)>,
    pub outgoing: BTreeSet<(i64, i64, u32)>,
    pub framed: bool,
    pub return_depth: i64,
}

impl Plan {}

#[must_use]
pub fn pins(
    body: &LirBody,
    layout: &Plan,
) -> IndexMap<u32, RegId> {
    let registers: IndexMap<i64, RegId> = layout.registers.iter().copied().collect();
    let mut result = IndexMap::default();
    for one in body.insns() {
        let Some(what) = &one.what else { continue };
        let Some(register) = registers.get(&one.at) else { continue };
        for operand in what.sources.iter().chain(&what.dests) {
            if let Loc::Held(held) = operand {
                result.insert(held.value, *register);
            }
        }
    }
    result
}
