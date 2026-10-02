//! The LLVM intrinsics MIR uses, as `Intrinsics.td` states them: each one's
//! name, signature and attributes. The parser gives a declaration its
//! attributes, as LLVM does on creating it; the verifier checks its
//! signature; the interpreter runs it.

use crate::module::Function;
use crate::opcode::{Attribute, BinaryOp};
use crate::types::{FloatKind, Type, TypeId, Types};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intrinsic {
    /// `llvm.{s,u}{add,sub,mul}.with.overflow`: the wrapped result, and
    /// whether it overflowed.
    WithOverflow { op: BinaryOp, signed: bool },
    /// `llvm.{s,u}{max,min}`.
    MinMax { signed: bool, max: bool },
    /// `llvm.smul.fix` and `llvm.sdiv.fix`: signed fixed point with as many
    /// fraction bits as the third argument says.
    Fixed { divide: bool },
    FMulAdd,
    /// `llvm.fabs`, `llvm.sqrt` and their kin: a function of one float.
    Unary(FloatFunction),
    /// Rounds to an integer as the rounding mode says: by default, to
    /// nearest, ties to even.
    LRint,
    MemSet,
    LifetimeStart,
    LifetimeEnd,
    /// An I/O port's value, read: a target intrinsic, as `llvm.x86.*` are.
    PortIn,
    /// A value written to an I/O port.
    PortOut,
    /// The bytes between two pointers into one object, as a wrapping integer:
    /// a target's own, where a pointer's integer form is not its address.
    PtrDiff,
    /// `llvm.va_start`: the list its argument points to made to point at
    /// the calling function's first variadic argument.
    VaStart,
    /// `llvm.dbg.declare`: the variable its `!var` names lives where its
    /// argument points. No code; as a debugger may read or write the
    /// variable at any time, the pointer is captured.
    DbgDeclare,
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

/// What names inline code: `llrm.ia16.code.<hex bytes>` and, per argument,
/// `.<offset>` of the word that takes its displacement, `p<n>` or `m<n>`
/// added.
const CODE: &str = "llrm.ia16.code.";

/// Inline code's name: `bytes`, and each argument's word offset and addend.
pub fn code_name(bytes: &[u8], places: &[(usize, i64)]) -> String {
    let hex: String = bytes.iter().map(|one| format!("{one:02x}")).collect();
    let places: String = places.iter().map(|(at, addend)| format!(".{at}{}{}", if *addend < 0 { 'm' } else { 'p' }, addend.unsigned_abs())).collect();
    format!("{CODE}{hex}{places}")
}

/// Inline code's bytes, and each argument's word offset and addend.
pub fn code(name: &str) -> Option<(Vec<u8>, Vec<(usize, i64)>)> {
    let mut parts = name.strip_prefix(CODE)?.split('.');
    let hex = parts.next()?;
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes = (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok()).collect::<Option<Vec<u8>>>()?;
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
/// `llrm.ia16.asm.<hex bytes>.<in>.<out>.<clobbers>.<m|n>`, each list its
/// registers joined by `_` (`-` for none), `m` where it reaches memory.
const ASM: &str = "llrm.ia16.asm.";

/// An inline assembly block: registers by their 16-bit names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsmBlock {
    pub code: Vec<u8>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub clobbers: Vec<String>,
    pub memory: bool,
}

pub fn asm_name(block: &AsmBlock) -> String {
    let hex: String = block.code.iter().map(|one| format!("{one:02x}")).collect();
    let list = |names: &[String]| if names.is_empty() { "-".to_owned() } else { names.join("_") };
    format!("{ASM}{hex}.{}.{}.{}.{}", list(&block.inputs), list(&block.outputs), list(&block.clobbers), if block.memory { "m" } else { "n" })
}

pub fn asm(name: &str) -> Option<AsmBlock> {
    let [hex, inputs, outputs, clobbers, memory] = name.strip_prefix(ASM)?.split('.').collect::<Vec<_>>()[..] else { return None };
    if hex.len() % 2 != 0 {
        return None;
    }
    let code = (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok()).collect::<Option<Vec<u8>>>()?;
    let list = |names: &str| if names == "-" { Vec::new() } else { names.split('_').map(str::to_owned).collect() };
    let memory = match memory {
        "m" => true,
        "n" => false,
        _ => return None,
    };
    Some(AsmBlock { code, inputs: list(inputs), outputs: list(outputs), clobbers: list(clobbers), memory })
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
    /// Rounds to an integral float as the rounding mode says.
    Rint,
}

impl FloatFunction {
    pub fn apply(self, x: f64) -> f64 {
        match self {
            Self::Fabs => x.abs(),
            Self::Sqrt => x.sqrt(),
            Self::Sin => x.sin(),
            Self::Cos => x.cos(),
            Self::Atan => x.atan(),
            Self::Log2 => x.log2(),
            Self::Exp2 => x.exp2(),
            Self::Rint => x.round_ties_even(),
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
    Void,
}

struct Spec {
    name: &'static str,
    intrinsic: Intrinsic,
    overloads: &'static [Kind],
    returns: Slot,
    parameters: &'static [(Slot, &'static [&'static str])],
    attrs: &'static [&'static str],
    memory: &'static [(Option<&'static str>, &'static str)],
}

const PURE: &[&str] = &["nocallback", "nofree", "nosync", "nounwind", "speculatable", "willreturn"];
const NO_MEMORY: &[(Option<&str>, &str)] = &[(None, "none")];
/// Any memory: a device may start DMA. `Machine::port_touches_memory`
/// narrows a call to one that cannot.
const PORT_MEMORY: &[(Option<&str>, &str)] = &[(None, "readwrite")];
const PORT_ATTRS: &[&str] = &["nocallback", "nofree", "nounwind", "willreturn"];
const INTS: &[(Slot, &[&str])] = &[(Slot::Any(0), &[]), (Slot::Any(0), &[])];

const fn arithmetic(name: &'static str, intrinsic: Intrinsic, returns: Slot, parameters: &'static [(Slot, &'static [&'static str])], kind: Kind) -> Spec {
    let overloads: &[Kind] = match kind {
        Kind::Float => &[Kind::Float],
        _ => &[Kind::Int],
    };
    Spec { name, intrinsic, overloads, returns, parameters, attrs: PURE, memory: NO_MEMORY }
}

const fn overflow(name: &'static str, op: BinaryOp, signed: bool) -> Spec {
    arithmetic(name, Intrinsic::WithOverflow { op, signed }, Slot::WithFlag(0), INTS, Kind::Int)
}

const FLOAT: &[(Slot, &[&str])] = &[(Slot::Any(0), &[])];

const fn unary(name: &'static str, function: FloatFunction) -> Spec {
    arithmetic(name, Intrinsic::Unary(function), Slot::Any(0), FLOAT, Kind::Float)
}

const fn min_max(name: &'static str, signed: bool, max: bool) -> Spec {
    arithmetic(name, Intrinsic::MinMax { signed, max }, Slot::Any(0), INTS, Kind::Int)
}

const FIXED: &[(Slot, &[&str])] = &[(Slot::Any(0), &[]), (Slot::Any(0), &[]), (Slot::Int(32), &["immarg"])];

const LIFETIME: &[(Slot, &[&str])] = &[(Slot::Int(64), &["immarg"]), (Slot::Any(0), &["nocapture"])];
const LIFETIME_ATTRS: &[&str] = &["nocallback", "nofree", "nosync", "nounwind", "willreturn"];

const TABLE: [Spec; 30] = [
    overflow("llvm.sadd.with.overflow", BinaryOp::Add, true),
    overflow("llvm.uadd.with.overflow", BinaryOp::Add, false),
    overflow("llvm.ssub.with.overflow", BinaryOp::Sub, true),
    overflow("llvm.usub.with.overflow", BinaryOp::Sub, false),
    overflow("llvm.smul.with.overflow", BinaryOp::Mul, true),
    overflow("llvm.umul.with.overflow", BinaryOp::Mul, false),
    min_max("llvm.smax", true, true),
    min_max("llvm.smin", true, false),
    min_max("llvm.umax", false, true),
    min_max("llvm.umin", false, false),
    arithmetic("llvm.smul.fix", Intrinsic::Fixed { divide: false }, Slot::Any(0), FIXED, Kind::Int),
    // Not speculatable: a zero divisor is undefined.
    Spec {
        name: "llvm.sdiv.fix",
        intrinsic: Intrinsic::Fixed { divide: true },
        overloads: &[Kind::Int],
        returns: Slot::Any(0),
        parameters: FIXED,
        attrs: &["nocallback", "nofree", "nosync", "nounwind", "willreturn"],
        memory: NO_MEMORY,
    },
    arithmetic("llvm.fmuladd", Intrinsic::FMulAdd, Slot::Any(0), &[(Slot::Any(0), &[]), (Slot::Any(0), &[]), (Slot::Any(0), &[])], Kind::Float),
    unary("llvm.fabs", FloatFunction::Fabs),
    unary("llvm.sqrt", FloatFunction::Sqrt),
    unary("llvm.sin", FloatFunction::Sin),
    unary("llvm.cos", FloatFunction::Cos),
    unary("llvm.atan", FloatFunction::Atan),
    unary("llvm.log2", FloatFunction::Log2),
    unary("llvm.exp2", FloatFunction::Exp2),
    unary("llvm.rint", FloatFunction::Rint),
    Spec {
        name: "llvm.lrint",
        intrinsic: Intrinsic::LRint,
        overloads: &[Kind::Int, Kind::Float],
        returns: Slot::Any(0),
        parameters: &[(Slot::Any(1), &[])],
        attrs: PURE,
        memory: NO_MEMORY,
    },
    Spec {
        name: "llvm.memset",
        intrinsic: Intrinsic::MemSet,
        overloads: &[Kind::Pointer, Kind::Int],
        returns: Slot::Void,
        parameters: &[(Slot::Any(0), &["nocapture", "writeonly"]), (Slot::Int(8), &[]), (Slot::Any(1), &[]), (Slot::Int(1), &["immarg"])],
        attrs: &["nocallback", "nofree", "nounwind", "willreturn"],
        memory: &[(Some("argmem"), "write")],
    },
    Spec {
        name: "llvm.lifetime.start",
        intrinsic: Intrinsic::LifetimeStart,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: LIFETIME,
        attrs: LIFETIME_ATTRS,
        memory: &[(Some("argmem"), "readwrite")],
    },
    Spec {
        name: "llvm.lifetime.end",
        intrinsic: Intrinsic::LifetimeEnd,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: LIFETIME,
        attrs: LIFETIME_ATTRS,
        memory: &[(Some("argmem"), "readwrite")],
    },
    Spec {
        name: "llrm.ia16.in",
        intrinsic: Intrinsic::PortIn,
        overloads: &[Kind::Int],
        returns: Slot::Any(0),
        parameters: &[(Slot::Int(16), &[])],
        attrs: PORT_ATTRS,
        memory: PORT_MEMORY,
    },
    Spec {
        name: "llvm.va_start",
        intrinsic: Intrinsic::VaStart,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: &[(Slot::Any(0), &[])],
        attrs: &["nocallback", "nofree", "nosync", "nounwind", "willreturn"],
        memory: &[(Some("argmem"), "readwrite")],
    },
    Spec {
        name: "llvm.dbg.declare",
        intrinsic: Intrinsic::DbgDeclare,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: &[(Slot::Any(0), &[])],
        attrs: &["nounwind"],
        memory: &[(Some("argmem"), "readwrite")],
    },
    Spec {
        name: "llrm.ia16.ptrdiff",
        intrinsic: Intrinsic::PtrDiff,
        overloads: &[Kind::Int, Kind::Pointer],
        returns: Slot::Any(0),
        parameters: &[(Slot::Any(1), &[]), (Slot::Any(1), &[])],
        attrs: PURE,
        memory: NO_MEMORY,
    },
    Spec {
        name: "llrm.ia16.out",
        intrinsic: Intrinsic::PortOut,
        overloads: &[Kind::Int],
        returns: Slot::Void,
        parameters: &[(Slot::Int(16), &[]), (Slot::Any(0), &[])],
        attrs: PORT_ATTRS,
        memory: PORT_MEMORY,
    },
];

/// Gives a function named `name` its intrinsic's attributes, if it names
/// one; as in LLVM, whatever attributes it had are dropped.
pub(crate) fn declare(function: &mut Function, name: &str) {
    let Some(intrinsic) = Intrinsic::named(name) else { return };
    let (mut attrs, parameter_attrs) = intrinsic.attributes();
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

/// Whether `name` is in LLVM's reserved namespace, or llrm's own for
/// its target's intrinsics.
pub fn is_reserved(name: &str) -> bool {
    name.starts_with("llvm.") || name.starts_with("llrm.ia16.")
}

/// The type suffix LLVM mangles an overloaded type into.
fn mangle(types: &Types, ty: TypeId) -> String {
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
    pub fn fixed(divide: bool, width: u32, a: i128, b: i128, scale: u32) -> Option<i128> {
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

    /// The function's attributes and each parameter's.
    pub fn attributes(self) -> (Vec<Attribute>, Vec<Vec<Attribute>>) {
        if matches!(self, Intrinsic::Code | Intrinsic::Asm) {
            return (vec![crate::facts::Fact::NoUnwind.carrier()], Vec::new());
        }
        let spec = self.spec();
        let flags = |names: &[&str]| names.iter().map(|one| Attribute::Flag((*one).to_owned())).collect::<Vec<_>>();
        let mut attrs = flags(spec.attrs);
        attrs.push(Attribute::Memory(spec.memory.iter().map(|&(location, access)| (location.map(str::to_owned), access.to_owned())).collect()));
        (attrs, spec.parameters.iter().map(|(_, one)| flags(one)).collect())
    }

    /// Checks that `function_type` is this intrinsic's, overloaded as
    /// `name` mangles it; the error is LLVM's.
    pub fn check(self, name: &str, types: &Types, function_type: TypeId) -> Result<(), String> {
        let Type::Function { returns, parameters, variadic } = types.get(function_type) else { unreachable!("a function's type") };
        if self == Intrinsic::Asm {
            let block = asm(name).ok_or("Inline assembly's name does not parse!")?;
            let word = |ty: &TypeId| types.int_bits(*ty) == Some(16) || matches!(types.get(*ty), Type::Pointer(0));
            let answers = match &block.outputs[..] {
                [] => types.is_void(*returns),
                [_] => word(returns),
                outputs => matches!(types.get(*returns), Type::Struct { fields, .. } if fields.len() == outputs.len() && fields.iter().all(word)),
            };
            if !answers {
                return Err("Intrinsic has incorrect return type!".to_owned());
            }
            if *variadic || parameters.len() != block.inputs.len() || !parameters.iter().all(word) {
                return Err("Intrinsic has incorrect argument type!".to_owned());
            }
            return Ok(());
        }
        if self == Intrinsic::Code {
            let (_, places) = code(name).ok_or("Inline code's name does not parse!")?;
            if types.int_bits(*returns) != Some(32) {
                return Err("Intrinsic has incorrect return type!".to_owned());
            }
            if *variadic || parameters.len() != places.len() || !parameters.iter().all(|one| matches!(types.get(*one), Type::Pointer(_))) {
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
                    Type::Struct { fields, packed: false } => matches!(fields[..], [value, flag] if types.int_bits(flag) == Some(1) && overload(at, value)),
                    _ => false,
                },
                Slot::Int(bits) => types.int_bits(ty) == Some(bits),
                Slot::Void => types.is_void(ty),
            }
        };
        if !matches(spec.returns, *returns) {
            return Err("Intrinsic has incorrect return type!".to_owned());
        }
        if *variadic || parameters.len() != spec.parameters.len() || !spec.parameters.iter().zip(parameters).all(|((slot, _), ty)| matches(*slot, *ty)) {
            return Err("Intrinsic has incorrect argument type!".to_owned());
        }
        let mangled: String = std::iter::once(spec.name.to_owned()).chain(bound.iter().map(|ty| mangle(types, ty.expect("every overload appears")))).collect::<Vec<_>>().join(".");
        if name != mangled {
            return Err(format!("Intrinsic name not mangled correctly for type arguments! Should be: {mangled}"));
        }
        Ok(())
    }
}
