//! Port of `qbopt/backend/floatalloc.py`: assign floating LIR values to the
//! target register stack.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use crate::support::hash::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::IndexMap;

use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::allocate::live;
use crate::backend::floatregions::{Raised, boundary};
use crate::backend::spillplacement;
use crate::backend::constpool::Pool;
use crate::backend::floatassign;
use crate::backend::frame::Frame;
use crate::backend::lower::Unlowered;
use crate::backend::select;
use crate::model::ir::{Held, Imm, Loc, Mem, Operation, Reg, Semantics, St};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

pub(super) fn unlowered(message: &str) -> Raised {
    Raised::Unlowered(Unlowered(message.to_owned()))
}

pub(super) fn st(index: usize) -> Loc {
    Loc::St(St { index: index as u32 })
}

pub(super) fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

/// `lir.Insn(at, (at, at), what, (), ())`.
pub(super) fn inserted(at: i64, what: Semantics) -> Insn {
    Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new())
}

pub(super) fn name_is(what: &Semantics, name: &str) -> bool {
    what.name.as_deref() == Some(name)
}

/// Python's `not what.name`.
pub(super) fn unnamed(what: &Semantics) -> bool {
    what.name.as_deref().is_none_or(str::is_empty)
}

pub(super) fn width_of(arg: &Loc) -> Option<u32> {
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
pub(super) fn cell_of(load: &Insn) -> &Mem {
    match &load.what.as_ref().expect("a home load has semantics").sources[0] {
        Loc::Mem(cell) => cell,
        _ => unreachable!("a home load reads memory"),
    }
}

const _ARITHMETIC: [&str; 4] = ["fadd", "fsub", "fmul", "fdiv"];
// `left op right` with the operands' places swapped: `st(i) := st(0) - st(i)` is `fsubr st(i),st(0)`.
const _REVERSED: [(&str, &str); 4] = [("fadd", "fadd"), ("fmul", "fmul"), ("fsub", "fsubr"), ("fdiv", "fdivr")];

fn _reversed(name: &str) -> &'static str {
    _REVERSED.iter().find(|(key, _)| *key == name).map(|(_, value)| *value).expect("_REVERSED[name]")
}

pub(super) fn _floating(arg: &Loc) -> bool {
    matches!(arg, Loc::Held(held) if held.width == 10)
}

pub(super) fn _loads_memory(what: Option<&Semantics>) -> bool {
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
pub(super) fn _two_values(what: Option<&Semantics>) -> Option<(String, Held, Held)> {
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
pub(super) fn _memory_name(name: &str, cell_is_left: bool, load: &Insn) -> Option<String> {
    let mut name = if cell_is_left { _reversed(name) } else { name }.to_owned();
    if name_is(load.what.as_ref().expect("a home load has semantics"), "fild") {
        name = format!("fi{}", &name[1..]);
    }
    select::float_memory(&name, cell_of(load), 0).is_some().then_some(name)
}

/// A stack slot whose value is overwritten: popped at once.
const DEAD: u32 = u32::MAX;

/// The x87 register stack through one block, GCC's reg-stack and LLVM's
/// X86FloatingPoint: it converts, and decides nothing. `floatassign` left
/// every floating value in a register exactly while it is live, never
/// across a boundary, and more than eight nowhere.
///
/// An operand an instruction consumes dies there, and the result takes its
/// slot. A value no longer needed is popped where it dies.
struct _Stack {
    floating: HashSet<u32>,
    values: Vec<u32>, // top first
    sequence: Vec<Arc<Insn>>,
    reads: IndexMap<u32, VecDeque<i64>>,
    defs: IndexMap<u32, VecDeque<i64>>,
    live_out: BTreeSet<u32>,
    here: i64,
    out: Vec<Arc<Insn>>,
    one: Option<Arc<Insn>>,
    absorbed: HashSet<i64>, // later copies of a group already taken
}

impl _Stack {
    fn new(floating: HashSet<u32>) -> Self {
        Self {
            floating,
            values: Vec::new(),
            sequence: Vec::new(),
            reads: IndexMap::default(),
            defs: IndexMap::default(),
            live_out: BTreeSet::new(),
            here: -1,
            out: Vec::new(),
            one: None,
            absorbed: HashSet::default(),
        }
    }

    fn one(&self) -> &Arc<Insn> {
        self.one.as_ref().expect("an instruction is being allocated")
    }

    /// Enter a block with `arriving` on the stack.
    fn block(&mut self, block: &LirBlock, live_out: BTreeSet<u32>, arriving: Vec<u32>) {
        self.sequence = block.insns.clone();
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
                        self.reads.entry(arg.value).or_default().push_back(read);
                    }
                }
            }
            for arg in &what.dests {
                if let Loc::Held(arg) = arg {
                    if arg.width == 10 {
                        self.defs.entry(arg.value).or_default().push_back(written);
                    }
                }
            }
        }
        for positions in self.reads.values_mut().chain(self.defs.values_mut()) {
            positions.make_contiguous().sort_unstable();
        }
        (self.live_out, self.values, self.here) = (live_out, arriving, -1);
        self.absorbed.clear();
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

    /// Before an instruction the stack cannot cross, where nothing is left on it.
    fn flush(&mut self) -> Result<(), Raised> {
        self.pop_dead();
        if !self.values.is_empty() {
            return Err(unlowered("a floating value is live across a boundary"));
        }
        Ok(())
    }

    /// Leave the block with exactly `wanted` on the stack.
    ///
    /// A wanted value this block does not have is read by none of its
    /// successors: another exit into the bundle has it, and the slot is filled.
    fn leave(&mut self, wanted: &[u32]) -> Result<(), Raised> {
        self.here = self.sequence.len() as i64;
        while let Some(slot) = self.values.iter().position(|value| !wanted.contains(value)) {
            if self.live_after(self.values[slot]) {
                return Err(unlowered("a live floating value leaves the block off the stack"));
            }
            self.pop(slot);
        }
        for value in wanted.iter().rev() {
            if self.values.contains(value) {
                continue;
            }
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

    /// Where the value is read after this instruction, before it is defined again.
    fn pending(&mut self, value: u32) -> VecDeque<i64> {
        let (here, until) = (self.here, self.next_def(value));
        let reads = self.reads.entry(value).or_default();
        while reads.front().is_some_and(|first| *first <= here) {
            reads.pop_front();
        }
        reads.iter().copied().take_while(|read| until.is_none_or(|until| *read <= until)).collect()
    }

    /// Whether the value is read after this instruction.
    fn survives(&mut self, value: u32) -> bool {
        self.live_after(value) || !self.pending(value).is_empty()
    }

    fn insert(&mut self, what: Semantics) {
        let at = self.one().at;
        self.out.push(Arc::new(inserted(at, what)));
    }

    /// `emit(what, *, uses=(), widths=(), **changes)`: `requires` and
    /// `symbol` are the only changes callers pass.
    fn emit(&mut self, what: Semantics) {
        let one = self.one();
        let mut made = (**one).clone();
        made.what = Some(what);
        made.uses = one.uses.iter().copied().filter(|value| !self.floating.contains(value)).collect();
        made.defines = one.defines.iter().copied().filter(|value| !self.floating.contains(value)).collect();
        made.widths = one.widths.iter().copied().filter(|pair| !self.floating.contains(&pair.0)).collect();
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
        if self.values.len() + count > 8 {
            return Err(unlowered("floating instruction requires too many stack operands"));
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

    fn top(&mut self, value: u32) -> Result<(), Raised> {
        if !self.values.contains(&value) {
            return Err(unlowered("floating stack input is unavailable"));
        }
        self.exchange(index_of(&self.values, value));
        Ok(())
    }

    fn allocate(&mut self, one: &Arc<Insn>) -> Result<(), Raised> {
        if !Arc::ptr_eq(&self.sequence[self.here as usize], one) {
            return Err(unlowered("floating region positions disagree"));
        }
        self.one = Some(Arc::clone(one));
        let what = one.what.clone().expect("a floating instruction has semantics");
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
        if results.len() > 1 || (_float_copy(one).is_none() && results.iter().any(|result| self.values.contains(result))) {
            return Err(unlowered("floating stack result is not a fresh value"));
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
        } else if what.op == Operation::Compare && operands.len() == 1 && results.is_empty() && matches!(what.sources.as_slice(), [Loc::Held(_), Loc::Mem(_)]) {
            self.compare_memory(operands[0], what.sources[1].clone())?;
        } else if what.op == Operation::FloatLoad && operands.is_empty() && !results.is_empty() {
            self.room(1)?;
            self.emit(Semantics { dests: vec![st(0)], ..what });
            self.values.insert(0, results[0]);
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
        if let Some((_, missing)) = pairs.iter().find(|(_, source)| !self.values.contains(source)) {
            return Err(unlowered(&format!("floating copy source {missing} is not on the stack")));
        }
        let mut copied = Vec::new();
        for slot in 0..self.values.len() {
            let value = self.values[slot];
            let mut wanted = pairs.iter().filter(|(_, source)| *source == value).map(|(result, _)| *result);
            let kept = !results.contains(&value) && self.survives(value);
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
                self.emit(Semantics { name: Some("fst".to_owned()), sources: vec![st(0)], ..what });
                return Ok(());
            }
            self.duplicate(source)?;
        }
        self.emit(Semantics { name, sources: vec![st(0)], ..what });
        self.values.remove(0);
        Ok(())
    }

    /// `left` against a cell: `fcomp m`.
    fn compare_memory(&mut self, left: u32, cell: Loc) -> Result<(), Raised> {
        self.top(left)?;
        if self.survives(left) {
            self.duplicate(left)?;
        }
        let what = self.one().what.clone().expect("a floating instruction has semantics");
        self.emit(Semantics { name: Some("fcomp".to_owned()), dests: Vec::new(), sources: vec![st(0), cell], ..what });
        self.values.remove(0);
        self.status()
    }

    /// `left` against `right`, the answer moved from the status word into the flags.
    fn compare(&mut self, left: u32, right: u32) -> Result<(), Raised> {
        // Left on top and right beneath it, each a copy where it is read again:
        // `fld st(i)` copies from any slot.
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
        self.emit(semantics(Operation::Compare, "fcompp", Vec::new(), vec![st(0), st(1)]));
        self.values.drain(..2);
        self.status()
    }

    /// The comparison defines flags through SAHF. A raised runtime helper
    /// may additionally expose AX as an opaque clobber to later ABI code;
    /// FNSTSW is the instruction that produces that value, not FCOM(PP).
    fn status(&mut self) -> Result<(), Raised> {
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
        self.emit(what);
        self.values[0] = result;
        Ok(())
    }

    fn arithmetic(&mut self, name: &str, left: Held, right: Held, result: u32) -> Result<(), Raised> {
        let (left, right) = (left.value, right.value);
        for value in [left, right] {
            if !self.values.contains(&value) {
                return Err(unlowered("floating stack input is unavailable"));
            }
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
            self.emit(semantics(Operation::FloatArith, name, vec![st(slot)], operands));
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
            self.emit(semantics(Operation::FloatArith, operation, vec![st(0)], vec![st(0), st(other)]));
            self.values[0] = result;
        } else if dies_left && dies_right {
            let operation = format!("{}p", if forward { _reversed(name) } else { name });
            self.emit(semantics(Operation::FloatArithPop, &operation, vec![st(other)], vec![st(other), st(0)]));
            self.values[other] = result;
            self.values.remove(0);
        } else {
            let operation = if forward { _reversed(name) } else { name };
            self.emit(semantics(Operation::FloatArith, operation, vec![st(other)], vec![st(other), st(0)]));
            self.values[other] = result;
        }
        Ok(())
    }
}

/// `(result, source)` of a copy between floating values.
pub(super) fn _float_copy(one: &Insn) -> Option<(u32, u32)> {
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

/// The whole function in stack form, GCC's reg-stack over LLVM's edge bundles.
///
/// Blocks go in reverse postorder. A bundle holds every floating value live
/// at one of its borders; the first exit reaching it fixes their order, and
/// every other exit shuffles to that order. A value on the stack at an entry
/// that the block does not read is popped there.
fn _converted(body: &LirBody) -> Result<LirBody, Raised> {
    let floating = floatassign::_floating_values(body);
    if floating.is_empty() {
        return Ok(body.clone());
    }
    let (live_in, live_out) = live(body);
    let floats = |set: &BTreeSet<u32>| -> BTreeSet<u32> { set.iter().copied().filter(|value| floating.contains(value)).collect() };
    let bundles = spillplacement::bundles(body);
    let mut held: IndexMap<usize, BTreeSet<u32>> = IndexMap::default();
    for block in &body.blocks {
        let (entry, exit) = bundles.of[&block.at];
        held.entry(entry).or_default().extend(floats(&live_in[&block.at]));
        held.entry(exit).or_default().extend(floats(&live_out[&block.at]));
    }
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut settled: IndexMap<usize, Vec<u32>> = IndexMap::default();
    let mut stack = _Stack::new(floating.clone());
    let mut made: IndexMap<i64, LirBlock> = IndexMap::default();
    let (order, _) = _reverse_postorder(body);
    for at in order {
        let block = at_of[&at];
        let (entry, exit) = bundles.of[&at];
        let arriving = settled.entry(entry).or_insert_with(|| held[&entry].iter().copied().collect()).clone();
        stack.block(block, floats(&live_out[&at]), arriving);
        let border = |at: i64| Arc::new(inserted(at, semantics(Operation::Nothing, "", Vec::new(), Vec::new())));
        stack.out = Vec::new();
        stack.one = Some(border(block.insns.first().map_or(block.at, |first| first.at)));
        stack.pop_dead();
        let cut = _terminators(block);
        for (position, one) in block.insns.iter().enumerate().take(cut) {
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
        stack.one = Some(border(block.insns.get(cut).or(block.insns.last()).map_or(block.at, |one| one.at)));
        if block.succ.iter().any(|successor| at_of.contains_key(successor)) {
            let wanted = settled
                .entry(exit)
                .or_insert_with(|| {
                    let chosen = &held[&exit];
                    let mut wanted: Vec<u32> = chosen.iter().copied().filter(|value| !stack.values.contains(value)).collect();
                    wanted.extend(stack.values.iter().copied().filter(|value| chosen.contains(value)));
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
        let mut one = block.clone();
        one.insns = std::mem::take(&mut stack.out);
        made.insert(at, one);
    }
    let mut out = body.clone();
    out.blocks = body.blocks.iter().map(|block| made.shift_remove(&block.at).expect("every block allocated")).collect();
    Ok(out)
}

/// Where the instructions leaving the block begin; the stack shuffles there.
pub(super) fn _terminators(block: &LirBlock) -> usize {
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

/// Both passes: `floatassign`'s decisions, then stack form.
pub fn allocated<'a>(
    body: &LirBody,
    mut frame: Option<&mut Frame>,
    pool: Option<&mut Pool>,
    basic_semantics: bool,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<LirBody, Raised> {
    let target = targets::profile(cpu).map_err(Raised::Value)?;
    let assigned = floatassign::assigned(body, frame.as_deref_mut(), pool, basic_semantics, target)?;
    _truncating(&_converted(&assigned)?, frame)
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
/// Pass two, reg-stack: `floatassign`'s flat registers in stack form.
pub struct FloatAlloc {
    pub frame: Option<Rc<RefCell<Frame>>>,
}

impl LIRTransform for FloatAlloc {
    fn class_name(&self) -> &'static str {
        "FloatAlloc"
    }

    fn name(&self) -> &str {
        "floatalloc"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        let mut frame = self.frame.as_ref().map(|frame| frame.borrow_mut());
        _converted(&body).and_then(|converted| _truncating(&converted, frame.as_deref_mut())).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "floatalloc_tests.rs"]
mod tests;
