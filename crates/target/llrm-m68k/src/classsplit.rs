//! A value an address register would hold across a loop only to be copied into
//! one after it is held in a data register instead.
//!
//! # How it works
//! [`hold_in_data_registers`] runs after selection, before allocation. A value
//! with no address use of its own, read only by data operands and by copies and
//! phis into values that end in an address register, is the source of a join
//! the coalescer takes (the copy goes, the value is narrowed to an address
//! register). When it is live through a loop it is not used in, that costs a
//! register of that class there: with the scratch address registers busy in the
//! loop (`memcpy`: the source and the destination pointers), the value takes a
//! callee-saved one and the function saves and restores it. Held to a data
//! register, the copy stays a `move.l` after the loop and a scratch data
//! register holds the value. Whether it pays is decided by the function's two
//! builds ([`crate::trial`]).
//!
//! DEVIATE: the pin joins the copy whatever the loop in between
//! (`RegisterCoalescer`, `shouldCoalesce`, which M68k does not override), and
//! its own `llc` saves `%a2` here too; the bar is gcc, which holds such a value
//! in a data register across the loop. Owner: m68k. Exit: 2026-11-15, the
//! coalescer's join decision prices the registers a value occupies across a
//! loop, or this stays as the m68k value.

use llrm_spike_mf::regalloc::successors;
use llrm_spike_mf::{Kind, Operand, Reg};

use crate::mfunc::{Class, MFunction, operand_class};

/// Whether operand `position` of an instruction of `kind` names an address
/// register.
fn needs_address(
    kind: Kind,
    position: usize,
) -> bool {
    matches!(
        kind,
        Kind::Op(name) if operand_class(name, position) == Some(Class::Ar32)
    )
}

/// Whether operand `position` of an instruction of `kind` may be a data
/// register.
fn takes_data(
    kind: Kind,
    position: usize,
) -> bool {
    match kind {
        Kind::Copy => true,
        Kind::Op(name) => operand_class(name, position).is_some_and(|class| class.meet(Class::Dr32).is_some()),
    }
}

/// The number of values held to a data register.
pub fn hold_in_data_registers(function: &mut MFunction) -> usize {
    let count = function.classes.len();
    let blocks = function.blocks.len();
    let next = successors(function);
    // reach[a][b]: b follows a by one step or more
    let reach: Vec<Vec<bool>> = (0..blocks)
        .map(|start| {
            let mut seen = vec![false; blocks];
            let mut work = next[start].clone();
            while let Some(to) = work.pop() {
                if !std::mem::replace(&mut seen[to], true) {
                    work.extend(&next[to]);
                }
            }
            seen
        })
        .collect();
    let mut defs = vec![0u32; count];
    let mut phi_result = vec![false; count];
    let mut wants_address = vec![false; count];
    for block in &function.blocks {
        for phi in &block.phis {
            if let Reg::Virt(v) = phi.result {
                defs[v as usize] += 2;
                phi_result[v as usize] = true;
            }
        }
        for inst in &block.insts {
            for (position, operand) in inst.operands.iter().enumerate() {
                match operand {
                    Operand::Def(Reg::Virt(v)) => defs[*v as usize] += 1,
                    Operand::Use(Reg::Virt(v)) if needs_address(inst.kind, position) => {
                        wants_address[*v as usize] = true
                    }
                    _ => {}
                }
            }
        }
    }
    // values that end in an address register: they need one, or are read only
    // by copies and phis into such values
    let mut addressy: Vec<bool> = (0..count).map(|v| wants_address[v] || function.classes[v] == Class::Ar32).collect();
    loop {
        let mut reads = vec![0u32; count];
        let mut stray = vec![false; count];
        for block in &function.blocks {
            for phi in &block.phis {
                for (_, incoming) in &phi.incoming {
                    if let (Reg::Virt(from), Reg::Virt(to)) = (incoming, phi.result) {
                        reads[*from as usize] += 1;
                        stray[*from as usize] |= !addressy[to as usize];
                    }
                }
            }
            for inst in &block.insts {
                let into = match (inst.kind, &inst.operands[..]) {
                    (Kind::Copy, [Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(_))]) => Some(*to as usize),
                    _ => None,
                };
                for operand in &inst.operands {
                    if let Operand::Use(Reg::Virt(from)) = operand {
                        reads[*from as usize] += 1;
                        stray[*from as usize] |= into.is_none_or(|to| !addressy[to]);
                    }
                }
            }
        }
        let mut changed = false;
        for v in 0..count {
            if !addressy[v] && reads[v] > 0 && !stray[v] {
                addressy[v] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut held = 0;
    for v in 0..count as u32 {
        let at = v as usize;
        if defs[at] != 1 || phi_result[at] || wants_address[at] || function.classes[at] != Class::Xr32 {
            continue;
        }
        let mut used = vec![false; blocks];
        let mut defined = vec![false; blocks];
        let mut copies: Vec<usize> = Vec::new();
        let mut fits = true;
        for (b, block) in function.blocks.iter().enumerate() {
            for phi in &block.phis {
                for (from, incoming) in &phi.incoming {
                    if *incoming == Reg::Virt(v) {
                        used[*from] = true;
                        match phi.result {
                            Reg::Virt(to) if addressy[to as usize] => copies.push(*from),
                            _ => fits = false,
                        }
                    }
                }
            }
            for inst in &block.insts {
                for (position, operand) in inst.operands.iter().enumerate() {
                    match operand {
                        Operand::Def(Reg::Virt(d)) if *d as usize == at => {
                            defined[b] = true;
                            fits &= takes_data(inst.kind, position);
                        }
                        Operand::Use(Reg::Virt(u)) if *u as usize == at => {
                            used[b] = true;
                            match inst.kind {
                                Kind::Copy => {
                                    let into = inst
                                        .operands
                                        .iter()
                                        .find_map(
                                            |o| match o {
                                                Operand::Def(Reg::Virt(d)) => Some(*d as usize),
                                                _ => None,
                                            },
                                        );
                                    if into.is_some_and(|d| addressy[d]) {
                                        copies.push(b);
                                    } else {
                                        fits = false;
                                    }
                                }
                                Kind::Op(_) => fits &= takes_data(inst.kind, position),
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if !fits || copies.is_empty() {
            continue;
        }
        let mut live_in = vec![false; blocks];
        let mut live_out = vec![false; blocks];
        loop {
            let mut changed = false;
            for b in (0..blocks).rev() {
                let out = next[b].iter().any(|&to| live_in[to]);
                let live = (used[b] && !defined[b]) || (out && !defined[b]);
                if out != live_out[b] || live != live_in[b] {
                    live_out[b] = out;
                    live_in[b] = live;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // a loop block the value is live through and not used in, none of the
        // copies in its loop
        let through = (0..blocks).any(|b| {
            reach[b][b]
                && live_in[b]
                && live_out[b]
                && !used[b]
                && copies.iter().all(|&u| !(reach[u][b] && reach[b][u]))
        });
        if through {
            function.classes[at] = Class::Dr32;
            held += 1;
        }
    }
    held
}
