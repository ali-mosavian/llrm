//! A function's blocks as `llrm_graph` walks them: each block by its id,
//! in layout order, the entry first.

use llrm_graph::loops::Node;
use llrm_mir::module::{BlockId, Function};

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
