//! Every value an instruction or terminator uses is defined where the
//! definition dominates the use: earlier in the same block, or in a block
//! that dominates it. A parameter dominates everything. The one dominator
//! computation is `llrm-support`'s (`graph`).

use llrm_support::graph::{Node, dominance};
use llrm_support::hash::HashMap;

use crate::model::{Function, Operand};

struct Edges {
    id: i64,
    to: Vec<i64>,
}

impl Node for Edges {
    fn at(&self) -> i64 {
        self.id
    }

    fn succ(&self) -> &[i64] {
        &self.to
    }
}

/// The values `operand` reads: the value itself, a place's base or origin,
/// the values of its indices.
fn values(
    operand: &Operand,
    out: &mut Vec<i64>,
) {
    match operand {
        Operand::ValueRef(one) => out.push(one.value),
        Operand::Constant(_) | Operand::PlaceRef(_) => {}
        Operand::ArrayElement(one) => one.indices.iter().for_each(|index| values(index, out)),
        Operand::ProjectedPlace(one) => one.indices.iter().for_each(|index| values(index, out)),
        Operand::IndirectPlace(one) => {
            out.push(one.base);
            out.extend(one.origin);
        }
        Operand::DescriptorPlace(one) => out.push(one.base),
    }
}

/// The first use of `function` whose definition does not dominate it.
///
/// Every entry (the first, and RESUME's) is reached from a root of its own
/// that nothing else dominates, so a definition on the way to one entry does
/// not reach another. A block no entry reaches is not checked.
pub fn check(function: &Function) -> Result<(), String> {
    let root = function.blocks.iter().map(|one| one.id).min().unwrap_or(0) - 1;
    let entries = std::iter::once(function.entry).chain(function.external_entries.iter().copied());
    let mut edges: Vec<Edges> =
        function.blocks.iter().map(|one| Edges { id: one.id, to: one.terminator.targets.clone() }).collect();
    edges.push(Edges { id: root, to: entries.collect() });
    let dominance = dominance(&edges, Some(root));
    // Where each value is defined: block, and position in it.
    let mut defined: HashMap<i64, (i64, usize)> = HashMap::default();
    for block in &function.blocks {
        for (at, instruction) in block.instructions.iter().enumerate() {
            for &result in &instruction.results {
                defined.insert(result, (block.id, at));
            }
        }
    }
    for block in function.blocks.iter().filter(|one| dominance.reachable(one.id)) {
        let uses = block
            .instructions
            .iter()
            .enumerate()
            .map(|(at, one)| (at, format!("instruction {}", one.id), one.operands.as_slice()));
        let end = std::iter::once((
            block.instructions.len(),
            format!("the terminator of block {}", block.id),
            block.terminator.operands.as_slice(),
        ));
        for (at, whose, operands) in uses.chain(end) {
            let mut used = Vec::new();
            operands.iter().for_each(|one| values(one, &mut used));
            for value in used {
                // Not defined by an instruction: a parameter.
                let Some(&(home, position)) = defined.get(&value) else { continue };
                let reaches = if home == block.id { position < at } else { dominance.dominates(home, block.id) };
                if !reaches {
                    return Err(format!(
                        "{whose} uses value {value}, defined in block {home}, which does not dominate it"
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::model::{Block, Function, Instruction, Op, Operand, Terminator, TerminatorKind, Value};

    fn add(
        id: i64,
        result: i64,
        of: i64,
    ) -> Instruction {
        Instruction::new(id, Op::Add, vec![result], vec![Operand::value_ref(of), Operand::value_ref(of)])
    }

    fn jump(to: i64) -> Terminator {
        Terminator::new(TerminatorKind::Jump, Vec::new(), vec![to])
    }

    fn returns(value: i64) -> Terminator {
        Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(value)], Vec::new())
    }

    /// Block 1 branches to 2 and 3, which join in 4. The value 5 is defined in
    /// 2.
    fn diamond(
        join: Terminator,
        left: Vec<Instruction>,
    ) -> Function {
        let values = (1..=6).map(|id| Value { id, r#type: 1 }).collect();
        let branch = Terminator::new(TerminatorKind::Branch, vec![Operand::value_ref(1)], vec![2, 3]);
        let blocks = vec![
            Block::new(1, Vec::new(), branch),
            Block::new(2, left, jump(4)),
            Block::new(3, Vec::new(), jump(4)),
            Block::new(4, vec![add(9, 6, 1)], join),
        ];
        let mut function = Function::new(1, "f", 1, values, Vec::new(), blocks, 1);
        function.parameters = vec![1];
        function
    }

    /// A value defined in one arm and used in the join, where the other arm
    /// reaches it too, was accepted: the lowering then read a register the
    /// path through the other arm never wrote.
    #[test]
    fn a_use_the_definition_does_not_dominate_is_refused() {
        assert_eq!(check(&diamond(returns(6), vec![add(7, 5, 1)])), Ok(()));
        let error = check(&diamond(returns(5), vec![add(7, 5, 1)])).unwrap_err();
        assert!(error.contains("uses value 5, defined in block 2, which does not dominate"), "{error}");
    }

    /// A use above its definition in the same block is not dominated.
    #[test]
    fn a_use_before_its_definition_in_a_block_is_refused() {
        let values = (1..=3).map(|id| Value { id, r#type: 1 }).collect();
        let block = Block::new(1, vec![add(1, 2, 3), add(2, 3, 1)], returns(2));
        let mut function = Function::new(1, "f", 1, values, Vec::new(), vec![block], 1);
        function.parameters = vec![1];
        assert!(check(&function).unwrap_err().contains("instruction 1 uses value 3"));
    }

    /// A definition on the way to the first entry does not reach another entry
    /// (RESUME's): a use past the other entry was accepted.
    #[test]
    fn a_definition_reaching_only_one_of_two_entries_is_refused() {
        let values = (1..=4).map(|id| Value { id, r#type: 1 }).collect();
        let blocks = vec![Block::new(1, vec![add(1, 2, 1)], jump(2)), Block::new(2, vec![add(2, 3, 2)], returns(3))];
        let mut function = Function::new(1, "f", 1, values, Vec::new(), blocks, 1);
        function.parameters = vec![1];
        assert_eq!(check(&function), Ok(()));
        function.external_entries = vec![2];
        let error = check(&function).unwrap_err();
        assert!(error.contains("instruction 2 uses value 2, defined in block 1"), "{error}");
    }
}
