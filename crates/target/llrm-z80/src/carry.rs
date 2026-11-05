//! The carry of a long add or subtract, in the flag.
//!
//! # How it works
//! A 32-bit add comes out of selection as the add of the low words, the carry
//! rebuilt by a compare (`sum < addend`, `SETCC8`, `ZEXT8`), and the high
//! words' sum with that byte added: nine instructions and a branch where the
//! Z80 has `adc hl, rr`. When the carry is the one the low add (or `sub hl,
//! rr`) left in the flag, the compare and the byte go, the high add moves
//! behind the low one, and it is `adc hl, rr` (`sbc hl, rr`) of the two high
//! words.
//!
//! Only `ld`, `ex` and `push`/`pop` stand between the two: none touches the
//! flags.
//!
//! DEVIATE: no LLVM pass. Reason: the pin's `ExpandIntRes_ADDSUB` takes the
//! target's `UADDO_CARRY` where it is legal; this target's carry is not a value
//! selection can name yet. Exit: `UADDO_CARRY` selected to `adc hl, rr`.

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::reg;

/// Fuses the long adds and subtracts of `function` into `adc` / `sbc`.
pub fn use_the_carry(function: &mut MFunction) -> bool {
    // every read of a virtual register in the function: the byte and the sums
    // go with the instructions only if nothing outside them reads them (a
    // condition byte is carried across blocks by a copy)
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
            apply(&mut block.insts, &mut function.classes, found);
            changed = true;
        }
    }
    changed
}

struct Found {
    /// The high word's `Copy AH <- a; ADD16 / SUB16; Copy v <- AH`; none where
    /// the high word is added to the carry alone.
    high: Option<[usize; 3]>,
    /// The high word's value where the byte was the operand copied into hl.
    added: Option<u32>,
    /// A copy made between the low op and the high add, moved up with the pair.
    hoist: Option<usize>,
    /// The compare, the condition byte and its widening.
    carry: [usize; 3],
    /// The final `Copy AH <- v` and the add of the carry; its result copy
    /// stays.
    last: [usize; 2],
    /// The low op's result copy: the new pair goes behind it.
    low_out: usize,
    subtract: bool,
}

fn virt(operand: &Operand) -> Option<u32> {
    match operand {
        Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) => Some(*v),
        _ => None,
    }
}

fn defines(
    inst: &Inst,
    register: Reg,
) -> bool {
    inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == register))
}

fn uses(
    inst: &Inst,
    v: u32,
) -> usize {
    inst.operands.iter().filter(|o| matches!(o, Operand::Use(Reg::Virt(w)) if *w == v)).count()
}

fn recognised(
    insts: &[Inst],
    reads: &std::collections::HashMap<u32, usize>,
) -> Option<Found> {
    let hl = Reg::Phys(reg::HL);
    let flags = Reg::Phys(reg::FLAGS);
    for (f, last) in insts.iter().enumerate() {
        let subtract = match last.kind {
            Kind::Op("ADD16") => false,
            Kind::Op("SUB16") => true,
            _ => continue,
        };
        let [Operand::Def(_), Operand::Use(_), Operand::Use(Reg::Virt(rhs))] = last.operands[..] else { continue };
        let Some(into) = insts[..f].iter().rposition(|i| defines(i, hl)) else { continue };
        if into + 1 != f || insts[into].kind != Kind::Copy {
            continue;
        }
        let [_, Operand::Use(Reg::Virt(copied))] = insts[into].operands[..] else { continue };
        // the carry's byte is the added operand, or (a sum only) the one
        // copied into hl
        let widened = |v: u32| {
            insts[..f]
                .iter()
                .rposition(|i| defines(i, Reg::Virt(v)))
                .is_some_and(|at| insts[at].kind == Kind::Op("ZEXT8"))
        };
        let (z, high_sum, commuted) = if widened(rhs) {
            (rhs, copied, false)
        } else if !subtract && widened(copied) {
            (copied, rhs, true)
        } else {
            continue;
        };
        // the byte: zext8 <- setcc8 (carry) <- cp16
        let Some(zext) = insts[..f].iter().rposition(|i| defines(i, Reg::Virt(z))) else { continue };
        let [Operand::Def(_), Operand::Use(Reg::Virt(c))] = insts[zext].operands[..] else { continue };
        if insts[zext].kind != Kind::Op("ZEXT8") {
            continue;
        }
        let Some(set) = insts[..zext].iter().rposition(|i| defines(i, Reg::Virt(c))) else { continue };
        if insts[set].kind != Kind::Op("SETCC8") || !matches!(insts[set].operands[..], [_, Operand::Imm(3), _]) {
            continue;
        }
        let Some(cp) = insts[..set].iter().rposition(|i| defines(i, flags)) else { continue };
        if insts[cp].kind != Kind::Op("CP16") {
            continue;
        }
        let [_, Operand::Use(Reg::Virt(x)), Operand::Use(Reg::Virt(y)), ..] = insts[cp].operands[..] else { continue };
        // the low word's op: the copy into hl, the op, the copy out. A sum is
        // compared with an addend, so its op comes first; a difference
        // compares the operands themselves, and its op may come after
        let wanted = if subtract { Kind::Op("SUB16") } else { Kind::Op("ADD16") };
        let low = insts.iter().enumerate().find_map(|(at, i)| {
            let [Operand::Def(a), Operand::Use(b), Operand::Use(Reg::Virt(rhs))] = i.operands[..] else { return None };
            if i.kind != wanted || a != hl || b != hl || at == f || at < 1 {
                return None;
            }
            let [Operand::Def(_), Operand::Use(Reg::Virt(lhs))] = insts[at - 1].operands[..] else { return None };
            let Some(Operand::Def(Reg::Virt(sum))) = insts.get(at + 1).and_then(|n| n.operands.first()) else {
                return None;
            };
            let copies = insts[at - 1].kind == Kind::Copy
                && insts[at + 1].kind == Kind::Copy
                && defines(&insts[at - 1], hl)
                && matches!(insts[at + 1].operands[1], Operand::Use(r) if r == hl);
            let carries = if subtract { (x, y) == (lhs, rhs) } else { at < cp && x == *sum && (y == rhs || y == lhs) };
            (copies && carries).then_some(at)
        });
        let Some(low) = low else { continue };
        let low_out = low + 1;
        // the compare's flags are read by the byte alone
        let others = insts[cp + 1..]
            .iter()
            .enumerate()
            .take_while(|(_, i)| !i.sets_flags)
            .any(|(at, i)| i.reads_flags && cp + 1 + at != set);
        if others {
            continue;
        }
        // the high word's own op
        let defined_before = |v: u32| match insts.iter().position(|i| defines(i, Reg::Virt(v))) {
            Some(at) => at < low_out,
            None => true,
        };
        if [c, z].iter().any(|v| reads.get(v) != Some(&1)) {
            continue;
        }
        // the high words' own sum (`Copy AH <- a; ADD16; Copy v <- AH`), or a
        // plain value: a long grown by a widened word has no high addend
        let wanted = if subtract { Kind::Op("SUB16") } else { Kind::Op("ADD16") };
        let chain = (|| {
            if commuted {
                return None;
            }
            let out = insts[..into].iter().rposition(|i| defines(i, Reg::Virt(high_sum)))?;
            if insts[out].kind != Kind::Copy || out < 2 || reads.get(&high_sum) != Some(&1) {
                return None;
            }
            let (high_op, high_in) = (out - 1, out - 2);
            let high = &insts[high_op];
            if high.kind != wanted || insts[high_in].kind != Kind::Copy || !defines(&insts[high_in], hl) {
                return None;
            }
            let [Operand::Def(_), Operand::Use(_), Operand::Use(Reg::Virt(high_rhs))] = high.operands[..] else {
                return None;
            };
            let [_, Operand::Use(Reg::Virt(high_lhs))] = insts[high_in].operands[..] else { return None };
            (defined_before(high_lhs) && defined_before(high_rhs)).then_some([high_in, high_op, out])
        })();
        // a plain value made after the low op by a copy of an earlier one moves
        // up with the pair
        let mut hoist = None;
        if chain.is_none() && !defined_before(high_sum) {
            let made = insts.iter().position(|i| defines(i, Reg::Virt(high_sum)));
            let ok = made
                .filter(|&at| at > low_out && at < f)
                .filter(
                    |&at| matches!(
                        insts[at].operands[..],
                        [Operand::Def(_), Operand::Use(Reg::Virt(src))] if defined_before(src)
                    ) && insts[at].kind == Kind::Copy
                        && reads.get(&high_sum) == Some(&1),
                );
            match ok {
                Some(at) => hoist = Some(at),
                None => continue,
            }
        }
        // the result copy follows the pair now: nothing may read its value
        // between the old place and the new
        let Some(result) = insts.get(f + 1) else { continue };
        let [Operand::Def(Reg::Virt(done)), Operand::Use(r)] = result.operands[..] else { continue };
        if result.kind != Kind::Copy || r != hl {
            continue;
        }
        let (from, to) = (f.min(low_out), f.max(low_out));
        if insts[from + 2..=to].iter().any(|i| uses(i, done) > 0) {
            continue;
        }
        return Some(Found {
            high: chain,
            added: commuted.then_some(high_sum),
            hoist,
            carry: [cp, set, zext],
            last: [into, f],
            low_out,
            subtract,
        });
    }
    None
}

fn apply(
    insts: &mut Vec<Inst>,
    classes: &mut Vec<crate::mfunc::Class>,
    found: Found,
) {
    let flags = Reg::Phys(reg::FLAGS);
    let Found { high, added: commuted, hoist, carry, last, low_out, subtract } = found;
    // the low op now leaves its carry for the pair behind it
    let low = low_out - 1;
    insts[low].operands.push(Operand::Def(flags));
    let mut added = Vec::new();
    if let Some(at) = hoist {
        added.push(insts[at].clone());
    }
    let (mut high_in, mut wide) = match high {
        Some(high) => (insts[high[0]].clone(), insts[high[1]].clone()),
        None => {
            // no high addend: a zero in a register of its own
            let zero = classes.len() as u32;
            classes.push(crate::mfunc::Class::Gr16);
            added.push(Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(Reg::Virt(zero)), Operand::Imm(0)]));
            let mut wide = insts[last[1]].clone();
            wide.operands[2] = Operand::Use(Reg::Virt(zero));
            let mut into = insts[last[0]].clone();
            if let Some(value) = commuted {
                into.operands[1] = Operand::Use(Reg::Virt(value));
            }
            (into, wide)
        }
    };
    wide.kind = Kind::Op(if subtract { "SBC16" } else { "ADC16" });
    wide.reads_flags = true;
    wide.sets_flags = true;
    wide.operands.push(Operand::Use(flags));
    wide.operands.push(Operand::Def(flags));
    let result = insts[last[1] + 1].clone();
    high_in.origin = insts[last[1]].origin;
    // the high sum's copy out is gone with its op; the result copy follows
    // the pair
    let mut gone: Vec<usize> = high.iter().flatten().chain(&carry).chain(&last).copied().collect();
    gone.push(last[1] + 1);
    gone.extend(hoist);
    gone.sort_unstable();
    let behind = low_out + 1 - gone.iter().filter(|&&at| at <= low_out).count();
    for &at in gone.iter().rev() {
        insts.remove(at);
    }
    added.extend([high_in, wide, result]);
    insts.splice(behind..behind, added);
}
