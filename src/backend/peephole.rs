//! Port of `qbopt/backend/peephole.py`: simplifications that depend on the
//! final physical register assignment.

use std::cell::RefCell;
use std::collections::BTreeSet;
use crate::support::hash::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use iced_x86::{Decoder, DecoderOptions, FlowControl, OpAccess, Register, RflagsBits};
use crate::support::hash::{IndexMap, IndexSet};

use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::frame::Frame;
use crate::backend::{
    copyprop, copysink, liveness, machinecse, machinedce, phielim, regthrash, select, spillforward, storecombine,
    target,
};
use crate::frontends::bc::declen;
use crate::model::ir::{self, Address, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::mir;
use crate::model::passes::LIRTransform;

pub use crate::backend::lanes::{Lane, Lanes};
/// Python's `dict[int, set]` keyed by `id(one)`.
pub type DeadAfter = HashMap<usize, Lanes>;
/// Python's `Counter[int]`: read with a 0 default.
pub type Counter = IndexMap<u32, i64>;

/// Python `id(one)` on an LIR instruction.
pub fn id(one: &Arc<Insn>) -> usize {
    Arc::as_ptr(one) as usize
}

fn count(counter: &Counter, value: u32) -> i64 {
    counter.get(&value).copied().unwrap_or(0)
}

fn emit(what: &Semantics) -> Option<select::Emitted> {
    select::emit(what, 0, None, false, false, None)
}

fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

fn with_what(one: &Insn, what: Semantics) -> Insn {
    Insn { what: Some(what), ..one.clone() }
}

fn deduped<T: Clone + Eq + std::hash::Hash>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    items.into_iter().collect::<IndexSet<T>>().into_iter().collect()
}

fn imm(value: i64, width: u32) -> Imm {
    Imm { value, width, address: None }
}

fn reg(register: Register, width: u32) -> Reg {
    Reg { register, width }
}

fn full32(register: Register) -> Register {
    register.full_register32()
}

pub struct Peephole {
    /// One frame, shared with the phases before this one, as Python shares it.
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub cpu: Profile,
}

impl Peephole {
    pub fn new<'a>(frame: Option<Rc<RefCell<Frame>>>, cpu: impl Into<ProfileOrName<'a>>) -> Result<Self, String> {
        Ok(Self { frame, cpu: targets::profile(cpu)?.clone() })
    }

    /// Drop only synthetic reservations when no added stack storage remains.
    fn _frame(&self, body: LirBody) -> LirBody {
        let Some(frame) = &self.frame else {
            return body;
        };
        let frame = frame.borrow();
        if frame.size() == 0 {
            return body;
        }
        for one in body.insns() {
            let Some(what) = &one.what else {
                return body;
            };
            if what.op == Operation::Barrier {
                return body;
            }
            for arg in what.sources.iter().chain(&what.dests) {
                let (address, through) = match arg {
                    Loc::Mem(cell) => (cell.addr, Some(cell.through)),
                    Loc::Address(cell) => (cell.addr, Some(cell.through)),
                    Loc::Imm(value) => (value.address, None),
                    _ => continue,
                };
                if address.is_none() && through.is_some() {
                    return body;
                }
                if through.is_some_and(|through| {
                    [Register::BP, Register::EBP, Register::SP, Register::ESP].contains(&through)
                }) && address.is_none_or(|address| address.space != Space::Frame)
                {
                    return body;
                }
                if address.is_some_and(|address| address.space == Space::Frame && address.disp < frame.floor) {
                    return body;
                }
            }
        }
        let blocks = body
            .blocks
            .iter()
            .map(|block| block.with_insns(lir::without(&block.insns, |one| one.frame_adjust, None::<fn(&Arc<Insn>) -> Arc<Insn>>)))
            .collect();
        LirBody { blocks, ..body }
    }
}

impl LIRTransform for Peephole {
    fn class_name(&self) -> &'static str {
        "Peephole"
    }

    fn name(&self) -> &str {
        "peephole"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        // Before the rest: a thrashed copy is one fewer instruction for
        // everything below to reason about, and it is the only pass here
        // that can remove a copy the coalescer refused on colourability.
        let body = regthrash::thrashed(phielim::unsplit(&body));
        let body = concatenated(&body);
        let body = frame_copies(&body, &self.cpu)?;
        let body = copyprop::forwarded(&body);
        let body = extensions(&body);
        let body = copysink::sunk(&body);
        let body = spillforward::forwarded(&body);
        let body = storecombine::combined(&body);
        let body = pushed_constants(&body);
        let body = far_loads(&fused(&overwritten(&shuttles(&restored_copies(&high_extracts(
            &transferred(&commuted(&constants(&pushes(&body)))),
            &self.cpu,
        )?)))));
        let body = addresses(&body, &self.cpu)?;
        let body = secondary_bases(&body, &self.cpu)?;
        let body = increments(&body);
        let body = machinecse::eliminated(&body)?;
        let body = waits(&zero_compares(&tested(&zeroes(&narrowed_moves(&body)))));
        Ok(self._frame(machinedce::eliminated(body)))
    }
}

/// Pack two word halves without using the stack.
///
/// CONCAT_LOW lowers portably to `push high; push low; pop wide` before
/// allocation.  Once the low word and the wide result share a physical root,
/// a 386 has BCC's two-instruction answer instead: shift the unknown upper
/// half away, then funnel the high word in with SHRD.  The original sequence
/// preserves flags, so this is legal only where physical flag liveness proves
/// the SHRD flags dead.
pub fn concatenated(body: &LirBody) -> LirBody {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(block, exits[&block.at].clone());
        let mut insns = block.insns.clone();
        for index in 0..insns.len().saturating_sub(2) {
            let (high_push, low_push, wide_pop) =
                (Arc::clone(&insns[index]), Arc::clone(&insns[index + 1]), Arc::clone(&insns[index + 2]));
            if high_push.op.as_ref().map(|op| op.kind) != Some(mir::Kind::Concat)
                || [&high_push, &low_push, &wide_pop].into_iter().any(|one| (one.what.is_none()
                || !one.clobbers.is_empty()
                || !one.clobbers_high.is_empty()
                || !one.requires.is_empty()
                || !one.delivers.is_empty()
                || !one.spread.is_empty()
                || one.group.is_some()
                || one.symbol == Some(true)
                || one.frame_adjust
                || one.spill_reload
                || one.spill_store))
            {
                continue;
            }
            let (Some(first), Some(second), Some(third)) = (&high_push.what, &low_push.what, &wide_pop.what) else {
                continue;
            };
            let (high, low, result) = match (
                (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
                (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
                (third.op, third.name.as_deref(), third.dests.as_slice(), third.sources.as_slice()),
            ) {
                (
                    (Operation::Push, Some("push"), [], [Loc::Reg(high)]),
                    (Operation::Push, Some("push"), [], [Loc::Reg(low)]),
                    (Operation::Pop, Some("pop"), [Loc::Reg(result)], []),
                ) => {
                    if high.width != low.width
                        || high.width != 2
                        || result.width != 4
                        || ir::root(low.register) != ir::root(result.register)
                        || ir::root(high.register) == ir::root(result.register)
                    {
                        continue;
                    }
                    (*high, *low, *result)
                }
                _ => continue,
            };
            let _ = low;
            let count = Loc::Imm(imm(16, 1));
            let wide_high = reg(target::named(high.register, 4), 4);
            let shifted = semantics(
                Operation::Binary,
                "shl",
                vec![Loc::Reg(result)],
                vec![Loc::Reg(result), count.clone()],
            );
            let funnelled = semantics(
                Operation::Funnel,
                "shrd",
                vec![Loc::Reg(result)],
                vec![Loc::Reg(result), Loc::Reg(wide_high), count],
            );
            let encoded: Vec<Option<select::Emitted>> = [&shifted, &funnelled].into_iter().map(emit).collect();
            if encoded.iter().any(Option::is_none) {
                continue;
            }
            let mut modified_flags = Lanes::new();
            for made in &encoded {
                let code = made.as_ref().map_or(Vec::new(), |made| made.code.clone());
                let mut decoder = Decoder::new(16, &code, DecoderOptions::NONE);
                for insn in &mut decoder {
                    modified_flags.extend(_flag_lanes(insn.rflags_modified()));
                }
            }
            if !modified_flags.is_subset(&dead_after[&id(&wide_pop)]) {
                continue;
            }
            insns[index] = lir::anchor(Arc::clone(&high_push));
            insns[index + 1] = Arc::new(with_what(&low_push, shifted));
            insns[index + 2] = Arc::new(with_what(&wide_pop, funnelled));
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// Use a dead GPR for an allocated frame-to-frame parallel copy.
///
/// Parallel-copy scheduling has to work even when every GPR is live, so its
/// unconditional spelling is a balanced PUSH-memory/POP-memory pair.  After
/// allocation, physical liveness can prove that a same-width register is
/// dead across a particular pair.  Two MOVs then avoid the temporary stack
/// traffic without changing flags, stack depth, or either frame address.
pub fn frame_copies<'a>(body: &LirBody, cpu: impl Into<ProfileOrName<'a>>) -> Result<LirBody, String> {
    let profile = targets::profile(cpu)?;
    let forms = ["push_m", "pop_m", "mov_rm", "mov_mr"];
    if !forms.iter().all(|form| profile.prices(form)) {
        return Ok(body.clone());
    }
    if profile.cost("mov_rm")? + profile.cost("mov_mr")? >= profile.cost("push_m")? + profile.cost("pop_m")? {
        return Ok(body.clone());
    }
    if !body.blocks.iter().any(|block| {
        block.insns.iter().zip(block.insns.iter().skip(1)).any(|(first, second)| {
            first.what.as_ref().is_some_and(|what| what.op == Operation::Push)
                && second.what.as_ref().is_some_and(|what| what.op == Operation::Pop)
        })
    }) {
        return Ok(body.clone()); // avoid whole-body liveness when no stack shuttle exists
    }

    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(block, exits[&block.at].clone());
        let mut insns = block.insns.clone();
        for index in 0..insns.len().saturating_sub(1) {
            let (pushed, popped) = (Arc::clone(&insns[index]), Arc::clone(&insns[index + 1]));
            if [&pushed, &popped].into_iter().any(|one| (one.what.is_none()
                || !one.clobbers.is_empty()
                || !one.clobbers_high.is_empty()
                || !one.requires.is_empty()
                || !one.delivers.is_empty()
                || !one.spread.is_empty()
                || one.group.is_some()
                || one.symbol == Some(true)
                || one.frame_adjust
                || one.spill_reload
                || one.spill_store)) {
                continue;
            }
            if pushed.at != popped.at
                || popped.covers != Some((pushed.at, pushed.at))
                || popped.op.is_some()
                || !pushed.defines.is_empty()
                || !pushed.uses.is_empty()
                || !popped.defines.is_empty()
                || !popped.uses.is_empty()
            {
                continue; // only parcopy's synthetic balanced transfer
            }
            let (Some(first), Some(second)) = (&pushed.what, &popped.what) else {
                continue;
            };
            let (source, destination) = match (
                (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
                (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
            ) {
                (
                    (Operation::Push, Some("push"), [], [Loc::Mem(source)]),
                    (Operation::Pop, Some("pop"), [Loc::Mem(destination)], []),
                ) => {
                    if source.width != destination.width
                        || ![2, 4].contains(&source.width)
                        || !_frame_cell(source)
                        || !_frame_cell(destination)
                    {
                        continue;
                    }
                    (source.clone(), destination.clone())
                }
                _ => continue,
            };
            let scratch = target::AVAILABLE
                .into_iter()
                .filter(|register| {
                    _lanes(target::named(*register, i64::from(source.width))).is_subset(&dead_after[&id(&popped)])
                })
                .map(|register| target::named(register, i64::from(source.width)))
                .next();
            let Some(scratch) = scratch else {
                continue;
            };
            let temporary = reg(scratch, source.width);
            let load = with_what(
                &pushed,
                semantics(Operation::Move, "mov", vec![Loc::Reg(temporary)], vec![Loc::Mem(source)]),
            );
            let store = with_what(
                &popped,
                semantics(Operation::Move, "mov", vec![Loc::Mem(destination)], vec![Loc::Reg(temporary)]),
            );
            if emit(load.what.as_ref().expect("set above")).is_none()
                || emit(store.what.as_ref().expect("set above")).is_none()
            {
                continue;
            }
            insns[index] = Arc::new(load);
            insns[index + 1] = Arc::new(store);
        }
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

/// Fold a load or transitive extension into one widening instruction.
///
/// This is deliberately post-allocation.  MIR says that both conversions
/// happen; x86 says that `movzx edx,byte ptr [m]` can implement the same
/// value as either `mov dx,[m]; movzx edx,dx` or
/// `movzx dx,byte ptr [m]; movzx edx,dx`.  A transitive extension requires
/// one physical register root for both results so it preserves every
/// incidental register byte.  A plain load may use another register only
/// when its SSA value has no other reader.
pub fn extensions(body: &LirBody) -> LirBody {
    let mut users = Counter::default();
    for block in &body.blocks {
        for one in &block.insns {
            for value in &one.uses {
                *users.entry(*value).or_insert(0) += 1;
            }
        }
    }
    for block in &body.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                *users.entry(*value).or_insert(0) += 1;
            }
        }
    }
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = block.insns.clone();
        // `enumerate(insns[:-1])` walks a copy taken before the loop.
        let snapshot: Vec<Arc<Insn>> = insns[..insns.len().saturating_sub(1)].to_vec();
        for (index, first) in snapshot.iter().enumerate() {
            for following in index + 1..insns.len() {
                let second = Arc::clone(&insns[following]);
                let made = _extension(first, &second, &users);
                if let Some(made) = made {
                    if _extension_may_move_before(&made, first, &insns[index + 1..following]) {
                        insns[index] = made;
                        // The first instruction now defines the final value.  The
                        // anchor keeps the second instruction's byte ownership
                        // without leaving a second virtual definition behind.
                        let mut anchored = (*lir::anchor(Arc::clone(&second))).clone();
                        anchored.defines = Vec::new();
                        anchored.uses = Vec::new();
                        insns[following] = Arc::new(anchored);
                        break;
                    }
                }
                if !first.defines.is_empty()
                    && first
                        .defines
                        .iter()
                        .any(|value| second.uses.contains(value) || second.defines.contains(value))
                {
                    break;
                }
            }
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// Whether widening the load at its original position crosses no register use.
///
/// Combining a load with a later extension keeps the memory read in place,
/// but writes the extension's wider destination earlier.  Independent
/// parameter loads may sit between the two, as source frontends commonly
/// batch their entry loads.  Refuse whenever those intervening instructions
/// observe or replace any newly-written physical lane.
fn _extension_may_move_before(made: &Insn, first: &Insn, crossed: &[Arc<Insn>]) -> bool {
    if crossed.is_empty() {
        // Nothing to cross. Asking anyway refused every unrolled clone, whose
        // effects `_register_effects` will not read.
        return true;
    }
    let original = _register_effects(first, false, true);
    let combined = _register_effects(made, false, true);
    let (Some(original), Some(combined)) = (original, combined) else {
        return false;
    };
    let newly_written: Lanes = combined.1.minus(&original.1);
    for one in crossed {
        let effects = _register_effects(one, false, true);
        match effects {
            None => return false,
            Some(effects) => {
                if newly_written.iter().any(|lane| effects.0.contains(lane) || effects.1.contains(lane)) {
                    return false;
                }
            }
        }
    }
    true
}

fn _extension(first: &Insn, second: &Insn, users: &Counter) -> Option<Arc<Insn>> {
    if [first, second].into_iter().any(|one| {
        one.what.is_none()
            || !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.frame_adjust
            || one.spill_reload
            || one.spill_store
    }) {
        return None;
    }
    // The follower forms below have register operands only, so even an
    // unroller's conservative `symbol=True` marker cannot belong to an
    // encoded relocation there.  The first instruction retains the actual
    // memory operand and its ownership; its replacement anchor clears the
    // follower marker.
    let (one, two) = (first.what.as_ref()?, second.what.as_ref()?);
    // `getattr(source, "width", 0)`.
    let width = |source: &Loc| match source {
        Loc::Reg(one) => one.width,
        Loc::Mem(one) => one.width,
        Loc::Imm(one) => one.width,
        Loc::Held(one) => one.width,
        Loc::Address(_) | Loc::St(_) => 0,
    };
    let (extension, destination, source) = match (
        (one.op, one.name.as_deref(), one.dests.as_slice(), one.sources.as_slice()),
        (two.op, two.name.as_deref(), two.dests.as_slice(), two.sources.as_slice()),
    ) {
        (
            (Operation::Move, Some("mov"), [Loc::Reg(middle)], [source @ Loc::Mem(cell)]),
            (
                Operation::Extend,
                Some(second_name @ ("movsx" | "movzx")),
                [Loc::Reg(destination)],
                [Loc::Reg(repeated)],
            ),
        ) => {
            if middle != repeated
                || !(cell.width == middle.width && middle.width < destination.width)
                || first.defines.len() != 1
                || second.uses != first.defines
                || count(users, first.defines[0]) != 1
            {
                return None;
            }
            (second_name, *destination, source.clone())
        }
        (
            (Operation::Extend, Some(first_name @ ("movsx" | "movzx")), [Loc::Reg(middle)], [source]),
            (
                Operation::Extend,
                Some(second_name @ ("movsx" | "movzx")),
                [Loc::Reg(destination)],
                [Loc::Reg(repeated)],
            ),
        ) => {
            if first_name != second_name
                || middle != repeated
                || ir::root(middle.register) != ir::root(destination.register)
                || !(width(source) < middle.width && middle.width < destination.width)
                || first.defines.len() != 1
                || second.uses != first.defines
                || count(users, first.defines[0]) != 1
            {
                return None;
            }
            (first_name, *destination, source.clone())
        }
        _ => return None,
    };
    let what = semantics(Operation::Extend, extension, vec![Loc::Reg(destination)], vec![source]);
    emit(&what)?;
    let uses = deduped(
        first
            .uses
            .iter()
            .copied()
            .chain(second.uses.iter().copied().filter(|value| !first.defines.contains(value))),
    );
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    for (value, width) in first.widths.iter().chain(&second.widths) {
        widths.insert(*value, *width);
    }
    Some(Arc::new(Insn {
        what: Some(what),
        defines: second.defines.clone(),
        uses,
        widths: widths.into_iter().collect(),
        ..first.clone()
    }))
}

/// Materialize a call's literal once when both stack and register need it.
pub fn pushed_constants(body: &LirBody) -> LirBody {
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut out: Vec<Arc<Insn>> = Vec::new();
        for one in &block.insns {
            if let Some(push) = out.last().cloned() {
                if let (Some(pushed), Some(moved)) = (&push.what, &one.what) {
                    if let (
                        (Operation::Push, Some("push"), [], [Loc::Imm(literal)]),
                        (Operation::Move, Some("mov"), [Loc::Reg(dest)], [source]),
                    ) = (
                        (pushed.op, pushed.name.as_deref(), pushed.dests.as_slice(), pushed.sources.as_slice()),
                        (moved.op, moved.name.as_deref(), moved.dests.as_slice(), moved.sources.as_slice()),
                    ) {
                        if Loc::Imm(literal.clone()) == *source
                            && literal.address.is_none()
                            && dest.width == literal.width
                            && [2, 4].contains(&literal.width)
                            && target::WIDTHS.get(&dest.register) == Some(&i64::from(dest.width))
                            && ![Register::ESP, Register::EBP].contains(&full32(dest.register))
                            && one.covers == Some((one.at, one.at))
                            && push.covers.is_some()
                            && push.covers.is_some_and(|covers| covers.1 == one.at)
                            && push.defines.is_empty()
                            && push.uses.is_empty()
                            && one.uses.is_empty()
                            && [&push, one].into_iter().all(|item| {
                                !(!item.clobbers.is_empty()
                                    || !item.requires.is_empty()
                                    || !item.delivers.is_empty()
                                    || !item.spread.is_empty()
                                    || item.group.is_some()
                                    || item.symbol == Some(true)
                                    || item.frame_adjust
                                    || item.spill_reload)
                            })
                        {
                            let dest = *dest;
                            let last = out.len() - 1;
                            out[last] = Arc::new(Insn {
                                at: push.at,
                                covers: Some((push.at, push.at)),
                                symbol: Some(false),
                                ..(**one).clone()
                            });
                            out.push(Arc::new(Insn {
                                what: Some(Semantics { sources: vec![Loc::Reg(dest)], ..pushed.clone() }),
                                uses: one.defines.clone(),
                                ..(*push).clone()
                            }));
                            continue;
                        }
                    }
                }
            }
            out.push(Arc::clone(one));
        }
        blocks.push(block.with_insns(out));
    }
    body.with_blocks(blocks)
}

/// Two adjacent immediate word pushes have one dword's stack layout.
pub fn pushes(body: &LirBody) -> LirBody {
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut out = Vec::new();
        let mut index = 0;
        while index < block.insns.len() {
            let pair = &block.insns[index..(index + 2).min(block.insns.len())];
            if pair.len() == 2
                && pair.iter().all(|one| {
                    !(!one.clobbers.is_empty()
                        || !one.requires.is_empty()
                        || !one.delivers.is_empty()
                        || !one.defines.is_empty()
                        || !one.uses.is_empty()
                        || one.symbol == Some(true)
                        || !one.spread.is_empty())
                })
            {
                if let (Some(first), Some(second)) = (&pair[0].what, &pair[1].what) {
                    if let (
                        (
                            Operation::Push,
                            Some("push"),
                            [],
                            [Loc::Imm(Imm { value: high, width: 2, address: None })],
                        ),
                        (Operation::Push, Some("push"), [], [Loc::Imm(Imm { value: low, width: 2, address: None })]),
                    ) = (
                        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
                        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
                    ) {
                        let what = semantics(
                            Operation::Push,
                            "push",
                            vec![],
                            vec![Loc::Imm(imm(((high & 0xFFFF) << 16) | (low & 0xFFFF), 4))],
                        );
                        let combined = Arc::new(with_what(&pair[0], what));
                        let removed = Arc::clone(&pair[1]);
                        let folded = lir::without(
                            &[combined, Arc::clone(&removed)],
                            |one| Arc::ptr_eq(one, &removed),
                            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
                        );
                        if folded.len() == 1 {
                            out.extend(folded);
                            index += 2;
                            continue;
                        }
                    }
                }
            }
            out.push(Arc::clone(&block.insns[index]));
            index += 1;
        }
        blocks.push(block.with_insns(out));
    }
    body.with_blocks(blocks)
}

pub fn _lanes(register: Register) -> Lanes {
    if target::SEGMENTS.contains(&register) {
        let width = target::width_of(register).expect("a segment register has a width");
        return (0..width).map(|byte| (register, byte as u32)).collect();
    }
    let full = full32(register);
    if ![
        Register::EAX,
        Register::EBX,
        Register::ECX,
        Register::EDX,
        Register::ESI,
        Register::EDI,
        Register::EBP,
    ]
    .contains(&full)
    {
        return Lanes::new();
    }
    let start = u32::from([Register::AH, Register::BH, Register::CH, Register::DH].contains(&register));
    (start..start + register.size() as u32).map(|byte| (full, byte)).collect()
}

/// Write only the live low word of a register-only dword move.
///
/// Frontends may naturally keep a scalar as a dword until an ABI boundary
/// that consumes only its low word.  Once allocation and physical liveness
/// prove the upper lanes dead, retaining the operand-size prefix and wide
/// immediate is not part of the program semantics.  Memory sources are
/// excluded because narrowing an access can change volatility or faults.
pub fn narrowed_moves(body: &LirBody) -> LirBody {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(block, exits[&block.at].clone());
        let mut insns = Vec::new();
        for one in &block.insns {
            let mut changed = None;
            if let Some(what) = &one.what {
                if what.op == Operation::Move
                    && what.name.as_deref() == Some("mov")
                    && what.dests.len() == 1
                    && what.sources.len() == 1
                    && matches!(&what.dests[0], Loc::Reg(dest) if dest.width == 4)
                    && matches!(&what.sources[0], Loc::Reg(Reg { width: 4, .. }) | Loc::Imm(Imm { width: 4, .. }))
                    && !(!one.clobbers.is_empty()
                        || !one.clobbers_high.is_empty()
                        || !one.requires.is_empty()
                        || !one.delivers.is_empty())
                    && one.group.is_none()
                    && one.symbol != Some(true)
                    && !one.frame_adjust
                {
                    let Loc::Reg(destination) = &what.dests[0] else { unreachable!() };
                    let upper: Lanes = _lanes(destination.register).into_iter().filter(|lane| lane.1 >= 2).collect();
                    if !upper.is_empty() && upper.is_subset(&dead_after[&id(one)]) {
                        let dest = reg(target::named(destination.register, 2), 2);
                        let source = match &what.sources[0] {
                            Loc::Reg(source) => Loc::Reg(reg(target::named(source.register, 2), 2)),
                            Loc::Imm(source) => Loc::Imm(imm(source.value & 0xFFFF, 2)),
                            _ => unreachable!(),
                        };
                        let candidate = Semantics { dests: vec![Loc::Reg(dest)], sources: vec![source], ..what.clone() };
                        if emit(&candidate).is_some() {
                            changed = Some(Arc::new(with_what(one, candidate)));
                        }
                    }
                }
            }
            insns.push(changed.unwrap_or_else(|| Arc::clone(one)));
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// Use a saved accumulator in place for commutative two-address operations.
pub fn commuted(body: &LirBody) -> LirBody {
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = block.insns.clone();
        let mut removed: HashSet<usize> = HashSet::default();
        for index in 2..insns.len() {
            let (saved, copied, combined) =
                (Arc::clone(&insns[index - 2]), Arc::clone(&insns[index - 1]), Arc::clone(&insns[index]));
            if [&saved, &copied, &combined].into_iter().any(|one| {
                removed.contains(&id(one))
                    || !one.clobbers.is_empty()
                    || !one.requires.is_empty()
                    || !one.delivers.is_empty()
                    || !one.spread.is_empty()
                    || one.group.is_some()
            }) {
                continue;
            }
            if copied.symbol == Some(true) || combined.symbol == Some(true) {
                continue;
            }
            let (Some(first), Some(second), Some(third)) = (&saved.what, &copied.what, &combined.what) else {
                continue;
            };
            if let (
                (Operation::Move, Some("mov"), [Loc::Reg(temporary)], [Loc::Reg(accumulator)]),
                (Operation::Move, Some("mov"), [destination], [Loc::Reg(term)]),
                (Operation::Binary, name, [result], [left, right]),
            ) = (
                (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
                (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
                (third.op, third.name.as_deref(), third.dests.as_slice(), third.sources.as_slice()),
            ) {
                let accumulated = Loc::Reg(*accumulator);
                if !name.is_some_and(|name| ["add", "and", "or", "xor"].contains(&name))
                    || !(accumulated == *destination && destination == result && result == left)
                    || *right != Loc::Reg(*temporary)
                    || !(accumulator.width == temporary.width && temporary.width == term.width)
                    || ![2, 4].contains(&accumulator.width)
                    || !_lanes(accumulator.register).is_disjoint(&_lanes(temporary.register))
                {
                    continue;
                }
                insns[index] = Arc::new(with_what(
                    &combined,
                    Semantics { sources: vec![accumulated, Loc::Reg(*term)], ..third.clone() },
                ));
                removed.insert(id(&copied));
            }
        }
        let insns = lir::without(&insns, |one| removed.contains(&id(one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>);
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// Write a commutative result directly into its copied destination.
///
/// Once registers are assigned, a tied `A = op(A, B); B = A` pair can be
/// spelled `B = op(B, A)` when A dies at the copy.  This is the physical
/// counterpart of two-address commutation: it asks about exact register-lane
/// liveness, so doing it in MIR or before allocation would be unsound.  Keep
/// the eliminated synthetic copy as a virtual anchor for later source-map and
/// SSA consumers.
pub fn transferred(body: &LirBody) -> LirBody {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(block, exits[&block.at].clone());
        let mut insns = Vec::new();
        let mut index = 0;
        while index < block.insns.len() {
            let pair = &block.insns[index..(index + 2).min(block.insns.len())];
            let changed = if pair.len() == 2 { _transferred(pair, &dead_after) } else { None };
            if let Some(changed) = changed {
                insns.extend(changed);
                index += 2;
                continue;
            }
            insns.push(Arc::clone(&block.insns[index]));
            index += 1;
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

fn _transferred(parts: &[Arc<Insn>], dead_after: &DeadAfter) -> Option<Vec<Arc<Insn>>> {
    let (combined, copied) = (&parts[0], &parts[1]);
    if parts.iter().any(|one| (one.what.is_none()
                || !one.clobbers.is_empty()
                || !one.clobbers_high.is_empty()
                || !one.requires.is_empty()
                || !one.delivers.is_empty()
                || !one.spread.is_empty()
                || one.group.is_some()
                || one.symbol == Some(true)
                || one.frame_adjust
                || one.spill_reload
                || one.spill_store)) {
        return None;
    }
    // Only an allocator/two-address copy owns no source bytes.  Removing a
    // source instruction is a different layout transformation and must retain
    // its own observable ownership contract.
    if copied.covers.is_none_or(|covers| covers.0 != covers.1) {
        return None;
    }
    let (first, second) = (combined.what.as_ref()?, copied.what.as_ref()?);
    let (left, right) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
    ) {
        (
            (operation, name, [Loc::Reg(destination)], [Loc::Reg(left), Loc::Reg(right)]),
            (Operation::Move, Some("mov"), [Loc::Reg(into)], [Loc::Reg(out_of)]),
        ) => {
            let commutative = operation == Operation::Binary
                && name.is_some_and(|name| ["add", "and", "or", "xor"].contains(&name))
                || operation == Operation::Multiply && name == Some("imul");
            if !commutative
                || destination != left
                || out_of != left
                || into != right
                || !(destination.width == left.width && left.width == right.width && right.width == into.width)
                || ir::root(left.register) == ir::root(right.register)
                || !_lanes(left.register).is_subset(&dead_after[&id(copied)])
            {
                return None;
            }
            (*left, *right)
        }
        _ => return None,
    };

    let what = Semantics { dests: vec![Loc::Reg(right)], sources: vec![Loc::Reg(right), Loc::Reg(left)], ..first.clone() };
    let before = emit(first);
    let after = emit(&what);
    let (Some(before), Some(after)) = (before, after) else {
        return None;
    };
    if before.code.len() != after.code.len() {
        return None;
    }
    Some(vec![Arc::new(with_what(combined, what)), lir::anchor(Arc::clone(copied))])
}

/// Select direct register or memory forms for a dword's high word.
///
/// A spilled value's high-half extraction can reach allocated LIR as
/// `mov R32,[slot]; shr R32,16`.  When only R16 survives, reading
/// `word [slot+2]` computes the same value without the shift.  The proof is
/// necessarily physical: the preserved upper register lanes and every flag
/// the shift would write must both be dead.
///
/// The portable register form is `push R32; pop dead16; pop high16`.  Once
/// registers are allocated, one `shld high32,R32,16` avoids all three stack
/// accesses when its wider destination lanes and flags are dead.  An already
/// selected `mov`/`shr` pair converges to that same final form.
pub fn high_extracts<'a>(body: &LirBody, cpu: impl Into<ProfileOrName<'a>>) -> Result<LirBody, String> {
    let profile = targets::profile(cpu)?;
    let mut virtual_uses = Counter::default();
    for block in &body.blocks {
        for one in &block.insns {
            for value in &one.uses {
                *virtual_uses.entry(*value).or_insert(0) += 1;
            }
        }
    }
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(block, exits[&block.at].clone());
        let mut insns = Vec::new();
        let mut index = 0;
        while index < block.insns.len() {
            let triple = &block.insns[index..(index + 3).min(block.insns.len())];
            let changed = if triple.len() == 3 {
                _register_high_extract(triple, &dead_after, &virtual_uses, profile)?
            } else {
                None
            };
            if let Some(changed) = changed {
                insns.extend(changed);
                index += 3;
                continue;
            }
            let pair = &block.insns[index..(index + 2).min(block.insns.len())];
            let changed = if pair.len() == 2 {
                match _selected_register_high_extract(pair, &dead_after, &virtual_uses, profile)? {
                    Some(changed) => Some(changed),
                    None => _high_extract(pair, &dead_after),
                }
            } else {
                None
            };
            if let Some(changed) = changed {
                insns.extend(changed);
                index += 2;
                continue;
            }
            insns.push(Arc::clone(&block.insns[index]));
            index += 1;
        }
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

fn _register_high_extract(
    parts: &[Arc<Insn>],
    dead_after: &DeadAfter,
    virtual_uses: &Counter,
    cpu: &Profile,
) -> Result<Option<Vec<Arc<Insn>>>, String> {
    let (pushed, discarded, kept) = (&parts[0], &parts[1], &parts[2]);
    if parts.iter().any(|one| (one.what.is_none()
                || !one.clobbers.is_empty()
                || !one.clobbers_high.is_empty()
                || !one.requires.is_empty()
                || !one.delivers.is_empty()
                || !one.spread.is_empty()
                || one.group.is_some()
                || one.symbol == Some(true)
                || one.frame_adjust
                || one.spill_reload
                || one.spill_store)) {
        return Ok(None);
    }
    let (Some(first), Some(second), Some(third)) = (&pushed.what, &discarded.what, &kept.what) else {
        return Ok(None);
    };
    let (source, high) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
        (third.op, third.name.as_deref(), third.dests.as_slice(), third.sources.as_slice()),
    ) {
        (
            (Operation::Push, Some("push"), [], [Loc::Reg(source @ Reg { width: 4, .. })]),
            (Operation::Pop, Some("pop"), [Loc::Reg(Reg { width: 2, .. })], []),
            (Operation::Pop, Some("pop"), [Loc::Reg(high @ Reg { width: 2, .. })], []),
        ) => (*source, *high),
        _ => return Ok(None),
    };
    if pushed.uses.len() != 1
        || discarded.defines.len() != 1
        || kept.defines.len() != 1
        || count(virtual_uses, discarded.defines[0]) != 0
    {
        return Ok(None);
    }
    let wide_high = reg(target::named(high.register, 4), 4);
    let count = Loc::Imm(imm(16, 1));
    let shift = semantics(
        Operation::Binary,
        "shr",
        vec![Loc::Reg(wide_high)],
        vec![Loc::Reg(wide_high), count.clone()],
    );

    let Some(effects) = _register_effects(&with_what(kept, shift.clone()), false, true) else {
        return Ok(None);
    };
    let upper: Lanes = _lanes(wide_high.register).minus(&_lanes(high.register));
    let flags: Lanes = effects.1.and(&_flag_lanes(0xFFFF_FFFF));
    if !upper.or(&flags).is_subset(&dead_after[&id(kept)]) {
        return Ok(None);
    }
    let old_cost = cpu.cost("push_r")? + 2 * cpu.cost("pop_r")?;
    if ir::root(source.register) == ir::root(high.register) {
        // The low-half extraction has already copied the return value away.
        // Reusing the dying source root for its own high half needs no move.
        // Lanes outside HIGH must die here because SHR writes the whole root.
        if _lanes(source.register)
            .difference(&_lanes(high.register))
            .copied()
            .collect::<Lanes>()
            .is_subset(&dead_after[&id(kept)])
            && cpu.cost("shift_ri")? <= old_cost
            && emit(&shift).is_some()
        {
            return Ok(Some(vec![Arc::new(Insn {
                what: Some(shift),
                defines: kept.defines.clone(),
                ..(**pushed).clone()
            })]));
        }
        return Ok(None);
    }
    // Only DX is part of the medium-model return ABI.  With EDX's upper lanes
    // dead, SHLD's otherwise-observable old-destination contribution is dead
    // too: EDX <- EDX<<16 | SOURCE>>16 puts SOURCE.high16 directly in DX.
    // SHRD would put SOURCE.low16 in EDX.high16 and leave DX unrelated.
    let extract = semantics(
        Operation::Funnel,
        "shld",
        vec![Loc::Reg(wide_high)],
        vec![Loc::Reg(wide_high), Loc::Reg(source), count],
    );
    if cpu.cost("shift_ri")? > old_cost || emit(&extract).is_none() {
        return Ok(None);
    }
    Ok(Some(vec![Arc::new(Insn { what: Some(extract), defines: kept.defines.clone(), ..(**pushed).clone() })]))
}

/// Collapse an allocated `mov R32,S32; shr R32,16` to one SHLD.
///
/// Lowering may select this shape directly while a portable half extraction
/// reaches the same point as PUSH/POP/POP.  Both spellings implement the same
/// operation when only the destination's low word survives, so final machine
/// selection must canonicalize both independently of frontend provenance.
fn _selected_register_high_extract(
    parts: &[Arc<Insn>],
    dead_after: &DeadAfter,
    virtual_uses: &Counter,
    cpu: &Profile,
) -> Result<Option<Vec<Arc<Insn>>>, String> {
    let (moved, shift) = (&parts[0], &parts[1]);
    if parts.iter().any(|one| (one.what.is_none()
                || !one.clobbers.is_empty()
                || !one.clobbers_high.is_empty()
                || !one.requires.is_empty()
                || !one.delivers.is_empty()
                || !one.spread.is_empty()
                || one.group.is_some()
                || one.symbol == Some(true)
                || one.frame_adjust
                || one.spill_reload
                || one.spill_store)) {
        return Ok(None);
    }
    let (Some(first), Some(second)) = (&moved.what, &shift.what) else {
        return Ok(None);
    };
    let (destination, source) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
    ) {
        (
            (
                Operation::Move,
                Some("mov"),
                [Loc::Reg(destination @ Reg { width: 4, .. })],
                [Loc::Reg(source @ Reg { width: 4, .. })],
            ),
            (
                Operation::Binary,
                Some("shr"),
                [Loc::Reg(shifted @ Reg { width: 4, .. })],
                [Loc::Reg(read @ Reg { width: 4, .. }), Loc::Imm(Imm { value: 16, width: 1, .. })],
            ),
        ) => {
            if destination != shifted || shifted != read {
                return Ok(None);
            }
            (*destination, *source)
        }
        _ => return Ok(None),
    };
    if ir::root(destination.register) == ir::root(source.register)
        || moved.defines.len() != 1
        || moved.uses.len() != 1
        || shift.defines.len() != 1
        || shift.uses.len() != 1
    {
        return Ok(None);
    }
    let shared_two_address = moved.defines == shift.defines && shift.defines == shift.uses;
    let temporary_chain = shift.uses == moved.defines && count(virtual_uses, moved.defines[0]) == 1;
    if !(shared_two_address || temporary_chain) {
        return Ok(None);
    }

    let count = Loc::Imm(imm(16, 1));
    let extract = semantics(
        Operation::Funnel,
        "shld",
        vec![Loc::Reg(destination)],
        vec![Loc::Reg(destination), Loc::Reg(source), count],
    );
    let old_effects = _register_effects(shift, false, true);
    let new_effects = _register_effects(&with_what(moved, extract.clone()), false, true);
    let (Some(old_effects), Some(new_effects)) = (old_effects, new_effects) else {
        return Ok(None);
    };
    let low = target::named(destination.register, 2);
    let upper: Lanes = _lanes(destination.register).minus(&_lanes(low));
    let flags: Lanes = old_effects
        .1
        .union(&new_effects.1)
        .copied()
        .collect::<Lanes>()
        .intersection(&_flag_lanes(0xFFFF_FFFF))
        .copied()
        .collect();
    if !upper.or(&flags).is_subset(&dead_after[&id(shift)]) {
        return Ok(None);
    }
    let new_cost = cpu.cost("shift_ri")?;
    let old_cost = cpu.cost("mov_rr")? + cpu.cost("shift_ri")?;
    if new_cost > old_cost {
        return Ok(None);
    }

    let combined = Arc::new(Insn { what: Some(extract), defines: shift.defines.clone(), ..(**moved).clone() });
    let folded = lir::without(
        &[combined, Arc::clone(shift)],
        |one| Arc::ptr_eq(one, shift),
        None::<fn(&Arc<Insn>) -> Arc<Insn>>,
    );
    Ok(if folded.len() == 1 { Some(folded) } else { None })
}

fn _high_extract(parts: &[Arc<Insn>], dead_after: &DeadAfter) -> Option<Vec<Arc<Insn>>> {
    let (load, shift) = (&parts[0], &parts[1]);
    if parts.iter().any(|one| {
        one.what.is_none()
            || !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
            || one.frame_adjust
            || one.spill_store
    }) {
        return None;
    }
    // Only a synthetic reload may be narrowed.  A source memory operation can
    // be volatile, fault on bytes no longer read, or otherwise make its full
    // access width observable.
    let source = load.op.as_ref();
    if !load.inserted()
        || load.symbol != Some(false)
        || source.is_none_or(|source| {
            source.source_backed || !source.loads.is_empty() || !source.stores.is_empty() || source.volatile
        })
    {
        return None;
    }
    let (first, second) = (load.what.as_ref()?, shift.what.as_ref()?);
    let (wide, cell) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
    ) {
        (
            (Operation::Move, Some("mov"), [Loc::Reg(wide @ Reg { width: 4, .. })], [Loc::Mem(cell)]),
            (
                Operation::Binary,
                Some("shr"),
                [Loc::Reg(destination @ Reg { width: 4, .. })],
                [Loc::Reg(shifted @ Reg { width: 4, .. }), Loc::Imm(Imm { value: 16, .. })],
            ),
        ) if cell.width == 4 => {
            if destination != wide
                || shifted != wide
                || !_frame_cell(cell)
                || cell.addr.is_none()
                || cell.index.is_some()
                || cell.selector.is_some()
            {
                return None;
            }
            (*wide, cell.clone())
        }
        _ => return None,
    };

    let low = reg(target::named(wide.register, 2), 2);
    let preserved: Lanes = _lanes(wide.register).minus(&_lanes(low.register));
    let effects = _register_effects(shift, false, true)?;
    let flags: Lanes = effects.1.and(&_flag_lanes(0xFFFF_FFFF));
    if !preserved.or(&flags).is_subset(&dead_after[&id(shift)]) {
        return None;
    }

    let address = cell.addr.expect("checked above");
    let high = Mem {
        addr: Some(ir::Addr { disp: address.disp + 2, ..address }),
        width: 2,
        offset: cell.offset + 2,
        ..cell
    };
    let what = semantics(Operation::Move, "mov", vec![Loc::Reg(low)], vec![Loc::Mem(high)]);

    emit(&what)?;
    Some(vec![Arc::new(with_what(load, what)), lir::anchor(Arc::clone(shift))])
}

/// Do tied work in its source register when a copy restores the result.
///
/// After allocation, `T = S; T = op(T); S = T` leaves both registers
/// holding the result. `S = op(S); T = S` leaves exactly the same physical
/// state and flags, while removing one move. This is deliberately after
/// allocation: globally joining the two virtual intervals can make a
/// colourable graph spill, whereas this local rewrite changes no interval.
pub fn shuttles(body: &LirBody) -> LirBody {
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut out = Vec::new();
        let mut index = 0;
        while index < block.insns.len() {
            let triple = &block.insns[index..(index + 3).min(block.insns.len())];
            let changed = if triple.len() == 3 { _shuttle(triple) } else { None };
            if let Some(changed) = changed {
                out.extend(changed);
                index += 3;
                continue;
            }
            out.push(Arc::clone(&block.insns[index]));
            index += 1;
        }
        blocks.push(block.with_insns(out));
    }
    body.with_blocks(blocks)
}

/// Remove a synthetic save/restore when the source survives between them.
///
/// Allocation may preserve a low word in a temporary around a portable
/// high-word extraction.  Once the extraction has become a direct copy and
/// shift of another register, the original source is visibly untouched.
/// Retain both virtual operations as anchors, but emit neither physical move
/// when no intervening instruction writes the source or observes/changes the
/// temporary and the temporary is dead after the restore.
pub fn restored_copies(body: &LirBody) -> LirBody {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(block, exits[&block.at].clone());
        let mut insns = block.insns.clone();
        let mut changed: HashSet<usize> = HashSet::default();
        // `enumerate(insns)` walks the live list: later replacements are seen.
        for index in 0..insns.len() {
            let saved = Arc::clone(&insns[index]);
            if changed.contains(&id(&saved)) || !_synthetic_register_copy(&saved) {
                continue;
            }
            let what = saved.what.as_ref().expect("a synthetic copy has semantics");
            let (temporary, source) = match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
            {
                (Operation::Move, Some("mov"), [Loc::Reg(temporary)], [Loc::Reg(source)]) => (*temporary, *source),
                _ => continue,
            };
            if temporary.width != source.width || ![1, 2, 4].contains(&temporary.width) {
                continue;
            }
            let (temporary_lanes, source_lanes) = (_lanes(temporary.register), _lanes(source.register));
            if temporary_lanes.is_empty()
                || source_lanes.is_empty()
                || !temporary_lanes.is_disjoint(&source_lanes)
            {
                continue;
            }
            let tail: Vec<Arc<Insn>> = insns[index + 1..].to_vec();
            for restored in &tail {
                if _inverse_synthetic_copy(restored, &source, &temporary) {
                    if temporary_lanes.is_subset(&dead_after[&id(restored)]) {
                        insns[index] = lir::anchor(Arc::clone(&saved));
                        // `list.index`: the first equal element after the save.
                        let restore_at = index
                            + 1
                            + insns[index + 1..]
                                .iter()
                                .position(|one| one == restored)
                                .expect("the restore is in the list");
                        insns[restore_at] = lir::anchor(Arc::clone(restored));
                        changed.insert(id(&saved));
                        changed.insert(id(restored));
                    }
                    break;
                }
                let Some((reads, writes)) = _register_effects(restored, true, false) else {
                    break;
                };
                if !reads.is_disjoint(&temporary_lanes)
                    || !writes.is_disjoint(&temporary_lanes.or(&source_lanes))
                {
                    break;
                }
            }
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

fn _synthetic_register_copy(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| {
        what.op == Operation::Move
            && what.name.as_deref() == Some("mov")
            && what.dests.len() == 1
            && what.sources.len() == 1
            && matches!(what.dests[0], Loc::Reg(_))
            && matches!(what.sources[0], Loc::Reg(_))
    }) && one.covers.is_some_and(|covers| covers.0 == covers.1)
        && !(!one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty())
        && one.group.is_none()
        && one.symbol != Some(true)
        && !(one.frame_adjust || one.spill_reload || one.spill_store)
}

fn _inverse_synthetic_copy(one: &Insn, destination: &Reg, source: &Reg) -> bool {
    if !_synthetic_register_copy(one) {
        return false;
    }
    let what = one.what.as_ref().expect("a synthetic copy has semantics");
    what.dests == [Loc::Reg(*destination)] && what.sources == [Loc::Reg(*source)]
}

fn _shuttle(parts: &[Arc<Insn>]) -> Option<Vec<Arc<Insn>>> {
    let (saved, combined, restored) = (&parts[0], &parts[1], &parts[2]);
    if parts.iter().any(|one| {
        one.what.is_none()
            || !one.clobbers.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
            || one.frame_adjust
            || one.spill_reload
    }) {
        return None;
    }
    if [saved, restored].into_iter().any(|one| one.covers.is_none_or(|covers| covers.0 != covers.1)) {
        return None;
    }
    let (first, second, third) = (saved.what.as_ref()?, combined.what.as_ref()?, restored.what.as_ref()?);
    let (temporary, source, operation, name, target_) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice(), second.target),
        (third.op, third.name.as_deref(), third.dests.as_slice(), third.sources.as_slice()),
    ) {
        (
            (Operation::Move, Some("mov"), [Loc::Reg(temporary)], [Loc::Reg(source)]),
            (operation, name, [destination], operands, target_),
            (Operation::Move, Some("mov"), [last_destination], [last_source]),
        ) => {
            if ![Operation::Binary, Operation::Unary, Operation::Multiply].contains(&operation)
                || *destination != Loc::Reg(*temporary)
                || operands.is_empty()
                || operands[0] != Loc::Reg(*temporary)
                || *last_destination != Loc::Reg(*source)
                || *last_source != Loc::Reg(*temporary)
                || temporary.width != source.width
                || ![1, 2, 4].contains(&temporary.width)
                || ir::root(temporary.register) == ir::root(source.register)
            {
                return None;
            }
            (*temporary, *source, operation, name, target_)
        }
        _ => return None,
    };

    let rewritten = Semantics {
        op: operation,
        name: name.map(str::to_owned),
        dests: second.dests.iter().map(|one| _register_operand(one, temporary.register, source.register)).collect(),
        sources: second
            .sources
            .iter()
            .map(|one| _register_operand(one, temporary.register, source.register))
            .collect(),
        target: target_,
        indirect: false,
    };

    emit(&rewritten)?;
    let reverse = semantics(Operation::Move, "mov", vec![Loc::Reg(temporary)], vec![Loc::Reg(source)]);
    Some(vec![Arc::new(with_what(combined, rewritten)), Arc::new(with_what(restored, reverse))])
}

/// One allocated operand with aliases of `before` renamed to `after`.
pub fn _register_operand(one: &Loc, before: Register, after: Register) -> Loc {
    // `target.named(after, target.width_of(x))`: a width the target does not
    // know leaves `after` as named.
    let named = |register: Register| target::width_of(register).map_or(after, |width| target::named(after, width));
    match one {
        Loc::Reg(one) if ir::root(one.register) == ir::root(before) => {
            Loc::Reg(Reg { register: target::named(after, i64::from(one.width)), ..*one })
        }
        Loc::Mem(one) if [ir::root(one.through), ir::root(one.index_through)].contains(&ir::root(before)) => {
            Loc::Mem(Mem {
                through: if ir::root(one.through) == ir::root(before) { named(one.through) } else { one.through },
                index_through: if ir::root(one.index_through) == ir::root(before) {
                    named(one.index_through)
                } else {
                    one.index_through
                },
                ..one.clone()
            })
        }
        Loc::Address(one) => {
            let through = if ir::root(one.through) == ir::root(before) { named(one.through) } else { one.through };
            let index = if ir::root(one.index) == ir::root(before) { named(one.index) } else { one.index };
            Loc::Address(Address { through, index, ..one.clone() })
        }
        _ => one.clone(),
    }
}

pub fn _register_effects(one: &Insn, may_write: bool, flags: bool) -> Option<(Lanes, Lanes)> {
    // A symbol/source anchor is a placement fact, not an unknown machine
    // instruction.  Complete unrolling can leave many of these between a
    // definition and its ABI use; clearing liveness at each one kept dead
    // upper register lanes alive and defeated width selection.  A NOTHING
    // carrying a real clobber mask remains a barrier below.
    if one.what.as_ref().is_some_and(|what| {
        what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty)
    }) && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
    {
        return Some((Lanes::new(), Lanes::new()));
    }
    if !one.clobbers.is_empty() || one.symbol == Some(true) {
        return None;
    }
    let what = match &one.what {
        Some(what) if what.op != Operation::Barrier => what,
        // `getattr(one.op, "node", None)`: a `mir.Op` has no `node`, so the
        // decoded-opaque fallback never applies and this is unknown.
        _ => return None,
    };
    let encoded = emit(what)?;
    let mut decoder = Decoder::new(16, &encoded.code, DecoderOptions::NONE);
    let instructions: Vec<iced_x86::Instruction> = (&mut decoder).into_iter().collect();
    // A fixed-register ABI names the conventional register (AX, BX, ...)
    // separately from the value it carries.  The Held width is authoritative:
    // a dword in the BX slot occupies EBX, including its upper lanes.
    let mut reads: Lanes = one
        .requires
        .iter()
        .flat_map(|(held, register)| _lanes(target::named(*register, i64::from(held.width))))
        .collect();
    let mut writes: Lanes = one
        .delivers
        .iter()
        .flat_map(|(held, register)| _lanes(target::named(*register, i64::from(held.width))))
        .collect();
    let mut info = declen::instruction_info_factory();
    for insn in &instructions {
        if insn.is_invalid() || insn.flow_control() != FlowControl::Next {
            return None;
        }
        if flags {
            let read: Lanes = _flag_lanes(insn.rflags_read()).minus(&writes);
            reads.extend(read);
            // An undefined flag is no more the incoming flag than a defined
            // result is. LLVM models both as physical-register definitions;
            // omitting Iced's undefined mask made TEST appear to preserve AF
            // and shifts appear to preserve every flag.
            writes.extend(_flag_lanes(insn.rflags_modified()));
        }
        let used: Vec<(Register, OpAccess)> = info
            .info(insn)
            .used_registers()
            .iter()
            .map(|access| (access.register(), access.access()))
            .collect();
        for (register, access) in &used {
            let lanes = _lanes(*register);
            if declen::READS.contains(access) {
                let read: Lanes = lanes.minus(&writes);
                reads.extend(read);
            }
        }
        for (register, access) in &used {
            if [OpAccess::Write, OpAccess::ReadWrite].contains(access)
                || may_write && [OpAccess::CondWrite, OpAccess::ReadCondWrite].contains(access)
            {
                writes.extend(_lanes(*register));
            }
        }
    }
    Some((reads, writes))
}

pub fn _flag_lanes(mask: u32) -> Lanes {
    Lanes::flags(mask)
}

/// A bp-relative slot whose bytes the displacement alone names.
pub fn _frame_cell(cell: &Mem) -> bool {
    cell.addr.is_some_and(|addr| addr.space == Space::Frame) && cell.through == Register::BP && cell.base.is_none()
}

/// Whether two such slots share a byte -- arithmetic on the displacements.
pub fn _overlapping(a: &Mem, b: &Mem) -> bool {
    if !_frame_cell(a) || !_frame_cell(b) {
        return true;
    }
    let (a_addr, b_addr) = (a.addr.expect("a frame cell"), b.addr.expect("a frame cell"));
    a_addr.disp < b_addr.disp + i64::from(b.width) && b_addr.disp < a_addr.disp + i64::from(a.width)
}

/// The one frame slot this instruction writes, if that is all it writes.
///
/// The allocator's own store says so itself. Any other instruction has to
/// prove it from the operation it came from, because an inserted store
/// carries the `op` of whatever it stands beside and that one's stores are
/// not its own.
pub fn _frame_written(one: &Insn) -> Option<Mem> {
    let what = one.what.as_ref()?;
    let cells: Vec<&Mem> = what
        .dests
        .iter()
        .filter_map(|dest| match dest {
            Loc::Mem(cell) => Some(cell),
            _ => None,
        })
        .collect();
    if cells.len() != 1 || !_frame_cell(cells[0]) {
        return None;
    }
    let stores: &[mir::MemRef] = one.op.as_ref().map_or(&[], |op| op.stores.as_slice());
    if !one.spill_store
        && (stores.len() > 1
            || stores.iter().any(|reference| {
                reference.addr.is_none_or(|addr| addr.space != Space::Frame) || reference.base.is_some()
            }))
    {
        return None;
    }
    Some(cells[0].clone())
}

/// Remove overwritten moves, pure arithmetic and owned spill reloads.
///
/// What is dead on exit from the block is a fact about the whole body, not
/// something a backward walk of one block can assume away: a phi's parallel
/// copy is written as the last instruction there, which is exactly where
/// "assume everything live" refuses to look.
pub fn overwritten(body: &LirBody) -> LirBody {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut dead = exits[&block.at].clone();
        let mut redundant: HashSet<usize> = HashSet::default();
        for one in block.insns.iter().rev() {
            if liveness::_terminator(one.what.as_ref()) {
                let what = one.what.as_ref().expect("a terminator has semantics");
                if what.op == Operation::Branch {
                    dead = dead.minus(&_branch_reads(what));
                }
                continue;
            }
            let Some(effects) = _register_effects(one, false, true) else {
                dead.clear();
                continue;
            };
            if let Some(what) = &one.what {
                match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
                    (
                        Operation::Move,
                        Some("mov"),
                        [Loc::Reg(dest)],
                        [source @ (Loc::Reg(_) | Loc::Imm(_) | Loc::Mem(_))],
                    ) => {
                        let lanes = _lanes(dest.register);
                        let width = match source {
                            Loc::Reg(one) => one.width,
                            Loc::Imm(one) => one.width,
                            Loc::Mem(one) => one.width,
                            _ => unreachable!(),
                        };
                        if !lanes.is_empty()
                            && lanes.is_subset(&dead)
                            && dest.width == width
                            && one.requires.is_empty()
                            && one.delivers.is_empty()
                            && (matches!(source, Loc::Reg(_))
                                || matches!(source, Loc::Imm(value) if value.address.is_none())
                                || matches!(source, Loc::Mem(_)) && one.spill_reload)
                        {
                            redundant.insert(id(one));
                            continue;
                        }
                    }
                    (Operation::Extend, Some("movsx" | "movzx"), [Loc::Reg(dest)], [Loc::Reg(_)]) => {
                        // Pure, and nothing else it writes. Lowering a divide's
                        // sign word as `cwd` leaves the widening it replaced
                        // behind, with a register and an instruction to its name.
                        if !effects.1.is_empty()
                            && effects.1.is_subset(&dead)
                            && !_lanes(dest.register).is_empty()
                            && one.requires.is_empty()
                            && one.delivers.is_empty()
                            && one.spread.is_empty()
                            && one.group.is_none()
                            && one.symbol != Some(true)
                        {
                            redundant.insert(id(one));
                            continue;
                        }
                    }
                    (Operation::Binary, Some("add" | "sub" | "and" | "or" | "xor"), [Loc::Reg(dest)], sources) => {
                        if !effects.1.is_empty()
                            && effects.1.is_subset(&dead)
                            && !_lanes(dest.register).is_empty()
                            && one.requires.is_empty()
                            && one.delivers.is_empty()
                            && one.spread.is_empty()
                            && one.group.is_none()
                            && sources.iter().all(|source| {
                                matches!(source, Loc::Reg(_))
                                    || matches!(source, Loc::Imm(value) if value.address.is_none())
                            })
                        {
                            redundant.insert(id(one));
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            let (reads, writes) = effects;
            dead = dead.or(&writes).minus(&reads);
        }
        let insns = block
            .insns
            .iter()
            .map(|one| if redundant.contains(&id(one)) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
            .collect();
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

const _FUSED_BINARY: [&str; 5] = ["add", "sub", "and", "or", "xor"];
const _FUSED_UNARY: [&str; 4] = ["inc", "dec", "neg", "not"];

/// `mov r,[m]; op r,x; mov [m],r` is `op [m],x`; `mov r,[m]; cmp r,x` is `cmp [m],x`.
///
/// The memory forms set the flags the register forms do and leave the cell
/// as the store did. What they no longer write is r, so nothing may read r
/// after, and r must be neither how the cell is reached nor the operand.
/// Only instructions that stand for no object bytes are dropped.
pub fn fused(body: &LirBody) -> LirBody {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = block.insns.clone();
        // Use the one physical-liveness implementation.  Its declared-call
        // and complete-return fallback knows that caller-clobbered registers
        // die at a function exit; this local copy used to treat RETURN as a
        // terminator before consulting that contract and kept C's last CX
        // load alive for no semantic reason.
        let dead_by_insn = regthrash::_dead_after(block, exits[&block.at].clone());
        let dead_after: Vec<Lanes> = insns.iter().map(|one| dead_by_insn[&id(one)].clone()).collect();
        // A NOTHING is no machine instruction even when it still carries an
        // SSA edge.  Allocation leaves such anchors behind for identity
        // copies; looking only through edge-free anchors made physically
        // adjacent loads and compares invisible here.
        let work: Vec<usize> = insns
            .iter()
            .enumerate()
            .filter(|(_, one)| !_nothing(one))
            .map(|(index, _)| index)
            .collect();
        let mut at = 0;
        while at + 1 < work.len() {
            let load_at = work[at];
            let mut candidate = at + 1;
            let mut changed = false;
            while candidate < work.len() {
                let work_at = work[candidate];
                let store_at = if candidate + 1 < work.len() { Some(work[candidate + 1]) } else { None };
                let made = _fused(
                    &insns[load_at],
                    &insns[work_at],
                    store_at.map(|store_at| &insns[store_at]),
                    &dead_after[work_at],
                    &store_at.map_or_else(Lanes::new, |store_at| dead_after[store_at].clone()),
                );
                if let Some((replacement, used)) = made {
                    insns[work_at] = replacement;
                    // Keep the virtual definitions and byte ownership.  The
                    // fused machine instruction replaces the physical
                    // load/store only; deleting either instruction also
                    // deletes SSA edges carried by identity-copy anchors.
                    insns[load_at] = lir::anchor(Arc::clone(&insns[load_at]));
                    if used == 3 {
                        let store_at = store_at.expect("a three-part fusion has a store");
                        insns[store_at] = lir::anchor(Arc::clone(&insns[store_at]));
                    }
                    at = candidate + used - 1;
                    changed = true;
                    break;
                }
                if !_delays_memory_read(&insns[load_at], &insns[work_at]) {
                    break;
                }
                candidate += 1;
            }
            if !changed {
                at += 1;
            }
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// Whether `load` may read its cell after this register materialization.
///
/// A load of the other arithmetic operand commonly separates a cell's own
/// load from its operation.  Delaying the cell read is safe when the crossed
/// instruction only materializes a register, does not consume or replace the
/// loaded register, and does not change anything used to address the cell.
/// Memory-writing instructions are deliberately outside this rule: proving
/// their disjointness belongs in MIR, not in a machine peephole.
fn _delays_memory_read(load: &Insn, crossed: &Insn) -> bool {
    let Some(crossed_what) = &crossed.what else {
        return false;
    };
    if !crossed.clobbers.is_empty()
        || !crossed.requires.is_empty()
        || !crossed.delivers.is_empty()
        || !crossed.spread.is_empty()
        || crossed.group.is_some()
        || crossed.symbol == Some(true)
        || crossed.frame_adjust
    {
        return false;
    }
    if !(matches!(crossed_what.op, Operation::Move | Operation::Extend | Operation::Address)
        && matches!(crossed_what.dests.as_slice(), [Loc::Reg(_)]))
    {
        return false;
    }
    let loaded = _register_effects(load, false, true);
    let materialized = _register_effects(crossed, false, true);
    let (Some((load_reads, load_writes)), Some((crossed_reads, crossed_writes))) = (loaded, materialized) else {
        return false;
    };
    let address_lanes: Lanes = match load.what.as_ref().map(|what| what.sources.as_slice()) {
        Some([Loc::Mem(cell)]) => {
            let mut address_registers = BTreeSet::from([cell.through, cell.index_through]);
            if let Some(addr) = cell.addr {
                address_registers.insert(addr.segment);
                // Once MIR computed a base value, allocation's `through` is
                // the encoded register and BC's original `addr.base` is only
                // provenance.  A cell with no value still encodes that base.
                if cell.base.is_none() {
                    address_registers.insert(addr.base);
                }
            }
            address_registers.into_iter().flat_map(_lanes).collect()
        }
        _ => return false,
    };
    let crossed_all: Lanes = crossed_reads.or(&crossed_writes);
    let load_all: Lanes = load_reads.or(&address_lanes);
    !(!load_writes.is_disjoint(&crossed_all)
        || !load_all.is_disjoint(&crossed_writes)
        || load.defines.iter().any(|value| crossed.uses.contains(value)))
}

fn _fused(
    load: &Insn,
    work: &Insn,
    store: Option<&Arc<Insn>>,
    dead_work: &Lanes,
    dead_store: &Lanes,
) -> Option<(Arc<Insn>, usize)> {
    let plain = |one: &Insn, dropped: bool| {
        !(one.what.is_none()
            || !one.clobbers.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
            || one.frame_adjust
            || dropped && one.covers.is_some_and(|covers| covers.0 != covers.1))
    };

    if !plain(load, true) || !plain(work, false) {
        return None;
    }
    let loaded = load.what.as_ref()?;
    let (extension, register, cell) =
        match (loaded.op, loaded.name.as_deref(), loaded.dests.as_slice(), loaded.sources.as_slice()) {
            (Operation::Move, Some("mov"), [Loc::Reg(register)], [Loc::Mem(cell)]) => {
                if register.width != cell.width {
                    return None;
                }
                (None, *register, cell.clone())
            }
            (Operation::Extend, Some(extension @ ("movsx" | "movzx")), [Loc::Reg(register)], [Loc::Mem(cell)]) => {
                if register.width <= cell.width {
                    return None;
                }
                (Some(extension), *register, cell.clone())
            }
            _ => return None,
        };
    let root = ir::root(register.register);
    let addresses_itself = [ir::root(cell.through), ir::root(cell.index_through)].contains(&root);
    let lanes = _lanes(register.register);

    let operand = |one: &Loc| {
        matches!(one, Loc::Imm(value) if value.address.is_none())
            || matches!(one, Loc::Reg(value) if ir::root(value.register) != root)
    };

    let stored = || {
        !addresses_itself
            && store.is_some_and(|store| {
                plain(store, true) && {
                    let what = store.what.as_ref().expect("plain has semantics");
                    what.op == Operation::Move
                        && what.name.as_deref() == Some("mov")
                        && what.dests == [Loc::Mem(cell.clone())]
                        && what.sources == [Loc::Reg(register)]
                }
            })
            && lanes.is_subset(dead_store)
    };

    let worked = work.what.as_ref()?;
    let (made, used) = match (worked.op, worked.name.as_deref(), worked.dests.as_slice(), worked.sources.as_slice()) {
        (Operation::Compare, Some("cmp"), [], [Loc::Reg(tested), other]) => {
            if *tested != register || !operand(other) || !lanes.is_subset(dead_work) {
                return None;
            }
            let mut other = other.clone();
            if let Some(extension) = extension {
                // Zero tests the widened value as it does the cell, but for SF: movsx copies
                // the cell's top bit as the narrow compare does, movzx leaves it clear.
                if !matches!(other, Loc::Imm(Imm { value: 0, .. })) {
                    return None;
                }
                if extension == "movzx" && !_flag_lanes(RflagsBits::SF).is_subset(dead_work) {
                    return None;
                }
                other = Loc::Imm(imm(0, cell.width));
            }
            (semantics(Operation::Compare, "cmp", vec![], vec![Loc::Mem(cell.clone()), other]), 2)
        }
        (Operation::Binary, name, [Loc::Reg(dest)], [Loc::Reg(source), other]) => {
            if extension.is_some() || !name.is_some_and(|name| _FUSED_BINARY.contains(&name)) {
                return None;
            }
            let name = name.expect("checked above");
            if dest == source && *source == register && operand(other) && stored() {
                (
                    semantics(Operation::Binary, name, vec![Loc::Mem(cell.clone())], vec![Loc::Mem(cell.clone()), other.clone()]),
                    3,
                )
            // A frontend load is not a register-allocation decision.  When
            // its only physical consumer accepts a memory source, retain the
            // read at that consumer and let the temporary die.  This is the
            // source-operand counterpart of the destination round trip above.
            } else if dest == source
                && matches!(other, Loc::Reg(other) if *other == register)
                && ir::root(dest.register) != root
                && lanes.is_subset(dead_work)
            {
                (
                    semantics(Operation::Binary, name, vec![Loc::Reg(*dest)], vec![Loc::Reg(*source), Loc::Mem(cell.clone())]),
                    2,
                )
            } else {
                return None;
            }
        }
        (Operation::Unary, name, [Loc::Reg(dest)], sources) => {
            if extension.is_some()
                || !name.is_some_and(|name| _FUSED_UNARY.contains(&name))
                || *dest != register
                || sources.iter().any(|one| *one != Loc::Reg(register))
                || !stored()
            {
                return None;
            }
            let name = name.expect("checked above");
            (
                semantics(
                    Operation::Unary,
                    name,
                    vec![Loc::Mem(cell.clone())],
                    sources.iter().map(|_| Loc::Mem(cell.clone())).collect(),
                ),
                3,
            )
        }
        _ => return None,
    };
    emit(&made)?;
    Some((Arc::new(with_what(work, made)), used))
}

/// `mov r,[m]; mov es,[m+2]`, in either order, is `les r,[m]` (and FS, GS).
///
/// One instruction reads both words before it writes either register, so
/// the register the pair writes first may not reach the word it reads
/// second. Only instructions that stand for no object bytes are joined.
pub fn far_loads(body: &LirBody) -> LirBody {
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = block.insns.clone();
        let work: Vec<usize> = insns
            .iter()
            .enumerate()
            .filter(|(_, one)| !_skippable_nothing(one))
            .map(|(index, _)| index)
            .collect();
        let mut removed: HashSet<usize> = HashSet::default();
        let mut at = 0;
        while at + 1 < work.len() {
            let Some(made) = _far_load(&insns[work[at]], &insns[work[at + 1]]) else {
                at += 1;
                continue;
            };
            insns[work[at]] = made;
            removed.insert(work[at + 1]);
            at += 2;
        }
        let insns = insns
            .into_iter()
            .enumerate()
            .filter(|(index, _)| !removed.contains(index))
            .map(|(_, one)| one)
            .collect();
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

fn _far_load(first: &Insn, second: &Insn) -> Option<Arc<Insn>> {
    let mut words: Vec<(Reg, Mem)> = Vec::new();
    for one in [first, second] {
        let Some(what) = &one.what else {
            return None;
        };
        if !one.clobbers.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
            || one.frame_adjust
            || one.covers.is_some_and(|covers| covers.0 != covers.1)
        {
            return None;
        }
        match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
            (Operation::Move, Some("mov"), [Loc::Reg(dest)], [Loc::Mem(cell)]) => {
                if dest.width != 2 || cell.width != 2 {
                    return None;
                }
                words.push((*dest, cell.clone()));
            }
            _ => return None,
        }
    }
    let segments: Vec<&(Reg, Mem)> =
        words.iter().filter(|word| select::FAR_LOADS.contains_key(&word.0.register)).collect();
    let offsets: Vec<&(Reg, Mem)> =
        words.iter().filter(|word| !target::SEGMENTS.contains(&word.0.register)).collect();
    if segments.len() != 1 || offsets.len() != 1 {
        return None;
    }
    let ((segment, high), (offset, low)) = (segments[0].clone(), offsets[0].clone());
    if !_next_word(&low, &high) {
        return None;
    }
    let (written, read) = (words[0].0, &words[1].1);
    if target::SEGMENTS.contains(&written.register) {
        if read.addr.is_some_and(|addr| addr.segment == written.register) {
            return None;
        }
    } else if [ir::root(read.through), ir::root(read.index_through)].contains(&ir::root(written.register)) {
        return None;
    }
    let made = semantics(
        Operation::Move,
        select::FAR_LOADS[&segment.register].0,
        vec![Loc::Reg(offset), Loc::Reg(segment)],
        vec![Loc::Mem(Mem { width: 4, ..low })],
    );
    emit(&made)?;
    Some(Arc::new(Insn {
        what: Some(made),
        defines: deduped(first.defines.iter().chain(&second.defines).copied()),
        uses: deduped(first.uses.iter().chain(&second.uses).copied()),
        ..first.clone()
    }))
}

/// Whether `high` is the word right after `low`, reached the same way.
///
/// The displacement may be carried by the address, by the operand's offset,
/// or by both at once, so either may be the one two further on.
fn _next_word(low: &Mem, high: &Mem) -> bool {
    let same = |cell: &Mem| Mem { addr: cell.addr.map(|addr| ir::Addr { disp: 0, ..addr }), offset: 0, ..cell.clone() };
    if same(low) != same(high) {
        return false;
    }
    let (Some(low_addr), Some(high_addr)) = (low.addr, high.addr) else {
        return low.addr.is_none() && high.addr.is_none() && high.offset == low.offset + 2;
    };
    let moved = high_addr.disp - low_addr.disp;
    moved == 2 && [0, 2].contains(&(high.offset - low.offset)) || moved == 0 && high.offset == low.offset + 2
}

/// Replace a dead temporary's shift with a scaled 67h LEA.
///
/// The load remains a load.  Only the register-only `shl; add` tail is
/// selected differently, after virtual use counts prove that no later use
/// expects the temporary to contain its shifted value.
fn _loaded_scaled_add<'a>(
    parts: &[Arc<Insn>],
    uses: &Counter,
    cpu: impl Into<ProfileOrName<'a>>,
) -> Result<Option<Arc<Insn>>, String> {
    if parts.len() != 3
        || parts.iter().any(|one| {
            one.what.is_none()
                || !one.clobbers.is_empty()
                || !one.clobbers_high.is_empty()
                || !one.spread.is_empty()
                || one.group.is_some()
                || one.frame_adjust
        })
    {
        return Ok(None);
    }
    let (load, shift, addition) = (&parts[0], &parts[1], &parts[2]);
    let (first, second, third) = (
        load.what.as_ref().expect("checked above"),
        shift.what.as_ref().expect("checked above"),
        addition.what.as_ref().expect("checked above"),
    );
    let (temporary, total, amount) = match (
        (first.op, first.name.as_deref(), first.dests.as_slice(), first.sources.as_slice()),
        (second.op, second.name.as_deref(), second.dests.as_slice(), second.sources.as_slice()),
        (third.op, third.name.as_deref(), third.dests.as_slice(), third.sources.as_slice()),
    ) {
        (
            (Operation::Move, Some("mov"), [Loc::Reg(temporary)], [Loc::Mem(_)]),
            (
                Operation::Binary,
                Some("shl"),
                [shift_dest],
                [shift_source, Loc::Imm(Imm { value: amount, address: None, .. })],
            ),
            (Operation::Binary, Some("add"), [Loc::Reg(total)], [add_left, add_right]),
        ) => {
            if *shift_dest != Loc::Reg(*temporary)
                || *shift_source != Loc::Reg(*temporary)
                || *add_left != Loc::Reg(*total)
                || *add_right != Loc::Reg(*temporary)
                || !(1..=3).contains(amount)
            {
                return Ok(None);
            }
            (*temporary, *total, *amount)
        }
        _ => return Ok(None),
    };
    if temporary.width != total.width
        || ![2, 4].contains(&temporary.width)
        || !target::WIDTHS.contains_key(&temporary.register)
        || !target::WIDTHS.contains_key(&total.register)
        || ir::root(temporary.register) == ir::root(total.register)
        || ir::root(temporary.register) == Register::ESP
        || load.defines.len() != 1
        || shift.defines != load.defines
        || shift.uses != load.defines
        || !addition.uses.contains(&load.defines[0])
        || count(uses, load.defines[0]) != 2
    {
        return Ok(None);
    }
    let target_cpu = targets::profile(cpu)?;
    let old = target_cpu.operations.shift + target_cpu.operations.add;
    let mut new = target_cpu.operations.address + target_cpu.operations.prefix;
    if temporary.width < 4 {
        new += target_cpu.partial_register_stall;
    }
    if new > old {
        return Ok(None);
    }
    let what = semantics(
        Operation::Address,
        "lea",
        vec![Loc::Reg(total)],
        vec![Loc::Address(Address {
            through: full32(total.register),
            index: full32(temporary.register),
            scale: 1 << amount,
            ..Address::new(None)
        })],
    );
    Ok(Some(Arc::new(with_what(addition, what))))
}

/// Select physically adjacent load/scale/add tails across inert anchors.
fn _loaded_addresses(block: &LirBlock, flags_dead_out: bool, uses: &Counter, cpu: &Profile) -> Result<LirBlock, String> {
    let dead = _flags_dead_after(block, flags_dead_out);
    let mut insns = block.insns.clone();
    let work: Vec<usize> = insns
        .iter()
        .enumerate()
        .filter(|(_, one)| !_skippable_nothing(one))
        .map(|(index, _)| index)
        .collect();
    let mut removed: HashSet<usize> = HashSet::default();
    let mut at = 0;
    while at + 2 < work.len() {
        let indexes = &work[at..at + 3];
        let parts: Vec<Arc<Insn>> = indexes.iter().map(|index| Arc::clone(&insns[*index])).collect();
        let combined = if dead.contains(&id(&parts[2])) { _loaded_scaled_add(&parts, uses, cpu)? } else { None };
        let Some(combined) = combined else {
            at += 1;
            continue;
        };
        let shift = Arc::clone(&parts[1]);
        if shift.symbol == Some(true) {
            insns[indexes[1]] = lir::anchor(Arc::clone(&shift));
            insns[indexes[2]] = combined;
            at += 3;
            continue;
        }
        let folded = lir::without(
            &[Arc::clone(&shift), combined],
            |one| Arc::ptr_eq(one, &shift),
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        if folded.len() != 1 {
            at += 1;
            continue;
        }
        insns[indexes[2]] = Arc::clone(&folded[0]);
        removed.insert(indexes[1]);
        at += 3;
    }
    let insns =
        insns.into_iter().enumerate().filter(|(index, _)| !removed.contains(index)).map(|(_, one)| one).collect();
    Ok(block.with_insns(insns))
}

/// The instructions of `block` after which no arithmetic flag is read,
/// given whether any is read after its end.
fn _flags_dead_after(block: &LirBlock, dead_out: bool) -> HashSet<usize> {
    let mut dead: HashSet<usize> = HashSet::default();
    let mut flags_dead = dead_out;
    for one in block.insns.iter().rev() {
        if flags_dead {
            dead.insert(id(one));
        }
        flags_dead = _flags_before(one, flags_dead);
    }
    dead
}

/// Whether replacing `parts`, followed in their block by `after`, drops a
/// definition something still reads.
///
/// Allocated instructions carry virtual identities that need not follow
/// their physical two-address spelling, and coalescing gives one identity
/// several definitions. A definition is lost only where a read reaches it:
/// later in the block before the identity is defined again, or past the
/// block's end (`live_out`).
pub(crate) fn _loses_live_definition(parts: &[Arc<Insn>], combined: &Insn, after: &[Arc<Insn>], live_out: &BTreeSet<u32>) -> bool {
    let eliminated: BTreeSet<u32> = parts
        .iter()
        .flat_map(|one| one.defines.iter().copied())
        .filter(|value| !combined.defines.contains(value))
        .collect();
    let reads = |one: &Insn, value: u32| one.uses.contains(&value) || one.requires.iter().any(|(held, _)| held.value == value);
    eliminated.iter().any(|value| {
        let mut redefined: Option<Option<i64>> = None;
        for one in after {
            match redefined {
                // A group reads before any member writes.
                Some(group) if group.is_some() && one.group == group => {
                    if reads(one, *value) {
                        return true;
                    }
                    continue;
                }
                Some(_) => return false,
                None => {}
            }
            if reads(one, *value) {
                return true;
            }
            if one.defines.contains(value) {
                redefined = Some(one.group);
            }
        }
        redefined.is_none() && live_out.contains(value)
    })
}

/// Select LEA for allocated arithmetic when the replaced flags are dead.
pub fn addresses<'a>(body: &LirBody, cpu: impl Into<ProfileOrName<'a>>) -> Result<LirBody, String> {
    let target_cpu = targets::profile(cpu)?;
    let mut virtual_uses = Counter::default();
    for block in &body.blocks {
        for one in &block.insns {
            for value in &one.uses {
                *virtual_uses.entry(*value).or_insert(0) += 1;
            }
        }
    }
    for block in &body.blocks {
        for one in &block.insns {
            for (held, _register) in &one.requires {
                *virtual_uses.entry(held.value).or_insert(0) += 1;
            }
        }
    }
    for block in &body.blocks {
        for phi in &block.phis {
            for (_source, value) in &phi.incoming {
                *virtual_uses.entry(*value).or_insert(0) += 1;
            }
        }
    }
    let (_, live_out) = crate::backend::allocate::live(body);
    let flags_out = _flags_live_out(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let flags_dead_out = flags_out.get(&block.at).is_some_and(Lanes::is_empty);
        let block = _loaded_addresses(block, flags_dead_out, &virtual_uses, target_cpu)?;
        let dead = _flags_dead_after(&block, flags_dead_out);
        let live_out = live_out.get(&block.at).cloned().unwrap_or_default();
        let mut insns = Vec::new();
        let mut index = 0;
        while index < block.insns.len() {
            let rest = &block.insns[index..];
            match _affine_address(rest, &dead, target_cpu)? {
                Some((taken, combined)) if !_loses_live_definition(&rest[..taken], &combined, &rest[taken..], &live_out) => {
                    insns.push(combined);
                    index += taken;
                }
                _ => {
                    insns.push(Arc::clone(&block.insns[index]));
                    index += 1;
                }
            }
        }
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

/// `ir.ROOT.get(register)`: `ROOT` itself is private to `ir`, and its keys
/// are exactly the registers `root` renames plus the eight 32-bit roots.
fn _root_get(register: Register) -> Option<Register> {
    let rooted = ir::root(register);
    let key = rooted != register
        || [
            Register::EAX,
            Register::EBX,
            Register::ECX,
            Register::EDX,
            Register::ESI,
            Register::EDI,
            Register::EBP,
            Register::ESP,
        ]
        .contains(&register);
    key.then_some(rooted)
}

/// Replace repeated allocated address shuttles with one clean 67h base.
///
/// Constrained-occurrence splitting keeps a long-lived word value in any GPR
/// and creates short BX/SI/DI copies at native memory uses.  Once allocation
/// has chosen physical registers, several such copies may be dearer than
/// zero-extending the owner once and addressing through its 32-bit root.  The
/// latter is the medium model's legal address-size-prefixed fallback.
///
/// This is deliberately post-allocation: it recognizes only copies created
/// by allocation, changes no program algebra, checks every virtual consumer,
/// and compares both the selected CPU cost and exact encoded byte totals.
pub fn secondary_bases<'a>(body: &LirBody, cpu: impl Into<ProfileOrName<'a>>) -> Result<LirBody, String> {
    let profile = targets::profile(cpu)?;
    let Some(secondary) = profile.address_forms.iter().find(|form| form.secondary && form.index_width == 4) else {
        return Ok(body.clone());
    };

    let mut definitions: IndexMap<u32, Option<Arc<Insn>>> = IndexMap::default();
    let mut uses: IndexMap<u32, Vec<Arc<Insn>>> = IndexMap::default();
    for one in body.insns() {
        for value in &one.defines {
            if definitions.contains_key(value) {
                definitions.insert(*value, None);
            } else {
                definitions.insert(*value, Some(Arc::clone(&one)));
            }
        }
        for value in one.uses.iter().copied().collect::<BTreeSet<u32>>() {
            uses.entry(value).or_default().push(Arc::clone(&one));
        }
    }

    let source_register = |one: &Insn, value: u32| -> Option<Reg> {
        let what = one.what.as_ref()?;
        for (named, destination) in one.defines.iter().zip(&what.dests) {
            if let Loc::Reg(destination) = destination {
                if *named == value && destination.width == 2 {
                    return Some(*destination);
                }
            }
        }
        None
    };

    let derived = |value: u32, original: u32, source: &Reg| -> Option<(Arc<Insn>, Vec<Arc<Insn>>)> {
        let definition = definitions.get(&value).cloned().flatten();
        let consumers = uses.get(&value).cloned().unwrap_or_default();
        let definition = definition?;
        if !definition.inserted() || definition.uses != [original] || consumers.len() != 1 {
            return None;
        }
        if let Some(what) = &definition.what {
            if what.op != Operation::Nothing {
                match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
                    (Operation::Move, Some("mov"), [Loc::Reg(destination)], [Loc::Reg(copied)]) => {
                        if destination.width != 2 || copied != source {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
        }
        let consumer = &consumers[0];
        if Arc::ptr_eq(consumer, &definition) {
            return None;
        }
        let consumed = consumer.what.as_ref()?;
        let mut seen = 0;
        for operand in consumed.dests.iter().chain(&consumed.sources) {
            let selector_alias =
                matches!(operand, Loc::Mem(cell) if cell.selector.is_some_and(|selector| selector.value == value));
            for held in ir::values(operand) {
                if held.value == value {
                    seen += 1;
                    if !matches!(operand, Loc::Mem(cell)
                        if cell.base == Some(held)
                            && cell.index.is_none()
                            && !selector_alias
                            // A frame cell has two independent address
                            // components: fixed BP and the allocated dynamic
                            // index.  This rewrite widens one value only, so it
                            // cannot legally turn the pair into a 32-bit address.
                            && !cell.addr.is_some_and(|addr| addr.space == Space::Frame)
                            && target::width_of(cell.through) == Some(2))
                    {
                        return None;
                    }
                }
            }
        }
        if seen != 0 { Some((definition, consumers)) } else { None }
    };

    let rewritten = |one: &Insn, substitutions: &IndexMap<u32, (u32, Register)>| -> Arc<Insn> {
        let operand = |where_: &Loc| -> Loc {
            let Loc::Mem(cell) = where_ else {
                return where_.clone();
            };
            let Some(base) = cell.base else {
                return where_.clone();
            };
            let Some((original, register)) = substitutions.get(&base.value) else {
                return where_.clone();
            };
            Loc::Mem(Mem { base: Some(Held { value: *original, width: 4 }), through: *register, ..cell.clone() })
        };
        let what = one.what.as_ref().expect("a rewritten consumer has semantics");
        Arc::new(Insn {
            what: Some(Semantics {
                dests: what.dests.iter().map(operand).collect(),
                sources: what.sources.iter().map(operand).collect(),
                ..what.clone()
            }),
            uses: one
                .uses
                .iter()
                .map(|value| substitutions.get(value).map_or(*value, |substituted| substituted.0))
                .collect(),
            ..one.clone()
        })
    };

    let mut substitutions: IndexMap<u32, (u32, Register)> = IndexMap::default();
    let mut remove: HashSet<usize> = HashSet::default();
    let mut insert_after: HashMap<usize, Arc<Insn>> = HashMap::default();
    for (candidate, definition) in definitions.clone() {
        let Some(definition) = definition else {
            continue;
        };
        let Some(source) = source_register(&definition, candidate) else {
            continue;
        };
        let Some(root) = _root_get(source.register) else {
            continue;
        };
        if target::width_of(root) != Some(4) {
            continue;
        }
        let mut group = Vec::new();
        for (value, made) in &definitions {
            if *value == candidate || made.is_none() {
                continue;
            }
            if let Some((made, found)) = derived(*value, candidate, &source) {
                group.push((*value, made, found));
            }
        }
        if group.is_empty() {
            continue;
        }
        let trial_substitutions: IndexMap<u32, (u32, Register)> =
            group.iter().map(|(value, _made, _consumers)| (*value, (candidate, root))).collect();
        let mut consumers: IndexMap<usize, Arc<Insn>> = IndexMap::default();
        for (_v, _d, found) in &group {
            for one in found {
                consumers.insert(id(one), rewritten(one, &trial_substitutions));
            }
        }
        let extension = Arc::new(Insn {
            op: definition.op.clone(),
            ..Insn::new(
                definition.at,
                Some((definition.at, definition.at)),
                Some(semantics(Operation::Extend, "movzx", vec![Loc::Reg(reg(root, 4))], vec![Loc::Reg(source)])),
                vec![candidate],
                vec![candidate],
            )
        });
        let copies: Vec<Semantics> = group
            .iter()
            .filter_map(|(_value, made, _found)| made.what.clone())
            .filter(|what| what.name.as_deref() == Some("mov"))
            .collect();
        let before_parts: Vec<Semantics> = copies
            .iter()
            .cloned()
            .chain(
                group
                    .iter()
                    .flat_map(|(_value, _made, found)| found.iter().map(|one| one.what.clone().expect("a consumer"))),
            )
            .collect();
        let after_parts: Vec<Semantics> = std::iter::once(extension.what.clone().expect("set above"))
            .chain(consumers.values().map(|one| one.what.clone().expect("a consumer")))
            .collect();
        let before_encoded: Vec<Option<select::Emitted>> = before_parts.iter().map(emit).collect();
        let after_encoded: Vec<Option<select::Emitted>> = after_parts.iter().map(emit).collect();
        if before_encoded.iter().chain(&after_encoded).any(Option::is_none) {
            continue;
        }
        let before_cost = copies.len() as i64 * profile.cost("mov_rr")?;
        // The defining word write is immediately read as a full register by
        // MOVZX.  This is exactly the partial-register transition charged by
        // the final scorer; omitting it made P6/Core select a locally smaller
        // sequence that was substantially slower under their own profiles.
        let after_cost =
            profile.cost("movzx")? + profile.partial_register_stall + consumers.len() as i64 * secondary.use_cost;
        let before_bytes: usize = before_encoded.iter().flatten().map(|one| one.code.len()).sum();
        let after_bytes: usize = after_encoded.iter().flatten().map(|one| one.code.len()).sum();
        if after_cost >= before_cost || after_bytes > before_bytes {
            continue;
        }
        substitutions.extend(trial_substitutions);
        remove.extend(group.iter().map(|(_value, made, _found)| id(made)));
        insert_after.insert(id(&definition), extension);
    }

    if substitutions.is_empty() {
        return Ok(body.clone());
    }
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            if remove.contains(&id(one)) {
                continue;
            }
            insns.push(if one.uses.iter().any(|value| substitutions.contains_key(value)) {
                rewritten(one, &substitutions)
            } else {
                Arc::clone(one)
            });
            if let Some(extension) = insert_after.get(&id(one)) {
                insns.push(Arc::clone(extension));
            }
        }
        blocks.push(block.with_insns(insns));
    }
    Ok(body.with_blocks(blocks))
}

/// One register's multiple, as an affine chain computes it.
type Terms = Vec<(Register, i64)>;

/// The 67h address naming `terms` plus `disp`, if one does.
/// `scales` are the index scales the target's 32-bit address form takes.
fn _affine_form(terms: &[(Register, i64)], disp: i64, scales: &BTreeSet<i64>) -> Option<Address> {
    let at = |through: Register, index: Register, scale: i64| {
        (index != Register::ESP && scales.contains(&scale))
            .then_some(Address { through, index, scale, offset: disp, ..Address::new(None) })
    };
    match *terms {
        [(only, 1)] => Some(Address { through: only, offset: disp, ..Address::new(None) }),
        [(only, scale)] => at(only, only, scale - 1).or_else(|| at(Register::None, only, scale)),
        [(base, 1), (index, scale)] | [(index, scale), (base, 1)] => {
            at(base, index, scale).or_else(|| at(index, base, 1).filter(|_| scale == 1))
        }
        _ => None,
    }
}

/// `mov z,y` and the arithmetic after it that only rewrites z, as one 67h LEA.
///
/// Each step keeps z an affine sum of registers: `add`/`sub` of a constant,
/// `inc`/`dec`, `shl` by a constant, `add z,z` and `add z,w`. The longest
/// prefix whose flags are dead (`dead`), whose sum an address names, and
/// which the target prices no dearer in cycles or bytes becomes one LEA. A
/// word result keeps only the low sixteen bits, which the upper halves of
/// the registers read cannot reach. Returns the instructions consumed.
fn _affine_address(
    parts: &[Arc<Insn>],
    dead: &HashSet<usize>,
    cpu: &Profile,
) -> Result<Option<(usize, Arc<Insn>)>, String> {
    let plain = |one: &Insn| {
        one.what.is_some()
            && one.clobbers.is_empty()
            && one.clobbers_high.is_empty()
            && one.spread.is_empty()
            && one.group.is_none()
            && !one.frame_adjust
    };
    let Some(copy) = parts.first().filter(|one| plain(one)) else {
        return Ok(None);
    };
    let Some(wide) = cpu.address_forms.iter().find(|form| form.secondary && form.index_width == 4) else {
        return Ok(None);
    };
    let copied = copy.what.as_ref().expect("checked plain");
    let (dest, source) = match (copied.op, copied.name.as_deref(), copied.dests.as_slice(), copied.sources.as_slice()) {
        (Operation::Move, Some("mov"), [Loc::Reg(dest)], [Loc::Reg(source)]) => (*dest, *source),
        _ => return Ok(None),
    };
    let register = |one: &Reg| one.width == dest.width && target::WIDTHS.contains_key(&one.register);
    let z = full32(dest.register);
    if ![2, 4].contains(&dest.width) || !register(&dest) || !register(&source) || full32(source.register) == z {
        return Ok(None);
    }
    let bits = i64::from(dest.width) * 8;
    let wrapped = |value: i64| (value + (1 << (bits - 1))).rem_euclid(1 << bits) - (1 << (bits - 1));
    let (mut terms, mut disp): (Terms, i64) = (vec![(full32(source.register), 1)], 0);
    let mut old = cpu.operations.r#move;
    let mut best: Option<(usize, Address, i64)> = None;
    for (at, one) in parts.iter().enumerate().skip(1) {
        if !plain(one) {
            break;
        }
        let what = one.what.as_ref().expect("checked plain");
        let [Loc::Reg(written)] = what.dests.as_slice() else {
            break;
        };
        if *written != dest {
            break;
        }
        let first = what.sources.first();
        if first.is_some_and(|first| *first != Loc::Reg(dest)) {
            break;
        }
        match (what.op, what.name.as_deref(), &what.sources[1..]) {
            (Operation::Binary, Some(name @ ("add" | "sub")), [Loc::Imm(Imm { value, address: None, .. })]) => {
                disp = wrapped(if name == "add" { disp + value } else { disp - value });
                old += cpu.operations.add;
            }
            (Operation::Unary, Some(name @ ("inc" | "dec")), []) => {
                disp = wrapped(if name == "inc" { disp + 1 } else { disp - 1 });
                old += cpu.operations.add;
            }
            (Operation::Binary, Some("shl" | "sal"), [Loc::Imm(Imm { value: count @ 1..=3, address: None, .. })]) => {
                terms.iter_mut().for_each(|term| term.1 <<= count);
                disp = wrapped(disp << count);
                old += cpu.operations.shift;
            }
            (Operation::Binary, Some("add"), [Loc::Reg(other)]) if *other == dest => {
                terms.iter_mut().for_each(|term| term.1 *= 2);
                disp = wrapped(disp * 2);
                old += cpu.operations.add;
            }
            (Operation::Binary, Some("add"), [Loc::Reg(other)]) if register(other) => {
                let root = full32(other.register);
                match terms.iter_mut().find(|term| term.0 == root) {
                    Some(term) => term.1 += 1,
                    None => terms.push((root, 1)),
                }
                old += cpu.operations.add;
            }
            _ => break,
        }
        if terms.len() > 2 || terms.iter().any(|term| term.1 > 9) {
            break;
        }
        if !dead.contains(&id(one)) {
            continue;
        }
        if let Some(address) = _affine_form(&terms, disp, &wide.scales) {
            best = Some((at, address, old));
        }
    }
    let Some((last, address, old)) = best else {
        return Ok(None);
    };
    let replaced = &parts[..=last];
    let partial: HashSet<Register> = terms.iter().map(|term| term.0).collect();
    let stalls = if dest.width < 4 { partial.len() as i64 * cpu.partial_register_stall } else { 0 };
    if cpu.operations.address + cpu.operations.prefix + stalls > old {
        return Ok(None);
    }
    let what = semantics(Operation::Address, "lea", vec![Loc::Reg(dest)], vec![Loc::Address(address)]);
    let (Some(new), Some(before)) = (
        emit(&what),
        replaced.iter().map(|one| emit(one.what.as_ref().expect("checked plain"))).collect::<Option<Vec<_>>>(),
    ) else {
        return Ok(None);
    };
    if new.code.len() > before.iter().map(|one| one.code.len()).sum() {
        return Ok(None);
    }
    // One instruction owns the source interval the LEA keeps: a single
    // wider owner, or intervals that meet end to start.
    let owned: Vec<&Arc<Insn>> =
        replaced.iter().filter(|one| one.covers.is_some_and(|(start, end)| start != end)).collect();
    if owned.windows(2).any(|pair| pair[0].covers.expect("owned").1 != pair[1].covers.expect("owned").0) {
        return Ok(None);
    }
    let owner = owned.first().copied().unwrap_or(copy);
    let symbolic: Vec<&Arc<Insn>> = replaced.iter().filter(|one| one.symbol == Some(true)).collect();
    if !symbolic.is_empty() && (symbolic.len() != 1 || !Arc::ptr_eq(symbolic[0], owner)) {
        return Ok(None);
    }
    let covers = match (owned.first(), owned.last()) {
        (Some(first), Some(end)) => Some((first.covers.expect("owned").0, end.covers.expect("owned").1)),
        _ => owner.covers,
    };
    let result = &replaced[last];
    let intermediate: HashSet<u32> = replaced[..last].iter().flat_map(|one| one.defines.iter().copied()).collect();
    let uses = deduped(
        copy.uses
            .iter()
            .copied()
            .chain(replaced[1..].iter().flat_map(|one| one.uses.iter().copied()).filter(|value| !intermediate.contains(value))),
    );
    let live: HashSet<u32> = uses.iter().chain(&result.defines).copied().collect();
    let widths = deduped(replaced.iter().flat_map(|one| one.widths.iter().copied()).filter(|pair| live.contains(&pair.0)));
    Ok(Some((
        last + 1,
        Arc::new(Insn {
            what: Some(what),
            defines: result.defines.clone(),
            uses,
            widths,
            requires: deduped(replaced.iter().flat_map(|one| one.requires.iter().copied())),
            delivers: deduped(replaced.iter().flat_map(|one| one.delivers.iter().copied())),
            covers,
            ..(**owner).clone()
        }),
    )))
}

/// Select compact INC/DEC for a unit add whose carry result is dead.
///
/// MIR deliberately treats a source `inc` and `add x,1` as the same
/// arithmetic value.  Their allocated x86 forms differ only in CF: INC/DEC
/// preserve it.  Once physical flag liveness proves CF unobserved, retaining
/// the frontend's original spelling is neither semantic nor profitable.
pub fn increments(body: &LirBody) -> LirBody {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    let carry = _flag_lanes(RflagsBits::CF);
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(block, exits[&block.at].clone());
        let mut insns = Vec::new();
        for one in &block.insns {
            let mut one = Arc::clone(one);
            let mut candidate = None;
            if let Some(what) = &one.what {
                if let (
                    Operation::Binary,
                    Some(name @ ("add" | "sub")),
                    [Loc::Reg(destination)],
                    [Loc::Reg(source), Loc::Imm(Imm { value: 1, address: None, .. })],
                ) = (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
                {
                    if destination == source {
                        candidate = Some(semantics(
                            Operation::Unary,
                            if name == "add" { "inc" } else { "dec" },
                            vec![Loc::Reg(*destination)],
                            vec![Loc::Reg(*source)],
                        ));
                    }
                }
            }
            if let Some(candidate) = candidate {
                if carry.is_subset(&dead_after[&id(&one)]) {
                    let (before, after) = (emit(one.what.as_ref().expect("matched above")), emit(&candidate));
                    if let (Some(before), Some(after)) = (before, after) {
                        if after.code.len() <= before.code.len() {
                            one = Arc::new(with_what(&one, candidate));
                        }
                    }
                }
            }
            insns.push(one);
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

fn _flags_before(one: &Insn, flags_dead: bool) -> bool {
    let Some(what) = &one.what else {
        return false;
    };
    if !one.clobbers.is_empty() {
        return false;
    }
    match (what.op, what.name.as_deref()) {
        (Operation::Compare, Some("cmp" | "test")) => true,
        (Operation::Binary, Some("add" | "sub" | "and" | "or" | "xor")) => true,
        (Operation::Unary, Some("neg")) => true,
        // INC/DEC preserve carry, but a carry which is dead afterwards
        // is dead beforehand too.  Other arithmetic flags are replaced
        // by the operation, so an all-dead state crosses it unchanged.
        (Operation::Unary, Some("inc" | "dec")) => flags_dead,
        (Operation::Move, Some("mov")) | (Operation::Address, Some("lea")) => flags_dead,
        (Operation::Extend, Some("movsx" | "movzx" | "cwd" | "cdq")) => flags_dead,
        (Operation::Push, Some("push")) | (Operation::Pop, Some("pop")) => flags_dead,
        (Operation::Nothing, None | Some("")) | (Operation::Jump, _) | (Operation::Fill, _) => flags_dead,
        _ => {
            // Anything else by what it encodes: a far load writes no flag, and
            // missing from the list above it kept `mov ax,0` from becoming `xor`.
            let Some((reads, writes)) = _register_effects(one, false, true) else {
                return false;
            };
            if !reads.is_disjoint(&_ARITHMETIC_LANES) {
                return false;
            }
            flags_dead || _ARITHMETIC_LANES.is_subset(&writes)
        }
    }
}

/// `_ZERO_BRANCHES`, in its insertion order.
const _ZERO_BRANCHES: [(&str, u32); 4] =
    [("je", RflagsBits::ZF), ("jne", RflagsBits::ZF), ("js", RflagsBits::SF), ("jns", RflagsBits::SF)];

/// `_BRANCH_FLAGS`, in its insertion order.
const _BRANCH_FLAGS: [(&str, u32); 12] = [
    ("je", RflagsBits::ZF),
    ("jne", RflagsBits::ZF),
    ("js", RflagsBits::SF),
    ("jns", RflagsBits::SF),
    ("jl", RflagsBits::SF | RflagsBits::OF),
    ("jge", RflagsBits::SF | RflagsBits::OF),
    ("jle", RflagsBits::ZF | RflagsBits::SF | RflagsBits::OF),
    ("jg", RflagsBits::ZF | RflagsBits::SF | RflagsBits::OF),
    ("jb", RflagsBits::CF),
    ("jae", RflagsBits::CF),
    ("jbe", RflagsBits::CF | RflagsBits::ZF),
    ("ja", RflagsBits::CF | RflagsBits::ZF),
];

fn _lookup(table: &[(&str, u32)], name: Option<&str>) -> Option<u32> {
    let name = name?;
    table.iter().find(|(key, _)| *key == name).map(|(_, mask)| *mask)
}

/// The flags a conditional jump reads: those its condition names, or all where this does not know it.
pub fn _branch_reads(what: &Semantics) -> Lanes {
    _flag_lanes(_lookup(&_BRANCH_FLAGS, Some(what.name.as_deref().unwrap_or(""))).unwrap_or(0xFFFF_FFFF))
}

const _ARITHMETIC: u32 =
    RflagsBits::OF | RflagsBits::SF | RflagsBits::ZF | RflagsBits::AF | RflagsBits::CF | RflagsBits::PF;
// What `cmp r,0` leaves that the instruction computing r may not: inc keeps
// the carry, add and subtract set carry and overflow from their operands.
static _DIFFERING: LazyLock<Lanes> = LazyLock::new(|| _flag_lanes(RflagsBits::OF | RflagsBits::CF | RflagsBits::AF));
// What `xor r,r` writes: a direction flag read later, as a string fill reads it, is no objection.
static _ARITHMETIC_LANES: LazyLock<Lanes> = LazyLock::new(|| _flag_lanes(_ARITHMETIC));

/// `inc edi; cmp edi,0; jne` is `inc edi; jne`.
///
/// An add, subtract, logic or unary operation sets ZF and SF from its result
/// exactly as a zero test of that result does. The test goes where only its
/// branch reads those two, and no later instruction reads the carry,
/// overflow or adjust flags it would have cleared.
pub fn tested(body: &LirBody) -> LirBody {
    let live = _flags_live_out(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = block.insns.clone();
        // Moves change no flag, so the three may have a phi's copies between them.
        let work: Vec<usize> = insns
            .iter()
            .enumerate()
            .filter(|(_, one)| !_skippable_nothing(one))
            .map(|(index, _)| index)
            .collect();
        let at = |position: isize| work[usize::try_from(position).expect("a non-negative position")];
        let mut test_at = work.len() as isize - 2;
        while test_at >= 1 && _moves(&insns[at(test_at)], None) {
            test_at -= 1;
        }
        let register = if test_at >= 1 { _zero_tested(&insns[at(test_at)]) } else { None };
        let mut before_at = test_at - 1;
        while let Some(register) = &register {
            if !(before_at >= 0 && _moves(&insns[at(before_at)], Some(register))) {
                break;
            }
            before_at -= 1;
        }
        if let Some(register) = register {
            if before_at >= 0 && !work.is_empty() {
                let (before, test, branch) = (
                    Arc::clone(&insns[at(before_at)]),
                    Arc::clone(&insns[at(test_at)]),
                    Arc::clone(&insns[*work.last().expect("checked above")]),
                );
                if branch.what.as_ref().is_some_and(|what| {
                    what.op == Operation::Branch && _lookup(&_ZERO_BRANCHES, what.name.as_deref()).is_some()
                }) && _sets_from(&before, &register)
                    && live[&block.at].is_disjoint(&_DIFFERING)
                {
                    insns[at(test_at)] = Arc::new(Insn {
                        what: Some(semantics(Operation::Nothing, "", vec![], vec![])),
                        defines: Vec::new(),
                        uses: Vec::new(),
                        widths: Vec::new(),
                        ..(*test).clone()
                    });
                }
            }
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

static _ADJUST: LazyLock<Lanes> = LazyLock::new(|| _flag_lanes(RflagsBits::AF));
const _FLAG_READERS: [&str; 16] = [
    "adc", "sbb", "rcl", "rcr", "lahf", "pushf", "pushfd", "daa", "das", "aaa", "aas", "into", "int", "iret", "cmc",
    "salc",
];

fn _reads_flags(name: &str) -> bool {
    _FLAG_READERS.contains(&name) || ["j", "set", "cmov", "loop"].iter().any(|prefix| name.starts_with(prefix))
}

/// `cmp r,0; jcc` is `or r,r; jcc`, a byte shorter.
///
/// Both clear carry and overflow and set zero, sign and parity from r; only
/// the adjust flag differs, so the branch must be the next work and nothing
/// after the block may read AF. The branch reading straight after keeps OF
/// clear of anything between -- DOSBox's dynamic core loses it across OR and
/// SAHF.
pub fn zero_compares(body: &LirBody) -> LirBody {
    let live = _flags_live_out(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = block.insns.clone();
        let work: Vec<usize> = insns
            .iter()
            .enumerate()
            .filter(|(_, one)| !_skippable_nothing(one))
            .map(|(index, _)| index)
            .collect();
        let mut at = work.len() as isize - 2;
        // Moves change no flag, so a phi's copies may stand between the two.
        while at >= 0 && _moves(&insns[work[at as usize]], None) {
            at -= 1;
        }
        if at >= 0 && live[&block.at].is_disjoint(&_ADJUST) {
            let (test, branch) = (Arc::clone(&insns[work[at as usize]]), Arc::clone(&insns[work[work.len() - 1]]));
            let register = _zero_tested(&test);
            if let Some(register) = register {
                if target::WIDTHS.contains_key(&register.register)
                    && branch.what.as_ref().is_some_and(|what| {
                        what.op == Operation::Branch && _lookup(&_BRANCH_FLAGS, what.name.as_deref()).is_some()
                    })
                    && test.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("cmp"))
                {
                    insns[work[at as usize]] = Arc::new(with_what(
                        &test,
                        semantics(
                            Operation::Binary,
                            "or",
                            vec![Loc::Reg(register)],
                            vec![Loc::Reg(register), Loc::Reg(register)],
                        ),
                    ));
                }
            }
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// A plain move, writing nothing that shares a root with `register`.
fn _moves(one: &Insn, register: Option<&Reg>) -> bool {
    let Some(what) = &one.what else {
        return false;
    };
    if what.op != Operation::Move || what.name.as_deref() != Some("mov") || !one.clobbers.is_empty() {
        return false;
    }
    register.is_none_or(|register| {
        what.dests.iter().all(|dest| match dest {
            Loc::Reg(dest) => ir::root(dest.register) != ir::root(register.register),
            _ => true,
        })
    })
}

pub fn _nothing(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty))
}

/// A no-op with no virtual edge, safe to skip for physical adjacency.
pub fn _skippable_nothing(one: &Insn) -> bool {
    _nothing(one) && one.defines.is_empty() && one.uses.is_empty()
}

fn _zero_tested(one: &Insn) -> Option<Reg> {
    if !one.clobbers.is_empty() || one.symbol == Some(true) {
        return None;
    }
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        (Operation::Compare, Some("cmp"), [Loc::Reg(register), Loc::Imm(Imm { value: 0, address: None, .. })]) => {
            Some(*register)
        }
        (Operation::Compare, Some("test"), [Loc::Reg(register), Loc::Reg(other)]) if other == register => {
            Some(*register)
        }
        _ => None,
    }
}

fn _sets_from(one: &Insn, register: &Reg) -> bool {
    if !one.clobbers.is_empty() {
        return false;
    }
    let Some(what) = &one.what else {
        return false;
    };
    match (what.op, what.name.as_deref(), what.dests.as_slice()) {
        (Operation::Binary, Some("add" | "sub" | "and" | "or" | "xor"), [Loc::Reg(dest)]) => dest == register,
        (Operation::Unary, Some("inc" | "dec" | "neg"), [Loc::Reg(dest)]) => dest == register,
        _ => false,
    }
}

/// Which flag lanes something may read after each block's last instruction.
pub fn _flags_live_out(body: &LirBody) -> HashMap<i64, Lanes> {
    let every = _flag_lanes(_ARITHMETIC);
    // No calling convention passes the adjust flag in or out: a callee, a
    // caller after a return, and whatever runs after the body leaves may read
    // any other flag, but only an instruction here that reads AF reads it.
    let exits: Lanes = every.minus(&_ADJUST);

    let effects = |one: &Insn| -> (Lanes, Lanes) {
        if let Some(what) = &one.what {
            if what.op == Operation::Branch {
                return (_flag_lanes(_lookup(&_BRANCH_FLAGS, what.name.as_deref()).unwrap_or(_ARITHMETIC)), Lanes::new());
            }
        }
        if one.what.as_ref().is_some_and(|what| what.op == Operation::Jump) || _nothing(one) {
            return (Lanes::new(), Lanes::new());
        }
        if one.what.as_ref().is_some_and(|what| [Operation::Call, Operation::Return].contains(&what.op)) {
            return (exits.clone(), Lanes::new());
        }
        let Some((reads, writes)) = _register_effects(one, false, true) else {
            // Bytes this cannot encode -- a relocated operand, an x87 form --
            // still name their instruction, and only a few instructions read
            // a flag. Writes stay unknown, which only keeps flags live longer.
            if let Some(name) = one.what.as_ref().and_then(|what| what.name.as_deref()) {
                if !name.is_empty() && !_reads_flags(name) {
                    return (Lanes::new(), Lanes::new());
                }
            }
            return (every.clone(), Lanes::new());
        };
        (
            reads.into_iter().filter(|lane| lane.0 == Register::None).collect(),
            writes.into_iter().filter(|lane| lane.0 == Register::None).collect(),
        )
    };

    let steps: HashMap<i64, Vec<(Lanes, Lanes)>> = body
        .blocks
        .iter()
        .map(|block| (block.at, block.insns.iter().map(|one| effects(one)).collect()))
        .collect();
    let mut live_in: HashMap<i64, Lanes> = body.blocks.iter().map(|block| (block.at, Lanes::new())).collect();
    let mut out: HashMap<i64, Lanes> = HashMap::default();
    let mut changed = true;
    while changed {
        changed = false;
        for block in body.blocks.iter().rev() {
            let after: Lanes = if block.succ.is_empty() {
                exits.clone()
            } else {
                block.succ.iter().flat_map(|at| live_in.get(at).unwrap_or(&exits).iter().copied()).collect()
            };
            out.insert(block.at, after.clone());
            let mut live = after;
            for (reads, writes) in steps[&block.at].iter().rev() {
                live = live.minus(writes).or(reads);
            }
            if live != live_in[&block.at] {
                live_in.insert(block.at, live);
                changed = true;
            }
        }
    }
    out
}

/// Use XOR for zero only when later integer work replaces every arithmetic flag.
pub fn zeroes(body: &LirBody) -> LirBody {
    let live = _flags_live_out(body);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        // What the block's successors read, not a guess: `mov bx,0; jmp` to a
        // block that sets its own flags first zeroes with xor too.
        let mut flags_dead = live[&block.at].is_disjoint(&_ARITHMETIC_LANES);
        let mut insns = Vec::new();
        for one in block.insns.iter().rev() {
            let mut one = Arc::clone(one);
            let previous = _flags_before(&one, flags_dead);
            if let Some(what) = &one.what {
                if one.clobbers.is_empty() {
                    if let (
                        Operation::Move,
                        Some("mov"),
                        [Loc::Reg(dest)],
                        [Loc::Imm(Imm { value: 0, width, address: None })],
                    ) = (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
                    {
                        if flags_dead
                            && dest.width == *width
                            && [2, 4].contains(width)
                            && target::WIDTHS.contains_key(&dest.register)
                            && one.symbol != Some(true)
                        {
                            let dest = *dest;
                            one = Arc::new(with_what(
                                &one,
                                semantics(
                                    Operation::Binary,
                                    "xor",
                                    vec![Loc::Reg(dest)],
                                    vec![Loc::Reg(dest), Loc::Reg(dest)],
                                ),
                            ));
                        }
                    }
                }
            }
            flags_dead = previous;
            insns.push(one);
        }
        insns.reverse();
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

const _WAITING: [&str; 36] = [
    "wait", "fwait", "fld", "fild", "fst", "fstp", "fist", "fistp", "fadd", "faddp", "fiadd", "fsub", "fsubp", "fsubr",
    "fsubrp", "fisub", "fisubr", "fmul", "fmulp", "fimul", "fdiv", "fdivp", "fdivr", "fdivrp", "fidiv", "fidivr",
    "fchs", "fabs", "fsqrt", "fxch", "fcom", "fcomp", "fcompp", "fucom", "fucomp", "fucompp",
];

/// An immediately following waiting instruction already checks pending FP exceptions.
///
/// Intel SDM Vol. 1 section 8.3.12. Never cross integer work, an unknown
/// instruction, a non-waiting control instruction, or a block boundary.
pub fn waits(body: &LirBody) -> LirBody {
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut following: Option<&Semantics> = None;
        let mut redundant: HashSet<usize> = HashSet::default();
        for one in block.insns.iter().rev() {
            let what = one.what.as_ref();
            if what.is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty)) {
                continue;
            }
            if what.is_some_and(|what| matches!(what.name.as_deref(), Some("wait" | "fwait")))
                && following.is_some_and(|following| {
                    following.name.as_deref().is_some_and(|name| _WAITING.contains(&name))
                })
            {
                redundant.insert(id(one));
            } else {
                following = what;
            }
        }
        let insns = lir::without(&block.insns, |one| redundant.contains(&id(one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>);
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

/// What `constants` believes a register holds: a literal, or a fresh
/// `object()` standing for another register's unknown contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Known {
    Value(i64),
    Object(usize),
}

/// Reuse identical scalar register contents until an instruction overwrites them.
pub fn constants(body: &LirBody) -> LirBody {
    let mut objects = 0usize;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut held: IndexMap<Reg, Known> = IndexMap::default();
        let mut redundant: HashSet<usize> = HashSet::default();
        for one in &block.insns {
            let what = one.what.as_ref();
            if what.is_some_and(|what| {
                what.op == Operation::Nothing
                    && what.name.as_deref().is_none_or(str::is_empty)
                    && what.dests.is_empty()
                    && what.sources.is_empty()
                    && what.target.is_none()
            }) && one.clobbers.is_empty()
            {
                continue;
            }
            let moved = what.is_some_and(|what| what.op == Operation::Move && what.name.as_deref() == Some("mov"));
            let extend = what.is_some_and(|what| {
                what.op == Operation::Extend
                    && matches!(what.name.as_deref(), Some("cwd" | "cdq" | "movsx"))
                    && what.dests.len() == 1
                    && what.sources.len() == 1
                    && what.dests.iter().chain(&what.sources).all(|arg| matches!(arg, Loc::Reg(_)))
            });
            if !moved && !extend {
                match _register_effects(one, true, false) {
                    None => held.clear(),
                    Some((_reads, writes)) => held.retain(|dest, _| _lanes(dest.register).is_disjoint(&writes)),
                }
                continue;
            }
            let what = what.expect("a move or extension has semantics");
            let mut candidate = None;
            if moved && what.dests.len() == 1 && what.sources.len() == 1 {
                let (dest, source) = (&what.dests[0], &what.sources[0]);
                if let Loc::Reg(dest) = dest {
                    let source_width = match source {
                        Loc::Reg(source) => Some(source.width),
                        Loc::Imm(source) => Some(source.width),
                        _ => None,
                    };
                    if target::WIDTHS.contains_key(&dest.register) && source_width == Some(dest.width) {
                        match source {
                            Loc::Imm(source) if source.address.is_none() => {
                                candidate =
                                    Some((*dest, Known::Value(source.value & ((1i64 << (dest.width * 8)) - 1))));
                            }
                            Loc::Reg(source) if target::WIDTHS.contains_key(&source.register) => {
                                let value = *held.entry(*source).or_insert_with(|| {
                                    objects += 1;
                                    Known::Object(objects)
                                });
                                candidate = Some((*dest, value));
                            }
                            _ => {}
                        }
                    }
                }
            }
            if let Some((dest, value)) = &candidate {
                if one.clobbers.is_empty() && held.get(dest) == Some(value) {
                    redundant.insert(id(one));
                    continue;
                }
            }
            let mut written: HashSet<Register> =
                one.clobbers.iter().chain(&one.clobbers_high).map(|register| full32(*register)).collect();
            written.extend(what.dests.iter().filter_map(|dest| match dest {
                Loc::Reg(dest) => Some(full32(dest.register)),
                _ => None,
            }));
            held.retain(|dest, _| !written.contains(&full32(dest.register)));
            if let Some((dest, value)) = candidate {
                if one.clobbers.is_empty() {
                    held.insert(dest, value);
                }
            }
        }
        let insns = block
            .insns
            .iter()
            .map(|one| if redundant.contains(&id(one)) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
            .collect();
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

#[cfg(test)]
#[path = "peephole_tests.rs"]
mod tests;
