//! A loop's peeled first test merged with the copy at its latch.
//!
//! # How it works
//! Loop rotation leaves the loop's test twice: once before the loop (`S;
//! beq exit`, falling into the loop's head `H`) and once at the latch (`S; bne
//! H`, falling or jumping to `exit`). Both blocks have the same two successors
//! and end in the same instructions, so the copy in front goes and the entry
//! jumps into the latch's `S`: the branch of one block is the opposite of the
//! other's, which the merge reads off the opcode, as LLVM's `analyzeBranch` and
//! `reverseBranchCondition` do.
//!
//! The tail is the longest run of straight instructions both blocks end in; a
//! merge is kept when the tail and its branch are more bytes than the `jmp`
//! that replaces them.
//!
//! Translation: `BranchFolder::TailMergeBlocks` (`BranchFolding.cpp`).
//! DEVIATE 6502-TAIL-MERGE (owner 6502, exit 2026-11-24): it runs on the
//! expanded instructions of one function, where LLVM merges machine blocks; the
//! merged tail stays where the latch had it and the other block jumps into it.
//! After the layout choice, so its bytes do not tip it (`alloc::emit_on`).
//! Exit: D-6502 of #6259 puts the 6502 on the shared pipeline, whose
//! `TailMergeBlocks` the loop tests then go through.
//!
//! # Example
//! ```text
//!   ldy #0                 ldy #0
//!   lda (rc4),y            jmp t
//!   sta (rc2),y       =>  h:
//!   beq x                    <step>
//! h:                      t:
//!   <step>                  lda (rc4),y
//!   lda (rc4),y             sta (rc2),y
//!   sta (rc2),y             bne h
//!   bne h                 x:
//! x:
//! ```
//!
//! # Runs
//! [`crate::peephole::after_layout`], before [`crate::short_jump`].

use llrm_support::hash::HashMap;

use crate::asm::{Asm, Inst, Line, Op, Operand};

/// Whether the instruction can sit in a merged tail: it only works on the
/// registers, the flags and memory, and leaves the control where it was.
fn straight(op: Op) -> bool {
    use Op::*;
    matches!(
        op,
        Adc | And
            | Asl
            | Bit
            | Clc
            | Cmp
            | Cpx
            | Cpy
            | Dec
            | Dex
            | Dey
            | Eor
            | Inc
            | Inx
            | Iny
            | Lda
            | Ldx
            | Ldy
            | Lsr
            | Nop
            | Ora
            | Rol
            | Ror
            | Sbc
            | Sec
            | Sta
            | Stx
            | Sty
            | Stz
            | Tax
            | Tay
            | Txa
            | Tya
    )
}

/// How many instructions before `lines[a]` and before `lines[b]` are the same
/// straight instructions, with no label between them.
fn common_tail(
    lines: &[Line],
    a: usize,
    b: usize,
) -> usize {
    let mut length = 0;
    while length < a.min(b) {
        match (&lines[a - 1 - length], &lines[b - 1 - length]) {
            (Line::Inst(one), Line::Inst(other)) if one == other && straight(one.op) => length += 1,
            _ => break,
        }
    }
    length
}

/// Whether control leaving `lines[from - 1]` by falling through reaches
/// `label`: it is one of the labels that follow, or a `jmp` to it does.
fn falls_to(
    lines: &[Line],
    from: usize,
    label: &str,
) -> bool {
    let mut at = from;
    while let Some(Line::Label(name)) = lines.get(at) {
        if name == label {
            return true;
        }
        at += 1;
    }
    matches!(
        lines.get(at),
        Some(Line::Inst(jump)) if jump.op == Op::Jmp && jump.target() == Some(label)
    )
}

/// A label name no line has: the emitter's `_tm` kind, numbered from 1 and
/// renamed with the function's name (`alloc::own_labels`).
fn fresh_label(lines: &[Line]) -> String {
    let used = lines
        .iter()
        .filter_map(|line| match line {
            Line::Label(name) => name.strip_prefix("_tm")?.parse::<usize>().ok(),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    format!("_tm{}", used + 1)
}

/// One merge, if there is one: the entry's tail and branch become a `jmp` into
/// the latch's tail.
fn merge_once(asm: &mut Asm) -> bool {
    let lines = &asm.lines;
    let branches: Vec<usize> = (0..lines.len())
        .filter(|&at| {
            matches!(
                &lines[at],
                Line::Inst(inst) if inst.op.opposite().is_some() && inst.target().is_some()
            )
        })
        .collect();
    // the branches that go to each label: a latch is found by the loop's head
    let mut going_to: HashMap<&str, Vec<usize>> = HashMap::default();
    for &at in &branches {
        if let Line::Inst(inst) = &lines[at] {
            going_to.entry(inst.target().unwrap_or_default()).or_default().push(at);
        }
    }
    for &entry in &branches {
        let Line::Inst(first) = &lines[entry] else { continue };
        let (Some(opposite), Some(exit)) = (first.op.opposite(), first.target()) else { continue };
        let mut heads = Vec::new();
        while let Some(Line::Label(name)) = lines.get(entry + 1 + heads.len()) {
            heads.push(name.as_str());
        }
        for &latch in heads.iter().filter_map(|head| going_to.get(head)).flatten() {
            let Line::Inst(back) = &lines[latch] else { continue };
            if latch == entry || back.op != opposite {
                continue;
            }
            if !falls_to(lines, latch + 1, exit) {
                continue;
            }
            let length = common_tail(lines, entry, latch);
            let tail_bytes: usize = lines[entry - length..entry].iter().filter_map(Line::inst).map(Inst::bytes).sum();
            // the tail and its branch against the `jmp` that replaces them
            if length == 0 || tail_bytes + first.bytes() <= 3 {
                continue;
            }
            let start = latch - length;
            if start == 0 {
                continue;
            }
            let (entry_from, entry_to) = (entry - length, entry);
            let into = match &lines[start - 1] {
                Line::Label(name) => name.clone(),
                _ => fresh_label(lines),
            };
            let named = matches!(&lines[start - 1], Line::Label(_));
            let jump = Line::Inst(Inst::new(Op::Jmp, Operand::Direct(into.clone())));
            // the later edit first: the earlier one's positions hold
            if start > entry_to {
                if !named {
                    asm.lines.insert(start, Line::Label(into));
                }
                asm.lines.splice(entry_from..=entry_to, [jump]);
            } else {
                asm.lines.splice(entry_from..=entry_to, [jump]);
                if !named {
                    asm.lines.insert(start, Line::Label(into));
                }
            }
            return true;
        }
    }
    false
}

/// Each loop test that its latch repeats, jumped into at the latch.
pub fn merge_opposite_tails(asm: &mut Asm) -> bool {
    let mut merged = false;
    while merge_once(asm) {
        merged = true;
    }
    merged
}
