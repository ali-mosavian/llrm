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
        (Flags(1 << 8), "reassoc"),
        (Flags(1 << 9), "nnan"),
        (Flags(1 << 10), "ninf"),
        (Flags(1 << 11), "nsz"),
        (Flags(1 << 12), "arcp"),
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
    Str(String, Option<String>),
}

pub const FLAG_ATTRIBUTES: [&str; 37] = [
    "alwaysinline",
    "builtin",
    "cold",
    "convergent",
    "dead_on_unwind",
    "hot",
    "immarg",
    "inreg",
    "minsize",
    "mustprogress",
    "naked",
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
    "noreturn",
    "nosync",
    "noundef",
    "nounwind",
    "optnone",
    "optsize",
    "readnone",
    "readonly",
    "returned",
    "returns_twice",
    "signext",
    "speculatable",
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
pub const CONVENTIONS: [(&str, u32); 5] = [("ccc", 0), ("fastcc", 8), ("coldcc", 9), ("x86_stdcallcc", 64), ("x86_fastcallcc", 65)];

/// BASIC's own: arguments pushed left to right, popped by the callee.
pub const BASIC: u32 = 1000;

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
