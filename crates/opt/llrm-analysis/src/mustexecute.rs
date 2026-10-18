//! `SimpleLoopSafetyInfo`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `MustExecute.cpp`'s
//! `SimpleLoopSafetyInfo`: whether any block of a loop may throw, found by
//! asking each block whether it is guaranteed to transfer execution to its
//! successor, stopping at the first that is not.
//!
//! # Example
//! A loop that calls a function not marked `nounwind` may throw.

use llrm_mir::loopinfo::Loop;
use llrm_mir::valuetracking::guarantee::is_guaranteed_to_transfer_execution_to_successor_block;

use crate::scev::Cx;

/// `SimpleLoopSafetyInfo`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimpleLoopSafetyInfo {
    may_throw: bool,
    header_may_throw: bool,
}

impl SimpleLoopSafetyInfo {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MustExecute.cpp#L54
    /// `SimpleLoopSafetyInfo(CurLoop)` and `computeLoopSafetyInfo`.
    pub fn new(
        cx: &Cx,
        cur_loop: Loop,
    ) -> Self {
        let query = cx.query(None);
        let header = cur_loop.get_header();
        // Iterate over header and compute safety info.
        let header_may_throw = !is_guaranteed_to_transfer_execution_to_successor_block(&query, header);
        let mut may_throw = header_may_throw;
        // Iterate over loop instructions and compute safety info. Skip header
        // as it has been computed and stored in HeaderMayThrow. The first
        // block in loopinfo.Blocks is guaranteed to be the header.
        for &block in cur_loop.get_blocks().iter().skip(1) {
            may_throw |= !is_guaranteed_to_transfer_execution_to_successor_block(&query, block);
            if may_throw {
                break;
            }
        }
        Self { may_throw, header_may_throw }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MustExecute.cpp#L50
    /// `anyBlockMayThrow`.
    pub fn any_block_may_throw(&self) -> bool {
        self.may_throw
    }

    /// `HeaderMayThrow`.
    pub fn header_may_throw(&self) -> bool {
        self.header_may_throw
    }
}

#[cfg(test)]
mod tests {
    use llrm_mir::domtree::DomTree;
    use llrm_mir::loopinfo::LoopInfo;
    use llrm_mir::valuetracking::AssumptionCache;

    use super::*;

    /// `anyBlockMayThrow` of the loop headed by `%h` in `@f`.
    fn may_throw(body: &str) -> bool {
        let text = format!(
            "declare void @plain()
declare void @quiet() nounwind willreturn

define void @f(i32 %n) {{
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %b ]
  br label %b

b:
{body}  %i2 = add i32 %i, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret void
}}
"
        );
        let module = llrm_mir::parse::module(&text).expect("a module");
        let (_, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("@f");
        let layout = llrm_mir::datalayout::DataLayout::default();
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let assumptions = AssumptionCache::new(&module, function);
        let cx = Cx {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout: &layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let header = *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).unwrap();
        SimpleLoopSafetyInfo::new(&cx, loops.get_loop_for(header).expect("a loop")).any_block_may_throw()
    }

    #[test]
    fn test_a_loop_may_throw_when_a_call_in_a_non_header_block_may() {
        assert!(!may_throw(""), "no call");
        assert!(!may_throw("  call void @quiet()\n"), "nounwind willreturn");
        assert!(may_throw("  call void @plain()\n"), "a call that may unwind or not return");
    }
}
