//! Port of `qbopt/backend/floatalloc.py`: assign floating LIR values to the
//! target register stack.

use std::cell::RefCell;
use std::collections::VecDeque;
use crate::support::hash::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};

use crate::analysis::regions;
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::floatregions::{Raised, boundary, bridged};
use crate::backend::frame::Frame;
use crate::backend::lower::Unlowered;
use crate::backend::select;
use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space, St};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::pyset::PySet;

fn unlowered(message: &str) -> Raised {
    Raised::Unlowered(Unlowered(message.to_owned()))
}

fn st(index: usize) -> Loc {
    Loc::St(St { index: index as u32 })
}

fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

/// `lir.Insn(at, (at, at), what, (), ())`.
fn inserted(at: i64, what: Semantics) -> Insn {
    Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new())
}

fn name_is(what: &Semantics, name: &str) -> bool {
    what.name.as_deref() == Some(name)
}

/// Python's `not what.name`.
fn unnamed(what: &Semantics) -> bool {
    what.name.as_deref().is_none_or(str::is_empty)
}

fn width_of(arg: &Loc) -> Option<u32> {
    match arg {
        Loc::Held(held) => Some(held.width),
        Loc::Imm(imm) => Some(imm.width),
        _ => None,
    }
}

fn index_of(values: &[u32], value: u32) -> usize {
    values.iter().position(|one| *one == value).expect("list.index: value is not in list")
}

/// A home load's cell: every home is a load from memory.
fn cell_of(load: &Insn) -> &Mem {
    match &load.what.as_ref().expect("a home load has semantics").sources[0] {
        Loc::Mem(cell) => cell,
        _ => unreachable!("a home load reads memory"),
    }
}

/// x87 reads integers from memory, for named values and physical stack slots.
fn _integer_loads(body: &LirBody, mut frame: Option<&mut Frame>) -> Result<LirBody, Raised> {
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            let mut one = Arc::clone(one);
            if let Some(what) = one.what.clone() {
                if what.op == Operation::FloatLoad
                    && name_is(&what, "fild")
                    && what.sources.len() == 1
                    && what.dests.len() == 1
                    && matches!(what.sources[0], Loc::Held(_) | Loc::Imm(_))
                    && width_of(&what.sources[0]).is_some_and(|width| width == 2 || width == 4)
                    && matches!(what.dests[0], Loc::Held(_) | Loc::St(_))
                {
                    if let Loc::Imm(Imm { value: value @ (0 | 1), .. }) = what.sources[0] {
                        let mut made = (*one).clone();
                        made.what = Some(Semantics {
                            name: Some(if value == 0 { "fldz" } else { "fld1" }.to_owned()),
                            sources: Vec::new(),
                            ..what
                        });
                        insns.push(Arc::new(made));
                        continue;
                    }
                    let Some(frame) = frame.as_deref_mut() else {
                        return Err(unlowered("integer-to-floating conversion requires an owned frame"));
                    };
                    let (value, destination) = (&what.sources[0], &what.dests[0]);
                    let width = i64::from(width_of(value).expect("checked above"));
                    let cell = match destination {
                        Loc::Held(destination) => frame.cell(i64::from(destination.value), width)?,
                        _ => frame.cell(("integer-load", one.at), width)?,
                    };
                    let uses: Vec<u32> = match value {
                        Loc::Held(value) => vec![value.value],
                        _ => Vec::new(),
                    };
                    insns.push(Arc::new(Insn::new(
                        one.at,
                        Some((one.at, one.at)),
                        Some(semantics(Operation::Move, "mov", vec![Loc::Mem(cell.clone())], vec![value.clone()])),
                        Vec::new(),
                        uses.clone(),
                    )));
                    let mut made = (*one).clone();
                    made.what = Some(Semantics { sources: vec![Loc::Mem(cell)], ..what.clone() });
                    made.uses = one.uses.iter().copied().filter(|arg| !uses.contains(arg)).collect();
                    one = Arc::new(made);
                }
            }
            insns.push(one);
        }
        let mut made = block.clone();
        made.insns = insns;
        blocks.push(made);
    }
    let mut out = body.clone();
    out.blocks = blocks;
    Ok(out)
}

/// Materialize integer conversions, including runtime results in physical ST0.
fn _integer_stores(body: &LirBody, mut frame: Option<&mut Frame>, basic_semantics: bool) -> Result<LirBody, Raised> {
    let mut integer_readers: HashSet<u32> = body.pins.keys().copied().collect();
    for block in &body.blocks {
        integer_readers.extend(block.phis.iter().flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)));
        for one in &block.insns {
            integer_readers.extend(&one.uses);
            if let Some(what) = &one.what {
                integer_readers.extend(what.sources.iter().filter_map(|arg| match arg {
                    Loc::Held(held) => Some(held.value),
                    _ => None,
                }));
            }
        }
    }
    let unknown_readers = body
        .insns()
        .iter()
        .any(|one| one.what.as_ref().is_none_or(|what| what.op == Operation::Barrier));
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            let Some(what) = &one.what else {
                insns.push(Arc::clone(one));
                continue;
            };
            let result = match what.dests.as_slice() {
                [Loc::Held(result)] if result.width == 2 || result.width == 4 => *result,
                _ => {
                    insns.push(Arc::clone(one));
                    continue;
                }
            };
            if what.op != Operation::FloatStore
                || !(name_is(what, "fistp") || name_is(what, "fisttp"))
                || what.sources.len() != 1
            {
                insns.push(Arc::clone(one));
                continue;
            }
            let Some(frame) = frame.as_deref_mut() else {
                return Err(unlowered("floating-to-integer conversion requires an owned frame"));
            };
            let cell = frame.cell(("integer-conversion", i64::from(result.value)), i64::from(result.width))?;
            let wait = Arc::new(inserted(one.at, semantics(Operation::Nothing, "wait", Vec::new(), Vec::new())));
            let mut store = (**one).clone();
            store.what = Some(Semantics { dests: vec![Loc::Mem(cell.clone())], ..what.clone() });
            store.defines = one.defines.iter().copied().filter(|value| *value != result.value).collect();
            store.widths = one.widths.iter().copied().filter(|(value, _)| *value != result.value).collect();
            let store = Arc::new(store);
            if basic_semantics {
                insns.extend([Arc::clone(&wait), store, wait]);
            } else {
                insns.push(store);
            }
            if unknown_readers || integer_readers.contains(&result.value) {
                let mut conversion = Insn::new(
                    one.at,
                    Some((one.at, one.at)),
                    Some(semantics(Operation::Move, "mov", vec![Loc::Held(result)], vec![Loc::Mem(cell)])),
                    vec![result.value],
                    Vec::new(),
                );
                conversion.widths = vec![(result.value, result.width)];
                insns.push(Arc::new(conversion));
            }
        }
        let mut made = block.clone();
        made.insns = insns;
        blocks.push(made);
    }
    let mut out = body.clone();
    out.blocks = blocks;
    Ok(out)
}

const _ARITHMETIC: [&str; 4] = ["fadd", "fsub", "fmul", "fdiv"];
// `left op right` with the operands' places swapped: `st(i) := st(0) - st(i)` is `fsubr st(i),st(0)`.
const _REVERSED: [(&str, &str); 4] = [("fadd", "fadd"), ("fmul", "fmul"), ("fsub", "fsubr"), ("fdiv", "fdivr")];

fn _reversed(name: &str) -> &'static str {
    _REVERSED.iter().find(|(key, _)| *key == name).map(|(_, value)| *value).expect("_REVERSED[name]")
}

fn _floating(arg: &Loc) -> bool {
    matches!(arg, Loc::Held(held) if held.width == 10)
}

fn _loads_memory(what: Option<&Semantics>) -> bool {
    what.is_some_and(|what| {
        what.op == Operation::FloatLoad
            && (name_is(what, "fld") || name_is(what, "fild"))
            && what.sources.len() == 1
            && matches!(what.sources[0], Loc::Mem(_))
            && what.dests.len() == 1
            && matches!(what.dests[0], Loc::Held(_))
    })
}

/// Arithmetic on two floating values, as `result := left name right` with name in _ARITHMETIC.
fn _two_values(what: Option<&Semantics>) -> Option<(String, Held, Held)> {
    let what = what?;
    if !matches!(what.op, Operation::FloatArith | Operation::FloatArithPop)
        || what.sources.len() != 2
        || !what.sources.iter().all(_floating)
    {
        return None;
    }
    let full = what.name.as_deref()?;
    let name = if what.op == Operation::FloatArithPop { &full[..full.len() - 1] } else { full };
    let (Loc::Held(left), Loc::Held(right)) = (&what.sources[0], &what.sources[1]) else {
        unreachable!("both sources are floating")
    };
    if name == "fsubr" || name == "fdivr" {
        return Some((name[..name.len() - 1].to_owned(), *right, *left));
    }
    _ARITHMETIC.contains(&name).then(|| (name.to_owned(), *left, *right))
}

/// The CFG blocks execution can enter from this body's entry.
///
/// Stack state is a property of an executed edge.  A syntactic predecessor
/// in dead code cannot arrive at a join or require an x87 bridge; treating
/// it as one made an otherwise straight live edge spill an extended value.
/// Keep the dead block for layout and ordinary emission, but exclude its
/// edges from the allocator's live control-flow facts.
fn _reachable_blocks(blocks: &[LirBlock], entry: i64) -> HashSet<i64> {
    let at_of: HashMap<i64, &LirBlock> = blocks.iter().map(|block| (block.at, block)).collect();
    let (mut reached, mut pending) = (HashSet::default(), vec![entry]);
    while let Some(at) = pending.pop() {
        if reached.contains(&at) || !at_of.contains_key(&at) {
            continue;
        }
        reached.insert(at);
        pending.extend(&at_of[&at].succ);
    }
    reached
}

/// The instruction computing `name` with one operand read from `load`'s cell, if x87 has one.
fn _memory_name(name: &str, cell_is_left: bool, load: &Insn) -> Option<String> {
    let mut name = if cell_is_left { _reversed(name) } else { name }.to_owned();
    if name_is(load.what.as_ref().expect("a home load has semantics"), "fild") {
        name = format!("fi{}", &name[1..]);
    }
    select::float_memory(&name, cell_of(load), 0).is_some().then_some(name)
}

/// Whether the cell's address is made of SSA values, so every read of it names the same bytes.
fn _stable(cell: &Mem) -> bool {
    match cell.addr {
        None => return false,
        Some(addr) if addr.space == Space::Far && cell.selector.is_none() => return false,
        Some(_) => {}
    }
    cell.base.is_some() || cell.index.is_some() || matches!(cell.through, Register::None | Register::BP)
}

fn _reached(cell: &Mem) -> Option<Addr> {
    // regions reads a based address as reaching its whole region.
    let indexed = cell.base.is_some() || cell.index.is_some();
    if !indexed {
        return cell.addr;
    }
    let addr = cell.addr.expect("replace(None) raises");
    let base = if addr.base != Register::None {
        addr.base
    } else if cell.through != Register::None {
        cell.through
    } else {
        Register::SI
    };
    Some(Addr { base, ..addr })
}

fn _may_write(one: &Insn, cell: &Mem) -> bool {
    let Some(what) = &one.what else {
        return true;
    };
    if what.op == Operation::Fill {
        return true;
    }
    if what.dests.iter().any(|dest| {
        matches!(dest, Loc::Mem(dest) if dest.addr.is_none()
            // Rust-only endpoint overflow reads as "may overlap".
            || regions::addresses(_reached(dest), dest.width, _reached(cell), cell.width, None).unwrap_or(true))
    }) {
        return true;
    }
    let address: HashSet<u32> = [cell.base, cell.index, cell.selector].into_iter().flatten().map(|arg| arg.value).collect();
    one.defines.iter().any(|value| address.contains(value))
}

/// Whether the instruction can raise anything but a load's invalid operand.
///
/// Loads are left out: two reads that can each raise only invalid operation
/// raise the same thing in either order.
fn _may_raise(one: &Insn) -> bool {
    let Some(what) = &one.what else {
        return true;
    };
    match what.op {
        Operation::FloatArith | Operation::FloatArithPop | Operation::FloatUnary | Operation::Divide => true,
        Operation::FloatStore => {
            let extended = what.dests.iter().all(|dest| matches!(dest, Loc::Mem(dest) if dest.width == 10));
            !(name_is(what, "fstp") && extended)
        }
        Operation::Nothing => name_is(what, "wait"),
        _ => false,
    }
}

/// Whether an x87 store wrote the cell last, so it holds no signalling NaN.
fn _quiet(sequence: &[Arc<Insn>], position: usize, cell: &Mem) -> bool {
    for one in sequence[..position].iter().rev() {
        let what = one.what.as_ref().expect("a region instruction has semantics");
        if what.op == Operation::FloatStore
            && (name_is(what, "fstp") || name_is(what, "fst"))
            && matches!(what.dests.as_slice(), [Loc::Mem(dest)] if dest == cell)
        {
            return true;
        }
        if _may_write(one, cell) {
            return false;
        }
    }
    false
}

/// Whether the load at `position` may be read again by each reader instead of held on the stack.
///
/// GCC's memory equivalence: a value that is a cell nothing writes before its
/// last reader is that cell. Its first read moves to its first reader, so
/// nothing that can raise may come between unless the read cannot.
fn _rereadable(sequence: &[Arc<Insn>], position: usize, reads: &VecDeque<i64>) -> bool {
    let load = &sequence[position];
    if !_loads_memory(load.what.as_ref()) || !load.delivers.is_empty() || reads.is_empty() {
        return false;
    }
    let cell = cell_of(load);
    if !_stable(cell) {
        return false;
    }
    let what = load.what.as_ref().expect("checked above");
    let mut quiet = (name_is(what, "fild") || cell.width == 10).then_some(true);
    let last = *reads.back().expect("checked above") as usize;
    for step in position + 1..last {
        let one = &sequence[step];
        if _may_write(one, cell) {
            return false;
        }
        if (step as i64) < reads[0] && _may_raise(one) {
            if quiet.is_none() {
                quiet = Some(_quiet(sequence, position, cell));
            }
            if quiet != Some(true) {
                return false;
            }
        }
    }
    true
}

/// Map each repeated stable x87 cell read to the first still-current value.
///
/// Lowering names every ``fld`` with a fresh SSA value.  That is right at
/// the MIR/LIR boundary, but loses the fact that two loads of the same stable
/// cell, with no intervening write, read one x87 value.  Keep that fact here,
/// where the stack allocator can decide whether retaining the value costs
/// less than rereading it.  A call, opaque instruction, address redefinition,
/// or possibly-aliasing store invalidates the remembered cell through
/// ``_may_write``; this is deliberately the same memory proof used by the
/// existing memory-operand reuse path.
fn _equivalent_loads(sequence: &[Arc<Insn>]) -> IndexMap<u32, u32> {
    let mut available: IndexMap<(Option<String>, Mem), u32> = IndexMap::default();
    let mut aliases: IndexMap<u32, u32> = IndexMap::default();
    for one in sequence {
        // A volatile load is observable and may be backed by changing device
        // state.  It must neither be removed nor let an earlier ordinary read
        // stand for a later one.
        if one.op.as_ref().is_some_and(|op| op.volatile) {
            available.clear();
            continue;
        }
        available.retain(|key, _| !_may_write(one, &key.1));
        let what = one.what.as_ref();
        if !_loads_memory(what) || !one.delivers.is_empty() {
            continue;
        }
        let what = what.expect("checked above");
        let (Loc::Mem(cell), Loc::Held(result)) = (&what.sources[0], &what.dests[0]) else {
            unreachable!("checked above")
        };
        // An m80 value is the allocator's extended-precision spill format,
        // and distinct loads can deliberately denote distinct stack values.
        // Rounded scalar cells and integer conversions are ordinary source
        // memory values, so their unchanged reloads are equivalent.
        if cell.width == 10 || !_stable(cell) {
            continue;
        }
        let key = (what.name.clone(), cell.clone());
        if let Some(first) = available.get(&key) {
            aliases.insert(result.value, *first);
        } else {
            available.insert(key, result.value);
        }
    }
    aliases
}

type Region = (Vec<Arc<Insn>>, IndexMap<u32, VecDeque<i64>>, IndexMap<u32, u32>);

/// The instructions from here to the region's end, and where each floating value is read among them.
fn _region(blocks: &[LirBlock], mut index: usize, mut offset: usize, continues: &HashSet<usize>) -> Region {
    let finished = |sequence: Vec<Arc<Insn>>| -> Region {
        let aliases = _equivalent_loads(&sequence);
        let mut reads: IndexMap<u32, VecDeque<i64>> = IndexMap::default();
        for (position, instruction) in sequence.iter().enumerate() {
            for arg in &instruction.what.as_ref().expect("a region instruction has semantics").sources {
                if let Loc::Held(arg) = arg {
                    if arg.width == 10 {
                        reads.entry(*aliases.get(&arg.value).unwrap_or(&arg.value)).or_default().push_back(position as i64);
                    }
                }
            }
        }
        (sequence, reads, aliases)
    };
    let mut sequence = Vec::new();
    loop {
        for instruction in &blocks[index].insns[offset..] {
            if boundary(instruction) {
                return finished(sequence);
            }
            sequence.push(Arc::clone(instruction));
        }
        if !continues.contains(&index) {
            return finished(sequence);
        }
        (index, offset) = (index + 1, 0);
    }
}

/// The x87 register stack across one region, after LLVM's X86FloatingPoint.
///
/// An operand an instruction consumes dies there, and the result takes its
/// slot. A value loaded from a cell is not held at all while the cell stays
/// unwritten: each reader takes the cell as its memory operand or reloads it.
struct _Stack<'f, 'c> {
    frame: Option<&'f mut Frame>,
    floating: HashSet<u32>,
    cpu: &'c Profile,
    retain_homes: bool,
    values: Vec<u32>,                 // top first
    home: IndexMap<u32, Arc<Insn>>,   // the load that reads a value again
    defined: HashMap<u32, i64>,
    sequence: Vec<Arc<Insn>>,
    reads: IndexMap<u32, VecDeque<i64>>,
    aliases: IndexMap<u32, u32>,
    here: i64,
    out: Vec<Arc<Insn>>,
    one: Option<Arc<Insn>>,
    keep: HashSet<u32>,
    retained: HashSet<u32>,
}

impl<'f, 'c> _Stack<'f, 'c> {
    fn new(frame: Option<&'f mut Frame>, floating: HashSet<u32>, cpu: &'c Profile, retain_homes: bool) -> Self {
        Self {
            frame,
            floating,
            cpu,
            retain_homes,
            values: Vec::new(),
            home: IndexMap::default(),
            defined: HashMap::default(),
            sequence: Vec::new(),
            reads: IndexMap::default(),
            aliases: IndexMap::default(),
            here: -1,
            out: Vec::new(),
            one: None,
            keep: HashSet::default(),
            retained: HashSet::default(),
        }
    }

    fn one(&self) -> &Arc<Insn> {
        self.one.as_ref().expect("an instruction is being allocated")
    }

    fn region(&mut self, (sequence, reads, aliases): Region) {
        (self.sequence, self.reads, self.here) = (sequence, reads, -1);
        self.aliases = aliases;
        self.home.clear();
        self.retained.clear();
    }

    fn canonical(&self, mut value: u32) -> u32 {
        while let Some(next) = self.aliases.get(&value) {
            value = *next;
        }
        value
    }

    /// Replace a repeated direct cell read with its canonical stack value.
    fn semantics(&self, what: &Semantics) -> Semantics {
        let sources = what
            .sources
            .iter()
            .map(|arg| match arg {
                Loc::Held(held) if held.width == 10 => Loc::Held(Held { value: self.canonical(held.value), ..*held }),
                _ => arg.clone(),
            })
            .collect();
        Semantics { sources, ..what.clone() }
    }

    /// Where the value is read after this instruction.
    fn pending(&mut self, value: u32) -> VecDeque<i64> {
        let key = self.canonical(value);
        let here = self.here;
        let reads = self.reads.entry(key).or_default();
        while reads.front().is_some_and(|first| *first <= here) {
            reads.pop_front();
        }
        reads.clone()
    }

    /// Whether a stack copy of the value is read after this instruction.
    fn survives(&mut self, value: u32) -> bool {
        let value = self.canonical(value);
        if self.retained.contains(&value) && self.values.contains(&value) {
            return !self.pending(value).is_empty();
        }
        if !self.home.contains_key(&value) {
            return !self.pending(value).is_empty();
        }
        !self.pending(value).iter().all(|step| self._reads_cell(value, *step))
    }

    fn _reads_cell(&self, value: u32, step: i64) -> bool {
        let Some((name, left, right)) = _two_values(self.sequence[step as usize].what.as_ref()) else {
            return false;
        };
        let (left, right) = (self.canonical(left.value), self.canonical(right.value));
        left != right && _memory_name(&name, left == value, &self.home[&value]).is_some()
    }

    /// The load, register-operation, and memory-operation costs for ``name``.
    fn arithmetic_costs(&self, name: &str) -> Option<(i64, i64, i64)> {
        let base = match name {
            "fadd" | "fsub" => "x87_add",
            "fmul" => "x87_mul",
            "fdiv" => "x87_div",
            _ => panic!("KeyError: {name}"),
        };
        let memory = format!("{base}_m");
        if !["x87_load", base, memory.as_str()].iter().all(|form| self.cpu.prices(form)) {
            return None;
        }
        let cost = |form: &str| self.cpu.cost(form).expect("priced above");
        Some((cost("x87_load"), cost(base), cost(&memory)))
    }

    /// Whether a cell arithmetic form costs no more than loading it into x87.
    ///
    /// The stack form has one explicit ``fld`` and a register arithmetic;
    /// the direct form combines those two effects.  If the register operand
    /// survives, however, the direct form also needs ``fld st(i)`` to keep a
    /// copy, while loading the dying cell operand lets the result overwrite
    /// it. A profile without the form-specific prices keeps the historic
    /// legal memory folding policy only when that preservation copy is not
    /// required, rather than treating unavailable data as a zero-cost form.
    fn memory_arithmetic(&self, name: &str, preserve_kept: bool) -> bool {
        let Some((load, register, memory)) = self.arithmetic_costs(name) else {
            return !preserve_kept;
        };
        memory + i64::from(preserve_kept) * load <= load + register
    }

    /// Whether keeping a rereadable home on x87 is cheaper than using the home.
    ///
    /// This compares the complete remaining arithmetic use set.  A self-use
    /// needs a stack duplicate when the value remains live; an ordinary use
    /// can instead load its dying peer and overwrite that peer.  Only fully
    /// priced arithmetic-only use sets are candidates, and two spare stack
    /// positions are required so the choice cannot manufacture a spill.
    fn retain_home(&mut self, value: u32) -> bool {
        if !self.retain_homes || self.values.len() > 6 {
            return false;
        }
        // Order-free: `pending` only drops reads already behind `here`.
        let retained: Vec<u32> = self.retained.iter().copied().collect();
        for one in retained {
            if self.values.contains(&one) && !self.pending(one).is_empty() {
                return false;
            }
        }
        let positions: Vec<i64> = self.pending(value).into_iter().collect::<IndexSet<i64>>().into_iter().collect();
        if positions.is_empty() {
            return false;
        }
        let mut costs_by_step = Vec::new();
        for (ordinal, step) in positions.iter().enumerate() {
            let Some((name, left, right)) = _two_values(self.sequence[*step as usize].what.as_ref()) else {
                return false;
            };
            let (left, right) = (self.canonical(left.value), self.canonical(right.value));
            if value != left && value != right {
                return false;
            }
            // Retaining several ordinary operands whose live intervals
            // overlap can make each look profitable alone while forcing
            // exchanges between them.  Start only a retention interval at a
            // self-use, whose unavoidable duplicate is completely costed;
            // later ordinary uses can then consume dying peers around it.
            if ordinal == 0 && left != right {
                return false;
            }
            let Some((load, register, memory)) = self.arithmetic_costs(&name) else {
                return false;
            };
            let (home, kept);
            if left == right {
                home = load + register;
                kept = register + if Some(step) != positions.last() { load } else { 0 };
            } else {
                let operation = _memory_name(&name, left == value, &self.home[&value]);
                home = if operation.is_some() { memory.min(load + register) } else { load + register };
                kept = register;
            }
            costs_by_step.push((home, kept));
        }
        let load = self.cpu.cost("x87_load").expect("priced by arithmetic_costs");
        let home_cost: i64 = costs_by_step.iter().map(|(home, _)| home).sum();
        let retained_cost = load + costs_by_step.iter().map(|(_, kept)| kept).sum::<i64>();
        retained_cost < home_cost
    }

    fn insert(&mut self, what: Semantics) {
        let at = self.one().at;
        self.out.push(Arc::new(inserted(at, what)));
    }

    /// `emit(what, *, uses=(), widths=(), **changes)`: `requires` and
    /// `symbol` are the only changes callers pass.
    fn emit(
        &mut self,
        what: Semantics,
        uses: &[u32],
        widths: &[(u32, u32)],
        requires: Option<Vec<(Held, Register)>>,
        symbol: Option<Option<bool>>,
    ) {
        let one = self.one();
        let mut made = (**one).clone();
        made.what = Some(what);
        made.uses = one
            .uses
            .iter()
            .chain(uses)
            .copied()
            .filter(|value| !self.floating.contains(value))
            .collect::<IndexSet<u32>>()
            .into_iter()
            .collect();
        made.defines = one.defines.iter().copied().filter(|value| !self.floating.contains(value)).collect();
        made.widths = one
            .widths
            .iter()
            .chain(widths)
            .copied()
            .filter(|pair| !self.floating.contains(&pair.0))
            .collect::<IndexSet<(u32, u32)>>()
            .into_iter()
            .collect();
        if let Some(requires) = requires {
            made.requires = requires;
        }
        if let Some(symbol) = symbol {
            made.symbol = symbol;
        }
        self.out.push(Arc::new(made));
    }

    /// Nothing is emitted here, and the bytes are still accounted for.
    fn vacate(&mut self) {
        let one = self.one();
        if one.covers.is_some_and(|(start, end)| start != end) {
            let mut made = (**one).clone();
            made.what = Some(semantics(Operation::Nothing, "", Vec::new(), Vec::new()));
            made.uses = Vec::new();
            made.defines = Vec::new();
            made.widths = Vec::new();
            made.requires = Vec::new();
            self.out.push(Arc::new(made));
        }
    }

    fn exchange(&mut self, slot: usize) {
        if slot != 0 {
            let operands = vec![st(0), st(slot)];
            self.insert(semantics(Operation::Exchange, "fxch", operands.clone(), operands));
            self.values.swap(0, slot);
        }
    }

    fn room(&mut self, count: usize) -> Result<(), Raised> {
        while self.values.len() + count > 8 {
            if self.frame.is_none() {
                return Err(unlowered("floating spill requires an owned frame"));
            }
            // Python's `max`: the first slot with the latest next read.
            let mut victim: Option<(usize, i64)> = None;
            for slot in 0..self.values.len() {
                let value = self.values[slot];
                if self.keep.contains(&value) {
                    continue;
                }
                let key = self.pending(value).front().copied().unwrap_or(i64::MAX);
                if victim.is_none_or(|(_, best)| key > best) {
                    victim = Some((slot, key));
                }
            }
            let Some((victim, _)) = victim else {
                return Err(unlowered("floating instruction requires too many stack operands"));
            };
            self.exchange(victim);
            let value = self.values.remove(0);
            let cell = self.frame.as_deref_mut().expect("checked above").cell(("floating", i64::from(value)), 10)?;
            self.insert(semantics(Operation::FloatStore, "fstp", vec![Loc::Mem(cell.clone())], vec![st(0)]));
            let at = self.one().at;
            let load = semantics(Operation::FloatLoad, "fld", vec![Loc::Held(Held { value, width: 10 })], vec![Loc::Mem(cell)]);
            self.home.insert(value, Arc::new(inserted(at, load)));
        }
        Ok(())
    }

    fn duplicate(&mut self, value: u32) -> Result<(), Raised> {
        self.room(1)?;
        let index = index_of(&self.values, value);
        self.insert(semantics(Operation::FloatLoad, "fld", vec![st(0)], vec![st(index)]));
        self.values.insert(0, value);
        Ok(())
    }

    fn materialize(&mut self, value: u32) -> Result<(), Raised> {
        if !self.home.contains_key(&value) {
            return Err(unlowered("floating stack input is unavailable"));
        }
        self.room(1)?;
        let (load, at) = (Arc::clone(&self.home[&value]), self.one().at);
        let mut made = (*load).clone();
        made.at = at;
        made.covers = Some((at, at));
        made.what = Some(Semantics { dests: vec![st(0)], ..load.what.clone().expect("a home load has semantics") });
        made.defines = Vec::new();
        made.uses = load.uses.iter().copied().filter(|one| !self.floating.contains(one)).collect();
        made.widths = load.widths.iter().copied().filter(|pair| !self.floating.contains(&pair.0)).collect();
        made.spread = Vec::new();
        made.symbol = None;
        self.out.push(Arc::new(made));
        self.values.insert(0, value);
        Ok(())
    }

    fn top(&mut self, value: u32) -> Result<(), Raised> {
        if !self.values.contains(&value) {
            self.materialize(value)?;
        }
        self.exchange(index_of(&self.values, value));
        Ok(())
    }

    fn allocate(&mut self, one: &Arc<Insn>) -> Result<(), Raised> {
        if !Arc::ptr_eq(&self.sequence[self.here as usize], one) {
            return Err(unlowered("floating region positions disagree"));
        }
        self.one = Some(Arc::clone(one));
        let original = one.what.as_ref().expect("a floating instruction has semantics");
        let what = self.semantics(original);
        let held = |args: &[Loc]| -> Vec<u32> {
            args.iter()
                .filter_map(|arg| match arg {
                    Loc::Held(held) if held.width == 10 => Some(held.value),
                    _ => None,
                })
                .collect()
        };
        let operands = held(&what.sources);
        let results = held(&what.dests);
        self.keep = operands.iter().copied().collect();
        if _loads_memory(Some(original)) && results.len() == 1 && self.canonical(results[0]) != results[0] {
            self.vacate();
            return Ok(());
        }
        if results.len() > 1 || results.iter().any(|result| self.values.contains(result)) {
            return Err(unlowered("floating stack result is not a fresh value"));
        }
        for result in &results {
            self.defined.insert(*result, self.here);
        }
        let two = _two_values(Some(&what));
        if let (Some((name, left, right)), false) = (&two, results.is_empty()) {
            self.arithmetic(name, *left, *right, results[0])?;
        } else if what.op == Operation::FloatLoad && unnamed(&what) && what.sources.is_empty() && !results.is_empty() {
            // A call's result, which it left in st(0).
            if !self.values.is_empty() {
                return Err(unlowered("a call's floating result arrives on a stack that is not empty"));
            }
            self.values.insert(0, results[0]);
            self.vacate();
        } else if what.op == Operation::FloatStore && unnamed(&what) && what.dests.is_empty() && operands.len() == 1 {
            // A returned value, left in st(0) for the caller.
            self.top(operands[0])?;
            if self.values.len() != 1 {
                return Err(unlowered("a returned float leaves other values on the stack"));
            }
            self.values.remove(0);
            self.vacate();
        } else if what.op == Operation::Compare && operands.len() == 2 && results.is_empty() {
            self.compare(operands[0], operands[1])?;
        } else if what.op == Operation::FloatLoad && operands.is_empty() && !results.is_empty() {
            let reads = self.pending(results[0]);
            if _rereadable(&self.sequence, self.here as usize, &reads) {
                self.home.insert(results[0], Arc::clone(one));
                if self.retain_home(results[0]) {
                    self.retained.insert(results[0]);
                    self.room(1)?;
                    self.emit(Semantics { dests: vec![st(0)], ..what }, &[], &[], None, None);
                    self.values.insert(0, results[0]);
                } else {
                    self.vacate();
                }
            } else {
                self.room(1)?;
                self.emit(Semantics { dests: vec![st(0)], ..what }, &[], &[], None, None);
                self.values.insert(0, results[0]);
            }
        } else if what.op == Operation::FloatLoad && operands.len() == 1 && !results.is_empty() {
            self.copy(operands[0], results[0])?;
        } else if what.op == Operation::FloatStore && operands.len() == 1 && results.is_empty() {
            self.store(operands[0])?;
        } else if what.op == Operation::FloatUnary && operands.len() == 1 && !results.is_empty() {
            let what = Semantics { dests: vec![st(0)], sources: vec![st(0)], ..what };
            self.consume(operands[0], what, results[0])?;
        } else if what.op == Operation::FloatArith && !operands.is_empty() && _floating(&what.sources[0]) && !results.is_empty() {
            let Loc::Held(kept) = what.sources[0] else { unreachable!("checked above") };
            let sources = std::iter::once(st(0)).chain(what.sources[1..].iter().cloned()).collect();
            self.consume(kept.value, Semantics { dests: vec![st(0)], sources, ..what }, results[0])?;
        } else {
            return Err(unlowered("floating instruction has no allocation rule"));
        }
        Ok(())
    }

    fn copy(&mut self, source: u32, result: u32) -> Result<(), Raised> {
        if self.values.contains(&source) && !self.survives(source) {
            // GCC's move_for_stack_reg: a source dying here is renamed, not copied.
            let index = index_of(&self.values, source);
            self.values[index] = result;
            self.vacate();
            return Ok(());
        }
        if !self.values.contains(&source) {
            self.materialize(source)?;
            self.values[0] = result;
            self.vacate();
            return Ok(());
        }
        self.room(1)?;
        let what = self.one().what.clone().expect("a floating instruction has semantics");
        let index = index_of(&self.values, source);
        self.emit(Semantics { dests: vec![st(0)], sources: vec![st(index)], ..what }, &[], &[], None, None);
        self.values.insert(0, result);
        Ok(())
    }

    fn store(&mut self, source: u32) -> Result<(), Raised> {
        let what = self.one().what.clone().expect("a floating instruction has semantics");
        self.top(source)?;
        let name = if name_is(&what, "fst") { Some("fstp".to_owned()) } else { what.name.clone() };
        if self.survives(source) {
            if name.as_deref() == Some("fstp")
                && what.dests.iter().all(|dest| matches!(dest, Loc::Mem(dest) if dest.width == 4 || dest.width == 8))
            {
                self.emit(
                    Semantics { name: Some("fst".to_owned()), sources: vec![st(0)], ..what },
                    &[],
                    &[],
                    None,
                    None,
                );
                return Ok(());
            }
            self.duplicate(source)?;
        }
        self.emit(Semantics { name, sources: vec![st(0)], ..what }, &[], &[], None, None);
        self.values.remove(0);
        Ok(())
    }

    /// `left` against `right`, the answer moved from the status word into the flags.
    fn compare(&mut self, left: u32, right: u32) -> Result<(), Raised> {
        let load = self.home.get(&right).cloned();
        let fused = load.as_ref().filter(|load| {
            right != left
                && !self.values.contains(&right)
                && name_is(load.what.as_ref().expect("a home load has semantics"), "fld")
                && select::float_memory("fcomp", cell_of(load), 0).is_some()
        });
        if let Some(load) = fused.cloned() {
            self.top(left)?;
            if self.survives(left) {
                self.duplicate(left)?;
            }
            let one = Arc::clone(self.one());
            let covers = one.covers;
            let requires = load.requires.iter().chain(&one.requires).copied().collect::<IndexSet<_>>().into_iter().collect();
            self.emit(
                semantics(Operation::Compare, "fcomp", Vec::new(), vec![st(0), Loc::Mem(cell_of(&load).clone())]),
                &load.uses,
                &load.widths,
                Some(requires),
                Some(if covers.is_some_and(|(start, end)| start != end) { Some(false) } else { one.symbol }),
            );
            self.values.remove(0);
        } else {
            let both: PySet<i64> = [i64::from(left), i64::from(right)].into_iter().collect();
            let missing: PySet<i64> = both.iter().copied().filter(|value| !self.values.contains(&(*value as u32))).collect();
            let mut missing: Vec<u32> = missing.iter().map(|value| *value as u32).collect();
            missing.sort_by_key(|value| self.defined.get(value).copied().unwrap_or(-1));
            for value in missing {
                self.materialize(value)?;
            }
            // Left on top and right beneath it, each a copy where it is read again.
            if self.survives(left) {
                self.duplicate(left)?;
            } else {
                self.top(left)?;
            }
            if right == left || self.survives(right) {
                self.duplicate(right)?;
                self.exchange(1);
            } else if index_of(&self.values, right) != 1 {
                let slot = index_of(&self.values, right);
                self.exchange(slot);
                self.exchange(1);
                self.exchange(slot);
            }
            self.emit(semantics(Operation::Compare, "fcompp", Vec::new(), vec![st(0), st(1)]), &[], &[], None, None);
            self.values.drain(..2);
        }
        // The comparison defines flags through SAHF. A raised runtime helper
        // may additionally expose AX as an opaque clobber to later ABI code;
        // FNSTSW is the instruction that produces that value, not FCOM(PP).
        let one = Arc::clone(self.one());
        let produced: HashSet<u32> = one.defines.iter().copied().collect();
        let comparison = self.out.last().expect("a comparison was emitted");
        let mut made = (**comparison).clone();
        made.defines = comparison.defines.iter().copied().filter(|value| !produced.contains(value)).collect();
        made.delivers = comparison.delivers.iter().copied().filter(|(held, _)| !produced.contains(&held.value)).collect();
        made.widths = comparison.widths.iter().copied().filter(|pair| !produced.contains(&pair.0)).collect();
        *self.out.last_mut().expect("a comparison was emitted") = Arc::new(made);
        let at = one.at;
        let status = semantics(Operation::Barrier, "fnstsw", vec![Loc::Reg(Reg { register: Register::AX, width: 2 })], Vec::new());
        let mut word = Insn::new(at, Some((at, at)), Some(status), one.defines.clone(), Vec::new());
        word.delivers = one.delivers.clone();
        word.widths = one.widths.iter().copied().filter(|pair| produced.contains(&pair.0)).collect();
        self.out.push(Arc::new(word));
        self.insert(semantics(Operation::Nothing, "sahf", Vec::new(), Vec::new()));
        Ok(())
    }

    /// An instruction replacing the top with its result.
    fn consume(&mut self, source: u32, what: Semantics, result: u32) -> Result<(), Raised> {
        self.top(source)?;
        if self.survives(source) {
            self.duplicate(source)?;
        }
        self.emit(what, &[], &[], None, None);
        self.values[0] = result;
        Ok(())
    }

    fn arithmetic(&mut self, name: &str, left: Held, right: Held, result: u32) -> Result<(), Raised> {
        let (left, right) = (left.value, right.value);
        let mut cells: Vec<(u32, u32)> = [(right, left), (left, right)]
            .into_iter()
            .filter(|(cell, kept)| {
                cell != kept
                    && !self.values.contains(cell)
                    && self.home.contains_key(cell)
                    && _memory_name(name, *cell == left, &self.home[cell]).is_some()
            })
            .collect();
        // Both in their cells: the later one is the memory operand, so the loads keep their order.
        cells.sort_by_key(|pair| -self.defined.get(&pair.0).copied().unwrap_or(-1));
        if !cells.is_empty() && {
            let preserve_kept = self.survives(cells[0].1);
            self.memory_arithmetic(name, preserve_kept)
        } {
            let (cell, kept) = cells[0];
            let (load, covers) = (Arc::clone(&self.home[&cell]), self.one().covers);
            let operation = _memory_name(name, cell == left, &load).expect("checked above");
            self.top(kept)?;
            if self.survives(kept) {
                self.duplicate(kept)?;
            }
            let one = Arc::clone(self.one());
            let requires = load.requires.iter().chain(&one.requires).copied().collect::<IndexSet<_>>().into_iter().collect();
            self.emit(
                semantics(Operation::FloatArith, &operation, vec![st(0)], vec![st(0), Loc::Mem(cell_of(&load).clone())]),
                &load.uses,
                &load.widths,
                Some(requires),
                // The operand is another instruction's: its fixup is bound by address, not by this one's record.
                Some(if covers.is_some_and(|(start, end)| start != end) { Some(false) } else { one.symbol }),
            );
            self.values[0] = result;
            return Ok(());
        }
        let both: PySet<i64> = [i64::from(left), i64::from(right)].into_iter().collect();
        let missing: PySet<i64> = both.iter().copied().filter(|value| !self.values.contains(&(*value as u32))).collect();
        let mut missing: Vec<u32> = missing.iter().map(|value| *value as u32).collect();
        missing.sort_by_key(|value| self.defined.get(value).copied().unwrap_or(-1));
        for value in missing {
            self.materialize(value)?;
        }
        let (mut dies_left, dies_right) = (!self.survives(left), !self.survives(right));
        if left == right {
            self.exchange(index_of(&self.values, left));
            let mut slot = 0;
            if !dies_left {
                self.duplicate(left)?;
                // Two copies of one value: the result takes the slot that leaves the sooner read on top.
                let (later, product) = (self.pending(left), self.pending(result));
                slot = if !later.is_empty() && !product.is_empty() && later[0] < product[0] { 1 } else { 0 };
            }
            let operands = if dies_left { vec![st(0), st(0)] } else { vec![st(slot), st(1 - slot)] };
            self.emit(semantics(Operation::FloatArith, name, vec![st(slot)], operands), &[], &[], None, None);
            self.values[slot] = result;
            return Ok(());
        }
        // LLVM's handleTwoArgFP: a dying operand goes on top so the result can overwrite it.
        if self.values[0] != left && self.values[0] != right {
            if dies_left || dies_right {
                self.exchange(index_of(&self.values, if dies_left { left } else { right }));
            } else {
                self.duplicate(left)?;
                dies_left = true;
            }
        } else if !dies_left && !dies_right {
            self.duplicate(left)?;
            dies_left = true;
        }
        let forward = self.values[0] == left;
        let other = index_of(&self.values, if forward { right } else { left });
        if (forward && !dies_right) || (!forward && !dies_left) {
            let operation = if forward { name } else { _reversed(name) };
            self.emit(semantics(Operation::FloatArith, operation, vec![st(0)], vec![st(0), st(other)]), &[], &[], None, None);
            self.values[0] = result;
        } else if dies_left && dies_right {
            let operation = format!("{}p", if forward { _reversed(name) } else { name });
            self.emit(
                semantics(Operation::FloatArithPop, &operation, vec![st(other)], vec![st(other), st(0)]),
                &[],
                &[],
                None,
                None,
            );
            self.values[other] = result;
            self.values.remove(0);
        } else {
            let operation = if forward { _reversed(name) } else { name };
            self.emit(semantics(Operation::FloatArith, operation, vec![st(other)], vec![st(other), st(0)]), &[], &[], None, None);
            self.values[other] = result;
        }
        Ok(())
    }
}

/// The profile cost form of an allocated x87 instruction.
fn _floating_form(what: &Semantics) -> Option<String> {
    let name = what.name.as_deref().unwrap_or("");
    if what.op == Operation::FloatLoad {
        return Some("x87_load".to_owned());
    }
    if what.op == Operation::Exchange && name == "fxch" {
        return Some("x87_exchange".to_owned());
    }
    if what.op == Operation::FloatStore {
        return Some(if name.starts_with("fist") { "x87_convert_store" } else { "x87_store" }.to_owned());
    }
    if !matches!(what.op, Operation::FloatArith | Operation::FloatArithPop) {
        return None;
    }
    let name = name.strip_prefix("fi").unwrap_or(name);
    let name = name.strip_suffix('p').unwrap_or(name);
    let name = name.strip_suffix('r').unwrap_or(name);
    let base = match name {
        "fadd" | "fsub" => "x87_add",
        "fmul" => "x87_mul",
        "fdiv" => "x87_div",
        _ => return None,
    };
    Some(if what.sources.iter().any(|arg| matches!(arg, Loc::Mem(_))) { format!("{base}_m") } else { base.to_owned() })
}

type Candidate = (LirBody, IndexMap<i64, Vec<i64>>);

/// Allocate one complete stack candidate so its real shuffles can be priced.
fn _allocate_stack(
    body: &LirBody,
    frame: Option<&mut Frame>,
    floating: &HashSet<u32>,
    target: &Profile,
    continues: &HashSet<usize>,
    order: &[i64],
    retain_homes: bool,
) -> Result<Candidate, Raised> {
    let mut stack = _Stack::new(frame, floating.clone(), target, retain_homes);
    let mut blocks = Vec::new();
    let mut labels: IndexMap<i64, Vec<i64>> = IndexMap::default();
    let mut region = 0;
    for (index, block) in body.blocks.iter().enumerate() {
        if index == 0 || !continues.contains(&(index - 1)) {
            region += 1;
            stack.region(_region(&body.blocks, index, 0, continues));
        }
        stack.out = Vec::new();
        let mut output_regions = Vec::new();
        for (position, one) in block.insns.iter().enumerate() {
            let current_region = region;
            let before = stack.out.len();
            let mut begins_region = false;
            stack.here += 1;
            if one.what.as_ref().is_none_or(|what| !what.sources.iter().chain(&what.dests).any(_floating)) {
                if one.uses.iter().chain(&one.defines).any(|value| floating.contains(value)) {
                    return Err(unlowered("floating value used by an unmodelled instruction"));
                }
                if boundary(one) {
                    // bridged() gave every value read beyond here its own cell.
                    if !stack.values.is_empty() {
                        return Err(unlowered("floating stack crosses an unmodelled instruction"));
                    }
                    stack.region(_region(&body.blocks, index, position + 1, continues));
                    begins_region = true;
                }
                stack.out.push(Arc::clone(one));
            } else {
                stack.allocate(one)?;
            }
            output_regions.extend(std::iter::repeat_n(current_region, stack.out.len() - before));
            if begins_region {
                region += 1;
            }
        }
        if !stack.values.is_empty() && !continues.contains(&index) {
            return Err(unlowered("floating stack live-out requires cross-block allocation"));
        }
        let mut made = block.clone();
        made.insns = std::mem::take(&mut stack.out);
        blocks.push(made);
        labels.insert(block.at, output_regions);
    }
    let allocated_blocks: IndexMap<i64, LirBlock> = blocks.into_iter().map(|block| (block.at, block)).collect();
    let mut out = body.clone();
    out.blocks = order.iter().map(|at| allocated_blocks[at].clone()).collect();
    Ok((out, labels))
}

/// Target cost and instruction count for each independently empty-stack region.
fn _region_scores(
    body: &LirBody,
    labels: &IndexMap<i64, Vec<i64>>,
    target: &Profile,
) -> Result<IndexMap<i64, (i64, i64)>, Raised> {
    let mut costs: HashMap<i64, i64> = HashMap::default();
    let mut counts: IndexMap<i64, i64> = IndexMap::default();
    let mut unpriced: HashSet<i64> = HashSet::default();
    for block in &body.blocks {
        let regions = &labels[&block.at];
        if regions.len() != block.insns.len() {
            return Err(Raised::Value("x87 candidate region labels do not cover its instructions".to_owned()));
        }
        for (one, region) in block.insns.iter().zip(regions) {
            let emitted = one.what.as_ref().is_some_and(|what| !(what.op == Operation::Nothing && unnamed(what)));
            *counts.entry(*region).or_default() += i64::from(emitted);
            let Some(form) = one.what.as_ref().and_then(_floating_form) else {
                continue;
            };
            if target.prices(&form) {
                *costs.entry(*region).or_default() += target.cost(&form).expect("priced");
            } else {
                unpriced.insert(*region);
            }
        }
    }
    Ok(counts
        .iter()
        .map(|(region, count)| {
            let score = if unpriced.contains(region) { (*count, *count) } else { (costs.get(region).copied().unwrap_or(0), *count) };
            (*region, score)
        })
        .collect())
}

/// Choose the cheaper complete allocation independently at every empty stack.
fn _compose_regions(baseline: &Candidate, retained: &Candidate, target: &Profile) -> Result<LirBody, Raised> {
    let (baseline_body, baseline_labels) = baseline;
    let (retained_body, retained_labels) = retained;
    let baseline_scores = _region_scores(baseline_body, baseline_labels, target)?;
    let retained_scores = _region_scores(retained_body, retained_labels, target)?;
    let regions: HashSet<i64> = baseline_scores.keys().chain(retained_scores.keys()).copied().collect();
    let use_retained: HashSet<i64> = regions
        .into_iter()
        .filter(|region| {
            retained_scores.get(region).copied().unwrap_or((0, 0)) < baseline_scores.get(region).copied().unwrap_or((0, 0))
        })
        .collect();
    let retained_at: IndexMap<i64, &LirBlock> = retained_body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut blocks = Vec::new();
    for block in &baseline_body.blocks {
        let other = retained_at[&block.at];
        let mut baseline_groups: HashMap<i64, Vec<Arc<Insn>>> = HashMap::default();
        let mut retained_groups: HashMap<i64, Vec<Arc<Insn>>> = HashMap::default();
        for (one, region) in block.insns.iter().zip(&baseline_labels[&block.at]) {
            baseline_groups.entry(*region).or_default().push(Arc::clone(one));
        }
        for (one, region) in other.insns.iter().zip(&retained_labels[&block.at]) {
            retained_groups.entry(*region).or_default().push(Arc::clone(one));
        }
        let mut order: Vec<i64> = baseline_groups.keys().chain(retained_groups.keys()).copied().collect::<HashSet<i64>>().into_iter().collect();
        order.sort_unstable();
        let insns = order
            .iter()
            .flat_map(|region| {
                let groups = if use_retained.contains(region) { &retained_groups } else { &baseline_groups };
                groups.get(region).cloned().unwrap_or_default()
            })
            .collect();
        let mut made = block.clone();
        made.insns = insns;
        blocks.push(made);
    }
    let mut out = baseline_body.clone();
    out.blocks = blocks;
    Ok(out)
}

fn _floating_values(body: &LirBody) -> HashSet<u32> {
    body.blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter_map(|one| one.what.as_ref())
        .flat_map(|what| what.sources.iter().chain(&what.dests))
        .filter_map(|arg| match arg {
            Loc::Held(held) if held.width == 10 => Some(held.value),
            _ => None,
        })
        .collect()
}

pub fn allocated<'a>(
    body: &LirBody,
    mut frame: Option<&mut Frame>,
    basic_semantics: bool,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<LirBody, Raised> {
    let target = targets::profile(cpu).map_err(Raised::Value)?;
    let loaded = _integer_loads(body, frame.as_deref_mut())?;
    let mut body = _integer_stores(&loaded, frame.as_deref_mut(), basic_semantics)?;
    let floating = _floating_values(&body);
    if floating.is_empty() {
        return Ok(body);
    }
    let reachable = _reachable_blocks(&body.blocks, body.entry);
    let mut predecessors: IndexMap<i64, HashSet<i64>> = body.blocks.iter().map(|block| (block.at, HashSet::default())).collect();
    for block in &body.blocks {
        if !reachable.contains(&block.at) {
            continue;
        }
        for successor in &block.succ {
            if reachable.contains(successor) {
                predecessors.get_mut(successor).expect("a reachable block").insert(block.at);
            }
        }
    }
    let order: Vec<i64> = body.blocks.iter().map(|block| block.at).collect();
    let next_blocks: IndexMap<i64, i64> = body
        .blocks
        .iter()
        .filter(|block| {
            reachable.contains(&block.at)
                && block.succ.len() == 1
                && block.succ[0] != body.entry
                && reachable.contains(&block.succ[0])
                && predecessors.get(&block.succ[0]) == Some(&HashSet::from_iter([block.at]))
        })
        .map(|block| (block.at, block.succ[0]))
        .collect();
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let destinations: HashSet<i64> = next_blocks.values().copied().collect();
    let mut roots: Vec<i64> = order.iter().copied().filter(|at| reachable.contains(at) && !destinations.contains(at)).collect();
    roots.extend(order.iter().copied().filter(|at| !reachable.contains(at)));
    let (mut scheduled, mut seen): (Vec<LirBlock>, HashSet<i64>) = (Vec::new(), HashSet::default());
    for root in roots.iter().chain(&order) {
        let mut at = Some(*root);
        while let Some(here) = at {
            if seen.contains(&here) {
                break;
            }
            scheduled.push(at_of[&here].clone());
            seen.insert(here);
            at = next_blocks.get(&here).copied();
        }
    }
    body.blocks = scheduled;
    let continues: HashSet<usize> = body
        .blocks
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| next_blocks.get(&pair[0].at) == Some(&pair[1].at))
        .map(|(index, _)| index)
        .collect();

    // A region is keyed by instruction position: a phi is defined at -1 and
    // read at its predecessor's end.
    let (mut regions, mut region): (HashMap<(i64, i64), i64>, i64) = (HashMap::default(), 0);
    for (index, block) in body.blocks.iter().enumerate() {
        if index == 0 || !continues.contains(&(index - 1)) {
            region += 1;
        }
        regions.insert((block.at, -1), region);
        for (position, one) in block.insns.iter().enumerate() {
            region += i64::from(boundary(one));
            regions.insert((block.at, position as i64), region);
        }
        regions.insert((block.at, block.insns.len() as i64), region);
    }
    let body = bridged(&body, &regions, frame.as_deref_mut())?;
    if body.blocks.len() != order.len() {
        // Splitting critical edges changes the regions and their stack lifetimes.
        let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
        let mut replaced = body.clone();
        replaced.blocks = order
            .iter()
            .map(|at| by_at[at].clone())
            .chain(body.blocks.iter().filter(|block| !order.contains(&block.at)).cloned())
            .collect();
        return allocated(&replaced, frame, true, target);
    }
    let floating = _floating_values(&body);
    let baseline = _allocate_stack(&body, frame.as_deref_mut(), &floating, target, &continues, &order, false)?;
    let retained = _allocate_stack(&body, frame.as_deref_mut(), &floating, target, &continues, &order, true)?;
    let selected = _compose_regions(&baseline, &retained, target)?;
    _truncating(&selected, frame)
}


/// `fisttp` as a 387 has it: `fistp` with the control word set to round
/// toward zero and put back. After allocation, so the control-word barriers
/// split no region.
///
/// The caller's control word and its truncating form are saved once, at
/// entry: a callee leaves the control word as it found it, and nothing else
/// in the body writes it.
fn _truncating(body: &LirBody, frame: Option<&mut Frame>) -> Result<LirBody, Raised> {
    let fisttp = |one: &Insn| {
        one.what.as_ref().is_some_and(|what| what.op == Operation::FloatStore && name_is(what, "fisttp"))
    };
    if !body.insns().iter().any(|one| fisttp(one)) {
        return Ok(body.clone());
    }
    let Some(frame) = frame else {
        return Err(unlowered("rounding toward zero requires an owned frame"));
    };
    // Python keys these slots by the 1-tuples ("control",) and ("chop",).
    let (saved, chop) = (frame.cell(("control", 0), 2)?, frame.cell(("chop", 0), 2)?);
    let loaded_id = super::spiller::_next_value(body);
    let chopped_id = loaded_id + 1;
    let loaded = Held { value: loaded_id, width: 2 };
    let chopped = Held { value: chopped_id, width: 2 };

    let insn = |what: Semantics, at: i64| -> Arc<Insn> {
        let held = |args: &[Loc]| -> Vec<Held> {
            args.iter()
                .filter_map(|arg| match arg {
                    Loc::Held(held) => Some(*held),
                    _ => None,
                })
                .collect()
        };
        let defines = held(&what.dests).iter().map(|arg| arg.value).collect();
        let uses = held(&what.sources).iter().map(|arg| arg.value).collect();
        let widths = held(&what.dests).iter().chain(&held(&what.sources)).map(|arg| (arg.value, arg.width)).collect();
        let mut made = Insn::new(at, Some((at, at)), Some(what), defines, uses);
        made.widths = widths;
        Arc::new(made)
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        if block.at == body.entry {
            let at = block.insns.first().map_or(block.at, |first| first.at);
            insns.extend([
                insn(semantics(Operation::Barrier, "fnstcw", vec![Loc::Mem(saved.clone())], Vec::new()), at),
                insn(semantics(Operation::Move, "mov", vec![Loc::Held(loaded)], vec![Loc::Mem(saved.clone())]), at),
                insn(
                    semantics(
                        Operation::Binary,
                        "or",
                        vec![Loc::Held(chopped)],
                        vec![Loc::Held(loaded), Loc::Imm(Imm { value: 0x0C00, width: 2, address: None })],
                    ),
                    at,
                ),
                insn(semantics(Operation::Move, "mov", vec![Loc::Mem(chop.clone())], vec![Loc::Held(chopped)]), at),
            ]);
        }
        for one in &block.insns {
            if !fisttp(one) {
                insns.push(Arc::clone(one));
                continue;
            }
            let mut made = (**one).clone();
            made.what = Some(Semantics { name: Some("fistp".to_owned()), ..one.what.clone().expect("checked above") });
            insns.extend([
                insn(semantics(Operation::Barrier, "fldcw", Vec::new(), vec![Loc::Mem(chop.clone())]), one.at),
                Arc::new(made),
                insn(semantics(Operation::Barrier, "fldcw", Vec::new(), vec![Loc::Mem(saved.clone())]), one.at),
            ]);
        }
        let mut made = block.clone();
        made.insns = insns;
        blocks.push(made);
    }
    let mut out = body.clone();
    out.blocks = blocks;
    Ok(out)
}

/// Python holds the one mutable frame every machine phase shares.
pub struct FloatAlloc<'a> {
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub basic_semantics: bool,
    pub cpu: &'a Profile,
}

impl<'a> FloatAlloc<'a> {
    pub fn new(
        frame: Option<Rc<RefCell<Frame>>>,
        basic_semantics: bool,
        cpu: impl Into<ProfileOrName<'a>>,
    ) -> Result<Self, String> {
        Ok(Self { frame, basic_semantics, cpu: targets::profile(cpu)? })
    }
}

impl LIRTransform for FloatAlloc<'_> {
    fn class_name(&self) -> &'static str {
        "FloatAlloc"
    }

    fn name(&self) -> &str {
        "floatalloc"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        let mut frame = self.frame.as_ref().map(|frame| frame.borrow_mut());
        allocated(&body, frame.as_deref_mut(), self.basic_semantics, self.cpu).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "floatalloc_tests.rs"]
mod tests;
