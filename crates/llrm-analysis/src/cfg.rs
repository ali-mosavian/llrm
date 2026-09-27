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
}
