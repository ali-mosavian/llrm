//! The form of a run of one-bit shifts of a word on two cells, chosen at
//! emission by the bytes the text is left with.
//!
//! A run of `k` steps `first one; second two` is written three ways: in the
//! cells (`k` steps of 4 B), as a counted loop (9 B whatever `k`), or with one
//! of the two bytes in `a` (`lda b; (first/second)*k; sta b`: the byte that
//! arrives in `a` or leaves in it costs no load or store once the passes
//! drop the copy and the reload). Which is shorter depends on the copy before
//! and the reload after, so each form is cleaned as the later passes will
//! clean it and the shortest text is taken.
//!
//! The cleaned stretch ends where the block does: a cell the block's exit does
//! not read is dead there, which the function's liveness says exactly. A
//! stretch cut short by a label, a branch inside the block or the window
//! assumes every cell is read.
//!
//! The pin writes a shift by a constant as `k` instructions and leaves the
//! byte's register to the allocator: RAGreedy's copy hint keeps the shifted
//! byte where it arrives, and its class choice takes `a` or a cell. Here the
//! byte's home is chosen after allocation by the price of the text instead.

use std::collections::BTreeSet;

use crate::asm::{Asm, Inst, Line, Op, Operand};

/// Lines on each side of a run the cleaned stretch may reach: the cleanup
/// looks a few lines either way, and the work of a function stays in
/// proportion to its runs.
const WINDOW: usize = 40;

thread_local! {
    /// The lines of every candidate built on this thread: the work counter the
    /// scaling test reads (wall time flakes on a loaded host).
    pub static CANDIDATE_LINES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Where the shifted byte lives while the run goes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Form {
    /// `first one; second two`, `k` times, on the cells.
    Cells,
    /// `ldx #k; L: first one; second two; dex; bne L`.
    Loop,
    /// `lda held; ...; sta held` with `held` (`one` or `two`) as `a`.
    Held(bool),
}

/// An index register that may count a loop: its load and step, and the number
/// that keeps the loop's label its own.
#[derive(Clone, Copy, Debug)]
pub struct Counter {
    pub load: Op,
    pub step: Op,
    pub label: usize,
}

/// A run in the text, as the emitter wrote it.
#[derive(Clone, Debug)]
pub struct Site {
    pub block: usize,
    pub start: usize,
    pub end: usize,
    pub first: Op,
    pub second: Op,
    pub one: String,
    pub two: String,
    pub run: usize,
    pub counter: Option<Counter>,
    /// `a` is dead after the run, so it may carry a byte through it.
    pub a_dead: bool,
    pub place: u32,
    pub base: Form,
}

/// What follows a block: where its closing branches or returns begin and
/// where it ends in the text, and what its successors read.
#[derive(Clone, Debug, Default)]
pub struct Exit {
    pub terminators: usize,
    pub end: usize,
    pub cells: BTreeSet<String>,
    pub a: bool,
    pub x: bool,
    pub y: bool,
}

fn inst(
    op: Op,
    operand: Operand,
    place: u32,
) -> Line {
    Line::Inst(Inst { op, operand, place })
}

/// The lines of `form` for `site`.
pub fn lines_of(
    site: &Site,
    form: Form,
) -> Vec<Line> {
    let cell = |name: &String| Operand::Direct(name.clone());
    let place = site.place;
    let mut out = Vec::with_capacity(2 * site.run + 6);
    let (one, two) = match form {
        Form::Held(false) => (Operand::Acc, cell(&site.two)),
        Form::Held(true) => (cell(&site.one), Operand::Acc),
        _ => (cell(&site.one), cell(&site.two)),
    };
    let step = |out: &mut Vec<Line>| {
        out.push(inst(site.first, one.clone(), place));
        out.push(inst(site.second, two.clone(), place));
    };
    match (form, &site.counter) {
        (Form::Loop, Some(counter)) => {
            let label = format!("_sh{}", counter.label);
            out.push(inst(counter.load, Operand::Imm(site.run.to_string()), place));
            out.push(Line::Label(label.clone()));
            step(&mut out);
            out.push(inst(counter.step, Operand::None, place));
            out.push(inst(Op::Bne, Operand::Direct(label), place));
        }
        (Form::Held(second), _) => {
            let held = cell(if second { &site.two } else { &site.one });
            out.push(inst(Op::Lda, held.clone(), place));
            for _ in 0..site.run {
                step(&mut out);
            }
            out.push(inst(Op::Sta, held, place));
        }
        _ => {
            for _ in 0..site.run {
                step(&mut out);
            }
        }
    }
    out
}

fn boundary(line: &Line) -> bool {
    match line {
        Line::Inst(inst) => inst.target().is_some() || matches!(inst.op, Op::Rts | Op::Jmp),
        _ => true,
    }
}

fn bytes(lines: &[Line]) -> usize {
    lines.iter().filter_map(Line::inst).map(Inst::bytes).sum()
}

/// The clocks of `lines`, a loop's trips counted: only to break a tie in bytes.
fn clocks(
    lines: &[Line],
    cmos: bool,
    trips: usize,
) -> u64 {
    let cpu = if cmos { crate::dagisel::hooks::Cpu::W65c02 } else { crate::dagisel::hooks::Cpu::Mos6502 };
    let mut total = 0u64;
    let mut body = 0u64;
    let mut inside = false;
    for line in lines {
        match line {
            Line::Label(label) if label.starts_with("_sh") => inside = true,
            Line::Inst(inst) => {
                let cost = crate::timing::instruction(cpu, inst.op.name(), &inst.operand.to_string())
                    .map_or(0, |cost| u64::from(cost.cycles));
                total += cost;
                if inside {
                    body += cost + u64::from(inst.op == Op::Bne);
                }
                if inst.op == Op::Bne {
                    inside = false;
                }
            }
            _ => {}
        }
    }
    total + body * trips.saturating_sub(1) as u64
}

/// Picks the form of each run of `sites` (in text order, from the text `code`
/// holds) and writes it in. `exits` is indexed by block.
pub fn choose(
    code: &mut Asm,
    sites: &[Site],
    exits: &[Exit],
) {
    // the sites after the one at hand are already written: a block's end
    // moves by what they changed
    let mut grown = vec![0isize; exits.len()];
    for site in sites.iter().rev() {
        let exit = &exits[site.block];
        let shift = grown[site.block];
        let at = |position: usize| (position as isize + shift) as usize;
        let (terminators, block_end) = (at(exit.terminators), at(exit.end));
        let lines = &code.lines;
        let first = site.start.saturating_sub(WINDOW);
        let start = lines[first..site.start].iter().rposition(boundary).map_or(first, |i| first + i + 1);
        let limit = (site.end + WINDOW).min(block_end).min(lines.len());
        let found = lines[site.end..limit].iter().position(boundary).map(|i| site.end + i);
        let stop = found.unwrap_or(limit);
        // the stretch reaches the block's end, or the closing branches of it
        let exact = (found.is_some() || limit == block_end) && stop >= terminators;
        let ends_in_inst = found.is_some_and(|at| matches!(lines[at], Line::Inst(_)));
        let returns = ends_in_inst && matches!(&lines[stop], Line::Inst(Inst { op: Op::Rts, .. }));
        let tail_end = if ends_in_inst { stop + 1 } else { stop };
        let mut forms = vec![site.base];
        let mut offer = |form: Form| {
            if !forms.contains(&form) {
                forms.push(form);
            }
        };
        offer(Form::Cells);
        if site.counter.is_some() {
            offer(Form::Loop);
        }
        if site.a_dead {
            offer(Form::Held(false));
            offer(Form::Held(true));
        }
        let priced: Vec<(Form, Asm)> = forms
            .iter()
            .map(|&form| {
                let mut region = lines[start..site.start].to_vec();
                region.extend(lines_of(site, form));
                region.extend_from_slice(&lines[site.end..tail_end]);
                CANDIDATE_LINES.with(|work| work.set(work.get() + region.len()));
                let mut candidate = close(code, region, exit, exact, returns, ends_in_inst);
                clean(&mut candidate);
                (form, candidate)
            })
            .collect();
        let least = priced.iter().map(|(_, candidate)| bytes(&candidate.lines)).min().unwrap_or(0);
        let mut tied = priced.iter().filter(|(_, candidate)| bytes(&candidate.lines) == least);
        // the first of the shortest (the form written first, then the rest);
        // among several, the one that runs in fewer clocks
        let best = if tied.clone().count() == 1 {
            tied.next().map(|(form, _)| *form)
        } else {
            tied.min_by_key(|(form, candidate)| {
                clocks(&candidate.lines, code.cmos, if *form == Form::Loop { site.run } else { 1 })
            })
            .map(|(form, _)| *form)
        };
        let Some(form) = best else { continue };
        if form != site.base {
            let new = lines_of(site, form);
            grown[site.block] += new.len() as isize - (site.end - site.start) as isize;
            code.lines.splice(site.start..site.end, new);
        }
    }
}

/// `region` as an `Asm` that ends where its text ends: a return as written,
/// a branch with both ways read as the block's exit, a fall-through closed by
/// a return. A stretch cut inside the block is closed by a branch to nowhere,
/// which reads every cell and register.
fn close(
    code: &Asm,
    mut region: Vec<Line>,
    exit: &Exit,
    exact: bool,
    returns: bool,
    ends_in_inst: bool,
) -> Asm {
    let rts = || Line::Inst(Inst { op: Op::Rts, operand: Operand::None, place: 0 });
    if returns {
        return code.candidate_from(region);
    }
    let mut closed;
    if exact {
        if ends_in_inst {
            if let Some(Line::Inst(last)) = region.last_mut() {
                if last.op == Op::Jmp || last.op == Op::Bra {
                    last.op = Op::Rts;
                    last.operand = Operand::None;
                } else if last.target().is_some() {
                    last.operand = Operand::Direct("__region_exit".to_owned());
                    region.push(rts());
                    region.push(Line::Label("__region_exit".to_owned()));
                }
            }
        }
        region.push(rts());
        closed = code.candidate_from(region);
        closed.no_a_result = !exit.a;
        closed.no_x_result = !exit.x;
        closed.no_y_result = !exit.y;
        closed.result_cells = exit.cells.clone();
    } else {
        if !ends_in_inst {
            region.push(Line::Inst(Inst {
                op: Op::Bne,
                operand: Operand::Direct("__region_exit".to_owned()),
                place: 0,
            }));
        }
        let every: BTreeSet<String> = region
            .iter()
            .filter_map(|line| match line {
                Line::Inst(Inst { operand, .. }) => operand.name().map(str::to_owned),
                _ => None,
            })
            .collect();
        closed = code.candidate_from(region);
        closed.no_a_result = false;
        closed.no_x_result = false;
        closed.no_y_result = false;
        closed.result_cells = every;
    }
    closed
}

/// The cleanup the later passes give a candidate, in their order.
fn clean(candidate: &mut Asm) {
    crate::peephole_cells::chain_ends_in_a(candidate);
    crate::peephole_cells::forward_cell_copy(candidate);
    crate::peephole::drop_reloads(candidate);
    // a word that arrives in A:X is stored to its cells and read back
    // (`stx c; lda c`)
    if crate::copyprop::enabled(std::env::var_os("LLRM_6502_MCP").as_deref()) {
        crate::copyprop::copy_propagation(candidate);
    }
    crate::peephole_cells::drop_dead_stores(candidate);
    crate::peephole::drop_dead_writes(candidate);
    crate::peephole_cells::store_then_index_load(candidate);
}
