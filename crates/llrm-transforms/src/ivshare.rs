//! Counters of one loop that step alike, made one: LLVM's IndVarSimplify
//! eliminating congruent induction variables (`replaceCongruentIVs`), and
//! also a counter a constant distance ahead of another, which becomes that
//! one plus the distance.
//!
//! Adapted from llrm-core's `optimize/ivshare.rs`. The old one made one
//! change a call and left the next to the pipeline's next round; this one
//! repeats to a fixed point. The replaced phi goes; its step is left for
//! `dead`.
//!
//! Dropped, with no rich-MIR counterpart: a long pair's high half tied to
//! a counter (`transform::halves`, the merged upper word), and the seed an
//! invented add claimed bytes beside.
//!
//! The old tests are skipped there: their fixture spied on
//! `strength.reduced`. These are new.

use llrm_analysis::induction::{self, Affine, AffineOperand};
use llrm_analysis::{cfg, memory};
use llrm_graph::loops;
use llrm_mir::module::{InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{Outer, Unit};
use llrm_support::hash::IndexMap;

use crate::strength::{_emitted, _operand};

/// What a counter's phi becomes.
enum Shared {
    /// Another counter's phi, stepping alike from the same start.
    Twin(ValueId),
    /// Another counter's phi plus a constant.
    Offset(ValueId, AffineOperand),
}

/// Every counter another of its loop steps alike to, replaced by that one;
/// whether any was.
pub fn shared(unit: &mut Unit, outer: &Outer) -> bool {
    let mut changed = false;
    loop {
        let found = _found(&memory::Unit::within(unit.context, unit.layout, unit.function, outer));
        let Some((phi, shared)) = found else { return changed };
        let result = unit.function.instruction(phi).result.expect("a phi's value");
        let with = match shared {
            Shared::Twin(root) => Operand::Value(root),
            Shared::Offset(root, distance) => {
                let header = unit.function.parent(phi).expect("a placed phi");
                let first = *unit.function.block(header).instructions().iter().find(|&&one| unit.function.instruction(one).opcode != Opcode::Phi).expect("a terminator");
                let distance = _operand(unit, &distance);
                let ty = unit.function.value(result).ty;
                _emitted(unit, Opcode::Binary(BinaryOp::Add), ty, vec![Operand::Value(root), distance], first)
            }
        };
        unit.function.replace_all_uses_with(result, with);
        unit.function.set_operands(phi, Vec::new());
        unit.function.erase(phi).expect("its uses were replaced");
        changed = true;
    }
}

/// The first counter of any loop another counter replaces, and how.
fn _found(view: &memory::Unit) -> Option<(InstId, Shared)> {
    let function = view.function;
    for loop_ in loops::loops(&cfg::graph(function), None) {
        let counters = induction::basics(view, &loop_);
        for derived in counters.values() {
            let ValueDef::Instruction(phi) = function.value(derived.value).def else { continue };
            if let Some(twin) = _twin(&counters, derived) {
                return Some((phi, Shared::Twin(twin.value)));
            }
            if let Some((base, distance)) = _offset(view, &counters, derived) {
                return Some((phi, Shared::Offset(base.value, distance)));
            }
        }
    }
    None
}

/// Another counter stepping as `derived` does a constant distance behind
/// it, and the distance.
///
/// Two starts are that far apart when `induction::distance` says so: an
/// add of a constant to the other's start, or strength reduction's `a[i].x`
/// and `a[i].y`, starting 4 apart. A counter whose start adds to another's
/// start takes that one; otherwise the canonical counter is the lower id,
/// so the pair converges.
fn _offset<'a>(view: &memory::Unit, counters: &'a IndexMap<ValueId, Affine>, derived: &Affine) -> Option<(&'a Affine, AffineOperand)> {
    if !matches!(derived.step, AffineOperand::Const(_)) {
        return None;
    }
    let width = derived.start.width();
    let seed = match derived.start {
        AffineOperand::Value(start, _) => view.defining(Operand::Value(start)).map(|(_, op)| op),
        AffineOperand::Const(_) => None,
    };
    let direct = match seed {
        Some(seed) if seed.opcode == Opcode::Binary(BinaryOp::Add) => &seed.operands[..],
        _ => &[],
    };
    let seeded = |one: &Affine| direct.iter().any(|&operand| induction::term(view, operand).as_ref() == Some(&one.start));
    let mut ordered = counters.values().collect::<Vec<_>>();
    ordered.sort_by_key(|one| (!seeded(one), one.value));
    for one in ordered {
        if one.value == derived.value || one.step != derived.step || (!seeded(one) && one.value > derived.value) || one.start.width() != width {
            continue;
        }
        let Some(distance) = induction::distance(view, &derived.start, &one.start, width) else { continue };
        return Some((one, AffineOperand::constant(distance, width)));
    }
    None
}

/// Another counter of this loop, of a lower id, that advances identically.
fn _twin<'a>(counters: &'a IndexMap<ValueId, Affine>, derived: &Affine) -> Option<&'a Affine> {
    counters.values().find(|one| one.value < derived.value && one.start == derived.start && one.step == derived.step)
}

#[cfg(test)]
#[path = "ivshare_tests.rs"]
mod tests;
