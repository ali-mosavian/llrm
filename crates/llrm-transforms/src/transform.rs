//! Helpers adapted from llrm-core's `optimize/transform.rs`, each copied as
//! a ported pass needs it; the pipeline itself is not ported yet.
//!
//! Every block ends in a terminator and a dead block leaves nothing behind:
//! the old `_unreachable` kept a dead block's operations emptied for byte
//! ownership, which the rich MIR does not have, so here the block goes.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::Opcode;

use crate::edges;
use crate::lcssa::{arms, from_arms};

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

/// Without the blocks the entry does not reach, nor phi inputs from them.
pub fn _unreachable(function: &mut Function) -> Result<(), String> {
    let entry = function.entry().expect("a defined function");
    let (mut reached, mut pending) = (BTreeSet::new(), vec![entry]);
    while let Some(at) = pending.pop() {
        if reached.insert(at) {
            pending.extend(function.successors(at));
        }
    }
    let dead = function.layout().iter().copied().filter(|block| !reached.contains(block)).collect::<Vec<_>>();
    if dead.is_empty() {
        return Ok(());
    }
    for &block in &reached {
        for phi in edges::phis(function, block) {
            let incoming = arms(function, phi).into_iter().filter(|(_, source)| !dead.contains(source)).collect::<Vec<_>>();
            if incoming.len() * 2 != function.instruction(phi).operands.len() {
                function.set_operands(phi, from_arms(&incoming));
            }
        }
    }
    let instructions = dead.iter().flat_map(|&block| function.block(block).instructions().to_vec()).collect::<Vec<_>>();
    for &inst in &instructions {
        function.set_operands(inst, Vec::new());
    }
    for inst in instructions {
        function.erase(inst)?;
    }
    for block in dead {
        function.erase_block(block)?;
    }
    Ok(())
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

    use super::{_trivial_phis, _unreachable};

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
        let function = module.function_mut("f").unwrap().1;
        _unreachable(function).unwrap();
        _trivial_phis(function).unwrap();
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
