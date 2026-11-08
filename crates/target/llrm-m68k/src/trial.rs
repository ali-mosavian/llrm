//! Two builds of one function, the cheaper kept.
//!
//! # How it works
//! [`cheaper`] prices both builds on the CPU: bytes as they are, clocks
//! weighted by loop depth (a backward branch to a label makes the lines
//! between a loop; each nesting multiplies by ten), then asks the level's
//! [`Price`] whether the first beats the second. A price the tables cannot
//! give keeps the second (the build without the optional pass).
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

/// Bytes, and clocks of `items` on `cpu` weighted by how often each runs: an
/// instruction counts the share of runs that reach it (a two-way branch splits
/// evenly) times ten per loop it sits in (a backward branch to a label makes
/// the lines between a loop).
fn price_of(
    items: &[Item],
    cpu: Cpu,
) -> Option<(usize, u32)> {
    let labels: HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .filter_map(|(at, item)| if let Item::Label(name) = item { Some((name.as_str(), at)) } else { None })
        .collect();
    let mut loops: Vec<(usize, usize)> = Vec::new();
    // the share of runs reaching each item, along forward edges only, in
    // thousandths
    let mut share = vec![0u64; items.len() + 1];
    share[0] = 1000;
    for (at, item) in items.iter().enumerate() {
        let Item::Inst(inst) = item else {
            // a label or a line mark passes the share on
            share[at + 1] += share[at];
            continue;
        };
        let ends = ending(inst, &labels);
        let (jump, branch) = match ends {
            Some(Ending::Jump(to)) => (Some(to), None),
            Some(Ending::Branch(to)) => (None, Some(to)),
            _ => (None, None),
        };
        for to in jump.into_iter().chain(branch) {
            if to <= at {
                loops.push((to, at));
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
    let (mut bytes, mut clocks) = (0usize, 0u64);
    for (at, item) in items.iter().enumerate() {
        let Item::Inst(inst) = item else { continue };
        let size = sizes[at];
        let cost = crate::timing::cost(cpu, &mc::print(inst).ok()?, size as u32, false).ok()?.clocks;
        let depth = loops.iter().filter(|(from, to)| *from <= at && at <= *to).count().min(3);
        bytes += size;
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
    match (price_of(with, cpu), price_of(without, cpu)) {
        (Some(with_cost), Some(without_cost)) => price.beats(cpu, with_cost, without_cost),
        _ => false,
    }
}

/// `with` if it beats `without` under `price` on `cpu`, else `without`.
pub fn cheaper(
    with: Vec<Item>,
    without: Vec<Item>,
    cpu: Cpu,
    price: Price,
) -> Vec<Item> {
    match (price_of(&with, cpu), price_of(&without, cpu)) {
        (Some(with_cost), Some(without_cost)) if price.beats(cpu, with_cost, without_cost) => with,
        _ => without,
    }
}
