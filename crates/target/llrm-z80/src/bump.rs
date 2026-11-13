//! A word stepped by a flag: `inc rr` / `dec rr` behind a `jr`.
//!
//! # How it works
//! The carry of a long's low word that no `adc` can take (an increment or a
//! decrement leaves none) comes out of selection as a flag read into a byte
//! (`SETCC8`, `ZEXT8`) and added to the high word. When the high word is
//! added to that byte alone, `jr` over `inc rr` (`dec rr` for a subtract)
//! does it in three bytes, whatever the flag: `inc hl; ld a, h; or l; jr nz,
//! 1$; inc de` is a long's increment.
//!
//! Nothing but copies may stand between the flag's reader and the add: they
//! leave the flags alone.
//!
//! DEVIATE: no LLVM pass. Reason: the pin's `add x, (zext (setcc))` stays an
//! add; a target with a condition-predicated increment has none. Exit: none.

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::reg;

/// Steps each word added to (subtracted by) a flag by `INC16c` / `DEC16c`.
pub fn step_by_flags(function: &mut MFunction) -> bool {
    let mut reads = std::collections::HashMap::<u32, usize>::new();
    for block in &function.blocks {
        for inst in &block.insts {
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    *reads.entry(*v).or_default() += 1;
                }
            }
        }
        for phi in &block.phis {
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    *reads.entry(*v).or_default() += 1;
                }
            }
        }
    }
    let mut changed = false;
    for block in &mut function.blocks {
        while let Some(found) = recognised(&block.insts, &reads) {
            apply(&mut block.insts, found);
            changed = true;
        }
    }
    changed
}

struct Found {
    /// `SETCC8`, `ZEXT8`.
    flag: [usize; 2],
    /// The copy into hl before the add, and the add.
    copies: Vec<usize>,
    add: usize,
    other: u32,
    condition: i64,
    subtract: bool,
}

fn defines(
    inst: &Inst,
    register: Reg,
) -> bool {
    inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == register))
}

fn recognised(
    insts: &[Inst],
    reads: &std::collections::HashMap<u32, usize>,
) -> Option<Found> {
    let (hl, flags) = (Reg::Phys(reg::HL), Reg::Phys(reg::FLAGS));
    for (add, inst) in insts.iter().enumerate() {
        let subtract = match inst.kind {
            Kind::Op("ADD16") => false,
            Kind::Op("SUB16") => true,
            _ => continue,
        };
        let [Operand::Def(d), Operand::Use(u), Operand::Use(Reg::Virt(rhs))] = inst.operands[..] else { continue };
        if d != hl || u != hl || add < 1 || insts[add - 1].kind != Kind::Copy || !defines(&insts[add - 1], hl) {
            continue;
        }
        let [_, Operand::Use(Reg::Virt(copied))] = insts[add - 1].operands[..] else { continue };
        let flag_of = |v: u32| -> Option<[usize; 2]> {
            if reads.get(&v) != Some(&1) {
                return None;
            }
            let zext = insts[..add].iter().rposition(|i| defines(i, Reg::Virt(v)))?;
            let [Operand::Def(_), Operand::Use(Reg::Virt(c))] = insts[zext].operands[..] else { return None };
            if insts[zext].kind != Kind::Op("ZEXT8") || reads.get(&c) != Some(&1) {
                return None;
            }
            let set = insts[..zext].iter().rposition(|i| defines(i, Reg::Virt(c)))?;
            (insts[set].kind == Kind::Op("SETCC8")).then_some([set, zext])
        };
        // the byte is the added operand, or (a sum only) the one copied in
        let (flag, other, copies) = if let Some(flag) = flag_of(rhs) {
            (flag, copied, vec![add - 1])
        } else if !subtract && let Some(flag) = flag_of(copied) {
            (flag, rhs, vec![add - 1])
        } else {
            continue;
        };
        let [Operand::Def(_), Operand::Imm(condition), Operand::Use(read)] = insts[flag[0]].operands[..] else {
            continue;
        };
        if read != flags || !(0..4).contains(&condition) {
            continue;
        }
        // only copies between the flag's reader and the add, and the sum
        // goes out by a copy of hl
        if insts[flag[0] + 1..add].iter().any(|i| i.kind != Kind::Copy && i.kind != Kind::Op("ZEXT8")) {
            continue;
        }
        if !matches!(
            insts.get(add + 1),
            Some(i) if i.kind == Kind::Copy && matches!(i.operands[..], [Operand::Def(Reg::Virt(_)), Operand::Use(r)] if r == hl)
        ) {
            continue;
        }
        return Some(Found { flag, copies, add, other, condition, subtract });
    }
    None
}

fn apply(
    insts: &mut Vec<Inst>,
    found: Found,
) {
    let Found { flag, copies, add, other, condition, subtract } = found;
    // A long's high word is in de: both steps are there. A subtract stepped in hl (the pair its sum was read from) moved
    // it through `ex de, hl` (long_step_by_one `dec`: 13 B for 7); corpus-wide, none of the 100 cells grows by it.
    let stepped = Reg::Phys(reg::DE);
    let flags = Reg::Phys(reg::FLAGS);
    let origin = insts[add].origin;
    let mut copy = Inst::new(Kind::Copy, vec![Operand::Def(stepped), Operand::Use(Reg::Virt(other))]);
    copy.origin = origin;
    let mut step = Inst::new(
        Kind::Op(if subtract { "DEC16c" } else { "INC16c" }),
        vec![Operand::Def(stepped), Operand::Use(stepped), Operand::Imm(condition), Operand::Use(flags)],
    );
    step.ties = vec![(0, 1)];
    step.reads_flags = true;
    step.origin = origin;
    // the sum's copy out reads the stepped word
    let out = &mut insts[add + 1];
    out.operands[1] = Operand::Use(stepped);
    insts.splice(add..=add, [copy, step]);
    let mut gone: Vec<usize> = flag.iter().chain(&copies).copied().collect();
    gone.sort_unstable();
    for &at in gone.iter().rev() {
        insts.remove(at);
    }
}
