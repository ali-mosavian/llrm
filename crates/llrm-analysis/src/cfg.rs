//! A function's blocks as `llrm_graph` walks them: each block by its id,
//! in layout order, the entry first.

use std::collections::BTreeSet;

use llrm_graph::loops::{self, Node};
use llrm_mir::module::{BlockId, Function, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::{Constant, ConstantKind, Context};

/// A block's id and its successors' ids.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub at: i64,
    pub succ: Vec<i64>,
}

impl Node for Block {
    fn at(&self) -> i64 {
        self.at
    }

    fn succ(&self) -> &[i64] {
        &self.succ
    }
}

pub fn id(block: BlockId) -> i64 {
    i64::from(block.0)
}

pub fn block(at: i64) -> BlockId {
    BlockId(at as u32)
}

/// `function`'s graph, in layout order.
pub fn graph(function: &Function) -> Vec<Block> {
    function.layout().iter().map(|&block| Block { at: id(block), succ: function.successors(block).into_iter().map(id).collect() }).collect()
}

/// Blocks the entry does not reach, gone, and each phi's inputs from
/// edges that no longer exist with them.
///
/// Copied from llrm-core's `optimize/transform.rs` `_unreachable`. The old
/// MIR kept a dead block as an inert byte owner; the rich MIR owns no bytes.
pub fn _unreachable(context: &mut Context, function: &mut Function) -> bool {
    let blocks = graph(function);
    let Some(entry) = blocks.first().map(|block| block.at) else { return false };
    let successors = blocks.iter().map(|block| (block.at, &block.succ)).collect::<std::collections::BTreeMap<_, _>>();
    let (mut reached, mut pending) = (BTreeSet::new(), vec![entry]);
    while let Some(at) = pending.pop() {
        if reached.insert(at) {
            pending.extend(successors[&at].iter().copied());
        }
    }
    let predecessors = loops::predecessors(&blocks);
    let mut changed = false;
    for &at in &reached {
        let live = |from: &Operand| matches!(from, Operand::Block(one) if reached.contains(&id(*one)) && predecessors.get(&at).is_some_and(|from| from.contains(&id(*one))));
        for phi in function.block(block(at)).instructions().to_vec() {
            if function.instruction(phi).opcode != Opcode::Phi {
                break;
            }
            let operands = &function.instruction(phi).operands;
            let kept: Vec<Operand> = operands.chunks(2).filter(|pair| live(&pair[1])).flatten().copied().collect();
            if kept.len() != operands.len() {
                function.set_operands(phi, kept);
                changed = true;
            }
        }
    }
    let dead: Vec<BlockId> = blocks.iter().filter(|one| !reached.contains(&one.at)).map(|one| block(one.at)).collect();
    // Values defined there are used only there, or by phis on dropped edges.
    for &one in &dead {
        for inst in function.block(one).instructions().to_vec().into_iter().rev() {
            if let Some(result) = function.instruction(inst).result {
                let poison = context.constant(Constant { ty: function.value(result).ty, kind: ConstantKind::Poison });
                function.replace_all_uses_with(result, Operand::Constant(poison));
            }
            function.erase(inst).expect("its uses were replaced");
        }
    }
    for &one in &dead {
        function.erase_block(one).expect("an emptied block nothing names");
    }
    changed || !dead.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{function, parsed};

    #[test]
    fn a_functions_loop_is_found_through_its_graph() {
        let module = parsed("define void @f(i1 %c) {
b1:
  br label %b2

b2:
  br i1 %c, label %b2, label %b3

b3:
  ret void
}
");
        let function = function(&module, "f");
        let found = llrm_graph::loops::loops(&graph(function), None);
        assert_eq!(found.len(), 1);
        assert_eq!(block(found[0].header), function.layout()[1]);
    }

    #[test]
    fn a_switch_names_each_target_once_and_an_unreachable_block_stays_in_the_graph() {
        let module = parsed("define void @f(i16 %s) {
b1:
  switch i16 %s, label %b2 [ i16 1, label %b2
                            i16 2, label %b3 ]

b2:
  ret void

b3:
  ret void

dead:
  br label %b2
}
");
        let function = function(&module, "f");
        let [b1, b2, b3, dead] = [0, 1, 2, 3].map(|at| id(function.layout()[at]));
        assert_eq!(
            graph(function),
            [
                Block { at: b1, succ: vec![b2, b3] },
                Block { at: b2, succ: vec![] },
                Block { at: b3, succ: vec![] },
                Block { at: dead, succ: vec![b2] },
            ]
        );
    }
}
