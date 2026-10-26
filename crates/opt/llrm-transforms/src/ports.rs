//! Each I/O port call narrowed to the memory its device reaches, as the
//! target describes the port.
//!
//! # How it works
//! `llrm.ia16.in` and `out` intrinsic calls with no `memory` attribute are
//! candidates. A call whose port range the target says reaches no memory
//! (`Machine::port_touches_memory`; every target answers from its platform's
//! port table, `ports.toml` for the PC, so the 16-bit and the flat DOS
//! targets alike) gets `memory(inaccessiblemem: readwrite)`: it touches
//! only its own state, so alias and dead-store passes may move memory
//! operations across it, and port calls still keep their order among
//! themselves. A call to a port that may reach memory (a DMA controller, say)
//! keeps the unknown effect, so a store before it stays.
//!
//! The port is a constant (masked to 16 bits), or else the interval
//! ValueTracking gives it at the call (`computeConstantRange` and known bits,
//! with the dominator tree and the assumptions; a `range` attribute or an
//! assume bounds it, a branch guarding it does not); an interval that wraps
//! through 8000h gives nothing.
//!
//! ```text
//!  out 968, v    ; the DAC's write index
//!    => out 968, v  memory(inaccessiblemem: readwrite)
//!  out 11, v     ; the 8237's mode: may start DMA
//!    => unchanged
//! ```
//!
//! DEVIATE LP-1 (no pin pass: LLVM states what an I/O port reaches on the
//! intrinsic definition, ours asks the target per call; exit: the frontends
//! declare the memory attribute on the `in`/`out` intrinsics and this pass is
//! deleted; owner w-6; review 2026-11-13).
//!
//! # Example
//! Before:
//! ```mir
//! define void @f(i16 %p, i8 %v) {
//! b0:
//!   call void @llrm.ia16.out.i8(i16 968, i8 %v)
//!   call void @llrm.ia16.out.i8(i16 11, i8 %v)
//!   %low = icmp sge i16 %p, 960
//!   br i1 %low, label %b1, label %b3
//!
//! b1:
//!   %high = icmp sle i16 %p, 975
//!   br i1 %high, label %b2, label %b3
//!
//! b2:
//!   call void @llrm.ia16.out.i8(i16 %p, i8 %v)
//!   br label %b3
//!
//! b3:
//!   call void @llrm.ia16.out.i8(i16 %p, i8 %v)
//!   ret void
//! }
//! ```
//! After:
//! ```mir
//! define void @f(i16 %p, i8 %v) {
//! b0:
//!   call void @llrm.ia16.out.i8(i16 968, i8 %v) memory(inaccessiblemem: readwrite)
//!   call void @llrm.ia16.out.i8(i16 11, i8 %v)
//!   %low = icmp sge i16 %p, 960
//!   br i1 %low, label %b1, label %b3
//!
//! b1:
//!   %high = icmp sle i16 %p, 975
//!   br i1 %high, label %b2, label %b3
//!
//! b2:
//!   call void @llrm.ia16.out.i8(i16 %p, i8 %v) memory(inaccessiblemem: readwrite)
//!   br label %b3
//!
//! b3:
//!   call void @llrm.ia16.out.i8(i16 %p, i8 %v)
//!   ret void
//! }
//! ```
//! Port 968 (the VGA's DAC write index) and the call in `b2`, which the
//! branches bound to the VGA's 960..975, are narrowed. Port 11 (the 8237's mode
//! register, which can start DMA) and the call in `b3`, whose port nothing
//! bounds, keep the unknown effect.
//!
//! From unit test
//! `test_a_port_call_is_narrowed_only_where_the_target_says_its_device_reaches_no_memory`
//! in
//! `ports_tests.rs`, on the x86-16 DOS target.
//!
//! # Runs
//! Twice: once over every body before the per-body pipeline, so a caller's
//! pipeline sees each callee's port calls narrowed, and as the second pass of
//! each round of [`crate::pipeline`]'s fixed point, before anything asks what a
//! port call does to memory. On at every level above -O0.
//!
//! # Needs
//! The target's port description, `Registers`, `Shape`.
//!
//! # Keeps
//! `Dominators` and `Loops`: only call attributes change.

use llrm_analysis::memory::Unit;
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::ranges;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::valuetracking::AssumptionCache;

pub struct Ports;

impl FunctionPass for Ports {
    fn name(&self) -> &'static str {
        "ports"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let silent = silent(unit, analyses);
        for &inst in &silent {
            unit.function.add_call_attr(
                inst,
                Attribute::Memory(vec![(Some("inaccessiblemem".to_owned()), "readwrite".to_owned())]),
            );
        }
        if silent.is_empty() {
            PreservedAnalyses::all()
        } else {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        }
    }
}

/// The port calls, not yet narrowed, whose ports the target says reach no
/// memory.
fn silent(
    unit: &passes::Unit,
    analyses: &mut Analyses,
) -> Vec<InstId> {
    let held = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
    let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
    let outer = std::rc::Rc::clone(analyses.outer());
    let memory =
        Unit::within(unit.context, unit.layout, unit.function, &outer).with_registers(&held).with_shape(&shape);
    let calls: Vec<_> = unit
        .function
        .walk()
        .filter(|&(_, inst)| matches!(
            memory.intrinsic(inst),
            Some(Intrinsic::PortIn | Intrinsic::PortOut)
        ))
        .filter(|&(_, inst)| match &unit.function.instruction(inst).opcode {
            Opcode::Call(info) | Opcode::Invoke(info) => {
                !info.attrs.iter().any(|attr| matches!(attr, Attribute::Memory(_)))
            }
            _ => false,
        })
        .collect();
    let target = outer.target();
    let tree = shape.dominance.tree(unit.function);
    let assumptions = std::cell::OnceCell::new();
    calls
        .into_iter()
        .filter(|&(_, inst)| {
            let port = unit.function.instruction(inst).operands[0];
            let ports = match memory.int_constant(port) {
                Some(bits) => Some(((bits & 0xFFFF) as i64, (bits & 0xFFFF) as i64)),
                None => {
                    let assumptions = assumptions.get_or_init(|| {
                        AssumptionCache::of(unit.context, &outer.globals, &outer.metadata, unit.function)
                    });
                    ranges::value_tracking_at(&memory, Some(tree), Some(assumptions), inst, port)
                        .and_then(|interval| unsigned(&interval.low, &interval.high))
                }
            };
            ports.is_some_and(|ports| !target.port_touches_memory(ports))
        })
        .map(|(_, inst)| inst)
        .collect()
}

/// A signed word interval as the port numbers it holds, where it does not
/// wrap through 8000h.
fn unsigned(
    low: &BigInt,
    high: &BigInt,
) -> Option<(i64, i64)> {
    let (low, high) = (i64::try_from(low).ok()?, i64::try_from(high).ok()?);
    match (low < 0, high < 0) {
        (false, false) => Some((low, high)),
        (true, true) => Some((low + 0x10000, high + 0x10000)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
