//! Value types of the DAG: [`MVT`] (the simple ones) and [`EVT`] (also an
//! integer of any width, as `i24` before promotion).

/// A simple value type, the pin's `MVT` with the vectors of integers and of
/// `f32`/`f64` that are 128 bits or wider, and `v2f32` (DEVIATE SD13: no
/// 64-bit integer vector, no `v1i64`; vector POC, owner m64).
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
    /// `v16i8`, `v8i16`, `v4i32`, `v2i64`: the 128-bit integer vectors an SSE
    /// register holds.
    V16I8,
    V8I16,
    V4I32,
    V2I64,
    /// `v4f32`, `v2f64`: the 128-bit float vectors (arm64, `addQRTypeForNEON`).
    V4F32,
    V2F64,
    /// `v2f32`: the 64-bit float vector `FPR64` holds (`addDRTypeForNEON`).
    V2F32,
    /// `v8i8`, `v4i16`, `v2i32`: the 64-bit integer vectors `FPR64` holds.
    V8I8,
    V4I16,
    V2I32,
    /// The 256-bit vectors no register holds: the type legalizer splits each
    /// in two (`TypeSplitVector`).
    V32I8,
    V16I16,
    V8I32,
    V4I64,
    V8F32,
    V4F64,
    /// The 512-bit and 1024-bit vectors of the pin's `MVT`: split down to
    /// the registers' width.
    V64I8,
    V32I16,
    V16I32,
    V8I64,
    V16F32,
    V8F64,
    V128I8,
    V64I16,
    V32I32,
    V16I64,
    V32F32,
    V16F64,
    /// IEEE binary128: never legal, softened to an `i128` (DEVIATE: the pin
    /// keeps it with the other floats).
    F128,
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

    /// `MVT::getVectorVT`: the vector of `count` lanes of `element`, if there
    /// is one.
    pub fn vector(
        element: MVT,
        count: u32,
    ) -> Option<MVT> {
        VECTORS.iter().find(|&&(_, lane, lanes)| lane == element && lanes == count).map(|&(vt, ..)| vt)
    }

    /// Every vector type.
    pub fn vectors() -> impl Iterator<Item = MVT> {
        VECTORS.iter().map(|&(vt, ..)| vt)
    }

    pub fn is_vector(self) -> bool {
        !matches!(
            self,
            MVT::I1
                | MVT::I8
                | MVT::I16
                | MVT::I32
                | MVT::I64
                | MVT::I128
                | MVT::F32
                | MVT::F64
                | MVT::F80
                | MVT::F128
                | MVT::Far
                | MVT::Other
                | MVT::Glue
        )
    }

    /// `getVectorElementType`.
    pub fn element(self) -> Option<MVT> {
        VECTORS.iter().find(|&&(vt, ..)| vt == self).map(|&(_, element, _)| element)
    }

    /// `getVectorNumElements`.
    pub fn lanes(self) -> Option<u32> {
        VECTORS.iter().find(|&&(vt, ..)| vt == self).map(|&(.., lanes)| lanes)
    }
}

/// Every vector type: its element and its lane count.
const VECTORS: [(MVT, MVT, u32); 28] = [
    (MVT::V16I8, MVT::I8, 16),
    (MVT::V8I16, MVT::I16, 8),
    (MVT::V4I32, MVT::I32, 4),
    (MVT::V2I64, MVT::I64, 2),
    (MVT::V4F32, MVT::F32, 4),
    (MVT::V2F64, MVT::F64, 2),
    (MVT::V2F32, MVT::F32, 2),
    (MVT::V8I8, MVT::I8, 8),
    (MVT::V4I16, MVT::I16, 4),
    (MVT::V2I32, MVT::I32, 2),
    (MVT::V32I8, MVT::I8, 32),
    (MVT::V16I16, MVT::I16, 16),
    (MVT::V8I32, MVT::I32, 8),
    (MVT::V4I64, MVT::I64, 4),
    (MVT::V8F32, MVT::F32, 8),
    (MVT::V4F64, MVT::F64, 4),
    (MVT::V64I8, MVT::I8, 64),
    (MVT::V32I16, MVT::I16, 32),
    (MVT::V16I32, MVT::I32, 16),
    (MVT::V8I64, MVT::I64, 8),
    (MVT::V16F32, MVT::F32, 16),
    (MVT::V8F64, MVT::F64, 8),
    (MVT::V128I8, MVT::I8, 128),
    (MVT::V64I16, MVT::I16, 64),
    (MVT::V32I32, MVT::I32, 32),
    (MVT::V16I64, MVT::I64, 16),
    (MVT::V32F32, MVT::F32, 32),
    (MVT::V16F64, MVT::F64, 16),
];

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
    pub const I128: EVT = EVT::Simple(MVT::I128);

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

    pub fn is_vector(self) -> bool {
        matches!(self, EVT::Simple(vt) if vt.is_vector())
    }

    /// `getVectorElementType`.
    pub fn element(self) -> EVT {
        match self {
            EVT::Simple(vt) => EVT::Simple(vt.element().expect("a vector type")),
            EVT::Int(_) => panic!("{self:?} is not a vector"),
        }
    }

    /// `getVectorNumElements`.
    pub fn lanes(self) -> u32 {
        match self {
            EVT::Simple(vt) => vt.lanes().expect("a vector type"),
            EVT::Int(_) => panic!("{self:?} is not a vector"),
        }
    }

    pub fn is_floating_point(self) -> bool {
        matches!(self, EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80 | MVT::F128))
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
                MVT::I128 | MVT::F128 => 128,
                MVT::Other | MVT::Glue => panic!("{vt:?} has no size"),
                vector => {
                    let (element, lanes) = vector.element().zip(vector.lanes()).expect("a vector type");
                    EVT::Simple(element).size_in_bits() * lanes
                }
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
