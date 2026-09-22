//! MINIMAL port of `qbopt/backend/frame.py` for `port/regalloc` only.
//!
//! `port/frame-phases` owns the full port. This carries what the spiller
//! and allocator call -- `Frame`, `slot`, `cell`, `size`, `of` -- without
//! the native-frame plan (`native`, `native_pins`), which needs
//! `nativeframe`. The merge takes the full port.

use std::fmt;

use iced_x86::Register;
use indexmap::IndexMap;

use crate::model::ir::{Addr, Loc, Mem, Operation, Space};
use crate::model::lir::LirBody;

// BC's frame is word-aligned.
pub const WORD: u32 = 2;
pub const ENTER: &str = "B$ENRA";
pub const LEAVE: &str = "B$EXSA";
pub const RUNTIME_SIZE: i64 = 10;

/// `int | tuple[str, int]`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SlotKey {
    Value(i64),
    Named(String, i64),
}

impl From<u32> for SlotKey {
    fn from(value: u32) -> Self {
        Self::Value(i64::from(value))
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
    pub floor: i64,
    pub slots: IndexMap<SlotKey, i64>,
    pub capacities: IndexMap<i64, u32>,
}

impl Frame {
    pub fn new(floor: i64) -> Self {
        Self { floor, slots: IndexMap::new(), capacities: IndexMap::new() }
    }

    /// How many bytes the prologue has to reserve beyond BC's own.
    pub fn size(&self) -> i64 {
        -(self.slots.values().copied().min().unwrap_or(self.floor) - self.floor)
    }

    /// This value's displacement, creating one where it has none.
    pub fn slot(&mut self, value: impl Into<SlotKey>, width: u32) -> Result<i64, Refused> {
        let value = value.into();
        if !self.slots.contains_key(&value) {
            let lowest = self.slots.values().copied().min().unwrap_or(self.floor);
            let capacity = width.max(WORD);
            let home = lowest - i64::from(capacity);
            self.slots.insert(value.clone(), home);
            self.capacities.insert(home, capacity);
        }
        Ok(self.slots[&value])
    }

    /// The memory operand that reads or writes this value's slot.
    pub fn cell(&mut self, value: impl Into<SlotKey>, width: u32) -> Result<Mem, Refused> {
        let home = self.slot(value, width)?;
        Ok(Mem {
            through: Register::BP,
            offset: 0,
            disp_width: 2,
            ..Mem::new(Some(Addr::new(Space::Frame, home)), width)
        })
    }
}

/// A frame for this body, starting below everything it already reaches.
pub fn of(body: &LirBody, calls: Option<&IndexMap<i64, String>>, family: &str) -> Result<Frame, Refused> {
    let mut floor = 0;
    let runtime_size = if family == "vbdos" { 20 } else { RUNTIME_SIZE };
    let mut constants: IndexMap<u32, i64> = IndexMap::new();
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
                (Operation::Move, [Loc::Held(held)], [Loc::Imm(count)]) => {
                    constants.insert(held.value, count.value);
                }
                (Operation::Call, _, _) if calls.and_then(|calls| calls.get(&one.at)).map(String::as_str) == Some(ENTER) => {
                    let sizes: Vec<u32> = one
                        .requires
                        .iter()
                        .filter(|(_held, register)| *register == Register::CX)
                        .map(|(held, _register)| held.value)
                        .collect();
                    if sizes.len() != 1 || !constants.contains_key(&sizes[0]) {
                        return Err(Refused("runtime frame size is not a known constant".to_owned()));
                    }
                    floor = floor.min(-runtime_size - constants[&sizes[0]]);
                }
                _ => {}
            }
            for place in what.dests.iter().chain(&what.sources) {
                let addr = match place {
                    Loc::Mem(cell) => cell.addr,
                    Loc::Address(address) => address.addr,
                    _ => None,
                };
                if let Some(addr) = addr {
                    if addr.space == Space::Frame {
                        floor = floor.min(addr.disp);
                    }
                }
            }
        }
    }
    Ok(Frame::new(floor))
}
