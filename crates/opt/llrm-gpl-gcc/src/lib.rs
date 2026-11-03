//! gcc's size estimate of a function and the edge growth of copying a
//! recursive one, after `estimate_num_insns` with `eni_size_weights`
//! (tree-inline.cc) and `want_inline_small_function_p` (ipa-inline.cc).
//!
//! Licence: GPL-3.0-or-later, as gcc's. This crate is optional (feature
//! `gcc-gpl` of `llrm-transforms`, off by default): a default build carries no
//! gcc-derived code.

use llrm_mir::module::{Function, InstId};
use llrm_mir::opcode::Opcode;

/// gcc's size of `body` (`estimate_num_insns` with `eni_size_weights`): a
/// compare in a branch is part of its condition (2), a load or store is an
/// assignment with a memory operand (2), a call is 1 and one per argument and
/// per result move, a cast and an address folded into its access are free.
pub fn gcc_size(body: &Function) -> i64 {
    let used_by = |inst: InstId, accepts: &dyn Fn(&Function, &llrm_mir::module::Use) -> bool| {
        let Some(result) = body.instruction(inst).result else { return false };
        let uses = body.users(result);
        !uses.is_empty() && uses.iter().all(|one| accepts(body, one))
    };
    body.walk()
        .map(|(_, inst)| {
            let instruction = body.instruction(inst);
            match &instruction.opcode {
                Opcode::Phi | Opcode::Alloca { .. } | Opcode::Freeze | Opcode::Unreachable | Opcode::Cast(_) => 0,
                Opcode::Br => {
                    if instruction.operands.len() == 1 {
                        0
                    } else {
                        2
                    }
                }
                Opcode::Ret => 1,
                Opcode::Load { .. } | Opcode::Store { .. } => 2,
                Opcode::ICmp(_) => i64::from(!used_by(inst, &|body, one| {
                    let user = body.instruction(one.user);
                    user.opcode == Opcode::Br && user.operands.len() == 3 && one.index == 0
                })),
                Opcode::GetElementPtr { .. } => {
                    i64::from(!used_by(inst, &|body, one| match body.instruction(one.user).opcode {
                        Opcode::Load { .. } => one.index == 0,
                        Opcode::Store { .. } => one.index == 1,
                        _ => false,
                    }))
                }
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    let arguments = instruction.operands.len().saturating_sub(1) as i64;
                    1 + arguments + i64::from(instruction.result.is_some_and(|r| !body.users(r).is_empty()))
                }
                _ => 1,
            }
        })
        .sum()
}

/// The size of the call a copy of a recursive body replaces, with the moves of
/// its arguments and result: `gcc_size` less this is gcc's edge growth in its
/// `-fdump-ipa-inline-details` for fib (size 12, growth 6), quicksort `sort`
/// (34, 25-28) and bintree `walk` (23, 15-22); the same for 1 and 3 parameters.
const RECURSIVE_CALL_SIZE: i64 = 6;

/// What a copy of `body` into itself adds, in `gcc_size`.
pub fn recursive_edge_growth(body: &Function) -> i64 {
    gcc_size(body) - RECURSIVE_CALL_SIZE
}
