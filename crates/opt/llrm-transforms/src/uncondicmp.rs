//! A block that holds an equality compare of a switch's value with a constant
//! (and perhaps a `select` on it), and jumps on, folded into the switch:
//! LLVM's
//! [`tryToSimplifyUncondBranchWithICmpSelectInIt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5328)
//! and its `ICmpInIt` form (a compare alone), the parts of
//! `simplifyUncondBranch`
//! ([L8800](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8800))
//! after the empty block's removal.
//!
//! The block's only predecessor is a `switch` on the compared value. Reached
//! by a case, the value is that case's, and the compare folds; reached by the
//! default, a compare with a value some case has is known, else the compare's
//! constant becomes a case of its own, to a block that jumps to the join with
//! the value the compare chose:
//!
//! # Example
//! ```text
//!   switch i8 %a, label %d [ 0, %end  1, %c ]        switch i8 %a, label %d [ 0, %end  1, %c  2, %edge ]
//!   d:  %t = icmp eq i8 %a, 2                  ==>   d:    br %end
//!       %v = select i1 %t, i8 C3, i8 C4              edge: br %end
//!       br %end                                      end: phi [ C1, ..], [ C4, %d ], [ C3, %edge ]
//! ```
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | No branch weights: the new case carries none. |
//! | D2 | A block reached by two cases of the switch is left (LLVM asserts a unique case value). |

use llrm_analysis::occurrence::{arms, from_arms};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::simplify::simplify_instruction;

use crate::edges;

/// `block`'s compare (and select) folded into the switch that alone reaches
/// it: whether it was.
pub fn folded(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let (Opcode::Br, &[Operand::Block(join)]) =
        (&function.instruction(last).opcode, &function.instruction(last).operands[..])
    else {
        return false;
    };
    let mut rest = function.iter_block(block).skip_while(|&inst| function.instruction(inst).opcode == Opcode::Phi);
    let Some(compare) = rest.next() else { return false };
    let Opcode::ICmp(predicate) = function.instruction(compare).opcode else { return false };
    if !matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) {
        return false;
    }
    let &[tested, case] = &function.instruction(compare).operands[..] else { return false };
    let Some(case_value) = int(context, case) else { return false };
    let selected = match rest.next() {
        Some(next) if next == last => None,
        Some(next) if function.instruction(next).opcode == Opcode::Select && rest.next() == Some(last) => Some(next),
        _ => return false,
    };
    drop(rest);
    on_switch(context, layout, function, block, join, compare, selected, predicate, tested, case, case_value)
}

/// The shared body of the two forms: `selected` is the `select` on the compare
/// where there is one.
#[allow(clippy::too_many_arguments)]
fn on_switch(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    block: BlockId,
    join: BlockId,
    compare: InstId,
    selected: Option<InstId>,
    predicate: IntPredicate,
    tested: Operand,
    case: Operand,
    case_value: u128,
) -> bool {
    // Phis in the block, or more than one reader of the compare or the select,
    // are too complex.
    if edges::phis(function, block).first().is_some() {
        return false;
    }
    let compare_result = function.instruction(compare).result.expect("a compare's value");
    let only_user = |function: &Function, inst: InstId| -> Option<InstId> {
        let value = function.instruction(inst).result?;
        function.users(value).only().map(|one| one.user)
    };
    let Some(compare_user) = only_user(function, compare) else { return false };
    let (select_values, user) = match selected {
        None => (None, compare_user),
        Some(select) => {
            let Some(user) = only_user(function, select) else { return false };
            let &[condition, if_true, if_false] = &function.instruction(select).operands[..] else { return false };
            if condition != Operand::Value(compare_result) {
                return false;
            }
            (Some((if_true, if_false)), user)
        }
    };
    let [pred] = function.predecessors(block)[..] else { return false };
    let Some(switch) = function.terminator(pred) else { return false };
    if function.instruction(switch).opcode != Opcode::Switch {
        return false;
    }
    let operands = function.instruction(switch).operands.clone();
    if operands[0] != tested {
        return false;
    }
    let Operand::Block(default) = operands[1] else { return false };
    let cases: Vec<(Operand, BlockId)> = operands[2..]
        .chunks(2)
        .map(|pair| match pair {
            [value, Operand::Block(target)] => (*value, *target),
            _ => unreachable!("a switch case is a value and a block"),
        })
        .collect();
    let i1 = context.types.int(1);
    let boolean = |value: bool| {
        Operand::Constant(context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(value)) }))
    };

    // Reached by a case: the value is the case's, and the compare folds.
    if default != block {
        let mut reaching = cases.iter().filter(|&&(_, target)| target == block);
        let (Some(&(value, _)), None) = (reaching.next(), reaching.next()) else { return false };
        function.set_operand(compare, 0, value);
        if let Some(simple) = simplify_instruction(context, layout, function, compare) {
            function.replace_value(compare_result, simple);
            function.erase(compare).expect("a compare nothing reads");
        }
        // The block is empty now, so it simplifies away.
        return true;
    }

    // Reached by the default: a compare with a value some case has is known.
    if cases.iter().any(|&(value, _)| int(context, value) == Some(case_value)) {
        function.replace_value(compare_result, boolean(predicate != IntPredicate::Eq));
        function.erase(compare).expect("a compare nothing reads");
        return true;
    }

    // The use of the select has to be the one phi of the join.
    let phi = match function.instruction(user).opcode {
        Opcode::Phi => user,
        _ => return false,
    };
    let join_phis = edges::phis(function, join);
    if join_phis != [phi] || function.parent(phi) != Some(join) {
        return false;
    }

    // The default gets the value of a compare that is false, the new case the
    // other.
    let (mut for_default, mut for_case) = match select_values {
        Some((if_true, if_false)) => (if_false, if_true),
        None => (boolean(false), boolean(true)),
    };
    if predicate == IntPredicate::Ne {
        std::mem::swap(&mut for_default, &mut for_case);
    }
    let replaced = selected.unwrap_or(compare);
    let replaced_result = function.instruction(replaced).result.expect("a value");
    function.replace_value(replaced_result, for_default);
    if selected.is_some() {
        function.erase(replaced).expect("a select nothing reads");
    }
    function.erase(compare).expect("a compare nothing reads");

    // The switch goes to the join on the compared value, through a new block.
    let edge = function.create_block(Some("switch.edge"));
    // Right before the block of the compare (SimplifyCFG.cpp:5463).
    function.insert_block_before(edge, block).expect("a new block");
    let mut with_case = operands.clone();
    with_case.extend([case, Operand::Block(edge)]);
    function.set_operands(switch, with_case);
    let void = function.instruction(switch).ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(join)], Flags::default(), None);
    function.insert(jump, Position::End(edge)).expect("a placed block");
    let mut incoming = arms(function, phi);
    incoming.push((for_case, edge));
    function.set_operands(phi, from_arms(&incoming));
    true
}

/// The value of an integer constant.
fn int(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
#[path = "uncondicmp_tests.rs"]
mod tests;
