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
    FMulAdd,
    /// `llvm.fabs`, `llvm.sqrt` and their kin: a function of one float.
    Unary(FloatFunction),
    /// Rounds to an integer as the rounding mode says: by default, to
    /// nearest, ties to even.
    LRint,
    MemSet,
    LifetimeStart,
    LifetimeEnd,
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
    memory: (Option<&'static str>, &'static str),
}

const PURE: &[&str] = &["nocallback", "nofree", "nosync", "nounwind", "speculatable", "willreturn"];
const NO_MEMORY: (Option<&str>, &str) = (None, "none");
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

const LIFETIME: &[(Slot, &[&str])] = &[(Slot::Int(64), &["immarg"]), (Slot::Any(0), &["nocapture"])];
const LIFETIME_ATTRS: &[&str] = &["nocallback", "nofree", "nosync", "nounwind", "willreturn"];

const TABLE: [Spec; 23] = [
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
        memory: (Some("argmem"), "write"),
    },
    Spec {
        name: "llvm.lifetime.start",
        intrinsic: Intrinsic::LifetimeStart,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: LIFETIME,
        attrs: LIFETIME_ATTRS,
        memory: (Some("argmem"), "readwrite"),
    },
    Spec {
        name: "llvm.lifetime.end",
        intrinsic: Intrinsic::LifetimeEnd,
        overloads: &[Kind::Pointer],
        returns: Slot::Void,
        parameters: LIFETIME,
        attrs: LIFETIME_ATTRS,
        memory: (Some("argmem"), "readwrite"),
    },
];

/// Gives a function named `name` its intrinsic's attributes, if it names
/// one; as in LLVM, whatever attributes it had are dropped.
pub(crate) fn declare(function: &mut Function, name: &str) {
    let Some(intrinsic) = Intrinsic::named(name) else { return };
    let (attrs, parameter_attrs) = intrinsic.attributes();
    function.attrs = attrs;
    for (slot, attrs) in function.parameter_attrs.iter_mut().zip(parameter_attrs) {
        *slot = attrs;
    }
}

/// Whether `name` is in LLVM's reserved namespace.
pub fn is_reserved(name: &str) -> bool {
    name.starts_with("llvm.")
}

/// The type suffix LLVM mangles an overloaded type into.
fn mangle(types: &Types, ty: TypeId) -> String {
    match types.get(ty) {
        Type::Int(bits) => format!("i{bits}"),
        Type::Float(FloatKind::Float) => "f32".to_owned(),
        Type::Float(FloatKind::Double) => "f64".to_owned(),
        Type::Pointer(space) => format!("p{space}"),
        _ => types.display(ty),
    }
}

impl Intrinsic {
    fn spec(self) -> &'static Spec {
        TABLE.iter().find(|spec| spec.intrinsic == self).expect("every intrinsic is in the table")
    }

    /// The intrinsic `name` declares, its type suffixes aside.
    pub fn named(name: &str) -> Option<Intrinsic> {
        TABLE
            .iter()
            .filter(|spec| name.strip_prefix(spec.name).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
            .max_by_key(|spec| spec.name.len())
            .map(|spec| spec.intrinsic)
    }

    /// The function's attributes and each parameter's.
    pub fn attributes(self) -> (Vec<Attribute>, Vec<Vec<Attribute>>) {
        let spec = self.spec();
        let flags = |names: &[&str]| names.iter().map(|one| Attribute::Flag((*one).to_owned())).collect::<Vec<_>>();
        let mut attrs = flags(spec.attrs);
        let (location, access) = spec.memory;
        attrs.push(Attribute::Memory(vec![(location.map(str::to_owned), access.to_owned())]));
        (attrs, spec.parameters.iter().map(|(_, one)| flags(one)).collect())
    }

    /// Checks that `function_type` is this intrinsic's, overloaded as
    /// `name` mangles it; the error is LLVM's.
    pub fn check(self, name: &str, types: &Types, function_type: TypeId) -> Result<(), String> {
        let spec = self.spec();
        let Type::Function { returns, parameters, variadic } = types.get(function_type) else { unreachable!("a function's type") };
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
