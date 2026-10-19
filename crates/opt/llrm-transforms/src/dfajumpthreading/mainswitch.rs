//! `MainSwitch`: which `switch` is worth looking for threading paths around.

use std::collections::VecDeque;

use llrm_mir::hash::HashSet;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;

use super::{DfaJumpThreadingOptions, phi_of};

/// A `switch` on a state that is a `phi` in a loop and is made of constants
/// and values nothing predicts.
pub(super) struct MainSwitch {
    pub(super) switch: InstId,
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
        is_candidate(function, loops, options, switch).then_some(Self { switch })
    }
}

/// What a value of the state's use-def chain is.
enum Kind {
    Phi(InstId),
    /// A `select`: `unfold` is stage S2 (DF1).
    Select,
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
                Opcode::Select => Kind::Select,
                _ => Kind::Other,
            },
            ValueDef::Argument(_) => Kind::Other,
        },
    }
}

/// `MainSwitch::isCandidate`
/// ([L478](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L478)):
/// a use-def walk from the switch condition to see if the switch is a
/// candidate. The `select`s it would collect to unfold reject the switch (DF1).
fn is_candidate(
    function: &Function,
    loops: &LoopInfo,
    options: &DfaJumpThreadingOptions,
    switch: InstId,
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
            Kind::Select => return false,
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
