//! Pass one of GCC's x87 allocation, as IRA and LRA do for any register
//! class: which floating values hold one of the eight flat stack registers,
//! and where. Every spill, restore and memory operand comes out
//! explicit, so `floatalloc` converts to stack form and decides nothing, as
//! reg-stack does.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;

use crate::analysis::intervals::{depths, level};
use crate::analysis::regions;
use crate::backend::allocate::{Live, live};
use crate::backend::constpool::{self, Pool};
use crate::backend::cpu::Profile;
use crate::backend::floatalloc::{
    _float_copy, _floating, _loads_memory, _memory_name, _terminators, _two_values, cell_of, inserted, name_is, semantics,
    unlowered, width_of,
};
use crate::backend::floatregions::{Raised, boundary};
use crate::backend::frame::Frame;
use crate::backend::select;
use crate::backend::spillplacement::{self, Border, Constraint};
use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet, IndexMap, IndexSet};

/// The flat registers: x87's stack depth.
const REGISTERS: usize = 8;

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
pub(crate) fn _equivalent_loads(sequence: &[Arc<Insn>], live_out: &BTreeSet<u32>) -> IndexMap<u32, u32> {
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


/// Where a value is read back from: its own spill cell, or the load it is
/// equivalent to, GCC's REG_EQUIV memory.
fn _spill_home(value: u32, cell: Mem) -> Arc<Insn> {
    let load = semantics(Operation::FloatLoad, "fld", vec![Loc::Held(Held { value, width: 10 })], vec![Loc::Mem(cell)]);
    Arc::new(Insn { widths: vec![(value, 10)], ..inserted(0, load) })
}

/// A cell of the spill format, for pricing forms before any cell is taken.
fn _probe() -> Mem {
    Mem { through: Register::BP, disp_width: 1, ..Mem::new(Some(Addr::new(Space::Frame, -8)), 8) }
}

/// `home` read again at `at`, defining `value`: LRA's restore.
fn _restored(home: &Insn, value: u32, at: i64) -> Arc<Insn> {
    let mut made = home.clone();
    made.at = at;
    made.covers = Some((at, at));
    made.what = Some(Semantics { dests: vec![Loc::Held(Held { value, width: 10 })], ..home.what.clone().expect("a home has semantics") });
    made.defines = vec![value];
    made.widths = home.widths.iter().copied().filter(|(one, _)| *one != value).chain([(value, 10)]).collect();
    made.spread = Vec::new();
    made.symbol = None;
    Arc::new(made)
}

/// `value` written to its spill cell at `at`.
fn _stored(value: u32, cell: &Mem, at: i64) -> Arc<Insn> {
    let what = semantics(Operation::FloatStore, "fstp", vec![Loc::Mem(cell.clone())], vec![Loc::Held(Held { value, width: 10 })]);
    Arc::new(Insn { spill_store: true, ..Insn::new(at, Some((at, at)), Some(what), Vec::new(), vec![value]) })
}

/// Nothing emitted where `one` was, its bytes still accounted for.
fn _vacated(one: &Insn) -> Arc<Insn> {
    let mut made = one.clone();
    made.what = Some(semantics(Operation::Nothing, "", Vec::new(), Vec::new()));
    (made.uses, made.defines, made.widths, made.requires) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    // No longer a copy, so no longer one of a phi's simultaneous copies.
    made.group = None;
    Arc::new(made)
}

fn _held_floats(args: &[Loc]) -> Vec<u32> {
    let mut out = Vec::new();
    for arg in args {
        if let Loc::Held(held) = arg {
            if held.width == 10 && !out.contains(&held.value) {
                out.push(held.value);
            }
        }
    }
    out
}

/// A store that can keep its source: `fst` exists for m32 and m64.
fn _keeps_source(what: &Semantics) -> bool {
    (name_is(what, "fstp") || name_is(what, "fst"))
        && what.dests.iter().all(|dest| matches!(dest, Loc::Mem(dest) if dest.width == 4 || dest.width == 8))
}

/// What reading `value` at an instruction costs, `(register, memory)`, with
/// `survives` saying which floating values are read again after it: the tied
/// copy a register form needs, and the memory operand or reload otherwise.
fn _use_costs(what: &Semantics, value: u32, home: &Insn, survives: &dyn Fn(u32) -> bool, cpu: &Profile) -> (f64, f64, bool) {
    let price = |form: &str| _price(cpu, form);
    let load = price("x87_load");
    if let Some((name, left, right)) = _two_values(Some(what)) {
        let (left, right) = (left.value, right.value);
        if left == right {
            return (if survives(value) { load } else { 0.0 }, load, false);
        }
        let other = if value == left { right } else { left };
        let tie = if survives(left) && survives(right) { load } else { 0.0 };
        let base = match name.as_str() {
            "fadd" | "fsub" => "x87_add",
            "fmul" => "x87_mul",
            _ => "x87_div",
        };
        let memory = _memory_name(&name, value == left, home)
            .map_or(f64::INFINITY, |_| price(&format!("{base}_m")) - price(base) + if survives(other) { load } else { 0.0 });
        return (tie, memory.min(load), memory <= load);
    }
    let kept = if survives(value) { load } else { 0.0 };
    match what.op {
        Operation::Compare if matches!(what.sources.get(1), Some(Loc::Held(held)) if held.value == value) => {
            let fused = select::float_memory("fcomp", cell_of(home), 0).is_some();
            (kept, if fused { 0.0 } else { load }, fused)
        }
        Operation::FloatStore if _keeps_source(what) => (0.0, load, false),
        _ => (kept, load, false),
    }
}

/// What an instruction form costs: its table price, and its issue, which
/// the table's latencies leave out.
fn _price(cpu: &Profile, form: &str) -> f64 {
    (cpu.cost(form).unwrap_or(1) + 1) as f64
}

/// A stretch of one value in a register: from a definition, a restore or a
/// block's entry, to its death, a boundary, or a block's exit.
#[derive(Default)]
struct Segment {
    value: u32,
    defs: Vec<(i64, usize)>,
    restore: Option<(i64, usize)>,
    /// Points it holds a register at: before instruction `position`.
    points: Vec<(i64, usize)>,
    /// Points it leaves for memory at.
    exits: Vec<(i64, usize)>,
    /// `(register, memory)` cost of each read, weighted by its block.
    uses: Vec<(f64, f64)>,
    /// Where it is read.
    reads: Vec<(i64, usize)>,
    /// Reads that can take its home as their memory operand.
    folds: usize,
}

/// Each block's positions grouped as they execute: a phi's copies are one step.
fn _steps(block: &LirBlock, cut: usize) -> Vec<(usize, usize)> {
    let mut steps = Vec::new();
    let mut position = 0;
    while position < cut {
        let mut last = position;
        if let (Some(group), Some(_)) = (block.insns[position].group, _float_copy(&block.insns[position])) {
            while last + 1 < cut
                && block.insns[last + 1].group == Some(group)
                && _float_copy(&block.insns[last + 1]).is_some()
            {
                last += 1;
            }
        }
        steps.push((position, last));
        position = last + 1;
    }
    steps
}

/// The floating values live after each step, from the block's own exit.
fn _live_after(block: &LirBlock, steps: &[(usize, usize)], out: &BTreeSet<u32>, floating: &HashSet<u32>) -> Vec<BTreeSet<u32>> {
    let mut alive: BTreeSet<u32> = out.clone();
    let mut after = vec![BTreeSet::new(); steps.len()];
    for (index, (first, last)) in steps.iter().enumerate().rev() {
        after[index] = alive.clone();
        let group = &block.insns[*first..=*last];
        for one in group {
            for value in &one.defines {
                alive.remove(value);
            }
        }
        alive.extend(group.iter().flat_map(|one| one.uses.iter().copied()).filter(|value| floating.contains(value)));
    }
    after
}

/// Segments, the allocnos they join into, and where each read and definition belongs.
struct Allocnos {
    segments: Vec<Segment>,
    parent: Vec<usize>,
    at_use: HashMap<(i64, usize, u32), usize>,
    at_def: HashMap<(i64, usize, u32), usize>,
    restores: IndexMap<(i64, usize), Vec<usize>>,
    exits: IndexMap<(i64, usize), Vec<usize>>,
}

impl Allocnos {
    fn made(&mut self, value: u32) -> usize {
        self.segments.push(Segment { value, ..Segment::default() });
        self.parent.push(self.parent.len());
        self.segments.len() - 1
    }

    fn root(&self, mut one: usize) -> usize {
        while self.parent[one] != one {
            one = self.parent[one];
        }
        one
    }

    fn join(&mut self, one: usize, other: usize) {
        let (one, other) = (self.root(one), self.root(other));
        if one != other {
            self.parent[other] = one;
        }
    }
}

struct Plan<'b> {
    body: &'b LirBody,
    floating: HashSet<u32>,
    homes: IndexMap<u32, Arc<Insn>>,
    allocnos: Allocnos,
    /// Root -> register cost minus memory cost, and whether it holds a definition leaving for memory.
    costs: IndexMap<usize, f64>,
}

/// Where each value is kept and read: LLVM's SpillPlacement per bundle for
/// the borders, and a segment per register stretch between them.
fn _allocnos(
    body: &LirBody,
    floating: &HashSet<u32>,
    homes: &IndexMap<u32, Arc<Insn>>,
    cpu: &Profile,
) -> Allocnos {
    let (live_in, live_out) = live(body);
    let floats = |set: &BTreeSet<u32>| -> BTreeSet<u32> { set.iter().copied().filter(|value| floating.contains(value)).collect() };
    let bundles = spillplacement::bundles(body);
    let stacked = _stacked(body, floating, &live_in, &live_out, &bundles);
    let cells = _shared_cells(body, floating, &live_out);
    // A bundle's registers are what any border of it holds: stack form
    // fills or pops the rest at each border.
    let mut held: IndexMap<usize, BTreeSet<u32>> = IndexMap::default();
    for block in &body.blocks {
        let (entry, exit) = bundles.of[&block.at];
        for (bundle, set) in [(entry, &live_in[&block.at]), (exit, &live_out[&block.at])] {
            let chosen = stacked.get(&bundle).cloned().unwrap_or_default();
            held.entry(bundle).or_default().extend(floats(set).intersection(&chosen).copied());
        }
    }
    let deep = depths(body);
    let mut allocnos = Allocnos {
        segments: Vec::new(),
        parent: Vec::new(),
        at_use: HashMap::default(),
        at_def: HashMap::default(),
        restores: IndexMap::default(),
        exits: IndexMap::default(),
    };
    let mut borders: IndexMap<(u32, usize), usize> = IndexMap::default();
    let mut border = |allocnos: &mut Allocnos, value: u32, bundle: usize, segment: usize| match borders.get(&(value, bundle)) {
        Some(other) => allocnos.join(*other, segment),
        None => {
            borders.insert((value, bundle), segment);
        }
    };
    for block in &body.blocks {
        let at = block.at;
        let weight = level(deep.get(&at).copied().unwrap_or(0));
        let cut = _terminators(block);
        let steps = _steps(block, cut);
        let (entering, leaving) = (floats(&live_in[&at]), floats(&live_out[&at]));
        let after = _live_after(block, &steps, &leaving, floating);
        let (entry, exit) = bundles.of[&at];
        let mut current: IndexMap<u32, usize> = IndexMap::default();
        for value in held.get(&entry).cloned().unwrap_or_default() {
            let segment = allocnos.made(value);
            allocnos.segments[segment].points.push((at, 0));
            border(&mut allocnos, value, entry, segment);
            if entering.contains(&value) {
                current.insert(value, segment);
            }
        }
        for (index, (first, last)) in steps.iter().copied().enumerate() {
            let group = &block.insns[first..=last];
            let floated = group.iter().any(|one| one.what.as_ref().is_some_and(|what| what.sources.iter().chain(&what.dests).any(_floating)));
            if !floated {
                if group.iter().any(|one| boundary(one)) {
                    for (_, segment) in current.drain(..) {
                        allocnos.segments[segment].exits.push((at, first));
                        allocnos.exits.entry((at, first)).or_default().push(segment);
                    }
                }
            } else {
                let survives = |value: u32| after[index].contains(&value);
                for (position, one) in group.iter().enumerate() {
                    let what = one.what.as_ref().expect("a floating instruction has semantics");
                    for value in _held_floats(&what.sources) {
                        let segment = *current.entry(value).or_insert_with(|| {
                            let segment = allocnos.made(value);
                            allocnos.segments[segment].restore = Some((at, first));
                            allocnos.segments[segment].points.push((at, first));
                            allocnos.restores.entry((at, first)).or_default().push(segment);
                            segment
                        });
                        let home = homes.get(&value).cloned().unwrap_or_else(|| _spill_home(value, _probe()));
                        let (register, memory, folds) = _use_costs(what, value, &home, &survives, cpu);
                        allocnos.segments[segment].folds += usize::from(folds);
                        allocnos.segments[segment].uses.push((register * weight, memory * weight));
                        allocnos.segments[segment].reads.push((at, first + position));
                        allocnos.at_use.insert((at, first + position, value), segment);
                    }
                }
                let read: BTreeSet<u32> = group.iter().flat_map(|one| _held_floats(&one.what.as_ref().expect("checked").sources)).collect();
                for value in read {
                    if !survives(value) {
                        current.shift_remove(&value);
                    }
                }
                for (position, one) in group.iter().enumerate() {
                    for value in _held_floats(&one.what.as_ref().expect("checked").dests) {
                        current.shift_remove(&value);
                        let segment = allocnos.made(value);
                        allocnos.at_def.insert((at, first + position, value), segment);
                        // IRA's coalescing: a copy from a dying source into the
                        // same spill cell is one allocno, a rename in a register
                        // and no instruction in memory.
                        let coalesced = _float_copy(one).filter(|(_, source)| {
                            !survives(*source)
                                && !homes.contains_key(source)
                                && !homes.contains_key(&value)
                                && cells.get(&value).is_some_and(|cell| cells.get(source) == Some(cell))
                        });
                        match coalesced {
                            Some((_, source)) => {
                                let read = allocnos.at_use[&(at, first + position, source)];
                                if let Some(last) = allocnos.segments[read].uses.last_mut() {
                                    *last = (0.0, 0.0);
                                }
                                allocnos.join(read, segment);
                            }
                            None => allocnos.segments[segment].defs.push((at, first + position)),
                        }
                        if survives(value) {
                            current.insert(value, segment);
                        }
                    }
                }
            }
            for segment in current.values() {
                allocnos.segments[*segment].points.push((at, last + 1));
            }
        }
        let chosen = held.get(&exit).cloned().unwrap_or_default();
        for value in &leaving {
            if chosen.contains(value) {
                continue;
            }
            if let Some(segment) = current.shift_remove(value) {
                allocnos.segments[segment].exits.push((at, cut));
                allocnos.exits.entry((at, cut)).or_default().push(segment);
            }
        }
        for value in chosen {
            let segment = match current.shift_remove(&value) {
                Some(segment) => segment,
                None => {
                    let segment = allocnos.made(value);
                    allocnos.segments[segment].points.push((at, cut));
                    if leaving.contains(&value) {
                        allocnos.segments[segment].restore = Some((at, cut));
                        allocnos.restores.entry((at, cut)).or_default().push(segment);
                    }
                    segment
                }
            };
            border(&mut allocnos, value, exit, segment);
        }
    }
    allocnos
}

impl Plan<'_> {
    /// IRA's cost of each allocno: its register form against its memory form.
    fn priced(&mut self, cpu: &Profile) {
        let deep = depths(self.body);
        let weight = |at: i64| level(deep.get(&at).copied().unwrap_or(0));
        let (load, store) = (_price(cpu, "x87_load"), _price(cpu, "x87_store"));
        let mut costs: IndexMap<usize, f64> = IndexMap::default();
        for (index, segment) in self.allocnos.segments.iter().enumerate() {
            let root = self.allocnos.root(index);
            let equivalent = self.homes.contains_key(&segment.value);
            let mut cost: f64 = segment.uses.iter().map(|(register, memory)| register - memory).sum();
            if let Some((at, _)) = segment.restore {
                cost += load * weight(at);
            }
            if equivalent {
                cost += segment.defs.iter().map(|(at, _)| load * weight(*at)).sum::<f64>();
            } else {
                // In memory, each definition is stored once after it; in a
                // register, it is stored where it leaves for memory, unless
                // it was just read from there.
                cost -= segment.defs.iter().map(|(at, _)| store * weight(*at)).sum::<f64>();
                if segment.restore.is_none() {
                    cost += segment.exits.iter().map(|(at, _)| store * weight(*at)).sum::<f64>();
                }
            }
            *costs.entry(root).or_default() += cost;
        }
        self.costs = costs;
    }

    /// The allocnos memory serves better, IRA's NO_REGS: the rest start in
    /// a register. At equal cost, a load its one reader can take as a
    /// memory operand is that operand, as GCC's combine folds it first.
    fn in_memory(&self) -> BTreeSet<usize> {
        let mut folded: IndexMap<usize, (usize, usize, bool)> = IndexMap::default();
        for (index, segment) in self.allocnos.segments.iter().enumerate() {
            let entry = folded.entry(self.allocnos.root(index)).or_insert((0, 0, true));
            entry.0 += segment.reads.len();
            entry.1 += segment.folds;
            entry.2 &= self.homes.contains_key(&segment.value);
        }
        self.costs
            .iter()
            .filter(|(root, cost)| **cost > 0.0 || (**cost == 0.0 && folded.get(*root) == Some(&(1, 1, true))))
            .map(|(root, _)| *root)
            .collect()
    }

    /// LRA's spill at a crowded point: of the allocnos holding a register
    /// there and not read or written by its instruction, the one read
    /// furthest ahead, as Belady; the cheaper to spill among equals.
    fn victim(&self, block: i64, position: usize, spilled: &BTreeSet<usize>) -> Option<usize> {
        let order: IndexMap<i64, usize> = self.body.blocks.iter().enumerate().map(|(index, one)| (one.at, index)).collect();
        let here = (order[&block], position);
        let busy: BTreeSet<u32> = self
            .body
            .blocks
            .iter()
            .find(|one| one.at == block)
            .and_then(|one| one.insns.get(position))
            .and_then(|one| one.what.as_ref())
            .map(|what| _held_floats(&what.sources).into_iter().chain(_held_floats(&what.dests)).collect())
            .unwrap_or_default();
        let mut next: IndexMap<usize, (usize, usize)> = IndexMap::default();
        for (index, segment) in self.allocnos.segments.iter().enumerate() {
            let root = self.allocnos.root(index);
            if spilled.contains(&root) || busy.contains(&segment.value) || !segment.points.contains(&(block, position)) {
                continue;
            }
            next.entry(root).or_insert((usize::MAX, usize::MAX));
        }
        for (index, segment) in self.allocnos.segments.iter().enumerate() {
            let root = self.allocnos.root(index);
            if let Some(soonest) = next.get_mut(&root) {
                for read in segment.reads.iter().map(|(at, position)| (order[at], *position)).filter(|read| *read > here) {
                    *soonest = (*soonest).min(read);
                }
            }
        }
        next.into_iter()
            .max_by(|(one, first), (other, second)| first.cmp(second).then(self.costs[one].total_cmp(&self.costs[other])).then(other.cmp(one)))
            .map(|(root, _)| root)
    }
}

/// Later loads of a cell nothing wrote since are its first load's value.
fn _aliased(body: &LirBody) -> LirBody {
    let (_, live_out) = live(body);
    let mut out = body.clone();
    for block in &mut out.blocks {
        let aliases = _equivalent_loads(&block.insns, &live_out[&block.at]);
        if aliases.is_empty() {
            continue;
        }
        let canonical = |value: u32| {
            let mut value = value;
            while let Some(next) = aliases.get(&value) {
                value = *next;
            }
            value
        };
        block.insns = block
            .insns
            .iter()
            .map(|one| {
                let Some(what) = &one.what else { return Arc::clone(one) };
                if what.dests.iter().any(|dest| matches!(dest, Loc::Held(held) if aliases.contains_key(&held.value))) {
                    return _vacated(one);
                }
                if !one.uses.iter().any(|value| aliases.contains_key(value)) {
                    return Arc::clone(one);
                }
                let sources = what
                    .sources
                    .iter()
                    .map(|arg| match arg {
                        Loc::Held(held) if held.width == 10 => Loc::Held(Held { value: canonical(held.value), ..*held }),
                        _ => arg.clone(),
                    })
                    .collect();
                let mut made = (**one).clone();
                made.what = Some(Semantics { sources, ..what.clone() });
                made.uses = one.uses.iter().map(|value| canonical(*value)).collect::<IndexSet<u32>>().into_iter().collect();
                Arc::new(made)
            })
            .collect();
    }
    out
}

/// Values a block reads from a cell nothing writes before their last
/// reader: GCC's memory equivalence, read again rather than kept.
fn _homes(body: &LirBody, floating: &HashSet<u32>) -> IndexMap<u32, Arc<Insn>> {
    let (_, live_out) = live(body);
    let mut defined: IndexMap<u32, usize> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for value in one.defines.iter().filter(|value| floating.contains(value)) {
            *defined.entry(*value).or_default() += 1;
        }
    }
    let mut homes = IndexMap::default();
    for block in &body.blocks {
        for (position, one) in block.insns.iter().enumerate() {
            if !_loads_memory(one.what.as_ref()) {
                continue;
            }
            let Some(Loc::Held(result)) = one.what.as_ref().and_then(|what| what.dests.first()) else { continue };
            if live_out[&block.at].contains(&result.value) || defined.get(&result.value) != Some(&1) {
                continue;
            }
            let reads: VecDeque<i64> = block.insns[position + 1..]
                .iter()
                .enumerate()
                .filter(|(_, other)| other.what.as_ref().is_some_and(|what| _held_floats(&what.sources).contains(&result.value)))
                .map(|(offset, _)| (position + 1 + offset) as i64)
                .collect();
            // A volatile read happens once, where it is: only its one reader, right after it, may take it.
            let once = !one.volatile() || reads.iter().eq([position as i64 + 1].iter());
            if once && _rereadable(&block.insns, position, &reads) {
                homes.insert(result.value, Arc::clone(one));
            }
        }
    }
    homes
}

impl Plan<'_> {
    /// LRA's rewrite of the chosen assignment: a store where a register
    /// stretch leaves for memory and a restore where one starts, as GCC's
    /// caller-save; a store after each definition of a spilled value, and
    /// each read of one as a memory operand or a reload.
    fn rewritten(&self, spilled: &BTreeSet<usize>, frame: &mut Option<&mut Frame>, cpu: &Profile) -> Result<LirBody, Raised> {
        let (_, live_out) = live(self.body);
        let cells = _shared_cells(self.body, &self.floating, &live_out);
        let mut homes = self.homes.clone();
        let mut home = |value: u32, frame: &mut Option<&mut Frame>| -> Result<Arc<Insn>, Raised> {
            if let Some(home) = homes.get(&value) {
                return Ok(Arc::clone(home));
            }
            let Some(frame) = frame.as_deref_mut() else {
                return Err(unlowered("floating spill requires an owned frame"));
            };
            let owner = cells.get(&value).copied().unwrap_or(value);
            let made = _spill_home(value, frame.cell(("floating", i64::from(owner)), 8)?);
            homes.insert(value, Arc::clone(&made));
            Ok(made)
        };
        let price = |form: &str| _price(cpu, form);
        let mut out = self.body.clone();
        for block in &mut out.blocks {
            let at = block.at;
            let cut = _terminators(block);
            let steps = _steps(block, cut);
            let leaving: BTreeSet<u32> = live_out[&at].iter().copied().filter(|value| self.floating.contains(value)).collect();
            let after = _live_after(block, &steps, &leaving, &self.floating);
            let mut insns: Vec<Arc<Insn>> = Vec::new();
            let restore = |insns: &mut Vec<Arc<Insn>>, position: usize, here: i64, frame: &mut Option<&mut Frame>, home: &mut dyn FnMut(u32, &mut Option<&mut Frame>) -> Result<Arc<Insn>, Raised>| -> Result<(), Raised> {
                for segment in self.allocnos.exits.get(&(at, position)).into_iter().flatten() {
                    let one = &self.allocnos.segments[*segment];
                    if !spilled.contains(&self.allocnos.root(*segment)) && one.restore.is_none() && !self.homes.contains_key(&one.value) {
                        insns.push(_stored(one.value, cell_of(&*home(one.value, frame)?), here));
                    }
                }
                for segment in self.allocnos.restores.get(&(at, position)).into_iter().flatten() {
                    if !spilled.contains(&self.allocnos.root(*segment)) {
                        let value = self.allocnos.segments[*segment].value;
                        insns.push(_restored(&*home(value, frame)?, value, here));
                    }
                }
                Ok(())
            };
            for (index, (first, last)) in steps.iter().copied().enumerate() {
                restore(&mut insns, first, block.insns[first].at, frame, &mut home)?;
                let survives = |value: u32| after[index].contains(&value);
                let (mut reloads, mut members, mut stores) = (Vec::new(), Vec::new(), Vec::new());
                for position in first..=last {
                    let one = &block.insns[position];
                    let Some(what) = one.what.as_ref().filter(|what| what.sources.iter().chain(&what.dests).any(_floating)) else {
                        members.push(Arc::clone(one));
                        continue;
                    };
                    let is_spilled = |value: u32, table: &HashMap<(i64, usize, u32), usize>| {
                        table.get(&(at, position, value)).is_some_and(|segment| spilled.contains(&self.allocnos.root(*segment)))
                    };
                    let reads: Vec<u32> = _held_floats(&what.sources).into_iter().filter(|value| is_spilled(*value, &self.allocnos.at_use)).collect();
                    let writes = _held_floats(&what.dests);
                    // A copy between spilled values sharing a cell is no instruction at all.
                    if let (Some((result, source)), [read]) = (_float_copy(one), reads.as_slice()) {
                        if *read == source
                            && is_spilled(result, &self.allocnos.at_def)
                            && !self.homes.contains_key(&source)
                            && cells.get(&result).is_some_and(|cell| cells.get(&source) == Some(cell))
                        {
                            members.push(_vacated(one));
                            continue;
                        }
                    }
                    if let [only] = writes.as_slice() {
                        if self.homes.contains_key(only) && is_spilled(*only, &self.allocnos.at_def) {
                            members.push(_vacated(one));
                            continue;
                        }
                    }
                    let mut made = Arc::clone(one);
                    let mut reloaded = Vec::new();
                    // One operand at most is memory: the later loaded, so the loads keep their order.
                    let mut order = reads.clone();
                    order.sort_by_key(|value| self.homes.get(value).map_or(i64::MIN, |home| home.at));
                    let fused = match order.last() {
                        Some(value) => {
                            self.fused(one, *value, &*home(*value, frame)?, &survives, &price)
                        }
                        None => None,
                    };
                    match fused {
                        Some(fused) => {
                            made = fused;
                            reloaded.extend(order[..order.len() - 1].iter().copied());
                        }
                        None => reloaded.extend(reads.iter().copied()),
                    }
                    for value in reloaded {
                        reloads.push(_restored(&*home(value, frame)?, value, one.at));
                    }
                    members.push(made);
                    for value in writes {
                        let segment = self.allocnos.at_def[&(at, position, value)];
                        let root = self.allocnos.root(segment);
                        if !self.homes.contains_key(&value) && spilled.contains(&root) {
                            let cell = cell_of(&*home(value, frame)?).clone();
                            stores.push(_stored(value, &cell, one.at));
                        }
                    }
                }
                insns.extend(reloads);
                insns.extend(members);
                insns.extend(stores);
            }
            let here = block.insns.get(cut).or(block.insns.last()).map_or(at, |one| one.at);
            restore(&mut insns, cut, here, frame, &mut home)?;
            insns.extend(block.insns[cut..].iter().cloned());
            block.insns = insns;
        }
        Ok(out)
    }

    /// `one` reading spilled `value` straight from its home, where x87 has
    /// that form and it costs no more than a reload.
    fn fused(&self, one: &Insn, value: u32, home: &Insn, survives: &dyn Fn(u32) -> bool, price: &dyn Fn(&str) -> f64) -> Option<Arc<Insn>> {
        let what = one.what.as_ref()?;
        let cell = Loc::Mem(cell_of(home).clone());
        let made = if let Some((name, left, right)) = _two_values(Some(what)) {
            if left.value == right.value {
                return None;
            }
            let operation = _memory_name(&name, value == left.value, home)?;
            let other = if value == left.value { right } else { left };
            let base = match name.as_str() {
                "fadd" | "fsub" => "x87_add",
                "fmul" => "x87_mul",
                _ => "x87_div",
            };
            let load = price("x87_load");
            if price(&format!("{base}_m")) - price(base) + if survives(other.value) { load } else { 0.0 } > load {
                return None;
            }
            semantics(Operation::FloatArith, &operation, what.dests.clone(), vec![Loc::Held(other), cell])
        } else if what.op == Operation::Compare
            && matches!(what.sources.as_slice(), [Loc::Held(left), Loc::Held(right)] if right.value == value && left.value != value)
            && select::float_memory("fcomp", cell_of(home), 0).is_some()
        {
            Semantics { name: Some("fcomp".to_owned()), sources: vec![what.sources[0].clone(), cell], ..what.clone() }
        } else {
            return None;
        };
        let mut fused = one.clone();
        fused.what = Some(made);
        let foreign = |values: &[u32]| values.iter().copied().filter(|one| !self.floating.contains(one)).collect::<Vec<u32>>();
        fused.uses = one.uses.iter().copied().filter(|one| *one != value).chain(foreign(&home.uses)).collect::<IndexSet<u32>>().into_iter().collect();
        fused.widths = one.widths.iter().chain(&home.widths).copied().filter(|(one, _)| *one != value && !(self.floating.contains(one) && home.defines.contains(one))).collect::<IndexSet<(u32, u32)>>().into_iter().collect();
        fused.requires = home.requires.iter().chain(&one.requires).copied().collect::<IndexSet<_>>().into_iter().collect();
        // The operand is another instruction's: its fixup is bound by address, not by this one's record.
        fused.symbol = if one.covers.is_some_and(|(start, end)| start != end) { Some(false) } else { one.symbol };
        Some(Arc::new(fused))
    }
}

/// The first instruction before which more floating values hold registers
/// than there are, counting what each bundle's borders hold.
fn _crowded(body: &LirBody, floating: &HashSet<u32>) -> Option<(i64, usize)> {
    let (live_in, live_out) = live(body);
    let bundles = spillplacement::bundles(body);
    let floats = |set: &BTreeSet<u32>| -> BTreeSet<u32> { set.iter().copied().filter(|value| floating.contains(value)).collect() };
    let mut held: IndexMap<usize, BTreeSet<u32>> = IndexMap::default();
    for block in &body.blocks {
        let (entry, exit) = bundles.of[&block.at];
        held.entry(entry).or_default().extend(floats(&live_in[&block.at]));
        held.entry(exit).or_default().extend(floats(&live_out[&block.at]));
    }
    for block in &body.blocks {
        let (entry, exit) = bundles.of[&block.at];
        let cut = _terminators(block);
        if held[&exit].len() > REGISTERS {
            return Some((block.at, cut));
        }
        if held[&entry].len() > REGISTERS {
            return Some((block.at, 0));
        }
        let steps = _steps(block, cut);
        let after = _live_after(block, &steps, &floats(&live_out[&block.at]), floating);
        for (index, (first, last)) in steps.iter().copied().enumerate() {
            let group = &block.insns[first..=last];
            let made: BTreeSet<u32> = group.iter().flat_map(|one| one.defines.iter().copied()).filter(|value| floating.contains(value)).collect();
            // Stack form keeps an operand read again by duplicating it first:
            // a compare pops what it reads, and a store without `fst` its source.
            let duplicated: usize = group
                .iter()
                .filter_map(|one| one.what.as_ref())
                .map(|what| match what.op {
                    Operation::Compare => _held_floats(&what.sources).iter().filter(|value| after[index].contains(value)).count(),
                    Operation::FloatStore if !what.dests.is_empty() && !_keeps_source(what) => {
                        _held_floats(&what.sources).iter().filter(|value| after[index].contains(value)).count()
                    }
                    _ => 0,
                })
                .sum();
            if after[index].union(&made).count() + duplicated > REGISTERS {
                return Some((block.at, first));
            }
        }
    }
    None
}

pub fn _floating_values(body: &LirBody) -> HashSet<u32> {
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

/// Pass one: every floating value's register or memory, made explicit.
pub fn assigned(
    body: &LirBody,
    mut frame: Option<&mut Frame>,
    pool: Option<&mut Pool>,
    basic_semantics: bool,
    cpu: &Profile,
) -> Result<LirBody, Raised> {
    let loaded = _integer_loads(body, frame.as_deref_mut(), pool)?;
    let body = _integer_stores(&loaded, frame.as_deref_mut(), basic_semantics)?;
    let floating = _floating_values(&body);
    if floating.is_empty() {
        return Ok(body);
    }
    let (live_in, _) = live(&body);
    if live_in[&body.entry].iter().any(|value| floating.contains(value)) {
        return Err(unlowered("floating stack input is unavailable"));
    }
    let body = _aliased(&body);
    let floating = _floating_values(&body);
    let homes = _homes(&body, &floating);
    let allocnos = _allocnos(&body, &floating, &homes, cpu);
    let mut plan = Plan { body: &body, floating, homes, allocnos, costs: IndexMap::default() };
    plan.priced(cpu);
    let mut spilled = plan.in_memory();
    let at_of: HashMap<(i64, i64), usize> = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter().enumerate().map(move |(position, one)| ((block.at, one.at), position)))
        .collect();
    loop {
        let rewritten = plan.rewritten(&spilled, &mut frame, cpu)?;
        let Some((block, position)) = _crowded(&rewritten, &_floating_values(&rewritten)) else {
            return Ok(rewritten);
        };
        let original = rewritten
            .blocks
            .iter()
            .find(|one| one.at == block)
            .and_then(|one| one.insns.get(position))
            .and_then(|one| at_of.get(&(block, one.at)).copied())
            .unwrap_or(position);
        let Some(victim) = plan.victim(block, original, &spilled) else {
            return Err(unlowered("floating instruction requires too many stack operands"));
        };
        spilled.insert(victim);
    }
}

/// Pass one as a machine phase.
pub struct FloatAssign<'a> {
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub pool: Option<Rc<RefCell<Pool>>>,
    pub basic_semantics: bool,
    pub cpu: &'a Profile,
}

impl LIRTransform for FloatAssign<'_> {
    fn class_name(&self) -> &'static str {
        "FloatAssign"
    }

    fn name(&self) -> &str {
        "floatassign"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        let mut frame = self.frame.as_ref().map(|frame| frame.borrow_mut());
        let mut pool = self.pool.as_ref().map(|pool| pool.borrow_mut());
        assigned(&body, frame.as_deref_mut(), pool.as_deref_mut(), self.basic_semantics, self.cpu).map_err(|error| error.to_string())
    }
}
