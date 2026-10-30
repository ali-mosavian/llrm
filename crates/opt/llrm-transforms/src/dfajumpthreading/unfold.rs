//! `unfold`: the `select`s of a switch's state chain, replaced by control flow
//! so each of their values is set on a path of its own.

use llrm_mir::edit::Position;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};

use super::incoming_value_for_block;
use super::mainswitch::SelectToUnfold;
use crate::edges::phis;
use crate::lcssa::{arms, from_arms};

/// `DFAJumpThreading::unfoldSelectInstrs`
/// ([L166](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L166)):
/// every `select` of the work list is unfolded, and the `select`s that its
/// values were are put on the list. The new blocks join the loop of the block
/// the `select` was in (`Preserve loop info`).
pub(super) fn unfold_select_instrs(
    function: &mut Function,
    loops: &mut LoopInfo,
    selects: &[SelectToUnfold],
) {
    let mut stack = selects.to_vec();
    while let Some(next) = stack.pop() {
        let Unfolded { found, start_block, new_blocks } = unfold(function, next);
        stack.extend(found);
        if let Some(one) = loops.get_loop_for(start_block).map(|one| one.id()) {
            loops.add_basic_blocks_to_loop(one, &new_blocks);
        }
    }
}

/// What `unfold` made of a `select`.
struct Unfolded {
    /// The `select`s newly found, for the work list.
    found: Vec<SelectToUnfold>,
    /// The block the `select` was set in (`StartBlock`).
    start_block: BlockId,
    /// The blocks made (`NewBBs`).
    new_blocks: Vec<BlockId>,
}

/// The `select` that defines `value`, if it is one.
fn select_of(
    function: &Function,
    value: Operand,
) -> Option<InstId> {
    let Operand::Value(id) = value else { return None };
    match function.value(id).def {
        ValueDef::Instruction(inst) if function.instruction(inst).opcode == Opcode::Select => Some(inst),
        _ => None,
    }
}

/// `Value::getName`: the name of `value`, empty if it has none.
fn name_of(
    function: &Function,
    value: Operand,
) -> String {
    match value {
        Operand::Value(id) => function.value(id).name.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

/// A `phi` of `ty` at the first insertion point of `block` (after its phis).
fn place_phi(
    function: &mut Function,
    block: BlockId,
    ty: llrm_mir::types::TypeId,
    operands: Vec<Operand>,
    name: &str,
) -> InstId {
    let phi = function.create_instruction(Opcode::Phi, ty, operands, Flags::default(), Some(name));
    let first = function
        .iter_block(block)
        .find(|&one| function.instruction(one).opcode != Opcode::Phi)
        .expect("a terminated block");
    function.insert(phi, Position::Before(first)).expect("a placed phi");
    phi
}

/// `PHINode::addIncoming`.
fn add_incoming(
    function: &mut Function,
    phi: InstId,
    value: Operand,
    block: BlockId,
) {
    let mut operands = function.instruction(phi).operands.clone();
    operands.extend([value, Operand::Block(block)]);
    function.set_operands(phi, operands);
}

/// `PHINode::removeIncomingValue(BB)`: the first input from `block`; the last
/// input takes its place.
fn remove_incoming(
    function: &mut Function,
    phi: InstId,
    block: BlockId,
) {
    let mut kept = arms(function, phi);
    if let Some(at) = kept.iter().position(|&(_, from)| from == block) {
        kept.swap_remove(at);
    }
    function.set_operands(phi, from_arms(&kept));
}

/// `replaceUsesOfWith(select, with)` on `user`.
fn replace_uses_of(
    function: &mut Function,
    user: InstId,
    select: InstId,
    with: Operand,
) {
    let value = Operand::Value(function.instruction(select).result.expect("a select's value"));
    let operands =
        function.instruction(user).operands.iter().map(|&one| if one == value { with } else { one }).collect();
    function.set_operands(user, operands);
}

/// `DFAJumpThreading::unfold`
/// ([L201](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L201)):
/// the `select` held in `to_unfold` replaced by control flow; the `select`s
/// newly found are returned for the work list, with the blocks made.
fn unfold(
    function: &mut Function,
    to_unfold: SelectToUnfold,
) -> Unfolded {
    let SelectToUnfold { select, mut user } = to_unfold;
    let result = function.instruction(select).result.expect("a select's value");
    let [condition, true_value, false_value] = function.instruction(select).operands[..] else {
        unreachable!("a select has a condition and two values")
    };
    let select_name = function.value(result).name.clone().unwrap_or_default();
    let ty = function.instruction(user).ty;
    let only = function.users(result).only().expect("a select to unfold has one use");
    debug_assert_eq!(only.user, user);
    // The select may come indirectly, instead of from where it is defined.
    let Operand::Block(start_block) = function.instruction(user).operands[only.index as usize + 1] else {
        unreachable!("a phi's input names its block")
    };
    let start_term = function.terminator(start_block).expect("a terminated block");
    let void = function.instruction(start_term).ty;
    let mut found = Vec::new();
    let new_blocks;

    if function.instruction(start_term).opcode == Opcode::Br && function.instruction(start_term).operands.len() == 1 {
        let Operand::Block(end_block) = function.instruction(start_term).operands[0] else {
            unreachable!("a jump names its target")
        };
        // Arbitrarily choose the 'false' side for a new input value to the PHI.
        let new_block = function.create_block(Some(&format!("{select_name}.si.unfold.false")));
        function.insert_block_before(new_block, end_block).expect("a new block");
        let to_end =
            function.create_instruction(Opcode::Br, void, vec![Operand::Block(end_block)], Flags::default(), None);
        function.insert(to_end, Position::End(new_block)).expect("a placed block");

        // StartBlock
        //   |  \
        //   |  NewBlock
        //   |  /
        // EndBlock
        let new_phi = function.create_instruction(
            Opcode::Phi,
            ty,
            vec![false_value, Operand::Block(start_block)],
            Flags::default(),
            Some(&format!("{}.si.unfold.phi", name_of(function, false_value))),
        );
        function.insert(new_phi, Position::Before(to_end)).expect("a placed phi");
        new_blocks = vec![new_block];
        let new_phi_value = Operand::Value(function.instruction(new_phi).result.expect("a phi's value"));

        // Update any other PHI nodes in EndBlock.
        for phi in phis(function, end_block) {
            if phi == user {
                continue;
            }
            let value = incoming_value_for_block(function, phi, start_block).expect("an input from the start");
            add_incoming(function, phi, value, new_block);
        }

        // Update the phi node of SI, which is its only use.
        if function.parent(user) == Some(end_block) {
            add_incoming(function, user, new_phi_value, new_block);
            replace_uses_of(function, user, select, true_value);
        } else {
            let end_phi = place_phi(function, end_block, ty, Vec::new(), &format!("{select_name}.si.unfold.phi"));
            let end_phi_value = Operand::Value(function.instruction(end_phi).result.expect("a phi's value"));
            let mut operands = Vec::new();
            for pred in function.predecessors(end_block) {
                if pred != start_block && pred != new_block {
                    operands.extend([end_phi_value, Operand::Block(pred)]);
                }
            }
            operands.extend([true_value, Operand::Block(start_block), new_phi_value, Operand::Block(new_block)]);
            function.set_operands(end_phi, operands);
            replace_uses_of(function, user, select, end_phi_value);
            user = end_phi;
        }

        if let Some(op) = select_of(function, true_value) {
            found.push(SelectToUnfold { select: op, user });
        }
        if let Some(op) = select_of(function, false_value) {
            found.push(SelectToUnfold { select: op, user: new_phi });
        }

        // Insert the real conditional branch based on the original condition.
        function.erase(start_term).expect("a jump nothing uses");
        let branch = function.create_instruction(
            Opcode::Br,
            void,
            vec![condition, Operand::Block(end_block), Operand::Block(new_block)],
            Flags::default(),
            None,
        );
        function.insert(branch, Position::End(start_block)).expect("a placed block");
    } else {
        let end_block = function.parent(user).expect("a placed phi");
        let new_true = function.create_block(Some(&format!("{select_name}.si.unfold.true")));
        let new_false = function.create_block(Some(&format!("{select_name}.si.unfold.false")));
        function.insert_block_before(new_true, end_block).expect("a new block");
        function.insert_block_before(new_false, end_block).expect("a new block");
        new_blocks = vec![new_true, new_false];

        // Def only has one use in EndBlock.
        // Before transformation:
        // StartBlock(Def)
        //   |      \
        // EndBlock  OtherBlock
        //  (Use)
        //
        // After transformation:
        // StartBlock(Def)
        //   |      \
        //   |       OtherBlock
        // NewBlockT
        //   |     \
        //   |   NewBlockF
        //   |      /
        //   |     /
        // EndBlock
        //  (Use)
        let false_to_end =
            function.create_instruction(Opcode::Br, void, vec![Operand::Block(end_block)], Flags::default(), None);
        function.insert(false_to_end, Position::End(new_false)).expect("a placed block");
        // Insert the real conditional branch based on the original condition.
        let branch = function.create_instruction(
            Opcode::Br,
            void,
            vec![condition, Operand::Block(end_block), Operand::Block(new_false)],
            Flags::default(),
            None,
        );
        function.insert(branch, Position::End(new_true)).expect("a placed block");

        let phi_true = function.create_instruction(
            Opcode::Phi,
            ty,
            vec![true_value, Operand::Block(start_block)],
            Flags::default(),
            Some(&format!("{}.si.unfold.phi", name_of(function, true_value))),
        );
        function.insert(phi_true, Position::Before(branch)).expect("a placed phi");
        let phi_false = function.create_instruction(
            Opcode::Phi,
            ty,
            vec![false_value, Operand::Block(new_true)],
            Flags::default(),
            Some(&format!("{}.si.unfold.phi", name_of(function, false_value))),
        );
        function.insert(phi_false, Position::Before(false_to_end)).expect("a placed phi");

        if let Some(op) = select_of(function, true_value) {
            found.push(SelectToUnfold { select: op, user: phi_true });
        }
        if let Some(op) = select_of(function, false_value) {
            found.push(SelectToUnfold { select: op, user: phi_false });
        }

        let value_of =
            |function: &Function, phi: InstId| Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        let (true_phi, false_phi) = (value_of(function, phi_true), value_of(function, phi_false));
        add_incoming(function, user, true_phi, new_true);
        add_incoming(function, user, false_phi, new_false);
        remove_incoming(function, user, start_block);

        // Update any other PHI nodes in EndBlock.
        for phi in phis(function, end_block) {
            if phi == user {
                continue;
            }
            let value = incoming_value_for_block(function, phi, start_block).expect("an input from the start");
            add_incoming(function, phi, value, new_true);
            add_incoming(function, phi, value, new_false);
            remove_incoming(function, phi, start_block);
        }

        // Update the appropriate successor of the start block to point to the
        // new unfolded block.
        let mut operands = function.instruction(start_term).operands.clone();
        let at = if operands[2] == Operand::Block(end_block) { 2 } else { 1 };
        operands[at] = Operand::Block(new_true);
        function.set_operands(start_term, operands);
    }

    // The select is now dead.
    function.erase(select).expect("a select nothing uses");
    Unfolded { found, start_block, new_blocks }
}
