//! A function's frame rows from the `.cfi_*` directives in its blocks: what
//! the canonical frame address and the saved registers are at each place in
//! the code. A block's state at its start is the one its predecessors end in
//! (the prologue and the epilogues of a shrink-wrapped function put blocks in
//! and out of the frame), so the states are followed along the branches, and a
//! row is written wherever the code changes the state or enters a block in
//! another one.

use std::collections::HashMap;

use llrm_object::assembler::Cfi;
use llrm_object::debug::FrameRow;

use crate::frame::Lowered;
use crate::layout::label_of;
use crate::mc::inst::Inst;
use crate::mc::unwind::{SP, register_label};

type State = (String, i64, Vec<(String, i64)>);

fn apply(
    state: &mut State,
    cfi: Cfi,
) {
    match cfi {
        Cfi::DefCfa { register, offset } => (state.0, state.1) = (register_label(register), offset),
        Cfi::DefCfaRegister { register } => state.0 = register_label(register),
        Cfi::DefCfaOffset { offset } => state.1 = offset,
        Cfi::Offset { register, offset } => state.2.push((register_label(register), offset)),
        Cfi::Restore { register } => {
            let name = register_label(register);
            state.2.retain(|(one, _)| *one != name);
        }
    }
}

/// Whether control goes on to the next block after `block`.
fn falls_through(block: &Lowered) -> bool {
    !matches!(
        block.insts.iter().rev().find(|inst| !matches!(inst, Inst::Cfi(_))),
        Some(Inst::Ret { .. } | Inst::Branch { link: false, .. } | Inst::BranchReg { link: false, .. })
    )
}

/// The rows of the function made of `blocks`, as byte offsets from its start;
/// empty where it declares no frame.
pub fn rows(blocks: &[Lowered]) -> Vec<FrameRow> {
    if !blocks.iter().flat_map(|block| &block.insts).any(|inst| matches!(inst, Inst::Cfi(_))) {
        return Vec::new();
    }
    let entry: State = (register_label(SP), 0, Vec::new());
    let index: HashMap<&str, usize> = blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
    let mut starts: Vec<Option<State>> = vec![None; blocks.len()];
    starts[0] = Some(entry.clone());
    let mut work = vec![0];
    while let Some(at) = work.pop() {
        let mut state = starts[at].clone().expect("reached");
        for inst in &blocks[at].insts {
            if let Inst::Cfi(cfi) = inst {
                apply(&mut state, *cfi);
            }
        }
        let leaves = blocks[at].insts.iter().filter_map(label_of).filter_map(|label| index.get(label).copied());
        let next = (falls_through(&blocks[at]) && at + 1 < blocks.len()).then_some(at + 1);
        for to in leaves.chain(next) {
            if starts[to].is_none() {
                starts[to] = Some(state.clone());
                work.push(to);
            }
        }
    }
    let mut out: Vec<FrameRow> = Vec::new();
    let mut offset = 0;
    let mut state = entry.clone();
    let row = |offset: usize, state: &State, out: &mut Vec<FrameRow>| {
        if out.last().is_some_and(|last| last.offset == offset) {
            out.pop();
        }
        out.push(FrameRow { offset, cfa_register: state.0.clone(), cfa_offset: state.1, saved: state.2.clone() });
    };
    for (at, block) in blocks.iter().enumerate() {
        if let Some(start) = &starts[at] {
            if *start != state {
                state = start.clone();
                row(offset, &state, &mut out);
            }
        }
        for inst in &block.insts {
            match inst {
                Inst::Cfi(cfi) => {
                    apply(&mut state, *cfi);
                    row(offset, &state, &mut out);
                }
                _ => offset += 4,
            }
        }
    }
    // A row is a change: one that repeats the one before it, or lies at the
    // function's end, says nothing.
    let mut kept: Vec<FrameRow> = Vec::new();
    for one in out {
        let before = kept
            .last()
            .map_or((&entry.0, entry.1, &entry.2), |last| (&last.cfa_register, last.cfa_offset, &last.saved));
        if (&one.cfa_register, one.cfa_offset, &one.saved) != before && one.offset < offset {
            kept.push(one);
        }
    }
    kept
}
