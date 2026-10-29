//! Keeping the condition codes a branch reads: a `move` into a data register
//! sets them, so a phi's copy between the instruction that set them and the
//! branch that reads them would change what the branch sees (`llc` saves and
//! restores `sr` around such a copy, `copyPhysReg`).
//!
//! Done on the allocated function. A copy that does not depend on the setter
//! moves above it. A copy that does (it copies the setter's result) stays, and
//! the flags are set again after it: a `tst` of the setter's result where the
//! branch reads only what `tst` gives, or the same compare again.
//!
//! DEVIATE: not `sr` save and restore. Reason: `move.w %sr, -(%sp)` is
//! privileged from the 68010 on, and the Mac, Amiga and Atari programs this
//! backend is for run in user mode; and the copies it protects are mostly
//! avoidable. Exit: never; a target choice.

use llrm_lir::registers::RegId;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::mfunc::MFunction;
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, reg};

fn root(
    register: Reg,
    assignment: &Assignment,
) -> RegId {
    REGISTER_INFO.root(match register {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    })
}

fn is_data(register: RegId) -> bool {
    [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7].contains(&register)
}

fn defs<'a>(
    inst: &'a Inst,
    assignment: &'a Assignment,
) -> impl Iterator<Item = RegId> + 'a {
    inst.operands.iter().filter_map(move |o| if let Operand::Def(r) = o { Some(root(*r, assignment)) } else { None })
}

fn uses<'a>(
    inst: &'a Inst,
    assignment: &'a Assignment,
) -> impl Iterator<Item = RegId> + 'a {
    inst.operands.iter().filter_map(move |o| if let Operand::Use(r) = o { Some(root(*r, assignment)) } else { None })
}

/// The mnemonic's base and size of an instruction: `("add", 'l')`.
fn mnemonic(inst: &Inst) -> Option<(&'static str, char)> {
    let Kind::Op(name) = inst.kind else { return None };
    let text = crate::dagisel::asm::asm(name).map(|row| row.1)?;
    let word = text.split('\t').next()?;
    let (base, size) = word.split_once('.').unwrap_or((word, "l"));
    Some((base, size.chars().next()?))
}

/// Whether the codes a setter leaves match `tst` of its result for the
/// branches that read them: `N` and `Z` always; `V` where the setter clears it.
fn retest_matches(
    setter: &str,
    reader: &Inst,
) -> bool {
    let clears_overflow =
        matches!(
            setter,
            "move" | "moveq" | "and" | "or" | "eor" | "not" | "lsr" | "asr" | "ext" | "clr"
        );
    let result_kind = clears_overflow || matches!(
        setter,
        "add" | "sub" | "addq" | "subq" | "neg" | "lsl" | "asl"
    );
    result_kind
        && match reader.kind {
            Kind::Op("Bne8" | "Beq8" | "Bmi8" | "Bpl8") => true,
            Kind::Op("Bgt8" | "Bge8" | "Blt8" | "Ble8") => clears_overflow,
            _ => false,
        }
}

/// Makes every flag reader in `function` see the codes of the instruction it
/// meant. An error where no way is known.
pub fn preserve_flags(
    function: &mut MFunction,
    assignment: &Assignment,
) -> Result<(), String> {
    for block in &mut function.blocks {
        let mut r = 0;
        while r < block.insts.len() {
            if !block.insts[r].reads_flags || block.insts[r].sets_flags {
                r += 1;
                continue;
            }
            let Some(mut s) = block.insts[..r].iter().rposition(|inst| inst.sets_flags) else {
                r += 1;
                continue;
            };
            // a copy that becomes a `move` into a data register, from `s + 1`
            // on
            let clobbering = |block: &llrm_spike_mf::Block, s: usize, r: usize| -> Vec<usize> {
                (s + 1..r)
                    .filter(|&i| {
                        matches!(
                            block.insts[i].operands[..],
                            [Operand::Def(to), Operand::Use(from)] if block.insts[i].kind == Kind::Copy && is_data(root(to, assignment)) && root(to, assignment) != root(from, assignment)
                        )
                    })
                    .collect()
            };
            // those that do not depend on what runs from the setter up to them
            // move above the setter
            loop {
                let movable = clobbering(block, s, r).into_iter().find(|&i| {
                    let [Operand::Def(to), Operand::Use(from)] = block.insts[i].operands[..] else { return false };
                    let (dst, src) = (root(to, assignment), root(from, assignment));
                    !block
                        .insts[s..i]
                        .iter()
                        .any(
                            |before| defs(before, assignment).any(|d| d == src || d == dst)
                                || uses(before, assignment).any(|u| u == dst),
                        )
                });
                let Some(i) = movable else { break };
                let moved = block.insts.remove(i);
                block.insts.insert(s, moved);
                s += 1;
            }
            let stay = clobbering(block, s, r);
            if stay.is_empty() {
                r += 1;
                continue;
            }
            // flags are set again right before the reader
            let setter = block.insts[s].clone();
            let reader = block.insts[r].clone();
            let (base, size) = mnemonic(&setter).ok_or("a flag setter with no mnemonic")?;
            let clobbered: Vec<RegId> = stay
                .iter()
                .filter_map(|&i| {
                    if let Operand::Def(to) = block.insts[i].operands[0] { Some(root(to, assignment)) } else { None }
                })
                .collect();
            let again = if base.starts_with("cmp") {
                let reads_clobbered = uses(&setter, assignment).any(|u| clobbered.contains(&u));
                (!reads_clobbered).then(|| setter.clone())
            } else if size == 'l' && retest_matches(base, &reader) {
                let result = setter.operands.iter().find_map(|o| if let Operand::Def(d) = o { Some(*d) } else { None });
                result
                    .filter(|d| !clobbered.contains(&root(*d, assignment)))
                    .map(
                        |d| {
                            let mut test = Inst::new(Kind::Op("TST32d"), vec![Operand::Use(d)]);
                            test.sets_flags = true;
                            test
                        },
                    )
            } else {
                None
            };
            let Some(again) = again else {
                return Err(format!("no way to keep the flags of {:?} past a copy for {:?}", setter.kind, reader.kind));
            };
            block.insts.insert(r, again);
            r += 2;
        }
    }
    Ok(())
}
