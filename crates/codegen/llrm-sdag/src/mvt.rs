//! Value types of the DAG: [`MVT`] (the simple ones) and [`EVT`] (also an
//! integer of any width, as `i24` before promotion).

/// A simple value type, the pin's `MVT` without vectors (DEVIATE SD13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MVT {
    I1,
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,
    F80,
    /// A far pointer, a selector and an offset (DEVIATE SD2): m16 only, the
    /// type legalizer expands it into two `i16`.
    Far,
    /// Chains (`MVT::Other`).
    Other,
    /// Glue (`MVT::Glue`): never CSE'd.
    Glue,
}

impl MVT {
    /// The simple integer type of `bits`, if there is one.
    pub fn integer(bits: u32) -> Option<MVT> {
        Some(match bits {
            1 => MVT::I1,
            8 => MVT::I8,
            16 => MVT::I16,
            32 => MVT::I32,
            64 => MVT::I64,
            128 => MVT::I128,
            _ => return None,
        })
    }
}

/// A value type: a simple one, or an integer of a width with no simple type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EVT {
    Simple(MVT),
    Int(u32),
}

impl From<MVT> for EVT {
    fn from(vt: MVT) -> EVT {
        EVT::Simple(vt)
    }
}

impl EVT {
    pub const OTHER: EVT = EVT::Simple(MVT::Other);
    pub const GLUE: EVT = EVT::Simple(MVT::Glue);
    pub const I1: EVT = EVT::Simple(MVT::I1);
    pub const I8: EVT = EVT::Simple(MVT::I8);
    pub const I16: EVT = EVT::Simple(MVT::I16);
    pub const I32: EVT = EVT::Simple(MVT::I32);
    pub const I64: EVT = EVT::Simple(MVT::I64);

    /// `EVT::getIntegerVT`: the simple type when the width has one, so equal
    /// types compare equal.
    pub fn integer(bits: u32) -> EVT {
        assert!((1..=128).contains(&bits), "integer width {bits}");
        MVT::integer(bits).map_or(EVT::Int(bits), EVT::Simple)
    }

    pub fn is_integer(self) -> bool {
        match self {
            EVT::Int(_) => true,
            EVT::Simple(vt) => matches!(
                vt,
                MVT::I1 | MVT::I8 | MVT::I16 | MVT::I32 | MVT::I64 | MVT::I128
            ),
        }
    }

    pub fn is_floating_point(self) -> bool {
        matches!(self, EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80))
    }

    /// `getSizeInBits`; a chain or glue has none.
    pub fn size_in_bits(self) -> u32 {
        match self {
            EVT::Int(bits) => bits,
            EVT::Simple(vt) => match vt {
                MVT::I1 => 1,
                MVT::I8 => 8,
                MVT::I16 => 16,
                MVT::I32 | MVT::F32 | MVT::Far => 32,
                MVT::I64 | MVT::F64 => 64,
                MVT::F80 => 80,
                MVT::I128 => 128,
                MVT::Other | MVT::Glue => panic!("{vt:?} has no size"),
            },
        }
    }

    pub fn bits_lt(
        self,
        other: EVT,
    ) -> bool {
        self.size_in_bits() < other.size_in_bits()
    }

    pub fn bits_le(
        self,
        other: EVT,
    ) -> bool {
        self.size_in_bits() <= other.size_in_bits()
    }

    pub fn bits_gt(
        self,
        other: EVT,
    ) -> bool {
        self.size_in_bits() > other.size_in_bits()
    }
}
