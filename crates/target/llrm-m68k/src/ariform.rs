//! DEVIATE M68K-ARI-FORM: a memory operand `(0,An)` is printed and encoded as
//! `(An)`: the register-indirect form is two bytes shorter and 4 clocks
//! faster on the 68000 (`move.l (An),Dn` 12, `move.l (d16,An),Dn` 16).
//!
//! Frame-index elimination turns a slot at the stack pointer's own address into
//! `(0,%sp)` (the pin's `M68kRegisterInfo::eliminateFrameIndex` keeps the
//! displacement form); gcc's RTL never builds `(plus An 0)`, so its text is
//! `sp@`. The twin is the instruction the pin's tables give for the same
//! operation with the `ARI` mode in place of `ARID` (`MOV32dp` -> `MOV32dj`),
//! found from the tables: the same asm string, and the same operands with the
//! displacement dropped. An instruction with no such twin keeps `(0,An)`.
//!
//! General solution: one rule for every instruction with an `ARID` operand,
//! not a list of spill opcodes.
//!
//! # Example
//! ```text
//! move.l (0,%sp), %d1      ->  move.l (%sp), %d1       4 B -> 2 B
//! move.l %d1, (0,%a0)      ->  move.l %d1, (%a0)
//! move.l (4,%sp), %d1      ->  unchanged
//! ```
//!
//! # See also
//! gcc `gcc/config/m68k/m68k.cc` `print_operand_address` (the `(An)` text) and
//! `gcc/simplify-rtx.cc` (`(plus x 0)` is `x`).

use crate::dagisel::asm::{AsmOperand, asm};
use crate::mc::{Inst, Operand};

/// `inst` with every `(0,An)` operand in the register-indirect form.
pub fn shorten(mut inst: Inst) -> Inst {
    while let Some(next) = once(&inst) {
        inst = next;
    }
    inst
}

fn once(inst: &Inst) -> Option<Inst> {
    // cheap exit: the lookups below are for the few instructions with a zero
    if !inst.operands.contains(&Operand::Imm(0)) {
        return None;
    }
    let &(_, text, from) = asm(inst.opcode)?;
    let group = from.iter().find(|one| {
        one.method.starts_with("printARID") && inst.operands.get(usize::from(one.at)) == Some(&Operand::Imm(0))
    })?;
    let twin = twin(inst.opcode, text, from, group)?;
    let mut operands = inst.operands.clone();
    operands.remove(usize::from(group.at));
    Some(Inst { opcode: twin, operands })
}

/// The instruction that is `name` with `group` in the `ARI` mode: one of the
/// mode letters of the name (`p`, `ARID`) is `j` (`ARI`) there.
fn twin(
    name: &str,
    text: &str,
    from: &[AsmOperand],
    group: &AsmOperand,
) -> Option<&'static str> {
    let letters = name.trim_end_matches(|c: char| c.is_ascii_lowercase()).len();
    name.char_indices()
        .filter(|&(at, c)| at >= letters && c == 'p')
        .find_map(
            |(at, _)| {
                let mut one = name.to_owned();
                one.replace_range(at..=at, "j");
                let &(twin, twin_text, to) = asm(&one)?;
                (twin_text == text && same_but(from, to, group)).then_some(twin)
            },
        )
}

/// Whether `to` is `from` with `group` an `ARI` of one operand: the other
/// operands keep their method, and those after it move up one.
fn same_but(
    from: &[AsmOperand],
    to: &[AsmOperand],
    group: &AsmOperand,
) -> bool {
    from.len() == to.len()
        && from.iter().all(|a| {
            to.iter()
                .find(|b| b.name == a.name)
                .is_some_and(
                    |b| if a.name == group.name {
                        b.at == a.at && b.method.replace("ARI", "ARID") == a.method
                    } else {
                        b.method == a.method && usize::from(b.at) + usize::from(a.at > group.at) == usize::from(a.at)
                    },
                )
        })
}
