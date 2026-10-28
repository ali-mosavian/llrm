//! Makes the floating code ready for register allocation: x87 integer
//! conversions and compares go through memory, and loads of one cell are one
//! value.
//!
//! # How it works
//! Runs before `RegAlloc` as `FloatPrep` (`prepared`); the allocator then gives
//! each floating value a flat x87 register and [`crate::backend::fpstack`]
//! turns those into stack form. This pass chooses no register.
//!
//! An integer a `fild` reads from a register or an immediate goes through a
//! frame cell (a constant through the constant pool, 0 and 1 as `fldz`/`fld1`).
//! A float-to-integer store lands in an owned cell, with `wait` around it in a
//! BASIC module, and the integer is read back only if something reads it.
//! Later loads of a cell nothing wrote since are given the first load's value
//! (`_aliased_by`). Instructions that must keep their order on the stack are
//! sequenced (`_sequenced`), and a compare, which `fnstsw` answers in AX, is
//! marked as clobbering it (`_compares_clobber`).
//!
//! A spilled value takes a ten-byte frame cell in extended format, so no
//! precision is lost.
//!
//! # Example
//! `fild st, [bp+6]` of a word held in a register becomes a store of the
//! register to a frame cell and a `fild` of the cell; `fild 1` becomes `fld1`.
//!
//! # Runs
//! Machine phase after `floatfold` and `phielim`, before `RegAlloc`, once per
//! function; a no-op when the body has no floating value. Not gated by an -O
//! level.
//!
//! # Needs
//! `live_among`, [`crate::backend::frame`] for cells, `constpool` for float
//! constants.
//!
//! # See also
//! [`crate::backend::fpstack`], `docs/optimizations/floating-pop-reuse.md`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::constpool::{self, Pool};
use crate::backend::floatalloc::{_loads_memory, inserted, name_is, semantics, unlowered, width_of};
use crate::backend::floatregions::Raised;
use crate::backend::frame::Frame;
use crate::backend::liverows::{Live, live_among};
use crate::backend::overlap;
use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashSet, IndexMap, IndexSet};

// The values `prepared` asked liveness of.
#[cfg(test)]
thread_local! {
    pub(crate) static ROWS_ASKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// x87 reads integers from memory, for named values and physical stack slots.
fn _integer_loads(
    body: &LirBody,
    mut frame: Option<&mut Frame>,
    mut pool: Option<&mut Pool>,
) -> Result<LirBody, Raised> {
    let regs = body.regs();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        let mut changed = false;
        for one in &block.insns {
            let mut one = Arc::clone(one);
            // Only a conversion of an integer is looked into: every other
            // instruction's semantics were cloned here to be dropped.
            if let Some(what) =
                one.what.as_ref().filter(|what| what.op == Operation::FloatLoad && name_is(what, "fild")).cloned()
            {
                if what.sources.len() == 1
                    && what.dests.len() == 1
                    && matches!(what.sources[0], Loc::Held(_) | Loc::Imm(_))
                    && width_of(&what.sources[0]).is_some_and(|width| width == 2 || width == 4)
                    && (matches!(what.dests[0], Loc::Held(_))
                        || crate::backend::target::positional_place(regs, &what.dests[0]))
                {
                    if let Loc::Imm(Imm { value: value @ (0 | 1), .. }) = what.sources[0]
                        && let Some((name, _)) = constpool::x87_constant(value as f64)
                    {
                        let mut made = (*one).clone();
                        made.what = Some(Semantics { name: Some(name.into()), sources: Vec::new(), ..what });
                        insns.push(Arc::new(made));
                        changed = true;
                        continue;
                    }
                    if let (Loc::Imm(Imm { value, width, .. }), Some(pool)) = (&what.sources[0], pool.as_deref_mut()) {
                        let value = if *width == 2 { *value as i16 as i32 } else { *value as i32 };
                        let cell = pool.cell(constpool::narrowest(f64::from(value)));
                        let mut made = (*one).clone();
                        made.what = Some(Semantics { name: Some("fld".into()), sources: vec![Loc::Mem(cell)], ..what });
                        insns.push(Arc::new(made));
                        changed = true;
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
                    changed = true;
                }
            }
            insns.push(one);
        }
        if changed {
            let mut made = block.clone();
            made.insns = insns.into();
            blocks.push(made);
        } else {
            blocks.push(block.clone());
        }
    }
    let mut out = body.clone();
    out.blocks = blocks;
    Ok(out)
}

/// Materialize integer conversions, including runtime results in physical ST0.
fn _integer_stores(
    body: &LirBody,
    mut frame: Option<&mut Frame>,
    basic_semantics: bool,
) -> Result<LirBody, Raised> {
    // The readers of every value are only asked for a conversion's result.
    let converts = |one: &Arc<Insn>| {
        one.what
            .as_ref()
            .is_some_and(
                |what| what.op == Operation::FloatStore
                    && (name_is(what, "fistp") || name_is(what, "fisttp"))
                    && what.sources.len() == 1
                    && matches!(
                        what.dests.as_slice(),
                        [Loc::Held(result)] if result.width == 2 || result.width == 4
                    ),
            )
    };
    if !body.iter_insns().any(converts) {
        return Ok(body.clone());
    }
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
    let unknown_readers =
        body.iter_insns().any(|one| one.what.as_ref().is_none_or(|what| what.op == Operation::Barrier));
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
        made.insns = insns.into();
        blocks.push(made);
    }
    let mut out = body.clone();
    out.blocks = blocks;
    Ok(out)
}

/// Whether the cell's address is made of SSA values, so every read of it names
/// the same bytes.
fn _stable(cell: &Mem) -> bool {
    match cell.addr {
        None => return false,
        Some(addr) if addr.space == Space::Far && cell.selector.is_none() => return false,
        Some(_) => {}
    }
    cell.base.is_some() || cell.index.is_some() || matches!(cell.through, RegId::None | RegId::BP)
}

fn _reached(cell: &Mem) -> Option<Addr> {
    // overlap reads a based address as reaching its whole region.
    let indexed = cell.base.is_some() || cell.index.is_some();
    if !indexed {
        return cell.addr;
    }
    let addr = cell.addr.expect("replace(None) raises");
    let base = if addr.base != RegId::None.iced() {
        addr.base
    } else if cell.through != RegId::None {
        cell.through.iced()
    } else {
        RegId::SI.iced()
    };
    Some(Addr { base, ..addr })
}

fn _may_write(
    one: &Insn,
    cell: &Mem,
) -> bool {
    let Some(what) = &one.what else {
        return true;
    };
    if matches!(what.op, Operation::Fill | Operation::Copy) {
        return true;
    }
    // A call or barrier writes what its MIR operation lists, or anything.
    if matches!(what.op, Operation::Call | Operation::Barrier) {
        if one.unmodeled_write() {
            return true;
        }
        let framed = cell.addr.is_some_and(|addr| addr.space == Space::Frame);
        if one.call.as_ref().is_some_and(|call| {
            call.writes() && !(framed && call.spares(crate::backend::bodyfacts::_frame_disp(cell), cell.width))
        }) {
            return true;
        }
    }
    if what.dests.iter().any(|dest| {
        matches!(dest, Loc::Mem(dest) if dest.addr.is_none()
            // Rust-only endpoint overflow reads as "may overlap".
            || overlap::may_overlap(_reached(dest), dest.width, _reached(cell), cell.width))
    }) {
        return true;
    }
    let address: HashSet<u32> =
        [cell.base, cell.index, cell.selector].into_iter().flatten().map(|arg| arg.value).collect();
    one.defines.iter().any(|value| address.contains(value))
}

pub(crate) fn _equivalent_loads(
    sequence: &[Arc<Insn>],
    live_out: &llrm_mir::dense::IdSet<u32>,
) -> IndexMap<u32, u32> {
    let mut available: IndexMap<(Option<std::sync::Arc<str>>, Mem), u32> = IndexMap::default();
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

pub(crate) fn _spill_home(
    value: u32,
    cell: Mem,
) -> Arc<Insn> {
    let load = semantics(Operation::FloatLoad, "fld", vec![Loc::Held(Held { value, width: 10 })], vec![Loc::Mem(cell)]);
    Arc::new(Insn { widths: vec![(value, 10)], defines: vec![value], ..inserted(0, load) })
}

pub(crate) fn _stored(
    value: u32,
    cell: &Mem,
    at: i64,
) -> Arc<Insn> {
    let what = semantics(
        Operation::FloatStore,
        "fstp",
        vec![Loc::Mem(cell.clone())],
        vec![Loc::Held(Held { value, width: 10 })],
    );
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

fn _aliased_by(
    body: &LirBody,
    live_out: &Live,
) -> (LirBody, bool) {
    let mut out = body.clone();
    let mut changed = false;
    for block in &mut out.blocks {
        let aliases = _equivalent_loads(&block.insns, &live_out[&block.at]);
        if aliases.is_empty() {
            continue;
        }
        changed = true;
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
                        Loc::Held(held) if held.width == 10 => {
                            Loc::Held(Held { value: canonical(held.value), ..*held })
                        }
                        _ => arg.clone(),
                    })
                    .collect();
                let mut made = (**one).clone();
                made.what = Some(Semantics { sources, ..what.clone() });
                made.uses =
                    one.uses.iter().map(|value| canonical(*value)).collect::<IndexSet<u32>>().into_iter().collect();
                Arc::new(made)
            })
            .collect();
    }
    (out, changed)
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

/// The legalization half of `assigned`, which greedy's RFP allocation
/// (RFP) leaves to run before it: an integer `fild` reads goes
/// through a cell, a float-to-integer store lands in one, and later loads of a
/// cell nothing wrote since are the first load's value. LLVM lowers the first
/// two in instruction selection.
pub fn prepared(
    body: &LirBody,
    mut frame: Option<&mut Frame>,
    pool: Option<&mut Pool>,
    basic_semantics: bool,
) -> Result<LirBody, Raised> {
    if _floating_values(body).is_empty()
        && !body.iter_insns().any(|one| {
            one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::FloatLoad | Operation::FloatStore))
        })
    {
        return Ok(body.clone());
    }
    let loaded = _integer_loads(body, frame.as_deref_mut(), pool)?;
    let body = _integer_stores(&loaded, frame.as_deref_mut(), basic_semantics)?;
    let floating = _floating_values(&body);
    if floating.is_empty() {
        return Ok(body);
    }
    // Only the floating values are asked of: no other value is followed.
    #[cfg(test)]
    ROWS_ASKED.with(|one| one.set(floating.len()));
    let (live_in, live_out) = live_among(&body, |value| floating.contains(&value));
    if live_in[&body.entry].iter().any(|value| floating.contains(&value)) {
        return Err(unlowered("floating stack input is unavailable"));
    }
    Ok(_sequenced(&_aliased_by(&body, &live_out).0))
}

/// A float compare clobbers the status-word register, as LLVM selects FCOM with
/// an `FNSTSW16r` that defines AX: no value lives there across it.
fn _compares_clobber(
    body: &LirBody,
    word: RegId,
) -> LirBody {
    let compares = |one: &Insn| {
        one.what
            .as_ref()
            .is_some_and(
                |what| what.op == Operation::Compare
                    && what.sources.iter().any(|source| matches!(source, Loc::Held(held) if held.width == 10)),
            )
    };
    if !body.iter_insns().any(|one| compares(one)) {
        return body.clone();
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let insns: Vec<Arc<Insn>> = block
                .insns
                .iter()
                .map(|one| {
                    if !compares(one) {
                        return Arc::clone(one);
                    }
                    let mut made = (**one).clone();
                    made.clobbers.insert(word);
                    Arc::new(made)
                })
                .collect();
            block.with_insns(insns.into())
        })
        .collect();
    body.with_blocks(blocks)
}

/// The x87 copies of each parallel-copy group as plain moves in an order that
/// reads before it writes, a temporary breaking a cycle: LLVM's PHI
/// elimination writes the copies one at a time, and greedy cannot place the
/// ten values a rotation of ten floats reads at once in seven registers.
fn _sequenced(body: &LirBody) -> LirBody {
    let float_copy = |one: &Insn| -> Option<(u32, u32)> {
        let what = one.what.as_ref()?;
        one.group?;
        if what.op != Operation::Move || what.name.as_deref() != Some("mov") {
            return None;
        }
        match (what.dests.as_slice(), what.sources.as_slice()) {
            ([Loc::Held(d)], [Loc::Held(s)]) if d.width == 10 && s.width == 10 => Some((d.value, s.value)),
            _ => None,
        }
    };
    if !body.iter_insns().any(|one| float_copy(one).is_some()) {
        return body.clone();
    }
    let mut fresh = body.next_value();
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let mut groups: Vec<i64> = Vec::new();
            for one in block.insns.iter() {
                if float_copy(one).is_some() {
                    let group = one.group.expect("a group");
                    if !groups.contains(&group) {
                        groups.push(group);
                    }
                }
            }
            if groups.is_empty() {
                return block.clone();
            }
            let mut insns: Vec<Arc<Insn>> = Vec::new();
            for one in block.insns.iter() {
                let Some((_, _)) = float_copy(one) else {
                    insns.push(Arc::clone(one));
                    continue;
                };
                let group = one.group.expect("a group");
                // The group is written out at its first float member.
                let Some(at) = groups.iter().position(|&g| g == group) else { continue };
                groups.remove(at);
                let members: Vec<Arc<Insn>> =
                    block.insns.iter().filter(|m| m.group == Some(group) && float_copy(m).is_some()).cloned().collect();
                let mut pending: Vec<(Arc<Insn>, u32, u32)> = members
                    .iter()
                    .filter_map(|m| float_copy(m).map(|(d, s)| (Arc::clone(m), d, s)))
                    .filter(|(_, d, s)| d != s)
                    .collect();
                let copy_of = |model: &Insn, d: u32, s: u32| -> Arc<Insn> {
                    let mut made = model.clone();
                    let what = made.what.as_mut().expect("a copy");
                    what.dests = vec![Loc::Held(Held { value: d, width: 10 })];
                    what.sources = vec![Loc::Held(Held { value: s, width: 10 })];
                    made.defines = vec![d];
                    made.uses = vec![s];
                    made.widths = vec![(d, 10), (s, 10)];
                    made.group = None;
                    Arc::new(made)
                };
                while !pending.is_empty() {
                    let ready = pending.iter().position(|(_, d, _)| !pending.iter().any(|(_, _, source)| source == d));
                    if let Some(at) = ready {
                        let (model, d, s) = pending.remove(at);
                        insns.push(copy_of(&model, d, s));
                        continue;
                    }
                    // Every destination is still read: save one, and read the
                    // saved copy where it was read.
                    let (model, d, _) = (Arc::clone(&pending[0].0), pending[0].1, pending[0].2);
                    let saved = fresh;
                    fresh += 1;
                    insns.push(copy_of(&model, saved, d));
                    for (_, _, source) in pending.iter_mut() {
                        if *source == d {
                            *source = saved;
                        }
                    }
                }
            }
            block.with_insns(insns.into())
        })
        .collect();
    body.with_blocks(blocks)
}

/// `prepared` as a machine phase: floatassign's slot under RFP.
pub struct FloatPrep {
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub pool: Option<Rc<RefCell<Pool>>>,
    pub basic_semantics: bool,
    /// The register `fnstsw` writes: the stackifier, after allocation, puts
    /// the status word there for every compare.
    pub status_word: Option<RegId>,
}

impl LIRTransform for FloatPrep {
    fn class_name(&self) -> &'static str {
        "FloatPrep"
    }

    fn name(&self) -> &str {
        "floatprep"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        let mut frame = self.frame.as_ref().map(|frame| frame.borrow_mut());
        let mut pool = self.pool.as_ref().map(|pool| pool.borrow_mut());
        let prepared = prepared(&body, frame.as_deref_mut(), pool.as_deref_mut(), self.basic_semantics)
            .map_err(|error| error.to_string())?;
        Ok(match self.status_word {
            Some(word) => _compares_clobber(&prepared, word),
            None => prepared,
        })
    }
}
