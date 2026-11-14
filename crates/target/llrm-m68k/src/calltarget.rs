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
use crate::dagisel::generated::m68k_m68000::MACHINE_OPCODES;
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
    order.into_iter().map(str::to_owned).collect()
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
    let code = MACHINE_OPCODES.iter().find(|(_, name)| *name == "LEA32b").map(|(code, _)| *code).ok_or("no LEA32b")?;
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
                let size = crate::encode::encode(inst)?.bytes.len();
                let priced = (size, crate::timing::cost(cpu, &mc::print(inst)?, size as u32, false)?.clocks);
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
/// expands a function to its instructions.
pub fn choose(
    function: &MFunction,
    assignment: &Assignment,
    cpu: Cpu,
    price: Price,
    lines: impl Fn(&MFunction) -> Result<Vec<Line>, String>,
) -> Result<Vec<(String, RegId)>, String> {
    let mut chosen: Vec<(String, RegId)> = Vec::new();
    if function.returns_twice {
        return Ok(chosen);
    }
    let candidates = repeated(function);
    let mut free = unused(function, assignment);
    if candidates.is_empty() || free.is_empty() {
        return Ok(chosen);
    }
    // a price the tables cannot give keeps the calls as they are
    let mut seen = Prices::new();
    let Some(mut best) = lines(function).and_then(|lines| price_of(&lines, cpu, &mut seen)).ok() else {
        return Ok(chosen);
    };
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
    Ok(chosen)
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
