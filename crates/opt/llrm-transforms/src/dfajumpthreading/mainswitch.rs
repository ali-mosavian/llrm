//! `MainSwitch`: which `switch` is worth looking for threading paths around.

use std::collections::VecDeque;

use llrm_mir::hash::HashSet;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;

use super::{DfaJumpThreadingOptions, phi_of};

/// `SelectInstToUnfold`
/// ([L142](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L142)):
/// a `select` of the state's chain and the `phi` that is its only use.
#[derive(Clone, Copy)]
pub(super) struct SelectToUnfold {
    pub(super) select: InstId,
    pub(super) user: InstId,
}

/// A `switch` on a state that is a `phi` in a loop and is made of constants,
/// `select`s and values nothing predicts.
pub(super) struct MainSwitch {
    pub(super) switch: InstId,
    /// The `select`s of the chain to unfold into control flow, in the order the
    /// walk found them.
    pub(super) selects: Vec<SelectToUnfold>,
}

impl MainSwitch {
    /// `MainSwitch::MainSwitch`
    /// ([L454](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L454)):
    /// the switch, if it is a candidate.
    pub(super) fn of(
        function: &Function,
        loops: &LoopInfo,
        options: &DfaJumpThreadingOptions,
        switch: InstId,
    ) -> Option<Self> {
        let mut selects = Vec::new();
        is_candidate(function, loops, options, switch, &mut selects).then_some(Self { switch, selects })
    }
}

/// What a value of the state's use-def chain is.
enum Kind {
    Phi(InstId),
    Select(InstId),
    Constant,
    /// Anything else: an unpredictable value.
    Other,
}

fn kind_of(
    function: &Function,
    value: Operand,
) -> Kind {
    match value {
        Operand::Constant(_) => Kind::Constant,
        Operand::Block(_) => Kind::Other,
        Operand::Value(id) => match function.value(id).def {
            ValueDef::Instruction(inst) => match function.instruction(inst).opcode {
                Opcode::Phi => Kind::Phi(inst),
                Opcode::Select => Kind::Select(inst),
                _ => Kind::Other,
            },
            ValueDef::Argument(_) => Kind::Other,
        },
    }
}

/// `MainSwitch::isCandidate`
/// ([L478](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L478)):
/// a use-def walk from the switch condition to see if the switch is a
/// candidate; also collects the `select`s to unfold.
fn is_candidate(
    function: &Function,
    loops: &LoopInfo,
    options: &DfaJumpThreadingOptions,
    switch: InstId,
    selects: &mut Vec<SelectToUnfold>,
) -> bool {
    let condition = function.instruction(switch).operands[0];
    if phi_of(function, condition).is_none() {
        return false;
    }
    // The switch must be in a loop.
    let switch_block = function.parent(switch).expect("a placed switch");
    let Some(lp) = loops.get_loop_for(switch_block) else { return false };

    let mut queue: VecDeque<(Operand, Option<BlockId>)> = VecDeque::new();
    let mut seen: HashSet<Operand> = HashSet::default();
    let mut add_to_queue =
        |queue: &mut VecDeque<(Operand, Option<BlockId>)>, value: Operand, block: Option<BlockId>| {
            if seen.insert(value) {
                queue.push_back((value, block));
            }
        };
    add_to_queue(&mut queue, condition, None);

    while let Some((current, incoming_block)) = queue.pop_front() {
        match kind_of(function, current) {
            Kind::Phi(phi) => {
                for (value, block) in super::incoming_per_block(function, phi).into_iter().map(|(b, v)| (v, b)) {
                    add_to_queue(&mut queue, value, Some(block));
                }
            }
            Kind::Select(select) => {
                if !is_valid_select(function, select, selects) {
                    return false;
                }
                let operands = &function.instruction(select).operands;
                add_to_queue(&mut queue, operands[1], incoming_block);
                add_to_queue(&mut queue, operands[2], incoming_block);
                let result = function.instruction(select).result.expect("a select's value");
                if let Some(user) = function.users(result).only().map(|one| one.user)
                    && function.instruction(user).opcode == Opcode::Phi
                {
                    selects.push(SelectToUnfold { select, user });
                }
            }
            Kind::Constant => {}
            Kind::Other => {
                // Allow unpredictable values. The hope is that those will be
                // the initial switch values that can be ignored (they will hit
                // the unthreaded switch) but this assumption will get checked
                // later after paths have been enumerated. If the unpredictable
                // value comes from the same inner loop it is likely that it
                // will also be on the enumerated paths, causing us to exit
                // after we have enumerated all the paths.
                let from = incoming_block.and_then(|block| loops.get_loop_for(block));
                if options.early_exit_heuristic && lp.contains_loop(from) {
                    return false;
                }
            }
        }
    }
    true
}

/// `MainSwitch::isValidSelectInst`
/// ([L549](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L549)):
/// whether `select` can be unfolded into control flow, given the ones already
/// collected.
fn is_valid_select(
    function: &Function,
    select: InstId,
    collected: &[SelectToUnfold],
) -> bool {
    let result = function.instruction(select).result.expect("a select's value");
    let Some(only) = function.users(result).only() else { return false };

    // The use of the select inst should be either a phi or another select.
    let user = function.instruction(only.user);
    if !matches!(user.opcode, Opcode::Phi | Opcode::Select) {
        return false;
    }

    let select_block = function.parent(select).expect("a placed select");

    // Currently, we can only expand select instructions in basic blocks with
    // one successor.
    let Some(term) = function.terminator(select_block) else { return false };
    let term = function.instruction(term);
    if term.opcode != Opcode::Br || term.operands.len() != 1 {
        return false;
    }

    // Only fold the select coming from directly where it is defined.
    if user.opcode == Opcode::Phi && user.operands[only.index as usize + 1] != Operand::Block(select_block) {
        return false;
    }

    // If select will not be sunk during unfolding, and it is in the same basic
    // block as another state defining select, then cannot unfold both.
    let this = Operand::Value(result);
    collected
        .iter()
        .all(
            |previous| {
                let operands = &function.instruction(previous.select).operands;
                operands[1] == this || operands[2] == this || function.parent(previous.select) != Some(select_block)
            },
        )
}
