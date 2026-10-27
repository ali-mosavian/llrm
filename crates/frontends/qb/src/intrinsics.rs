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
    /// A QuickrBASIC sized integer, named by its lowering.
    SizedInteger,
    Single,
    Double,
    Currency,
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
    /// A runtime routine of INTEGER arguments returning an INTEGER.
    RuntimeInteger(&'static str),
    CommandLine,
    HeapFree,
    FileQuery(&'static str),
    FileAttribute,
    Timer,
    Random,
    ToInteger,
    ToLong,
    ToIntegral {
        width: u8,
        signed: bool,
    },
    /// An f-string field's text.
    FormatField,
    ToSingle,
    ToDouble,
    PointerOffset,
    PointerSegment,
    VarptrString,
    StringAddress,
    StringSegment,
    ToCurrency,
    DateSerial,
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
    Pmap,
    Length,
    LowerBound,
    UpperBound,
    Asc,
    Val,
    RuntimeString(&'static str),
    Character,
    Mid,
    Left,
    Right,
    Instr,
    InputText,
    StringFill,
    Space,
    Environ,
    Directory,
    StringNumber,
    PackInteger,
    PackLong,
    PackSingle,
    PackDouble,
    PackCurrency,
    UnpackInteger,
    UnpackLong,
    UnpackSingle,
    UnpackDouble,
    UnpackCurrency,
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
    pub fn accepts(
        self,
        arity: usize,
    ) -> bool {
        (self.min_arity..=self.max_arity).contains(&arity)
    }

    pub fn available_in(
        self,
        dialect: Dialect,
    ) -> bool {
        self.dialects & dialect_bit(dialect) != 0
    }
}

const QBASIC: u8 = 1 << 0;
const QB45: u8 = 1 << 1;
const PDS: u8 = 1 << 2;
const VBDOS: u8 = 1 << 3;
const QUICKR: u8 = 1 << 4;
const ALL: u8 = QBASIC | QB45 | PDS | VBDOS | QUICKR;

const fn dialect_bit(dialect: Dialect) -> u8 {
    match dialect {
        Dialect::QBasic11 => QBASIC,
        Dialect::QuickBasic45 => QB45,
        Dialect::Pds71 => PDS,
        Dialect::VbDos => VBDOS,
        Dialect::Quickr => QUICKR,
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

/// A conversion to a QuickrBASIC sized integer.
macro_rules! sized_conversion {
    ($name:literal, $width:literal, $signed:literal) => {
        Intrinsic {
            name: $name,
            min_arity: 1,
            max_arity: 1,
            result: ResultClass::SizedInteger,
            effect: Effect::Pure,
            lowering: Lowering::ToIntegral { width: $width, signed: $signed },
            dialects: QUICKR,
        }
    };
}

pub static INTRINSICS: &[Intrinsic] = &[
    Intrinsic {
        name: crate::front::quickr::FORMAT_FIELD,
        min_arity: 1,
        max_arity: 2,
        result: ResultClass::String,
        effect: Effect::Runtime,
        lowering: Lowering::FormatField,
        dialects: QUICKR,
    },
    intrinsic!("ABS", 1..=1, DynamicNumeric, Pure, Lowering::Abs),
    intrinsic!("ASC", 1..=1, Integer, Runtime, Lowering::Asc),
    intrinsic!("ATN", 1..=1, DynamicNumeric, Pure, Lowering::Atan),
    sized_conversion!("CBYTE", 1, true),
    intrinsic!("CDBL", 1..=1, Double, Pure, Lowering::ToDouble),
    intrinsic!("CHR", 1..=1, String, Runtime, Lowering::Character),
    intrinsic!("CINT", 1..=1, Integer, Pure, Lowering::ToInteger),
    intrinsic!("CLNG", 1..=1, Long, Pure, Lowering::ToLong),
    intrinsic!("DATE", 0..=0, String, Runtime, Lowering::RuntimeString("B$FDAT")),
    intrinsic!("TIME", 0..=0, String, Runtime, Lowering::RuntimeString("B$FTIM")),
    intrinsic!("COMMAND", 0..=0, String, Runtime, Lowering::CommandLine),
    intrinsic!("COS", 1..=1, DynamicNumeric, Pure, Lowering::Cos),
    intrinsic!("CSNG", 1..=1, Single, Pure, Lowering::ToSingle),
    sized_conversion!("CUBYTE", 1, false),
    sized_conversion!("CUINT", 2, false),
    sized_conversion!("CULNG", 4, false),
    intrinsic!("DIR", 1..=1, String, Runtime, Lowering::Directory),
    intrinsic!("ENVIRON", 1..=1, String, Runtime, Lowering::Environ),
    intrinsic!("CSRLIN", 0..=0, Integer, Runtime, Lowering::RuntimeInteger("B$CSRL")),
    intrinsic!("EOF", 1..=1, Integer, Runtime, Lowering::RuntimeInteger("B$FEOF")),
    intrinsic!("ERL", 0..=0, Long, Runtime, Lowering::ErrorLine),
    intrinsic!("ERR", 0..=0, Integer, Runtime, Lowering::ErrorNumber),
    intrinsic!("EXP", 1..=1, DynamicNumeric, Pure, Lowering::Exp),
    intrinsic!("FIX", 1..=1, DynamicNumeric, Pure, Lowering::Truncate),
    intrinsic!("FREEFILE", 0..=0, Integer, Runtime, Lowering::RuntimeInteger("B$FREF")),
    intrinsic!("FRE", 1..=1, Long, Runtime, Lowering::HeapFree),
    intrinsic!("INT", 1..=1, DynamicNumeric, Pure, Lowering::Floor),
    intrinsic!("INKEY", 0..=0, String, Runtime, Lowering::RuntimeString("B$INKY")),
    intrinsic!("INPUT", 1..=2, String, Runtime, Lowering::InputText),
    intrinsic!("INSTR", 2..=3, Integer, Runtime, Lowering::Instr),
    intrinsic!("LCASE", 1..=1, String, Runtime, Lowering::RuntimeString("B$LCAS")),
    intrinsic!("LEFT", 2..=2, String, Runtime, Lowering::Left),
    intrinsic!("LEN", 1..=1, Integer, Runtime, Lowering::Length),
    intrinsic!("LBOUND", 1..=2, Integer, Runtime, Lowering::LowerBound),
    intrinsic!("LOC", 1..=1, Long, Runtime, Lowering::FileQuery("B$FLOC")),
    intrinsic!("LOF", 1..=1, Long, Runtime, Lowering::FileQuery("B$FLOF")),
    intrinsic!("FILEATTR", 2..=2, Long, Runtime, Lowering::FileAttribute),
    intrinsic!("LOG", 1..=1, DynamicNumeric, Pure, Lowering::Log),
    intrinsic!("LTRIM", 1..=1, String, Runtime, Lowering::RuntimeString("B$LTRM")),
    intrinsic!("SEEK", 1..=1, Long, Runtime, Lowering::FileQuery("B$FSEK")),
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
    Intrinsic {
        name: "CVC",
        min_arity: 1,
        max_arity: 1,
        result: ResultClass::Currency,
        effect: Effect::Runtime,
        lowering: Lowering::UnpackCurrency,
        dialects: PDS | VBDOS,
    },
    Intrinsic {
        name: "MKC",
        min_arity: 1,
        max_arity: 1,
        result: ResultClass::String,
        effect: Effect::Runtime,
        lowering: Lowering::PackCurrency,
        dialects: PDS | VBDOS,
    },
    intrinsic!("OCT", 1..=1, String, Runtime, Lowering::RadixText("B$FOCT")),
    intrinsic!("PEEK", 1..=1, Integer, ReadsMemory, Lowering::Peek),
    intrinsic!("INP", 1..=1, Integer, Device, Lowering::PortIn),
    intrinsic!("POINT", 2..=2, Integer, Runtime, Lowering::Point),
    intrinsic!("PMAP", 2..=2, Single, Runtime, Lowering::Pmap),
    intrinsic!("STICK", 1..=1, Integer, Runtime, Lowering::RuntimeInteger("B$STIK")),
    intrinsic!("STRIG", 1..=1, Integer, Runtime, Lowering::RuntimeInteger("B$FSTG")),
    intrinsic!("PEN", 1..=1, Integer, Runtime, Lowering::RuntimeInteger("B$FPEN")),
    intrinsic!("$VARPTR", 1..=1, String, Runtime, Lowering::VarptrString),
    intrinsic!("SADD", 1..=1, Integer, Pure, Lowering::StringAddress),
    Intrinsic {
        name: "DATESERIAL",
        min_arity: 3,
        max_arity: 3,
        result: ResultClass::Double,
        effect: Effect::Runtime,
        lowering: Lowering::DateSerial,
        dialects: VBDOS,
    },
    Intrinsic {
        name: "CCUR",
        min_arity: 1,
        max_arity: 1,
        result: ResultClass::Currency,
        effect: Effect::Pure,
        lowering: Lowering::ToCurrency,
        dialects: PDS | VBDOS,
    },
    Intrinsic {
        name: "SSEG",
        min_arity: 1,
        max_arity: 1,
        result: ResultClass::Integer,
        effect: Effect::Pure,
        lowering: Lowering::StringSegment,
        dialects: PDS | VBDOS,
    },
    intrinsic!("PLAY", 1..=1, Integer, Runtime, Lowering::RuntimeInteger("B$FPLY")),
    intrinsic!("POS", 1..=1, Integer, Runtime, Lowering::RuntimeInteger("B$FPOS")),
    // The event checks the front end writes (never a program's: `$` cannot
    // start a name).
    intrinsic!("$EVANY", 0..=0, Integer, Runtime, Lowering::RuntimeInteger("B$EVAN")),
    intrinsic!("$EVPD", 1..=1, Integer, Runtime, Lowering::RuntimeInteger("B$EVPD")),
    intrinsic!("RTRIM", 1..=1, String, Runtime, Lowering::RuntimeString("B$RTRM")),
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
    intrinsic!("UCASE", 1..=1, String, Runtime, Lowering::RuntimeString("B$UCAS")),
    intrinsic!("VAL", 1..=1, Double, Runtime, Lowering::Val),
    intrinsic!("VARPTR", 1..=1, Integer, Pure, Lowering::PointerOffset),
    intrinsic!("VARSEG", 1..=1, Integer, Pure, Lowering::PointerSegment),
];

/// Where an argument is out of a BC math routine's range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Range {
    AtLeast,
    AtMost,
    AbsAbove,
}

/// What BC 4.5's routine for an x87 sequence does to an argument out of its
/// range: ERROR `error` instead of the value or an INF/IND the x87 would give.
/// Measured under BC with ON ERROR, for SINGLE and DOUBLE alike (EXP, LOG,
/// SIN, COS, TAN; `^` through its exp2).
pub struct ArgumentCheck {
    pub routine: &'static str,
    pub range: Range,
    pub bound: &'static str,
    pub error: i64,
}

pub const ARGUMENT_CHECKS: [ArgumentCheck; 4] = [
    // exp2 of what EXP(x) and x ^ y feed it: the integer part of the
    // argument is a 16-bit INTEGER, so 32767.5 and up is Overflow
    // (EXP(22712.6) is INF, EXP(22712.8) is error 6; negative gives 0).
    ArgumentCheck { routine: "fexp2", range: Range::AtLeast, bound: "32767.5", error: 6 },
    // LOG(0), LOG(-0) and LOG of a negative: Illegal function call.
    ArgumentCheck { routine: "fln", range: Range::AtMost, bound: "0", error: 5 },
    // EXP(x) is exp2(x * log2(e)): the same 32767.5, as x.
    ArgumentCheck { routine: "fexp", range: Range::AtLeast, bound: "22712.700238998008", error: 6 },
    // SIN, COS and TAN: |x| above 2^31 (2147483648 is fine, 2147483648.5 is
    // not).
    ArgumentCheck { routine: "fsin", range: Range::AbsAbove, bound: "2147483648", error: 5 },
];

pub fn argument_check(routine: &str) -> Option<&'static ArgumentCheck> {
    ARGUMENT_CHECKS.iter().find(|check| check.routine == routine)
}

pub fn find(
    name: &str,
    dialect: Dialect,
) -> Option<&'static Intrinsic> {
    INTRINSICS.iter().find(|intrinsic| intrinsic.name == name && intrinsic.available_in(dialect))
}

/// Every function keyword, as spelled, with the dialects it is one in: QB
/// 4.5's from its help index
/// (tests/differential/conformance/qb45/coverage.toml), then PDS 7.1's
/// and VBDOS's own additions. FINANCE.LIB's functions are a library's.
pub static KEYWORDS: &[(&str, u8)] = &[
    ("ABS", ALL),
    ("ASC", ALL),
    ("ATN", ALL),
    ("CDBL", ALL),
    ("CHR$", ALL),
    ("CINT", ALL),
    ("CLNG", ALL),
    ("COMMAND$", ALL),
    ("COS", ALL),
    ("CSNG", ALL),
    ("CSRLIN", ALL),
    ("CVD", ALL),
    ("CVDMBF", ALL),
    ("CVI", ALL),
    ("CVL", ALL),
    ("CVS", ALL),
    ("CVSMBF", ALL),
    ("DATE$", ALL),
    ("ENVIRON$", ALL),
    ("EOF", ALL),
    ("ERDEV", ALL),
    ("ERDEV$", ALL),
    ("ERL", ALL),
    ("ERR", ALL),
    ("EXP", ALL),
    ("FILEATTR", ALL),
    ("FIX", ALL),
    ("FRE", ALL),
    ("FREEFILE", ALL),
    ("HEX$", ALL),
    ("INKEY$", ALL),
    ("INP", ALL),
    ("INPUT$", ALL),
    ("INSTR", ALL),
    ("INT", ALL),
    ("IOCTL$", ALL),
    ("LBOUND", ALL),
    ("LCASE$", ALL),
    ("LEFT$", ALL),
    ("LEN", ALL),
    ("LOC", ALL),
    ("LOF", ALL),
    ("LOG", ALL),
    ("LPOS", ALL),
    ("LTRIM$", ALL),
    ("MID$", ALL),
    ("MKD$", ALL),
    ("MKDMBF$", ALL),
    ("MKI$", ALL),
    ("MKL$", ALL),
    ("MKS$", ALL),
    ("MKSMBF$", ALL),
    ("OCT$", ALL),
    ("PEEK", ALL),
    ("PEN", ALL),
    ("PLAY", ALL),
    ("PMAP", ALL),
    ("POINT", ALL),
    ("POS", ALL),
    ("RIGHT$", ALL),
    ("RND", ALL),
    ("RTRIM$", ALL),
    ("SADD", ALL),
    ("SCREEN", ALL),
    ("SEEK", ALL),
    ("SETMEM", ALL),
    ("SGN", ALL),
    ("SIN", ALL),
    ("SPACE$", ALL),
    ("SPC", ALL),
    ("SQR", ALL),
    ("STICK", ALL),
    ("STR$", ALL),
    ("STRIG", ALL),
    ("STRING$", ALL),
    ("TAB", ALL),
    ("TAN", ALL),
    ("TIME$", ALL),
    ("TIMER", ALL),
    ("UBOUND", ALL),
    ("UCASE$", ALL),
    ("VAL", ALL),
    ("VARPTR", ALL),
    ("VARPTR$", ALL),
    ("VARSEG", ALL),
    ("CCUR", PDS | VBDOS),
    ("CURDIR$", PDS | VBDOS),
    ("DIR$", PDS | VBDOS),
    ("SSEG", PDS | VBDOS),
    ("SSEGADD", PDS | VBDOS),
    ("STACK", PDS | VBDOS),
    ("DATESERIAL", VBDOS),
    ("DATEVALUE", VBDOS),
    ("DOEVENTS", VBDOS),
    ("INPUTBOX$", VBDOS),
    ("MSGBOX", VBDOS),
    ("TIMESERIAL", VBDOS),
    ("TIMEVALUE", VBDOS),
];

/// Whether `name`, as spelled, is a function keyword of `dialect` the table
/// does not provide: a program naming one must be refused, never read as
/// an implicit variable.
pub fn unsupported(
    name: &str,
    dialect: Dialect,
) -> bool {
    let name = name.to_ascii_uppercase();
    let keyword = KEYWORDS.iter().any(|&(one, dialects)| one == name && dialects & dialect_bit(dialect) != 0);
    // A string intrinsic is spelled with its `$`.
    let spelled = |intrinsic: &Intrinsic| {
        if intrinsic.result == ResultClass::String { format!("{}$", intrinsic.name) } else { intrinsic.name.to_owned() }
    };
    keyword && !INTRINSICS.iter().any(|one| one.available_in(dialect) && spelled(one) == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every function QB 4.5's help index names is a keyword here: one the
    /// list lacked would read as an implicit variable.
    #[test]
    fn keywords_hold_every_function_of_qb45s_help() {
        let index = include_str!("../../../../tests/differential/conformance/qb45/coverage.toml");
        let titles = index.lines().filter_map(|line| line.strip_prefix("title = \"")?.strip_suffix('"'));
        let mut missing = Vec::new();
        for title in
            titles.filter(|one| one.ends_with("Function QuickSCREEN") || one.ends_with("Functions QuickSCREEN"))
        {
            let names = title
                .trim_end_matches("QuickSCREEN")
                .trim_end()
                .trim_end_matches("Functions")
                .trim_end_matches("Function");
            for name in names.split([',', ' ']).filter(|one| !one.is_empty() && *one != "and") {
                let name = name.trim_end_matches("(n)");
                if !KEYWORDS.iter().any(|&(one, dialects)| one == name && dialects & QB45 != 0) {
                    missing.push(name.to_owned());
                }
            }
        }
        assert!(missing.is_empty(), "{missing:?}");
    }

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
