//! Port of `qbopt/backend/frame.py`: the stack slots a body needs, and where
//! they are.
//!
//! A slot is two bytes at `[bp-n]`, and `size` is what the prologue has to
//! take off sp.

use std::fmt;

use iced_x86::Register;
use indexmap::IndexMap;

use crate::backend::nativeframe::{self, Plan};
use crate::model::ir::{Addr, Loc, Mem, Operation, Space};
use crate::model::lir::LirBody;

// BC's frame is word-aligned. Extended floating spills occupy five words.
pub const WORD: i64 = 2;
pub const ENTER: &str = "B$ENRA";
pub const LEAVE: &str = "B$EXSA";
pub const RUNTIME_SIZE: i64 = 10; // runtime/inc/stack.inc: FR_SIZE, below BP and above locals.

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

impl From<(&str, i64)> for SlotKey {
    fn from((name, number): (&str, i64)) -> Self {
        Self::Named(name.to_owned(), number)
    }
}

/// The existing runtime frame has no established extent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    pub native_pins: IndexMap<u32, Register>,
    // Capacity belongs to the stack object, not to whichever virtual value
    // first received it.
    pub capacities: IndexMap<i64, i64>,
}

impl Frame {
    #[must_use]
    pub fn new(floor: i64) -> Self {
        Self {
            floor,
            slots: IndexMap::new(),
            native: None,
            native_pins: IndexMap::new(),
            capacities: IndexMap::new(),
        }
    }

    /// How many bytes the prologue has to reserve beyond BC's own.
    #[must_use]
    pub fn size(&self) -> i64 {
        -(self.slots.values().copied().min().unwrap_or(self.floor) - self.floor)
    }

    /// This value's displacement, creating one where it has none.
    pub fn slot(&mut self, value: impl Into<SlotKey>, width: i64) -> Result<i64, Refused> {
        let value = value.into();
        if self.native.as_ref().is_some_and(|native| !native.framed) {
            return Err(Refused(
                "a frameless native procedure cannot hold a spill below its caller's BP".to_owned(),
            ));
        }
        if !self.slots.contains_key(&value) {
            let lowest = self.slots.values().copied().min().unwrap_or(self.floor);
            let capacity = width.max(WORD);
            self.slots.insert(value.clone(), lowest - capacity);
            self.capacities.insert(self.slots[&value], capacity);
        }
        Ok(self.slots[&value])
    }

    /// The memory operand that reads or writes this value's slot.
    pub fn cell(&mut self, value: impl Into<SlotKey>, width: i64) -> Result<Mem, Refused> {
        let disp = self.slot(value, width)?;
        Ok(Mem {
            through: Register::BP,
            offset: 0,
            disp_width: 2,
            ..Mem::new(Some(Addr::new(Space::Frame, disp)), width as u32)
        })
    }
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
    let mut constants: IndexMap<u32, i64> = IndexMap::new();
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
                    let sizes: Vec<u32> = one
                        .requires
                        .iter()
                        .filter(|(_, reg)| *reg == Register::CX)
                        .map(|(held, _)| held.value)
                        .collect();
                    if sizes.len() != 1 || !constants.contains_key(&sizes[0]) {
                        return Err(Refused("runtime frame size is not a known constant".to_owned()));
                    }
                    floor = floor.min(-runtime_size - constants[&sizes[0]]);
                }
                _ => {}
            }
            for where_ in what.dests.iter().chain(&what.sources) {
                let addr = match where_ {
                    Loc::Mem(memory) => memory.addr.as_ref(),
                    Loc::Address(address) => address.addr.as_ref(),
                    _ => None,
                };
                let Some(addr) = addr else { continue };
                if addr.space != Space::Frame {
                    continue;
                }
                if let (Some(native), Loc::Mem(memory)) = (&native, where_) {
                    if memory.stack_argument && native.outgoing.contains(&(one.at, addr.disp, memory.width)) {
                        continue;
                    }
                }
                floor = floor.min(addr.disp);
            }
        }
    }
    if native.as_ref().is_some_and(|native| floor < native.entry.floor) {
        return Err(Refused(
            "native frame references extend below its established reservation".to_owned(),
        ));
    }
    let native_pins = native.as_ref().map_or_else(IndexMap::new, |native| nativeframe::pins(body, native));
    Ok(Frame {
        native,
        native_pins,
        ..Frame::new(floor)
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;
    use indexmap::IndexMap;

    use super::{Refused, of};
    use crate::model::ir::{Addr, Address, Held, Imm, Loc, Operation, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    fn body_of(insns: Vec<Insn>) -> LirBody {
        let block = LirBlock::new(0, insns.into_iter().map(Arc::new).collect());
        LirBody::new("entry", 0, vec![block], IndexMap::new(), IndexMap::new())
    }

    #[test]
    fn test_entry_frame_size_comes_from_cx_not_call_arity() {
        // COM_CHECK_ARGS spilled at BP-2, corrupting FindFrame's linked list.
        use Register::{AX, BX, CX, DI, DX, SI};
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
                .map(|&reg| if reg == CX { size } else { Held { value: reg as u32, width: 2 } })
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
            let calls = IndexMap::from([(3, "B$ENRA".to_owned())]);
            let mut owned = of(&body, Some(&calls), "", None).unwrap();
            assert_eq!(owned.floor, -22);
            assert_eq!(owned.slot(200, 2), Ok(-24));
            // VBDOS pushes ten words below BP before subtracting CX, not five.
            let mut vbdos = of(&body, Some(&calls), "vbdos", None).unwrap();
            assert_eq!(vbdos.floor, -32);
            assert_eq!(vbdos.slot(200, 2), Ok(-34));
            let mut address = init.clone();
            address.at = 8;
            address.what = Some(semantics(
                Operation::Move,
                "lea",
                vec![Loc::Held(Held { value: 101, width: 2 })],
                vec![Loc::Address(Address::new(Some(Addr::new(Space::Frame, -32))))],
            ));
            let addressed = body_of(vec![init.clone(), call.clone(), address]);
            assert_eq!(of(&addressed, Some(&calls), "", None).unwrap().slot(200, 2), Ok(-34));
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
}
