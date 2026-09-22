//! The raise: `qbopt/model/mir.py` from `_RaisedOp` through `bodies`.
//!
//! Python keywords map to plain Rust: `detached(op, **changes)` is the caller
//! applying `changes` to a clone and handing it here.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use iced_x86::{Code, Register};

use super::{
    Arg, Cell, Const, FLAGS, FrameAddress, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, RaisedBody, Raising,
    Reach, Symbol, Synth, TRACKED, Value,
};
use crate::abi::runtime;
use crate::analysis::loops;
use std::rc::Rc;

use super::AllocationHints;
use crate::analysis::regions;
use crate::frontend::blocks::{self as split, Block};
use crate::frontend::extent::BodyKind;
use crate::frontend::{
    raising_addresses, raising_array_access, raising_arrays, raising_bytes, raising_call_memory, raising_calls,
    raising_carried, raising_conditions, raising_control, raising_copies, raising_defseg, raising_dispatch,
    raising_division, raising_float_calls, raising_float_results, raising_float_values, raising_floats, raising_frame,
    raising_literals, raising_longs, raising_numeric_policy, raising_returns, raising_words, stack,
};
use crate::model::ir::decode;
use crate::objectfile::cvinfo;
use crate::objectfile::module::{self, SourceMap};
use crate::legacy::calls;
use crate::model::ir::nodes::{Node, span};
use crate::model::ir::{Loc, Operation, ROOT, Semantics, root};
use crate::objectfile::module::{Addr, Module, Space};
use crate::support::hash::IndexMap;


/// Python `PHYSICAL`: the frame, the stack and the segment registers.
pub const PHYSICAL: [Register; 8] = [
    Register::SP,
    Register::ESP,
    Register::BP,
    Register::EBP,
    Register::DS,
    Register::ES,
    Register::SS,
    Register::CS,
];

/// Python `FROM_CONTRACT[one]`: a contract register's root, or `FLAGS`.
pub fn from_contract(one: runtime::Reg) -> Option<Register> {
    super::as_named(one).map(root)
}

/// Python `detached`: a rewritten raising operation without its decoded node.
pub fn detached(mut operation: Op) -> Op {
    operation.source_backed = false;
    if let Some(raising) = &mut operation.raising {
        raising.node = None;
    }
    operation
}

/// Python `source_free`: a raising rewrite that owns no input occurrence.
pub fn source_free(mut operation: Op) -> Op {
    operation.raising = None;
    operation.source_backed = false;
    operation.absorbed = Vec::new();
    operation
}

/// Python `raising_occurrence`: raw ownership on an operation still inside the raise.
pub fn raising_occurrence(
    operation: &Op,
    covers: (i64, i64),
    extra: Vec<(i64, i64)>,
    node: Option<Arc<Node>>,
) -> Op {
    let mut made = operation.clone();
    made.raising = Some(Box::new(Raising { node, covers: Some(covers), extra_covers: extra }));
    made
}

/// Python `_raising_ranges`: concrete ownership while recognition is inside the raise.
pub fn raising_ranges(op: &Op) -> Vec<(i64, i64)> {
    let Some(raising) = &op.raising else {
        return Vec::new();
    };
    raising.covers.into_iter().chain(raising.extra_covers.iter().copied()).collect()
}

/// Python `raising_owned`: the exact occurrences of `owners`, merged.
pub fn raising_owned(operation: Op, owners: &[&Op]) -> Op {
    let mut ranges: Vec<(i64, i64)> =
        owners.iter().flat_map(|owner| raising_ranges(owner)).filter(|span| span.0 < span.1).collect();
    ranges.sort();
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (low, high) in ranges {
        match merged.last_mut() {
            Some(last) if low <= last.1 => last.1 = last.1.max(high),
            _ => merged.push((low, high)),
        }
    }
    if merged.is_empty() {
        return operation;
    }
    let node = operation.node().cloned();
    let mut made = operation;
    made.raising = Some(Box::new(Raising { node, covers: Some(merged[0]), extra_covers: merged[1..].to_vec() }));
    made
}

/// Python `raising_adjacent`: two single source occurrences that touch.
pub fn raising_adjacent(first: &Op, second: &Op) -> bool {
    let (before, after) = (raising_ranges(first), raising_ranges(second));
    before.len() == 1 && after.len() == 1 && before[0].1 == after[0].0
}

/// Registers as Python's frozensets of `Register_`: iteration is sorted,
/// which is `sorted(..., key=lambda o: (o is not FLAGS, o))` since FLAGS is 0.
pub type Registers = BTreeSet<Register>;

/// Python `_call_touches`: what a call disturbs and reads, where established.
pub fn call_touches(name: Option<&str>, routine: Option<&runtime::Contract>) -> Option<(Registers, Registers)> {
    let owned;
    let routine = match routine {
        Some(one) => one,
        None => {
            owned = runtime::contract(name);
            &owned
        }
    };
    if !routine.established && !runtime::established_inputs(routine) {
        return None;
    }
    let changed: Registers = runtime::disturbs(routine).into_iter().filter_map(from_contract).collect();
    let mut disturbed: Registers = TRACKED.into_iter().filter(|one| changed.contains(one)).collect();
    disturbed.insert(FLAGS);
    if !runtime::established_inputs(routine) {
        let mut every: Registers = TRACKED.into_iter().collect();
        every.insert(FLAGS);
        return Some((disturbed, every));
    }
    let direct = if routine.direct_inputs.is_none() { &routine.inputs } else { &routine.direct_inputs };
    let reads = direct.iter().flatten().copied().filter_map(from_contract).collect();
    Some((disturbed, reads))
}

/// Python `_restore_touches`: what the restore idiom really disturbs.
pub fn restore_touches(node: &Node) -> Option<(Registers, Registers)> {
    let Node::Restore(node) = node else {
        return None;
    };
    let (source, into) = super::restore_pair(node.pair as i64)?;
    Some((BTreeSet::from([into]), BTreeSet::from([source, into])))
}

/// Python `_touched`: (defines, uses) as tracked variables, flags as FLAGS.
pub fn touched(
    node: &Node,
    calls: Option<&IndexMap<i64, String>>,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
) -> (Registers, Registers) {
    if matches!(node, Node::Opaque(opaque) if opaque.insn.insn.code() == Code::Into) {
        return (BTreeSet::new(), BTreeSet::from([FLAGS]));
    }
    if let (Some(calls), Node::Call(call)) = (calls, node) {
        let at = call.insn.at as i64;
        let known = call_touches(
            calls.get(&at).map(String::as_str),
            contracts.and_then(|contracts| contracts.get(&at)),
        );
        if let Some(known) = known {
            return known;
        }
    }
    if let Some(halves) = restore_touches(node) {
        return halves;
    }
    let effects = node.effects();
    let mut defines: Registers = match &effects.defs {
        None => TRACKED.into_iter().collect(),
        Some(defs) => defs.iter().copied().filter(|one| TRACKED.contains(one)).collect(),
    };
    let mut uses: Registers = match &effects.uses {
        None => TRACKED.into_iter().collect(),
        Some(used) => used.iter().copied().filter(|one| TRACKED.contains(one)).collect(),
    };
    if effects.defs.is_none() || !effects.flags_written.is_empty() {
        defines.insert(FLAGS);
    }
    if effects.uses.is_none() || !effects.flags_read.is_empty() {
        uses.insert(FLAGS);
    }
    (defines, uses)
}

/// Python `_referenced`: which fixup each operation's own operand carries, by op id.
pub fn _referenced(body: &MirBody, found: &Module) -> IndexMap<u32, Vec<i64>> {
    let known: BTreeSet<i64> = found.fixup_at.keys().copied().collect();
    let mut out = IndexMap::default();
    if known.is_empty() {
        return out;
    }
    let owned = |op: &Op| -> Option<i64> {
        let node = op.node()?;
        if matches!(**node, Node::Restore(_)) {
            return None;
        }
        let first = found.code.get(op.at as usize).copied();
        if matches!(first, Some(0x9a | 0xea)) && known.contains(&(op.at + 1)) {
            return Some(op.at + 1);
        }
        let (lo, hi) = span(node);
        let inside: Vec<i64> = known.iter().copied().filter(|&one| lo as i64 <= one && one < hi as i64).collect();
        if inside.len() == 1 { Some(inside[0]) } else { None }
    };
    for block in &body.blocks {
        for op in &block.ops {
            if let (Some(id), Some(at)) = (op.id, owned(op)) {
                out.insert(id, vec![at]);
            }
        }
    }
    out
}

impl RaisedBody {
    /// Python `replace(body, blocks=blocks)` on a `_RaisedBody`.
    pub fn with_blocks(&self, blocks: Vec<MirBlock>) -> RaisedBody {
        RaisedBody { body: self.body.with_blocks(blocks), origin: self.origin.clone(), pins: self.pins.clone() }
    }

    /// Python `replace(body, ...)` for fields other than blocks.
    pub fn body_mut(&mut self) -> &mut MirBody {
        &mut self.body
    }
}

/// Python `_BY_BRANCH`: a conditional branch's mnemonic is the comparison it reads.
fn by_branch(name: &str) -> Option<Kind> {
    Some(match name {
        "jl" | "jnge" => Kind::Lt,
        "jle" | "jng" => Kind::Le,
        "jg" | "jnle" => Kind::Gt,
        "jge" | "jnl" => Kind::Ge,
        "je" | "jz" => Kind::Eq,
        "jne" | "jnz" => Kind::Ne,
        "jb" | "jc" | "jnae" => Kind::Below,
        "jbe" | "jna" => Kind::BelowEq,
        "ja" | "jnbe" => Kind::Above,
        "jae" | "jnb" | "jnc" => Kind::AboveEq,
        _ => return None,
    })
}

/// Python `_stack_effect` over `_FLOAT_DEPTH`.
fn _stack_effect(what: &Semantics) -> Option<i64> {
    match what.op {
        Operation::FloatLoad => Some(1),
        Operation::FloatStore | Operation::FloatArithPop => Some(-1),
        Operation::FloatArith | Operation::FloatUnary => Some(0),
        _ => None,
    }
}

/// Python `_merged`: which use is only the previous contents of which result.
fn _merged(what: &Semantics, holds: &IndexMap<Register, Value>, written: &IndexMap<Register, Value>) -> OrderedMap<Value, Value> {
    let mut out = OrderedMap::new();
    if what.sources.is_empty() && what.dests.is_empty() {
        return out;
    }
    let named: BTreeSet<Register> = what
        .sources
        .iter()
        .filter_map(|one| match one {
            Loc::Reg(reg) => Some(root(reg.register)),
            _ => None,
        })
        .collect();
    let narrow: BTreeSet<Register> = what
        .dests
        .iter()
        .filter_map(|one| match one {
            Loc::Reg(reg) if reg.width == 2 => Some(root(reg.register)),
            _ => None,
        })
        .collect();
    for (register, value) in written {
        if value.flags {
            continue;
        }
        let rooted = root(*register);
        if named.contains(&rooted) && !narrow.contains(&rooted) {
            continue;
        }
        if let Some(was) = holds.get(register) {
            out.insert(*was, *value);
        }
    }
    out
}

/// Python `_normalised`: the operands the operation really has.
fn _normalised(kind: Kind, name: &str, args: Vec<Arg>) -> Vec<Arg> {
    if matches!(kind, Kind::Add | Kind::Sub) && matches!(name, "inc" | "dec") && args.len() == 1 {
        let width = super::arg_width(&args[0]);
        let mut args = args;
        args.push(Arg::Const(Const::new(1, width)));
        return args;
    }
    args
}

/// Python `Unraisable`: a contract this cannot honour.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unraisable(pub String);

impl std::fmt::Display for Unraisable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Python `_call_args`: a call's declared arguments, in contract slot order.
fn _call_args(routine: Option<&runtime::Contract>, holds: &IndexMap<Register, Value>) -> Result<(Vec<Arg>, bool), Unraisable> {
    let Some(routine) = routine else {
        return Ok((Vec::new(), false));
    };
    if !runtime::established_inputs(routine) {
        return Ok((Vec::new(), false));
    }
    let mut made = Vec::new();
    for one in runtime::direct_slots(routine) {
        let value = from_contract(one).and_then(|register| holds.get(&register).copied());
        let width = super::as_named(one).and_then(crate::backend::target::width_of);
        let (Some(value), Some(width)) = (value, width) else {
            return Err(Unraisable(format!("{} reads {} and nothing reaches it", routine.name, one.value())));
        };
        made.push(Arg::Held(Held { value, width: width as u32 }));
    }
    Ok((made, true))
}

/// Python `_Namer`: fresh values, and which one each variable currently holds.
struct Namer {
    next: u32,
    stack: IndexMap<Register, Vec<Value>>,
    named: IndexMap<Register, u32>,
    versions: IndexMap<Register, u32>,
    origin: OrderedMap<Value, Register>,
}

impl Namer {
    fn new() -> Self {
        Self {
            next: 0,
            stack: IndexMap::default(),
            named: IndexMap::default(),
            versions: IndexMap::default(),
            origin: OrderedMap::new(),
        }
    }

    fn fresh(&mut self, of: Register, at: i64) -> Value {
        self.next += 1;
        let count = self.named.len() as u32;
        let which = *self.named.entry(of).or_insert(count);
        let version = self.versions.get(&of).copied().unwrap_or(0) + 1;
        self.versions.insert(of, version);
        let made = Value { id: self.next, at, flags: of == FLAGS, variable: which, version };
        self.origin.insert(made, of);
        made
    }

    fn current(&mut self, of: Register, at: i64) -> Value {
        if self.stack.get(&of).is_none_or(Vec::is_empty) {
            let made = self.fresh(of, at);
            self.stack.entry(of).or_default().push(made);
        }
        *self.stack[&of].last().expect("pushed above")
    }
}

/// Python `_stack_slot`: where a push or pop's own cell sits, and sp afterwards.
fn _stack_slot(node: &Node, offset: Option<i64>, name: Option<&str>) -> (Option<i64>, Option<Addr>) {
    let Some(mut offset) = offset else {
        return (None, None);
    };
    let effects = node.effects();
    match node.semantics().op {
        Operation::Push => {
            let width = effects.stores.first().map_or(0, |one| one.width);
            if width == 0 {
                return (None, None);
            }
            offset -= i64::from(width);
            (Some(offset), Some(Addr::new(Space::Stack, offset)))
        }
        Operation::Pop => {
            let width = effects.loads.first().map_or(0, |one| one.width);
            if width == 0 {
                return (None, None);
            }
            (Some(offset + i64::from(width)), Some(Addr::new(Space::Stack, offset)))
        }
        _ => {
            if matches!(node, Node::Restore(_)) {
                return (Some(offset), None);
            }
            if let Some(consumed) = name.and_then(super::_consumed) {
                return (Some(offset + consumed), None);
            }
            let found = match node {
                Node::Opaque(one) => Some(&one.insn),
                Node::Long(one) => Some(&one.insn),
                Node::Call(one) => Some(&one.insn),
                _ => None,
            };
            if found.is_some_and(stack::touches_sp) {
                return (None, None);
            }
            (Some(offset), None)
        }
    }
}

/// Python `_memrefs`: `ir.Mem` cells with the values their address registers hold now.
fn _memrefs(
    cells: &[crate::model::ir::Mem],
    namer: &mut Namer,
    at: i64,
    slot: Option<Addr>,
    space: Option<Space>,
    beyond: Option<&Reach>,
) -> Vec<MemRef> {
    let mut out = Vec::new();
    for cell in cells {
        let addr = if cell.addr.is_none() && slot.is_some() { slot } else { cell.addr };
        let mut base = None;
        let mut base_width = 4;
        let rooted = root(cell.through);
        if TRACKED.contains(&rooted) {
            base = Some(namer.current(rooted, at));
            base_width = cell.through.size() as u32;
        }
        if let Some(addr) = addr {
            let rooted = root(addr.base);
            if TRACKED.contains(&rooted) {
                base = Some(namer.current(rooted, at));
                base_width = addr.base.size() as u32;
            }
        }
        let mut made = MemRef::new(addr, cell.width);
        made.base = base;
        made.space = space;
        made.beyond = beyond.cloned();
        made.base_width = base_width;
        out.push(made);
    }
    out
}

/// Python `raise_body`'s blocks as `loops` reads them.
struct Shape {
    at: i64,
    succ: Vec<i64>,
}

impl loops::Node for Shape {
    fn at(&self) -> i64 {
        self.at
    }

    fn succ(&self) -> &[i64] {
        &self.succ
    }
}

fn shapes(blocks: &[&Block]) -> Vec<Shape> {
    blocks
        .iter()
        .map(|block| Shape { at: block.at as i64, succ: block.succ.iter().map(|&one| one as i64).collect() })
        .collect()
}

/// Python `_placed`: which variables need a phi in which block.
fn _placed(
    blocks: &[&Block],
    nodes: &IndexMap<i64, Arc<Node>>,
    entry: Option<i64>,
    calls: Option<&IndexMap<i64, String>>,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
) -> BTreeMap<i64, Registers> {
    let frontier = loops::frontiers(&shapes(blocks), entry);
    let mut defines: IndexMap<Register, BTreeSet<i64>> = IndexMap::default();
    for block in blocks {
        for insn in &block.insns {
            let Some(node) = nodes.get(&(insn.at as i64)) else {
                continue;
            };
            for one in touched(node, calls, contracts).0 {
                defines.entry(one).or_default().insert(block.at as i64);
            }
        }
    }
    let mut needed: BTreeMap<i64, Registers> = blocks.iter().map(|block| (block.at as i64, Registers::new())).collect();
    for (variable, places) in &defines {
        let mut pending: Vec<i64> = places.iter().copied().collect();
        let mut seen = BTreeSet::new();
        while let Some(at) = pending.pop() {
            for &join in frontier.get(&at).into_iter().flatten() {
                if !seen.insert(join) {
                    continue;
                }
                needed.entry(join).or_default().insert(*variable);
                pending.push(join);
            }
        }
    }
    needed
}

/// Python `_AT_WIDTH[root][width]`: which register a root is at a width.
fn at_width(rooted: Register, width: u32) -> Option<Register> {
    ROOT.iter()
        .find(|(one, root)| *root == rooted && one.size() as u32 == width)
        .map(|(one, _)| *one)
}

/// Python `_RESOURCE`: a machine resource MIR holds no value for.
fn resource(register: Register) -> &'static str {
    match register {
        Register::ES => "es",
        Register::DS => "ds",
        Register::SS => "ss",
        Register::CS => "cs",
        Register::FS => "fs",
        Register::GS => "gs",
        _ => "",
    }
}

/// Python `_operands`: one node's operands as MIR's own, in the operation's order.
fn _operands(
    what: &Semantics,
    holds: &IndexMap<Register, Value>,
    written: &IndexMap<Register, Value>,
    loads: &[MemRef],
    stores: &[MemRef],
) -> (Vec<Arg>, Vec<Arg>) {
    let one = |loc: &Loc, cells: &mut std::collections::VecDeque<MemRef>, place: &IndexMap<Register, Value>| -> Arg {
        match loc {
            Loc::Reg(reg) => {
                let rooted = root(reg.register);
                match place.get(&rooted) {
                    Some(value) if at_width(rooted, reg.width) == Some(reg.register) => {
                        Arg::Held(Held { value: *value, width: reg.width })
                    }
                    _ => Arg::Opaque(super::Opaque::named(Some(loc.clone()), resource(reg.register))),
                }
            }
            Loc::Imm(imm) => match imm.address {
                Some(address) => Arg::Symbol(Symbol {
                    space: address.space,
                    index: address.index,
                    offset: address.disp,
                    width: imm.width,
                    addend: imm.value,
                }),
                None => Arg::Const(Const::new(imm.value, imm.width)),
            },
            Loc::Address(address)
                if address.through == Register::BP
                    && address.index == Register::None
                    && address.addr.is_some_and(|addr| addr.space == Space::Frame)
                    && matches!(what.dests.first(), Some(Loc::Reg(reg)) if reg.width == 2) =>
            {
                Arg::FrameAddress(FrameAddress::new(address.offset, 2))
            }
            Loc::Mem(mem) => {
                let Some(mut reference) = cells.pop_front() else {
                    return Arg::Opaque(super::Opaque::new(Some(loc.clone())));
                };
                let rooted = root(mem.through);
                if reference.base.is_none() && TRACKED.contains(&rooted) {
                    reference.base = holds.get(&rooted).copied();
                }
                Arg::Cell(Cell { r#ref: reference })
            }
            Loc::St(st) => Arg::Opaque(super::Opaque::named(Some(loc.clone()), format!("st{}", st.index))),
            _ => Arg::Opaque(super::Opaque::new(Some(loc.clone()))),
        }
    };
    let mut read: std::collections::VecDeque<MemRef> = loads.iter().cloned().collect();
    let mut write: std::collections::VecDeque<MemRef> = stores.iter().cloned().collect();
    (
        what.sources.iter().map(|loc| one(loc, &mut read, holds)).collect(),
        what.dests.iter().map(|loc| one(loc, &mut write, written)).collect(),
    )
}

/// Python `_rebased`: the same cells, holding whichever value reaches them now.
fn _rebased(refs: &[MemRef], namer: &mut Namer, at: i64) -> Vec<MemRef> {
    refs.iter()
        .map(|reference| {
            let mut base = None;
            if let Some(addr) = reference.addr {
                let rooted = root(addr.base);
                if TRACKED.contains(&rooted) {
                    base = Some(namer.current(rooted, at));
                }
            }
            let mut made = reference.clone();
            made.base = base;
            made
        })
        .collect()
}

/// Python `_absorbed_loads`: the memory a folded site reads, its own cells.
fn _absorbed_loads(args: &[Arg], namer: &mut Namer, at: i64) -> Vec<MemRef> {
    let cells: Vec<MemRef> = args
        .iter()
        .filter_map(|one| match one {
            Arg::Cell(cell) if cell.r#ref.addr.is_some() => Some(cell.r#ref.clone()),
            _ => None,
        })
        .collect();
    _rebased(&cells, namer, at)
}

/// `sorted(registers, key=lambda o: (o is not FLAGS, o))`.
fn in_order(registers: &Registers) -> Vec<Register> {
    registers.iter().copied().collect()
}

/// Python `raise_body`: one body's blocks, in SSA, or why they could not be.
///
/// The outer `Err` is Python's `Unraisable` escaping; the inner one is the
/// refusal string Python returns.
#[allow(clippy::too_many_arguments)]
pub fn raise_body(
    blocks: &[Block],
    nodes: &IndexMap<i64, Arc<Node>>,
    entry: Option<i64>,
    calls: Option<&IndexMap<i64, String>>,
    sites: Option<&IndexMap<i64, calls::CallSite>>,
    unreached: Option<&Reach>,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
    spared: Option<&IndexMap<i64, Vec<(Addr, u32)>>>,
) -> Result<Result<RaisedBody, String>, Unraisable> {
    let per_call;
    let chosen = match contracts {
        Some(contracts) => contracts,
        None => {
            per_call = runtime::per_call(calls.unwrap_or(&IndexMap::default()), "", &BTreeSet::new());
            &per_call
        }
    };
    let handles_errors = chosen.values().any(|contract| contract.error_handling);
    if blocks.is_empty() {
        return Ok(Err("no blocks to raise".to_owned()));
    }
    let start = entry.unwrap_or(blocks[0].at as i64);

    let everything: IndexMap<i64, &Block> = blocks.iter().map(|block| (block.at as i64, block)).collect();
    if !everything.contains_key(&start) {
        return Ok(Err(format!("the entry {start:#06x} is not one of these blocks")));
    }
    let mut reachable = BTreeSet::from([start]);
    let mut pending = vec![start];
    while let Some(at) = pending.pop() {
        for &successor in &everything[&at].succ {
            let successor = successor as i64;
            if everything.contains_key(&successor) && reachable.insert(successor) {
                pending.push(successor);
            }
        }
    }
    let blocks: Vec<&Block> = blocks.iter().filter(|block| reachable.contains(&(block.at as i64))).collect();

    if !loops::irreducible(&shapes(&blocks), Some(start)).is_empty() {
        return Ok(Err("the body's control flow is irreducible, so it has no dominator tree".to_owned()));
    }

    let by_at: IndexMap<i64, &Block> = blocks.iter().map(|block| (block.at as i64, *block)).collect();
    let idom = loops::immediate_dominators(&shapes(&blocks), Some(start));
    let mut children: IndexMap<i64, Vec<i64>> = blocks.iter().map(|block| (block.at as i64, Vec::new())).collect();
    for block in &blocks {
        if let Some(Some(parent)) = idom.get(&(block.at as i64)) {
            children.entry(*parent).or_default().push(block.at as i64);
        }
    }

    let needed = _placed(&blocks, nodes, Some(start), calls, Some(chosen));
    let mut namer = Namer::new();
    let mut phis: IndexMap<i64, IndexMap<Register, Phi>> =
        blocks.iter().map(|block| (block.at as i64, IndexMap::default())).collect();
    let ops: IndexMap<i64, Vec<Op>> = blocks.iter().map(|block| (block.at as i64, Vec::new())).collect();

    for block in &blocks {
        let at = block.at as i64;
        for variable in in_order(&needed[&at]) {
            let made = namer.fresh(variable, at);
            phis[&at].insert(variable, Phi::new(made));
        }
    }

    let mut raiser = Raiser {
        by_at,
        children,
        namer,
        phis,
        ops,
        start,
        nodes,
        calls,
        sites,
        unreached,
        chosen,
        spared,
        handles_errors,
    };
    raiser.rename(start)?;
    let Raiser { namer, mut phis, mut ops, .. } = raiser;
    let body = MirBody::new(
        start,
        blocks
            .iter()
            .map(|block| {
                let at = block.at as i64;
                MirBlock::new(
                    at,
                    phis.swap_remove(&at).expect("every block").into_values().collect(),
                    ops.swap_remove(&at).expect("every block"),
                    block.succ.iter().map(|&one| one as i64).filter(|one| reachable.contains(one)).collect(),
                )
            })
            .collect(),
    );
    Ok(Ok(RaisedBody { body, origin: namer.origin, pins: OrderedMap::new() }))
}

/// `raise_body`'s nested `rename`, with the closure's captures as fields.
struct Raiser<'a> {
    by_at: IndexMap<i64, &'a Block>,
    children: IndexMap<i64, Vec<i64>>,
    namer: Namer,
    phis: IndexMap<i64, IndexMap<Register, Phi>>,
    ops: IndexMap<i64, Vec<Op>>,
    start: i64,
    nodes: &'a IndexMap<i64, Arc<Node>>,
    calls: Option<&'a IndexMap<i64, String>>,
    sites: Option<&'a IndexMap<i64, calls::CallSite>>,
    unreached: Option<&'a Reach>,
    chosen: &'a IndexMap<i64, runtime::Contract>,
    spared: Option<&'a IndexMap<i64, Vec<(Addr, u32)>>>,
    handles_errors: bool,
}

impl Raiser<'_> {
    fn rename(&mut self, at: i64) -> Result<(), Unraisable> {
        let block = self.by_at[&at];
        let start = self.start;
        let mut pushed: Vec<Register> = Vec::new();

        for (variable, phi) in &self.phis[&at] {
            self.namer.stack.entry(*variable).or_default().push(phi.result);
            pushed.push(*variable);
        }

        let mut offset: Option<i64> = Some(0);

        let empty = IndexMap::default();
        let inside = super::_within(self.sites.unwrap_or(&empty));
        for insn in &block.insns {
            let Some(node) = self.nodes.get(&(insn.at as i64)).cloned() else {
                continue;
            };
            let insn_at = insn.at as i64;
            if inside.contains(&insn_at) {
                offset = _stack_slot(&node, offset, None).0;
                continue;
            }
            let name = self.calls.and_then(|calls| calls.get(&insn_at)).map(String::as_str);
            let slot;
            (offset, slot) = _stack_slot(&node, offset, name);
            let (defines, uses) = touched(&node, self.calls, Some(self.chosen));
            let ordered_uses = in_order(&uses);
            let mut used: Vec<Value> = ordered_uses.iter().map(|&one| self.namer.current(one, start)).collect();

            let contract = self.chosen.get(&insn_at);
            let read_reach = raising_call_memory::reachable(
                contract,
                contract.map_or(runtime::Memory::Any, |contract| contract.reads),
                self.unreached,
                self.handles_errors,
            );
            let write_reach = raising_call_memory::reachable(
                contract,
                contract.map_or(runtime::Memory::Any, |contract| contract.writes),
                self.unreached,
                self.handles_errors,
            );
            let semantics = node.semantics();
            let effects = node.effects();
            let loading_stack = semantics.op == Operation::Pop;
            let pushing = semantics.op == Operation::Push;
            let calling = semantics.op == Operation::Call;
            let mut loads = _memrefs(
                &effects.loads,
                &mut self.namer,
                start,
                if loading_stack { slot } else { None },
                if loading_stack { Some(Space::Stack) } else { None },
                read_reach.as_ref(),
            );
            let mut stores = _memrefs(
                &effects.stores,
                &mut self.namer,
                start,
                if pushing { slot } else { None },
                if pushing || calling { Some(Space::Stack) } else { None },
                write_reach.as_ref(),
            );
            if calling {
                let mut returned = MemRef::new(None, 4);
                returned.beyond = write_reach.clone();
                returned.excludes = self.spared.and_then(|spared| spared.get(&insn_at)).cloned().unwrap_or_default();
                stores.push(returned);
            }
            let memory_complete =
                effects.memory_complete || (calling && read_reach.is_some() && write_reach.is_some());
            let holds: IndexMap<Register, Value> =
                ordered_uses.iter().copied().zip(used.iter().copied()).collect();
            let before: IndexMap<Register, Value> =
                TRACKED.iter().map(|&one| (one, self.namer.current(one, start))).collect();
            let mut made = Vec::new();
            let ordered_defines = in_order(&defines);
            for &one in &ordered_defines {
                let value = self.namer.fresh(one, insn_at);
                self.namer.stack.entry(one).or_default().push(value);
                pushed.push(one);
                made.push(value);
            }
            let written: IndexMap<Register, Value> =
                ordered_defines.iter().copied().zip(made.iter().copied()).collect();
            let mut place = _operands(semantics, &holds, &written, &loads, &stores);
            let found_loads: Vec<MemRef> = cells(&place.0);
            if !found_loads.is_empty() {
                loads = found_loads;
            }
            let found_stores: Vec<MemRef> = cells(&place.1);
            if !found_stores.is_empty() {
                stores = found_stores;
            }
            let mut kind = super::kind_of(semantics, &place.0, &place.1);
            let mut operands = _normalised(kind, semantics.name.as_deref().unwrap_or(""), place.0.clone());
            let site = self.sites.and_then(|sites| sites.get(&insn_at));
            let span_of = span(&node);
            let mut covers = (span_of.0 as i64, span_of.1 as i64);
            let mut where_at = insn_at;
            let mut handed = None;
            let folded = site.and_then(|site| super::_absorbing(site, &written).map(|folded| (site, folded)));
            if let Some((site, (folded_kind, folded_args, folded_results))) = folded {
                kind = folded_kind;
                operands = folded_args.clone();
                place = (folded_args, folded_results);
                covers = (site.start as i64, site.end as i64);
                where_at = site.start as i64;
                handed = super::_hands_back(kind, &written, &before);
                loads = _absorbed_loads(&place.0, &mut self.namer, start);
                let mut references = loads.iter();
                operands = operands
                    .into_iter()
                    .map(|one| match one {
                        Arg::Cell(_) => Arg::Cell(Cell { r#ref: references.next().expect("one per cell").clone() }),
                        other => other,
                    })
                    .collect();
                place.0 = operands.clone();
                let mut seen: IndexMap<Value, ()> = IndexMap::default();
                for arg in &operands {
                    if let Arg::Held(held) = arg {
                        seen.insert(held.value, ());
                    }
                }
                for reference in &loads {
                    for value in [reference.base, reference.segment].into_iter().flatten() {
                        seen.insert(value, ());
                    }
                }
                used = seen.into_keys().collect();
                stores = Vec::new();
            }
            let called = if kind == Kind::Call {
                _call_args(self.chosen.get(&insn_at), &holds)?
            } else {
                (Vec::new(), true)
            };
            let args = if kind == Kind::Call { called.0.clone() } else { operands };
            let kept: Vec<Value> = made
                .iter()
                .copied()
                .filter(|one| handed.as_ref().is_none_or(|(now, _, _): &(Value, Value, Value)| one != now))
                .collect();
            let opcode = if matches!(*node, Node::Restore(_)) {
                OpCode::Synth(Synth::HalfToLow)
            } else {
                OpCode::Operation(semantics.op)
            };
            let mut op = Op::new(where_at, opcode, semantics.name.clone().unwrap_or_default(), kept, used);
            op.loads = loads;
            op.stores = stores;
            op.kind = kind;
            op.merges = _merged(semantics, &holds, &written);
            op.stack = _stack_effect(semantics);
            op.test = if kind == Kind::Branch { by_branch(semantics.name.as_deref().unwrap_or("")) } else { None };
            op.raised = Some((args.clone(), place.1.clone()));
            op.args = args;
            op.results = place.1;
            op.target = semantics.target;
            op.id = Some(super::next_id());
            op.args_known = called.1;
            op.memory_complete = memory_complete;
            op.reads_complete = effects.uses.is_some() && called.1;
            op.raising = Some(Box::new(Raising { node: Some(node.clone()), covers: Some(covers), extra_covers: Vec::new() }));
            self.ops[&at].push(op);
            if let Some((now, was, answer)) = handed {
                self.ops[&at].push(super::_handing_back(where_at, &node, now, was, answer));
            }
        }

        for &successor in &block.succ {
            let successor = successor as i64;
            let Some(variables) = self.phis.get(&successor).map(|phis| phis.keys().copied().collect::<Vec<_>>())
            else {
                continue;
            };
            for variable in variables {
                let value = self.namer.current(variable, start);
                self.phis[&successor][&variable].incoming.insert(at, value);
            }
        }

        let mut children = self.children[&at].clone();
        children.sort();
        for child in children {
            self.rename(child)?;
        }

        for variable in pushed.into_iter().rev() {
            self.namer.stack[&variable].pop();
        }
        Ok(())
    }
}

/// `tuple(one.ref for one in operands if isinstance(one, Cell))`.
fn cells(operands: &[Arg]) -> Vec<MemRef> {
    operands
        .iter()
        .filter_map(|one| match one {
            Arg::Cell(cell) => Some(cell.r#ref.clone()),
            _ => None,
        })
        .collect()
}

/// Python `RaisedBodies`: the raised bodies and their machine-provenance side table.
pub struct RaisedBodies {
    pub values: Vec<(String, Rc<MirBody>)>,
    pub source: SourceMap,
    pub hints: IndexMap<i64, AllocationHints>,
}

/// Python `_opaque_effects`: effects on resources not represented by SSA values.
fn _opaque_effects(node: Option<&Node>) -> (Option<BTreeSet<String>>, Option<BTreeSet<String>>) {
    let outside = |registers: &Option<BTreeSet<Register>>| {
        registers.as_ref().map(|registers| {
            registers
                .iter()
                .filter(|one| !TRACKED.contains(one))
                .map(|one| format!("resource-{}", *one as u32))
                .collect()
        })
    };
    match node {
        None => (Some(BTreeSet::new()), Some(BTreeSet::new())),
        Some(node) => (outside(&node.effects().defs), outside(&node.effects().uses)),
    }
}

/// Python `_record_provenance`: every raise-time occurrence, before recognition.
fn _record_provenance(body: &MirBody, source: &mut SourceMap) -> Vec<u32> {
    let mut recorded = Vec::new();
    for block in &body.blocks {
        for op in &block.ops {
            let Some(id) = op.id else {
                continue;
            };
            recorded.push(id);
            if let Some(node) = op.node() {
                source.nodes.insert(id, node.clone());
            }
            let spans = raising_ranges(op);
            source.occurrences.insert(id, spans.into_iter().filter(|span| span.0 < span.1).collect());
        }
    }
    recorded
}

/// Python `_absorbed_ids`: occurrences wholly represented by this operation's ranges.
fn _absorbed_ids(op: &Op, source: &SourceMap, candidates: &[u32], owned: Option<&[(i64, i64)]>) -> Vec<u32> {
    let ranges: Vec<(i64, i64)> = match owned {
        Some(owned) if !owned.is_empty() => owned.to_vec(),
        _ => raising_ranges(op),
    };
    let ranges: Vec<(i64, i64)> = ranges.into_iter().filter(|span| span.0 < span.1).collect();
    if ranges.is_empty() {
        return op.absorbed.clone();
    }
    let within = |span: &(i64, i64)| ranges.iter().any(|&(low, high)| low <= span.0 && span.1 <= high);
    let found = candidates.iter().copied().filter(|identity| {
        source.occurrences.get(identity).is_some_and(|spans| !spans.is_empty() && spans.iter().all(within))
    });
    let mut out: Vec<u32> = Vec::new();
    for one in op.absorbed.iter().copied().chain(found) {
        if !out.contains(&one) {
            out.push(one);
        }
    }
    out
}

/// Python `_completed_ownership`: disjoint folded-site ranges found after recognition.
fn _completed_ownership(
    body: RaisedBody,
    source: &SourceMap,
    candidates: &[u32],
    coverage: &IndexMap<u32, Vec<(i64, i64)>>,
) -> RaisedBody {
    if coverage.is_empty() {
        return body;
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            block.with_ops(
                block
                    .ops
                    .iter()
                    .map(|op| match op.id.and_then(|id| coverage.get(&id)) {
                        Some(owned) => {
                            let mut made = op.clone();
                            made.absorbed = _absorbed_ids(op, source, candidates, Some(owned));
                            made
                        }
                        None => op.clone(),
                    })
                    .collect(),
            )
        })
        .collect();
    body.with_blocks(blocks)
}

/// Python `_externalized`: every decoded node moved out of a completed raise.
fn _externalized(body: RaisedBody, source: &mut SourceMap, candidates: &[u32]) -> RaisedBody {
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let node = op.node().cloned();
            if let (Some(node), Some(id)) = (&node, op.id) {
                source.nodes.insert(id, node.clone());
            }
            let (opaque_defs, opaque_uses) = _opaque_effects(node.as_deref());
            let absorbed = _absorbed_ids(op, source, candidates, None);
            let mut made = op.clone();
            made.raising = None;
            made.source_backed = node.is_some();
            made.opaque_defs = opaque_defs;
            made.opaque_uses = opaque_uses;
            made.absorbed = absorbed;
            ops.push(made);
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

/// Python `_unreached`: what a runtime call can reach inside the program's data.
fn _unreached(found: &Module) -> Option<Reach> {
    found.program_data.map(|data| (data, module::escaped(found)))
}

/// Python `_frame_bounded` on a `_RaisedBody`: `replace` keeps its private maps.
fn frame_bounded_raised(body: RaisedBody) -> RaisedBody {
    let RaisedBody { body, origin, pins } = body;
    RaisedBody { body: super::frame_bounded(body, false), origin, pins }
}

/// Python `_provenanced`: every reference with its region set as provenance.
fn _provenanced(body: RaisedBody, found: &Module) -> Result<RaisedBody, String> {
    let layout = regions::RegionLayout {
        shared_segments: None,
        landmarks: module::landmarks(found).into_iter().collect(),
    };
    let private: BTreeSet<i64> = found.program_data.into_iter().collect();
    let spared: BTreeSet<i64> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.loads.iter().chain(&op.stores))
        .flat_map(|reference| &reference.excludes)
        .filter(|(addr, _)| addr.space == Space::Segment)
        .map(|(addr, _)| addr.index)
        .collect();
    let moved = |reference: &MemRef| -> Result<MemRef, String> {
        if reference.provenance.is_some() {
            return Ok(reference.clone());
        }
        let mut made = reference.clone();
        made.provenance = Some(
            regions::provenance(reference, None, Some(&layout), &private, &spared)
                .map_err(|error| format!("{error:?}"))?,
        );
        Ok(made)
    };
    let operand = |one: &Arg| -> Result<Arg, String> {
        Ok(match one {
            Arg::Cell(cell) => Arg::Cell(Cell { r#ref: moved(&cell.r#ref)? }),
            other => other.clone(),
        })
    };
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let mut made = op.clone();
            made.loads = op.loads.iter().map(moved).collect::<Result<_, _>>()?;
            made.stores = op.stores.iter().map(moved).collect::<Result<_, _>>()?;
            made.args = op.args.iter().map(operand).collect::<Result<_, _>>()?;
            made.results = op.results.iter().map(operand).collect::<Result<_, _>>()?;
            made.memory_values = op
                .memory_values
                .iter()
                .map(|(reference, value)| Ok((moved(reference)?, value.clone())))
                .collect::<Result<_, String>>()?;
            ops.push(made);
        }
        blocks.push(block.with_ops(ops));
    }
    let initial = body
        .initial
        .iter()
        .map(|(reference, value)| Ok((moved(reference)?, value.clone())))
        .collect::<Result<Vec<_>, String>>()?;
    let mut made = body.with_blocks(blocks);
    made.body.initial = initial;
    Ok(made)
}

/// Python `_returned`: what a call hands back, held to the registers BC reads it from.
fn _returned(body: &RaisedBody) -> IndexMap<Value, Register> {
    let mut read: BTreeSet<Value> = body.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.uses.iter().copied()).collect();
    read.extend(body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.values().copied()));
    let mut out = IndexMap::default();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        if op.kind != Kind::Call {
            continue;
        }
        for one in &op.defines {
            if one.flags || !read.contains(one) {
                continue;
            }
            if let Some(register) = body.origin.get(one) {
                out.insert(*one, *register);
            }
        }
    }
    out
}

/// Python `bodies`: every body in the module, raised, labelled, and skipping what will not.
pub fn bodies(
    found: &Module,
    blocks: &[Block],
    contracts: Option<&mut IndexMap<i64, runtime::Contract>>,
    basic_semantics: bool,
    bounds_checks: bool,
) -> Result<RaisedBodies, String> {
    let mut source = SourceMap::from_module(found);
    let unreached = _unreached(found);
    let result = match decode::decode_module(found) {
        Ok(result) => result,
        Err(_) => return Ok(RaisedBodies { values: Vec::new(), source, hints: IndexMap::default() }),
    };
    let decoded_nodes: IndexMap<i64, Arc<Node>> =
        result.iter().flat_map(|body| body.nodes.iter()).map(|node| (span(node).0 as i64, node.clone())).collect();
    let header = split::has_header(found);
    let nodes: IndexMap<i64, Arc<Node>> =
        decoded_nodes.iter().map(|(&at, node)| (at, raising_returns::returned(node, header, None))).collect();
    let mut return_registers: IndexMap<i64, Vec<Register>> = IndexMap::default();
    if header {
        for procedure in cvinfo::parse(&found.records).procedures {
            let Some(signature) = procedure.signature() else {
                continue;
            };
            let returned_type = cvinfo::type_name(signature.return_type, Some(&procedure.types));
            return_registers.insert(
                procedure.offset,
                if returned_type.as_deref() == Some("LONG") {
                    vec![Register::AX, Register::DX]
                } else {
                    vec![Register::AX]
                },
            );
        }
    }
    let mut own;
    let contracts: &mut IndexMap<i64, runtime::Contract> = match contracts {
        Some(contracts) => contracts,
        None => {
            own = runtime::for_module(found, None).map_err(|error| error.to_string())?;
            &mut own
        }
    };
    for body in &result {
        let Some(registers) = return_registers.get(&(body.body.seed as i64)) else {
            continue;
        };
        let direct: BTreeSet<runtime::Reg> = [(runtime::Reg::Ax, Register::AX), (runtime::Reg::Dx, Register::DX)]
            .into_iter()
            .filter(|(_, machine)| registers.contains(machine))
            .map(|(register, _)| register)
            .collect();
        for (&at, name) in &found.calls {
            if name == "B$EXSA" && body.body.ranges.iter().any(|&(lo, hi)| lo as i64 <= at && at < hi as i64) {
                let mut specialized = contracts[&at].clone();
                specialized.direct_inputs = Some(direct.clone());
                contracts.insert(at, specialized);
            }
        }
    }
    let spared = raising_call_memory::spared(found, &result, contracts);
    let mut out: Vec<(String, RaisedBody)> = Vec::new();
    let mut error_handlers: Vec<RaisedBody> = Vec::new();
    for body in &result {
        let mine: Vec<Block> = blocks
            .iter()
            .filter(|one| body.body.ranges.iter().any(|&(lo, hi)| lo <= one.at && one.at < hi))
            .cloned()
            .collect();
        let procedure_nodes_own;
        let procedure_nodes = match return_registers.get(&(body.body.seed as i64)) {
            Some(registers) => {
                procedure_nodes_own = decoded_nodes
                    .iter()
                    .map(|(&at, node)| (at, raising_returns::returned(node, header, Some(registers))))
                    .collect::<IndexMap<i64, Arc<Node>>>();
                &procedure_nodes_own
            }
            None => &nodes,
        };
        let mine = raising_control::terminal_edges(mine, contracts);
        if mine.is_empty() {
            continue;
        }
        let carried = raising_carried::carried(&mine, &nodes, &found.calls, contracts);
        contracts.extend(carried);
        let sites = super::_sites(found, blocks);
        let built = match raise_body(
            &mine,
            procedure_nodes,
            Some(body.body.seed as i64),
            Some(&found.calls),
            Some(&sites),
            unreached.as_ref(),
            Some(contracts),
            Some(&spared),
        )
        .map_err(|error| format!("Unraisable: {error}"))?
        {
            Ok(built) => built,
            Err(_) => continue,
        };
        let provenance = _record_provenance(&built, &mut source);
        let built = raising_frame::annotated(built, found, &mine, contracts);
        let built = super::with_live_outs(built);
        let built = if basic_semantics { built } else { raising_numeric_policy::native(built) };
        let built = raising_division::scalar(built);
        let built = raising_calls::arithmetic(built, found, &mine, basic_semantics);
        let built = raising_bytes::scalar(built);
        let built = raising_longs::sign_fills(built);
        let built = raising_longs::scalar(built).map_err(|error| error.to_string())?;
        let built = raising_longs::unary(built);
        // Unary recognition exposes whole sources for adjacent word stores.
        let built = raising_longs::scalar(built).map_err(|error| error.to_string())?;
        let built = raising_longs::arguments(built);
        let built = raising_copies::scalar(built, found);
        let built = raising_conditions::loaded(built);
        let defined = module::defines(&found.records, found.seg);
        let array_calls: IndexMap<i64, String> =
            found.calls.iter().filter(|(_, name)| !defined.contains(*name)).map(|(&at, name)| (at, name.clone())).collect();
        let built = raising_arrays::annotated(built, &array_calls, module::family(&found.records).value());
        let built = raising_array_access::native(built, found, bounds_checks)?;
        let built = raising_addresses::loaded(built, Some(contracts))?;
        let built = raising_call_memory::fixed_assignments(built, found);
        let built = raising_call_memory::indirect_results(built, found);
        let built = raising_defseg::raised(built, found, contracts, &mut source)?;
        let built = if basic_semantics { built } else { raising_float_calls::raised(built, found, contracts, &mut source) };
        let built = if basic_semantics { built } else { raising_float_results::raised(built, found, contracts, &mut source) };
        let built = raising_longs::arguments(built);
        let built = raising_longs::sign_fills(built);
        let built = raising_floats::annotated(built);
        let built = if basic_semantics { built } else { raising_numeric_policy::checkpoints(built) };
        let built = raising_float_values::loaded(raising_float_values::raised(built));
        let built = raising_words::scalar(built);
        let built = if body.body.kind == BodyKind::Main {
            raising_literals::initialized(built, found, Some(contracts))?
        } else {
            built
        };
        let built = raising_dispatch::raised(built, found, &mine);
        let referenced = super::_referenced(&built, found);
        source.refs.extend(referenced);
        let built = _externalized(built, &mut source, &provenance);
        let built = frame_bounded_raised(built);
        let (folded, absorbed, refs, coverage) = super::_folded(&built, found, blocks);
        let mut built = _completed_ownership(built, &source, &provenance, &coverage);
        source.absorbed.extend(absorbed);
        source.refs.extend(refs);
        source.coverage.extend(coverage);
        let mut held = _returned(&built);
        for (value, register) in folded.iter() {
            held.insert(*value, *register);
        }
        for (value, register) in held.iter() {
            built.pins.insert(*value, *register);
        }
        let label = format!("{} {}", body.body.kind.value(), body.body.name.as_deref().unwrap_or("(main)"));
        if body.body.kind == BodyKind::ErrorHandler {
            error_handlers.push(built.clone());
        }
        out.push((label, built));
    }
    if !basic_semantics {
        let procedures: Vec<(&str, &MirBody)> = out
            .iter()
            .filter(|(name, _)| name.starts_with("procedure "))
            .map(|(name, body)| (name.as_str(), &body.body))
            .collect();
        let result_only = raising_call_memory::result_only_functions(&procedures, found)?;
        if !result_only.is_empty() {
            out = out
                .into_iter()
                .map(|(name, body)| (name, raising_call_memory::complete_result_calls(body, &found.calls, &result_only)))
                .collect();
        }
    }
    if !error_handlers.is_empty() {
        let summaries: Vec<_> = error_handlers
            .iter()
            .map(|one| raising_call_memory::handler_effects(one, &found.calls, contracts, unreached.as_ref()))
            .collect();
        let summary = if summaries.iter().any(Option::is_none) {
            None
        } else {
            let summaries: Vec<_> = summaries.into_iter().flatten().collect();
            Some((
                summaries.iter().flat_map(|one| one.0.iter().cloned()).collect::<Vec<_>>(),
                summaries.iter().flat_map(|one| one.1.iter().cloned()).collect::<Vec<_>>(),
            ))
        };
        out = out
            .into_iter()
            .map(|(name, body)| {
                (
                    name,
                    frame_bounded_raised(raising_call_memory::with_handler_effects(
                        body,
                        summary.as_ref(),
                        &found.calls,
                        contracts,
                        unreached.as_ref(),
                    )),
                )
            })
            .collect();
    }
    let mut finished = Vec::new();
    for (name, body) in out {
        let mut body = super::with_live_outs(body);
        body.body.stack_in_data = true;
        finished.push((name, _provenanced(body, found)?));
    }
    let mut hints = IndexMap::default();
    for (_, body) in &finished {
        hints.insert(body.entry, AllocationHints::from_body(body).map_err(|error| error.to_string())?);
    }
    Ok(RaisedBodies {
        values: finished.into_iter().map(|(name, body)| (name, Rc::new(super::public(body)))).collect(),
        source,
        hints,
    })
}

/// Python `_with_hints`: placement reattached for raise-time recognition tests.
pub fn _with_hints(body: &MirBody, hints: &AllocationHints) -> RaisedBody {
    let mut values = BTreeSet::new();
    for block in &body.blocks {
        values.extend(block.phis.iter().map(|phi| phi.result));
        values.extend(block.phis.iter().flat_map(|phi| phi.incoming.values().copied()));
        for op in &block.ops {
            values.extend(op.defines.iter().chain(&op.uses).chain(&op.exits).copied());
        }
    }
    let mut origin = OrderedMap::new();
    for value in values {
        if let Some(place) = hints.origin_of(value) {
            origin.insert(value, place);
        }
    }
    let mut pins = OrderedMap::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        for (index, value) in op.defines.iter().enumerate() {
            if let Some(place) = hints.pin_of(op, index) {
                pins.insert(*value, place);
            }
        }
    }
    RaisedBody { body: body.clone(), origin, pins }
}

/// Python `_with_raise_context`: the private raise view for focused frontend tests.
pub fn _with_raise_context(body: &MirBody, hints: &AllocationHints, source: &SourceMap) -> RaisedBody {
    let occurrence = |op: &Op| -> Op {
        let spans: Vec<(i64, i64)> = op
            .absorbed
            .iter()
            .flat_map(|identity| source.occurrences.get(identity).into_iter().flatten().copied())
            .collect();
        let node = op.id.and_then(|id| source.nodes.get(&id)).cloned();
        if spans.is_empty() && node.is_none() {
            return op.clone();
        }
        raising_occurrence(op, spans.first().copied().unwrap_or((op.at, op.at)), spans.iter().skip(1).copied().collect(), node)
    };
    let private = body.with_blocks(
        body.blocks.iter().map(|block| block.with_ops(block.ops.iter().map(occurrence).collect())).collect(),
    );
    _with_hints(&private, hints)
}
