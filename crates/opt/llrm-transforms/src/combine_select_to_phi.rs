//! `foldSelectToPhi`: a select on a dominating branch's condition is a phi.
//!
//! Translation of
//! [`foldSelectToPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2696)
//! and `foldSelectToPhiImpl` at d1106deb71cc.
//!
//! MIRROR: the candidate blocks (the select's and its operands'), the
//! immediate dominator's branch on the condition or its `not`, edge
//! dominance, availability of each incoming value.
//!
//! # Example
//! ```text
//! b: br i1 %c, label %t, label %f                  m: %r = phi i16 [ -1, %t ], [ %i, %f ]
//! m: %r = select i1 %c, i16 -1, i16 %i   ==>
//! ```

use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;
use crate::combine_visit_select::Sel;

pub(crate) fn select_to_phi(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let mut candidates = vec![c.function.parent(inst)?];
    for operand in [sel.cond, sel.if_true, sel.if_false] {
        if let Some(def) = c.definition_of(operand)
            && let Some(block) = c.function.parent(def)
            && !candidates.contains(&block)
        {
            candidates.push(block);
        }
    }
    for block in candidates {
        let Some(incoming) = incoming_values(c, sel, block) else { continue };
        let at = c.first_insertion_point(block);
        let operands = incoming.into_iter().flat_map(|(value, from)| [value, Operand::Block(from)]).collect();
        let phi = c.insert_before(at, Opcode::Phi, sel.ty, operands, Flags::default());
        let result = c.function.instruction(phi).result.expect("a phi has a value");
        return Some(Visit::Replace(Operand::Value(result)));
    }
    None
}

/// `foldSelectToPhiImpl`'s inputs: one value per predecessor of `block`, None
/// if the immediate dominator's branch does not decide every edge.
fn incoming_values(
    c: &Combiner,
    sel: &Sel,
    block: BlockId,
) -> Option<Vec<(Operand, BlockId)>> {
    let tree = c.env.analyses.dominator_tree();
    let idom = tree.immediate_dominator(block)?;
    let branch = c.function.instruction(c.function.terminator(idom)?);
    let [condition, Operand::Block(true_successor), Operand::Block(false_successor)] = branch.operands[..] else {
        return None;
    };
    if branch.opcode != Opcode::Br || true_successor == false_successor {
        return None;
    }
    let (if_true, if_false) = if condition == sel.cond {
        (sel.if_true, sel.if_false)
    } else if pattern::not(&query(c, None), condition) == Some(sel.cond) {
        (sel.if_false, sel.if_true)
    } else {
        return None;
    };
    // The phi takes the true arm on edges the true edge dominates, the false
    // arm on edges the false edge dominates.
    let dominates = |successor: BlockId, from: BlockId| {
        (idom, successor) == (from, block) || tree.dominates_edge(c.function, idom, successor, from)
    };
    let mut inputs = Vec::new();
    for from in c.function.predecessors(block).iter().copied() {
        let value = if dominates(true_successor, from) {
            if_true
        } else if dominates(false_successor, from) {
            if_false
        } else {
            return None;
        };
        let value = c.do_phi_translation(value, block, from);
        // Available at the end of the predecessor.
        if let Some(def) = c.definition_of(value) {
            let terminator = c.function.terminator(from)?;
            if !tree.instruction_dominates(c.function, def, terminator) {
                return None;
            }
        }
        inputs.push((value, from));
    }
    (!inputs.is_empty()).then_some(inputs)
}
