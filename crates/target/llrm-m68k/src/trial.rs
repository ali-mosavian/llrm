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
            Item::WordTable(labels) => Some(2 * labels.len()),
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
        let cost = crate::instprice::clocks_of(cpu, inst, size as u32).ok()?
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

/// Bytes and clocks of `functions` (name, items): each function's clocks run once per call from the others, by the
/// number of call instructions that name it (one for a function nothing here calls). A callee priced once for four
/// calls looked cheaper than its four copies by three runs of its body (`pick`: 326 clk against 1080, measured 8606
/// against 7704 as a program).
fn price_of_functions(
    functions: &[(String, Vec<Item>)],
    cpu: Cpu,
) -> Option<(usize, u32)> {
    let priced: Vec<(usize, u32)> = functions.iter().map(|(_, items)| price_of(items, cpu, true)).collect::<Option<_>>()?;
    let calls = |from: &[Item], to: &str| -> u64 {
        from.iter()
            .filter(|item| matches!(item, Item::Inst(inst) if (inst.opcode.starts_with("CALL") || inst.opcode.starts_with("BSR"))
                && inst.operands.iter().any(|operand| matches!(operand, mc::Operand::Symbol(name, 0) if name == to))))
            .count() as u64
    };
    let mut weight = vec![1u64; functions.len()];
    // a call chain is as long as the functions here; a cycle stops at the cap
    for _ in 0..functions.len() {
        let next: Vec<u64> = functions
            .iter()
            .map(|(name, _)| {
                let runs: u64 = functions.iter().zip(&weight).filter(|((from, _), _)| from != name).map(|((_, items), w)| w * calls(items, name)).sum();
                runs.clamp(1, 1000)
            })
            .collect();
        if next == weight {
            break;
        }
        weight = next;
    }
    let bytes = priced.iter().map(|(bytes, _)| bytes).sum();
    let clocks = priced.iter().zip(&weight).map(|((_, clocks), w)| u64::from(*clocks) * w).sum::<u64>();
    Some((bytes, u32::try_from(clocks).unwrap_or(u32::MAX)))
}

/// Whether any of `functions` calls `__mulsi3` (by name or through the register the call-target hoist loaded).
pub fn calls_mulsi3(functions: &[(String, Vec<Item>)]) -> bool {
    functions.iter().any(|(_, items)| {
        items.iter().any(|item| {
            matches!(item, Item::Inst(inst) if (inst.opcode.starts_with("CALL") || inst.opcode == "LEA32b")
                && inst.operands.iter().any(|operand| matches!(operand, mc::Operand::Symbol(name, 0) if name == "__mulsi3")))
        })
    })
}

/// The bytes `with` may have in [`keeps_within_headroom`], in percent of `without`'s.
pub const HEADROOM_PERCENT: usize = 150;

/// DEVIATE OS-INLINE-TRIAL-CLOCKS: whether `with` is the build to keep at -Os, which counts bytes and clocks against
/// the best compiler's. See [`pays`].
///
pub fn keeps_within_headroom(
    with: &[(String, Vec<Item>)],
    without: &[(String, Vec<Item>)],
    rest_calls_mulsi3: impl FnOnce() -> bool,
    cpu: Cpu,
) -> bool {
    let (Some((with_bytes, with_clocks)), Some((bytes, clocks))) = (price_of_functions(with, cpu), price_of_functions(without, cpu)) else {
        return false;
    };
    let (links_with, links_without) = (calls_mulsi3(with), calls_mulsi3(without));
    let routine = if links_with != links_without && !rest_calls_mulsi3() { crate::routine::mulsi3_bytes(cpu) } else { 0 };
    let (with_bytes, bytes) = if links_with { (with_bytes + routine, bytes) } else { (with_bytes, bytes + routine) };
    if llrm_support::debug::enabled("trial") {
        eprintln!("trial headroom: with {with_bytes} B {with_clocks} clk, without {bytes} B {clocks} clk");
    }
    pays((with_bytes, with_clocks), (bytes, clocks))
}

/// `with` (bytes, clocks) against `without`: fewer bytes (clocks then), or more bytes only while they stay within
/// [`HEADROOM_PERCENT`] of `without`'s and the clocks fall by at least 1.5 times the share the bytes grow by (the bytes are exact, the clocks estimated). Measured at -Os
/// 68000 with the ceiling alone: `loop_versioning` +43 % bytes for -10 % clocks went over gcc's bytes bar, and so did
/// `loop_interchange` (+23 % for -5 %); `u64_shift_select_arms` (+28 % for -47 %) is under it.
fn pays(
    with: (usize, u32),
    without: (usize, u32),
) -> bool {
    let ((with_bytes, with_clocks), (bytes, clocks)) = (with, without);
    if (with_bytes, with_clocks) < (bytes, clocks) {
        return true;
    }
    let grown = (with_bytes.saturating_sub(bytes)) as u64;
    let saved = u64::from(clocks.saturating_sub(with_clocks));
    with_bytes * 100 <= bytes * HEADROOM_PERCENT && with_clocks < clocks && grown * u64::from(clocks) * 3 <= saved * bytes as u64 * 2
}

#[cfg(test)]
mod pays_tests {
    use super::pays;

    /// The -Os trial priced these pairs (`LLRM_DEBUG=trial`, bytes and weighted clocks of the differing functions).
    /// With the ceiling alone `loop_versioning`, `loop_interchange` and `stack_arrays_copy` were kept and then sat
    /// over the best compiler's bytes at 68000 (242 -> 346 B against 288).
    #[test]
    fn growth_must_be_paid_for_by_as_large_a_share_of_clocks() {
        assert!(pays((302, 820), (236, 1556)), "u64_shift_select_arms: +28 % bytes for -47 % clocks");
        assert!(pays((520, 4092), (512, 4440)), "copy_runs: +1.6 % for -8 %");
        assert!(!pays((346, 9092), (242, 10130)), "loop_versioning: +43 % for -10 %");
        assert!(!pays((358, 52284), (290, 55054)), "loop_interchange: +23 % for -5 %");
        assert!(!pays((174, 1414), (100, 1606)), "stack_arrays_copy with __mulsi3: +74 % for -12 %");
        assert!(!pays((108, 649), (98, 730)), "stack_arrays_copy 68020: +10 % for -11 % went 2 B over gcc's bar");
        assert!(pays((90, 900), (100, 1000)), "smaller and faster");
        assert!(!pays((100, 1000), (100, 1000)), "a tie goes to the plain build");
    }
}

/// Whether `without` replaces `with`: `with` stays only where it beats
/// `without` (a tie goes to `without`). `cold_clocks` false credits clocks only inside loops
/// (a straight-line multiply may never run: size decides, as in the pin). False
/// when either cannot be priced.
pub fn prefers_without(
    with: &[Item],
    without: &[Item],
    cpu: Cpu,
    price: Price,
    cold_clocks: bool,
) -> bool {
    let (Some(with_cost), Some(without_cost)) = (price_of(with, cpu, cold_clocks), price_of(without, cpu, cold_clocks)) else {
        return false;
    };
    !price.beats(cpu, with_cost, without_cost)
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
