//! Port of `qbopt/analysis/flags.py`: which flags are live.
//!
//! Ported so far: `Flag`, `ALL`, `DIVERGENT`. The block liveness functions
//! (`reads`, `writes`, `live_in`, `live_after`, `written_by`) follow with the
//! BC decode path they read.

use std::fmt;
use std::ops::{BitAnd, BitOr, BitOrAssign, Not};

use iced_x86::RflagsBits;

use crate::support::pyrepr::Repr;

/// `class Flag(IntFlag)`, valued as iced's `RflagsBits`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Flag(pub u32);

impl Flag {
    pub const NONE: Flag = Flag(0);
    pub const CF: Flag = Flag(RflagsBits::CF);
    pub const PF: Flag = Flag(RflagsBits::PF);
    pub const AF: Flag = Flag(RflagsBits::AF);
    pub const ZF: Flag = Flag(RflagsBits::ZF);
    pub const SF: Flag = Flag(RflagsBits::SF);
    pub const OF: Flag = Flag(RflagsBits::OF);

    /// Members in definition order, which is the order `repr` names them in.
    const MEMBERS: [(&'static str, Flag); 6] = [
        ("CF", Flag::CF),
        ("PF", Flag::PF),
        ("AF", Flag::AF),
        ("ZF", Flag::ZF),
        ("SF", Flag::SF),
        ("OF", Flag::OF),
    ];

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn union(self, other: Flag) -> Flag {
        Flag(self.0 | other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

pub const ALL: Flag = Flag(Flag::CF.0 | Flag::PF.0 | Flag::AF.0 | Flag::ZF.0 | Flag::SF.0 | Flag::OF.0);

/// The three whose value widening changes.
pub const DIVERGENT: Flag = Flag(Flag::ZF.0 | Flag::PF.0 | Flag::AF.0);

impl BitOr for Flag {
    type Output = Flag;
    fn bitor(self, other: Flag) -> Flag {
        Flag(self.0 | other.0)
    }
}

impl BitOrAssign for Flag {
    fn bitor_assign(&mut self, other: Flag) {
        self.0 |= other.0;
    }
}

impl BitAnd for Flag {
    type Output = Flag;
    fn bitand(self, other: Flag) -> Flag {
        Flag(self.0 & other.0)
    }
}

/// `~flag`: an `IntFlag` inverts within its members.
impl Not for Flag {
    type Output = Flag;
    fn not(self) -> Flag {
        Flag(!self.0 & ALL.0)
    }
}

/// `str(flag)`: an `IntFlag` prints as its integer.
impl fmt::Display for Flag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl Repr for Flag {
    fn repr(&self) -> String {
        if self.0 == 0 {
            return "<Flag.NONE: 0>".to_owned();
        }
        let mut names = Vec::new();
        let mut rest = self.0;
        for (name, member) in Flag::MEMBERS {
            if self.0 & member.0 != 0 {
                names.push(name.to_owned());
                rest &= !member.0;
            }
        }
        if rest != 0 {
            names.push(rest.to_string());
        }
        format!("<Flag.{}: {}>", names.join("|"), self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The draft valued Flag by x86 RFLAGS positions (CF = 1), not iced's
    /// RflagsBits (CF = 0x10) that Python's IntFlag uses; every mask against
    /// iced's reported reads and writes was off.
    #[test]
    fn flags_are_iced_rflags_bits_and_repr_like_python() {
        assert_eq!((Flag::CF.bits(), Flag::ZF.bits(), Flag::OF.bits()), (16, 4, 1));
        assert_eq!((Flag::CF | Flag::ZF).repr(), "<Flag.CF|ZF: 20>");
        assert_eq!((Flag::OF | Flag::PF | Flag::ZF).repr(), "<Flag.PF|ZF|OF: 37>");
        assert_eq!(ALL.repr(), "<Flag.CF|PF|AF|ZF|SF|OF: 63>");
        assert_eq!(Flag(0x40 | Flag::CF.0).repr(), "<Flag.CF|64: 80>");
        assert_eq!(Flag::NONE.repr(), "<Flag.NONE: 0>");
    }
}
