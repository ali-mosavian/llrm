//! Port of `qbopt/frontend/stack.py`: what a call's arguments are, when they
//! are not all pushed immediately before it.
//!
//! Scoped to one basic block. An ordinary instruction that provably never
//! touches sp is passed over; anything else resets the tracked depth to
//! "unknown" instead of guessing, which can only cost a frame that was
//! findable, never invent one that was not.

use std::sync::LazyLock;

use iced_x86::{Code, Register};

use crate::frontend::blocks::Block;
use crate::frontend::declen::{Insn, WRITES, instruction_info_factory};
use crate::support::hash::IndexMap;

// a candidate argument push, and how many bytes it adds -- not every
// instruction that moves sp by this much is one of these (`push cs` moves it
// by 2 and is never an argument), so this is deliberately not the same table
// as "instructions that touch sp" below
pub static PUSH_BYTES: LazyLock<IndexMap<Code, i64>> = LazyLock::new(|| {
    IndexMap::from_iter([
        (Code::Push_r16, 2),
        (Code::Push_rm16, 2),
        (Code::Pushw_imm8, 2),
        (Code::Push_imm16, 2),
        (Code::Push_rm32, 4),
        (Code::Pushd_imm8, 4),
        (Code::Pushd_imm32, 4),
        // BC never emits this, but calls.py's own restoring() does --
        // re-analysing already-rewritten code should not mistake it for an
        // sp-mover it cannot explain
        (Code::Push_r32, 4),
    ])
});

const _SP: [Register; 2] = [Register::SP, Register::ESP];

/// Whether this instruction could move the stack pointer by any means.
///
/// `stack_pointer_increment` reports 0 for `add sp,imm` and `leave`; only a
/// register-write check over sp/esp catches those too.
#[must_use]
pub fn touches_sp(insn: &Insn) -> bool {
    if insn.insn.stack_pointer_increment() != 0 {
        return true;
    }
    instruction_info_factory()
        .info(&insn.insn)
        .used_registers()
        .iter()
        .any(|used| _SP.contains(&used.register()) && WRITES.contains(&used.access()))
}

/// The pushes one call consumes, in push order -- deepest first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub call: Insn,
    pub pushed: Vec<Insn>,
}

/// Every call in this block whose arguments are named, stack position only.
///
/// `calls` maps a call instruction's own address to the routine it targets.
/// `arity` says how many long arguments a routine by that name takes, or
/// None if it is not one this can reason about at all; an answer under one
/// is treated the same as None.
pub fn frames(block: &Block, calls: &IndexMap<i64, String>, arity: &dyn Fn(&str) -> Option<i64>) -> Vec<Frame> {
    let mut stack: Vec<&Insn> = Vec::new();
    let mut found: Vec<Frame> = Vec::new();

    for insn in &block.insns {
        let name = calls.get(&(insn.at as i64));
        let needed = name.and_then(|name| arity(name));
        if let Some(needed) = needed.filter(|&needed| needed >= 1) {
            let mut have = 0;
            let mut take: Vec<&Insn> = Vec::new();
            for pushed in stack.iter().rev() {
                if have >= needed * 4 {
                    break;
                }
                // every entry in `stack` passed the `in PUSH_BYTES` check below
                // before being appended, so this always finds one
                have += PUSH_BYTES.get(&pushed.code()).copied().unwrap_or(0);
                take.push(pushed);
            }
            if have != needed * 4 {
                stack = Vec::new(); // the arity crosses into bytes this block never explained
                continue;
            }
            take.reverse();
            found.push(Frame { call: insn.clone(), pushed: take.iter().map(|one| (*one).clone()).collect() });
            let keep = stack.len() - take.len();
            stack.truncate(keep);
            continue;
        }
        if PUSH_BYTES.contains_key(&insn.code()) {
            stack.push(insn);
            continue;
        }
        if !touches_sp(insn) {
            continue;
        }
        // a pop, arithmetic on sp, a call this cannot account for -- anything
        // that could move the stack pointer without this knowing by how much
        stack = Vec::new();
    }

    found
}

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
