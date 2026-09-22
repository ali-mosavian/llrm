//! Port of `qbopt/objectfile/module.py`: one BC module, as the analysis
//! layer needs to see it.
//!
//! Ported so far: the address vocabulary (`Space`, `Addr`, `Group` and the
//! address constructors). `Module`, `SourceMap` and the BC-object readers
//! follow with the BC rewrite path.

use std::collections::BTreeSet;
use std::fmt;

use iced_x86::Register;

use crate::support::pyrepr::{self, Repr};

pub const CALL_FAR: u8 = 0x9A;

/// BC's own linker directive segment group.
pub const DGROUP: &str = "DGROUP";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Space {
    /// relocated: an offset into the segment `index` names
    Segment,
    /// relocated against EXTDEF; distinct symbols may alias
    External,
    /// bp-relative, so the displacement really is in the code
    Frame,
    /// a displacement in the code that no fixup claims
    Literal,
    /// relocated against a GRPDEF rather than a SEGDEF
    Group,
    /// a $DYNAMIC array element: `es:[bx]`, `es:[bx+2]`
    Far,
    /// a slot the code itself pushed, addressed by its depth
    Stack,
}

impl Space {
    pub const ALL: [Space; 7] =
        [Space::Segment, Space::External, Space::Frame, Space::Literal, Space::Group, Space::Far, Space::Stack];

    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Space::Segment => "SEGMENT",
            Space::External => "EXTERNAL",
            Space::Frame => "FRAME",
            Space::Literal => "LITERAL",
            Space::Group => "GROUP",
            Space::Far => "FAR",
            Space::Stack => "STACK",
        }
    }

    /// The `StrEnum` value, which is also `str(space)`.
    pub fn value(self) -> &'static str {
        match self {
            Space::Segment => "seg",
            Space::External => "external",
            Space::Frame => "bp",
            Space::Literal => "abs",
            Space::Group => "grp",
            Space::Far => "far",
            Space::Stack => "sp",
        }
    }
}

/// A `StrEnum` orders as its string value.
impl Ord for Space {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value().cmp(other.value())
    }
}

impl PartialOrd for Space {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Space {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.value())
    }
}

impl Repr for Space {
    fn repr(&self) -> String {
        pyrepr::str_enum("Space", self.name(), self.value())
    }
}

/// `INDEX_NAMES`: base is si/di for a SEGMENT array element and bx for a FAR one.
pub fn index_names(register: Register) -> Option<&'static str> {
    match register {
        Register::SI => Some("si"),
        Register::DI => Some("di"),
        Register::BX => Some("bx"),
        _ => None,
    }
}

/// `SEGMENT_NAMES`.
pub fn segment_names(register: Register) -> Option<&'static str> {
    match register {
        Register::ES => Some("es"),
        Register::DS => Some("ds"),
        Register::SS => Some("ss"),
        Register::CS => Some("cs"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Addr {
    pub space: Space,
    pub disp: i64,
    pub index: i64,
    /// NONE for a bare displacement; an array element's index register.
    pub base: Register,
    /// NONE except for an address whose selector differs from the default.
    pub segment: Register,
}

impl Addr {
    /// `Addr(space, disp)`.
    pub const fn new(space: Space, disp: i64) -> Self {
        Addr { space, disp, index: 0, base: Register::None, segment: Register::None }
    }

    /// Whether the address names bytes without a run-time index.
    pub fn direct(&self) -> bool {
        self.base == Register::None
    }

    pub fn plus(&self, bytes_along: i64) -> Addr {
        Addr { disp: self.disp + bytes_along, ..*self }
    }
}

/// `f"{value:+#x}"`.
pub fn signed_hex(value: i64) -> String {
    if value < 0 { format!("-{:#x}", value.unsigned_abs()) } else { format!("+{value:#x}") }
}

/// `f"r{register}"`: iced's register number, as Python prints the int.
fn numbered(register: Register) -> String {
    format!("r{}", register as u32)
}

impl Repr for Addr {
    fn repr(&self) -> String {
        if self.space == Space::Far {
            let seg = segment_names(self.segment).map_or_else(|| numbered(self.segment), str::to_owned);
            let base = index_names(self.base).map_or_else(|| numbered(self.base), str::to_owned);
            return format!("[{seg}:{base}{}]", signed_hex(self.disp));
        }
        let where_ = if matches!(self.space, Space::Segment | Space::External) {
            format!("{}:{}", self.space, self.index)
        } else {
            self.space.to_string()
        };
        let indexed = if self.base != Register::None {
            format!("+{}", index_names(self.base).map_or_else(|| numbered(self.base), str::to_owned))
        } else {
            String::new()
        };
        format!("[{where_}{indexed}{}]", signed_hex(self.disp))
    }
}

/// DGROUP's segment indexes, and which of them the link overlays.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Group {
    pub members: BTreeSet<i64>,
    /// the COMMON-combined ones
    pub shared: BTreeSet<i64>,
}

impl Group {
    pub fn new(members: impl IntoIterator<Item = i64>, shared: impl IntoIterator<Item = i64>) -> Self {
        Group { members: members.into_iter().collect(), shared: shared.into_iter().collect() }
    }

    pub fn contains(&self, index: i64) -> bool {
        self.members.contains(&index)
    }
}

pub fn frame_relative(literal: i64) -> Addr {
    Addr::new(Space::Frame, literal)
}

pub fn far_pointer(literal: i64, base: Register, segment: Register) -> Addr {
    Addr { base, segment, ..Addr::new(Space::Far, literal) }
}

/// The resolver for code with no fixups behind it, as every unit test has.
pub fn literal_only(_field_offset: i64, literal: i64) -> Addr {
    Addr::new(Space::Literal, literal)
}

/// The widest access anything here can name -- an x87 qword load.
pub const WIDEST: i64 = 8;

/// Whether [disp, disp+width) intersect -- arithmetic, not analysis.
pub fn _overlaps(a: &Addr, a_width: i64, b: &Addr, b_width: i64) -> bool {
    a.disp < b.disp + b_width && b.disp < a.disp + a_width
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected strings printed by `repr(Addr(...))` in Python.
    #[test]
    fn addr_repr_matches_python() {
        assert_eq!(frame_relative(-8).repr(), "[bp-0x8]");
        assert_eq!(Addr::new(Space::Literal, 0).repr(), "[abs+0x0]");
        assert_eq!(Addr { index: 3, ..Addr::new(Space::Segment, 0x12) }.repr(), "[seg:3+0x12]");
        assert_eq!(Addr { base: Register::SI, ..Addr::new(Space::Frame, 2) }.repr(), "[bp+si+0x2]");
        assert_eq!(far_pointer(2, Register::BX, Register::ES).repr(), "[es:bx+0x2]");
        assert_eq!(Addr { base: Register::AX, ..Addr::new(Space::Frame, 0) }.repr(), "[bp+r21+0x0]");
        assert_eq!(Space::Frame.repr(), "<Space.FRAME: 'bp'>");
    }
}
