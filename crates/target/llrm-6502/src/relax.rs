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

use std::collections::HashMap;

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
    loop {
        let mut offset = Vec::with_capacity(lines.len());
        let mut at = 0;
        for (line, &long) in lines.iter().zip(&long) {
            offset.push(at);
            at += bytes(line, long);
        }
        let mut moved = false;
        for (index, line) in lines.iter().enumerate() {
            let Some(inst) = branch(line).filter(|_| !long[index]) else { continue };
            let to = offset[labels[inst.target().unwrap()]] as i64;
            let reach = to - (offset[index] as i64 + 2);
            if !(-128..=127).contains(&reach) {
                long[index] = true;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let mut far = 0;
    for (line, long) in lines.into_iter().zip(long) {
        match line {
            Line::Inst(Inst { op, operand: Operand::Direct(target) }) if long && op.is_branch() => {
                if op == Op::Bra {
                    asm.lines.push(Line::Inst(Inst::new(Op::Jmp, Operand::Direct(target))));
                } else {
                    far += 1;
                    let skip = format!("_far{far}");
                    asm.lines.push(Line::Inst(Inst::new(op.opposite().unwrap(), Operand::Direct(skip.clone()))));
                    asm.lines.push(Line::Inst(Inst::new(Op::Jmp, Operand::Direct(target))));
                    asm.lines.push(Line::Label(skip));
                }
            }
            other => asm.lines.push(other),
        }
    }
}
