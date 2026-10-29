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

use std::collections::HashMap;
use std::fmt::Write;

use llrm_codegen::backend::frameobjects::Object;
use llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters;
use llrm_codegen::backend::selectiondag::function_lowering::FIRST_VIRTUAL;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{NodeData, SDValue};
use llrm_mir::context::GlobalId;
use crate::dagisel::hooks::Cpu;
use llrm_mir::module::{BlockId, Module};

use crate::REGISTER_INFO;
use crate::alloc;
use crate::dagisel::function::{Selected, Selection};
use crate::dagisel::generated::ids::{opcode as op, reg as ids};
use crate::dagisel::generated::instrinfo;
use crate::dagisel::pipeline::MosPins;
use crate::emit::{MInst, Operand, spill};
use crate::frame::{Function, Layout, layout};

const EXTRACT_SUBREG: u16 = 8;
const REG_SEQUENCE: u16 = 19;
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
    /// The width in bits of every value's register: a phi input can be read on an edge laid out
    /// before the block that defines it (a loop's back edge).
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
    /// The successors the current block branches to by label, for the edge
    /// blocks.
    branched_to: std::collections::HashSet<u32>,
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
    // a call through a pointer no `!callees` bounds may come back into this function
    let recursive = module.named(name).is_some_and(|id| {
        let graph = llrm_mir::callgraph::CallGraph::new(module);
        graph.recursive(id) || graph.calls_unknown(id)
    });
    let (items, _) = function_items(module, name, selection)?;
    let (function, labels) = alloc::build(name, &items).map_err(|why| format!("{name}: {}", why.0))?;
    let (code, slots) = alloc::emit(function, &labels, recursive, selection.cpu == Cpu::W65c02, selection.zero_page, selection.tuning)?;
    let mut body = format!(".export {name}\n{code}");
    body.push_str(".segment \"BSS\"\n");
    for (slot, size) in slots {
        writeln!(body, "{slot}: .res {size}").unwrap();
    }
    body.push_str(".segment \"CODE\"\n");
    Ok(body)
}

/// The function's lines in order, with the byte and word cells it made.
pub fn function_items(
    module: &Module,
    name: &str,
    selection: &Selection,
) -> Result<(Vec<Item>, (Vec<String>, Vec<String>)), String> {
    let blocks = &selection.blocks;
    let (laid_out, fixed) = lay_out_frame(blocks, &selection.locals);
    let mut cells = Cells {
        name,
        next: 0,
        bytes: Vec::new(),
        words: Vec::new(),
        virtual_regs: HashMap::new(),
        widths: selection.widths.clone(),
        frame: laid_out,
        fixed,
        saved: HashMap::new(),
        deferred: Vec::new(),
        branched_to: Default::default(),
    };
    let mut body: Vec<Item> = vec![Item::Label(name.to_owned())];
    let mut edges: Vec<Item> = Vec::new();
    for inst in cells.frame.prologue() {
        body.push(Item::Inst(inst));
    }
    for one in blocks {
        body.push(Item::Label(block_label(name, one.block.0)));
        let mut values: HashMap<(u32, u8), Operand> = HashMap::new();
        for &(register, vreg) in &one.live_ins {
            let from = REGISTER_INFO.name(register).ok_or("a live-in with no name")?;
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
                        body.extend(phi_copies(one, conditional, &mut cells).map_err(|why| format!("{why} (block {})", one.block.0))?);
                    }
                }
                for inst in lower(module, name, one, node, &mut values, &mut cells)? {
                    body.push(Item::Inst(inst));
                }
            }
        }
        if split {
            // The edge taken by falling into the next block copies in line; the
            // others are blocks after the function.
            for edge in &one.edges {
                let Some(to) = edge.to else { continue };
                let copies = parallel_copy(&edge.copies, &mut cells).map_err(|why| format!("{why} (block {} to {})", one.block.0, to.0))?;
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
    Ok((body, (cells.bytes, cells.words)))
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
        .filter_map(|(one, node)| match one.dag.nodes()[node.ops[1].node as usize].data {
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
        ISD::TokenFactor | ISD::EntryToken => Ok(Vec::new()),
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
            if matches!(
                &to,
                Operand::Reg(text) if ["a", "x", "y"].contains(&text.as_str())
            ) {
                cells.deferred.push(copy);
                return Ok(Vec::new());
            }
            Ok(vec![copy])
        }
        ISD::Machine(code) => {
            let code = code as u16;
            match code {
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
                            Operand::Reg(text) if cells.saved.contains_key(text) => {
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
                    all.push(MInst::new(code, operands));
                    all.extend(after);
                    Ok(all)
                }
            }
        }
        other => Err(format!("{other:?} is not a machine node")),
    }
}
