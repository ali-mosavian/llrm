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

/// Bytes, and clocks weighted by loop depth, of `items` on `cpu`.
fn price_of(
    items: &[Item],
    cpu: Cpu,
) -> Option<(usize, u32)> {
    let labels: HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .filter_map(|(at, item)| if let Item::Label(name) = item { Some((name.as_str(), at)) } else { None })
        .collect();
    // loops: [label, branch] for each branch back to a label
    let mut loops: Vec<(usize, usize)> = Vec::new();
    for (at, item) in items.iter().enumerate() {
        let Item::Inst(inst) = item else { continue };
        for operand in &inst.operands {
            if let mc::Operand::Symbol(name, 0) = operand {
                if let Some(&target) = labels.get(name.as_str()) {
                    if target <= at {
                        loops.push((target, at));
                    }
                }
            }
        }
    }
    let (mut bytes, mut clocks) = (0usize, 0u32);
    for (at, item) in items.iter().enumerate() {
        let Item::Inst(inst) = item else { continue };
        let size = crate::encode::encode(inst).ok()?.bytes.len();
        let cost = crate::timing::cost(cpu, &mc::print(inst).ok()?, size as u32, false).ok()?.clocks;
        let depth = loops.iter().filter(|(from, to)| *from <= at && at <= *to).count().min(3);
        bytes += size;
        clocks = clocks.saturating_add(cost.saturating_mul(10u32.pow(depth as u32)));
    }
    Some((bytes, clocks))
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
