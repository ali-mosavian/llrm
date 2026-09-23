//! Declarative catalogue of QB-family intrinsic functions.
//!
//! Name recognition belongs here.  Semantic lowering remains code because
//! conversions such as `INT` and descriptor-producing functions such as
//! `MID$` are algorithms, not ABI declarations.

use crate::dialect::Dialect;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultClass {
    DynamicNumeric,
    Integer,
    Long,
    Single,
    Double,
    String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Pure,
    ReadsMemory,
    /// Reads a device port: observable, never repeated or dropped.
    Device,
    Runtime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lowering {
    ErrorNumber,
    ErrorLine,
    FreeFile,
    CommandLine,
    HeapFree,
    FileLength,
    Timer,
    Random,
    ToInteger,
    ToLong,
    ToSingle,
    ToDouble,
    PointerOffset,
    PointerSegment,
    Abs,
    Sqrt,
    Sign,
    Sin,
    Cos,
    Tan,
    Atan,
    Log,
    Exp,
    Floor,
    Truncate,
    Peek,
    PortIn,
    Point,
    Length,
    LowerBound,
    UpperBound,
    Asc,
    Val,
    Eof,
    RuntimeString(&'static str),
    Character,
    Mid,
    Left,
    Right,
    Instr,
    StringFill,
    Space,
    Environ,
    Directory,
    StringNumber,
    PackInteger,
    PackLong,
    PackSingle,
    PackDouble,
    UnpackInteger,
    UnpackLong,
    UnpackSingle,
    UnpackDouble,
    RadixText(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Intrinsic {
    pub name: &'static str,
    pub min_arity: usize,
    pub max_arity: usize,
    pub result: ResultClass,
    pub effect: Effect,
    pub lowering: Lowering,
    dialects: u8,
}

impl Intrinsic {
    pub fn accepts(self, arity: usize) -> bool {
        (self.min_arity..=self.max_arity).contains(&arity)
    }

    pub fn available_in(self, dialect: Dialect) -> bool {
        self.dialects & dialect_bit(dialect) != 0
    }
}

const QBASIC: u8 = 1 << 0;
const QB45: u8 = 1 << 1;
const PDS: u8 = 1 << 2;
const VBDOS: u8 = 1 << 3;
const ALL: u8 = QBASIC | QB45 | PDS | VBDOS;

const fn dialect_bit(dialect: Dialect) -> u8 {
    match dialect {
        Dialect::QBasic11 => QBASIC,
        Dialect::QuickBasic45 => QB45,
        Dialect::Pds71 => PDS,
        Dialect::VbDos => VBDOS,
    }
}

macro_rules! intrinsic {
    ($name:literal, $min:literal ..= $max:literal, $result:ident, $effect:ident, $lowering:expr) => {
        Intrinsic {
            name: $name,
            min_arity: $min,
            max_arity: $max,
            result: ResultClass::$result,
            effect: Effect::$effect,
            lowering: $lowering,
            dialects: ALL,
        }
    };
}

pub static INTRINSICS: &[Intrinsic] = &[
    intrinsic!("ABS", 1..=1, DynamicNumeric, Pure, Lowering::Abs),
    intrinsic!("ASC", 1..=1, Integer, Runtime, Lowering::Asc),
    intrinsic!("ATN", 1..=1, DynamicNumeric, Pure, Lowering::Atan),
    intrinsic!("CDBL", 1..=1, Double, Pure, Lowering::ToDouble),
    intrinsic!("CHR", 1..=1, String, Runtime, Lowering::Character),
    intrinsic!("CINT", 1..=1, Integer, Pure, Lowering::ToInteger),
    intrinsic!("CLNG", 1..=1, Long, Pure, Lowering::ToLong),
    intrinsic!("COMMAND", 0..=0, String, Runtime, Lowering::CommandLine),
    intrinsic!("COS", 1..=1, DynamicNumeric, Pure, Lowering::Cos),
    intrinsic!("CSNG", 1..=1, Single, Pure, Lowering::ToSingle),
    intrinsic!("DIR", 1..=1, String, Runtime, Lowering::Directory),
    intrinsic!("ENVIRON", 1..=1, String, Runtime, Lowering::Environ),
    intrinsic!("EOF", 1..=1, Integer, Runtime, Lowering::Eof),
    intrinsic!("ERL", 0..=0, Long, Runtime, Lowering::ErrorLine),
    intrinsic!("ERR", 0..=0, Integer, Runtime, Lowering::ErrorNumber),
    intrinsic!("EXP", 1..=1, DynamicNumeric, Pure, Lowering::Exp),
    intrinsic!("FIX", 1..=1, DynamicNumeric, Pure, Lowering::Truncate),
    intrinsic!("FREEFILE", 0..=0, Integer, Runtime, Lowering::FreeFile),
    intrinsic!("FRE", 1..=1, Long, Runtime, Lowering::HeapFree),
    intrinsic!("INT", 1..=1, DynamicNumeric, Pure, Lowering::Floor),
    intrinsic!(
        "INKEY",
        0..=0,
        String,
        Runtime,
        Lowering::RuntimeString("B$INKY")
    ),
    intrinsic!("INSTR", 2..=3, Integer, Runtime, Lowering::Instr),
    intrinsic!(
        "LCASE",
        1..=1,
        String,
        Runtime,
        Lowering::RuntimeString("B$LCAS")
    ),
    intrinsic!("LEFT", 2..=2, String, Runtime, Lowering::Left),
    intrinsic!("LEN", 1..=1, Integer, Runtime, Lowering::Length),
    intrinsic!("LBOUND", 1..=2, Integer, Runtime, Lowering::LowerBound),
    intrinsic!("LOF", 1..=1, Long, Runtime, Lowering::FileLength),
    intrinsic!("LOG", 1..=1, DynamicNumeric, Pure, Lowering::Log),
    intrinsic!(
        "LTRIM",
        1..=1,
        String,
        Runtime,
        Lowering::RuntimeString("B$LTRM")
    ),
    intrinsic!("MID", 2..=3, String, Runtime, Lowering::Mid),
    intrinsic!("CVI", 1..=1, Integer, Runtime, Lowering::UnpackInteger),
    intrinsic!("CVL", 1..=1, Long, Runtime, Lowering::UnpackLong),
    intrinsic!("CVS", 1..=1, Single, Runtime, Lowering::UnpackSingle),
    intrinsic!("CVD", 1..=1, Double, Runtime, Lowering::UnpackDouble),
    intrinsic!("HEX", 1..=1, String, Runtime, Lowering::RadixText("B$FHEX")),
    intrinsic!("MKI", 1..=1, String, Runtime, Lowering::PackInteger),
    intrinsic!("MKL", 1..=1, String, Runtime, Lowering::PackLong),
    intrinsic!("MKS", 1..=1, String, Runtime, Lowering::PackSingle),
    intrinsic!("MKD", 1..=1, String, Runtime, Lowering::PackDouble),
    intrinsic!("OCT", 1..=1, String, Runtime, Lowering::RadixText("B$FOCT")),
    intrinsic!("PEEK", 1..=1, Integer, ReadsMemory, Lowering::Peek),
    intrinsic!("INP", 1..=1, Integer, Device, Lowering::PortIn),
    intrinsic!("POINT", 2..=2, Integer, Runtime, Lowering::Point),
    intrinsic!(
        "RTRIM",
        1..=1,
        String,
        Runtime,
        Lowering::RuntimeString("B$RTRM")
    ),
    intrinsic!("RIGHT", 2..=2, String, Runtime, Lowering::Right),
    intrinsic!("RND", 0..=1, Single, Runtime, Lowering::Random),
    intrinsic!("SGN", 1..=1, DynamicNumeric, Pure, Lowering::Sign),
    intrinsic!("SIN", 1..=1, DynamicNumeric, Pure, Lowering::Sin),
    intrinsic!("SQR", 1..=1, DynamicNumeric, Pure, Lowering::Sqrt),
    intrinsic!("SPACE", 1..=1, String, Runtime, Lowering::Space),
    intrinsic!("STR", 1..=1, String, Runtime, Lowering::StringNumber),
    intrinsic!("STRING", 2..=2, String, Runtime, Lowering::StringFill),
    intrinsic!("TAN", 1..=1, DynamicNumeric, Pure, Lowering::Tan),
    intrinsic!("TIMER", 0..=0, Single, Runtime, Lowering::Timer),
    intrinsic!("UBOUND", 1..=2, Integer, Runtime, Lowering::UpperBound),
    intrinsic!(
        "UCASE",
        1..=1,
        String,
        Runtime,
        Lowering::RuntimeString("B$UCAS")
    ),
    intrinsic!("VAL", 1..=1, Double, Runtime, Lowering::Val),
    intrinsic!("VARPTR", 1..=1, Integer, Pure, Lowering::PointerOffset),
    intrinsic!("VARSEG", 1..=1, Integer, Pure, Lowering::PointerSegment),
];

pub fn find(name: &str, dialect: Dialect) -> Option<&'static Intrinsic> {
    INTRINSICS
        .iter()
        .find(|intrinsic| intrinsic.name == name && intrinsic.available_in(dialect))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_owns_intrinsic_variant_identity() {
        let lowering = |name| find(name, Dialect::VbDos).unwrap().lowering;
        assert_eq!(lowering("SIN"), Lowering::Sin);
        assert_eq!(lowering("COS"), Lowering::Cos);
        assert_eq!(lowering("TAN"), Lowering::Tan);
        assert_eq!(lowering("VARPTR"), Lowering::PointerOffset);
        assert_eq!(lowering("VARSEG"), Lowering::PointerSegment);
        assert_eq!(lowering("INT"), Lowering::Floor);
        assert_eq!(lowering("FIX"), Lowering::Truncate);
    }
}
