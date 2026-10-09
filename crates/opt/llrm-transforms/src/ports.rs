//! Each I/O port call narrowed to the memory its device reaches, as the
//! target describes the port. One with no path to memory touches only its
//! own state, `inaccessiblemem`, which keeps port calls in order. A port
//! that is not a constant is narrowed where `ranges` bounds it.

use llrm_analysis::memory::Unit;
use llrm_analysis::{cfg, ranges};
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use num_bigint::BigInt;

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
    let mut bounded = None;
    let target = outer.target();
    calls
        .into_iter()
        .filter(|&(block, inst)| {
            let port = unit.function.instruction(inst).operands[0];
            let ports = match memory.int_constant(port) {
                Some(bits) => Some(((bits & 0xFFFF) as i64, (bits & 0xFFFF) as i64)),
                None => {
                    // What the counted loops bound is the manager's, asked when
                    // first needed.
                    let facts = bounded.get_or_insert_with(|| {
                        analyses.get::<llrm_analysis::manager::Bounded>(unit.context, unit.layout, unit.function)
                    });
                    let scope = facts
                        .as_ref()
                        .as_ref()
                        .ok()
                        .and_then(|facts| facts.at(cfg::id(block)))
                        .cloned()
                        .unwrap_or_default();
                    ranges::_operand(&memory, port, &scope, &held)
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
