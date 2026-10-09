//! The frame laid out again: a function whose frame reaches past `[bp-128]` is also tried with a hole above its allocas for the
//! spill slots; this makes that layout from the first one, by the slot each frame cell names, where it used to run the whole
//! backend a second time (docs/optimizations/second-frame.md).
//!
//! A cell is moved by its slot: the incoming arguments stay, a slot instruction selection laid out moves down by the hole, a spill
//! slot goes where `Frame::slot` would hand it out in a frame that has the hole. Anything this does not know how to move (bytes
//! of inline code that name a displacement, a cell in no known slot, a set of call-private ranges that is not what `outside`
//! makes) makes it give up, and the caller runs the backend again as before. `LLRM_CHECK_FRAME=1` runs it again either way and
//! compares.

use std::sync::Arc;

use crate::backend::assemble::Machined;
use crate::backend::frame::Frame;
use crate::model::ir::{Addr, Address, Loc, Mem, Semantics, Space};
use crate::model::lir::{outside, CallMemory, DebugPlace, DebugVariable, Insn, LirBlock, WHOLE_FRAME};
use crate::support::hash::IndexMap;

/// `first`, which was laid out under `frame` (no hole), laid out with `hole` bytes above the allocas for spill slots; and its
/// frame. None where it cannot be moved by its slots alone.
pub fn laid_again(first: &Machined, frame: &Frame, hole: i64) -> Option<(Machined, Frame)> {
    if frame.native.is_some() || frame.hole != 0 {
        return None;
    }
    let (again, homes) = replayed(frame, hole)?;
    let moved = Moves { frame, homes: &homes, hole };
    let mut blocks = Vec::with_capacity(first.body.blocks.len());
    for block in &first.body.blocks {
        let mut insns = Vec::with_capacity(block.insns.len());
        for one in &block.insns {
            insns.push(moved.insn(one)?);
        }
        blocks.push(LirBlock { insns: insns.into(), ..block.clone() });
    }
    let mut body = first.body.with_blocks(blocks);
    body.homes = Arc::new(first.body.homes.iter().map(|(value, cell)| Some((*value, moved.mem(cell)?))).collect::<Option<std::collections::BTreeMap<_, _>>>()?);
    body.variables = first.body.variables.iter().map(|one| moved.variable(one)).collect::<Option<Vec<_>>>()?;
    let reserve = -std::cmp::min(again.slots.values().copied().min().unwrap_or(0), again.floor);
    let mut inline = first.inline.clone();
    for (at, places) in &first.inline_places {
        let bytes = inline.get_mut(at)?;
        for (offset, disp, addend) in places {
            let new = moved.disp(frame.home_of(*disp)?, *disp)? + addend;
            bytes.get_mut(*offset..*offset + 2)?.copy_from_slice(&(new as u16).to_le_bytes());
        }
    }
    let machined = Machined { body, reserve, calls: first.calls.clone(), inline, inline_places: first.inline_places.iter().map(|(at, places)| Some((*at, places.iter().map(|(offset, disp, addend)| Some((*offset, moved.disp(frame.home_of(*disp)?, *disp)?, *addend))).collect::<Option<Vec<_>>>()?))).collect::<Option<_>>()?, far: first.far.clone(), pops: first.pops.clone(), popped: first.popped, registers: first.registers.clone(), landing: first.landing };
    Some((machined, again))
}

/// The frame `frame` would be with `hole`: the same floor lowered by the hole, the selector's slots moved, and the spill slots
/// handed out again in the order they were first made.
fn replayed(frame: &Frame, hole: i64) -> Option<(Frame, IndexMap<i64, i64>)> {
    let mut again = Frame::new(frame.floor - hole);
    again.hole = hole;
    again.extents = frame.extents.iter().map(|(start, size)| (start - hole, *size)).collect();
    let mut homes: IndexMap<i64, i64> = IndexMap::default();
    for (key, home) in &frame.slots {
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
    frame: &'a Frame,
    /// Each spill slot's first byte in the old layout and in the new.
    homes: &'a IndexMap<i64, i64>,
    hole: i64,
}

impl Moves<'_> {
    /// Where the byte at `disp` of a cell tagged with slot `home` is in the new layout.
    fn disp(&self, home: i64, disp: i64) -> Option<i64> {
        if home == 0 {
            return Some(disp);
        }
        // A spill slot: where the replay put the slot, and the cell's place in it.
        if let Some(found) = self.homes.get(&home) {
            return Some(disp - home + found);
        }
        if self.frame.extents.iter().any(|(start, _)| *start == home) {
            return Some(disp - self.hole);
        }
        None
    }

    fn addr(&self, addr: Addr, offset: i64, in_frame: bool) -> Option<(Addr, i64)> {
        let new = match (addr.space, addr.slot_home()) {
            (Space::Frame, Some(home)) => self.disp(home, addr.disp)?,
            (Space::Frame, None) => return None,
            // An indexed array: a literal displacement through BP, the selector's.
            (Space::Literal, _) if in_frame => addr.disp - self.hole,
            _ => return Some((addr, offset)),
        };
        // `offset` repeats the displacement on a cell the spiller made from a folded frame address.
        let offset = if addr.space == Space::Frame && addr.disp != 0 && offset == addr.disp { new } else { offset };
        Some((Addr { disp: new, ..addr }, offset))
    }

    fn mem(&self, cell: &Mem) -> Option<Mem> {
        let Some(addr) = cell.addr else { return Some(cell.clone()) };
        let (addr, offset) = self.addr(addr, cell.offset, cell.in_frame())?;
        Some(Mem { addr: Some(addr), offset, ..cell.clone() })
    }

    fn address(&self, cell: &Address) -> Option<Address> {
        let Some(addr) = cell.addr else { return Some(cell.clone()) };
        let (addr, offset) = self.addr(addr, cell.offset, cell.in_frame())?;
        Some(Address { addr: Some(addr), offset, ..cell.clone() })
    }

    fn loc(&self, place: &Loc) -> Option<Loc> {
        Some(match place {
            Loc::Mem(cell) => Loc::Mem(self.mem(cell)?),
            Loc::Address(cell) => Loc::Address(self.address(cell)?),
            other => other.clone(),
        })
    }

    fn insn(&self, one: &Arc<Insn>) -> Option<Arc<Insn>> {
        let what = match &one.what {
            Some(what) => Some(Semantics { dests: what.dests.iter().map(|place| self.loc(place)).collect::<Option<_>>()?, sources: what.sources.iter().map(|place| self.loc(place)).collect::<Option<_>>()?, ..what.clone() }),
            None => None,
        };
        let call = match &one.call {
            Some(call) => Some(Arc::new(CallMemory { private: self.private(&call.private)?, ..(**call).clone() })),
            None => None,
        };
        if what == one.what && call == one.call {
            return Some(Arc::clone(one));
        }
        Some(Arc::new(Insn { what, call, ..(**one).clone() }))
    }

    /// The call-private ranges when the selector's slots are `hole` lower: the complement of the bytes some pointer reaches.
    fn private(&self, private: &[(Addr, u32)]) -> Option<Vec<(Addr, u32)>> {
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
        // An incoming argument's range (from BP up) stays; what the selector laid out below BP moves.
        Some(outside(&reach.iter().map(|(start, end)| if *start >= 0 { (*start, *end) } else { (start - self.hole, end - self.hole) }).collect()))
    }

    fn variable(&self, one: &DebugVariable) -> Option<DebugVariable> {
        Some(match &one.place {
            DebugPlace::At(addr) if addr.space == Space::Frame => match addr.slot_home() {
                Some(home) => DebugVariable { place: DebugPlace::At(Addr { disp: self.disp(home, addr.disp)?, ..*addr }), ..one.clone() },
                None => return None,
            },
            _ => one.clone(),
        })
    }
}

/// What differs between two machined functions, or None where they are the same.
pub fn difference(left: &Machined, right: &Machined) -> Option<String> {
    if left.reserve != right.reserve {
        return Some(format!("reserve {} against {}", left.reserve, right.reserve));
    }
    if left.inline_places != right.inline_places {
        return Some(format!("inline places {:?} against {:?}", left.inline_places, right.inline_places));
    }
    if left.calls != right.calls || left.inline != right.inline || left.far != right.far || left.pops != right.pops || left.popped != right.popped || left.landing != right.landing {
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
    if left.body.blocks.iter().map(|block| (block.at, &block.succ)).ne(right.body.blocks.iter().map(|block| (block.at, &block.succ))) {
        return Some("blocks".to_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::backend::select::{emit_in, priced_in};
    use crate::model::ir::{Addr, Loc, Mem, Operation, Reg, Semantics, Space};
    use iced_x86::Register;

    fn load(addr: Addr) -> Semantics {
        let cell = Mem { through: Register::EBP, ..Mem::new(Some(addr), 4) };
        Semantics { name: Some("mov".to_owned()), dests: vec![Loc::Reg(Reg { register: Register::EAX, width: 4 })], sources: vec![Loc::Mem(cell)], ..Semantics::new(Operation::Move) }
    }

    /// A frame cell was priced by the displacement its slot happened to have under the first layout, so the second layout (which puts
    /// the spill slots near) decided differently, and the second backend run could not be replaced by moving the cells: sieve at -m32
    /// -O2 allocated `add [slot], 1` in one and a reload, add and store in the other.
    #[test]
    fn test_a_frame_cell_is_priced_the_same_wherever_its_slot_is() {
        let length = |addr| priced_in(32, &load(addr), 0, None, false, false, None).expect("encodes").code.len();
        let near = Addr::new(Space::Frame, -8).in_slot(-8);
        let far = Addr::new(Space::Frame, -2000).in_slot(-2000);
        assert_eq!(length(near), length(far));
        let (a, b) = (emit_in(32, &load(near), 0, None, false, false, None).expect("encodes"), emit_in(32, &load(far), 0, None, false, false, None).expect("encodes"));
        assert_ne!(a.code.len(), b.code.len(), "premise: encoded, the two displacements differ");
        // An incoming argument's place is fixed: it keeps its displacement.
        let incoming = |disp| length(Addr::new(Space::Frame, disp).in_slot(0));
        assert_ne!(incoming(8), incoming(2000));
    }

    /// A byval argument's incoming range is escaped like a local, but it does not move with the hole.
    #[test]
    fn test_an_escaped_incoming_range_stays_where_it_is_when_the_slots_move() {
        use super::Moves;
        use crate::backend::frame::Frame;
        let frame = Frame::new(0);
        let homes = crate::support::hash::IndexMap::default();
        let moves = Moves { frame: &frame, homes: &homes, hole: 6 };
        let reach = |ranges: &[(i64, i64)]| crate::model::lir::outside(&ranges.iter().copied().collect());
        assert_eq!(moves.private(&reach(&[(-40, -8), (4, 12)])), Some(reach(&[(-46, -14), (4, 12)])));
    }
}
