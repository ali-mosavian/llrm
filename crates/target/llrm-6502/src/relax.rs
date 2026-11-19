//! Branches that miss their target, made long, by the bytes between them.
//!
//! # How it works
//! A conditional branch reaches 127 bytes forward and 128 back. Each line has
//! a size in bytes ([`crate::asm::Inst::bytes`], an upper bound where the
//! assembler picks the form). Every branch starts short; one whose target lies
//! out of reach becomes the opposite branch over a `jmp`, which is three bytes
//! longer, and the pass runs again until no branch moves, because a longer
//! branch can push another out of reach. A `bra` becomes a `jmp`. The sizes
//! only grow, so the passes end. A line that is not a label or an instruction
//! counts as three bytes.
//!
//! DEVIATE: a pass over the expanded instructions of one function, before the
//! shared branch relaxation can run on them. Exit: D-6502 of #6259.
//!
//! # Example
//! ```text
//!   bne l           beq _far1
//!   <130 bytes>  =>  jmp l
//! l:                _far1:
//!                   <130 bytes>
//!                 l:
//! ```

use llrm_support::hash::HashMap;

use crate::asm::{Asm, Inst, Line, Op, Operand};

/// The bytes of a line, with a branch counted as it now stands.
fn bytes(
    line: &Line,
    long: bool,
) -> usize {
    match line {
        Line::Label(_) => 0,
        Line::Raw(_) => 3,
        Line::Inst(inst) if long => {
            if inst.op == Op::Bra {
                3
            } else {
                5
            }
        }
        Line::Inst(inst) => inst.bytes(),
    }
}

/// The branch a line is, with the label it goes to.
fn branch(line: &Line) -> Option<&Inst> {
    line.inst().filter(|inst| inst.op.is_branch() && inst.target().is_some())
}

/// `asm` with every branch out of reach made long.
pub fn relax(asm: &mut Asm) {
    let lines = std::mem::take(&mut asm.lines);
    let labels: HashMap<&str, usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(at, line)| match line {
            Line::Label(label) => Some((label.as_str(), at)),
            _ => None,
        })
        .collect();
    // a branch to a label not in this function reaches only as a jump
    let mut long: Vec<bool> = lines
        .iter()
        .map(|line| branch(line).is_some_and(|inst| !labels.contains_key(inst.target().unwrap())))
        .collect();
    // a conditional branch out of reach of its target may reach a `jmp` to
    // it: 2 bytes where the long form is 5
    let mut via: Vec<Option<usize>> = vec![None; lines.len()];
    let mut tried = vec![false; lines.len()];
    loop {
        let mut offset = Vec::with_capacity(lines.len());
        let mut at = 0;
        for (line, &long) in lines.iter().zip(&long) {
            offset.push(at);
            at += bytes(line, long);
        }
        // a long branch is `opposite; jmp`: its `jmp` is two bytes in
        let pending: Vec<std::cell::Cell<bool>> = long.iter().map(|&l| std::cell::Cell::new(l)).collect();
        let place = |at: usize| offset[at] + if pending[at].get() && branch(&lines[at]).is_some() { 2 } else { 0 };
        let reaches = |from: usize, to: usize| (-128..=127).contains(&(place(to) as i64 - (offset[from] as i64 + 2)));
        let mut moved = false;
        let mut decided: Vec<(usize, Option<usize>)> = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let Some(inst) = branch(line).filter(|_| !long[index]) else { continue };
            let target = inst.target().unwrap();
            if via[index].map_or_else(|| reaches(index, labels[target]), |jump| reaches(index, jump)) {
                continue;
            }
            moved = true;
            let jump = (inst.op != Op::Bra)
                .then(|| {
                    (0..lines.len())
                        .filter(|&j| {
                            let jumps_there = match &lines[j] {
                                Line::Inst(Inst { op: Op::Jmp, operand: Operand::Direct(to), .. }) => to == target,
                                other => {
                                    pending[j].get()
                                        && branch(other).is_some_and(|b| b.op != Op::Bra && b.target() == Some(target))
                                }
                            };
                            j != index && jumps_there && reaches(index, j)
                        })
                        .min_by_key(|&j| place(j).abs_diff(offset[index]))
                })
                .flatten()
                .filter(|_| !tried[index]);
            pending[index].set(jump.is_none());
            decided.push((index, jump));
        }
        for (index, jump) in decided {
            tried[index] = true;
            via[index] = jump;
            long[index] = jump.is_none();
        }
        if !moved {
            break;
        }
    }
    let mut far = 0;
    let jumps_used: Vec<usize> = via.iter().flatten().copied().collect();
    for (index, (line, long)) in lines.into_iter().zip(long).enumerate() {
        let is_jump = matches!(line, Line::Inst(Inst { op: Op::Jmp, .. }));
        if jumps_used.contains(&index) && is_jump {
            asm.lines.push(Line::Label(format!("_far{index}t")));
        }
        match line {
            Line::Inst(Inst { op, operand: Operand::Direct(_), .. }) if via[index].is_some() && op.is_branch() => {
                let to = format!("_far{}t", via[index].unwrap());
                asm.lines.push(Line::Inst(Inst::new(op, Operand::Direct(to))));
            }
            Line::Inst(Inst { op, operand: Operand::Direct(target), .. }) if long && op.is_branch() => {
                if op == Op::Bra {
                    asm.lines.push(Line::Inst(Inst::new(Op::Jmp, Operand::Direct(target))));
                } else {
                    far += 1;
                    let skip = format!("_far{far}");
                    asm.lines.push(Line::Inst(Inst::new(op.opposite().unwrap(), Operand::Direct(skip.clone()))));
                    if jumps_used.contains(&index) {
                        asm.lines.push(Line::Label(format!("_far{index}t")));
                    }
                    asm.lines.push(Line::Inst(Inst::new(Op::Jmp, Operand::Direct(target))));
                    asm.lines.push(Line::Label(skip));
                }
            }
            other => asm.lines.push(other),
        }
    }
}
