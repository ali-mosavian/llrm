//! InstCombine's dead `llvm.experimental.noalias.scope.decl`: `AliasScopeTracker`
//! ([InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5975)).
//!
//! # How it works
//! A scope declaration only matters if some instruction is in the scope
//! (`!alias.scope`) and some other is outside it (`!noalias`). While the
//! worklist is seeded, each instruction's two scope lists, and the scopes in
//! them, are recorded; a declaration whose scope is missing from either set is
//! deleted.
//!
//! # Example
//! ```text
//! call void @llvm.experimental.noalias.scope.decl(metadata !1)   ==>   (gone)
//! ; no instruction carries !alias.scope !1 and !noalias !1
//! ```
//!
//! MIRROR: the two sets and the test.
//!
//! # Used by
//! `combine` (`prepare`, which seeds the worklist).

use std::collections::BTreeSet;

use llrm_mir::module::{Function, InstId, MetadataId, MetadataNode, MetadataOperand};

#[derive(Default)]
pub struct AliasScopeTracker {
    used_alias_scopes_and_lists: BTreeSet<MetadataId>,
    used_noalias_scopes_and_lists: BTreeSet<MetadataId>,
}

impl AliasScopeTracker {
    /// `analyse`: record the scope lists `inst` carries.
    pub fn analyse(
        &mut self,
        function: &Function,
        metadata: &[MetadataNode],
        inst: InstId,
    ) {
        for (kind, list) in &function.instruction(inst).metadata {
            let container = match kind.as_str() {
                "alias.scope" => &mut self.used_alias_scopes_and_lists,
                "noalias" => &mut self.used_noalias_scopes_and_lists,
                _ => continue,
            };
            if !container.insert(*list) {
                continue;
            }
            for operand in &metadata[list.0 as usize].operands {
                if let MetadataOperand::Node(scope) = operand {
                    container.insert(*scope);
                }
            }
        }
    }

    /// `isNoAliasScopeDeclDead`.
    pub fn is_no_alias_scope_decl_dead(
        &self,
        function: &Function,
        metadata: &[MetadataNode],
        inst: InstId,
    ) -> bool {
        let Some((_, list)) =
            function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "noalias.scope.decl")
        else {
            return false;
        };
        match metadata[list.0 as usize].operands.first() {
            Some(MetadataOperand::Node(scope)) => {
                !self.used_alias_scopes_and_lists.contains(scope) || !self.used_noalias_scopes_and_lists.contains(scope)
            }
            // Not a node: throw it away.
            _ => true,
        }
    }
}

#[cfg(test)]
#[path = "combine_alias_scope_tests.rs"]
mod tests;
