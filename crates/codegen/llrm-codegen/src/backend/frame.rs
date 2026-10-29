//! The stack frame of one function body: where each spill slot, temporary and
//! selector-made stack object lives, and how many bytes the prologue reserves.
//!
//! # How it works
//! Displacements are bytes from the frame register, negative below it. A
//! `Frame` has a `floor`, the deepest displacement already reached, and hands
//! out slots below it:
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','textColor':'#64748b'}}}%%
//! block-beta
//!   columns 8
//!   o1["bp+n"] c1["incoming arguments"]:3 s1[" "] n1[" "]:3
//!   o2["bp+0"] c2["caller's bp"]:3 s2[" "] n2[" "]:3
//!   o3[" "] c3["hole"]:3 s3[" "] n3["spill slots fill this first#59; ends at bp-hole"]:3
//!   o4[" "] c4["selector slots"]:3 s4[" "] n4["allocas, homed arguments (extents)"]:3
//!   o5[" "] c5["runtime frame"]:3 s5[" "] n5["BASIC: fixed size, from the runtime entry call"]:3
//!   o6[" "] c6["spill slots"]:3 s6[" "] n6["one per value, at least a word (slot)"]:3
//!   classDef off fill:transparent,stroke:transparent,stroke-width:2px,color:#64748b
//!   classDef cell fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff
//!   class o1,n1,s1,o2,n2,s2,o3,n3,s3,o4,n4,s4,o5,n5,s5,o6,n6,s6 off
//!   class c1,c2,c3,c4,c5,c6 cell
//! ```
//!
//! *Frame layout, incoming arguments at the top*
//!
//! A spill slot is aligned as its width is, up to `align`: the target's stack
//! slot (4 in flat code, a word in real mode), or its stack alignment (8) for a
//! function that needs the stack aligned (`calleefacts`: one holding a double
//! or calling one that does), as gcc's `assign_stack_local` and LLVM's
//! `calculateFrameObjectOffsets` do. A dword or a quad off its alignment costs
//! a 486 3 clocks (Intel486 data sheet 240440-002, section 10.1, Clock Count
//! Assumptions 2; Table 2.1 gives the alignments). The frame register is
//! 8-aligned where the stack is at the call (`masm::parts` rounds the frame,
//! `isel` pads a call's arguments), so a displacement that is a multiple of 8
//! is an aligned address.
//!
//! `of` builds the frame after selection. It scans the body for the lowest
//! frame cell referenced (outgoing native stack arguments excepted) and, for a
//! BASIC procedure that enters through the runtime's frame call, for the
//! runtime frame size: the constant in `cx` at that call plus the runtime's own
//! 10 bytes (20 for VB/DOS). A size that is not a known constant, or a native
//! frame referenced below its reservation, is `Refused`.
//!
//! `slot` gives a value a slot on first request, keyed by value or by a name;
//! it fills the hole just below the frame register first, then the space under
//! the lowest slot. A slot belongs to its value: sharing is done by the callers
//! that colour slots (`spiller`, isel's alloca groups), not here. `cell`
//! returns the slot as a frame memory operand. `saved` and `restore` roll back
//! the slots of a trial that is discarded.
//!
//! `tagged` marks every frame cell in the body with the slot it is in (nearest
//! `extents` entry), so later phases and the verifier can tell which stack
//! object an address belongs to; a cell in no slot is refused. A pointer one
//! past a slot's end or one before its start belongs to that slot.
//!
//! # Example
//! `keep(a: f64, b: f64)` on `-m32 -O0` already uses the cells `[bp-0x10]` and
//! `[bp-0x8]`. When `FloatAssign` has to store `a` across a call, it asks the
//! frame for a slot and gets `[bp-0x18]`, the next eight bytes below the lowest
//! in use.
//!
//! ```lir
//! keep: 14 instructions
//!   block 0x0000
//!     0x0000  v1 := fld [[bp+0x8]]  ; inserted
//!     0x0003  [[bp-0x10]] := fstp v3  ; inserted
//!     0x0005  [[bp-0x8]] := fstp v4  ; inserted
//!     ; ...
//!     0x0007  [[bp-0x18]] := fstp v1  ; inserted
//!     0x0007  call   ; inserted; delivers v6@EAX; clobbers EAX,ST0
//!     0x0009  v1 := fld [[bp-0x18]]  ; inserted
//! ```
//!
//! From `ISEL_DUMP=1` of that function, stage `FloatAssign`.
//!
//! # Runs
//! `lir frame` in `assemble::staged`, once per function after selection and
//! `framefree`; the frame is then shared by every machine phase that spills.
//!
//! # Needs
//! [`crate::backend::isel`] for `extents`, `calls` and `hole`;
//! [`crate::backend::nativeframe`] for a native procedure's reservation.
//!
//! # See also
//! `docs/optimizations/second-frame.md`.

use std::fmt;

use llrm_lir::registers::RegId;

use crate::backend::nativeframe::{self, Plan};
use crate::model::ir::{Addr, Loc, Mem, Operation, Space};
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

// BC's frame is word-aligned. Extended floating spills occupy five words.
pub const WORD: i64 = 2;
pub const ENTER: &str = "B$ENRA";
pub const LEAVE: &str = "B$EXSA";
// runtime/inc/stack.inc: FR_SIZE, below BP and above locals.
pub const RUNTIME_SIZE: i64 = 10;

/// Python `SlotKey = int | tuple[str, int]`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SlotKey {
    Value(i64),
    Named(String, i64),
}

impl From<i64> for SlotKey {
    fn from(value: i64) -> Self {
        Self::Value(value)
    }
}

/// An untyped integer literal.
impl From<i32> for SlotKey {
    fn from(value: i32) -> Self {
        Self::Value(i64::from(value))
    }
}

impl From<u32> for SlotKey {
    fn from(value: u32) -> Self {
        Self::Value(i64::from(value))
    }
}

impl From<(&str, i64)> for SlotKey {
    fn from((name, number): (&str, i64)) -> Self {
        Self::Named(name.to_owned(), number)
    }
}

/// The existing runtime frame has no established extent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

/// Where a body's stack objects are. Mutable: slots are handed out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    // The deepest displacement BC's own code already reaches, which is
    // negative. Everything this hands out is below it.
    pub floor: i64,
    pub slots: IndexMap<SlotKey, i64>,
    pub native: Option<Plan>,
    pub native_pins: IndexMap<u32, RegId>,
    // Capacity belongs to the stack object, not to whichever virtual value
    // first received it.
    pub capacities: IndexMap<i64, i64>,
    /// Bytes just below BP, above the allocas, that spill slots fill first.
    pub hole: i64,
    /// The slots the selector laid out (first byte, size); see
    /// `isel::Selected::extents`.
    pub extents: Vec<(i64, i64)>,
    /// Whether the spiller gives each spilled value a slot of its own, to be
    /// shared once the allocation is chosen (`spiller::slots_colored`).
    pub defer_colours: bool,
    /// `LiveStacks`: each spilled home's segments `[start, end)` in the
    /// allocator's slot numbering, merged from the spilled registers' ranges
    /// as `InlineSpiller` writes them. Empty where the spills were not written
    /// inline (`spiller::slots_colored` then works the homes' liveness out).
    pub stacks: IndexMap<i64, Vec<(i64, i64)>>,
    /// The most a slot is aligned to, in bytes: the target's stack slot, so a
    /// dword or a quad is not left across a doubleword boundary, which a 486
    /// charges 3 clocks for (`WORD` where BC's frame is word-aligned).
    pub align: i64,
    /// The widest alignment of an alloca or stack temporary: a hole above them
    /// that is not a multiple of it moves them by more than itself.
    pub alloca_align: i64,
    /// The largest outgoing-argument area of a call, reserved in the
    /// prologue where no call is pushed
    /// (`backend::callframe::ReserveCallFrame`;
    /// the pin's `MaxCallFrameSize`). The lowest bytes of the frame.
    pub call_area: i64,
    /// The homes of the selector's scratch temporaries (a rounded float's
    /// cell): `slots_colored` shares them with each other, never with a spill
    /// slot, which they would split and widen.
    pub scratch: std::collections::BTreeSet<i64>,
}

impl Frame {
    #[must_use]
    pub fn new(floor: i64) -> Self {
        Self {
            floor,
            slots: IndexMap::default(),
            native: None,
            native_pins: IndexMap::default(),
            capacities: IndexMap::default(),
            hole: 0,
            extents: Vec::new(),
            defer_colours: false,
            stacks: IndexMap::default(),
            align: WORD,
            alloca_align: 1,
            call_area: 0,
            scratch: Default::default(),
        }
    }

    /// The first byte of the slot that holds `disp`, as the layout the cells
    /// were selected under put it: the lowest of the selector's extents
    /// nearest it, the incoming arguments (from BP up) as one slot at 0; none
    /// when it laid out no slot.
    pub(crate) fn home_of(
        &self,
        disp: i64,
    ) -> Option<i64> {
        // [bp+0] holds the caller's BP, never data: an address there is one
        // past the end of the slot that ends at BP.
        if disp > 0 || (disp == 0 && !self.extents.iter().any(|(start, size)| start + size == 0)) {
            return Some(0);
        }
        // The slot nearest the displacement, the lowest on a tie: a pointer
        // into a slot, one past its end (a strength-reduced loop's
        // limit) or before its start (a pre-incremented one) belongs to the
        // slot it was made from.
        let gap =
            |(start, size): &(i64, i64)| if disp < *start { start - disp } else { (disp - (start + size - 1)).max(0) };
        self.extents.iter().min_by_key(|extent| (gap(extent), extent.0)).map(|(start, _)| *start)
    }

    /// `body` with every frame cell tagged with its slot (`Addr::in_slot`) and
    /// the body marked `slotted`: the verifier then fails any cell a later
    /// phase makes without one. A cell in no slot is refused.
    pub fn tagged(
        &self,
        body: &LirBody,
    ) -> Result<LirBody, Refused> {
        let tag = |addr: Addr| -> Result<Addr, Refused> {
            if addr.slot_home().is_some() {
                return Ok(addr);
            }
            self.home_of(addr.disp)
                .map(|home| addr.in_slot(home))
                .ok_or_else(
                    || Refused(format!(
                        "@{}: a frame cell at {} is in no slot the selector laid out ({:?})",
                        body.name, addr.disp, self.extents
                    )),
                )
        };
        let tag_loc = |place: &Loc| -> Result<Loc, Refused> {
            let Some(at) = place.address().filter(|at| framed(at.addr)) else { return Ok(place.clone()) };
            let addr = at.addr.map(&tag).transpose()?;
            Ok(place.map_address(|at| crate::model::ir::AddressRef { addr, ..at }))
        };
        let mut blocks = Vec::with_capacity(body.blocks.len());
        for block in &body.blocks {
            let mut insns = Vec::with_capacity(block.insns.len());
            for one in &block.insns {
                let Some(what) = &one.what else {
                    insns.push(std::sync::Arc::clone(one));
                    continue;
                };
                let dests = what.dests.iter().map(&tag_loc).collect::<Result<Vec<_>, _>>()?;
                let sources = what.sources.iter().map(&tag_loc).collect::<Result<Vec<_>, _>>()?;
                if !what.dests.iter().chain(&what.sources).any(untagged) {
                    insns.push(std::sync::Arc::clone(one));
                } else {
                    insns.push(std::sync::Arc::new(crate::model::lir::Insn {
                        what: Some(crate::model::ir::Semantics { dests, sources, ..what.clone() }),
                        ..(**one).clone()
                    }));
                }
            }
            blocks.push(crate::model::lir::LirBlock { insns: insns.into(), ..block.clone().into_inner() }.into());
        }
        let mut tagged = body.with_blocks(blocks);
        let homes = body
            .homes
            .iter()
            .map(|(value, cell)| {
                Ok((
                    *value,
                    if framed(cell.addr) {
                        Mem { addr: cell.addr.map(&tag).transpose()?, ..cell.clone() }
                    } else {
                        cell.clone()
                    },
                ))
            })
            .collect::<Result<std::collections::BTreeMap<_, _>, Refused>>()?;
        tagged.homes = std::sync::Arc::new(homes);
        tagged.variables = body
            .variables
            .iter()
            .map(|variable| match &variable.place {
                crate::model::lir::DebugPlace::At(addr) if addr.space == Space::Frame => {
                    Ok(crate::model::lir::DebugVariable {
                        place: crate::model::lir::DebugPlace::At(tag(*addr)?),
                        ..variable.clone()
                    })
                }
                _ => Ok(variable.clone()),
            })
            .collect::<Result<Vec<_>, Refused>>()?;
        tagged.slotted = true;
        Ok(tagged)
    }

    /// How many bytes the prologue has to reserve beyond BC's own.
    #[must_use]
    pub fn size(&self) -> i64 {
        -(self.lowest() - self.floor) + self.call_area
    }

    /// The lowest home, or the floor.
    fn lowest(&self) -> i64 {
        self.slots.values().copied().min().map_or(self.floor, |lowest| lowest.min(self.floor))
    }

    /// Whether `disp` is in spill storage: below the floor, or in the hole.
    pub fn spills_at(
        &self,
        disp: i64,
    ) -> bool {
        disp < self.floor || (-self.hole..0).contains(&disp)
    }

    /// This value's displacement, creating one where it has none.
    ///
    /// A slot belongs to its value: `Frame::slot` shares none. The sharing is
    /// the colourers', `spiller::_color_slots`, `spiller::slots_colored` and
    /// isel's `alloca_groups` (the rule is `slots`), and both stand down in
    /// a function that calls a `returns_twice` routine
    /// (`LirBody::returns_twice`, `memory::calls_returns_twice`): after
    /// `longjmp` a value spilled before `setjmp` is read from its slot, so
    /// a slot recycled for another value (LLVM's stack colouring) would
    /// hand back the wrong one.
    pub fn slot(
        &mut self,
        value: impl Into<SlotKey>,
        width: impl Into<i64>,
    ) -> Result<i64, Refused> {
        let value = value.into();
        let width: i64 = width.into();
        if self.native.as_ref().is_some_and(|native| !native.framed) {
            return Err(Refused("a frameless native procedure cannot hold a spill below its caller's BP".to_owned()));
        }
        if !self.slots.contains_key(&value) {
            let capacity = width.max(WORD);
            // Aligned as its width is, at most to `align`: gcc's
            // `assign_stack_local` and LLVM's `calculateFrameObjectOffsets`.
            let align = (1 << capacity.ilog2()).min(self.align).max(1);
            let below = |at: i64| at.div_euclid(align) * align;
            let filled = self.slots.values().copied().filter(|home| *home >= -self.hole).min().unwrap_or(0);
            let home = if below(filled - capacity) >= -self.hole {
                below(filled - capacity)
            } else {
                below(self.lowest() - capacity)
            };
            self.slots.insert(value.clone(), home);
            self.capacities.insert(self.slots[&value], capacity);
        }
        Ok(self.slots[&value])
    }

    /// The memory operand that reads or writes this value's slot.
    pub fn saved(&self) -> (IndexMap<SlotKey, i64>, IndexMap<i64, i64>) {
        (self.slots.clone(), self.capacities.clone())
    }

    /// Forget a discarded trial's slots. Kept, a later spill of the same
    /// value reuses its stale slot without an overlap check.
    pub fn restore(
        &mut self,
        saved: &(IndexMap<SlotKey, i64>, IndexMap<i64, i64>),
    ) {
        self.slots = saved.0.clone();
        self.capacities = saved.1.clone();
        self.stacks.clear();
    }

    pub fn cell(
        &mut self,
        value: impl Into<SlotKey>,
        width: impl Into<i64>,
    ) -> Result<Mem, Refused> {
        let width: i64 = width.into();
        let disp = self.slot(value, width)?;
        Ok(Mem {
            through: crate::model::ir::FRAME,
            offset: 0,
            disp_width: 2,
            ..Mem::new(Some(Addr::new(Space::Frame, disp).in_slot(disp)), width as u32)
        })
    }
}

/// Whether `addr` is a frame cell proper: the indexed form of an array (a
/// literal displacement through BP) is not tagged, since a literal address's
/// `index` may be a symbol's.
fn framed(addr: Option<Addr>) -> bool {
    addr.is_some_and(|addr| addr.space == Space::Frame)
}

/// Whether a frame operand names no slot: its `Addr` carries no tag.
fn untagged(place: &Loc) -> bool {
    place.address().is_some_and(|cell| framed(cell.addr) && cell.addr.is_some_and(|addr| addr.slot_home().is_none()))
}

/// A frame for this body, starting below everything it already reaches.
pub fn of(
    body: &LirBody,
    calls: Option<&IndexMap<i64, String>>,
    family: &str,
    native: Option<Plan>,
) -> Result<Frame, Refused> {
    let mut floor = native.as_ref().map_or(0, |native| native.entry.floor);
    // VBDCL10E rtenexit 0024..0036 pushes ten words before SUB SP,CX.
    let runtime_size = if family == "vbdos" { 20 } else { RUNTIME_SIZE };
    let mut constants: IndexMap<u32, i64> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
                (Operation::Move, [Loc::Held(held)], [Loc::Imm(count)]) => {
                    constants.insert(held.value, count.value);
                }
                (Operation::Call, _, _)
                    if calls.and_then(|calls| calls.get(&one.at)).map(String::as_str) == Some(ENTER) =>
                {
                    let sizes: Vec<u32> =
                        one.requires.iter().filter(|(_, reg)| *reg == RegId::CX).map(|(held, _)| held.value).collect();
                    if sizes.len() != 1 || !constants.contains_key(&sizes[0]) {
                        return Err(Refused("runtime frame size is not a known constant".to_owned()));
                    }
                    floor = floor.min(-runtime_size - constants[&sizes[0]]);
                }
                _ => {}
            }
            for where_ in what.dests.iter().chain(&what.sources) {
                let Some(addr) = where_.address().and_then(|one| one.addr) else { continue };
                if addr.space != Space::Frame {
                    continue;
                }
                if let (Some(native), Loc::Mem(memory)) = (&native, where_) {
                    if memory.stack_argument && native.outgoing.contains(&(one.at, addr.disp, memory.width)) {
                        continue;
                    }
                }
                // A cell that says which object it was made from and lies
                // below it (an index that makes up the rest) reaches no deeper
                // than the object.
                floor = floor.min(addr.slot_home().map_or(addr.disp, |home| addr.disp.max(home)));
            }
        }
    }
    if native.as_ref().is_some_and(|native| floor < native.entry.floor) {
        return Err(Refused("native frame references extend below its established reservation".to_owned()));
    }
    let native_pins = native.as_ref().map_or_else(IndexMap::default, |native| nativeframe::pins(body, native));
    Ok(Frame { native, native_pins, ..Frame::new(floor) })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Refused, of};
    use crate::model::ir::{Addr, AddressRef, Held, Imm, Loc, Operation, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Semantics {
        Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
    }

    fn body_of(insns: Vec<Insn>) -> LirBody {
        let block = LirBlock::new(0, insns.into_iter().map(Arc::new).collect());
        LirBody::new("entry", 0, vec![block.into()], IndexMap::default(), IndexMap::default())
    }

    #[test]
    fn test_entry_frame_size_comes_from_cx_not_call_arity() {
        // COM_CHECK_ARGS spilled at BP-2, corrupting FindFrame's linked list.
        use llrm_lir::registers::RegId;
        const AX: RegId = RegId::AX;
        const BX: RegId = RegId::BX;
        const CX: RegId = RegId::CX;
        const DI: RegId = RegId::DI;
        const DX: RegId = RegId::DX;
        const SI: RegId = RegId::SI;
        for registers in [vec![CX], vec![BX, CX], vec![AX, BX, CX, DX, SI, DI]] {
            let size = Held { value: 100, width: 2 };
            let init = Insn::new(
                0,
                Some((0, 3)),
                Some(semantics(
                    Operation::Move,
                    "mov",
                    vec![Loc::Held(size)],
                    vec![Loc::Imm(Imm { value: 12, width: 2, address: None })],
                )),
                vec![100],
                vec![],
            );
            let operands: Vec<Held> = registers
                .iter()
                .map(|&reg| if reg == CX { size } else { Held { value: reg.index() as u32, width: 2 } })
                .collect();
            let mut call = Insn::new(
                3,
                Some((3, 8)),
                Some(semantics(Operation::Call, "call", vec![], operands.iter().copied().map(Loc::Held).collect())),
                vec![],
                operands.iter().map(|value| value.value).collect(),
            );
            call.requires = operands.iter().copied().zip(registers.iter().copied()).collect();
            let body = body_of(vec![init.clone(), call.clone()]);
            let calls = IndexMap::from_iter([(3, "B$ENRA".to_owned())]);
            let mut owned = of(&body, Some(&calls), "", None).unwrap();
            assert_eq!(owned.floor, -22);
            assert_eq!(owned.slot(200_i64, 2), Ok(-24));
            // VBDOS pushes ten words below BP before subtracting CX, not five.
            let mut vbdos = of(&body, Some(&calls), "vbdos", None).unwrap();
            assert_eq!(vbdos.floor, -32);
            assert_eq!(vbdos.slot(200_i64, 2), Ok(-34));
            let mut address = init.clone();
            address.at = 8;
            address.what = Some(semantics(
                Operation::Move,
                "lea",
                vec![Loc::Held(Held { value: 101, width: 2 })],
                vec![Loc::Address(AddressRef::new(Some(Addr::new(Space::Frame, -32))))],
            ));
            let addressed = body_of(vec![init.clone(), call.clone(), address]);
            assert_eq!(of(&addressed, Some(&calls), "", None).unwrap().slot(200_i64, 2), Ok(-34));
            for required in [vec![], vec![(Held { value: 999, width: 2 }, CX)]] {
                let mut invalid_call = call.clone();
                invalid_call.requires = required;
                let invalid = body_of(vec![init.clone(), invalid_call]);
                let Err(Refused(message)) = of(&invalid, Some(&calls), "", None) else {
                    panic!("an unknown size is refused");
                };
                assert!(message.contains("known constant"));
            }
        }
    }

    /// matmul's end pointer is one past its array and quicksort's one before:
    /// an address made from a slot is tagged with that slot, not refused
    /// (bench matmul -O2: "a frame cell at -32 is in no slot").
    #[test]
    fn test_a_pointer_one_past_a_slot_or_before_it_is_tagged_with_the_slot() {
        let mut frame = super::Frame::new(0);
        frame.extents = vec![(-160, 128), (-288, 128)];
        assert_eq!(frame.home_of(-100), Some(-160));
        assert_eq!(frame.home_of(-32), Some(-160), "one past the end");
        assert_eq!(frame.home_of(-30), Some(-160), "past the end");
        assert_eq!(frame.home_of(-292), Some(-288), "before the start");
        assert_eq!(frame.home_of(-160), Some(-160), "the start of one slot is the end of another");
        assert_eq!(frame.home_of(6), Some(0), "an incoming argument");
    }

    /// d_faces' spills sat at `[esp+210]`: a quad and its dwords at 2 mod 4 in
    /// flat code (30 of 62 qword and 81 of 187 dword stack operands), each
    /// 3 clocks more on a 486. The slot is aligned as its width is, up to
    /// the target's.
    #[test]
    fn test_a_slot_is_aligned_to_its_width_up_to_the_targets_stack_slot() {
        let mut flat = super::Frame::new(0);
        flat.align = 4;
        let homes: Vec<i64> = [(1_i64, 2), (2, 8), (3, 2), (4, 4), (5, 8)]
            .iter()
            .map(|&(value, width)| flat.slot(value, width).expect("a slot"))
            .collect();
        assert!(homes[1] % 4 == 0 && homes[3] % 4 == 0 && homes[4] % 4 == 0, "{homes:?}");
        assert!(homes[0] % 2 == 0 && homes[2] % 2 == 0, "{homes:?}");
        let mut real = super::Frame::new(0);
        assert_eq!(real.slot(1_i64, 2).expect("a slot"), -2);
        assert_eq!(real.slot(2_i64, 8).expect("a slot"), -10, "real mode keeps BC's words");
    }

    #[test]
    fn test_a_spill_cell_names_the_slot_it_is_the_home_of() {
        let mut frame = super::Frame::new(-10);
        let cell = frame.cell(7_i64, 2).expect("a cell");
        let addr = cell.addr.expect("an address");
        assert_eq!(addr.slot_home(), Some(addr.disp));
    }
}
