//! Where the value a variable has is, over the code as it was emitted: LLVM's instruction-referencing LiveDebugValues
//! (`InstrRefBasedLDV`), after the allocator and every pass that follows it.
//!
//! Instruction selection tells which instructions define a value `-g` names, and masm marks where each landed (`Mark::Def`: the
//! register or frame cell the instruction before the mark wrote) and where a variable takes a value (`Mark::Note`). Nothing else
//! of the allocator is read. The code is decoded and followed from its entry along every path, with what each register and frame
//! cell holds: a move copies it, any other write loses it, a call loses what the callee clobbers, and where paths join only what
//! all of them agree on is kept. A variable is where its value is: a register if one holds it, else a cell.
//!
//! No value is the answer wherever the code cannot be followed or the value is in no register or cell. An optimizer that wrote
//! the value somewhere unrecorded costs the variable its location there, never a wrong one.

use std::collections::{BTreeMap, BTreeSet};

use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction, InstructionInfoFactory, Mnemonic, OpAccess, OpKind, Register};
use llrm_object::debug::FrameRow;

use super::masm::{Mark, Place};
use crate::model::lir::{DebugNote, NoteValue};

/// What holds a value: a register as the instruction named it, or a frame cell.
type Held = Place;

/// What is known before an instruction runs.
#[derive(Clone, Debug, Eq, PartialEq)]
struct State {
    /// The value each register and cell holds.
    holds: BTreeMap<Held, u32>,
    /// The value each variable, or piece of one, has.
    has: BTreeMap<(u32, Option<(u32, u32)>), NoteValue>,
}

/// The target's registers as the tracking needs them, from its description: which register each view belongs to, which two address
/// cells, and which hold values (the roots of class `gpr`).
pub struct Regs {
    frame: String,
    stack: String,
    general: Vec<String>,
    roots: BTreeMap<String, String>,
}

impl Regs {
    pub fn new(file: &[llrm_target::registers::Register], frame: Register, stack: Register) -> Self {
        let roots: BTreeMap<String, String> = file.iter().map(|one| (one.name.clone(), one.root.clone())).collect();
        let general = file.iter().filter(|one| one.is("gpr") && one.root == one.name).map(|one| one.name.clone()).collect();
        let named = |register: Register| {
            let name = format!("{register:?}").to_lowercase();
            roots.get(&name).cloned().unwrap_or(name)
        };
        Self { frame: named(frame), stack: named(stack), general, roots }
    }

    /// The register `register` is a view of; any register the description does not know is itself.
    fn root(&self, register: Register) -> String {
        let name = format!("{register:?}").to_lowercase();
        self.roots.get(&name).cloned().unwrap_or(name)
    }

    /// The registers a call that says nothing of what it clobbers may: every general one but the two that address cells.
    fn volatile(&self) -> impl Iterator<Item = &String> {
        self.general.iter().filter(|one| **one != self.frame && **one != self.stack)
    }
}

fn register_of(held: &Held, regs: &Regs) -> Option<String> {
    match held {
        Held::Register(register) => Some(regs.root(*register)),
        Held::Cell { .. } => None,
    }
}

fn cells_overlap(a: (i64, u32), b: (i64, u32)) -> bool {
    a.0 < b.0 + i64::from(b.1) && b.0 < a.0 + i64::from(a.1)
}

impl State {
    /// Nothing holds `held` any more, nor anything overlapping it.
    fn lose(&mut self, held: &Held, regs: &Regs) {
        match held {
            Held::Register(register) => {
                let family = regs.root(*register);
                self.holds.retain(|one, _| register_of(one, regs).as_ref() != Some(&family));
            }
            Held::Cell { disp, bytes } => self.holds.retain(|one, _| !matches!(one, Held::Cell { disp: d, bytes: b } if cells_overlap((*d, *b), (*disp, *bytes)))),
        }
    }

    /// No frame cell holds anything known.
    fn lose_cells(&mut self) {
        self.holds.retain(|held, _| !matches!(held, Held::Cell { .. }));
    }

    /// `held` holds `value`, and nothing overlapping it does.
    fn set(&mut self, held: Held, value: u32, regs: &Regs) {
        self.lose(&held, regs);
        self.holds.insert(held, value);
    }

    /// Only what both agree on.
    fn meet(&mut self, other: &State) -> bool {
        let (holds, has) = (self.holds.len(), self.has.len());
        self.holds.retain(|held, value| other.holds.get(held) == Some(value));
        self.has.retain(|variable, value| other.has.get(variable) == Some(value));
        (holds, has) != (self.holds.len(), self.has.len())
    }
}

/// A frame cell, as the frame register would address it, of a memory operand of `one`, if it names one: through the stack
/// pointer or the frame register alone, which `rows` says how far from the canonical frame address at `at`.
fn cell(one: &Instruction, rows: &[FrameRow], at: usize, bias: i64, bytes: u32, regs: &Regs) -> Option<Held> {
    if one.memory_index() != Register::None || one.memory_base() == Register::None {
        return None;
    }
    let row = rows.iter().rev().find(|row| row.offset <= at)?;
    let base = regs.root(one.memory_base());
    if base != row.cfa_register {
        return None;
    }
    let disp = i64::from(one.memory_displacement32() as i32);
    Some(Held::Cell { disp: disp - row.cfa_offset + bias, bytes })
}

/// What `one` does to `state`: a move of a whole register or cell copies what it holds; any other write loses what it writes.
fn step(state: &mut State, one: &Instruction, rows: &[FrameRow], bias: i64, info: &mut InstructionInfoFactory, clobbers: Option<&[Register]>, exposed: bool, regs: &Regs) {
    let at = one.ip() as usize;
    let size = |kind: OpKind, register: Register| if kind == OpKind::Register { register.size() as u32 } else { one.memory_size().size() as u32 };
    let copy = one.mnemonic() == Mnemonic::Mov && one.op_count() == 2 && matches!((one.op0_kind(), one.op1_kind()), (OpKind::Register, OpKind::Register | OpKind::Memory) | (OpKind::Memory, OpKind::Register));
    if copy {
        let (to, from) = (size(one.op0_kind(), one.op0_register()), size(one.op1_kind(), one.op1_register()));
        let place = |kind: OpKind, register: Register, bytes: u32| match kind {
            OpKind::Register => Some(Held::Register(register)),
            _ => cell(one, rows, at, bias, bytes, regs),
        };
        let source = place(one.op1_kind(), one.op1_register(), from).and_then(|held| state.holds.get(&held).copied());
        let target = place(one.op0_kind(), one.op0_register(), to);
        match (target, source) {
            (Some(target), Some(value)) if to == from => state.set(target, value, regs),
            (Some(target), _) => state.lose(&target, regs),
            // A write through a pointer may write a frame cell whose address the function let out.
            (None, _) => {
                if exposed && one.op0_kind() == OpKind::Memory {
                    state.lose_cells();
                }
            }
        }
        return;
    }
    let used = info.info(one);
    for register in used.used_registers() {
        if matches!(register.access(), OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite | OpAccess::ReadCondWrite) {
            state.lose(&Held::Register(register.register()), regs);
        }
    }
    for memory in used.used_memory() {
        if matches!(memory.access(), OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite | OpAccess::ReadCondWrite) {
            match cell(one, rows, at, bias, memory.memory_size().size() as u32, regs) {
                Some(held) => state.lose(&held, regs),
                None if exposed => state.lose_cells(),
                None => {}
            }
        }
    }
    // The stack pointer's own pushes are cells the code names by no operand: a push loses what it covers.
    if matches!(one.mnemonic(), Mnemonic::Push | Mnemonic::Pushad | Mnemonic::Pushfd | Mnemonic::Pushf) {
        let row = rows.iter().rev().find(|row| row.offset <= at);
        if let Some(row) = row.filter(|row| row.cfa_register == regs.stack) {
            let bytes = (-i64::from(one.stack_pointer_increment())).max(0);
            state.lose(&Held::Cell { disp: -row.cfa_offset - bytes + bias, bytes: bytes as u32 }, regs);
        }
    }
    if one.flow_control() == FlowControl::Call || one.flow_control() == FlowControl::IndirectCall {
        // What the callee is handed the address of it may write.
        if exposed {
            state.lose_cells();
        }
        match clobbers {
            Some(registers) => registers.iter().for_each(|register| state.lose(&Held::Register(*register), regs)),
            None => regs.volatile().for_each(|root| state.holds.retain(|one, _| register_of(one, regs).as_ref() != Some(root))),
        }
    }
}

/// Where a variable's value is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Where {
    Place(Place),
    /// In no place: the value is the constant.
    Constant(i64),
}

/// `state` after the variable `note` names takes its value: the whole variable replaces every piece of it, and a piece replaces
/// the whole and the pieces it overlaps.
fn noted(state: &mut State, note: &DebugNote) {
    let overlaps = |a: (u32, u32), b: (u32, u32)| a.0 < b.0 + b.1 && b.0 < a.0 + a.1;
    state.has.retain(|&(variable, piece), _| {
        variable != note.variable
            || match (piece, note.piece) {
                (Some(kept), Some(new)) => !overlaps(kept, new),
                _ => false,
            }
    });
    if let Some(value) = match note.value {
        NoteValue::Nothing => None,
        value => Some(value),
    } {
        state.has.insert((note.variable, note.piece), value);
    }
}

/// The place the value of `variable`'s `piece` is in at a point where `state` holds it: a register, else a cell.
fn where_is(state: &State, variable: u32, piece: Option<(u32, u32)>) -> Option<Where> {
    match *state.has.get(&(variable, piece))? {
        NoteValue::Nothing => None,
        NoteValue::Constant(value) => Some(Where::Constant(value)),
        NoteValue::Value(value) => {
            let mut found: Vec<&Held> = state.holds.iter().filter(|(_, held)| **held == value).map(|(held, _)| held).collect();
            found.sort_by_key(|held| matches!(held, Held::Cell { .. }));
            found.first().map(|held| Where::Place(**held))
        }
    }
}

/// Each variable the notes name, with the ranges of `code` (start, end) it is in a place over. `marks` are the procedure's, by
/// the offset into `code` they stand at, in the order masm wrote them; `rows` its frame rows; `bias` how far below the
/// canonical frame address the frame register would sit (a cell `Place::Cell { disp }` is `disp - bias` from it).
pub fn tracked(code: &[u8], regs: &Regs, rows: &[FrameRow], bias: i64, notes: &[DebugNote], marks: &[(usize, Mark)]) -> BTreeMap<(u32, Option<(u32, u32)>), Vec<(usize, usize, Where)>> {
    let mut decoder = Decoder::with_ip(32, code, 0, DecoderOptions::NONE);
    let mut decode = |at: usize| -> Option<Instruction> {
        decoder.set_position(at).ok()?;
        decoder.set_ip(at as u64);
        let one = decoder.decode();
        (!one.is_invalid()).then_some(one)
    };
    // What the marks at each offset say: the values the instruction ending there made, the notes before the one starting there,
    // and what the call ending there clobbered.
    let mut defs: BTreeMap<usize, Vec<(u32, Place)>> = BTreeMap::new();
    let mut before: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
    let mut clobbers: BTreeMap<usize, Vec<Register>> = BTreeMap::new();
    for &(at, mark) in marks {
        match mark {
            Mark::Def { tag, place } => defs.entry(at).or_default().push((tag, place)),
            Mark::Note(note) => before.entry(at).or_default().push(note),
            Mark::Clobbered(register) => clobbers.entry(at).or_default().push(register),
            _ => {}
        }
    }
    let mut info = InstructionInfoFactory::new();
    // Whether the function lets the address of a frame cell out: then a call, or a write through a pointer, may write any of them
    // (which one, only the allocator's frame layout says, and the code is read after it).
    let exposed = {
        let mut scan = Decoder::with_ip(32, code, 0, DecoderOptions::NONE);
        let mut found = false;
        while scan.can_decode() {
            let one = scan.decode();
            if one.mnemonic() == Mnemonic::Lea && [&regs.frame, &regs.stack].contains(&&regs.root(one.memory_base())) {
                found = true;
                break;
            }
        }
        found
    };
    let mut seen: BTreeMap<usize, (Instruction, State)> = BTreeMap::new();
    // What is said before the first instruction (the arguments, in the registers they arrive in) holds from the entry.
    let mut first = State { holds: BTreeMap::new(), has: BTreeMap::new() };
    for &(tag, place) in defs.get(&0).into_iter().flatten() {
        first.set(place, tag, regs);
    }
    let mut work: Vec<(usize, State)> = vec![(0, first)];
    while let Some((at, mut state)) = work.pop() {
        // Where paths join, only what they agree on.
        if let Some((_, known)) = seen.get_mut(&at) {
            if !known.meet(&state) {
                continue;
            }
            state = known.clone();
        }
        let Some(one) = seen.get(&at).map(|(one, _)| *one).or_else(|| decode(at)) else { continue };
        seen.insert(at, (one, state.clone()));
        // The variables take their values before the instruction; they are what a debugger at it reads.
        for &note in before.get(&at).into_iter().flatten() {
            noted(&mut state, &notes[note as usize]);
        }
        // What the instruction does, then what it is told it made.
        let end = at + one.len();
        step(&mut state, &one, rows, bias, &mut info, clobbers.get(&end).map(Vec::as_slice), exposed, regs);
        for &(tag, place) in defs.get(&end).into_iter().flatten() {
            state.set(place, tag, regs);
        }
        let (falls, jumps) = match one.flow_control() {
            FlowControl::Next | FlowControl::Call | FlowControl::IndirectCall | FlowControl::XbeginXabortXend | FlowControl::Interrupt | FlowControl::Exception => (true, false),
            FlowControl::ConditionalBranch => (true, true),
            FlowControl::UnconditionalBranch => (false, true),
            FlowControl::Return | FlowControl::IndirectBranch => (false, false),
        };
        if falls && end < code.len() {
            work.push((end, state.clone()));
        }
        if jumps && matches!(one.op0_kind(), OpKind::NearBranch32 | OpKind::NearBranch16) {
            let target = one.near_branch_target() as usize;
            if target < code.len() {
                work.push((target, state));
            }
        }
    }
    // The variable's place before each instruction, with the notes of that point taken.
    let variables: BTreeSet<(u32, Option<(u32, u32)>)> = notes.iter().map(|note| (note.variable, note.piece)).collect();
    let mut out: BTreeMap<(u32, Option<(u32, u32)>), Vec<(usize, usize, Where)>> = variables.iter().map(|&key| (key, Vec::new())).collect();
    for (&at, (one, entry)) in &seen {
        let mut state = entry.clone();
        for &note in before.get(&at).into_iter().flatten() {
            noted(&mut state, &notes[note as usize]);
        }
        let end = at + one.len();
        for &(variable, piece) in &variables {
            let Some(place) = where_is(&state, variable, piece) else { continue };
            let ranges = out.get_mut(&(variable, piece)).expect("every variable has its ranges");
            match ranges.last_mut() {
                Some(last) if last.1 == at && last.2 == place => last.1 = end,
                _ => ranges.push((at, end, place)),
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "valuetrack_tests.rs"]
mod tests;
