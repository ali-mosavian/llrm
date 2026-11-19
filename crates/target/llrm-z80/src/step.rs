//! A pair stepped by two or three: `inc rr` / `dec rr` that many times.
//!
//! # How it works
//! `ld rr, #k` and `add hl, rr` with k in 2..=3 or -3..=-2 become one
//! instruction on `hl` alone, `INC16xN` / `DEC16xN`, when nothing reads the
//! carry the add set. The printer made the same chain from the pair (the
//! `add_small_constant` of [`crate::expand`]) but the allocator had been
//! charged a pair for the constant: fill_nest's loop constant was made again
//! on every trip because that pair took its register.
//!
//! This runs after [`crate::carry`], which reads an `add hl, rr` whose carry
//! feeds the high word of a wide add; a step selected before it would hide that
//! add.
//!
//! # See also
//! `X86InstrInfo.td` `ADD32ri`: the constant is an immediate operand, not a
//! register.

use llrm_support::hash::HashMap;

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::reg;

/// Steps by 2 or 3, as the instruction name; `None` for any other constant.
fn step_name(value: i64) -> Option<&'static str> {
    let value = if value >= 0x8000 { value - 0x1_0000 } else { value };
    match value {
        2 => Some("INC16x2"),
        3 => Some("INC16x3"),
        -2 => Some("DEC16x2"),
        -3 => Some("DEC16x3"),
        _ => None,
    }
}

/// Replaces each `ld rr, #k; add hl, rr` whose constant has no other reader
/// and whose carry nothing reads by a step on `hl`. Whether anything changed.
pub fn step_by_small_constants(function: &mut MFunction) -> bool {
    let mut defs: HashMap<u32, usize> = HashMap::default();
    let mut uses: HashMap<u32, usize> = HashMap::default();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            match operand {
                Operand::Def(Reg::Virt(v)) => *defs.entry(*v).or_default() += 1,
                Operand::Use(Reg::Virt(v)) => *uses.entry(*v).or_default() += 1,
                _ => {}
            }
        }
    }
    // every constant a pair is made from, wherever it sits: the hoisting
    // moves it out of the loop the add is in
    let mut made: HashMap<u32, (usize, usize, i64)> = HashMap::default();
    for (b, block) in function.blocks.iter().enumerate() {
        for (n, inst) in block.insts.iter().enumerate() {
            if let (Kind::Op("LD16ri"), [Operand::Def(Reg::Virt(v)), Operand::Imm(value)]) =
                (inst.kind, &inst.operands[..])
            {
                made.insert(*v, (b, n, *value));
            }
        }
    }
    let mut changed = false;
    let mut gone: Vec<(usize, usize)> = Vec::new();
    for block in &mut function.blocks {
        for at in 0..block.insts.len() {
            let inst = &block.insts[at];
            let [Operand::Def(Reg::Phys(d)), Operand::Use(Reg::Phys(a)), Operand::Use(Reg::Virt(v))] =
                &inst.operands[..]
            else {
                continue;
            };
            if inst.kind != Kind::Op("ADD16")
                || *d != reg::HL
                || *a != reg::HL
                || defs.get(v) != Some(&1)
                || uses.get(v) != Some(&1)
            {
                continue;
            }
            let Some(&(b, n, value)) = made.get(v) else { continue };
            let Some(name) = step_name(value) else { continue };
            // the carry is dead where the block sets the flags again before any
            // reader; a block that ends first leaves it to the printer's check
            let dead = block.insts[at + 1..]
                .iter()
                .find(|one| one.reads_flags() || one.sets_flags())
                .is_some_and(|one| one.sets_flags() && !one.reads_flags());
            if !dead {
                continue;
            }
            let mut step: Inst = inst.clone();
            step.kind = Kind::Op(name);
            step.operands.truncate(2);
            step.sets_flags = false;
            step.commutable = false;
            block.insts[at] = step;
            gone.push((b, n));
            changed = true;
        }
    }
    gone.sort_unstable_by(|x, y| y.cmp(x));
    for (b, n) in gone {
        function.blocks[b].insts.remove(n);
    }
    changed
}
