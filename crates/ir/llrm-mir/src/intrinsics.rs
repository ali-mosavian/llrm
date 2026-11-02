//! The LLVM intrinsics MIR uses, as `Intrinsics.td` states them: each one's
//! name, signature and attributes. The parser gives a declaration its
//! attributes, as LLVM does on creating it; the verifier checks its
//! signature; the interpreter runs it.

use crate::Operand;
use crate::context::{ConstantKind, Context, mask};
use crate::intrinsic_attrs;
use crate::module::Function;
use crate::opcode::{Attribute, BinaryOp};
use crate::types::{FloatKind, Type, TypeId, Types};

/// The constant length (or cell count) operand of a memory intrinsic: a size,
/// so unsigned at the width of its own type. Every reader of the operand asks
/// this one, none sign-extends it: a 16-bit 40000 is not -25536.
pub fn length(
    context: &Context,
    operand: Operand,
) -> Option<u64> {
    let Operand::Constant(id) = operand else { return None };
    let constant = context.get(id);
    let ConstantKind::Int(bits) = constant.kind else { return None };
    u64::try_from(bits & mask(context.types.int_bits(constant.ty)?)).ok()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intrinsic {
    /// `llvm.{s,u}{add,sub,mul}.with.overflow`: the wrapped result, and
    /// whether it overflowed.
    WithOverflow {
        op: BinaryOp,
        signed: bool,
    },
    /// `llvm.{s,u}{add,sub}.sat`: the result clamped to the type's range
    /// rather than wrapped.
    Saturating {
        op: BinaryOp,
        signed: bool,
    },
    /// `llvm.{s,u}{max,min}`.
    MinMax {
        signed: bool,
        max: bool,
    },
    /// `llvm.smul.fix` and `llvm.sdiv.fix`: signed fixed point with as many
    /// fraction bits as the third argument says.
    Fixed {
        divide: bool,
    },
    /// `llvm.umul.fix` and `llvm.{s,u}mul.fix.sat`: as `smul.fix`, unsigned
    /// or clamped to the range rather than wrapped.
    FixedMul {
        signed: bool,
        saturating: bool,
    },
    /// `llvm.{s,u}shl.sat`: the first argument shifted left by the second,
    /// clamped to the range rather than wrapped.
    ShiftSat {
        signed: bool,
    },
    /// `llvm.ctlz`, `llvm.cttz` and `llvm.ctpop`.
    BitCount(BitCount),
    /// `llvm.ptrmask`: the pointer with the bits the mask clears cleared.
    PtrMask,
    /// `llvm.bswap`: the bytes of its argument in the opposite order.
    ByteSwap,
    /// `llvm.bitreverse`: the bits of its argument in the opposite order.
    BitReverse,
    /// `llvm.fshl` and `llvm.fshr`: the concatenation of the first two
    /// arguments shifted by the third modulo the width, the high half
    /// (`fshl`) or the low half (`fshr`) kept. Both arguments the same value,
    /// a rotate.
    FunnelShift {
        left: bool,
    },
    /// `llvm.abs`: the absolute value; the second argument says whether the
    /// minimum value, which has none, is poison.
    Abs,
    /// `llvm.scmp` and `llvm.ucmp`: -1, 0 or 1 as the first argument is less
    /// than, equal to or greater than the second, in the result's integer type.
    Compare {
        signed: bool,
    },
    FMulAdd,
    /// `llvm.copysign`: the first argument with the second's sign.
    CopySign,
    /// `llvm.fabs`, `llvm.sqrt` and their kin: a function of one float.
    Unary(FloatFunction),
    /// Rounds to an integer as the rounding mode says: by default, to
    /// nearest, ties to even.
    LRint,
    /// `llvm.llrint`: `LRint` with a `long long` result, which a target
    /// without the instruction calls `llrint` for.
    LLRint,
    MemSet,
    /// `llvm.experimental.memset.pattern`: its first argument's cells, as many
    /// as the third says and as wide as the second, each become the second.
    MemSetPattern,
    /// `llvm.memcpy`: as many bytes from its second argument to its first as
    /// its third says, byte for byte. A byte the source never wrote stays
    /// only that byte undefined; the two do not overlap.
    MemCpy,
    /// `llvm.memmove`: as `memcpy`, where the two may overlap: every byte
    /// read is as it was before the call.
    MemMove,
    LifetimeStart,
    LifetimeEnd,
    /// `llvm.invariant.start`: from here the memory its second argument
    /// reaches, as far as the constant first one says (-1: the whole object),
    /// is never written, until the `invariant.end` of its result or the end of
    /// the program. Returns the pointer-typed scope that ends it. A pass may
    /// use it and no code runs it.
    InvariantStart,
    /// `llvm.invariant.end`: ends the scope its first argument, an
    /// `invariant.start`'s result, began, over the same size and memory.
    InvariantEnd,
    /// `llvm.assume`: its condition holds here, which a pass may use and no
    /// code checks. Undefined where it does not.
    Assume,
    /// `llvm.is.constant`: whether its argument is a constant once the
    /// optimizer is done, which `lower-constant-intrinsics` decides and
    /// removes (LowerConstantIntrinsics.cpp); no code runs it. Only the
    /// integer overload is known here.
    IsConstant,
    /// `llvm.objectsize`: the bytes from a pointer to the end of its object,
    /// or the unknown answer (all ones, zero where the minimum is asked),
    /// which `lower-constant-intrinsics` decides and removes; no code runs it.
    ObjectSize,
    /// `llvm.prefetch`: a hint to bring an address into the cache; no code is
    /// needed where the target has no prefetch instruction.
    Prefetch,
    /// `llvm.fake.use`: keeps its operands live for a debugger at `-Og`; no
    /// code runs it.
    FakeUse,
    /// `llvm.expect`: its first argument, which the second says it is expected
    /// to be: a hint `lower-expect` turns into branch weights and removes
    /// (LowerExpectIntrinsic.cpp); no code runs it.
    Expect,
    /// `llrm.noalias`: its argument, which it, and what is based on it, is
    /// reached by no pointer that is not, while anything based on it is used:
    /// C's `restrict`, said of a loop's pointers by the pass that tested that
    /// the bytes they reach do not meet. A pass may use it and no code runs it.
    Restrict,
    /// An I/O port's value, read: a target intrinsic, as `llvm.x86.*` are.
    PortIn,
    /// A value written to an I/O port.
    PortOut,
    /// The bytes between two pointers into one object, as a wrapping integer:
    /// a target's own, where a pointer's integer form is not its address.
    PtrDiff,
    /// The far pointer naming the same byte as its huge argument, its offset
    /// as low as the selector allows: a target's own window over the bytes
    /// from there that no displacement carries out of (`Machine::huge_window`).
    Window,
    /// `llvm.va_start`: the list its argument points to made to point at
    /// the calling function's first variadic argument.
    VaStart,
    /// `llvm.va_end`: the list its argument points to is no longer read. It
    /// does nothing where the list owns no resource.
    VaEnd,
    /// `llvm.va_copy`: the list the second argument points to, copied to the
    /// first.
    VaCopy,
    /// `llvm.stacksave`: the stack pointer, to give back to `stackrestore`.
    StackSave,
    /// `llvm.stackrestore`: the stack pointer `stacksave` returned made the
    /// stack pointer again, releasing what was allocated since.
    StackRestore,
    /// Inline machine code, laid down where it is called: its name carries
    /// the bytes, and each argument is a frame place whose displacement it
    /// reads at a byte offset the name gives. It answers what it leaves in
    /// dx:ax, and may read, write or clobber anything.
    Code,
    /// Inline assembly that declares its registers: its name carries the
    /// bytes and the 16-bit registers it reads, answers and changes, each
    /// argument going in one and each result coming out of one, in order.
    /// It has side effects, and its bytes jump nowhere outside themselves.
    Asm,
}

/// Metadata on a `llvm.memmove` call saying which way its copy may run, as the
/// pass that made it proved: ascending addresses, or descending. Without
/// either, lowering compares the two pointers itself.
pub const FORWARD: &str = "llrm.forward";
pub const BACKWARD: &str = "llrm.backward";

/// What names inline code: `llrm.ia16.code.<hex bytes>` and, per argument,
/// `.<offset>` of the word that takes its displacement, `p<n>` or `m<n>`
/// added.
const CODE: &str = "llrm.ia16.code.";

/// Inline code's name: `bytes`, and each argument's word offset and addend.
pub fn code_name(
    bytes: &[u8],
    places: &[(usize, i64)],
) -> String {
    let hex: String = bytes.iter().map(|one| format!("{one:02x}")).collect();
    let places: String = places
        .iter()
        .map(|(at, addend)| format!(".{at}{}{}", if *addend < 0 { 'm' } else { 'p' }, addend.unsigned_abs()))
        .collect();
    format!("{CODE}{hex}{places}")
}

/// Inline code's bytes, and each argument's word offset and addend.
pub fn code(name: &str) -> Option<(Vec<u8>, Vec<(usize, i64)>)> {
    let mut parts = name.strip_prefix(CODE)?.split('.');
    let hex = parts.next()?;
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    let places = parts
        .map(|part| {
            let split = part.find(['p', 'm'])?;
            let (at, addend) = (part[..split].parse::<usize>().ok()?, part[split + 1..].parse::<i64>().ok()?);
            (at + 2 <= bytes.len()).then_some((at, if part.as_bytes()[split] == b'm' { -addend } else { addend }))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((bytes, places))
}

/// What names inline assembly with declared registers:
/// `llrm.ia16.asm.<hex bytes>.<in>.<out>.<clobbers>.<m|n>` and, per word naming
/// a symbol, `.f<at>_<kind>_<addend>`; each list its registers joined by `_`
/// (`-` for none), `m` where it reaches memory. The symbols' addresses are the
/// call's first arguments, in order, before the registers'.
const ASM: &str = "llrm.ia16.asm.";

/// A word of an inline block that names a symbol: where, as what (`offset`,
/// `segment`, `reloff`), and plus what.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsmFixup {
    pub at: usize,
    pub kind: String,
    pub addend: i64,
}

/// An inline assembly block: registers by their 16-bit names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsmBlock {
    pub code: Vec<u8>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub clobbers: Vec<String>,
    pub memory: bool,
    pub fixups: Vec<AsmFixup>,
}

pub fn asm_name(block: &AsmBlock) -> String {
    let hex: String = block.code.iter().map(|one| format!("{one:02x}")).collect();
    let list = |names: &[String]| if names.is_empty() { "-".to_owned() } else { names.join("_") };
    let fixups: String = block.fixups.iter().map(|one| format!(".f{}_{}_{}", one.at, one.kind, one.addend)).collect();
    format!(
        "{ASM}{hex}.{}.{}.{}.{}{fixups}",
        list(&block.inputs),
        list(&block.outputs),
        list(&block.clobbers),
        if block.memory { "m" } else { "n" }
    )
}

pub fn asm(name: &str) -> Option<AsmBlock> {
    let parts = name.strip_prefix(ASM)?.split('.').collect::<Vec<_>>();
    let [hex, inputs, outputs, clobbers, memory, fixups @ ..] = &parts[..] else {
        return None;
    };
    let (hex, inputs, outputs, clobbers, memory) = (*hex, *inputs, *outputs, *clobbers, *memory);
    if hex.len() % 2 != 0 {
        return None;
    }
    let code = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    let list = |names: &str| if names == "-" { Vec::new() } else { names.split('_').map(str::to_owned).collect() };
    let memory = match memory {
        "m" => true,
        "n" => false,
        _ => return None,
    };
    let fixups = fixups
        .iter()
        .map(|one| {
            let mut fields = one.strip_prefix('f')?.splitn(3, '_');
            Some(AsmFixup {
                at: fields.next()?.parse().ok()?,
                kind: fields.next()?.to_owned(),
                addend: fields.next()?.parse().ok()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(AsmBlock { code, inputs: list(inputs), outputs: list(outputs), clobbers: list(clobbers), memory, fixups })
}

/// What `llvm.ctlz`, `llvm.cttz` and `llvm.ctpop` count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BitCount {
    LeadingZeros,
    TrailingZeros,
    Population,
}

impl BitCount {
    /// The count in the `width`-bit `bits`; a zero has `width` zeros.
    pub fn apply(
        self,
        bits: u128,
        width: u32,
    ) -> u128 {
        u128::from(match self {
            Self::LeadingZeros => crate::apint::countl_zero(bits, width),
            Self::TrailingZeros => crate::apint::countr_zero(bits, width),
            Self::Population => bits.count_ones(),
        })
    }

    /// Whether a zero argument is poison where the intrinsic's second
    /// argument says so: the two zero counts have one, `ctpop` has none.
    pub fn has_zero_poison_flag(self) -> bool {
        self != Self::Population
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatFunction {
    Fabs,
    Sqrt,
    Sin,
    Cos,
    Atan,
    Log2,
    Exp2,
    Log,
    Exp,
    /// Rounds to an integral float as the rounding mode says.
    Rint,
    Floor,
    Ceil,
    Trunc,
    /// Halfway cases away from zero.
    Round,
    /// Halfway cases to even.
    RoundEven,
    /// As `Rint`, without raising inexact.
    NearbyInt,
}

impl FloatFunction {
    pub fn apply(
        self,
        x: f64,
    ) -> f64 {
        match self {
            Self::Fabs => x.abs(),
            Self::Sqrt => x.sqrt(),
            Self::Sin => x.sin(),
            Self::Cos => x.cos(),
            Self::Atan => x.atan(),
            Self::Log2 => x.log2(),
            Self::Exp2 => x.exp2(),
            Self::Log => x.ln(),
            Self::Exp => x.exp(),
            Self::Rint | Self::RoundEven | Self::NearbyInt => x.round_ties_even(),
            Self::Floor => x.floor(),
            Self::Ceil => x.ceil(),
            Self::Trunc => x.trunc(),
            Self::Round => x.round(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Int,
    Float,
    Pointer,
}

#[derive(Clone, Copy)]
enum Slot {
    /// The nth overloaded type.
    Any(usize),
    /// `{ T, i1 }` of the nth overloaded type.
    WithFlag(usize),
    Int(u32),
    /// A pointer in address space 0, whatever the overload is.
    Ptr,
    Void,
}

struct Spec {
    name: &'static str,
    intrinsic: Intrinsic,
    overloads: &'static [Kind],
    returns: Slot,
    parameters: &'static [Slot],
    /// What MIR states itself: `Intrinsics.td` has no entry for the name,
    /// or MIR's signature is not its.
    own: Option<Own>,
}

#[derive(Clone, Copy)]
struct Own {
    attrs: &'static [&'static str],
    memory: &'static [(Option<&'static str>, &'static str)],
    parameters: &'static [&'static [&'static str]],
}

const PURE: &[&str] = &["nocallback", "nofree", "nosync", "nounwind", "speculatable", "willreturn"];
const NO_MEMORY: &[(Option<&str>, &str)] = &[(None, "none")];
/// Any memory: a device may start DMA. `Machine::port_touches_memory`
/// narrows a call to one that cannot.
const PORT_MEMORY: &[(Option<&str>, &str)] = &[(None, "readwrite")];
const PORT_ATTRS: &[&str] = &["nocallback", "nofree", "nounwind", "willreturn"];
const INTS: &[Slot] = &[Slot::Any(0), Slot::Any(0)];

const fn arithmetic(
    name: &'static str,
    intrinsic: Intrinsic,
    returns: Slot,
    parameters: &'static [Slot],
    kind: Kind,
) -> Spec {
    let overloads: &[Kind] = match kind {
        Kind::Float => &[Kind::Float],
        _ => &[Kind::Int],
    };
    Spec { name, intrinsic, overloads, returns, parameters, own: None }
}

const fn overflow(
    name: &'static str,
    op: BinaryOp,
    signed: bool,
) -> Spec {
    arithmetic(name, Intrinsic::WithOverflow { op, signed }, Slot::WithFlag(0), INTS, Kind::Int)
}

const fn fixed_mul(
    name: &'static str,
    signed: bool,
    saturating: bool,
) -> Spec {
    arithmetic(name, Intrinsic::FixedMul { signed, saturating }, Slot::Any(0), FIXED, Kind::Int)
}

const fn shift_sat(
    name: &'static str,
    signed: bool,
) -> Spec {
    arithmetic(name, Intrinsic::ShiftSat { signed }, Slot::Any(0), INTS, Kind::Int)
}

const fn saturating(
    name: &'static str,
    op: BinaryOp,
    signed: bool,
) -> Spec {
    arithmetic(name, Intrinsic::Saturating { op, signed }, Slot::Any(0), INTS, Kind::Int)
}

const FLOAT: &[Slot] = &[Slot::Any(0)];

const fn unary(
    name: &'static str,
    function: FloatFunction,
) -> Spec {
    arithmetic(name, Intrinsic::Unary(function), Slot::Any(0), FLOAT, Kind::Float)
}

const fn min_max(
    name: &'static str,
    signed: bool,
    max: bool,
) -> Spec {
    arithmetic(name, Intrinsic::MinMax { signed, max }, Slot::Any(0), INTS, Kind::Int)
}

const fn bit_count(
    name: &'static str,
    count: BitCount,
) -> Spec {
    let parameters: &[Slot] =
        if matches!(count, BitCount::Population) { &[Slot::Any(0)] } else { &[Slot::Any(0), Slot::Int(1)] };
    arithmetic(name, Intrinsic::BitCount(count), Slot::Any(0), parameters, Kind::Int)
}

const UNARY: &[Slot] = &[Slot::Any(0)];
const TERNARY: &[Slot] = &[Slot::Any(0), Slot::Any(0), Slot::Any(0)];
const ABS: &[Slot] = &[Slot::Any(0), Slot::Int(1)];

const fn compare(
    name: &'static str,
    signed: bool,
) -> Spec {
    Spec {
        name,
        intrinsic: Intrinsic::Compare { signed },
        overloads: &[Kind::Int, Kind::Int],
        returns: Slot::Any(0),
        parameters: &[Slot::Any(1), Slot::Any(1)],
        own: None,
    }
}

const FIXED: &[Slot] = &[Slot::Any(0), Slot::Any(0), Slot::Int(32)];

const LIFETIME: &[Slot] = &[Slot::Int(64), Slot::Any(0)];
const LIFETIME_PARAMETERS: &[&[&str]] = &[&["immarg"], &["nocapture"]];
const LIFETIME_ATTRS: &[&str] = &["nocallback", "nofree", "nosync", "nounwind", "willreturn"];

const TABLE: [Spec; 76] = [
    overflow("llvm.sadd.with.overflow", BinaryOp::Add, true),
    overflow("llvm.uadd.with.overflow", BinaryOp::Add, false),
    overflow("llvm.ssub.with.overflow", BinaryOp::Sub, true),
    overflow("llvm.usub.with.overflow", BinaryOp::Sub, false),
    overflow("llvm.smul.with.overflow", BinaryOp::Mul, true),
    overflow("llvm.umul.with.overflow", BinaryOp::Mul, false),
    saturating("llvm.sadd.sat", BinaryOp::Add, true),
    saturating("llvm.uadd.sat", BinaryOp::Add, false),
    saturating("llvm.ssub.sat", BinaryOp::Sub, true),
    saturating("llvm.usub.sat", BinaryOp::Sub, false),
    min_max("llvm.smax", true, true),
    min_max("llvm.smin", true, false),
    min_max("llvm.umax", false, true),
    min_max("llvm.umin", false, false),
    bit_count("llvm.ctlz", BitCount::LeadingZeros),
    bit_count("llvm.cttz", BitCount::TrailingZeros),
    bit_count("llvm.ctpop", BitCount::Population),
    arithmetic("llvm.bswap", Intrinsic::ByteSwap, Slot::Any(0), UNARY, Kind::Int),
    arithmetic("llvm.bitreverse", Intrinsic::BitReverse, Slot::Any(0), UNARY, Kind::Int),
    arithmetic("llvm.fshl", Intrinsic::FunnelShift { left: true }, Slot::Any(0), TERNARY, Kind::Int),
    arithmetic("llvm.fshr", Intrinsic::FunnelShift { left: false }, Slot::Any(0), TERNARY, Kind::Int),
    arithmetic("llvm.abs", Intrinsic::Abs, Slot::Any(0), ABS, Kind::Int),
    compare("llvm.scmp", true),
    compare("llvm.ucmp", false),
    Spec {
        name: "llvm.ptrmask",
        intrinsic: Intrinsic::PtrMask,
        overloads: &[Kind::Pointer, Kind::Int],
        returns: Slot::Any(0),
        parameters: &[Slot::Any(0), Slot::Any(1)],
        own: None,
    },
    arithmetic("llvm.smul.fix", Intrinsic::Fixed { divide: false }, Slot::Any(0), FIXED, Kind::Int),
    fixed_mul("llvm.umul.fix", false, false),
    fixed_mul("llvm.smul.fix.sat", true, true),
    fixed_mul("llvm.umul.fix.sat", false, true),
    shift_sat("llvm.sshl.sat", true),
    shift_sat("llvm.ushl.sat", false),
    Spec {
        name: "llvm.sdiv.fix",
        intrinsic: Intrinsic::Fixed { divide: true },
        overloads: &[Kind::Int],
        returns: Slot::Any(0),
        parameters: FIXED,
        own: None,
    },
    arithmetic(
        "llvm.fmuladd",
        Intrinsic::FMulAdd,
        Slot::Any(0),
        &[Slot::Any(0), Slot::Any(0), Slot::Any(0)],
        Kind::Float,
    ),
    arithmetic("llvm.copysign", Intrinsic::CopySign, Slot::Any(0), &[Slot::Any(0), Slot::Any(0)], Kind::Float),
    unary("llvm.fabs", FloatFunction::Fabs),
    unary("llvm.sqrt", FloatFunction::Sqrt),
    unary("llvm.sin", FloatFunction::Sin),
    unary("llvm.cos", FloatFunction::Cos),
    unary("llvm.atan", FloatFunction::Atan),
    unary("llvm.log2", FloatFunction::Log2),
    unary("llvm.exp2", FloatFunction::Exp2),
    unary("llvm.log", FloatFunction::Log),
    unary("llvm.exp", FloatFunction::Exp),
    unary("llvm.rint", FloatFunction::Rint),
    unary("llvm.floor", FloatFunction::Floor),
    unary("llvm.ceil", FloatFunction::Ceil),
    unary("llvm.trunc", FloatFunction::Trunc),
    unary("llvm.round", FloatFunction::Round),
    unary("llvm.roundeven", FloatFunction::RoundEven),
    unary("llvm.nearbyint", FloatFunction::NearbyInt),
    Spec {
        name: "llvm.lrint",
        intrinsic: Intrinsic::LRint,
        overloads: &[Kind::Int, Kind::Float],
        returns: Slot::Any(0),
        parameters: &[Slot::Any(1)],
        own: None,
    },
    Spec {
        name: "llvm.llrint",
        intrinsic: Intrinsic::LLRint,
        overloads: &[Kind::Int, Kind::Float],
        returns: Slot::Any(0),
        parameters: &[Slot::Any(1)],
        own: None,
    },
    Spec {
        name: "llvm.memset",
        intrinsic: Intrinsic::MemSet,
        overloads: &[Kind::Pointer, Kind::Int],
        returns: Slot::Void,
        parameters: &[Slot::Any(0), Slot::Int(8), Slot::Any(1), Slot::Int(1)],
        own: None,
    },
    Spec {
        name: "llvm.experimental.memset.pattern",
        intrinsic: Intrinsic::MemSetPattern,
        overloads: &[Kind::Pointer, Kind::Int, Kind::Int],
        returns: Slot::Void,
        parameters: &[Slot::Any(0), Slot::Any(1), Slot::Any(2), Slot::Int(1)],
        own: None,
    },
    Spec {
        name: "llvm.memcpy",
        intrinsic: Intrinsic::MemCpy,
        overloads: &[Kind::Pointer, Kind::Pointer, Kind::Int],
        returns: Slot::Void,
        parameters: &[Slot::Any(0), Slot::Any(1), Slot::Any(2), Slot::Int(1)],
        own: None,
    },
    Spec {
        name: "llvm.memmove",
        intrinsic: Intrinsic::MemMove,
        overloads: &[Kind::Pointer, Kind::Pointer, Kind::Int],
        returns: Slot::Void,
        parameters: &[Slot::Any(0), Slot::Any(1), Slot::Any(2), Slot::Int(1)],
        own: None,
    },
    Spec {
        name: "llvm.expect",
        intrinsic: Intrinsic::Expect,
        overloads: &[Kind::Int],
        returns: Slot::Any(0),
        parameters: INTS,
        own: None,
    },
    Spec {
        name: "llvm.is.constant",
        intrinsic: Intrinsic::IsConstant,
        overloads: &[Kind::Int],
        returns: Slot::Int(1),
        parameters: &[Slot::Any(0)],
        own: None,
    },
    Spec {
        name: "llvm.objectsize",
        intrinsic: Intrinsic::ObjectSize,
        overloads: &[Kind::Int, Kind::Pointer],
        returns: Slot::Any(0),
        parameters: &[Slot::Any(1), Slot::Int(1), Slot::Int(1), Slot::Int(1)],
        own: None,
    },
    Spec {
        name: "llvm.assume",
        intrinsic: Intrinsic::Assume,
        overloads: &[],
        returns: Slot::Void,
        parameters: &[Slot::Int(1)],
        own: None,
    },
    Spec {
        name: "llvm.prefetch",
        intrinsic: Intrinsic::Prefetch,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: &[Slot::Any(0), Slot::Int(32), Slot::Int(32), Slot::Int(32)],
        own: None,
    },
    Spec {
        name: "llvm.fake.use",
        intrinsic: Intrinsic::FakeUse,
        overloads: &[],
        returns: Slot::Void,
        parameters: &[],
        own: None,
    },
    Spec {
        name: "llrm.noalias",
        intrinsic: Intrinsic::Restrict,
        overloads: &[Kind::Pointer],
        returns: Slot::Any(0),
        parameters: &[Slot::Any(0)],
        own: Some(Own { attrs: PURE, memory: NO_MEMORY, parameters: &[] }),
    },
    // Own attributes: MIR's markers still take the size, which `Intrinsics.td`
    // has dropped, so its parameter positions are not MIR's.
    Spec {
        name: "llvm.lifetime.start",
        intrinsic: Intrinsic::LifetimeStart,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: LIFETIME,
        own: Some(Own { attrs: LIFETIME_ATTRS, memory: &[(Some("argmem"), "write")], parameters: LIFETIME_PARAMETERS }),
    },
    Spec {
        name: "llvm.lifetime.end",
        intrinsic: Intrinsic::LifetimeEnd,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: LIFETIME,
        own: Some(Own { attrs: LIFETIME_ATTRS, memory: &[(Some("argmem"), "write")], parameters: LIFETIME_PARAMETERS }),
    },
    // `Intrinsics.td` states both: the size is `immarg`, the scope result and
    // argument are `ptr` whatever the overloaded pointer is.
    Spec {
        name: "llvm.invariant.start",
        intrinsic: Intrinsic::InvariantStart,
        overloads: &[Kind::Pointer],
        returns: Slot::Ptr,
        parameters: &[Slot::Int(64), Slot::Any(0)],
        own: None,
    },
    Spec {
        name: "llvm.invariant.end",
        intrinsic: Intrinsic::InvariantEnd,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: &[Slot::Ptr, Slot::Int(64), Slot::Any(0)],
        own: None,
    },
    Spec {
        name: "llrm.ia16.in",
        intrinsic: Intrinsic::PortIn,
        overloads: &[Kind::Int],
        returns: Slot::Any(0),
        parameters: &[Slot::Int(16)],
        own: Some(Own { attrs: PORT_ATTRS, memory: PORT_MEMORY, parameters: &[] }),
    },
    Spec {
        name: "llvm.va_start",
        intrinsic: Intrinsic::VaStart,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: &[Slot::Any(0)],
        own: None,
    },
    Spec {
        name: "llvm.va_end",
        intrinsic: Intrinsic::VaEnd,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: &[Slot::Any(0)],
        own: None,
    },
    Spec {
        name: "llvm.stacksave",
        intrinsic: Intrinsic::StackSave,
        overloads: &[Kind::Pointer],
        returns: Slot::Any(0),
        parameters: &[],
        own: None,
    },
    Spec {
        name: "llvm.stackrestore",
        intrinsic: Intrinsic::StackRestore,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: &[Slot::Any(0)],
        own: None,
    },
    Spec {
        name: "llvm.va_copy",
        intrinsic: Intrinsic::VaCopy,
        overloads: &[Kind::Pointer, Kind::Pointer],
        returns: Slot::Void,
        parameters: &[Slot::Any(0), Slot::Any(1)],
        own: None,
    },
    Spec {
        name: "llrm.ia16.ptrdiff",
        intrinsic: Intrinsic::PtrDiff,
        overloads: &[Kind::Int, Kind::Pointer],
        returns: Slot::Any(0),
        parameters: &[Slot::Any(1), Slot::Any(1)],
        own: Some(Own { attrs: PURE, memory: NO_MEMORY, parameters: &[] }),
    },
    Spec {
        name: "llrm.ia16.window",
        intrinsic: Intrinsic::Window,
        overloads: &[Kind::Pointer, Kind::Pointer],
        returns: Slot::Any(0),
        parameters: &[Slot::Any(1)],
        own: Some(Own { attrs: PURE, memory: NO_MEMORY, parameters: &[] }),
    },
    Spec {
        name: "llrm.ia16.out",
        intrinsic: Intrinsic::PortOut,
        overloads: &[Kind::Int],
        returns: Slot::Void,
        parameters: &[Slot::Int(16), Slot::Any(0)],
        own: Some(Own { attrs: PORT_ATTRS, memory: PORT_MEMORY, parameters: &[] }),
    },
];

/// Gives a function named `name` its intrinsic's attributes, if it names
/// one; as in LLVM, whatever attributes it had are dropped.
pub(crate) fn declare(
    function: &mut Function,
    types: &Types,
    name: &str,
) {
    let Some((mut attrs, parameter_attrs)) = Intrinsic::named(name).and_then(Intrinsic::attributes) else {
        return from_intrinsics_td(function, types, name);
    };
    // Assembly that reaches no memory still has effects, kept in order with
    // the ports: its own state, as a port that reaches no memory has.
    if asm(name).is_some_and(|one| !one.memory) {
        attrs.push(Attribute::Memory(vec![(Some("inaccessiblemem".to_owned()), "readwrite".to_owned())]));
    }
    function.attrs = attrs;
    for (slot, attrs) in function.parameter_attrs.iter_mut().zip(parameter_attrs) {
        *slot = attrs;
    }
}

/// The attributes `Intrinsics.td` gives the intrinsic `name` names.
fn from_intrinsics_td(
    function: &mut Function,
    types: &Types,
    name: &str,
) {
    let Some(set) = table_set(name) else { return };
    let Type::Function { returns, parameters, .. } = types.get(function.ty).clone() else { return };
    function.attrs = set.function.iter().map(|one| Attribute::Flag((*one).to_owned())).collect();
    if !set.memory.is_empty() {
        function.attrs.push(Attribute::Memory(
            set.memory.iter().map(|&(location, access)| (location.map(str::to_owned), access.to_owned())).collect(),
        ));
    }
    function.return_attrs.clear();
    function.parameter_attrs.iter_mut().for_each(Vec::clear);
    for &(at, attrs) in set.arguments {
        let ty = if at == 0 { Some(returns) } else { parameters.get(at as usize - 1).copied() };
        let Some(ty) = ty else { continue };
        let attrs = attrs.iter().filter_map(|attr| match *attr {
            intrinsic_attrs::Attr::Flag(one) => Some(Attribute::Flag(one.to_owned())),
            intrinsic_attrs::Attr::Int(one, value) => Some(Attribute::Int(one.to_owned(), value)),
            // At the argument's own width; `lower == upper` is no range.
            intrinsic_attrs::Attr::Range(lower, upper) => {
                let bits = types.int_bits(ty)?;
                let wrap = |one: i64| (one as i128 as u128) & (u128::MAX >> (128 - bits));
                (wrap(lower) != wrap(upper)).then(|| Attribute::Range { ty, lower: wrap(lower), upper: wrap(upper) })
            }
        });
        let slot =
            if at == 0 { Some(&mut function.return_attrs) } else { function.parameter_attrs.get_mut(at as usize - 1) };
        if let Some(slot) = slot {
            slot.extend(attrs);
        }
    }
}

/// `Intrinsics.td`'s attributes of the intrinsic `name` names: the longest
/// name in the table that `name` is, or extends by a type suffix where the
/// intrinsic is overloaded.
pub(crate) fn table_set(name: &str) -> Option<&'static intrinsic_attrs::Set> {
    if !name.starts_with("llvm.") {
        return None;
    }
    let mut end = name.len();
    loop {
        if let Ok(at) = intrinsic_attrs::NAMES.binary_search_by_key(&&name[..end], |&(one, ..)| one) {
            let (_, set, overloaded) = intrinsic_attrs::NAMES[at];
            return (overloaded || end == name.len()).then(|| &intrinsic_attrs::SETS[set as usize]);
        }
        end = name[..end].rfind('.')?;
    }
}

/// Whether `name` is in LLVM's reserved namespace, or llrm's own for
/// its target's intrinsics.
/// `Intrinsic::Window`'s name, from `huge` pointers to `far` ones.
pub fn window_name(
    types: &Types,
    far: TypeId,
    huge: TypeId,
) -> String {
    format!("llrm.ia16.window.{}.{}", mangle(types, far), mangle(types, huge))
}

pub fn is_reserved(name: &str) -> bool {
    name.starts_with("llvm.") || name.starts_with("llrm.ia16.") || name.starts_with("llrm.noalias")
}

/// The type suffix LLVM mangles an overloaded type into.
fn mangle(
    types: &Types,
    ty: TypeId,
) -> String {
    match types.get(ty) {
        Type::Int(bits) => format!("i{bits}"),
        Type::Float(FloatKind::Float) => "f32".to_owned(),
        Type::Float(FloatKind::Double) => "f64".to_owned(),
        Type::Float(FloatKind::X86Fp80) => "f80".to_owned(),
        Type::Pointer(space) => format!("p{space}"),
        _ => types.display(ty),
    }
}

impl Intrinsic {
    /// `llvm.smul.fix` or `llvm.sdiv.fix` of the signed `width`-bit `a` and
    /// `b` with `scale` fraction bits, signed: at twice the width, the
    /// product shifted down (toward negative infinity) or the dividend
    /// shifted up and divided (toward zero), then wrapped. `None` for a
    /// zero divisor, which is undefined.
    pub fn fixed(
        divide: bool,
        width: u32,
        a: i128,
        b: i128,
        scale: u32,
    ) -> Option<i128> {
        let wide = if divide { (a << scale).checked_div(b)? } else { (a * b) >> scale };
        let shift = 128 - width;
        Some((wide << shift) >> shift)
    }

    fn spec(self) -> &'static Spec {
        TABLE.iter().find(|spec| spec.intrinsic == self).expect("every intrinsic is in the table")
    }

    /// The intrinsic `name` declares, its type suffixes aside.
    pub fn named(name: &str) -> Option<Intrinsic> {
        if name.starts_with(CODE) {
            return Some(Intrinsic::Code);
        }
        if name.starts_with(ASM) {
            return Some(Intrinsic::Asm);
        }
        TABLE
            .iter()
            .filter(|spec| name.strip_prefix(spec.name).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
            .max_by_key(|spec| spec.name.len())
            .map(|spec| spec.intrinsic)
    }

    /// The function's attributes and each parameter's, where MIR states them;
    /// none where `Intrinsics.td` does.
    pub fn attributes(self) -> Option<(Vec<Attribute>, Vec<Vec<Attribute>>)> {
        if matches!(self, Intrinsic::Code | Intrinsic::Asm) {
            return Some((vec![crate::facts::Fact::NoUnwind.carrier()], Vec::new()));
        }
        let own = self.spec().own?;
        let flags = |names: &[&str]| names.iter().map(|one| Attribute::Flag((*one).to_owned())).collect::<Vec<_>>();
        let mut attrs = flags(own.attrs);
        attrs.push(Attribute::Memory(
            own.memory.iter().map(|&(location, access)| (location.map(str::to_owned), access.to_owned())).collect(),
        ));
        Some((attrs, own.parameters.iter().map(|one| flags(one)).collect()))
    }

    /// Checks that `function_type` is this intrinsic's, overloaded as
    /// `name` mangles it; the error is LLVM's.
    pub fn check(
        self,
        name: &str,
        types: &Types,
        function_type: TypeId,
    ) -> Result<(), String> {
        let Type::Function { returns, parameters, variadic } = types.get(function_type) else {
            unreachable!("a function's type")
        };
        if self == Intrinsic::Asm {
            let block = asm(name).ok_or("Inline assembly's name does not parse!")?;
            let word = |ty: &TypeId| {
                matches!(types.int_bits(*ty), Some(16 | 32)) || matches!(types.get(*ty), Type::Pointer(0))
            };
            let answers = match &block.outputs[..] {
                [] => types.is_void(*returns),
                [_] => word(returns),
                outputs => {
                    matches!(
                        types.get(*returns),
                        Type::Struct { fields, .. } if fields.len() == outputs.len() && fields.iter().all(word)
                    )
                }
            };
            if !answers {
                return Err("Intrinsic has incorrect return type!".to_owned());
            }
            // The symbols a block names come first, as addresses, then the
            // registers'.
            let (symbols, registers) = parameters.split_at(block.fixups.len().min(parameters.len()));
            if *variadic
                || symbols.len() != block.fixups.len()
                || registers.len() != block.inputs.len()
                || !symbols.iter().all(|ty| matches!(types.get(*ty), Type::Pointer(_)))
                || !registers.iter().all(word)
            {
                return Err("Intrinsic has incorrect argument type!".to_owned());
            }
            return Ok(());
        }
        // `void (...)`: any values, kept live for a debugger.
        if self == Intrinsic::FakeUse {
            return if types.is_void(*returns) {
                Ok(())
            } else {
                Err("Intrinsic has incorrect return type!".to_owned())
            };
        }
        if self == Intrinsic::Code {
            let (_, places) = code(name).ok_or("Inline code's name does not parse!")?;
            if types.int_bits(*returns) != Some(32) {
                return Err("Intrinsic has incorrect return type!".to_owned());
            }
            // The places' addresses first, then the arguments the code finds on
            // the stack.
            if *variadic
                || parameters.len() < places.len()
                || !parameters[..places.len()].iter().all(|one| matches!(types.get(*one), Type::Pointer(_)))
            {
                return Err("Intrinsic has incorrect argument type!".to_owned());
            }
            return Ok(());
        }
        let spec = self.spec();
        let mut bound: Vec<Option<TypeId>> = vec![None; spec.overloads.len()];
        let mut matches = |slot: Slot, ty: TypeId| -> bool {
            let mut overload = |at: usize, ty: TypeId| {
                let kind = match types.get(ty) {
                    Type::Int(_) => Kind::Int,
                    Type::Float(_) => Kind::Float,
                    Type::Pointer(_) => Kind::Pointer,
                    _ => return false,
                };
                kind == spec.overloads[at] && *bound[at].get_or_insert(ty) == ty
            };
            match slot {
                Slot::Any(at) => overload(at, ty),
                Slot::WithFlag(at) => match types.get(ty) {
                    Type::Struct { fields, packed: false } => {
                        matches!(
                            fields[..],
                            [value, flag] if types.int_bits(flag) == Some(1) && overload(at, value)
                        )
                    }
                    _ => false,
                },
                Slot::Int(bits) => types.int_bits(ty) == Some(bits),
                Slot::Ptr => matches!(types.get(ty), Type::Pointer(0)),
                Slot::Void => types.is_void(ty),
            }
        };
        if !matches(spec.returns, *returns) {
            return Err("Intrinsic has incorrect return type!".to_owned());
        }
        if *variadic
            || parameters.len() != spec.parameters.len()
            || !spec.parameters.iter().zip(parameters).all(|(slot, ty)| matches(*slot, *ty))
        {
            return Err("Intrinsic has incorrect argument type!".to_owned());
        }
        let mangled: String = std::iter::once(spec.name.to_owned())
            .chain(bound.iter().map(|ty| mangle(types, ty.expect("every overload appears"))))
            .collect::<Vec<_>>()
            .join(".");
        if name != mangled {
            return Err(format!("Intrinsic name not mangled correctly for type arguments! Should be: {mangled}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An enumerated `llvm.*` intrinsic with no entry would have no
    /// attributes at all; only the ones MIR states itself have none.
    #[test]
    fn every_enumerated_intrinsic_has_attributes_from_one_place() {
        for spec in TABLE.iter() {
            let from_table = table_set(spec.name).is_some();
            assert_ne!(from_table, spec.own.is_some(), "{}", spec.name);
        }
    }
}

#[cfg(test)]
mod length_tests {
    use super::length;
    use crate::Operand;
    use crate::context::Context;

    /// A 16-bit 40000 was read as -25536 by the selector, and a fill of it
    /// stored nothing. Every reader asks `length`, which is unsigned at the
    /// operand's own width.
    #[test]
    fn test_a_length_is_unsigned_at_the_width_of_its_type() {
        let context = Context::new();
        let (i16_, i32_) = (context.types.int(16), context.types.int(32));
        for (ty, value) in
            [(i16_, 32767), (i16_, 32768), (i16_, 40000), (i16_, 65535), (i32_, 65535), (i32_, 2_147_483_648)]
        {
            let constant = context.int(ty, value);
            assert_eq!(length(&context, Operand::Constant(constant)), Some(value as u64), "{value}");
        }
        let negative = context.int(i16_, -1);
        assert_eq!(length(&context, Operand::Constant(negative)), Some(65535), "-1 at i16 is 65535");
    }
}
