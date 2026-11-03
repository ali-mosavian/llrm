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
//! [`drop_covered_masks`] drops an `and` of a long immediate whose register
//! the next instruction to touch it ands again with bits the first keeps all
//! of: `and.l #255, %d0; and.l #1, %d0` is the second alone. The pin leaves
//! both (its `MOVZX` expansion is an `and`, the `i1` mask another).
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

/// `(def, use, bits kept)` of a register-to-register `and` of a long immediate
/// or zero extension to 32 bits: the instruction clears every bit outside the
/// mask and keeps those inside.
fn masks(inst: &llrm_spike_mf::Inst) -> Option<(Reg, Reg, u32)> {
    let Kind::Op(name) = inst.kind else { return None };
    match (name, &inst.operands[..]) {
        ("AND32di", [Operand::Def(def), Operand::Use(base), Operand::Imm(mask)]) => Some((*def, *base, *mask as u32)),
        (_, [Operand::Def(def), Operand::Use(source)]) => match crate::expand::extension(name)? {
            (false, 32, 'd', 8) => Some((*def, *source, 0xFF)),
            (false, 32, 'd', 16) => Some((*def, *source, 0xFFFF)),
            _ => None,
        },
        _ => None,
    }
}

/// The root registers an instruction names.
fn touched(
    inst: &llrm_spike_mf::Inst,
    register: &impl Fn(&Reg) -> Reg,
) -> Vec<Reg> {
    inst.operands
        .iter()
        .filter_map(|operand| match operand {
            Operand::Def(reg) | Operand::Use(reg) => Some(register(reg)),
            _ => None,
        })
        .collect()
}

/// Deletes each mask of `Rn` (`and.l #K1`, or a zero extension) whose next
/// toucher of `Rn` is `and.l #K2, Rn` with `K2 & !K1 == 0`, when nothing
/// between reads the codes the first sets.
pub fn drop_covered_masks(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: &Reg| match reg {
        Reg::Virt(v) => Reg::Phys(REGISTER_INFO.root(assignment.0[*v as usize])),
        Reg::Phys(p) => Reg::Phys(REGISTER_INFO.root(*p)),
    };
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let covered = (|| {
                let Some((def, base, first)) = masks(&block.insts[at]) else { return false };
                let own = register(&def);
                if own != register(&base) || flags_read_after(&block.insts, at) != Some(0) {
                    return false;
                }
                for after in &block.insts[at + 1..] {
                    if !touched(after, &register).contains(&own) {
                        if after.call || after.terminator || after.barrier || after.side_effects {
                            return false;
                        }
                        continue;
                    }
                    let Some((d, b, second)) = masks(after) else { return false };
                    return register(&d) == own && register(&b) == own && second & !first == 0;
                }
                false
            })();
            if covered {
                block.insts.remove(at);
            } else {
                at += 1;
            }
        }
    }
}
