//! A pointer moved by a constant is added to in an address register: `addq`,
//! `lea` or `adda`, whichever costs least on the CPU, in place of `move %a0,
//! %d2; addq #6, %d2; move %d2, %a0`.
//!
//! # How it works
//! The pin's add of an immediate takes and gives a data register, so a pointer
//! that loads from its base goes through two copies around it. [`address_adds`]
//! finds an add whose input is a copy of an address register (used only by the
//! add) and whose result reaches nothing but address registers, through copies
//! and phis, and makes it an address-register instruction priced from
//! the timing tables: `addq`/`subq` for 1 to 8, `lea (d16,An)` for a
//! displacement that fits 16 bits, `adda` otherwise. On a smaller size they
//! are ordered by bytes first. None of the three change the codes, so the add
//! must not be one whose codes a branch reads.
//!
//! DEVIATE: the pin keeps the add in a data register and leaves the class to
//! its register allocator, which splits it with copies. Reason: this
//! allocator does not split, and best of gcc and clang is the goal. Exit: none.
//!
//! # See also
//! `M68kInstrInfo::copyPhysReg`, which is what the copies become.

use std::collections::HashSet;

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::dagisel::hooks::Cpu;
use crate::dagisel::local;
use crate::mc;
use crate::mfunc::{Class, MFunction, op_inst};
use crate::reg;

/// A use of a virtual register: an instruction operand, or a phi's incoming.
enum Use {
    /// A copy into this register.
    Copy(u32),
    Phi(u32),
    Other,
}

/// Every use of every virtual register, in one pass over the function. A
/// rewrite only removes the use of the copy it deletes and defines a register
/// no one else uses, so the table stays right for the registers it is asked
/// about.
fn uses_by_register(function: &MFunction) -> Vec<Vec<Use>> {
    let mut all: Vec<Vec<Use>> = (0..function.classes.len()).map(|_| Vec::new()).collect();
    for block in &function.blocks {
        for phi in &block.phis {
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    all[*v as usize].push(match phi.result {
                        Reg::Virt(result) => Use::Phi(result),
                        Reg::Phys(_) => Use::Other,
                    });
                }
            }
        }
        for inst in &block.insts {
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    all[*v as usize].push(match (&inst.kind, &inst.operands[..]) {
                        (Kind::Copy, [Operand::Def(Reg::Virt(dst)), _]) => Use::Copy(*dst),
                        _ => Use::Other,
                    });
                }
            }
        }
    }
    all
}

/// Whether everything `v` reaches, through copies and phis, ends in an address
/// register.
fn reaches_addresses_only(
    function: &MFunction,
    uses: &[Vec<Use>],
    v: u32,
    seen: &mut HashSet<u32>,
) -> bool {
    if !seen.insert(v) {
        return true;
    }
    let own = &uses[v as usize];
    !own.is_empty()
        && own.iter().all(|one| match one {
            Use::Copy(to) | Use::Phi(to) => match function.classes[*to as usize] {
                Class::Ar32 => true,
                Class::Xr32 => reaches_addresses_only(function, uses, *to, seen),
                _ => false,
            },
            Use::Other => false,
        })
}

/// The instruction that adds `value` to an address register by `name`'s form,
/// as machine operands `[result, base, ...]`.
struct Candidate {
    opcode: &'static str,
    /// The operands for a result `a` and a base `b`.
    operands: fn(Reg, Reg, i64) -> Vec<Operand>,
    value: i64,
}

fn tied(
    result: Reg,
    base: Reg,
    value: i64,
) -> Vec<Operand> {
    vec![Operand::Def(result), Operand::Use(base), Operand::Imm(value)]
}

fn displaced(
    result: Reg,
    base: Reg,
    value: i64,
) -> Vec<Operand> {
    vec![Operand::Def(result), Operand::Imm(value), Operand::Use(base)]
}

fn candidates(value: i64) -> Vec<Candidate> {
    let mut all = Vec::new();
    if (1..=8).contains(&value) {
        all.push(Candidate { opcode: "ADDQ32ai", operands: tied, value });
    } else if (-8..=-1).contains(&value) {
        all.push(Candidate { opcode: "SUBQ32ai", operands: tied, value: -value });
    }
    if (-32768..=32767).contains(&value) {
        all.push(Candidate { opcode: "LEA32p", operands: displaced, value });
    }
    all.push(Candidate { opcode: "ADD32ai", operands: tied, value });
    all
}

/// The clocks and bytes of `one` on `cpu`, with a fixed pair of registers.
fn price(
    one: &Candidate,
    cpu: Cpu,
) -> Option<(u32, u32)> {
    let (a, b) = (llrm_lir::registers::RegId::from(reg::A1), llrm_lir::registers::RegId::from(reg::A0));
    let operands = (one.operands)(Reg::Phys(a), Reg::Phys(b), one.value)
        .into_iter()
        .map(|operand| match operand {
            Operand::Def(Reg::Phys(r)) | Operand::Use(Reg::Phys(r)) => mc::Operand::Reg(r),
            Operand::Imm(value) => mc::Operand::Imm(value),
            _ => unreachable!("a candidate has registers and an immediate"),
        })
        .collect::<Vec<_>>();
    // a tied form names its register once as a result and once as the base: the
    // same register
    let operands = if one.opcode == "LEA32p" {
        operands
    } else {
        vec![operands[0].clone(), operands[0].clone(), operands[2].clone()]
    };
    let inst = mc::Inst { opcode: one.opcode, operands };
    let bytes = crate::encode::encode(&inst).ok()?.bytes.len() as u32;
    let clocks = crate::timing::cost(cpu, &mc::print(&inst).ok()?, bytes, false).ok()?.clocks;
    Some((clocks, bytes))
}

fn cheapest(
    value: i64,
    cpu: Cpu,
    size: bool,
) -> Option<Candidate> {
    candidates(value)
        .into_iter()
        .filter_map(|one| {
            price(&one, cpu).map(|(clocks, bytes)| (if size { (bytes, clocks) } else { (clocks, bytes) }, one))
        })
        .min_by_key(|(key, _)| *key)
        .map(|(_, one)| one)
}

/// Makes each pointer add that qualifies an address-register instruction.
pub fn address_adds(
    function: &mut MFunction,
    cpu: Cpu,
    size: bool,
) {
    let uses = uses_by_register(function);
    for b in 0..function.blocks.len() {
        let mut at = 0;
        while at < function.blocks[b].insts.len() {
            if let Some(done) = rewrite_one(function, &uses, b, at, cpu, size) {
                // the copy before the add is gone
                at = done;
            } else {
                at += 1;
            }
        }
    }
}

fn rewrite_one(
    function: &mut MFunction,
    uses: &[Vec<Use>],
    b: usize,
    at: usize,
    cpu: Cpu,
    size: bool,
) -> Option<usize> {
    let insts = &function.blocks[b].insts;
    let Kind::Op(name) = insts[at].kind else { return None };
    let sign = match name {
        "ADD32di" | "ADDQ32di" => 1,
        "SUB32di" | "SUBQ32di" => -1,
        _ => return None,
    };
    let [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(x)), Operand::Imm(value)] = insts[at].operands[..] else {
        return None;
    };
    // the base: a copy of an address register, read by this add alone
    let copy = insts[..at]
        .iter()
        .rposition(
            |one| matches!(
                &one.operands[..],
                [Operand::Def(Reg::Virt(def)), _] if *def == x
            ),
        )?;
    let Kind::Copy = insts[copy].kind else { return None };
    let Operand::Use(base @ Reg::Virt(y)) = insts[copy].operands[1] else { return None };
    if function.classes[y as usize] != Class::Ar32 || uses[x as usize].len() != 1 {
        return None;
    }
    // the add's codes are read by nobody
    for after in &insts[at + 1..] {
        if after.sets_flags || after.call {
            break;
        }
        if after.reads_flags {
            return None;
        }
    }
    if !reaches_addresses_only(function, uses, d, &mut HashSet::new()) {
        return None;
    }
    let one = cheapest(sign * value, cpu, size)?;
    let desc = local::desc(local::code_of(one.opcode)?)?;
    let mut inst: Inst = op_inst(one.opcode, desc, (one.operands)(Reg::Virt(d), base, one.value));
    // none of the three changes the codes; the pin's `adda` says it does
    inst.sets_flags = false;
    function.classes[d as usize] = Class::Ar32;
    let insts = &mut function.blocks[b].insts;
    insts[at] = inst;
    insts.remove(copy);
    Some(at)
}
