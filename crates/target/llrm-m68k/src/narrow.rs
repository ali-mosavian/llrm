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
