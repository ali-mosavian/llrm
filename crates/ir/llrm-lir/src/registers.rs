//! A target's register file as its description states it (`registers.regs`),
//! asked through queries, as LLVM's generated `MCRegisterInfo` is: the width,
//! root, lane, name and classes of a register, and the view of a root at a
//! width. The target generates its `Info` from its description; the backend
//! reads the one the driver bound. A `RegId` is iced's `Register` until the
//! newtype.

use iced_x86::Register;

pub type RegId = Register;

/// The classes a description may give a register, one bit each, in this order.
/// A name not here is refused where the table is generated.
pub const CLASSES: [&str; 12] =
    ["base", "byte", "frame", "gpr", "index", "int", "pc", "positional", "reserved", "stack", "string_segment", "x87"];

/// The class bits.
pub mod class {
    pub const BASE: u32 = 1 << 0;
    pub const BYTE: u32 = 1 << 1;
    pub const FRAME: u32 = 1 << 2;
    pub const GPR: u32 = 1 << 3;
    pub const INDEX: u32 = 1 << 4;
    /// An integer register: the 8, 16 and 32-bit views the width tables name.
    pub const INT: u32 = 1 << 5;
    pub const PC: u32 = 1 << 6;
    /// A position in a stack: an exchange of it is an effect and it is no
    /// register a pass renames or removes.
    pub const POSITIONAL: u32 = 1 << 7;
    pub const RESERVED: u32 = 1 << 8;
    pub const STACK: u32 = 1 << 9;
    pub const STRING_SEGMENT: u32 = 1 << 10;
    pub const X87: u32 = 1 << 11;
}

/// One register of the file.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub name: &'static str,
    pub bits: u32,
    pub root: RegId,
    /// The bit offset inside the root.
    pub lane: u32,
    /// `class` bits.
    pub classes: u32,
}

/// A target's register file: an entry by iced's number, and each register at
/// each width by its root.
pub struct Info {
    pub table: [Option<Entry>; 256],
    /// (root, bits, register) for every entry.
    pub views: &'static [(RegId, u32, RegId)],
}

impl Info {
    /// The entry for `register`, if the description lists it.
    pub fn get(
        &self,
        register: RegId,
    ) -> Option<&Entry> {
        self.table.get(register as usize).and_then(Option::as_ref)
    }

    /// Whether the description lists `register`.
    pub fn known(
        &self,
        register: RegId,
    ) -> bool {
        self.get(register).is_some()
    }

    /// The width of `register`, in bytes.
    pub fn bytes(
        &self,
        register: RegId,
    ) -> Option<i64> {
        self.get(register).map(|one| i64::from(one.bits / 8))
    }

    /// The register `register` is a view of (itself for a root, and for one the
    /// description does not list).
    pub fn root(
        &self,
        register: RegId,
    ) -> RegId {
        self.get(register).map_or(register, |one| one.root)
    }

    /// Which of a root's four bytes `register` names, one bit each: the lane
    /// mask.
    pub fn lanes(
        &self,
        register: RegId,
    ) -> i64 {
        self.get(register).map_or(0b1111, |one| {
            let width = (one.bits / 8).min(4);
            ((1_i64 << width) - 1) << (one.lane / 8)
        })
    }

    /// `register`'s own name, lowercase.
    pub fn name(
        &self,
        register: RegId,
    ) -> Option<&'static str> {
        self.get(register).map(|one| one.name)
    }

    /// Whether the description gives `register` every class in `mask`.
    pub fn in_class(
        &self,
        register: RegId,
        mask: u32,
    ) -> bool {
        self.get(register).is_some_and(|one| one.classes & mask == mask)
    }

    /// The register of `root` that is `bits` wide: the first by iced's number
    /// where several share it (AL and AH are both EAX's byte; AL is the one
    /// named).
    pub fn view(
        &self,
        root: RegId,
        bits: u32,
    ) -> Option<RegId> {
        self.views
            .iter()
            .filter(|(of, width, _)| *of == root && *width == bits)
            .map(|(_, _, one)| *one)
            .min_by_key(|one| *one as usize)
    }

    /// The entries in iced's number order, with their registers.
    pub fn entries(&self) -> impl Iterator<Item = (RegId, &Entry)> {
        self.views.iter().map(|(_, _, one)| *one).filter_map(|one| self.get(one).map(|entry| (one, entry)))
    }
}
