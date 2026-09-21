//! Source occurrences recognised as halves of a BC long operation.
//!
//! Direct port of `qbopt.legacy.lift.py:{Kind,Decoded,FIXUP}`.  These are
//! deliberately decoded OMF-source facts, not generic machine instructions.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::LazyLock;

use iced_x86::Code;

use crate::object::omf::module::Addr;

/// The seven single-half forms `lift.classify()` recognises.
///
/// Direct port of `qbopt.legacy.lift.py:Kind`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    Load,
    Store,
    Alu,
    Move,
    RegAlu,
    Not,
    AluImm,
}

impl Kind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Load => "ld",
            Self::Store => "st",
            Self::Alu => "op",
            Self::Move => "mv",
            Self::RegAlu => "rr",
            Self::Not => "not",
            Self::AluImm => "opi",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Ord for Kind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for Kind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// One classified half of a BC long operation.
///
/// Direct port of `qbopt.legacy.lift.py:Decoded`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Decoded {
    pub kind: Kind,
    pub pair: usize,
    pub half: usize,
    pub length: usize,
    pub src_pair: usize,
    pub alu: Option<Code>,
    pub mem: Option<Addr>,
    pub dlen: usize,
    pub disp_at: Option<usize>,
    pub imm: Option<i64>,
}

impl Decoded {
    #[must_use]
    pub const fn new(kind: Kind, pair: usize, half: usize, length: usize) -> Self {
        Self {
            kind,
            pair,
            half,
            length,
            src_pair: 0,
            alu: None,
            mem: None,
            dlen: 0,
            disp_at: None,
            imm: None,
        }
    }
}

/// The byte-identical restore sequences emitted after a widened operation.
///
/// Direct port of `qbopt.legacy.lift.py:FIXUP`.
pub static FIXUP: LazyLock<BTreeMap<usize, [u8; 4]>> = LazyLock::new(|| {
    BTreeMap::from([(0, [0x66, 0x50, 0x58, 0x5A]), (1, [0x66, 0x51, 0x59, 0x5B])])
});

#[cfg(test)]
mod tests {
    use super::{Decoded, FIXUP, Kind};

    #[test]
    fn omf_source_nodes_kind_spelling_and_order_are_python_strenum_order() {
        let mut kinds = vec![
            Kind::Load,
            Kind::Store,
            Kind::Alu,
            Kind::Move,
            Kind::RegAlu,
            Kind::Not,
            Kind::AluImm,
        ];
        kinds.sort();
        assert_eq!(
            kinds.into_iter().map(Kind::as_str).collect::<Vec<_>>(),
            ["ld", "mv", "not", "op", "opi", "rr", "st"]
        );
        assert_eq!(Kind::AluImm.to_string(), "opi");
    }

    #[test]
    fn omf_source_nodes_decoded_defaults_and_fixups_match_lift() {
        let decoded = Decoded::new(Kind::Load, 1, 0, 3);
        assert_eq!(decoded.kind, Kind::Load);
        assert_eq!(decoded.pair, 1);
        assert_eq!(decoded.half, 0);
        assert_eq!(decoded.length, 3);
        assert_eq!(decoded.src_pair, 0);
        assert_eq!(decoded.alu, None);
        assert_eq!(decoded.mem, None);
        assert_eq!(decoded.dlen, 0);
        assert_eq!(decoded.disp_at, None);
        assert_eq!(decoded.imm, None);
        assert_eq!(FIXUP.get(&0).unwrap(), &[0x66, 0x50, 0x58, 0x5A]);
        assert_eq!(FIXUP.get(&1).unwrap(), &[0x66, 0x51, 0x59, 0x5B]);
    }
}
