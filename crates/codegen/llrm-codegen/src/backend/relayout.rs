//! Lays a function's frame out a second time, with a hole for the spill slots,
//! by moving each frame cell of the first layout.
//!
//! # How it works
//! A frame operand past `[bp-128]` needs a four-byte displacement. When a
//! function has a spill area and such operands, a layout with a hole of
//! `spilled` bytes above the allocas, for the spill slots to fill, may put them
//! nearer. That layout is made from the first one, without running the backend
//! again, because every frame cell names the slot it lies in
//! (`Addr::slot_home`).
//!
//! `replayed` makes the new `Frame`: its floor lowered by the hole, the extents
//! of the slots instruction selection laid out moved, and the spill slots
//! handed out again by `Frame::slot` in the order they were first made, which
//! gives each old slot a new first byte. `Moves` then rewrites each cell by its
//! slot:
//!
//! ```text
//!  slot of the cell                    new displacement
//!  incoming argument (home 0)          unchanged
//!  laid out by isel (an extent)        disp - hole
//!  spill slot (home h -> h')           disp - h + h'
//! ```
//!
//! The same move is applied to the operands of every instruction, the
//! call-private ranges of calls (the complement of what some pointer reaches;
//! an incoming argument's range from BP up stays), the debug homes and
//! variables, and the displacement bytes of inline code (`inline_places`).
//! `Machined::reserve` is recomputed from the new frame. Pricing and
//! encodability decisions treat a frame cell as near whatever its slot, so the
//! two layouts decide alike (a frame cell priced by its displacement once made
//! them differ).
//!
//! `ordered` is the other relayout: the pin's
//! `X86FrameLowering::orderFrameObjects` (the use-density order
//! `PrologEpilogInserter` asks for at every optimisation level), applied to the
//! first layout when it has far operands and the order reaches fewer of them.
//! Objects sort by uses per byte, highest first, ties by alignment and then by
//! later creation, and are laid from BP downward; a BASIC frame reorders the
//! selector's slots only, since the runtime zero-fills them and the spill slots
//! lie outside what it fills.
//!
//! It gives up and returns `None`, and the first layout stands, on a frame cell
//! in no known slot, on inline-code bytes it cannot move, on call-private
//! ranges that are not what `outside` makes, on a native frame, or on a frame
//! that already has a hole.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   first["<b>first body</b><br/><small>Frame#123;hole = 0#125;</small>"]:::mid
//!   second["<b>second body</b>"]:::mid
//!   keep["<b>keep the one with fewer</b><br/><small>far operands (far_frame)</small>"]:::mid
//!   first -->|"far_frame(first) #gt; 0<br/>and spilled #gt; 0<br/>laid_again(first, frame,<br/>spilled)"| second
//!   second --> keep
//! ```
//!
//! *Laying the frame out a second time*
//!
//! # Example
//! None: the unit tests of this file check the pricing of a frame cell and the
//! move of a call-private range, not a relaid function, and no test prints one.
//! Read `Moves` for the displacement rule in the table above.
//!
//! # Runs
//! In `machined_once` (`assemble.rs`), after the first layout is chosen, unless
//! the target is basic, the frame is native, nothing is spilled, or no frame
//! operand is far. `LLRM_CHECK_FRAME=1` also runs the backend again with the
//! hole and panics if the result differs (`difference`) from the relaid
//! function, or if it could not be relaid.
//!
//! # See also
//! - Design:
//!   <https://github.com/ali-mosavian/llrm/blob/18265caabe0332942d4052f0ea2f5fe67320d0ec/docs/optimizations/second-frame.md>

use std::sync::Arc;

use crate::backend::assemble::Machined;
use crate::backend::frame::Frame;
use crate::model::ir::{Addr, AddressRef, Loc, Mem, Semantics, Space};
use crate::model::lir::{CallMemory, DebugPlace, DebugVariable, Insn, LirBlock, LirBody, WHOLE_FRAME, outside};
use crate::support::hash::IndexMap;

/// `first`, which was laid out under `frame` (no hole), laid out with `hole`
/// bytes above the allocas for spill slots; and its frame. None where it cannot
/// be moved by its slots alone.
pub fn laid_again(
    first: &Machined,
    frame: &Frame,
    hole: i64,
) -> Option<(Machined, Frame)> {
    if frame.native.is_some() || frame.hole != 0 {
        return None;
    }
    let (mut again, homes) = replayed(frame, hole)?;
    again.call_area = frame.call_area;
    let moved = Moves { frame: Some(frame), homes: &homes, hole };
    let mut machined = shifted(first, frame, &moved)?;
    // The area `ReserveCallFrame` set aside lies below the locals, as in
    // `assemble.rs`.
    machined.reserve = -std::cmp::min(again.slots.values().copied().min().unwrap_or(0), again.floor) + again.call_area;
    Some((machined, again))
}

/// `first` with every cell, inline-code displacement and debug home moved by
/// `moved`; its reserve is the caller's to set.
fn shifted(
    first: &Machined,
    frame: &Frame,
    moved: &Moves,
) -> Option<Machined> {
    let body = moved_body(&first.body, moved)?;
    let mut inline = first.inline.clone();
    for (at, places) in &first.inline_places {
        let bytes = inline.get_mut(at)?;
        for (offset, disp, addend) in places {
            let new = moved.disp(frame.home_of(*disp)?, *disp)? + addend;
            bytes.get_mut(*offset..*offset + 2)?.copy_from_slice(&(new as u16).to_le_bytes());
        }
    }
    Some(Machined {
        body,
        reserve: first.reserve,
        calls: first.calls.clone(),
        inline,
        inline_symbols: first.inline_symbols.clone(),
        inline_places: first
            .inline_places
            .iter()
            .map(|(at, places)| {
                Some((
                    *at,
                    places
                        .iter()
                        .map(|(offset, disp, addend)| {
                            Some((*offset, moved.disp(frame.home_of(*disp)?, *disp)?, *addend))
                        })
                        .collect::<Option<Vec<_>>>()?,
                ))
            })
            .collect::<Option<_>>()?,
        far: first.far.clone(),
        pops: first.pops.clone(),
        popped: first.popped,
        registers: first.registers.clone(),
        landing: first.landing,
    })
}

/// One stack object for [`order`].
struct Object {
    /// Its first byte, which names it (`Addr::slot_home`).
    home: i64,
    size: i64,
    align: i64,
    /// The frame operands that name it.
    uses: u64,
}

/// `X86FrameLowering::orderFrameObjects` for a frame-pointer frame: the indices
/// of `objects` (in creation order) nearest BP first. The pin sorts ascending
/// by `uses / size` (compared cross-multiplied, a tie by alignment), stable,
/// and flips the list for an FP frame.
fn order(objects: &[Object]) -> Vec<usize> {
    let mut sorted: Vec<usize> = (0..objects.len()).collect();
    sorted.sort_by(|&a, &b| {
        let (a, b) = (&objects[a], &objects[b]);
        (a.uses * b.size as u64).cmp(&(b.uses * a.size as u64)).then(a.align.cmp(&b.align))
    });
    sorted.reverse();
    sorted
}

/// The new first byte of each object of `objects`, laid in `placed` order from
/// `top` (the distance below BP where the first begins), as
/// `calculateFrameObjectOffsets`: add the size, align, name the offset.
fn laid(
    objects: &[Object],
    placed: &[usize],
    top: i64,
) -> Vec<(i64, i64)> {
    let mut depth = top;
    placed
        .iter()
        .map(|&at| {
            let one = &objects[at];
            depth = (depth + one.size + one.align - 1).div_euclid(one.align) * one.align;
            (one.home, -depth)
        })
        .collect()
}

/// `first` laid out as the pin's `orderFrameObjects` orders its stack objects:
/// the selector's slots and, where the runtime does not own the frame, the
/// spill slots, by uses per byte. None where it cannot be moved by its slots
/// alone, or the order would not fit where the objects are.
pub fn ordered(
    first: &Machined,
    frame: &Frame,
    basic: bool,
) -> Option<(Machined, Frame)> {
    if frame.native.is_some() || frame.hole != 0 {
        return None;
    }
    let reach = frame.align.max(1);
    let align_of = |home: i64| {
        let mut align = 1;
        while align * 2 <= reach && home.rem_euclid(align * 2) == 0 {
            align *= 2;
        }
        align
    };
    let mut objects: Vec<Object> = Vec::new();
    let mut extents = frame.extents.clone();
    extents.sort_by_key(|(start, size)| std::cmp::Reverse(start + size));
    for (start, size) in extents {
        objects.push(Object { home: start, size: if size == 0 { 4 } else { size }, align: align_of(start), uses: 0 });
    }
    if !basic {
        let mut spills = std::collections::BTreeSet::new();
        for home in frame.slots.values() {
            if spills.insert(*home) {
                let size = *frame.capacities.get(home)?;
                objects.push(Object {
                    home: *home,
                    size: if size == 0 { 4 } else { size },
                    align: align_of(*home),
                    uses: 0,
                });
            }
        }
    }
    if objects.len() < 2 {
        return None;
    }
    let index: IndexMap<i64, usize> = objects.iter().enumerate().map(|(at, one)| (one.home, at)).collect();
    for one in first.body.insns() {
        let Some(what) = &one.what else { continue };
        for place in what.dests.iter().chain(&what.sources) {
            let Some(addr) = place.address().and_then(|at| at.addr) else { continue };
            if addr.space == Space::Frame
                && let Some(at) = addr.slot_home().and_then(|home| index.get(&home))
            {
                objects[*at].uses += 1;
            }
        }
    }
    let top = -objects.iter().map(|one| one.home + one.size).max()?;
    let old_low = objects.iter().map(|one| one.home).min()?;
    let placed = order(&objects);
    let mut homes: IndexMap<i64, i64> = laid(&objects, &placed, top.max(0)).into_iter().collect();
    if homes.values().copied().min()? < old_low || homes.iter().all(|(old, new)| old == new) {
        return None;
    }
    // spill slots the order leaves where they are
    for home in frame.slots.values() {
        homes.entry(*home).or_insert(*home);
    }
    let mut again = frame.clone();
    let moved = |home: &i64| homes.get(home).copied().unwrap_or(*home);
    again.extents = frame.extents.iter().map(|(start, size)| (moved(start), *size)).collect();
    again.slots = frame.slots.iter().map(|(key, home)| (key.clone(), moved(home))).collect();
    again.capacities = frame.capacities.iter().map(|(home, size)| (moved(home), *size)).collect();
    again.stacks = frame.stacks.iter().map(|(home, ranges)| (moved(home), ranges.clone())).collect();
    let moves = Moves { frame: Some(frame), homes: &homes, hole: 0 };
    let mut machined = shifted(first, frame, &moves)?;
    machined.reserve = first.reserve;
    if llrm_support::env_flag!("LLRM_CHECK_SLOTS")
        && let Some(cell) = reaching_past_its_slot(&machined.body, &again)
    {
        panic!("the frame laid again has a cell reaching past its spill slot: {cell:?}");
    }
    Some((machined, again))
}

/// A frame cell of `body` longer than the spill slot it is tagged with in
/// `frame`: it reads or writes the bytes of the next slot.
fn reaching_past_its_slot(
    body: &LirBody,
    frame: &Frame,
) -> Option<Mem> {
    body.blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter_map(|one| one.what.as_ref())
        .flat_map(|what| what.dests.iter().chain(&what.sources))
        .filter_map(|place| match place {
            Loc::Mem(cell) => Some(cell),
            _ => None,
        })
        .find(|cell| {
            cell.addr.is_some_and(|addr| {
                addr.space == Space::Frame
                    && addr.slot_home().is_some_and(|home| {
                        frame
                            .capacities
                            .get(&home)
                            .is_some_and(|capacity| home != 0 && addr.disp - home + i64::from(cell.width) > *capacity)
                    })
            })
        })
        .cloned()
}

/// `body` with every cell moved by `moved`.
fn moved_body(
    body: &LirBody,
    moved: &Moves,
) -> Option<LirBody> {
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for block in &body.blocks {
        let mut insns = Vec::with_capacity(block.insns.len());
        for one in &block.insns {
            insns.push(moved.insn(one)?);
        }
        blocks.push(LirBlock { insns: insns.into(), ..block.clone().into_inner() }.into());
    }
    let mut out = body.with_blocks(blocks);
    out.homes = Arc::new(
        body.homes
            .iter()
            .map(|(value, cell)| Some((*value, moved.mem(cell)?)))
            .collect::<Option<std::collections::BTreeMap<_, _>>>()?,
    );
    out.variables = body.variables.iter().map(|one| moved.variable(one)).collect::<Option<Vec<_>>>()?;
    Some(out)
}

/// `body` with every cell below BP `bytes` lower, as a frame whose callee-saved
/// registers are pushed first lays it out; None where a cell names no slot.
pub fn pushed_down(
    body: &LirBody,
    bytes: i64,
) -> Option<LirBody> {
    let homes = IndexMap::default();
    moved_body(body, &Moves { frame: None, homes: &homes, hole: bytes })
}

/// The frame `frame` would be with `hole`: the same floor lowered by the hole,
/// the selector's slots moved, and the spill slots handed out again in the
/// order they were first made.
fn replayed(
    frame: &Frame,
    hole: i64,
) -> Option<(Frame, IndexMap<i64, i64>)> {
    let mut again = Frame::new(frame.floor - hole);
    again.align = frame.align;
    again.hole = hole;
    again.extents = frame.extents.iter().map(|(start, size)| (start - hole, *size)).collect();
    let mut homes: IndexMap<i64, i64> = IndexMap::default();
    for (key, home) in &frame.slots {
        // A rounding temporary is laid with the allocas, below them: it moves
        // by the hole as they do, and is no filler of it.
        if matches!(
            key,
            crate::backend::frame::SlotKey::Named(name, _) if name == "temporary"
        ) {
            let moved = home - hole;
            again.slots.insert(key.clone(), moved);
            again.capacities.insert(moved, *frame.capacities.get(home)?);
            again.scratch.insert(moved);
            homes.insert(*home, moved);
            continue;
        }
        match homes.get(home) {
            Some(found) => {
                again.slots.insert(key.clone(), *found);
            }
            None => {
                let capacity = *frame.capacities.get(home)?;
                let found = again.slot(key.clone(), capacity).ok()?;
                homes.insert(*home, found);
            }
        }
    }
    // A capacity no key owns (a discarded trial's) has no cell.
    if frame.capacities.keys().any(|home| !homes.contains_key(home)) {
        return None;
    }
    Some((again, homes))
}

struct Moves<'a> {
    /// None: every cell below BP moves by `hole` (`pushed_down`).
    frame: Option<&'a Frame>,
    /// Each spill slot's first byte in the old layout and in the new.
    homes: &'a IndexMap<i64, i64>,
    hole: i64,
}

impl Moves<'_> {
    /// The first byte of the slot `home` in the new layout: what a cell moved
    /// out of it is tagged with.
    fn home(
        &self,
        home: i64,
    ) -> Option<i64> {
        if home == 0 {
            return Some(0);
        }
        let Some(frame) = self.frame else { return Some(home - self.hole) };
        if let Some(found) = self.homes.get(&home) {
            return Some(*found);
        }
        frame.extents.iter().any(|(start, _)| *start == home).then_some(home - self.hole)
    }

    /// Where the byte at `disp` of a cell tagged with slot `home` is in the new
    /// layout.
    fn disp(
        &self,
        home: i64,
        disp: i64,
    ) -> Option<i64> {
        if home == 0 {
            return Some(disp);
        }
        let Some(frame) = self.frame else { return Some(disp - self.hole) };
        // A spill slot: where the replay put the slot, and the cell's place in
        // it.
        if let Some(found) = self.homes.get(&home) {
            return Some(disp - home + found);
        }
        if frame.extents.iter().any(|(start, _)| *start == home) {
            return Some(disp - self.hole);
        }
        None
    }

    fn addr(
        &self,
        addr: Addr,
        offset: i64,
    ) -> Option<(Addr, i64)> {
        let (new, home) = match (addr.space, addr.slot_home()) {
            (Space::Frame, Some(home)) => (self.disp(home, addr.disp)?, self.home(home)?),
            (Space::Frame, None) => return None,
            _ => return Some((addr, offset)),
        };
        // `offset` repeats the displacement on a cell the spiller made from a
        // folded frame address.
        let offset = if addr.space == Space::Frame && addr.disp != 0 && offset == addr.disp { new } else { offset };
        Some((Addr { disp: new, ..addr }.in_slot(home), offset))
    }

    fn mem(
        &self,
        cell: &Mem,
    ) -> Option<Mem> {
        let Some(addr) = cell.addr else { return Some(cell.clone()) };
        // The replay hands each spill slot out alone, so a cell reaching past
        // its slot (a `les` over the two word slots of a far pointer) would
        // read whatever lies next to it in the new layout.
        if let (Some(frame), Space::Frame, Some(home)) = (self.frame, addr.space, addr.slot_home())
            && let Some(capacity) = frame.capacities.get(&home).filter(|_| self.homes.contains_key(&home))
            && addr.disp - home + i64::from(cell.width) > *capacity
        {
            if llrm_support::env_flag!("LLRM_COUNT_RELAYOUT_SKIPS") {
                eprintln!("RELAYOUT-SKIP");
            }
            return None;
        }
        let (addr, offset) = self.addr(addr, cell.offset)?;
        Some(Mem { addr: Some(addr), offset, ..cell.clone() })
    }

    fn address(
        &self,
        cell: &AddressRef,
    ) -> Option<AddressRef> {
        let Some(addr) = cell.addr else { return Some(cell.clone()) };
        let (addr, offset) = self.addr(addr, cell.offset)?;
        Some(AddressRef { addr: Some(addr), offset, ..cell.clone() })
    }

    fn loc(
        &self,
        place: &Loc,
    ) -> Option<Loc> {
        Some(match place {
            Loc::Mem(cell) => Loc::Mem(self.mem(cell)?),
            Loc::Address(cell) => Loc::Address(self.address(cell)?),
            other => other.clone(),
        })
    }

    fn insn(
        &self,
        one: &Arc<Insn>,
    ) -> Option<Arc<Insn>> {
        // Most instructions name no cell that moves: they are shared as they
        // are, without a copy of their semantics.
        let what = match &one.what {
            Some(what) => {
                let dests: Vec<Loc> = what.dests.iter().map(|place| self.loc(place)).collect::<Option<_>>()?;
                let sources: Vec<Loc> = what.sources.iter().map(|place| self.loc(place)).collect::<Option<_>>()?;
                (dests != what.dests || sources != what.sources).then(|| Semantics { dests, sources, ..what.clone() })
            }
            None => None,
        };
        let call = match &one.call {
            Some(call) => {
                let private = self.private(&call.private)?;
                (private != call.private).then(|| Arc::new(CallMemory { private, ..(**call).clone() }))
            }
            None => None,
        };
        if what.is_none() && call.is_none() {
            return Some(Arc::clone(one));
        }
        Some(Arc::new(Insn {
            what: what.or_else(|| one.what.clone()),
            call: call.or_else(|| one.call.clone()),
            ..(**one).clone()
        }))
    }

    /// The call-private ranges when the selector's slots are `hole` lower: the
    /// complement of the bytes some pointer reaches.
    fn private(
        &self,
        private: &[(Addr, u32)],
    ) -> Option<Vec<(Addr, u32)>> {
        // A call that may reach the whole frame has no private range to move.
        if private.is_empty() {
            return Some(Vec::new());
        }
        let (whole, size) = WHOLE_FRAME;
        let (low, high) = (whole.disp, whole.disp + i64::from(size));
        let mut reach = std::collections::BTreeSet::new();
        let mut at = low;
        for (start, length) in private {
            if start.space != Space::Frame || start.disp < at {
                return None;
            }
            if start.disp > at {
                reach.insert((at, start.disp));
            }
            at = start.disp + i64::from(*length);
        }
        if at < high {
            reach.insert((at, high));
        }
        if outside(&reach) != private {
            return None;
        }
        if reach.is_empty() {
            return Some(private.to_vec());
        }
        // A range reaching an end of the frame is no slot of the selector's.
        if reach.iter().any(|(start, end)| *start <= low || *end >= high) {
            return None;
        }
        // An incoming argument's range (from BP up) stays; what the selector
        // laid out below BP moves, each slot to where it went.
        let mut moved = std::collections::BTreeSet::new();
        for (start, end) in &reach {
            if *start >= 0 {
                moved.insert((*start, *end));
                continue;
            }
            let Some(frame) =
                self.frame.filter(|frame| frame.extents.iter().any(|(at, _)| self.homes.contains_key(at)))
            else {
                moved.insert((start - self.hole, end - self.hole));
                continue;
            };
            let mut covered = 0;
            for (at, size) in &frame.extents {
                let (low, high) = ((*start).max(*at), (*end).min(at + size));
                if low < high {
                    let to = self.homes.get(at).copied().unwrap_or(*at);
                    moved.insert((to + (low - at), to + (high - at)));
                    covered += high - low;
                }
            }
            // a range outside the selector's slots is no slot to move
            if covered != end - start {
                return None;
            }
        }
        Some(outside(&moved))
    }

    fn variable(
        &self,
        one: &DebugVariable,
    ) -> Option<DebugVariable> {
        Some(match &one.place {
            DebugPlace::At(addr) if addr.space == Space::Frame => match addr.slot_home() {
                Some(home) => DebugVariable {
                    place: DebugPlace::At(
                        Addr { disp: self.disp(home, addr.disp)?, ..*addr }.in_slot(self.home(home)?),
                    ),
                    ..one.clone()
                },
                None => return None,
            },
            _ => one.clone(),
        })
    }
}

/// What differs between two machined functions, or None where they are the
/// same.
pub fn difference(
    left: &Machined,
    right: &Machined,
) -> Option<String> {
    if left.reserve != right.reserve {
        return Some(format!("reserve {} against {}", left.reserve, right.reserve));
    }
    if left.inline_places != right.inline_places {
        return Some(format!("inline places {:?} against {:?}", left.inline_places, right.inline_places));
    }
    if left.calls != right.calls
        || left.inline != right.inline
        || left.inline_symbols != right.inline_symbols
        || left.far != right.far
        || left.pops != right.pops
        || left.popped != right.popped
        || left.landing != right.landing
    {
        return Some("calls, inline code, far calls, pops or landing".to_owned());
    }
    if left.body.variables != right.body.variables {
        return Some(format!("variables {:?} against {:?}", left.body.variables, right.body.variables));
    }
    if *left.body.homes != *right.body.homes {
        return Some("homes".to_owned());
    }
    let (a, b) = (left.body.insns(), right.body.insns());
    if a.len() != b.len() {
        return Some(format!("{} instructions against {}", a.len(), b.len()));
    }
    for (one, other) in a.iter().zip(&b) {
        if one != other {
            return Some(format!("at {:#06x}:\n  {one:?}\n  {other:?}", one.at));
        }
    }
    if left.body.blocks.iter().map(|block| (block.at, &block.succ)).ne(right
        .body
        .blocks
        .iter()
        .map(|block| (block.at, &block.succ)))
    {
        return Some("blocks".to_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use llrm_lir::registers::RegId;

    use crate::backend::select::{emit_in, priced_in};
    use crate::model::ir::{Addr, Loc, Mem, Operation, Reg, Semantics, Space};

    fn load(addr: Addr) -> Semantics {
        let cell = Mem { through: RegId::EBP, ..Mem::new(Some(addr), 4) };
        Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Reg(Reg { register: RegId::EAX, width: 4 })],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::Move)
        }
    }

    /// A frame cell was priced by the displacement its slot happened to have
    /// under the first layout, so the second layout (which puts the spill
    /// slots near) decided differently, and the second backend run could not be
    /// replaced by moving the cells: sieve at -m32 -O2 allocated `add [slot],
    /// 1` in one and a reload, add and store in the other.
    #[test]
    fn test_a_frame_cell_is_priced_the_same_wherever_its_slot_is() {
        let length = |addr| priced_in(32, &load(addr), 0, None, false, false, None).expect("encodes").code.len();
        let near = Addr::new(Space::Frame, -8).in_slot(-8);
        let far = Addr::new(Space::Frame, -2000).in_slot(-2000);
        assert_eq!(length(near), length(far));
        let (a, b) = (
            emit_in(32, &load(near), 0, None, false, false, None).expect("encodes"),
            emit_in(32, &load(far), 0, None, false, false, None).expect("encodes"),
        );
        assert_ne!(a.code.len(), b.code.len(), "premise: encoded, the two displacements differ");
        // An incoming argument's place is fixed: it keeps its displacement.
        let incoming = |disp| length(Addr::new(Space::Frame, disp).in_slot(0));
        assert_ne!(incoming(8), incoming(2000));
    }

    /// A byval argument's incoming range is escaped like a local, but it does
    /// not move with the hole.
    #[test]
    fn test_an_escaped_incoming_range_stays_where_it_is_when_the_slots_move() {
        use super::Moves;
        use crate::backend::frame::Frame;
        let frame = Frame::new(0);
        let homes = crate::support::hash::IndexMap::default();
        let moves = Moves { frame: Some(&frame), homes: &homes, hole: 6 };
        let reach = |ranges: &[(i64, i64)]| crate::model::lir::outside(&ranges.iter().copied().collect());
        assert_eq!(moves.private(&reach(&[(-40, -8), (4, 12)])), Some(reach(&[(-46, -14), (4, 12)])));
    }

    /// A call that reaches the whole frame (no private range) made the second
    /// layout give up: dag grep.nib -Os kept every spill below its 1 KiB
    /// buffer, 56 far operands against the old selector's 5.
    #[test]
    fn test_a_call_that_reaches_the_whole_frame_moves_with_the_slots() {
        use super::Moves;
        use crate::backend::frame::Frame;
        let frame = Frame::new(0);
        let homes = crate::support::hash::IndexMap::default();
        let moves = Moves { frame: Some(&frame), homes: &homes, hole: 6 };
        assert_eq!(moves.private(&[]), Some(Vec::new()));
    }

    /// The second layout moved a cell but kept its old slot's tag, so the tags
    /// of the body did not name the slots of its frame: d_faces -Os had ten
    /// cells in no slot (`frame.slots`, `extents`), which a pass that lays
    /// slots out by their uses must refuse.
    #[test]
    fn test_a_cell_moved_to_the_second_layout_is_tagged_with_the_slot_it_moved_to() {
        use super::Moves;
        use crate::backend::frame::Frame;
        let mut frame = Frame::new(0);
        frame.extents = vec![(-100, 32)];
        let homes = [(-12, -20)].into_iter().collect();
        let moves = Moves { frame: Some(&frame), homes: &homes, hole: 8 };
        let cell = |disp: i64, home: i64| Addr::new(Space::Frame, disp).in_slot(home);
        let (spilled, _) = moves.addr(cell(-10, -12), 0).expect("a spill slot moves");
        assert_eq!((spilled.disp, spilled.slot_home()), (-18, Some(-20)));
        let (laid, _) = moves.addr(cell(-90, -100), 0).expect("an extent moves");
        assert_eq!((laid.disp, laid.slot_home()), (-98, Some(-108)));
        let (incoming, _) = moves.addr(cell(8, 0), 0).expect("an incoming argument stays");
        assert_eq!((incoming.disp, incoming.slot_home()), (8, Some(0)));
    }

    /// A `les` over the two word spill slots of a far pointer was moved by its
    /// first slot alone, so the second layout read its selector from whatever
    /// lay after the offset slot: d_faces -O2 loaded ES from an unwritten
    /// slot and drew the wrong picture (oracle-qcport).
    #[test]
    fn test_a_cell_reaching_past_its_spill_slot_is_not_moved() {
        use super::Moves;
        use crate::backend::frame::Frame;
        let mut frame = Frame::new(0);
        frame.capacities.insert(-12, 2);
        let homes = [(-12, -20)].into_iter().collect();
        let moves = Moves { frame: Some(&frame), homes: &homes, hole: 8 };
        let cell =
            |width| Mem { through: RegId::EBP, ..Mem::new(Some(Addr::new(Space::Frame, -12).in_slot(-12)), width) };
        assert!(moves.mem(&cell(2)).is_some(), "premise: a word in its word slot moves");
        assert_eq!(moves.mem(&cell(4)), None);
    }

    #[global_allocator]
    static COUNTING: llrm_support::alloc_count::Counting = llrm_support::alloc_count::Counting;

    /// An instruction that names no cell that moves was copied whole (its
    /// semantics twice) before it was found equal and shared: d_faces -O2
    /// spent 1.6 M Ir on 4 K such copies. Now it costs the two operand lists.
    #[test]
    fn test_an_instruction_naming_no_moved_cell_is_shared_without_a_copy() {
        use std::sync::Arc;

        use super::Moves;
        use crate::backend::frame::Frame;
        use crate::model::lir::Insn;
        let reg = |register| Loc::Reg(Reg { register, width: 4 });
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![reg(RegId::EAX)],
            sources: vec![reg(RegId::EAX), reg(RegId::EDX)],
            ..Semantics::new(Operation::Move)
        };
        let one = Arc::new(Insn::new(0, None, Some(what), vec![], vec![]));
        let frame = Frame::new(0);
        let homes = crate::support::hash::IndexMap::default();
        let moves = Moves { frame: Some(&frame), homes: &homes, hole: 8 };
        let (moved, made) = llrm_support::alloc_count::counted(|| moves.insn(&one));
        assert!(Arc::ptr_eq(&moved.expect("nothing to refuse"), &one));
        assert!(made <= 2, "{made} allocations for an instruction that does not move");
    }

    /// `LLRM_CHECK_SLOTS` finds a dword read of a word slot, and passes one of
    /// a dword slot.
    #[test]
    fn test_the_slot_check_finds_a_cell_reaching_past_its_slot() {
        use crate::backend::frame::Frame;
        let body = reading(&[(-12, -12)]);
        let mut frame = Frame::new(0);
        frame.capacities.insert(-12, 4);
        assert_eq!(super::reaching_past_its_slot(&body, &frame), None);
        frame.capacities.insert(-12, 2);
        assert!(super::reaching_past_its_slot(&body, &frame).is_some());
    }

    /// The body of one block that reads each of `disps` once, tagged with the
    /// slot it lies in.
    fn reading(reads: &[(i64, i64)]) -> crate::model::lir::LirBody {
        use std::sync::Arc;

        use crate::model::lir::{Insn, LirBlock, LirBody};
        let insns = reads
            .iter()
            .enumerate()
            .map(|(at, (disp, home))| {
                Arc::new(Insn::new(
                    at as i64,
                    None,
                    Some(load(Addr::new(Space::Frame, *disp).in_slot(*home))),
                    vec![],
                    vec![],
                ))
            })
            .collect();
        LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], Default::default(), Default::default())
    }

    fn machined(body: crate::model::lir::LirBody) -> crate::backend::assemble::Machined {
        crate::backend::assemble::Machined {
            body,
            reserve: 140,
            calls: Default::default(),
            inline: Default::default(),
            inline_places: Default::default(),
            inline_symbols: Default::default(),
            far: Default::default(),
            pops: Default::default(),
            popped: 0,
            registers: llrm_target::FrameRegisters {
                pointer: RegId::BP,
                stack: RegId::SP,
                saved: Vec::new(),
                slot: 2,
                align: 2,
                optional: false,
                enter: false,
                wrap_split_size: false,
                slot_scratch: None,
                free: false,
            },
            landing: None,
        }
    }

    fn far_cells(body: &crate::model::lir::LirBody) -> usize {
        body.insns()
            .iter()
            .filter_map(|one| one.what.as_ref())
            .flat_map(|what| what.sources.iter().chain(&what.dests))
            .filter_map(|place| place.address().and_then(|at| at.addr))
            .filter(|addr| addr.space == Space::Frame && !(-128..128).contains(&addr.disp))
            .count()
    }

    /// Two 46-byte arrays are allocated before a word that is read six times,
    /// so it lies at `[bp-130]`, a four-byte displacement every time
    /// (bounds.bas LOCALS, qb m16 -Os: 105 far operands against the old
    /// selector's 44, +60 bytes). The pin orders the frame objects by uses
    /// per byte and puts the word at `[bp-2]`.
    #[test]
    fn test_a_word_read_often_is_laid_nearer_bp_than_the_arrays() {
        use super::ordered;
        use crate::backend::frame::Frame;
        let mut frame = Frame::new(-132);
        frame.align = 2;
        frame.extents = vec![(-46, 46), (-92, 46), (-132, 2)];
        let body =
            reading(&[(-46, -46), (-132, -132), (-132, -132), (-132, -132), (-132, -132), (-132, -132), (-132, -132)]);
        assert_eq!(far_cells(&body), 6, "the premise");
        let (done, again) = ordered(&machined(body), &frame, true).expect("the order fits");
        assert_eq!(far_cells(&done.body), 0);
        let word = again.extents.iter().find(|(_, size)| *size == 2).expect("the word");
        assert_eq!(word.0, -2);
    }

    /// bc_circle_aspect at m32 -Os printed 4 pixels for 100: `B$CIRC` reserves
    /// a 12-byte call area, took the second layout, and that layout dropped
    /// the area, so `c.cx` and `c.cy` lay where the `octet` call stores its
    /// stack arguments. The reserve must still hold the area below the
    /// locals.
    #[test]
    fn test_the_second_layout_keeps_the_reserved_call_area() {
        use super::laid_again;
        use crate::backend::frame::Frame;
        let mut frame = Frame::new(-8);
        frame.align = 4;
        frame.call_area = 12;
        let home = frame.slot(7_i64, 4).expect("a spill slot");
        let (done, again) = laid_again(&machined(reading(&[(home, home)])), &frame, 8).expect("it can be laid again");
        let lowest = again.slots.values().copied().chain([again.floor]).min().expect("a floor");
        assert_eq!(again.call_area, 12);
        assert_eq!(done.reserve, -lowest + 12, "the locals lie above the outgoing area");
    }

    /// An order that would need more room than the objects have is not taken.
    #[test]
    fn test_an_order_that_does_not_fit_leaves_the_frame_alone() {
        use super::ordered;
        use crate::backend::frame::Frame;
        let mut frame = Frame::new(-7);
        frame.align = 4;
        // laid densest first, byte, word, dword need 8 bytes with the padding;
        // the three lie in 7
        frame.extents = vec![(-4, 4), (-6, 2), (-7, 1)];
        let body = reading(&[(-7, -7), (-7, -7), (-7, -7), (-6, -6), (-6, -6), (-6, -6), (-4, -4)]);
        assert!(ordered(&machined(body), &frame, true).is_none());
    }

    /// The pin's comparator: uses per byte, highest first; a tie by alignment,
    /// then the later object.
    #[test]
    fn test_objects_are_ordered_by_uses_per_byte_then_alignment_then_creation() {
        use super::{Object, order};
        let one = |size, align, uses| Object { home: 0, size, align, uses };
        // 3 uses of 2 bytes beat 5 of 8; equal density 1: the wider alignment
        // first; then the later
        let objects = [one(8, 2, 5), one(2, 2, 3), one(4, 2, 4), one(4, 4, 4), one(4, 4, 4)];
        assert_eq!(order(&objects), [1, 4, 3, 2, 0]);
    }
}
