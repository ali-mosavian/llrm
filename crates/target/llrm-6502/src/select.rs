//! A selected function to ca65: each scheduled node becomes `MInst`s for
//! [`emit::expand`].
//!
//! # How it works
//! Every value gets a memory cell: `v_<fn>_<n>` for a byte, `w_<fn>_<n>` for a
//! word, defined in the BSS. A `CopyFromReg` copies a register into its cell, a
//! `CopyToReg` copies a cell into the register, a machine node reads and writes
//! cells, `EXTRACT_SUBREG` names a half of a word cell and `REG_SEQUENCE`
//! copies two bytes into one. The carry and the flags stay in the hardware
//! (`c`, `p`): the scheduler keeps them adjacent to their user.
//!
//! DEVIATE: no register allocator; every value is spilled. Reason: the spike
//! checks selection and emission, not allocation. Exit: the shared allocator
//! takes the 6502 (post-parity).

use std::fmt::Write;

use llrm_codegen::backend::frameobjects::Object;
use llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters;
use llrm_codegen::backend::selectiondag::function_lowering::FIRST_VIRTUAL;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{NodeData, SDValue};
use llrm_mir::context::GlobalId;
use llrm_mir::module::{BlockId, Module};
use llrm_support::hash::HashMap;

use crate::REGISTER_INFO;
use crate::alloc;
use crate::dagisel::function::{Selected, Selection};
use crate::dagisel::generated::ids::{opcode as op, reg as ids};
use crate::dagisel::generated::instrinfo;
use crate::dagisel::hooks::Cpu;
use crate::dagisel::pipeline::MosPins;
use crate::emit::{MInst, Operand, spill};
use crate::frame::{Function, Layout, layout};

const EXTRACT_SUBREG: u16 = 8;
const REG_SEQUENCE: u16 = 19;
const COPY: u16 = 20;
/// `RegisterClass` ids of the generated descriptions.
const CARRY: i16 = ids::CarryRegClassID as i16;
const FLAGS: i16 = ids::FlagsRegClassID as i16;
const IMAG16: i16 = ids::Imag16RegClassID as i16;

/// A line of a function before it is text: a label, a machine instruction, or
/// raw ca65.
#[derive(Clone, Debug)]
pub enum Item {
    Label(String),
    Inst(MInst),
    Text(String),
}

struct Cells<'a> {
    name: &'a str,
    next: usize,
    bytes: Vec<String>,
    words: Vec<String>,
    /// Virtual registers live across blocks, by number.
    virtual_regs: HashMap<u32, Operand>,
    /// The width in bits of every value's register: a phi input can be read on
    /// an edge laid out before the block that defines it (a loop's back
    /// edge).
    widths: HashMap<u32, u32>,
    /// The laid-out frame (`frame.rs`), and how many of its objects are the
    /// fixed ones.
    frame: Layout,
    fixed: usize,
    /// The byte cells that hold a spilled carry (`true`) or flags (`false`).
    saved: HashMap<String, bool>,
    /// Copies into `a`, `x` and `y`, held back to the `RET`: every other copy
    /// goes through `a`.
    deferred: Vec<MInst>,
    /// The unit being lowered ends in a call: its copies into `a`, `x` and `y`
    /// are arguments (`lower_call` puts them last) and stay in place.
    in_call: bool,
    /// The successors the current block branches to by label, for the edge
    /// blocks.
    branched_to: llrm_support::hash::HashSet<u32>,
}

impl Cells<'_> {
    fn byte(&mut self) -> Operand {
        self.next += 1;
        let cell = format!("v_{}_{}", self.name, self.next);
        self.bytes.push(cell.clone());
        Operand::Reg(cell)
    }

    fn word(&mut self) -> Operand {
        self.next += 1;
        let cell = format!("w_{}_{}", self.name, self.next);
        self.words.push(cell.clone());
        Operand::Reg(cell)
    }
}

fn block_label(
    name: &str,
    block: u32,
) -> String {
    format!("{name}_b{block}")
}

/// Whether a global of this linkage is visible to other objects: an internal
/// or private one is not, and a same-named helper in another object must not
/// collide with it at link time.
pub fn exported(linkage: llrm_mir::module::Linkage) -> bool {
    !matches!(
        linkage,
        llrm_mir::module::Linkage::Internal | llrm_mir::module::Linkage::Private
    )
}

/// The function `name` of `module` as ca65 text: its blocks, selected, on
/// virtual registers and allocated (`alloc.rs`), in layout order.
///
/// DEVIATE: a function in a cycle of calls keeps its spills on the soft stack;
/// its other values stay in registers or spill slots. Exit: none needed.
pub fn function_text(
    module: &Module,
    name: &str,
    selection: &Selection,
) -> Result<String, String> {
    // a call through a pointer no `!callees` bounds may come back into this
    // function
    let recursive = module
        .named(name)
        .is_some_and(
            |id| {
                let graph = llrm_mir::callgraph::CallGraph::new(module);
                graph.recursive(id) || graph.calls_unknown(id)
            },
        );
    let (items, _, places) = function_items_with_places(module, name, selection)?;
    let (mut function, labels) =
        alloc::build_with_places(name, &items, places).map_err(|why| format!("{name}: {}", why.0))?;
    function.returns_twice = module
        .named(name)
        .and_then(|id| module.global(id).function())
        .is_some_and(|one| llrm_mir::memory::calls_returns_twice(module, one));
    let (code, slots) = alloc::emit(
        function,
        &labels,
        recursive,
        selection.cpu == Cpu::W65c02,
        selection.zero_page,
        selection.tuning,
        selection.source.as_ref().map(|(file, line)| (file.as_str(), *line)),
    )?;
    let export = module.named(name).is_none_or(|id| exported(module.global(id).linkage));
    let mut body = format!("{}{code}", if export { format!(".export {name}\n") } else { String::new() });
    body.push_str(".segment \"BSS\"\n");
    for (slot, size) in slots {
        writeln!(body, "{slot}: .res {size}").unwrap();
    }
    body.push_str(".segment \"CODE\"\n");
    Ok(body)
}

/// The functions `parts` of `module` as ca65 text, in order. The callee-save
/// helpers are linked once and cost [`alloc::SAVE_HELPERS_BYTES`], so the
/// functions that would use them keep them only where together they save more
/// than that; otherwise each saves its cells inline. Break-even: `k` users of
/// `n = 3` cells (9 bytes each) pay from 4 users; one user of 8 cells (39
/// bytes) pays alone.
pub fn module_text(
    module: &Module,
    parts: &[(&str, &Selection)],
) -> Result<Vec<String>, String> {
    let mut texts =
        parts.iter().map(|(name, selection)| function_text(module, name, selection)).collect::<Result<Vec<_>, _>>()?;
    let cells = |text: &str| -> Option<u32> {
        let (before, _) = text.split_once("\n  jsr __csave")?;
        before.rsplit_once("ldy #")?.1.trim().parse().ok()
    };
    let users: Vec<(usize, u32)> = texts.iter().enumerate().filter_map(|(at, text)| Some((at, cells(text)?))).collect();
    let gain: i64 = users.iter().map(|(_, n)| alloc::shared_save_gain(*n)).sum();
    if !users.is_empty() && gain <= alloc::SAVE_HELPERS_BYTES {
        for (at, _) in users {
            texts[at] = alloc::with_shared_saves(false, || function_text(module, parts[at].0, parts[at].1))?;
        }
    }
    // the wide speed division is linked once and costs 141 bytes:
    // a module whose divisions are cold calls the speed routines with the
    // small wide path
    let wide = |text: &str| {
        ["__udivsi3", "__divsi3", "__umodsi3", "__modsi3"].iter().any(|name| text.contains(&format!(" {name}_fast\n")))
    };
    if texts.iter().any(|text| wide(text)) && !crate::hotdivision::hot(module) {
        for at in 0..texts.len() {
            if wide(&texts[at]) {
                texts[at] = alloc::with_fast_division(false, || function_text(module, parts[at].0, parts[at].1))?;
            }
        }
    }
    // at size the long divisions are the compact 32-step walk; the speed family
    // with the small wide path stops at the quotient's first bit: the
    // module's choice by the share of its clocks it saves
    let size = parts.first().is_some_and(|(_, selection)| selection.tuning == alloc::Tuning::SIZE);
    let compact_division = |text: &str| {
        ["__udivsi3", "__divsi3", "__umodsi3", "__modsi3"].iter().any(|name| text.contains(&format!(" {name}\n")))
    };
    if size && texts.iter().any(|text| compact_division(text)) {
        let mut faster = texts.clone();
        for at in 0..faster.len() {
            if compact_division(&faster[at]) {
                faster[at] = alloc::with_size_fast_division(true, || {
                    alloc::with_fast_division(false, || function_text(module, parts[at].0, parts[at].1))
                })?;
            }
        }
        if fast_division_pays_at_size(module, parts, &texts, &faster)? {
            texts = faster;
        }
    }
    // at speed the 16-bit divisions are the speed routine (31 bytes over the
    // compact one, linked once): a module takes the compact one, bytes first,
    // unless the clocks the speed one saves are worth them
    let speed_division16 = |text: &str| {
        ["__udivhi3", "__divhi3", "__umodhi3", "__modhi3"].iter().any(|name| text.contains(&format!(" {name}\n")))
    };
    if !size && texts.iter().any(|text| speed_division16(text)) {
        let mut compact = texts.clone();
        for at in 0..compact.len() {
            if speed_division16(&compact[at]) {
                compact[at] = alloc::with_compact_division16(true, || function_text(module, parts[at].0, parts[at].1))?;
            }
        }
        if !speed_division16_pays(module, parts, &texts, &compact)? {
            texts = compact;
        }
    }
    // the speed multiply is 19 bytes linked once: a module whose multiplies
    // are cold keeps the compact routine
    let calls_multiply = |text: &str| text.contains(" __mulsi3\n");
    if texts.iter().any(|text| calls_multiply(text))
        && crate::hotmultiply::hot(module)
        && (parts.first().is_none_or(|(_, selection)| selection.tuning != alloc::Tuning::SIZE)
            || fast_multiply_pays_at_size(module, parts, &texts)?)
    {
        for at in 0..texts.len() {
            if calls_multiply(&texts[at]) {
                texts[at] = alloc::with_fast_multiply(true, || function_text(module, parts[at].0, parts[at].1))?;
            }
        }
    }
    Ok(texts)
}

/// The function's lines in order, with the byte and word cells it made.
pub fn function_items(
    module: &Module,
    name: &str,
    selection: &Selection,
) -> Result<(Vec<Item>, (Vec<String>, Vec<String>)), String> {
    function_items_with_places(module, name, selection).map(|(items, cells, _)| (items, cells))
}

/// [`function_items`], with the source places the instructions'
/// [`MInst::origin`]s number from 1 (`SDLoc` to `DebugLoc`).
pub fn function_items_with_places(
    module: &Module,
    name: &str,
    selection: &Selection,
) -> Result<(Vec<Item>, (Vec<String>, Vec<String>), Vec<llrm_codegen::model::lir::SrcLoc>), String> {
    let mut places: Vec<llrm_codegen::model::lir::SrcLoc> = Vec::new();
    let mut number_of: HashMap<llrm_codegen::model::lir::SrcLoc, u32> = HashMap::default();
    let blocks = &selection.blocks;
    let (laid_out, fixed) = lay_out_frame(blocks, &selection.locals);
    let mut cells = Cells {
        name,
        next: 0,
        bytes: Vec::new(),
        words: Vec::new(),
        virtual_regs: HashMap::default(),
        widths: selection.widths.clone(),
        frame: laid_out,
        fixed,
        saved: HashMap::default(),
        deferred: Vec::new(),
        in_call: false,
        branched_to: Default::default(),
    };
    let mut body: Vec<Item> = vec![Item::Label(name.to_owned())];
    let mut edges: Vec<Item> = Vec::new();
    let mut prologue = Some(cells.frame.prologue());
    for one in blocks {
        let mut values: HashMap<(u32, u8), Operand> = HashMap::default();
        // the frame's stack adjust goes through `a`: an argument that arrives
        // in `a` or `x` is copied out before it, and the prologue is
        // then outside the first block
        for &(register, vreg) in &one.live_ins {
            let from = REGISTER_INFO.name(register).ok_or("a live-in with no name")?;
            if prologue.is_some() && ["a", "x"].contains(&from) {
                let cell = cell_for(&mut cells, 8);
                cells.virtual_regs.insert(vreg, cell.clone());
                body.push(Item::Inst(copy(8, cell, Operand::reg(from))));
            }
        }
        for inst in prologue.take().into_iter().flatten() {
            body.push(Item::Inst(inst));
        }
        body.push(Item::Label(block_label(name, one.block.0)));
        for &(register, vreg) in &one.live_ins {
            let from = REGISTER_INFO.name(register).ok_or("a live-in with no name")?;
            if cells.virtual_regs.contains_key(&vreg) {
                continue;
            }
            let bits = if from.starts_with("rs") { 16 } else { 8 };
            let cell = cell_for(&mut cells, bits);
            cells.virtual_regs.insert(vreg, cell.clone());
            body.push(Item::Inst(copy(bits, cell, Operand::reg(from))));
        }
        let mut phis_done = false;
        // A block that branches to several blocks puts each edge's copies on
        // the edge.
        let split = one.edges.iter().any(|edge| edge.to.is_some());
        cells.branched_to.clear();
        for unit in &one.units {
            cells.in_call = unit
                .nodes
                .iter()
                .any(
                    |&node| matches!(
                        one.dag.nodes()[node as usize].opcode,
                        ISD::Machine(code) if [op::JSR, op::JSRI].contains(&(code as u16))
                    ),
                );
            for &node in unit.nodes.iter().rev() {
                if matches!(
                    one.dag.nodes()[node as usize].opcode,
                    ISD::Machine(code) if code as u16 == op::RET
                ) {
                    for inst in cells.frame.epilogue() {
                        body.push(Item::Inst(inst));
                    }
                    for inst in std::mem::take(&mut cells.deferred) {
                        body.push(Item::Inst(inst));
                    }
                }
                if !phis_done && is_terminator(&one.dag.nodes()[node as usize].opcode) {
                    phis_done = true;
                    let ISD::Machine(code) = one.dag.nodes()[node as usize].opcode else { unreachable!() };
                    let conditional = ![op::JMP, op::BRA, op::RET].contains(&(code as u16));
                    if !split {
                        body.extend(
                            phi_copies(one, conditional, &mut cells)
                                .map_err(|why| format!("{why} (block {})", one.block.0))?,
                        );
                    }
                }
                let place = one.dag.lines.get(one.dag.nodes()[node as usize].order as usize).copied().flatten();
                let number = place.map(|place| {
                    *number_of
                        .entry(place)
                        .or_insert_with(
                            || {
                                places.push(place);
                                places.len() as u32
                            },
                        )
                });
                for mut inst in lower(module, name, one, node, &mut values, &mut cells)? {
                    if let (0, Some(number)) = (inst.origin, number) {
                        inst.origin = number;
                    }
                    body.push(Item::Inst(inst));
                }
            }
        }
        // An IR `unreachable` emits nothing in the pin (the machine block has
        // no successors); here a block with no terminator falls into the next
        // one, so the end is stated.
        let ends_unreachable = module.named(name).and_then(|id| module.global(id).function()).is_some_and(|function| {
            // a block a lowering made out of an IR block has no number there
            (one.block.0 as usize) < function.block_capacity()
                && function
                    .terminator(one.block)
                    .is_some_and(|last| function.instruction(last).opcode == llrm_mir::opcode::Opcode::Unreachable)
        });
        if ends_unreachable {
            body.push(Item::Inst(MInst::new(crate::emit::spill::UNREACHABLE, Vec::new())));
        }
        if split {
            // The edge taken by falling into the next block copies in line; the
            // others are blocks after the function.
            for edge in &one.edges {
                let Some(to) = edge.to else { continue };
                let copies = parallel_copy(&edge.copies, &mut cells)
                    .map_err(|why| format!("{why} (block {} to {})", one.block.0, to.0))?;
                if cells.branched_to.contains(&to.0) {
                    edges.push(Item::Label(edge_label(name, one.block.0, to.0)));
                    edges.extend(copies);
                    edges.push(Item::Inst(MInst::new(op::JMP, vec![Operand::Label(block_label(name, to.0))])));
                } else {
                    body.extend(copies);
                }
            }
        } else if !phis_done {
            // A `br` to the next block is dropped by layout: the block ends
            // with no terminator.
            body.extend(phi_copies(one, false, &mut cells).map_err(|why| format!("{why} (block {})", one.block.0))?);
        }
    }
    body.extend(edges);
    Ok((expand_selects(name, body), (cells.bytes, cells.words), places))
}

/// `EmitInstrWithCustomInserter` for `SELECT_CC`: every `SETCC8`, `CMOV8` and
/// `CMOV16` becomes a head that branches over a block holding the true arm's
/// move, so the blocks are real ones for everything after selection (the pin
/// splits the machine block at the pseudo into head, true and join blocks and
/// makes a phi of the two values,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp#L38957>).
///
/// The result is a cell of its own written on both edges, as the phi's copies
/// write it (the false arm before the branch, the true arm in the new block),
/// and copied to the select's result after the join: the pin's phi result is a
/// fresh register, and where the result goes (a memory cell, a loop-carried
/// register) is the coalescer's to join or not. With the result written in its
/// destination on both edges, a memory destination was stored twice
/// (`fletcher`: +4 B, +80 cy).
///
/// A constant an arm takes is loaded into the result in that arm: the pin's
/// phi copy of a constant is a `MOV32ri` made in the predecessor, and
/// `MachineSink` moves one into the block of the edge that reads it
/// (`machine_sink_in_place` keeps a rematerializable instruction where it is:
/// DEVIATE there). The load stays in its block where something else reads it.
///
/// DEVIATE 6502-SELECT-1: the expansion is on the item list, before the
/// builder makes the machine function, not on machine blocks. Reason: this
/// target has no shared machine function to split. Exit: the day the 6502
/// reaches the shared flow (`crate::dagisel` into `llrm_codegen`).
///
/// DEVIATE 6502-SELECT-2: for `BCC` and `BCS` the compare is made before the
/// head's move (`spill::COMPARE`), where the pin copies at the end of the
/// block, after the compare (`findPHICopyInsertPoint`,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/PHIElimination.cpp>).
/// For the other conditions the compare is redone at the branch, after the
/// move: a load or a transfer sets N and Z. Reason: the flags are not
/// allocated, so the carry alone survives a move. Exit: 2026-11-20, a
/// generated flags description (see `compare_before_step`).
///
/// DEVIATE 6502-SELECT-3: the new blocks' labels are `<fn>_cm<n>`, not
/// `_b<n>`: the loop depth of a block is read from the last `_b<n>` label
/// (`timing`), and a block made here is as deep as the block it was cut from.
/// Reason: the depth table is the IR's. Exit: a depth table of the machine
/// blocks.
fn expand_selects(
    name: &str,
    items: Vec<Item>,
) -> Vec<Item> {
    const BRANCHES: [u16; 6] = [op::BEQ, op::BNE, op::BCC, op::BCS, op::BMI, op::BPL];
    let sunk = sole_use_constants(&items);
    let mut out = Vec::with_capacity(items.len());
    let mut made = 0;
    for (at, item) in items.into_iter().enumerate() {
        let Item::Inst(inst) = &item else {
            out.push(item);
            continue;
        };
        if sunk.defs.contains(&at) {
            continue;
        }
        // (result, first value, second value, the condition under which the
        // second is taken, move)
        let o = &inst.operands;
        // the result is made in a cell of its own and copied to the select's
        // result after the join, as the pin's phi result is a fresh register
        // that the allocator may or may not join with where it goes
        let fresh = |prefix: &str| Operand::Reg(format!("{prefix}_sel{}_{name}", made + 1));
        let (moves, code, copied) = match (inst.opcode, o.as_slice()) {
            (op::SETCC8, [d, Operand::Imm(code), ..]) => {
                let s = fresh("v");
                let load = |value| MInst::new(op::LDI8, vec![s.clone(), Operand::Imm(value)]);
                ((load(1), load(0)), *code, copy(8, d.clone(), s))
            }
            (op::CMOV8, [d, f, t, Operand::Imm(code), ..]) => {
                let s = fresh("v");
                let arm = |v: &Operand| sunk.value(at, v, &s, op::LDI8, 8);
                ((arm(f), arm(t)), *code ^ 1, copy(8, d.clone(), s.clone()))
            }
            (op::CMOV16, [d, f, t, Operand::Imm(code), ..]) => {
                let s = fresh("w");
                let arm = |v: &Operand| sunk.value(at, v, &s, op::LDI16, 16);
                ((arm(f), arm(t)), *code ^ 1, copy(16, d.clone(), s.clone()))
            }
            _ => {
                out.push(item);
                continue;
            }
        };
        made += 1;
        let (join, arm) = (format!("{name}_cm{made}"), format!("{name}_cm{made}t"));
        let place = |mut m: MInst| {
            m.origin = inst.origin;
            Item::Inst(m)
        };
        // the head's move runs after the compare where the branch reads only C
        // (`BCC`, `BCS`): a load or a transfer sets N and Z, not C
        if matches!(BRANCHES[code as usize], op::BCC | op::BCS) {
            out.push(place(MInst::new(spill::COMPARE, vec![Operand::Imm(i64::from(code))])));
        }
        out.push(place(moves.0));
        out.push(place(MInst::new(BRANCHES[code as usize], vec![Operand::Label(join.clone())])));
        out.push(Item::Label(arm));
        out.push(place(moves.1));
        out.push(Item::Label(join));
        out.push(place(copied));
    }
    out
}

/// The constants (`LDI8`, `LDI16`) written once that a select reads.
struct SoleUse {
    /// The item positions of the constant definitions that move into an arm.
    defs: std::collections::HashSet<usize>,
    /// (select position, cell) -> the constant's value.
    uses: HashMap<(usize, String), i64>,
}

impl SoleUse {
    /// The arm's move into `d`: a load when `v` is such a constant, else a
    /// copy.
    fn value(
        &self,
        at: usize,
        v: &Operand,
        d: &Operand,
        load: u16,
        bits: u32,
    ) -> MInst {
        match v {
            Operand::Reg(name) => match self.uses.get(&(at, name.clone())) {
                Some(value) => MInst::new(load, vec![d.clone(), Operand::Imm(*value)]),
                None => copy(bits, d.clone(), v.clone()),
            },
            _ => copy(bits, d.clone(), v.clone()),
        }
    }
}

fn sole_use_constants(items: &[Item]) -> SoleUse {
    // occurrences of each cell, a word and its high half counted as one, and
    // how many instructions write it
    let base = |name: &str| name.strip_suffix("+1").unwrap_or(name).to_owned();
    let mut seen: HashMap<String, usize> = HashMap::default();
    let mut written: HashMap<String, usize> = HashMap::default();
    for item in items {
        let Item::Inst(inst) = item else { continue };
        for (at, o) in inst.operands.iter().enumerate() {
            if let Operand::Reg(name) = o {
                *seen.entry(base(name)).or_default() += 1;
                if at == 0 {
                    *written.entry(base(name)).or_default() += 1;
                }
            }
        }
    }
    let mut loads: HashMap<String, (usize, i64)> = HashMap::default();
    for (at, item) in items.iter().enumerate() {
        let Item::Inst(inst) = item else { continue };
        if let (op::LDI8 | op::LDI16, [Operand::Reg(d), Operand::Imm(value)]) = (inst.opcode, inst.operands.as_slice())
            && (d.starts_with("v_") || d.starts_with("w_"))
            && written[&base(d)] == 1
        {
            loads.insert(d.clone(), (at, *value));
        }
    }
    let mut sunk = SoleUse { defs: Default::default(), uses: HashMap::default() };
    for (at, item) in items.iter().enumerate() {
        let Item::Inst(inst) = item else { continue };
        if let (op::CMOV8 | op::CMOV16, [_, f, t, ..]) = (inst.opcode, inst.operands.as_slice()) {
            for v in [f, t] {
                if let Operand::Reg(name) = v
                    && let Some(&(def, value)) = loads.get(name)
                    && def < at
                {
                    // the load moves when the select is its only reader
                    if seen[&base(name)] == 2 {
                        sunk.defs.insert(def);
                    }
                    sunk.uses.insert((at, name.clone()), value);
                }
            }
        }
    }
    sunk
}

/// The frame: the fixed objects (the stack arguments, at the offsets the
/// calling hook gave them) then the locals, and the largest call sequence of
/// the function reserved at the bottom.
///
/// DEVIATE: the call area is reserved for the whole function
/// (`hasReservedCallFrame`), so the sequence pseudos emit nothing. Reason: the
/// spike has no dynamic allocas. Exit: with them.
fn lay_out_frame(
    blocks: &[Selected],
    locals: &[i64],
) -> (Layout, usize) {
    let mut offsets = std::collections::BTreeMap::new();
    for one in blocks {
        offsets.extend(one.dag.fixed_offsets.iter().map(|(index, offset)| ((-index - 1) as usize, *offset)));
    }
    let fixed = offsets.keys().max().map_or(0, |last| last + 1);
    let object = |size, offset| Object { size, align: 1, offset, callee_saved: false, dead: false };
    let mut objects: Vec<Object> = (0..fixed).map(|at| object(1, offsets.get(&at).copied().unwrap_or(0))).collect();
    objects.extend(locals.iter().map(|&size| object(size, 0)));
    let max_call_frame = blocks
        .iter()
        .flat_map(|one| one.dag.nodes().iter().map(move |node| (one, node)))
        .filter(|(_, node)| {
            matches!(
                node.opcode,
                ISD::Machine(code) if code as u16 == op::ADJCALLSTACKDOWN
            )
        })
        // the machine node's operands are the bytes pushed, the bytes popped,
        // then the chain
        .filter_map(|(one, node)| match one.dag.nodes()[node.ops[0].node as usize].data {
            NodeData::Const { bits, .. } => Some(bits as i64),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let function =
        Function { objects, fixed, max_call_frame, adjusts_stack: max_call_frame > 0, ..Function::default() };
    (layout(&function), fixed)
}

fn is_terminator(opcode: &ISD) -> bool {
    matches!(
        opcode,
        ISD::Machine(code) if [op::BEQ, op::BNE, op::BCC, op::BCS, op::BMI, op::BPL, op::JMP, op::BRA, op::RET].contains(&(*code as u16))
    )
}

/// The copies into the phis of the successors, placed before the block's first
/// terminator. A conditional branch reads the flags, which the copies' loads
/// change: they are saved around.
///
/// DEVIATE: see `dagisel::function`.
fn phi_copies(
    one: &Selected,
    conditional: bool,
    cells: &mut Cells,
) -> Result<Vec<Item>, String> {
    let copies: Vec<(u32, u32)> = one.edges.iter().flat_map(|edge| edge.copies.iter().copied()).collect();
    if copies.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    if conditional {
        out.push(Item::Text("  php\n".into()));
    }
    out.extend(parallel_copy(&copies, cells)?);
    if conditional {
        out.push(Item::Text("  plp\n".into()));
    }
    Ok(out)
}

/// `copies` (input register, phi's register) done at once: a copy whose
/// destination another still reads waits, and a cycle (a swap) is broken by
/// saving one old value in a cell.
fn parallel_copy(
    copies: &[(u32, u32)],
    cells: &mut Cells,
) -> Result<Vec<Item>, String> {
    let mut pending: Vec<(Operand, Operand, u32)> = Vec::new();
    for &(input, own) in copies {
        let from = match cells.virtual_regs.get(&input) {
            Some(cell) => cell.clone(),
            None => {
                let bits = *cells.widths.get(&input).ok_or_else(|| format!("phi input {input} is never named"))?;
                let cell = cell_for(cells, bits);
                cells.virtual_regs.insert(input, cell.clone());
                cell
            }
        };
        let Operand::Reg(name) = &from else { unreachable!() };
        let bits = if name.starts_with("w_") { 16 } else { 8 };
        let to = match cells.virtual_regs.get(&own) {
            Some(cell) => cell.clone(),
            None => {
                let cell = cell_for(cells, bits);
                cells.virtual_regs.insert(own, cell.clone());
                cell
            }
        };
        if from != to {
            pending.push((from, to, bits));
        }
    }
    let mut out = Vec::new();
    while !pending.is_empty() {
        let free = (0..pending.len())
            .find(|&at| !pending.iter().enumerate().any(|(other, p)| other != at && p.0 == pending[at].1));
        if let Some(at) = free {
            let (from, to, bits) = pending.remove(at);
            out.push(Item::Inst(copy(bits, to, from)));
        } else {
            let (_, to, bits) = pending[0].clone();
            let saved = cell_for(cells, bits);
            out.push(Item::Inst(copy(bits, saved.clone(), to.clone())));
            for p in pending.iter_mut().filter(|p| p.0 == to) {
                p.0 = saved.clone();
            }
        }
    }
    Ok(out)
}

fn edge_label(
    name: &str,
    from: u32,
    to: u32,
) -> String {
    format!("{name}_e{from}_{to}")
}

/// Where `value` is: a constant, a label, a register or a cell.
fn location(
    module: &Module,
    name: &str,
    one: &Selected,
    value: SDValue,
    values: &HashMap<(u32, u8), Operand>,
    cells: &mut Cells,
) -> Result<Operand, String> {
    let node = &one.dag.nodes()[value.node as usize];
    match node.data {
        NodeData::Const { bits, .. } => Ok(Operand::Imm(bits as i64)),
        NodeData::FrameIndex(index) => {
            let at = if index < 0 { (-index - 1) as usize } else { cells.fixed + index as usize };
            Ok(Operand::Imm(cells.frame.address(at)))
        }
        NodeData::Block(block) => {
            // A branch to a successor with phi copies from this block goes
            // through its edge block.
            let edge = one.edges.iter().find(|edge| edge.to == Some(BlockId(block)) && !edge.copies.is_empty());
            cells.branched_to.insert(block);
            Ok(Operand::Label(if edge.is_some() {
                edge_label(name, one.block.0, block)
            } else {
                block_label(name, block)
            }))
        }
        NodeData::Global { symbol, offset } => {
            let global = module.global(GlobalId(symbol));
            let text = global.name.as_deref().ok_or("an unnamed global")?;
            Ok(Operand::Label(if offset == 0 { text.to_owned() } else { format!("{text}{offset:+}") }))
        }
        NodeData::Symbol(id) => Ok(Operand::Label(one.dag.symbol_names[id as usize].clone())),
        NodeData::PhysReg(id) => Ok(Operand::Reg(REGISTER_INFO.name(id).ok_or("a register with no name")?.to_owned())),
        NodeData::Register(number) if number < FIRST_VIRTUAL => {
            let id = MosPins.phys_reg(number as u16).ok_or_else(|| format!("pin register {number} is not ours"))?;
            Ok(Operand::Reg(REGISTER_INFO.name(id).ok_or("a register with no name")?.to_owned()))
        }
        NodeData::Register(number) => {
            if let Some(found) = cells.virtual_regs.get(&number) {
                return Ok(found.clone());
            }
            let cell = if one.dag.value_type(SDValue { node: value.node, res: 0 }).size_in_bits() > 8 {
                cells.word()
            } else {
                cells.byte()
            };
            cells.virtual_regs.insert(number, cell.clone());
            Ok(cell)
        }
        _ => {
            values.get(&(value.node, value.res)).cloned().ok_or_else(|| format!("node {} has no value yet", value.node))
        }
    }
}

/// A byte or a word cell for a value of that type.
fn cell_for(
    cells: &mut Cells,
    bits: u32,
) -> Operand {
    if bits > 8 { cells.word() } else { cells.byte() }
}

fn copy(
    bits: u32,
    to: Operand,
    from: Operand,
) -> MInst {
    MInst::new(if bits > 8 { op::COPY16 } else { op::COPY8 }, vec![to, from])
}

fn lower(
    module: &Module,
    name: &str,
    one: &Selected,
    node: u32,
    values: &mut HashMap<(u32, u8), Operand>,
    cells: &mut Cells,
) -> Result<Vec<MInst>, String> {
    let n = &one.dag.nodes()[node as usize];
    let data_bits = |res: usize| n.values[res].size_in_bits();
    let at = |index: usize| n.ops[index];
    match n.opcode {
        // the frame objects share nothing here (no stack coloring), so a
        // lifetime marker says nothing
        ISD::TokenFactor | ISD::EntryToken | ISD::LifetimeStart | ISD::LifetimeEnd => Ok(Vec::new()),
        ISD::CopyFromReg => {
            let from = location(module, name, one, at(1), values, cells)?;
            let bits = data_bits(0);
            let cell = cell_for(cells, bits);
            values.insert((node, 0), cell.clone());
            Ok(vec![copy(bits, cell, from)])
        }
        ISD::CopyToReg => {
            let to = location(module, name, one, at(1), values, cells)?;
            let from = location(module, name, one, at(2), values, cells)?;
            let bits = one.dag.value_type(at(2)).size_in_bits();
            let copy = copy(bits, to.clone(), from);
            if !cells.in_call
                && matches!(
                    &to,
                    Operand::Reg(text) if ["a", "x", "y"].contains(&text.as_str())
                )
            {
                cells.deferred.push(copy);
                return Ok(Vec::new());
            }
            Ok(vec![copy])
        }
        ISD::Machine(code) => {
            let code = code as u16;
            match code {
                // `FREEZE` selects to a `COPY` of its operand
                COPY => {
                    let from = location(module, name, one, at(0), values, cells)?;
                    let bits = data_bits(0);
                    let cell = cell_for(cells, bits);
                    values.insert((node, 0), cell.clone());
                    Ok(vec![copy(bits, cell, from)])
                }
                EXTRACT_SUBREG => {
                    let word = location(module, name, one, at(0), values, cells)?;
                    let Operand::Reg(word) = word else { return Err("EXTRACT_SUBREG of a non-register".into()) };
                    let index = location(module, name, one, at(1), values, cells)?;
                    let Operand::Imm(index) = index else { return Err("EXTRACT_SUBREG without an index".into()) };
                    let (low, high) = match word.strip_prefix("rs") {
                        Some(pair) => {
                            let pair: usize = pair.parse().map_err(|_| "a bad pair register")?;
                            (format!("rc{}", 2 * pair), format!("rc{}", 2 * pair + 1))
                        }
                        None => (word.clone(), format!("{word}+1")),
                    };
                    let low = if index == i64::from(ids::sub_lo) { low } else { high };
                    values.insert((node, 0), Operand::Reg(low));
                    Ok(Vec::new())
                }
                REG_SEQUENCE => {
                    let (low, high) = (at(1), at(3));
                    let (low, high) = (
                        location(module, name, one, low, values, cells)?,
                        location(module, name, one, high, values, cells)?,
                    );
                    let cell = cells.word();
                    let Operand::Reg(word) = &cell else { unreachable!() };
                    values.insert((node, 0), cell.clone());
                    Ok(vec![
                        MInst::new(op::COPY8, vec![Operand::Reg(word.clone()), low]),
                        MInst::new(op::COPY8, vec![Operand::Reg(format!("{word}+1")), high]),
                    ])
                }
                op::ADJCALLSTACKDOWN | op::ADJCALLSTACKUP => match location(module, name, one, at(1), values, cells)? {
                    Operand::Imm(0) => Ok(Vec::new()),
                    _ => Err("a call with stack arguments needs frame.rs".into()),
                },
                _ => {
                    let desc = instrinfo::desc(code).ok_or_else(|| format!("no description of opcode {code}"))?;
                    let mut operands = Vec::new();
                    let (mut before, mut after) = (Vec::new(), Vec::new());
                    for (index, out) in desc.operands[..usize::from(desc.num_defs)].iter().enumerate() {
                        let cell = match out.reg_class {
                            CARRY | FLAGS => {
                                let cell = cells.byte();
                                let Operand::Reg(text) = &cell else { unreachable!() };
                                cells.saved.insert(text.clone(), out.reg_class == CARRY);
                                after.push(MInst::new(
                                    if out.reg_class == CARRY { spill::SAVE_CARRY } else { spill::SAVE_FLAGS },
                                    vec![cell.clone()],
                                ));
                                values.insert((node, index as u8), cell.clone());
                                operands.push(Operand::reg(if out.reg_class == CARRY { "c" } else { "p" }));
                                continue;
                            }
                            IMAG16 => cells.word(),
                            _ => cells.byte(),
                        };
                        values.insert((node, index as u8), cell.clone());
                        operands.push(cell);
                    }
                    let wanted = usize::from(desc.num_operands) - usize::from(desc.num_defs);
                    for &input in n.ops.iter() {
                        let ty = one.dag.value_type(input);
                        let plumbing = (ty == EVT::OTHER || ty == EVT::GLUE)
                            && matches!(one.dag.nodes()[input.node as usize].data, NodeData::None);
                        if plumbing || operands.len() == usize::from(desc.num_operands) {
                            continue;
                        }
                        let at = location(module, name, one, input, values, cells)?;
                        match &at {
                            // a carry read as a value (a compare of two bits)
                            // is the saved byte, 0 or 1; only a carry operand
                            // is loaded back into C
                            Operand::Reg(text)
                                if cells.saved.contains_key(text)
                                    && !(cells.saved[text]
                                        && desc.operands.get(operands.len()).is_some_and(|o| o.reg_class != CARRY)) =>
                            {
                                let carry = cells.saved[text];
                                before.push(MInst::new(
                                    if carry { spill::LOAD_CARRY } else { spill::LOAD_FLAGS },
                                    vec![at.clone()],
                                ));
                                operands.push(Operand::reg(if carry { "c" } else { "p" }));
                            }
                            _ => operands.push(at),
                        }
                    }
                    if operands.len() - usize::from(desc.num_defs) != wanted {
                        return Err(format!(
                            "opcode {code}: {} operands, {} wanted",
                            operands.len(),
                            desc.num_operands
                        ));
                    }
                    let mut all = before;
                    // A frame object in the pointer of a memory access is `rs0`
                    // plus its distance, where one `(zp),Y` offset reaches;
                    // past 255 the address is built (`eliminateFrameIndex`).
                    let at = if code == op::STZP { 0 } else { operands.len().wrapping_sub(2) };
                    let framed = (matches!(code, op::LDP8 | op::LDP16 | op::STP8 | op::STP16 | op::STZP)
                        || crate::pointer_alu::pointer_alu(code).is_some_and(|fold| !fold.indexed))
                        && n.ops
                            .iter()
                            .any(|input| one.dag.nodes()[input.node as usize].opcode == ISD::TargetFrameIndex);
                    if framed {
                        if let (Some(Operand::Imm(base)), Some(Operand::Imm(extra))) =
                            (operands.get(at).cloned(), operands.get(at + 1).cloned())
                        {
                            let distance = base + extra;
                            if (0..=255).contains(&distance) {
                                operands[at] = Operand::reg("rs0");
                                operands[at + 1] = Operand::Imm(distance);
                            } else {
                                let pointer = cells.word();
                                all.push(MInst::new(op::LEASP16, vec![pointer.clone(), Operand::Imm(distance)]));
                                operands[at] = pointer;
                                operands[at + 1] = Operand::Imm(0);
                            }
                        }
                    }
                    all.push(MInst::new(code, operands));
                    all.extend(after);
                    Ok(all)
                }
            }
        }
        other => Err(format!("{other:?} is not a machine node")),
    }
}

/// At size the speed multiply (`hotmultiply::FAST_BYTES` more, linked once) is
/// the module's choice by the share of its clocks it saves against the share of
/// its bytes it adds (`mulimage::pays_by_share`): the static runs of its
/// multiplies times what a call saves, against the module priced as it stands.
///
/// DEVIATE 6502-MUL-OS (owner llrm-target-6502, exit 2026-11-24): the pin's
/// `__mulsi3` is one routine. mul_constants -Os ran 200-trip loops of the
/// 32-trip compact walk, 42.0M clocks against llvm-mos's 13.6M.
fn fast_multiply_pays_at_size(
    module: &Module,
    parts: &[(&str, &Selection)],
    texts: &[String],
) -> Result<bool, String> {
    let (clocks, bytes) = priced(module, parts, texts)?;
    let saved = (crate::hotmultiply::runs(module) * crate::hotmultiply::SAVED_CLOCKS) as u64;
    Ok(crate::mulimage::pays_by_share((saved, crate::hotmultiply::FAST_BYTES as i64), clocks, bytes))
}

/// The module as it stands, in estimated clocks and bytes
/// (`weights::run_cost`).
fn priced(
    module: &Module,
    parts: &[(&str, &Selection)],
    texts: &[String],
) -> Result<(u64, u64), String> {
    let Some((_, first)) = parts.first() else { return Ok((0, 0)) };
    let layout =
        llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().ok_or("a module with no datalayout")?)?;
    let names: Vec<String> = parts.iter().map(|(name, _)| (*name).to_owned()).collect();
    let weights: Vec<Vec<u64>> = names
        .iter()
        .map(|name| {
            let function = module.named(name).and_then(|id| module.global(id).function());
            function.map_or_else(Vec::new, |function| crate::weights::block_weights(module, function, &layout))
        })
        .collect();
    crate::weights::run_cost(first.cpu, &names, texts, &weights)
}

/// Whether a module at speed keeps the speed 16-bit divisions: the share of its
/// clocks they save is at least `[share] percent_per_percent` times the share
/// of its bytes they add (the image they link).
///
/// DEVIATE 6502-DIV-16 (owner llrm-target-6502, exit 2026-11-24): the pin's
/// `__udivhi3` is one routine. Reason: bytes first on this target at every
/// level (user ruling 2026-11-14); the compact routine is 31 bytes smaller and
/// 215 clocks slower a call on average (`hotdivision::SAVED_AT_SPEED_16`).
fn speed_division16_pays(
    module: &Module,
    parts: &[(&str, &Selection)],
    speed: &[String],
    compact: &[String],
) -> Result<bool, String> {
    let names: Vec<String> = parts.iter().map(|(name, _)| (*name).to_owned()).collect();
    let growth = crate::mulimage::image_bytes(&crate::mulimage::image_of_texts(&names, speed))
        - crate::mulimage::image_bytes(&crate::mulimage::image_of_texts(&names, compact));
    let (clocks, bytes) = priced(module, parts, compact)?;
    // the priced clocks leave out the routine's own run
    let runs = crate::hotdivision::runs16(module);
    let clocks = clocks + (runs * crate::hotdivision::CLOCKS_COMPACT_16) as u64;
    let saved = (runs * crate::hotdivision::SAVED_AT_SPEED_16) as u64;
    Ok(crate::mulimage::pays_by_share((saved, growth), clocks, bytes))
}

/// At size the speed long divisions (the small wide path, bytes by the image
/// they link) are the module's choice by the share of its clocks they save
/// against the share of its bytes they add.
///
/// DEVIATE 6502-DIV-OS (owner llrm-target-6502, exit 2026-11-24): the pin's
/// `__udivXi3` is one routine, and it starts at the quotient's first bit.
/// loop_interchange -Os ran 13.9M clocks against llvm-mos's 3.3M.
fn fast_division_pays_at_size(
    module: &Module,
    parts: &[(&str, &Selection)],
    compact: &[String],
    faster: &[String],
) -> Result<bool, String> {
    let names: Vec<String> = parts.iter().map(|(name, _)| (*name).to_owned()).collect();
    let growth = crate::mulimage::image_bytes(&crate::mulimage::image_of_texts(&names, faster))
        - crate::mulimage::image_bytes(&crate::mulimage::image_of_texts(&names, compact));
    let (clocks, bytes) = priced(module, parts, compact)?;
    let saved = (crate::hotdivision::runs(module) * crate::hotdivision::SAVED_AT_SIZE) as u64;
    Ok(crate::mulimage::pays_by_share((saved, growth), clocks, bytes))
}
