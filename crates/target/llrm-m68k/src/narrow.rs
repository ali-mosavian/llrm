//! `eori.l #4129, %d0` is `eori.w #4129, %d0` when the immediate's upper half
//! changes nothing: the word form leaves the upper half of the register as it
//! was, and `x ^ 0`, `x | 0` and `x & -1` leave it too. 4 bytes for 6 and 8
//! clocks for 16 on the 68000.
//!
//! # How it works
//! [`narrow_logic_immediates`] runs after allocation, where the instruction
//! reads and writes the same register. An `and`, `or` or `eor` of a long
//! immediate whose upper half is all ones (`and`) or zero (`or`, `eor`)
//! becomes the word row of the same name, where no reader of the codes after
//! it exists ([`crate::cmpelim::flags_read_after`]): the word form sets N and
//! Z from the low half only.
//!
//! [`merge_logic_immediates`] joins two such rows in a row on one register into
//! one with the combined immediate: `and.l #255` then `and.l #1` is `and.l
//! #1`, the zero-extension of a compare's byte and the mask of its `i1`.
//!
//! DEVIATE: the pin has no such narrowing; its DAG keeps the 32-bit constant
//! and its `targetShrinkDemandedConstant` is only a hook. gcc's `iorsi3`/
//! `xorsi3` output templates print the `.w` form. Reason: best of gcc and clang
//! is the goal. Exit: none. Off for `Options::PINNED`.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::REGISTER_INFO;
use crate::cmpelim::flags_read_after;
use crate::dagisel::local;
use crate::mfunc::{MFunction, op_inst};
use crate::regalloc::Assignment;

/// The word row of a long-immediate logic row, and whether its immediate is
/// narrowed by an upper half of ones (`and`) or zeros.
fn word_for(name: &str) -> Option<(&'static str, bool)> {
    Some(match name {
        "AND32di" => ("AND16di", true),
        "OR32di" => ("OR16di", false),
        "XOR32di" => ("XOR16di", false),
        _ => return None,
    })
}

pub fn narrow_logic_immediates(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: &Reg| match reg {
        Reg::Virt(v) => REGISTER_INFO.root(assignment.0[*v as usize]),
        Reg::Phys(p) => REGISTER_INFO.root(*p),
    };
    for block in &mut function.blocks {
        for at in 0..block.insts.len() {
            let Kind::Op(name) = block.insts[at].kind else { continue };
            let Some((target, ones)) = word_for(name) else { continue };
            let [Operand::Def(def), Operand::Use(base), Operand::Imm(value)] = block.insts[at].operands[..] else {
                continue;
            };
            let upper = (value as u32) >> 16;
            if upper != if ones { 0xFFFF } else { 0 } || register(&def) != register(&base) {
                continue;
            }
            if flags_read_after(&block.insts, at) != Some(0) {
                continue;
            }
            let Some(desc) = local::code_of(target).and_then(local::desc) else { continue };
            let word = i64::from(value as u32 & 0xFFFF);
            block.insts[at] = op_inst(target, desc, vec![Operand::Def(def), Operand::Use(base), Operand::Imm(word)]);
        }
    }
}

/// A logic row with an immediate: its operation (`&`, `|`, `^` as 0, 1, 2) and
/// width in bits.
fn logic_row(name: &str) -> Option<(u8, u32)> {
    Some(match name {
        "AND8di" => (0, 8),
        "AND16di" => (0, 16),
        "AND32di" => (0, 32),
        "OR8di" => (1, 8),
        "OR16di" => (1, 16),
        "OR32di" => (1, 32),
        "XOR8di" => (2, 8),
        "XOR16di" => (2, 16),
        "XOR32di" => (2, 32),
        _ => return None,
    })
}

fn logic_name(
    operation: u8,
    bits: u32,
) -> &'static str {
    match (operation, bits) {
        (0, 8) => "AND8di",
        (0, 16) => "AND16di",
        (0, _) => "AND32di",
        (1, 8) => "OR8di",
        (1, 16) => "OR16di",
        (1, _) => "OR32di",
        (_, 8) => "XOR8di",
        (_, 16) => "XOR16di",
        _ => "XOR32di",
    }
}

/// `and`, `or` and `eor` of immediates on one register, in a row (the rows
/// between them neither touch the register nor read or set the codes, and are
/// no call), are one row with the immediates combined: `a & b`, `a | b`,
/// `a ^ b`. A narrower row stands for the wider one with the bits it leaves
/// alone (ones for `and`, zeros for `or` and `eor`), so `and.l #255` then
/// `and.w #1` is `and.l #1`, and the merged row is the wider of the two.
/// The first row's codes are overwritten by the second's. The second's codes
/// come from its own width: when the merged row is wider than the second, it
/// merges only if no reader of the codes follows.
///
/// DEVIATE: the pin keeps both (`MOVZXd32d8` then `AND32di 1`, countgt); its
/// combiner does not join them. Reason: best of gcc and clang is the goal.
/// Exit: 2026-12-04, with convergence step 5 (the shared flow's peephole
/// owns it). Off for `Options::PINNED`.
pub fn merge_logic_immediates(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: &Reg| match reg {
        Reg::Virt(v) => REGISTER_INFO.root(assignment.0[*v as usize]),
        Reg::Phys(p) => REGISTER_INFO.root(*p),
    };
    // (operation, width, register, immediate, sets the codes)
    let in_place = |inst: &llrm_spike_mf::Inst| -> Option<(u8, u32, llrm_lir::registers::RegId, u32, bool)> {
        let Kind::Op(name) = inst.kind else { return None };
        if let Some((operation, bits)) = logic_row(name) {
            return match inst.operands[..] {
                [Operand::Def(def), Operand::Use(base), Operand::Imm(value)] if register(&def) == register(&base) => {
                    Some((operation, bits, register(&def), value as u32, true))
                }
                _ => None,
            };
        }
        // a zero extension of a register into itself is `and` of the source's
        // mask (`ExpandMOVSZX_RR`), which leaves the codes alone
        let (false, to, 'd', from) = crate::expand::extension(name)? else { return None };
        match inst.operands[..] {
            [Operand::Def(def), Operand::Use(source)] if register(&def) == register(&source) => {
                Some((0, to, register(&def), if from == 8 { 0xFF } else { 0xFFFF }, false))
            }
            _ => None,
        }
    };
    let mentions = |inst: &llrm_spike_mf::Inst, root| {
        inst.operands
            .iter()
            .any(
                |operand| matches!(
                    operand,
                    Operand::Def(r) | Operand::Use(r) if register(r) == root
                ),
            )
    };
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let Some((operation, bits_a, root, a, _)) = in_place(&block.insts[at]) else {
                at += 1;
                continue;
            };
            // the next row of the same operation on the register, past rows
            // that leave both alone
            let mut next = None;
            for later in at + 1..block.insts.len() {
                let inst = &block.insts[later];
                if let Some((op_b, bits_b, root_b, b, sets_b)) = in_place(inst)
                    && root_b == root
                    && op_b == operation
                {
                    next = Some((later, bits_b, b, sets_b));
                    break;
                }
                // A row that only sets the codes is passed: the second row sets them again
                // (and `flags_read_after` guards a second that does not).
                if mentions(inst, root) || inst.reads_flags() || inst.call || inst.terminator {
                    break;
                }
            }
            let Some((later, bits_b, b, sets_b)) = next else {
                at += 1;
                continue;
            };
            let width = bits_a.max(bits_b);
            if (width != bits_b || !sets_b) && flags_read_after(&block.insts, later) != Some(0) {
                at += 1;
                continue;
            }
            let all = |bits: u32| if bits == 32 { u32::MAX } else { (1u32 << bits) - 1 };
            // the bits a narrower row leaves alone: ones for `and`, zeros
            // otherwise
            let widen = |value: u32, bits: u32| {
                let kept = if operation == 0 { !all(bits) } else { 0 };
                (value & all(bits)) | kept
            };
            let (x, y) = (widen(a, bits_a), widen(b, bits_b));
            let combined = match operation {
                0 => x & y,
                1 => x | y,
                _ => x ^ y,
            } & all(width);
            let name = logic_name(operation, width);
            let Some(desc) = local::code_of(name).and_then(local::desc) else {
                at += 1;
                continue;
            };
            let (Operand::Def(def) | Operand::Use(def)) = block.insts[later].operands[0] else {
                at += 1;
                continue;
            };
            let value = if width == 32 { i64::from(combined as i32) } else { i64::from(combined) };
            block.insts[later] = op_inst(name, desc, vec![Operand::Def(def), Operand::Use(def), Operand::Imm(value)]);
            block.insts.remove(at);
        }
    }
}
