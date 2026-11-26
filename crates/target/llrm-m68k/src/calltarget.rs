//! A callee called again and again, called through an address register:
//! `jsr (%a2)` (2 B) for each `jsr name` (6 B).
//!
//! # How it works
//! After allocation, [`through_registers`] puts `lea name, %aN` at the top of
//! the entry block and turns every direct call of `name` into `jsr (%aN)`.
//! `%aN` is a callee-saved address register the allocation left alone, so
//! nothing is spilled for it; the frame saves and restores it. [`choose`]
//! tries the callees most called first, and keeps one only when the function,
//! frame included, is cheaper under the level's [`Price`].
//!
//! DEVIATE: the pin calls `jsr name` every time (`M68kISelLowering::LowerCall`
//! emits `MxCall` of the global; no pass hoists the target). Reason: best of
//! gcc and clang is the goal; gcc keeps up to five call targets in registers
//! and memory_expansion's 900 calls are 3 KB of its gap. Owner: m68k. Exit:
//! 2026-12-05, the shared flow prices a call target as a register candidate
//! (convergence step 5) or this stays as the m68k value.
//!
//! The price is static (each instruction once, as [`crate::callpush`] prices a
//! sequence): a call in a loop gains more than it counts.
//!
//! # See also
//! `GCSE`/`cse_main` in gcc's `cse.cc`, which costs `jsr name` against `jsr
//! (an)`.

use std::collections::HashMap;

use llrm_lir::registers::RegId;
use llrm_spike_mf::{Kind, Operand, Reg};

use crate::dagisel::generated::instrinfo;
use crate::dagisel::hooks::Cpu;
use crate::expand::Line;
use crate::mfunc::{MFunction, op_inst};
use crate::regalloc::Assignment;
use crate::timing::Price;
use crate::{REGISTER_INFO, frame, mc, reg};

/// The address registers a call target may live in, in the order they are
/// taken.
/// `%a6` is the frame pointer when the frame has one, and is free otherwise.
const ADDRESS: [RegId; 5] = [reg::A2, reg::A3, reg::A4, reg::A5, reg::A6];

/// The callees `function` calls directly more than once, most called first.
pub fn repeated(function: &MFunction) -> Vec<String> {
    repeated_counts(function).into_iter().map(|(name, _)| name).collect()
}

/// [`repeated`], each with its number of calls.
fn repeated_counts(function: &MFunction) -> Vec<(String, usize)> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        if let (Kind::Op("CALLb" | "CALLq"), Some(Operand::Symbol(name, 0))) = (inst.kind, inst.operands.first()) {
            let seen = count.entry(name).or_insert(0);
            if *seen == 0 {
                order.push(name);
            }
            *seen += 1;
        }
    }
    order.retain(|name| count[name] > 1);
    order.sort_by_key(|name| std::cmp::Reverse(count[name]));
    order.into_iter().map(|name| (name.to_owned(), count[name])).collect()
}

/// What routing `count` calls of `name` through a register adds to a function
/// (bytes, clocks), the register's save and restore apart: the load is paid,
/// each call gets cheaper by at most the dearer of its two direct forms less
/// the call through a register.
fn routed_delta(
    name: &str,
    count: usize,
    cpu: Cpu,
) -> Option<(i64, i64)> {
    let register = mc::Operand::Reg(reg::A2);
    let through =
        crate::instprice::price_of(cpu, &mc::Inst { opcode: "CALLj", operands: vec![register.clone()] }).ok()?;
    let symbol = mc::Operand::Symbol(name.to_owned(), 0);
    let load =
        crate::instprice::price_of(cpu, &mc::Inst { opcode: "LEA32b", operands: vec![register, symbol.clone()] })
            .ok()?;
    let direct = |opcode| crate::instprice::price_of(cpu, &mc::Inst { opcode, operands: vec![symbol.clone()] }).ok();
    let (absolute, relative) = (direct("CALLb")?, direct("CALLq")?);
    let dearest = (absolute.0.max(relative.0), absolute.1.max(relative.1));
    let calls = count as i64;
    Some((
        load.0 as i64 - calls * (dearest.0 as i64 - through.0 as i64),
        i64::from(load.1) - calls * (i64::from(dearest.1) - i64::from(through.1)),
    ))
}

/// Whether some callee of `function` is called often enough that a register
/// for its address makes the function smaller even with the register's
/// four-byte `movem` share. The allocator then leaves `%a6` to this pass:
/// the pin allocates it, and `jsr (%a6)` for 91 calls of `copy_runs` -O2 was
/// 364 B it did not get back.
pub fn pays(
    function: &MFunction,
    cpu: Cpu,
) -> bool {
    const SAVE_AND_RESTORE_BYTES: i64 = 4;
    repeated_counts(function).iter().any(|(name, count)| {
        routed_delta(name, *count, cpu).is_some_and(|(bytes, _)| bytes + SAVE_AND_RESTORE_BYTES < 0)
    })
}

/// A lower bound on the price of routing `set`'s callees through registers,
/// from the price `best` of the function as it stands: each callee's load is
/// paid, each of its `count` calls gets cheaper by at most the dearer of its
/// two direct forms less the call through a register, and the saved register
/// and the frame only add. `None` where a price is not to be had.
fn lower_bound(
    set: &[(String, usize)],
    best: (usize, u32),
    cpu: Cpu,
) -> Option<(usize, u32)> {
    let (mut bytes, mut clocks) = (best.0 as i64, i64::from(best.1));
    for (name, count) in set {
        let (b, c) = routed_delta(name, *count, cpu)?;
        bytes += b;
        clocks += c;
    }
    // the register each callee takes is saved and restored by the frame: pushed
    // and popped one by one, or by `movem`, which adds no bytes for another
    // register and a few clocks; the cheaper of the two, at least
    let clocks_of =
        |text: &str, bytes: u32| crate::timing::cost(cpu, text, bytes, false).ok().map(|one| i64::from(one.clocks));
    let push = clocks_of("move.l %a2,-(%sp)", 2)? + clocks_of("move.l (%sp)+,%a2", 2)?;
    let movem = (clocks_of("movem.l %a2/%a3,-(%sp)", 4)? - clocks_of("movem.l %a2,-(%sp)", 4)?)
        + (clocks_of("movem.l (%sp)+,%a2/%a3", 4)? - clocks_of("movem.l (%sp)+,%a2", 4)?);
    clocks += set.len() as i64 * push.min(movem).max(0);
    Some((bytes.max(0) as usize, u32::try_from(clocks.max(0)).unwrap_or(u32::MAX)))
}

/// Whether any instruction of `function` names a callee-saved register, which
/// the frame then saves and restores.
pub fn saves_a_register(
    function: &MFunction,
    assignment: &Assignment,
) -> bool {
    let saved = frame::callee_saved();
    function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .flat_map(|inst| &inst.operands)
        .any(
            |operand| {
                let (Operand::Def(r) | Operand::Use(r)) = operand else { return false };
                let physical = match r {
                    Reg::Virt(v) => assignment.0[*v as usize],
                    Reg::Phys(p) => *p,
                };
                saved.contains(&REGISTER_INFO.root(physical))
            },
        )
}

/// The callee-saved address registers no instruction of `function` names.
pub fn unused(
    function: &MFunction,
    assignment: &Assignment,
) -> Vec<RegId> {
    let named = |wanted: RegId| {
        function
            .blocks
            .iter()
            .flat_map(|block| &block.insts)
            .flat_map(|inst| &inst.operands)
            .any(
                |operand| {
                    let (Operand::Def(r) | Operand::Use(r)) = operand else { return false };
                    let physical = match r {
                        Reg::Virt(v) => assignment.0[*v as usize],
                        Reg::Phys(p) => *p,
                    };
                    REGISTER_INFO.root(physical) == wanted
                },
            )
    };
    let saved = frame::callee_saved();
    ADDRESS.into_iter().filter(|one| saved.contains(one) && !named(*one)).collect()
}

/// Calls of each name in `targets` go through its register, which is loaded at
/// the top of the entry block.
pub fn through_registers(
    function: &mut MFunction,
    targets: &[(String, RegId)],
) -> Result<(), String> {
    let code = crate::dagisel::opcodes::code_of("LEA32b").ok_or("no LEA32b")?;
    let desc = instrinfo::desc(code).ok_or("no description of LEA32b")?;
    let mut loads = Vec::new();
    for (name, register) in targets {
        loads.push(op_inst("LEA32b", desc, vec![Operand::Def(Reg::Phys(*register)), Operand::Symbol(name.clone(), 0)]));
    }
    for inst in function.blocks.iter_mut().flat_map(|block| &mut block.insts) {
        let (Kind::Op("CALLb" | "CALLq"), Some(Operand::Symbol(name, 0))) = (inst.kind, inst.operands.first()) else {
            continue;
        };
        let Some((_, register)) = targets.iter().find(|(target, _)| target == name) else { continue };
        inst.kind = Kind::Op("CALLj");
        inst.operands[0] = Operand::Use(Reg::Phys(*register));
    }
    let entry = function.blocks.first_mut().ok_or("a function with no block")?;
    entry.insts.splice(0..0, loads);
    Ok(())
}

/// What each instruction costs on a cpu, by instruction: an instruction
/// that comes up again in another candidate's expansion is priced once.
type Prices = HashMap<mc::Inst, (usize, u32)>;

thread_local! {
    static PRICED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What `lines` cost on `cpu`: bytes and clocks, each instruction once.
fn price_of(
    lines: &[Line],
    cpu: Cpu,
    seen: &mut Prices,
) -> Result<(usize, u32), String> {
    let (mut bytes, mut clocks) = (0, 0);
    for line in lines {
        let Line::Inst(inst) = line else { continue };
        let (size, time) = match seen.get(inst) {
            Some(&priced) => priced,
            None => {
                PRICED.with(|count| count.set(count.get() + 1));
                let priced = crate::instprice::price_of(cpu, inst)?;
                seen.insert(inst.clone(), priced);
                priced
            }
        };
        bytes += size;
        clocks += time;
    }
    Ok((bytes, clocks))
}

/// The callees to call through registers, each with its register: those that
/// make the function cheaper under `price`, tried most called first. `lines`
/// expands a function to its instructions. With them, the expansion of the
/// function as it stands once they are called through registers, when the
/// search made it (the expansion it priced the choice by).
pub fn choose(
    function: &MFunction,
    assignment: &Assignment,
    cpu: Cpu,
    price: Price,
    lines: impl Fn(&MFunction) -> Result<Vec<Line>, String>,
) -> Result<(Vec<(String, RegId)>, Option<Vec<Line>>), String> {
    let mut chosen: Vec<(String, RegId)> = Vec::new();
    if function.returns_twice {
        return Ok((chosen, None));
    }
    let counted = repeated_counts(function);
    let candidates: Vec<String> = counted.iter().map(|(name, _)| name.clone()).collect();
    let calls = |name: &str| counted.iter().find(|(callee, _)| callee == name).map_or(0, |(_, count)| *count);
    let mut free = unused(function, assignment);
    if candidates.is_empty() || free.is_empty() {
        return Ok((chosen, None));
    }
    // a price the tables cannot give keeps the calls as they are
    let mut seen = Prices::new();
    let Ok(base) = lines(function) else { return Ok((chosen, None)) };
    let Ok(mut best) = price_of(&base, cpu, &mut seen) else { return Ok((chosen, None)) };
    let mut expansion = Some(base);
    // A callee that does not pay alone may pay with the ones tried before it:
    // the register's save and restore is paid once for them all.
    let mut pending: Vec<String> = Vec::new();
    for name in candidates {
        let mut taken = None;
        let with_pending: Vec<String> = pending.iter().cloned().chain(std::iter::once(name.clone())).collect();
        for set in [vec![name.clone()], with_pending] {
            if set.len() > free.len() || (set.len() > 1 && pending.is_empty()) {
                continue;
            }
            // a set that cannot beat the build as it stands even at the best
            // case is not expanded
            let members: Vec<(String, usize)> = set.iter().map(|callee| (callee.clone(), calls(callee))).collect();
            if lower_bound(&members, best, cpu).is_some_and(|low| !price.beats(cpu, low, best)) {
                continue;
            }
            let mut targets = chosen.clone();
            targets.extend(set.iter().cloned().zip(free.iter().copied()));
            let mut trial = function.clone();
            through_registers(&mut trial, &targets)?;
            let Ok(expanded) = lines(&trial) else { continue };
            // a frame that made `%a6` its pointer leaves it no call target
            if expanded.iter().any(|line| {
                matches!(
                    line,
                    Line::Inst(inst) if inst.opcode == "LINK16" || inst.opcode == "UNLK"
                )
            }) {
                continue;
            }
            let Some(cost) = price_of(&expanded, cpu, &mut seen).ok() else { continue };
            if price.beats(cpu, cost, best) {
                best = cost;
                taken = Some((targets, set.len()));
                expansion = Some(expanded);
                break;
            }
        }
        match taken {
            Some((targets, used)) => {
                free.drain(..used);
                chosen = targets;
                if used > 1 {
                    pending.clear();
                }
            }
            None => pending.push(name),
        }
    }
    Ok((chosen, expansion))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(
        opcode: &'static str,
        imm: i64,
    ) -> Line {
        Line::Inst(mc::Inst { opcode, operands: vec![mc::Operand::Imm(imm)] })
    }

    /// Every candidate callee re-expanded the whole function and priced every
    /// line again (`choose`: 17% of matmul's -O2 compile, 31% of grep's); the
    /// lines two expansions share are priced once.
    #[test]
    fn a_line_two_expansions_share_is_priced_once() {
        let first = [line("NOP", 0), line("RTS", 0)];
        let second = [line("NOP", 0), line("RTS", 0), line("NOP", 0)];
        let before = PRICED.with(std::cell::Cell::get);
        let mut seen = Prices::new();
        let one = price_of(&first, Cpu::M68000, &mut seen).expect("priced");
        let two = price_of(&second, Cpu::M68000, &mut seen).expect("priced");
        assert_eq!(PRICED.with(std::cell::Cell::get) - before, 2, "NOP and RTS, once each");
        assert_eq!(two.0, one.0 + 2, "the third line is the NOP again");
    }
}
