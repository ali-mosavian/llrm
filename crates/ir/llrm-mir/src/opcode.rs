//! LLVM's instructions, each with what its class carries beside operands.

use crate::types::TypeId;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    UDiv,
    SDiv,
    URem,
    SRem,
    Shl,
    LShr,
    AShr,
    And,
    Or,
    Xor,
    FAdd,
    FSub,
    FMul,
    FDiv,
    FRem,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CastOp {
    Trunc,
    ZExt,
    SExt,
    FPTrunc,
    FPExt,
    FPToUI,
    FPToSI,
    UIToFP,
    SIToFP,
    PtrToInt,
    IntToPtr,
    BitCast,
    AddrSpaceCast,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IntPredicate {
    Eq,
    Ne,
    Ugt,
    Uge,
    Ult,
    Ule,
    Sgt,
    Sge,
    Slt,
    Sle,
}

impl IntPredicate {
    /// The predicate that holds where this one does not.
    pub fn inverse(self) -> Self {
        match self {
            Self::Eq => Self::Ne,
            Self::Ne => Self::Eq,
            Self::Ugt => Self::Ule,
            Self::Uge => Self::Ult,
            Self::Ult => Self::Uge,
            Self::Ule => Self::Ugt,
            Self::Sgt => Self::Sle,
            Self::Sge => Self::Slt,
            Self::Slt => Self::Sge,
            Self::Sle => Self::Sgt,
        }
    }

    /// The predicate that holds with the operands exchanged.
    pub fn swapped(self) -> Self {
        match self {
            Self::Ugt => Self::Ult,
            Self::Uge => Self::Ule,
            Self::Ult => Self::Ugt,
            Self::Ule => Self::Uge,
            Self::Sgt => Self::Slt,
            Self::Sge => Self::Sle,
            Self::Slt => Self::Sgt,
            Self::Sle => Self::Sge,
            same => same,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatPredicate {
    False,
    Oeq,
    Ogt,
    Oge,
    Olt,
    Ole,
    One,
    Ord,
    Ueq,
    Ugt,
    Uge,
    Ult,
    Ule,
    Une,
    Uno,
    True,
}

/// Keyword spellings, shared by the parser and the printer.
pub const BINARY: [(BinaryOp, &str); 18] = [
    (BinaryOp::Add, "add"),
    (BinaryOp::Sub, "sub"),
    (BinaryOp::Mul, "mul"),
    (BinaryOp::UDiv, "udiv"),
    (BinaryOp::SDiv, "sdiv"),
    (BinaryOp::URem, "urem"),
    (BinaryOp::SRem, "srem"),
    (BinaryOp::Shl, "shl"),
    (BinaryOp::LShr, "lshr"),
    (BinaryOp::AShr, "ashr"),
    (BinaryOp::And, "and"),
    (BinaryOp::Or, "or"),
    (BinaryOp::Xor, "xor"),
    (BinaryOp::FAdd, "fadd"),
    (BinaryOp::FSub, "fsub"),
    (BinaryOp::FMul, "fmul"),
    (BinaryOp::FDiv, "fdiv"),
    (BinaryOp::FRem, "frem"),
];

pub const CAST: [(CastOp, &str); 13] = [
    (CastOp::Trunc, "trunc"),
    (CastOp::ZExt, "zext"),
    (CastOp::SExt, "sext"),
    (CastOp::FPTrunc, "fptrunc"),
    (CastOp::FPExt, "fpext"),
    (CastOp::FPToUI, "fptoui"),
    (CastOp::FPToSI, "fptosi"),
    (CastOp::UIToFP, "uitofp"),
    (CastOp::SIToFP, "sitofp"),
    (CastOp::PtrToInt, "ptrtoint"),
    (CastOp::IntToPtr, "inttoptr"),
    (CastOp::BitCast, "bitcast"),
    (CastOp::AddrSpaceCast, "addrspacecast"),
];

pub const INT_PREDICATE: [(IntPredicate, &str); 10] = [
    (IntPredicate::Eq, "eq"),
    (IntPredicate::Ne, "ne"),
    (IntPredicate::Ugt, "ugt"),
    (IntPredicate::Uge, "uge"),
    (IntPredicate::Ult, "ult"),
    (IntPredicate::Ule, "ule"),
    (IntPredicate::Sgt, "sgt"),
    (IntPredicate::Sge, "sge"),
    (IntPredicate::Slt, "slt"),
    (IntPredicate::Sle, "sle"),
];

pub const FLOAT_PREDICATE: [(FloatPredicate, &str); 16] = [
    (FloatPredicate::False, "false"),
    (FloatPredicate::Oeq, "oeq"),
    (FloatPredicate::Ogt, "ogt"),
    (FloatPredicate::Oge, "oge"),
    (FloatPredicate::Olt, "olt"),
    (FloatPredicate::Ole, "ole"),
    (FloatPredicate::One, "one"),
    (FloatPredicate::Ord, "ord"),
    (FloatPredicate::Ueq, "ueq"),
    (FloatPredicate::Ugt, "ugt"),
    (FloatPredicate::Uge, "uge"),
    (FloatPredicate::Ult, "ult"),
    (FloatPredicate::Ule, "ule"),
    (FloatPredicate::Une, "une"),
    (FloatPredicate::Uno, "uno"),
    (FloatPredicate::True, "true"),
];

/// The spelling of `value` in `table`.
pub fn spelling<T: PartialEq + Copy>(table: &[(T, &'static str)], value: T) -> &'static str {
    table.iter().find(|(one, _)| *one == value).map(|(_, name)| *name).expect("every value is spelled")
}

/// The value `word` spells in `table`.
pub fn spelled<T: Copy>(table: &[(T, &'static str)], word: &str) -> Option<T> {
    table.iter().find(|(_, name)| *name == word).map(|(one, _)| *one)
}

/// Poison-generating and fast-math flags, in LLVM's printing order.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Flags(u16);

impl Flags {
    pub const NUW: Flags = Flags(1);
    pub const NSW: Flags = Flags(1 << 1);
    pub const EXACT: Flags = Flags(1 << 2);
    pub const DISJOINT: Flags = Flags(1 << 3);
    pub const NNEG: Flags = Flags(1 << 4);
    pub const SAMESIGN: Flags = Flags(1 << 5);
    pub const INBOUNDS: Flags = Flags(1 << 6);
    pub const NUSW: Flags = Flags(1 << 7);
    pub const REASSOC: Flags = Flags(1 << 8);
    pub const NNAN: Flags = Flags(1 << 9);
    pub const NINF: Flags = Flags(1 << 10);
    pub const NSZ: Flags = Flags(1 << 11);
    pub const ARCP: Flags = Flags(1 << 12);
    pub const FAST: Flags = Flags(0x7f << 8);

    pub const NAMES: [(Flags, &'static str); 16] = [
        (Flags::DISJOINT, "disjoint"),
        (Flags::EXACT, "exact"),
        (Flags::INBOUNDS, "inbounds"),
        (Flags::NUSW, "nusw"),
        (Flags::NUW, "nuw"),
        (Flags::NSW, "nsw"),
        (Flags::NNEG, "nneg"),
        (Flags::SAMESIGN, "samesign"),
        (Flags::FAST, "fast"),
        (Flags::REASSOC, "reassoc"),
        (Flags::NNAN, "nnan"),
        (Flags::NINF, "ninf"),
        (Flags::NSZ, "nsz"),
        (Flags::ARCP, "arcp"),
        (Flags(1 << 13), "contract"),
        (Flags(1 << 14), "afn"),
    ];

    pub fn contains(self, other: Flags) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: Flags) {
        self.0 |= other.0;
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The flags both have.
    pub fn intersect(self, other: Flags) -> Flags {
        Flags(self.0 & other.0)
    }

    /// The words LLVM prints, `fast` standing for all seven fast-math flags.
    pub fn words(self) -> Vec<&'static str> {
        let fast = self.contains(Flags::FAST);
        Flags::NAMES
            .iter()
            .filter(|(flag, _)| self.contains(*flag) && (!fast || flag.0 < 1 << 8 || *flag == Flags::FAST))
            .map(|(_, name)| *name)
            .collect()
    }
}

/// A function, parameter, return or call-site attribute.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Attribute {
    Flag(String),
    Int(String, u64),
    Type(String, TypeId),
    /// `memory(...)`: each location, or none for the default, and its access.
    Memory(Vec<(Option<String>, String)>),
    /// `range(iN lower, upper)`: the half-open range of values, as bits.
    Range { ty: TypeId, lower: u128, upper: u128 },
    /// `initializes((lower, upper), ...)`: the half-open byte ranges of a
    /// pointer parameter's memory the function writes before reading, on
    /// every path to a return.
    Initializes(Vec<(i64, i64)>),
    Str(String, Option<String>),
}

impl Attribute {
    /// This attribute of `from`'s types in `types`: none where it names a
    /// type `types` cannot hold.
    pub fn imported(&self, types: &mut crate::types::Types, from: &crate::types::Types) -> Option<Attribute> {
        Some(match self {
            Attribute::Type(name, ty) => Attribute::Type(name.clone(), types.imported(from, *ty)?),
            Attribute::Range { ty, lower, upper } => Attribute::Range { ty: types.imported(from, *ty)?, lower: *lower, upper: *upper },
            other => other.clone(),
        })
    }
}

pub const FLAG_ATTRIBUTES: [&str; 42] = [
    "alwaysinline",
    "builtin",
    "cold",
    "convergent",
    "dead_on_unwind",
    "hot",
    "immarg",
    "inlinehint",
    "inreg",
    "minsize",
    "mustprogress",
    "naked",
    "nearcode",
    "nest",
    "noalias",
    "nobuiltin",
    "nocallback",
    "nocapture",
    "nofree",
    "noinline",
    "nomerge",
    "nonnull",
    "norecurse",
    "noretain",
    "noreturn",
    "nosync",
    "noundef",
    "nounwind",
    "optnone",
    "optsize",
    "readnone",
    "readonly",
    "releases",
    "returned",
    "returns_twice",
    "signext",
    "speculatable",
    "threeway",
    "willreturn",
    "writable",
    "writeonly",
    "zeroext",
];

pub const INT_ATTRIBUTES: [&str; 4] = ["align", "alignstack", "dereferenceable", "dereferenceable_or_null"];

pub const TYPE_ATTRIBUTES: [&str; 5] = ["byref", "byval", "elementtype", "inalloca", "sret"];

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Tail {
    #[default]
    None,
    Tail,
    MustTail,
    NoTail,
}

/// What a call or invoke carries beside its operands.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CallInfo {
    pub function_type: TypeId,
    /// LLVM's calling convention number; 0 is C's.
    pub calling_convention: u32,
    pub return_attrs: Vec<Attribute>,
    pub argument_attrs: Vec<Vec<Attribute>>,
    pub attrs: Vec<Attribute>,
    pub tail: Tail,
}

/// The calling conventions LLVM names, by number; any other is `ccN`.
pub const CONVENTIONS: [(&str, u32); 8] = [("ccc", 0), ("fastcc", 8), ("coldcc", 9), ("x86_stdcallcc", 64), ("x86_fastcallcc", 65), ("x86_intrcc", 83), ("watcallcc", WATCALL), ("sysvcc", SYSV)];

/// The calling conventions a target names by the `cc` of its description (calling.toml): `ccc` is the
/// one stating `cc = "cdecl"`, and each other is asked for as `<cc>cc`.
/// The string attribute that says how an argument is passed where its convention's registers do not: `memory`
/// (a struct's words, in memory with everything after) or `result-pointer` (where a struct result is written).
pub const ARGUMENT: &str = "llrm-argument";
pub const MEMORY: &str = "memory";
pub const RESULT_POINTER: &str = "result-pointer";

/// What `attrs` say of how its argument is passed: `MEMORY` or `RESULT_POINTER`.
pub fn argument_class(attrs: &[Attribute]) -> Option<&str> {
    attrs.iter().find_map(|one| match one {
        Attribute::Str(key, Some(value)) if key == ARGUMENT => Some(value.as_str()),
        _ => None,
    })
}

/// `sysvcc`: the i386 System V ABI, gcc's on Linux.
pub const SYSV: u32 = 1003;

/// `watcallcc`: Open Watcom's register convention.
pub const WATCALL: u32 = 1002;

/// BASIC's own: arguments pushed left to right, popped by the callee.
pub const BASIC: u32 = 1000;

/// LLVM's `x86_intrcc`: an interrupt handler, entered with the flags pushed
/// and left by `iret`. Unlike LLVM's, its one parameter points at the frame
/// the handler saved (the target's `interrupt_frame` in its calling.toml), not at the IP, CS and flags alone.
pub const X86_INTR: u32 = 83;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Clause {
    Catch,
    Filter,
}

/// Operands, in order: `ret [v]`; `br dest` or `br cond, true, false`;
/// `switch v, default, (case, dest)*`; `invoke args.., normal, unwind,
/// callee` and `call args.., callee`, as LLVM's CallBase keeps them;
/// `phi (v, block)*`; `alloca [count]`; `store value, ptr`; `gep ptr,
/// indices..`; `landingpad clauses..`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Opcode {
    Ret,
    Br,
    Switch,
    Invoke(Box<CallInfo>),
    Resume,
    Unreachable,
    FNeg,
    Binary(BinaryOp),
    Cast(CastOp),
    ExtractValue(Vec<u32>),
    InsertValue(Vec<u32>),
    Alloca { allocated: TypeId, align: Option<u64>, address_space: u32 },
    Load { align: Option<u64>, volatile: bool },
    Store { align: Option<u64>, volatile: bool },
    GetElementPtr { source: TypeId },
    ICmp(IntPredicate),
    FCmp(FloatPredicate),
    Phi,
    Select,
    Freeze,
    Call(Box<CallInfo>),
    LandingPad { cleanup: bool, clauses: Vec<Clause> },
}

impl Opcode {
    pub fn is_terminator(&self) -> bool {
        matches!(self, Self::Ret | Self::Br | Self::Switch | Self::Invoke(_) | Self::Resume | Self::Unreachable)
    }

    pub fn mnemonic(&self) -> &'static str {
        match self {
            Self::Ret => "ret",
            Self::Br => "br",
            Self::Switch => "switch",
            Self::Invoke(_) => "invoke",
            Self::Resume => "resume",
            Self::Unreachable => "unreachable",
            Self::FNeg => "fneg",
            Self::Binary(op) => spelling(&BINARY, *op),
            Self::Cast(op) => spelling(&CAST, *op),
            Self::ExtractValue(_) => "extractvalue",
            Self::InsertValue(_) => "insertvalue",
            Self::Alloca { .. } => "alloca",
            Self::Load { .. } => "load",
            Self::Store { .. } => "store",
            Self::GetElementPtr { .. } => "getelementptr",
            Self::ICmp(_) => "icmp",
            Self::FCmp(_) => "fcmp",
            Self::Phi => "phi",
            Self::Select => "select",
            Self::Freeze => "freeze",
            Self::Call(_) => "call",
            Self::LandingPad { .. } => "landingpad",
        }
    }
}

/// LLVM's `fastcc`, the convention the compiler gives a function it sees every caller of: here
/// arguments pushed right to left and popped by the callee (`ret N`), which a caller's
/// `add sp,N` then does not repeat.
pub const FAST: u32 = 8;
