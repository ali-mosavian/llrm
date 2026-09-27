//! Helpers adapted from llrm-core's `optimize/transform.rs`, each copied as
//! a ported pass needs it; the pipeline itself is not ported yet.
//! `_unreachable` is `llrm_analysis::cfg::_unreachable`.
//!
//! `live` is gvn's share, taken here for Dead; `halves`, which it read, has
//! no counterpart: a value here is whole.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::{Context, GlobalId};
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;

use crate::edges;
use crate::lcssa::{arms, from_arms};

/// Values something that stays reads, to a fixed point.
pub fn live(context: &Context, callees: &Callees, function: &Function) -> BTreeSet<ValueId> {
    let mut alive = BTreeSet::new();
    let mut pending = function.walk().map(|(_, inst)| inst).filter(|&inst| crate::dead::_kept(context, callees, function, inst)).collect::<Vec<_>>();
    while let Some(inst) = pending.pop() {
        for &operand in &function.instruction(inst).operands {
            if let Operand::Value(value) = operand
                && alive.insert(value)
                && let ValueDef::Instruction(defining) = function.value(value).def
            {
                pending.push(defining);
            }
        }
    }
    alive
}

/// Every function of `module` that has a body.
pub(crate) fn bodies(module: &Module) -> Vec<GlobalId> {
    module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(id, _, _)| id).collect()
}

/// The comparison supplying `branch`'s condition: the `icmp` in `block`
/// that defines it.
///
/// The old one found the flag-setting `cmp`, or a `test`-like and/or/xor
/// read for equality, whose flags the branch read; both are an `icmp` here.
/// A float compare was refused as floating work, so `fcmp` is not one.
pub fn _comparison(function: &Function, block: BlockId, branch: InstId) -> Option<InstId> {
    let instruction = function.instruction(branch);
    if instruction.opcode != Opcode::Br || instruction.operands.len() != 3 {
        return None;
    }
    let Operand::Value(condition) = instruction.operands[0] else {
        return None;
    };
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .find(|&one| function.instruction(one).result == Some(condition))
        .filter(|&one| matches!(function.instruction(one).opcode, Opcode::ICmp(_)))
}

/// Resolve single-valued joins after an edge disappears: each phi keeps
/// only its predecessors' inputs, and one naming a single other value is
/// that value.
pub fn _trivial_phis(function: &mut Function) -> Result<(), String> {
    let predecessors = function.layout().iter().map(|&block| (block, function.predecessors(block))).collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in function.layout().to_vec() {
            for phi in edges::phis(function, block) {
                let result = function.instruction(phi).result.expect("a phi's value");
                let incoming = arms(function, phi).into_iter().filter(|(_, at)| predecessors[&block].contains(at)).collect::<Vec<_>>();
                let mut values = incoming.iter().map(|&(value, _)| value).filter(|&value| value != Operand::Value(result)).collect::<Vec<_>>();
                values.dedup();
                if let [value] = values[..] {
                    function.replace_all_uses_with(result, value);
                    function.set_operands(phi, Vec::new());
                    function.erase(phi)?;
                    changed = true;
                } else if from_arms(&incoming) != function.instruction(phi).operands {
                    function.set_operands(phi, from_arms(&incoming));
                }
            }
        }
        if !changed {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::{parsed, printed};

    use super::_trivial_phis;

    /// A dead arm and the block it came from go; the join it leaves with one
    /// value is that value.
    #[test]
    fn test_a_dead_arm_leaves_a_join_that_is_its_one_value() {
        let mut module = parsed(
            "define i16 @f(i16 %x) {
b0:
  br label %b2

b1:
  %y = add i16 %x, 1
  br label %b2

b2:
  %r = phi i16 [ %x, %b0 ], [ %y, %b1 ]
  %s = add i16 %r, 2
  ret i16 %s
}
",
        );
        let mut function = module.function_mut("f").unwrap().1.clone();
        llrm_analysis::cfg::_unreachable(&mut module.context, &mut function);
        _trivial_phis(&mut function).unwrap();
        *module.function_mut("f").unwrap().1 = function;
        assert_eq!(
            printed(&module),
            "define i16 @f(i16 %x) {
b0:
  br label %b2

b2:
  %s = add i16 %x, 2
  ret i16 %s
}
"
        );
    }

    /// A phi that reads only itself and one other value is that value.
    #[test]
    fn test_a_loop_phi_of_one_outside_value_is_that_value() {
        let mut module = parsed(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  %r = phi i16 [ %x, %b0 ], [ %r, %b1 ]
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %r
}
",
        );
        _trivial_phis(module.function_mut("f").unwrap().1).unwrap();
        assert_eq!(
            printed(&module),
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %x
}
"
        );
    }
}
