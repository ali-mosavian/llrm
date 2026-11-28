//! `x / y` and `x % y` of one pair of operands are one `divsl.l`.
//!
//! # How it works
//! [`remainders_from_quotients`] finds, in a block, a divide `q = x / y`, a
//! multiply of `q` and `y` that only a subtract reads, and the subtract
//! `r = x - q * y`, and makes the divide the `divsl.l y, r:q` of
//! [`crate::dagisel::divrem`]. The multiply and the subtract set the codes;
//! the pass runs only where nothing reads the multiply's, and a reader of the
//! subtract's gets a `tst.l` of the remainder. The multiply may read a copy
//! of the quotient and its own product (the two-address form).
//!
//! DEVIATE: the pin has no `DIVSL`; it divides and then multiplies back.
//! Reason: gcc divides once (muldiv 68020 -O2 42 B against 28). Exit:
//! 2026-11-12.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::cmpelim::flags_read_after;
use crate::dagisel::local;
use crate::foldload::use_counts;
use crate::mfunc::{MFunction, op_inst};

/// The divide and the pair it makes; the multiply back is `muls.l` for both,
/// the low 32 bits of a product being the same signed or not.
const FORMS: [(&str, &str); 2] = [("SDIVd32d32", "SDIVREM32"), ("UDIVd32d32", "UDIVREM32")];
const MULTIPLY: &str = "SMULd32d32";

/// The register a virtual register is a copy of, followed to its source: the
/// divisor of the divide and of the multiply back are often two copies of one
/// value.
fn sources(function: &MFunction) -> Vec<Reg> {
    let mut of: Vec<Reg> = (0..function.classes.len() as u32).map(Reg::Virt).collect();
    for block in &function.blocks {
        for inst in &block.insts {
            if let (Kind::Copy, [Operand::Def(Reg::Virt(to)), Operand::Use(from @ Reg::Virt(_))]) =
                (&inst.kind, &inst.operands[..])
            {
                of[*to as usize] = *from;
            }
        }
    }
    of
}

fn canonical(
    of: &[Reg],
    reg: Reg,
) -> Reg {
    let mut reg = reg;
    while let Reg::Virt(v) = reg {
        let next = of[v as usize];
        if next == reg {
            break;
        }
        reg = next;
    }
    reg
}

fn virt(operand: &Operand) -> Option<Reg> {
    match operand {
        Operand::Use(reg @ Reg::Virt(_)) | Operand::Def(reg @ Reg::Virt(_)) => Some(*reg),
        _ => None,
    }
}

pub fn remainders_from_quotients(function: &mut MFunction) {
    let uses = use_counts(function);
    let of = sources(function);
    let same = |a: Reg, b: Reg| canonical(&of, a) == canonical(&of, b);
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let Some(&(_, pair)) = FORMS.iter().find(|form| block.insts[at].kind == Kind::Op(form.0)) else {
                at += 1;
                continue;
            };
            let [Operand::Def(quotient), Operand::Use(dividend), Operand::Use(divisor)] = block.insts[at].operands[..]
            else {
                at += 1;
                continue;
            };
            // The multiply reads the quotient or the copy its two-address form
            // makes of it.
            let mut copies = vec![quotient];
            let mut found = None;
            for (offset, inst) in block.insts[at + 1..].iter().enumerate() {
                match (&inst.kind, &inst.operands[..]) {
                    (Kind::Copy, [Operand::Def(copy), Operand::Use(from)]) if copies.contains(from) => {
                        copies.push(*copy);
                    }
                    (Kind::Op(name), [Operand::Def(product), x, y]) if *name == MULTIPLY => {
                        let (Some(x), Some(y)) = (virt(x), virt(y)) else { continue };
                        let by_quotient =
                            (copies.contains(&x) && same(y, divisor)) || (copies.contains(&y) && same(x, divisor));
                        if by_quotient {
                            // the product read as its own tied operand is the
                            // multiply's, not a reader
                            let own = u32::from(x == *product) + u32::from(y == *product);
                            found = Some((*product, at + 1 + offset, own));
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some((product, back, own)) = found else {
                at += 1;
                continue;
            };
            let Reg::Virt(product_number) = product else { unreachable!() };
            if uses[product_number as usize] != 1 + own {
                at += 1;
                continue;
            }
            let Some((remainder, subtract)) = block.insts[back + 1..].iter().enumerate().find_map(|(offset, inst)| {
                let Kind::Op("SUB32dd") = inst.kind else { return None };
                let [Operand::Def(remainder), Operand::Use(from), Operand::Use(taken)] = inst.operands[..] else {
                    return None;
                };
                (same(from, dividend) && taken == product).then_some((remainder, back + 1 + offset))
            }) else {
                at += 1;
                continue;
            };
            // The multiply's codes may not be read. The subtract's may: a `tst`
            // of the remainder gives the codes the
            // compare elimination took from it (`divsl.l` sets them from the
            // quotient).
            let multiply_is_read = flags_read_after(&block.insts, back) != Some(0);
            let Some(read) = flags_read_after(&block.insts, subtract).filter(|_| !multiply_is_read) else {
                at += 1;
                continue;
            };
            let test = if read == 0 { None } else { local::code_of("TST32d").and_then(local::desc) };
            if read != 0 && test.is_none() {
                at += 1;
                continue;
            }
            let Some(desc) = local::code_of(pair).and_then(local::desc) else { break };
            block.insts[at] = op_inst(
                pair,
                desc,
                vec![Operand::Def(quotient), Operand::Def(remainder), Operand::Use(dividend), Operand::Use(divisor)],
            );
            match test {
                Some(desc) => block.insts[subtract] = op_inst("TST32d", desc, vec![Operand::Use(remainder)]),
                None => {
                    block.insts.remove(subtract);
                }
            }
            block.insts.remove(back);
            at += 1;
        }
    }
    drop_dead_copies(function);
}

/// The multiply's copy of the divisor has no reader once the multiply is gone.
fn drop_dead_copies(function: &mut MFunction) {
    loop {
        let uses = use_counts(function);
        let mut removed = false;
        for block in &mut function.blocks {
            let before = block.insts.len();
            block.insts.retain(|inst| {
                !matches!(
                    (&inst.kind, &inst.operands[..]),
                    (Kind::Copy, [Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(_))]) if uses[*to as usize] == 0
                )
            });
            removed |= block.insts.len() != before;
        }
        if !removed {
            break;
        }
    }
}
