//! Port of `qbopt/backend/floatalloc.py`: assign floating LIR values to the
//! target register stack.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use crate::support::hash::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};

use crate::analysis::regions;
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::allocate::{Live, live};
use crate::backend::floatregions::{Raised, boundary};
use crate::backend::spillplacement::{self, Border, Constraint};
use crate::backend::constpool::{self, Pool};
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
fn _integer_loads(body: &LirBody, mut frame: Option<&mut Frame>, mut pool: Option<&mut Pool>) -> Result<LirBody, Raised> {
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
                    if let (Loc::Imm(Imm { value, width, .. }), Some(pool)) = (&what.sources[0], pool.as_deref_mut()) {
                        let value = if *width == 2 { *value as i16 as i32 } else { *value as i32 };
                        let cell = pool.cell(constpool::narrowest(f64::from(value)));
                        let mut made = (*one).clone();
                        made.what = Some(Semantics { name: Some("fld".to_owned()), sources: vec![Loc::Mem(cell)], ..what });
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
/// A value read in a later block keeps its own name: the successor reads it.
fn _equivalent_loads(sequence: &[Arc<Insn>], live_out: &BTreeSet<u32>) -> IndexMap<u32, u32> {
    let mut available: IndexMap<(Option<String>, Mem), u32> = IndexMap::default();
    let mut aliases: IndexMap<u32, u32> = IndexMap::default();
    for one in sequence {
        // A volatile load is observable and may be backed by changing device
        // state.  It must neither be removed nor let an earlier ordinary read
        // stand for a later one.
        if one.volatile() {
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
        if let Some(first) = available.get(&key).filter(|_| !live_out.contains(&result.value)) {
            aliases.insert(result.value, *first);
        } else {
            available.insert(key, result.value);
        }
    }
    aliases
}

/// A stack slot whose value is overwritten: popped at once.
const DEAD: u32 = u32::MAX;

/// The x87 register stack through one block, after GCC's reg-stack and LLVM's X86FloatingPoint.
///
/// An operand an instruction consumes dies there, and the result takes its
/// slot. A value loaded from a cell is not held at all while the cell stays
/// unwritten: each reader takes the cell as its memory operand or reloads it.
/// A value no longer needed is popped where it dies.
struct _Stack<'f, 'c> {
    frame: Option<&'f mut Frame>,
    floating: HashSet<u32>,
    cpu: &'c Profile,
    retain_homes: bool,
    values: Vec<u32>,                 // top first
    home: IndexMap<u32, Arc<Insn>>,   // the load that reads a value again
    spills: IndexMap<u32, Arc<Insn>>, // each value's 8-byte cell, read back
    cells: IndexMap<u32, u32>,        // value -> the value whose cell it shares
    defined: HashMap<u32, i64>,
    sequence: Vec<Arc<Insn>>,
    reads: IndexMap<u32, VecDeque<i64>>,
    defs: IndexMap<u32, VecDeque<i64>>,
    live_out: BTreeSet<u32>,
    aliases: IndexMap<u32, u32>,
    here: i64,
    out: Vec<Arc<Insn>>,
    one: Option<Arc<Insn>>,
    keep: HashSet<u32>,
    retained: HashSet<u32>,
    absorbed: HashSet<i64>, // later copies of a group already taken
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
            spills: IndexMap::default(),
            cells: IndexMap::default(),
            defined: HashMap::default(),
            sequence: Vec::new(),
            reads: IndexMap::default(),
            defs: IndexMap::default(),
            live_out: BTreeSet::new(),
            aliases: IndexMap::default(),
            here: -1,
            out: Vec::new(),
            one: None,
            keep: HashSet::default(),
            retained: HashSet::default(),
            absorbed: HashSet::default(),
        }
    }

    fn one(&self) -> &Arc<Insn> {
        self.one.as_ref().expect("an instruction is being allocated")
    }

    /// Enter a block with `arriving` on the stack and `stored` in their spill cells.
    fn block(&mut self, block: &LirBlock, live_out: BTreeSet<u32>, arriving: Vec<u32>, stored: &[u32]) -> Result<(), Raised> {
        self.sequence = block.insns.clone();
        self.aliases = _equivalent_loads(&self.sequence, &live_out);
        (self.reads, self.defs) = (IndexMap::default(), IndexMap::default());
        // A group's copies are simultaneous: all read at its first, all write at its last.
        let mut spans: IndexMap<i64, (i64, i64)> = IndexMap::default();
        for (position, instruction) in self.sequence.iter().enumerate() {
            if let (Some(group), Some(_)) = (instruction.group, _float_copy(instruction)) {
                spans.entry(group).or_insert((position as i64, position as i64)).1 = position as i64;
            }
        }
        for (position, instruction) in self.sequence.iter().enumerate() {
            let Some(what) = instruction.what.as_ref() else { continue };
            let span = instruction.group.filter(|_| _float_copy(instruction).is_some()).map(|group| spans[&group]);
            let (read, written) = span.unwrap_or((position as i64, position as i64));
            for arg in &what.sources {
                if let Loc::Held(arg) = arg {
                    if arg.width == 10 {
                        let key = *self.aliases.get(&arg.value).unwrap_or(&arg.value);
                        self.reads.entry(key).or_default().push_back(read);
                    }
                }
            }
            for arg in &what.dests {
                if let Loc::Held(arg) = arg {
                    if arg.width == 10 && !self.aliases.contains_key(&arg.value) {
                        self.defs.entry(arg.value).or_default().push_back(written);
                    }
                }
            }
        }
        for positions in self.reads.values_mut().chain(self.defs.values_mut()) {
            positions.make_contiguous().sort_unstable();
        }
        (self.live_out, self.values, self.here) = (live_out, arriving, -1);
        self.home.clear();
        self.retained.clear();
        self.defined.clear();
        self.keep.clear();
        self.absorbed.clear();
        for value in stored {
            let load = self.spill_load(*value)?;
            self.home.insert(*value, load);
        }
        Ok(())
    }

    /// The load reading `value` back from its own spill cell.
    fn spill_load(&mut self, value: u32) -> Result<Arc<Insn>, Raised> {
        if let Some(load) = self.spills.get(&value) {
            return Ok(Arc::clone(load));
        }
        let Some(frame) = self.frame.as_deref_mut() else {
            return Err(unlowered("floating spill requires an owned frame"));
        };
        let owner = self.cells.get(&value).copied().unwrap_or(value);
        let cell = frame.cell(("floating", i64::from(owner)), 8)?;
        let load = semantics(Operation::FloatLoad, "fld", vec![Loc::Held(Held { value, width: 10 })], vec![Loc::Mem(cell)]);
        let load = Arc::new(inserted(self.sequence.first().map_or(0, |first| first.at), load));
        self.spills.insert(value, Arc::clone(&load));
        Ok(load)
    }

    /// Store the top in its spill cell and pop it.
    fn spill_top(&mut self) -> Result<(), Raised> {
        let value = self.values[0];
        let load = self.spill_load(value)?;
        let cell = cell_of(&load).clone();
        self.insert(semantics(Operation::FloatStore, "fstp", vec![Loc::Mem(cell)], vec![st(0)]));
        self.home.insert(value, load);
        self.values.remove(0);
        Ok(())
    }

    /// Drop slot `slot`: `fstp st(i)` moves the top over it.
    fn pop(&mut self, slot: usize) {
        self.insert(semantics(Operation::FloatStore, "fstp", vec![st(slot)], vec![st(0)]));
        self.values[slot] = self.values[0];
        self.values.remove(0);
    }

    /// Pop every value nothing reads again.
    fn pop_dead(&mut self) {
        while let Some(slot) = (0..self.values.len()).find(|slot| !self.survives(self.values[*slot])) {
            self.pop(slot);
        }
    }

    /// Whether the value is read after here, from a stack slot or its home.
    fn needed(&mut self, value: u32) -> bool {
        self.live_after(value) || !self.pending(value).is_empty()
    }

    /// Empty the stack before an instruction it cannot cross, keeping what is read later in memory.
    fn flush(&mut self) -> Result<(), Raised> {
        while let Some(&value) = self.values.first() {
            if !self.home.contains_key(&value) && self.needed(value) {
                self.spill_top()?;
            } else {
                self.pop(0);
            }
        }
        Ok(())
    }

    /// Leave the block with exactly `wanted` on the stack, and every other value read later in its cell.
    ///
    /// A wanted value this block does not have is read by none of its
    /// successors: another exit into the bundle has it, and the slot is filled.
    fn leave(&mut self, wanted: &[u32]) -> Result<(), Raised> {
        self.here = self.sequence.len() as i64;
        self.keep.clear();
        while let Some(slot) = self.values.iter().position(|value| !wanted.contains(value)) {
            let value = self.values[slot];
            if self.live_after(value) && !self.home.contains_key(&value) {
                self.exchange(slot);
                self.spill_top()?;
            } else {
                self.pop(slot);
            }
        }
        for value in wanted.iter().rev() {
            if self.values.contains(value) {
                continue;
            }
            if self.home.contains_key(value) {
                self.materialize(*value)?;
            } else {
                // Not read on any path from here: the successors that share the slot pop it.
                self.room(1)?;
                let filler = if self.values.is_empty() {
                    semantics(Operation::FloatLoad, "fldz", vec![st(0)], Vec::new())
                } else {
                    semantics(Operation::FloatLoad, "fld", vec![st(0)], vec![st(0)])
                };
                self.insert(filler);
                self.values.insert(0, *value);
            }
        }
        // Each exchange puts the top where it belongs, or brings the first misplaced value up.
        while self.values != wanted {
            let slot = if self.values[0] == wanted[0] {
                (0..wanted.len()).find(|slot| self.values[*slot] != wanted[*slot]).expect("the stacks differ")
            } else {
                index_of(wanted, self.values[0])
            };
            self.exchange(slot);
        }
        Ok(())
    }

    /// The next definition of the value after here.
    fn next_def(&mut self, value: u32) -> Option<i64> {
        let here = self.here;
        let defs = self.defs.entry(value).or_default();
        while defs.front().is_some_and(|first| *first <= here) {
            defs.pop_front();
        }
        defs.front().copied()
    }

    /// Whether the value leaves the block as it is now.
    fn live_after(&mut self, value: u32) -> bool {
        self.live_out.contains(&value) && self.next_def(value).is_none()
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

    /// Where the value is read after this instruction, before it is defined again.
    fn pending(&mut self, value: u32) -> VecDeque<i64> {
        let key = self.canonical(value);
        let (here, until) = (self.here, self.next_def(key));
        let reads = self.reads.entry(key).or_default();
        while reads.front().is_some_and(|first| *first <= here) {
            reads.pop_front();
        }
        reads.iter().copied().take_while(|read| until.is_none_or(|until| *read <= until)).collect()
    }

    /// Whether a stack copy of the value is read after this instruction.
    fn survives(&mut self, value: u32) -> bool {
        let value = self.canonical(value);
        if self.live_after(value) {
            return true;
        }
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
            if self.home.contains_key(&self.values[0]) {
                self.pop(0);
            } else {
                self.spill_top()?;
            }
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
        if results.len() > 1 || (_float_copy(one).is_none() && results.iter().any(|result| self.values.contains(result))) {
            return Err(unlowered("floating stack result is not a fresh value"));
        }
        for result in &results {
            self.defined.insert(*result, self.here);
            self.home.shift_remove(result);
            self.retained.remove(result);
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
            if !self.live_out.contains(&results[0]) && _rereadable(&self.sequence, self.here as usize, &reads) {
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
        } else if matches!(what.op, Operation::FloatLoad | Operation::Move) && operands.len() == 1 && !results.is_empty() {
            // A phi's copies on one edge are simultaneous: take the whole group here.
            let mut pairs = vec![(results[0], operands[0])];
            if let Some(group) = one.group {
                for (position, other) in self.sequence.iter().enumerate().skip(self.here as usize + 1) {
                    if other.group == Some(group) {
                        if let Some(pair) = _float_copy(other) {
                            pairs.push(pair);
                            self.absorbed.insert(position as i64);
                        }
                    }
                }
            }
            if let Some(last) = self.absorbed.iter().max() {
                self.here = self.here.max(*last);
            }
            self.copies(&pairs)?;
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
            return Err(unlowered(&format!(
                "floating instruction has no allocation rule: {:?} {:?} of {} operands, {} results",
                what.op,
                what.name,
                operands.len(),
                results.len()
            )));
        }
        Ok(())
    }

    /// Simultaneous copies, `(result, source)`: GCC's move_for_stack_reg.
    ///
    /// A source dying here is renamed, not copied, so a phi's copies at a
    /// block's end cost nothing while their sources die. A value one of them
    /// overwrites is popped.
    fn copies(&mut self, pairs: &[(u32, u32)]) -> Result<(), Raised> {
        let results: HashSet<u32> = pairs.iter().map(|(result, _)| *result).collect();
        self.keep = pairs.iter().flat_map(|(result, source)| [*result, *source]).collect();
        // A source waiting in the cell its result shares is already the result.
        let mut free = Vec::new();
        for (result, source) in pairs {
            let spilled = self.home.get(source).is_some_and(|home| self.spills.get(source).is_some_and(|load| Arc::ptr_eq(home, load)));
            if spilled && !self.values.contains(source) && self.cells.get(result) == self.cells.get(source) && self.cells.contains_key(result) {
                free.push(*result);
            }
        }
        let pairs: Vec<(u32, u32)> = pairs.iter().copied().filter(|(result, _)| !free.contains(result)).collect();
        let mut loaded = HashSet::default();
        for (_, source) in &pairs {
            if !self.values.contains(source) {
                self.materialize(*source)?;
                loaded.insert(*source);
            }
        }
        for result in &results {
            self.defined.insert(*result, self.here);
            self.home.shift_remove(result);
            self.retained.remove(result);
        }
        for result in free {
            let load = self.spill_load(result)?;
            self.home.insert(result, load);
        }
        let mut copied = Vec::new();
        for slot in 0..self.values.len() {
            let value = self.values[slot];
            let mut wanted = pairs.iter().filter(|(_, source)| *source == value).map(|(result, _)| *result);
            let kept = !results.contains(&value) && !loaded.contains(&value) && self.survives(value);
            if !kept {
                self.values[slot] = wanted.next().unwrap_or(if results.contains(&value) { DEAD } else { value });
            }
            copied.extend(wanted.map(|result| (self.values[slot], result)));
        }
        for (from, result) in copied {
            self.room(1)?;
            let index = index_of(&self.values, from);
            self.insert(semantics(Operation::FloatLoad, "fld", vec![st(0)], vec![st(index)]));
            self.values.insert(0, result);
        }
        self.vacate();
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
        // AX is written whether or not a value is delivered in it:
        // nothing else may live there across the compare.
        word.clobbers = std::collections::BTreeSet::from([Register::AX]);
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

/// `(result, source)` of a copy between floating values.
fn _float_copy(one: &Insn) -> Option<(u32, u32)> {
    let what = one.what.as_ref()?;
    match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::FloatLoad | Operation::Move, [Loc::Held(result)], [Loc::Held(source)])
            if result.width == 10 && source.width == 10 =>
        {
            Some((result.value, source.value))
        }
        _ => None,
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

/// The blocks in reverse postorder from the entry, then the unreachable ones, and how many are reached.
fn _reverse_postorder(body: &LirBody) -> (Vec<i64>, usize) {
    let at_of: HashMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let (mut seen, mut order) = (HashSet::default(), Vec::new());
    let mut pending: Vec<(i64, usize)> = vec![(body.entry, 0)];
    seen.insert(body.entry);
    while let Some((at, next)) = pending.pop() {
        let successors = at_of.get(&at).map_or(&[][..], |block| block.succ.as_slice());
        if let Some(successor) = successors.get(next) {
            pending.push((at, next + 1));
            if at_of.contains_key(successor) && seen.insert(*successor) {
                pending.push((*successor, 0));
            }
        } else if at_of.contains_key(&at) {
            order.push(at);
        }
    }
    order.reverse();
    let reached = order.len();
    order.extend(body.blocks.iter().map(|block| block.at).filter(|at| !seen.contains(at)));
    (order, reached)
}

/// Allocate the whole function's stack, GCC's reg-stack over LLVM's edge bundles.
///
/// Blocks go in reverse postorder. The first exit reaching a bundle fixes
/// its stack: the values `stacked` puts there, in that exit's order. Every
/// other exit into it shuffles to that order, and keeps what else is read
/// later in each value's own 8-byte cell. A value on the stack at an entry
/// that the block does not read is popped there.
fn _allocate_function(
    body: &LirBody,
    frame: Option<&mut Frame>,
    floating: &HashSet<u32>,
    target: &Profile,
    retain_homes: bool,
    pieces: &Pieces,
    stacked: &IndexMap<usize, BTreeSet<u32>>,
    cells: &IndexMap<u32, u32>,
) -> Result<Candidate, Raised> {
    let (live_in, live_out) = live(body);
    let floats = |set: &BTreeSet<u32>| -> BTreeSet<u32> { set.iter().copied().filter(|value| floating.contains(value)).collect() };
    let bundles = spillplacement::bundles(body);
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut settled: IndexMap<usize, Vec<u32>> = IndexMap::default();
    let mut stack = _Stack::new(frame, floating.clone(), target, retain_homes);
    stack.cells = cells.clone();
    let mut made: IndexMap<i64, LirBlock> = IndexMap::default();
    let mut labels: IndexMap<i64, Vec<usize>> = IndexMap::default();
    let (order, reached) = _reverse_postorder(body);
    for (index, at) in order.into_iter().enumerate() {
        let block = at_of[&at];
        let piece = &pieces.local[&at];
        let mut labelled: Vec<usize> = Vec::new();
        let (entry, exit) = bundles.of[&at];
        let arriving = settled.entry(entry).or_default().clone();
        // Nothing arrives at a block no path reaches: its successors' slots are filled.
        let stored: Vec<u32> = if index < reached {
            floats(&live_in[&at]).into_iter().filter(|value| !arriving.contains(value)).collect()
        } else {
            Vec::new()
        };
        stack.block(block, floats(&live_out[&at]), arriving, &stored)?;
        let border = |at: i64| Arc::new(inserted(at, semantics(Operation::Nothing, "", Vec::new(), Vec::new())));
        stack.out = Vec::new();
        stack.one = Some(border(block.insns.first().map_or(block.at, |first| first.at)));
        stack.pop_dead();
        labelled.resize(stack.out.len(), piece[0]);
        let cut = _terminators(block);
        for (position, one) in block.insns.iter().enumerate().take(cut) {
            labelled.resize(stack.out.len(), piece[position.saturating_sub(1)]);
            stack.here = stack.here.max(position as i64);
            if stack.absorbed.contains(&(position as i64)) {
                stack.one = Some(Arc::clone(one));
                stack.vacate();
                continue;
            }
            if one.what.as_ref().is_none_or(|what| !what.sources.iter().chain(&what.dests).any(_floating)) {
                if one.uses.iter().chain(&one.defines).any(|value| floating.contains(value)) {
                    return Err(unlowered("floating value used by an unmodelled instruction"));
                }
                if boundary(one) {
                    stack.one = Some(Arc::clone(one));
                    stack.flush()?;
                }
                stack.out.push(Arc::clone(one));
            } else {
                stack.allocate(one)?;
                stack.pop_dead();
            }
        }
        labelled.resize(stack.out.len(), piece[cut.saturating_sub(1)]);
        stack.one = Some(border(block.insns.get(cut).or(block.insns.last()).map_or(block.at, |one| one.at)));
        if block.succ.iter().any(|successor| at_of.contains_key(successor)) {
            let wanted = settled
                .entry(exit)
                .or_insert_with(|| {
                    let chosen = stacked.get(&exit).cloned().unwrap_or_default();
                    let missing = stack.live_out.iter().copied().filter(|value| chosen.contains(value) && !stack.values.contains(value));
                    let held = stack.values.iter().copied().filter(|value| chosen.contains(value));
                    let mut wanted: Vec<u32> = missing.take(8 - held.clone().count()).collect();
                    wanted.extend(held);
                    wanted
                })
                .clone();
            stack.leave(&wanted)?;
        } else {
            stack.here = block.insns.len() as i64;
            stack.pop_dead();
            if !stack.values.is_empty() {
                return Err(unlowered("floating value on the stack where the function leaves"));
            }
        }
        stack.out.extend(block.insns[cut..].iter().cloned());
        labelled.resize(stack.out.len(), piece[cut]);
        let mut one = block.clone();
        one.insns = std::mem::take(&mut stack.out);
        made.insert(at, one);
        labels.insert(at, labelled);
    }
    let mut out = body.clone();
    out.blocks = body.blocks.iter().map(|block| made.shift_remove(&block.at).expect("every block allocated")).collect();
    Ok((out, labels))
}

type Candidate = (LirBody, IndexMap<i64, Vec<usize>>);

/// Value -> the value whose spill cell it shares.
///
/// Values joined by copies share one cell where their lives do not overlap,
/// so a copy between two spilled values is no instruction at all: LLVM's
/// spill-slot coloring, within each copy-joined web.
fn _shared_cells(body: &LirBody, floating: &HashSet<u32>, live_out: &Live) -> IndexMap<u32, u32> {
    let mut web: IndexMap<u32, u32> = IndexMap::default();
    fn find(web: &mut IndexMap<u32, u32>, mut one: u32) -> u32 {
        while let Some(next) = web.get(&one).copied().filter(|next| *next != one) {
            one = next;
        }
        one
    }
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if let Some((result, source)) = _float_copy(one) {
            let (result, source) = (find(&mut web, result), find(&mut web, source));
            web.insert(result, result);
            web.insert(source, result);
        }
    }
    let members: Vec<u32> = web.keys().copied().collect();
    let root: IndexMap<u32, u32> = members.iter().map(|value| (*value, find(&mut web, *value))).collect();
    // Two members overlap where one is live as the other is defined.
    let mut overlaps: HashSet<(u32, u32)> = HashSet::default();
    for block in &body.blocks {
        let mut alive: BTreeSet<u32> = live_out[&block.at].iter().copied().filter(|value| root.contains_key(value)).collect();
        for one in block.insns.iter().rev() {
            for defined in one.defines.iter().filter(|value| root.contains_key(*value)) {
                for other in alive.iter().filter(|other| *other != defined && root[*other] == root[defined]) {
                    overlaps.insert((*defined, *other));
                    overlaps.insert((*other, *defined));
                }
            }
            for value in &one.defines {
                alive.remove(value);
            }
            alive.extend(one.uses.iter().copied().filter(|value| root.contains_key(value) && floating.contains(value)));
        }
    }
    let mut cells: IndexMap<u32, u32> = IndexMap::default();
    let mut colors: IndexMap<u32, Vec<u32>> = IndexMap::default(); // web -> each cell's owner
    for value in members.iter().copied().collect::<BTreeSet<u32>>() {
        let owners = colors.entry(root[&value]).or_default();
        let taken = |owner: &u32| cells.iter().any(|(other, cell)| cell == owner && overlaps.contains(&(value, *other)));
        let owner = owners.iter().copied().find(|owner| !taken(owner)).unwrap_or(value);
        if owner == value {
            owners.push(value);
        }
        cells.insert(value, owner);
    }
    cells
}

/// Bundle -> the floating values it holds on the stack, LLVM's SpillPlacement per value.
///
/// A block wants a value on the stack at a border where it reads the value
/// before the stack is next emptied, or holds it after the stack was last
/// emptied; memory where a call or barrier comes first. A block the value
/// passes through links its bundles, or wants memory when it empties the stack.
fn _stacked(body: &LirBody, floating: &HashSet<u32>, live_in: &Live, live_out: &Live, bundles: &spillplacement::Bundles) -> IndexMap<usize, BTreeSet<u32>> {
    // Per block: where the stack is first and last emptied, and each value's first and last event.
    let mut borders: IndexMap<i64, (Option<usize>, Option<usize>, IndexMap<u32, (usize, usize)>)> = IndexMap::default();
    for block in &body.blocks {
        let (mut first, mut last, mut events) = (None, None, IndexMap::<u32, (usize, usize)>::default());
        for (position, one) in block.insns.iter().enumerate() {
            let floated = one.what.as_ref().is_some_and(|what| what.sources.iter().chain(&what.dests).any(_floating));
            if !floated && boundary(one) {
                first = first.or(Some(position));
                last = Some(position);
            }
            // A copy moves a value between names, in whichever place its source is.
            if _float_copy(one).is_some() {
                continue;
            }
            for value in one.uses.iter().chain(&one.defines).filter(|value| floating.contains(value)) {
                events.entry(*value).or_insert((position, position)).1 = position;
            }
        }
        borders.insert(block.at, (first, last, events));
    }
    let mut placement = spillplacement::Placement::new(body, bundles);
    let mut stacked: IndexMap<usize, BTreeSet<u32>> = IndexMap::default();
    let values: BTreeSet<u32> = live_out.values().flatten().copied().filter(|value| floating.contains(value)).collect();
    for value in values {
        let (mut constraints, mut spilled, mut links) = (Vec::new(), Vec::new(), Vec::new());
        for block in &body.blocks {
            let (entering, leaving) = (live_in[&block.at].contains(&value), live_out[&block.at].contains(&value));
            if !entering && !leaving {
                continue;
            }
            let (first, last, events) = &borders[&block.at];
            let Some((earliest, latest)) = events.get(&value) else {
                if first.is_some() { spilled.push(block.at) } else { links.push(block.at) }
                continue;
            };
            let border = |stacked: bool| if stacked { Border::PrefReg } else { Border::PrefSpill };
            constraints.push(Constraint {
                block: block.at,
                entry: if entering { border(first.is_none_or(|first| *earliest < first)) } else { Border::DontCare },
                exit: if leaving { border(last.is_none_or(|last| *latest > last)) } else { Border::DontCare },
                weight: 1.0,
            });
        }
        placement.prepare();
        placement.add_constraints(&constraints);
        placement.add_pref_spill(&spilled, false);
        placement.add_links(&links);
        placement.scan();
        placement.iterate();
        for bundle in placement.finish() {
            stacked.entry(bundle).or_default().insert(value);
        }
    }
    stacked
}

/// Where the instructions leaving the block begin; the stack shuffles there.
fn _terminators(block: &LirBlock) -> usize {
    let mut cut = block.insns.len();
    while cut > 0
        && block.insns[cut - 1]
            .what
            .as_ref()
            .is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch | Operation::Return))
    {
        cut -= 1;
    }
    cut
}

/// Stretches of the function no floating value is live across.
///
/// A block is cut wherever no floating value is live; a bundle whose exits
/// carry a value joins its exits' last and entries' first stretches. Each
/// candidate's stack is empty at every cut, so any may be taken per piece.
struct Pieces {
    /// Block -> the piece of each instruction, and of the block's end last.
    local: IndexMap<i64, Vec<usize>>,
    /// Piece -> the joined piece it belongs to.
    joined: Vec<usize>,
}

fn _pieces(body: &LirBody, floating: &HashSet<u32>) -> Pieces {
    let (_, live_out) = live(body);
    let floats = |values: &mut dyn Iterator<Item = u32>| -> BTreeSet<u32> { values.filter(|value| floating.contains(value)).collect() };
    let mut local: IndexMap<i64, Vec<usize>> = IndexMap::default();
    let mut count = 0;
    for block in &body.blocks {
        // Position p is the point after instruction p - 1.
        let mut empty = vec![false; block.insns.len() + 1];
        let mut alive = floats(&mut live_out[&block.at].iter().copied());
        empty[block.insns.len()] = alive.is_empty();
        for (position, one) in block.insns.iter().enumerate().rev() {
            for value in &one.defines {
                alive.remove(value);
            }
            alive.extend(floats(&mut one.uses.iter().copied()));
            empty[position] = alive.is_empty();
        }
        let leaving = _terminators(block);
        let mut pieces = Vec::with_capacity(empty.len());
        for (position, cut) in empty.iter().enumerate() {
            if position > 0 && position <= leaving && *cut {
                count += 1;
            }
            pieces.push(count);
        }
        count += 1;
        local.insert(block.at, pieces);
    }
    let mut joined: Vec<usize> = (0..count).collect();
    fn find(joined: &mut [usize], mut one: usize) -> usize {
        while joined[one] != one {
            joined[one] = joined[joined[one]];
            one = joined[one];
        }
        one
    }
    let bundles = spillplacement::bundles(body);
    let mut ends: IndexMap<usize, (bool, Vec<usize>)> = IndexMap::default();
    for block in &body.blocks {
        let (entry, exit) = bundles.of[&block.at];
        let pieces = &local[&block.at];
        ends.entry(entry).or_default().1.push(pieces[0]);
        let leaving = ends.entry(exit).or_default();
        leaving.1.push(pieces[_terminators(block)]);
        leaving.0 |= live_out[&block.at].iter().any(|value| floating.contains(value));
    }
    for (carried, pieces) in ends.values() {
        if *carried {
            for piece in &pieces[1..] {
                let (one, other) = (find(&mut joined, pieces[0]), find(&mut joined, *piece));
                joined[other] = one;
            }
        }
    }
    let joined = (0..count).map(|piece| find(&mut joined, piece)).collect();
    Pieces { local, joined }
}

/// Each joined piece from the candidate that prices it lower.
fn _composed(candidates: &[Candidate], pieces: &Pieces, target: &Profile) -> LirBody {
    let mut scores: IndexMap<(usize, usize), (i64, i64)> = IndexMap::default();
    for (index, (body, labels)) in candidates.iter().enumerate() {
        for block in &body.blocks {
            for (one, piece) in block.insns.iter().zip(&labels[&block.at]) {
                let score = scores.entry((pieces.joined[*piece], index)).or_default();
                score.1 += i64::from(one.what.as_ref().is_some_and(|what| !(what.op == Operation::Nothing && unnamed(what))));
                if let Some(form) = one.what.as_ref().and_then(_floating_form) {
                    score.0 += target.cost(&form).unwrap_or(1);
                }
            }
        }
    }
    let chosen = |piece: usize| {
        let root = pieces.joined[piece];
        (0..candidates.len()).min_by_key(|index| scores.get(&(root, *index)).copied().unwrap_or_default()).expect("a candidate")
    };
    let mut out = candidates[0].0.clone();
    for (position, block) in out.blocks.iter_mut().enumerate() {
        let mut insns = Vec::new();
        for piece in pieces.local[&block.at].iter().copied().collect::<BTreeSet<usize>>() {
            let (body, labels) = &candidates[chosen(piece)];
            let made = &body.blocks[position];
            insns.extend(made.insns.iter().zip(&labels[&made.at]).filter(|(_, label)| **label == piece).map(|(one, _)| Arc::clone(one)));
        }
        block.insns = insns;
    }
    out
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
    pool: Option<&mut Pool>,
    basic_semantics: bool,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<LirBody, Raised> {
    let target = targets::profile(cpu).map_err(Raised::Value)?;
    let loaded = _integer_loads(body, frame.as_deref_mut(), pool)?;
    let mut body = _integer_stores(&loaded, frame.as_deref_mut(), basic_semantics)?;
    let floating = _floating_values(&body);
    if floating.is_empty() {
        return Ok(body);
    }
    let (live_in, live_out) = live(&body);
    if live_in[&body.entry].iter().any(|value| floating.contains(value)) {
        return Err(unlowered("floating stack input is unavailable"));
    }
    let pieces = _pieces(&body, &floating);
    let stacked = _stacked(&body, &floating, &live_in, &live_out, &spillplacement::bundles(&body));
    let cells = _shared_cells(&body, &floating, &live_out);
    let baseline = _allocate_function(&body, frame.as_deref_mut(), &floating, target, false, &pieces, &stacked, &cells)?;
    let retained = _allocate_function(&body, frame.as_deref_mut(), &floating, target, true, &pieces, &stacked, &cells)?;
    let selected = _composed(&[baseline, retained], &pieces, target);
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
    pub pool: Option<Rc<RefCell<Pool>>>,
    pub basic_semantics: bool,
    pub cpu: &'a Profile,
}

impl<'a> FloatAlloc<'a> {
    pub fn new(
        frame: Option<Rc<RefCell<Frame>>>,
        pool: Option<Rc<RefCell<Pool>>>,
        basic_semantics: bool,
        cpu: impl Into<ProfileOrName<'a>>,
    ) -> Result<Self, String> {
        Ok(Self { frame, pool, basic_semantics, cpu: targets::profile(cpu)? })
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
        let mut pool = self.pool.as_ref().map(|pool| pool.borrow_mut());
        allocated(&body, frame.as_deref_mut(), pool.as_deref_mut(), self.basic_semantics, self.cpu).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "floatalloc_tests.rs"]
mod tests;
