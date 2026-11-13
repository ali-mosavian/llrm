//! Two builds of one function, the cheaper kept.
//!
//! # How it works
//! [`cheaper`] prices both builds on the CPU: bytes as they are, clocks
//! weighted by loop depth (a backward branch to a label makes the lines
//! between that can reach it a loop; each nesting multiplies by ten), then asks
//! the level's [`Price`] whether the first beats the second. A price the tables
//! cannot give keeps the second (the build without the optional pass).
//!
//! A pass whose price needs the allocation (the moves an address register
//! costs) is decided here, by building the function both ways, instead of
//! guessing before allocation.

use std::collections::HashMap;

use crate::compile::Item;
use crate::dagisel::hooks::Cpu;
use crate::mc;
use crate::timing::Price;

/// How a block ends: where its branch goes, and whether control also falls on.
#[derive(Clone, Copy)]
enum Ending {
    Return,
    Jump(usize),
    Branch(usize),
    Falls,
}

fn ending(
    inst: &mc::Inst,
    labels: &HashMap<&str, usize>,
) -> Option<Ending> {
    let target = inst
        .operands
        .iter()
        .find_map(
            |operand| match operand {
                mc::Operand::Symbol(name, 0) => labels.get(name.as_str()).copied(),
                _ => None,
            },
        );
    match (inst.opcode, target) {
        ("RTS", _) => Some(Ending::Return),
        (name, Some(to)) if name.starts_with("BRA") => Some(Ending::Jump(to)),
        (name, Some(to)) if name.starts_with('B') && !name.starts_with("BSR") => Some(Ending::Branch(to)),
        _ => None,
    }
}

/// The bytes of each item once the assembler has laid the code out: an 8-bit
/// branch whose displacement does not fit, or is 0, takes its 16-bit form
/// (`object::assemble`). Without this a build with one more far branch looks
/// 2 bytes smaller than it is.
fn relaxed_sizes(
    items: &[Item],
    labels: &HashMap<&str, usize>,
) -> Option<Vec<usize>> {
    let mut sizes: Vec<usize> = items
        .iter()
        .map(|item| match item {
            Item::Inst(inst) => crate::encode::encode(inst).ok().map(|encoded| encoded.bytes.len()),
            Item::Bytes(bytes) => Some(bytes.len()),
            Item::Table(labels) => Some(4 * labels.len()),
            _ => Some(0),
        })
        .collect::<Option<_>>()?;
    loop {
        let mut offsets = Vec::with_capacity(items.len() + 1);
        let mut at = 0usize;
        for size in &sizes {
            offsets.push(at);
            at += size;
        }
        offsets.push(at);
        let mut changed = false;
        for (index, item) in items.iter().enumerate() {
            let Item::Inst(inst) = item else { continue };
            if !(inst.opcode.starts_with('B') && inst.opcode.ends_with('8')) || sizes[index] != 2 {
                continue;
            }
            let Some(target) = inst.operands.iter().find_map(|operand| match operand {
                mc::Operand::Symbol(name, 0) => labels.get(name.as_str()).copied(),
                _ => None,
            }) else {
                continue;
            };
            let displacement = offsets[target] as i64 - (offsets[index] as i64 + 2);
            if !(-128..128).contains(&displacement) || displacement == 0 {
                sizes[index] = 4;
                changed = true;
            }
        }
        if !changed {
            return Some(sizes);
        }
    }
}

/// The clocks of the runtime routine `inst` calls, which the call instruction
/// does not show ([`crate::routine`]).
fn callee_clocks(
    inst: &mc::Inst,
    cpu: Cpu,
    hoisted: &[llrm_lir::registers::RegId],
) -> u32 {
    let direct =
        inst.operands.iter().any(|operand| matches!(operand, mc::Operand::Symbol(name, 0) if name == "__mulsi3"));
    // a call through the register `lea __mulsi3, An` set (the call-target
    // hoist)
    let through = inst.opcode == "CALLj"
        && inst.operands.iter().any(|operand| matches!(operand, mc::Operand::Reg(reg) if hoisted.contains(reg)));
    if inst.opcode.starts_with("CALL") && (direct || through) { crate::routine::mulsi3_floor(cpu) } else { 0 }
}

/// The registers `lea __mulsi3, An` loads.
fn mulsi3_registers(items: &[Item]) -> Vec<llrm_lir::registers::RegId> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Inst(inst) if inst.opcode == "LEA32b" => match inst.operands.as_slice() {
                [mc::Operand::Reg(reg), mc::Operand::Symbol(name, 0)] if name == "__mulsi3" => Some(*reg),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// The items of the loop headed at `header` whose back edges are at `latches`:
/// those in the span that can reach a back edge without leaving it. The exit
/// path laid out between two back edges (a `bra` to the code after the loop)
/// is in the span but runs once per call, not once per trip.
fn loop_members(
    ends: &[Option<Ending>],
    header: usize,
    latches: &[usize],
) -> Vec<bool> {
    let last = latches.iter().copied().max().unwrap_or(header);
    let mut member = vec![false; ends.len()];
    for &latch in latches {
        member[latch] = true;
    }
    loop {
        let mut changed = false;
        for at in (header..=last).rev() {
            if member[at] {
                continue;
            }
            let (first, second) = match ends[at] {
                Some(Ending::Return) => (None, None),
                Some(Ending::Jump(to)) => (Some(to), None),
                Some(Ending::Branch(to)) => (Some(to), Some(at + 1)),
                _ => (Some(at + 1), None),
            };
            if [first, second].into_iter().flatten().any(|next| (header..=last).contains(&next) && member[next]) {
                member[at] = true;
                changed = true;
            }
        }
        if !changed {
            return member;
        }
    }
}

/// Bytes, and clocks of `items` on `cpu` weighted by how often each runs: an
/// instruction counts the share of runs that reach it (a two-way branch splits
/// evenly) times ten per loop it sits in ([`loop_members`]).
fn price_of(
    items: &[Item],
    cpu: Cpu,
    cold_clocks: bool,
) -> Option<(usize, u32)> {
    let labels: HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .filter_map(|(at, item)| if let Item::Label(name) = item { Some((name.as_str(), at)) } else { None })
        .collect();
    // header -> its back edges: several to one label are one loop
    let mut loops: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut ends_of: Vec<Option<Ending>> = Vec::with_capacity(items.len());
    // the share of runs reaching each item, along forward edges only, in
    // thousandths
    let mut share = vec![0u64; items.len() + 1];
    share[0] = 1000;
    for (at, item) in items.iter().enumerate() {
        let Item::Inst(inst) = item else {
            // a label or a line mark passes the share on
            share[at + 1] += share[at];
            ends_of.push(None);
            continue;
        };
        let ends = ending(inst, &labels);
        ends_of.push(ends);
        let (jump, branch) = match ends {
            Some(Ending::Jump(to)) => (Some(to), None),
            Some(Ending::Branch(to)) => (None, Some(to)),
            _ => (None, None),
        };
        for to in jump.into_iter().chain(branch) {
            if to <= at {
                loops.entry(to).or_default().push(at);
            }
        }
        match ends {
            Some(Ending::Return) => {}
            Some(Ending::Jump(to)) => {
                if to > at {
                    share[to] += share[at];
                }
            }
            Some(Ending::Branch(to)) => {
                if to > at {
                    share[to] += share[at] / 2;
                    share[at + 1] += share[at] - share[at] / 2;
                } else {
                    share[at + 1] += share[at];
                }
            }
            _ => share[at + 1] += share[at],
        }
    }
    let sizes = relaxed_sizes(items, &labels)?;
    let members: Vec<Vec<bool>> =
        loops.iter().map(|(&header, latches)| loop_members(&ends_of, header, latches)).collect();
    let (mut bytes, mut clocks) = (0usize, 0u64);
    let hoisted = mulsi3_registers(items);
    for (at, item) in items.iter().enumerate() {
        let Item::Inst(inst) = item else { continue };
        let size = sizes[at];
        let cost = crate::timing::cost(cpu, &mc::print(inst).ok()?, size as u32, false).ok()?.clocks
            + callee_clocks(inst, cpu, &hoisted);
        let depth = members.iter().filter(|member| member[at]).count().min(3);
        bytes += size;
        if depth == 0 && !cold_clocks {
            continue;
        }
        clocks += u64::from(cost) * share[at] * 10u64.pow(depth as u32);
    }
    // thousandths of a run, rounded to the nearest clock
    Some((bytes, u32::try_from((clocks + 500) / 1000).unwrap_or(u32::MAX)))
}

/// Whether `with` beats `without` under `price` on `cpu`; false when either
/// cannot be priced.
pub fn keeps(
    with: &[Item],
    without: &[Item],
    cpu: Cpu,
    price: Price,
) -> bool {
    match (price_of(with, cpu, true), price_of(without, cpu, true)) {
        (Some(with_cost), Some(without_cost)) => price.beats(cpu, with_cost, without_cost),
        _ => false,
    }
}

/// Whether `without` replaces `with`. The size and rate prices keep `with`
/// only where it beats `without` (a tie goes to `without`). The strict price
/// (-O1: no worse in bytes and in clocks) replaces `with` only where `without`
/// is no worse in both: where each wins one, `with` stands, so neither axis is
/// given up for the other. `cold_clocks` false credits clocks only inside loops
/// (a straight-line multiply may never run: size decides, as in the pin). False
/// when either cannot be priced.
pub fn prefers_without(
    with: &[Item],
    without: &[Item],
    cpu: Cpu,
    price: Price,
    cold_clocks: bool,
) -> bool {
    let (Some(with_cost), Some(without_cost)) = (price_of(with, cpu, cold_clocks), price_of(without, cpu, cold_clocks))
    else {
        return false;
    };
    match price {
        Price::Strict => without_cost.0 <= with_cost.0 && without_cost.1 <= with_cost.1,
        Price::Size | Price::Rate => !price.beats(cpu, with_cost, without_cost),
    }
}

/// `with` if it beats `without` under `price` on `cpu`, else `without`.
pub fn cheaper(
    with: Vec<Item>,
    without: Vec<Item>,
    cpu: Cpu,
    price: Price,
) -> Vec<Item> {
    match (price_of(&with, cpu, true), price_of(&without, cpu, true)) {
        (Some(with_cost), Some(without_cost)) if price.beats(cpu, with_cost, without_cost) => with,
        _ => without,
    }
}
