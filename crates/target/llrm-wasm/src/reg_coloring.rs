//! `WebAssemblyRegColoring`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssemblyRegColoring.cpp)):
//! the registers that are left (those not on the value stack) share wasm
//! locals: a register takes the first existing one of its class whose
//! registers' live ranges it does not overlap.
//!
//! # How it works
//! Registers are sorted by spill weight, heaviest first, then by where their
//! range starts, and each takes the first color of the same class none of whose
//! members overlap it, or a color of its own. A register nothing reads is
//! skipped: `ExplicitLocals` drops its value.
//!
//! DEVIATE 1: the live ranges are computed here, per block over the current
//! layout, with two slots per instruction (it reads at the first, writes at the
//! second), and the block frequency that weights an operand is `32^depth` of
//! the loops containing the block (the pin's `MachineBlockFrequencyInfo` also
//! halves at branches). Exit: the shared live intervals and block frequency
//! (S4), no later than 2026-11-20.
//!
//! DEVIATE 2: a function that calls `setjmp` or another function that returns
//! twice is found by the callee's name, where the pin reads the attribute.
//! Exit: carry `returns_twice` from the module (2026-11-13).
//!
//! # See also
//! `WebAssemblyRegColoring.cpp`, pin d1106deb71cc: `regColoring`.

use std::collections::HashSet;

use crate::liveness::liveness;
use crate::mir::{Function, Operand, VReg};

/// A half-open run of slots.
type Segment = (u32, u32);

/// The functions whose return the pin's `exposesReturnsTwice` finds.
const RETURNS_TWICE: [&str; 6] = ["setjmp", "_setjmp", "sigsetjmp", "__sigsetjmp", "savectx", "getcontext"];

/// The live range of each register: its segments, ascending.
struct Ranges {
    segments: Vec<Vec<Segment>>,
    weight: Vec<f64>,
}

/// How many loops contain each block: back edges of a depth-first walk from
/// the entry, and the blocks that reach the edge's source without passing its
/// target.
fn loop_depths(f: &Function) -> Vec<u32> {
    let n = f.blocks.len();
    let mut preds = vec![Vec::new(); n];
    for (b, succs) in f.succs.iter().enumerate() {
        for &s in succs {
            preds[s].push(b);
        }
    }
    // 0 unvisited, 1 on the walk, 2 done.
    let mut state = vec![0u8; n];
    let mut back_edges = Vec::new();
    let mut stack = vec![(0usize, 0usize)];
    state[0] = 1;
    while let Some(&mut (b, ref mut next)) = stack.last_mut() {
        if let Some(&s) = f.succs[b].get(*next) {
            *next += 1;
            match state[s] {
                0 => {
                    state[s] = 1;
                    stack.push((s, 0));
                }
                1 => back_edges.push((b, s)),
                _ => {}
            }
        } else {
            state[b] = 2;
            stack.pop();
        }
    }
    let mut depth = vec![0u32; n];
    let mut headers: Vec<usize> = back_edges.iter().map(|&(_, header)| header).collect();
    headers.sort_unstable();
    headers.dedup();
    for header in headers {
        let mut body = HashSet::from([header]);
        let mut work: Vec<usize> = back_edges.iter().filter(|e| e.1 == header).map(|e| e.0).collect();
        while let Some(b) = work.pop() {
            if body.insert(b) {
                work.extend(preds[b].iter().copied());
            }
        }
        for b in body {
            depth[b] += 1;
        }
    }
    depth
}

fn ranges(f: &Function) -> Ranges {
    let n = f.vreg_count();
    let (_, live_out) = liveness(f);
    let depth = loop_depths(f);
    let mut segments: Vec<Vec<Segment>> = vec![Vec::new(); n];
    let mut weight = vec![0.0; n];
    let mut base = 0u32;
    let mut open: Vec<Option<u32>> = vec![None; n];
    let mut starts = Vec::with_capacity(f.blocks.len());
    for block in &f.blocks {
        starts.push(base);
        base += 2 * block.len() as u32 + 2;
    }
    for (b, block) in f.blocks.iter().enumerate() {
        let start = starts[b];
        let end = start + 2 * block.len() as u32;
        let frequency = 32f64.powi(depth[b] as i32);
        // Where each live register's range, going backwards, ends; `touched`
        // are the registers this block opened or defined.
        let mut touched: Vec<VReg> = live_out[b].iter().copied().collect();
        for &r in &live_out[b] {
            open[r as usize] = Some(end);
        }
        for (i, inst) in block.iter().enumerate().rev() {
            let write = start + 2 * i as u32 + 1;
            for operand in &inst.operands {
                if let Operand::Def(r) = *operand {
                    touched.push(r);
                    weight[r as usize] += frequency;
                    match open[r as usize].take() {
                        Some(last) => segments[r as usize].push((write, last)),
                        None => segments[r as usize].push((write, write + 1)),
                    }
                }
            }
            for operand in &inst.operands {
                if let Operand::Use(r) = *operand {
                    touched.push(r);
                    weight[r as usize] += frequency;
                    open[r as usize].get_or_insert(write);
                }
            }
        }
        for r in touched {
            if let Some(last) = open[r as usize].take() {
                segments[r as usize].push((start, last));
            }
        }
    }
    for list in &mut segments {
        list.sort_unstable();
    }
    Ranges { segments, weight }
}

/// Whether two ascending lists of segments share a slot.
fn overlaps(
    one: &[Segment],
    two: &[Segment],
) -> bool {
    let (mut a, mut b) = (0, 0);
    while a < one.len() && b < two.len() {
        if one[a].0 < two[b].1 && two[b].0 < one[a].1 {
            return true;
        }
        if one[a].1 <= two[b].1 {
            a += 1;
        } else {
            b += 1;
        }
    }
    false
}

/// Whether anything changed.
pub fn reg_coloring(f: &mut Function) -> bool {
    if f.blocks.iter().flatten().any(|inst| {
        inst.operands
            .iter()
            .any(
                |o| matches!(
                    o,
                    Operand::Sym(at) if RETURNS_TWICE.contains(&f.symbols[*at as usize].0.as_str())
                ),
            )
    }) {
        return false;
    }
    let uses = f.def_use();
    let range = ranges(f);
    let mut sorted: Vec<VReg> =
        (0..f.vreg_count() as VReg).filter(|&r| !f.is_stackified(r) && !uses.use_empty(r)).collect();
    // Heaviest first, then by where the range starts.
    sorted.sort_by(|&a, &b| {
        let (wa, wb) = (range.weight[a as usize], range.weight[b as usize]);
        wb.partial_cmp(&wa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(
                || {
                    let (sa, sb) = (&range.segments[a as usize], &range.segments[b as usize]);
                    match (sa.first(), sb.first()) {
                        (Some(x), Some(y)) => x.cmp(y),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                },
            )
    });
    // `colors[c]` is the register that names color `c`, `members[c]` its
    // ranges.
    let mut colors: Vec<VReg> = Vec::new();
    let mut members: Vec<Vec<VReg>> = Vec::new();
    let mut mapping: Vec<VReg> = (0..f.vreg_count() as VReg).collect();
    let mut changed = false;
    for &reg in &sorted {
        let ty = f.vreg_type(reg);
        let fits = colors
            .iter()
            .enumerate()
            .position(
                |(c, &named)| f.vreg_type(named) == ty
                    && members[c].iter().all(|&other| {
                        let theirs = &range.segments[other as usize];
                        theirs.is_empty() || !overlaps(theirs, &range.segments[reg as usize])
                    }),
            );
        let color = fits.unwrap_or_else(|| {
            colors.push(reg);
            members.push(Vec::new());
            colors.len() - 1
        });
        members[color].push(reg);
        mapping[reg as usize] = colors[color];
        changed |= colors[color] != reg;
    }
    if !changed {
        return false;
    }
    for operand in f.blocks.iter_mut().flatten().flat_map(|inst| inst.operands.iter_mut()) {
        match operand {
            Operand::Def(r) | Operand::Use(r) => *r = mapping[*r as usize],
            _ => {}
        }
    }
    true
}
