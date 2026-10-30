//! LLVM's instructions, each with what its class carries beside operands.

use std::cmp::Ordering;

use crate::types::TypeId;

/// LLVM's `AtomicOrdering` (`unordered` and `not_atomic` aside: a plain access
/// is a `Load` or a `Store`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub enum AtomicOrdering {
    Unordered,
    Monotonic,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

impl AtomicOrdering {
    /// The spelling in the text format.
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Unordered => "unordered",
            Self::Monotonic => "monotonic",
            Self::Acquire => "acquire",
            Self::Release => "release",
            Self::AcqRel => "acq_rel",
            Self::SeqCst => "seq_cst",
        }
    }

    pub fn named(word: &str) -> Option<Self> {
        [Self::Unordered, Self::Monotonic, Self::Acquire, Self::Release, Self::AcqRel, Self::SeqCst]
            .into_iter()
            .find(|one| one.spelling() == word)
    }
}

/// The operation of an `atomicrmw`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RmwOp {
    Xchg,
    Add,
    Sub,
    And,
    Nand,
    Or,
    Xor,
    Max,
    Min,
    UMax,
    UMin,
}

pub const RMW_OPS: [(RmwOp, &str); 11] = [
    (RmwOp::Xchg, "xchg"),
    (RmwOp::Add, "add"),
    (RmwOp::Sub, "sub"),
    (RmwOp::And, "and"),
    (RmwOp::Nand, "nand"),
    (RmwOp::Or, "or"),
    (RmwOp::Xor, "xor"),
    (RmwOp::Max, "max"),
    (RmwOp::Min, "min"),
    (RmwOp::UMax, "umax"),
    (RmwOp::UMin, "umin"),
];

/// Which atomic instruction an [`Opcode::Atomic`] is, and its operands:
/// `load atomic` (pointer), `store atomic` (value, pointer), `atomicrmw`
/// (pointer, value), `cmpxchg` (pointer, expected, new; the result is `{ T, i1
/// }`) and `fence` (none).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AtomicKind {
    Load,
    Store,
    Rmw(RmwOp),
    CmpXchg { failure: AtomicOrdering, weak: bool },
    Fence,
}

/// What `Opcode::Atomic` carries. LLVM keeps the ordering and the scope on
/// `LoadInst`, `StoreInst`, `AtomicRMWInst`, `AtomicCmpXchgInst` and
/// `FenceInst`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AtomicInfo {
    pub kind: AtomicKind,
    /// The success ordering of a `cmpxchg`.
    pub ordering: AtomicOrdering,
    /// `syncscope("singlethread")`.
    pub single_thread: bool,
    pub volatile: bool,
    pub align: Option<u64>,
}

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
    /// A float rounded to its own type's precision: what a store to a variable
    /// of that type does to a value the target holds wider (x87's
    /// extended). The operand and the result have one type.
    FPRound,
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

    /// `CmpInst::isSigned`.
    pub fn is_signed(self) -> bool {
        matches!(self, Self::Sgt | Self::Sge | Self::Slt | Self::Sle)
    }

    /// `CmpInst::getSignedPredicate`: an unsigned predicate's signed twin.
    pub fn signed(self) -> Self {
        match self {
            Self::Ugt => Self::Sgt,
            Self::Uge => Self::Sge,
            Self::Ult => Self::Slt,
            Self::Ule => Self::Sle,
            same => same,
        }
    }

    /// `CmpInst::isTrueWhenEqual`.
    pub fn true_when_equal(self) -> bool {
        matches!(self, Self::Eq | Self::Uge | Self::Ule | Self::Sge | Self::Sle)
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

    /// Whether `x self y` for `width`-bit integers given as unsigned bits.
    pub fn holds(
        self,
        x: u128,
        y: u128,
        width: u32,
    ) -> bool {
        let (sx, sy) = (crate::context::signed(x, width), crate::context::signed(y, width));
        match self {
            Self::Eq => x == y,
            Self::Ne => x != y,
            Self::Ugt => x > y,
            Self::Uge => x >= y,
            Self::Ult => x < y,
            Self::Ule => x <= y,
            Self::Sgt => sx > sy,
            Self::Sge => sx >= sy,
            Self::Slt => sx < sy,
            Self::Sle => sx <= sy,
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

impl FloatPredicate {
    /// `CmpInst::isOrdered`.
    pub fn is_ordered(self) -> bool {
        matches!(
            self,
            Self::Oeq | Self::Ogt | Self::Oge | Self::Olt | Self::Ole | Self::One | Self::Ord
        )
    }

    /// `CmpInst::isUnordered`.
    pub fn is_unordered(self) -> bool {
        matches!(
            self,
            Self::Uno | Self::Ueq | Self::Ugt | Self::Uge | Self::Ult | Self::Ule | Self::Une
        )
    }

    /// `FCmpInst::compare`
    /// ([Instructions.cpp:3916](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/Instructions.cpp#L3916)):
    /// whether the predicate holds for operands that compare as `order`, None
    /// being unordered.
    pub fn holds(
        self,
        order: Option<Ordering>,
    ) -> bool {
        use Ordering::{Equal, Greater, Less};
        match self {
            Self::False => false,
            Self::True => true,
            Self::Uno => order.is_none(),
            Self::Ord => order.is_some(),
            Self::Ueq => matches!(order, None | Some(Equal)),
            Self::Oeq => order == Some(Equal),
            Self::Une => order != Some(Equal),
            Self::One => matches!(order, Some(Less | Greater)),
            Self::Ult => matches!(order, None | Some(Less)),
            Self::Olt => order == Some(Less),
            Self::Ugt => matches!(order, None | Some(Greater)),
            Self::Ogt => order == Some(Greater),
            Self::Ule => order != Some(Greater),
            Self::Ole => matches!(order, Some(Less | Equal)),
            Self::Uge => order != Some(Less),
            Self::Oge => matches!(order, Some(Greater | Equal)),
        }
    }

    /// `CmpInst::getInversePredicate`: the predicate that holds where this one
    /// does not (an ordered one becomes the unordered of its complement).
    pub fn inverse(self) -> Self {
        match self {
            Self::False => Self::True,
            Self::True => Self::False,
            Self::Oeq => Self::Une,
            Self::Ogt => Self::Ule,
            Self::Oge => Self::Ult,
            Self::Olt => Self::Uge,
            Self::Ole => Self::Ugt,
            Self::One => Self::Ueq,
            Self::Ord => Self::Uno,
            Self::Ueq => Self::One,
            Self::Ugt => Self::Ole,
            Self::Uge => Self::Olt,
            Self::Ult => Self::Oge,
            Self::Ule => Self::Ogt,
            Self::Une => Self::Oeq,
            Self::Uno => Self::Ord,
        }
    }

    /// `CmpInst::getSwappedPredicate`: the predicate that holds with the
    /// operands exchanged.
    pub fn swapped(self) -> Self {
        match self {
            Self::Ogt => Self::Olt,
            Self::Oge => Self::Ole,
            Self::Olt => Self::Ogt,
            Self::Ole => Self::Oge,
            Self::Ugt => Self::Ult,
            Self::Uge => Self::Ule,
            Self::Ult => Self::Ugt,
            Self::Ule => Self::Uge,
            same => same,
        }
    }
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

pub const CAST: [(CastOp, &str); 15] = [
    (CastOp::Trunc, "trunc"),
    (CastOp::ZExt, "zext"),
    (CastOp::SExt, "sext"),
    (CastOp::FPTrunc, "fptrunc"),
    (CastOp::FPExt, "fpext"),
    (CastOp::FPRound, "fpround"),
    (CastOp::FPToUI, "fptoui"),
    (CastOp::FPToSI, "fptosi"),
    (CastOp::UIToFP, "uitofp"),
    (CastOp::SIToFP, "sitofp"),
    (CastOp::PtrToInt, "ptrtoint"),
    // Newer LLVM's `ptrtoaddr`: the address of a pointer as an integer, read as
    // `ptrtoint` (the same where pointer and address have one width); written
    // as
    // `ptrtoint`, the first spelling of the operation.
    (CastOp::PtrToInt, "ptrtoaddr"),
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
pub fn spelling<T: PartialEq + Copy>(
    table: &[(T, &'static str)],
    value: T,
) -> &'static str {
    table.iter().find(|(one, _)| *one == value).map(|(_, name)| *name).expect("every value is spelled")
}

/// The value `word` spells in `table`.
pub fn spelled<T: Copy>(
    table: &[(T, &'static str)],
    word: &str,
) -> Option<T> {
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
    pub const CONTRACT: Flags = Flags(1 << 13);
    pub const FAST: Flags = Flags(0x7f << 8);
    /// What makes a result poison where the operation's promise breaks:
    /// `Instruction::dropPoisonGeneratingFlags`.
    pub const POISON_GENERATING: Flags = Flags(
        Flags::NUW.0
            | Flags::NSW.0
            | Flags::EXACT.0
            | Flags::DISJOINT.0
            | Flags::NNEG.0
            | Flags::SAMESIGN.0
            | Flags::INBOUNDS.0
            | Flags::NUSW.0
            | Flags::NNAN.0
            | Flags::NINF.0,
    );
    /// On a `switch`: the backend lowers it as a jump table, its cases the
    /// indices `0..n` in a register of the pointer's width.
    pub const TABLE: Flags = Flags(1 << 15);

    pub const NAMES: [(Flags, &'static str); 17] = [
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
        (Flags::CONTRACT, "contract"),
        (Flags(1 << 14), "afn"),
        (Flags::TABLE, "table"),
    ];

    pub fn contains(
        self,
        other: Flags,
    ) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(
        &mut self,
        other: Flags,
    ) {
        self.0 |= other.0;
    }

    pub fn remove(
        &mut self,
        other: Flags,
    ) {
        self.0 &= !other.0;
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The flags of `self` that `other` does not have.
    pub fn without(
        self,
        other: Flags,
    ) -> Flags {
        Flags(self.0 & !other.0)
    }

    /// The flags either has.
    pub fn union(
        self,
        other: Flags,
    ) -> Flags {
        Flags(self.0 | other.0)
    }

    /// The flags both have.
    pub fn intersect(
        self,
        other: Flags,
    ) -> Flags {
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
    Range {
        ty: TypeId,
        lower: u128,
        upper: u128,
    },
    /// `initializes((lower, upper), ...)`: the half-open byte ranges of a
    /// pointer parameter's memory the function writes before reading, on
    /// every path to a return.
    Initializes(Vec<(i64, i64)>),
    Str(String, Option<String>),
}

impl Attribute {
    /// This attribute of `from`'s types in `types`: none where it names a
    /// type `types` cannot hold.
    pub fn imported(
        &self,
        types: &mut crate::types::Types,
        from: &crate::types::Types,
    ) -> Option<Attribute> {
        Some(match self {
            Attribute::Type(name, ty) => Attribute::Type(name.clone(), types.imported(from, *ty)?),
            Attribute::Range { ty, lower, upper } => {
                Attribute::Range { ty: types.imported(from, *ty)?, lower: *lower, upper: *upper }
            }
            other => other.clone(),
        })
    }
}

pub const FLAG_ATTRIBUTES: [&str; 46] = [
    "alwaysinline",
    "builtin",
    "cold",
    "convergent",
    "dead_on_unwind",
    "flagsresult",
    "frame_bound",
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
    "noipa",
    "nomerge",
    "nonnull",
    "norecurse",
    "noretain",
    "noreturn",
    "nosync",
    "noundef",
    "nounwind",
    "null_pointer_is_valid",
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

/// Attributes the pin's clang states that no pass here reads (stack protection,
/// unwind tables, `dead_on_return`...), kept as flags; `uwtable` may carry its
/// kind in parentheses (`uwtable(sync)`).
pub const TARGET_FLAG_ATTRIBUTES: [&str; 9] = [
    "allockind",
    "allocsize",
    "dead_on_return",
    "nocreateundeforpoison",
    "nofreeobj",
    "ssp",
    "sspreq",
    "sspstrong",
    "uwtable",
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
pub const CONVENTIONS: [(&str, u32); 12] = [
    ("ccc", 0),
    ("fastcc", 8),
    ("coldcc", 9),
    ("x86_stdcallcc", 64),
    ("x86_fastcallcc", 65),
    ("x86_intrcc", 83),
    ("x86_regcallcc", REGCALL),
    ("watcallcc", WATCALL),
    ("sysvcc", SYSV),
    ("ia16cc", IA16),
    ("regparm3cc", REGPARM3),
    ("regparm3popcc", REGPARM3POP),
];

/// The calling conventions a target names by the `cc` of its description
/// (calling.toml): `ccc` is the one stating `cc = "cdecl"`, and each other is
/// asked for as `<cc>cc`. The string attribute that says how an argument is
/// passed where its convention's registers do not: `memory` (a struct's words,
/// in memory with everything after) or `result-pointer` (where a struct result
/// is written).
pub const ARGUMENT: &str = "llrm-argument";
pub const MEMORY: &str = "memory";
pub const RESULT_POINTER: &str = "result-pointer";

/// What `attrs` say of how its argument is passed: `MEMORY` or
/// `RESULT_POINTER`.
pub fn argument_class(attrs: &[Attribute]) -> Option<&str> {
    attrs
        .iter()
        .find_map(
            |one| match one {
                Attribute::Str(key, Some(value)) if key == ARGUMENT => Some(value.as_str()),
                // LLVM's own marker for a struct result's address
                Attribute::Type(name, _) if name == "sret" => Some(RESULT_POINTER),
                _ => None,
            },
        )
}

/// `regparm3cc`: gcc's `-mregparm=3`, by the size of the argument.
pub const REGPARM3: u32 = 1005;

/// `regparm3popcc`: `regparm3cc` whose callee removes the stack arguments.
pub const REGPARM3POP: u32 = 1006;

/// `ia16cc`: gcc-ia16's convention.
pub const IA16: u32 = 1004;

/// `sysvcc`: the i386 System V ABI, gcc's on Linux.
pub const SYSV: u32 = 1003;

/// `watcallcc`: Open Watcom's register convention.
/// clang's `x86_regcallcc`.
pub const REGCALL: u32 = 92;
pub const WATCALL: u32 = 1002;

/// BASIC's own: arguments pushed left to right, popped by the callee.
pub const BASIC: u32 = 1000;

/// LLVM's `x86_intrcc`: an interrupt handler, entered with the flags pushed
/// and left by `iret`. Unlike LLVM's, its one parameter points at the frame
/// the handler saved (the target's `interrupt_frame` in its calling.toml), not
/// at the IP, CS and flags alone.
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
#[derive(Clone, Debug, Eq, Hash)]
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
    Alloca {
        allocated: TypeId,
        align: Option<u64>,
        address_space: u32,
    },
    Load {
        align: Option<u64>,
        volatile: bool,
    },
    Store {
        align: Option<u64>,
        volatile: bool,
    },
    /// An atomic access or a fence.
    ///
    /// DEVIATE A1 (no number): LLVM carries the ordering and the scope on
    /// `LoadInst`, `StoreInst`, `AtomicRMWInst`, `AtomicCmpXchgInst` and
    /// `FenceInst`; here one opcode does, so `Load` and `Store` (352
    /// construction sites) keep their fields. Lost: no load or store fold
    /// sees an atomic one (store-to-load forwarding, merging, SROA of an
    /// atomic access), which is the safe side. Exit: ordering fields on
    /// `Load` and `Store`. The pipeline sees calls instead (`atomic::hidden`),
    /// so no pass classifies an atomic.
    Atomic(AtomicInfo),
    GetElementPtr {
        source: TypeId,
    },
    ICmp(IntPredicate),
    FCmp(FloatPredicate),
    Phi,
    Select,
    Freeze,
    Call(Box<CallInfo>),
    LandingPad {
        cleanup: bool,
        clauses: Vec<Clause>,
    },
}

/// `==` inlined: a compare with a unit variant (`opcode == Opcode::Phi`) is
/// one discriminant test, where the derived `eq` was a call and the temporary's
/// drop glue (4% of a diamond-800 compile).
impl PartialEq for Opcode {
    /// A compare with a unit variant or one that carries a small enum
    /// (`Binary`, `Cast`, `ICmp`, `FCmp`) is a discriminant test and a byte
    /// compare in the caller; the variants with vectors and fields compare
    /// in `eq_fields`. The one `#[inline]` match was a call at most sites:
    /// 12.3 M Ir (1.4%) of a d_faces compile.
    #[inline(always)]
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        match (self, other) {
            (Self::Binary(a), Self::Binary(b)) => a == b,
            (Self::Cast(a), Self::Cast(b)) => a == b,
            (Self::ICmp(a), Self::ICmp(b)) => a == b,
            (Self::FCmp(a), Self::FCmp(b)) => a == b,
            _ => {
                std::mem::discriminant(self) == std::mem::discriminant(other)
                    && (!self.carries_fields() || self.eq_fields(other))
            }
        }
    }
}

impl Opcode {
    /// The variants whose payload is more than a small enum.
    #[inline(always)]
    fn carries_fields(&self) -> bool {
        matches!(
            self,
            Self::Invoke(_)
                | Self::Call(_)
                | Self::ExtractValue(_)
                | Self::InsertValue(_)
                | Self::Alloca { .. }
                | Self::Load { .. }
                | Self::Store { .. }
                | Self::Atomic(_)
                | Self::GetElementPtr { .. }
                | Self::LandingPad { .. }
        )
    }

    /// The payload compare of two opcodes of one variant.
    #[inline(never)]
    fn eq_fields(
        &self,
        other: &Self,
    ) -> bool {
        match (self, other) {
            (Self::Invoke(a), Self::Invoke(b)) | (Self::Call(a), Self::Call(b)) => a == b,
            (Self::ExtractValue(a), Self::ExtractValue(b)) | (Self::InsertValue(a), Self::InsertValue(b)) => a == b,
            (
                Self::Alloca { allocated: a, align: b, address_space: c },
                Self::Alloca { allocated: d, align: e, address_space: f },
            ) => a == d && b == e && c == f,
            (Self::Load { align: a, volatile: b }, Self::Load { align: c, volatile: d })
            | (Self::Store { align: a, volatile: b }, Self::Store { align: c, volatile: d }) => a == c && b == d,
            (Self::Atomic(a), Self::Atomic(b)) => a == b,
            (Self::GetElementPtr { source: a }, Self::GetElementPtr { source: b }) => a == b,
            (Self::LandingPad { cleanup: a, clauses: b }, Self::LandingPad { cleanup: c, clauses: d }) => {
                a == c && b == d
            }
            _ => true,
        }
    }

    pub fn is_terminator(&self) -> bool {
        matches!(
            self,
            Self::Ret | Self::Br | Self::Switch | Self::Invoke(_) | Self::Resume | Self::Unreachable
        )
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
            Self::Atomic(info) => match info.kind {
                AtomicKind::Load => "load",
                AtomicKind::Store => "store",
                AtomicKind::Rmw(_) => "atomicrmw",
                AtomicKind::CmpXchg { .. } => "cmpxchg",
                AtomicKind::Fence => "fence",
            },
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

/// LLVM's `fastcc`, the convention the compiler gives a function it sees every
/// caller of: here arguments pushed right to left and popped by the callee
/// (`ret N`), which a caller's `add sp,N` then does not repeat.
pub const FAST: u32 = 8;

#[cfg(test)]
mod tests {
    use super::*;

    /// `Opcode::eq` is hand-written (inlined) and must stay the derived one:
    /// equal only to itself, payload included.
    #[test]
    fn opcode_equality_compares_the_variant_and_its_payload() {
        let info = || {
            Box::new(CallInfo {
                function_type: crate::context::Context::default().types.void(),
                calling_convention: 0,
                return_attrs: Vec::new(),
                argument_attrs: Vec::new(),
                attrs: Vec::new(),
                tail: Tail::None,
            })
        };
        let ops = [
            Opcode::Ret,
            Opcode::Br,
            Opcode::Phi,
            Opcode::Select,
            Opcode::Freeze,
            Opcode::Binary(BinaryOp::Add),
            Opcode::Binary(BinaryOp::Sub),
            Opcode::Cast(CastOp::ZExt),
            Opcode::Cast(CastOp::SExt),
            Opcode::ICmp(IntPredicate::Eq),
            Opcode::ICmp(IntPredicate::Ne),
            Opcode::Load { align: None, volatile: false },
            Opcode::Load { align: Some(2), volatile: false },
            Opcode::Load { align: None, volatile: true },
            Opcode::Store { align: None, volatile: false },
            Opcode::ExtractValue(vec![0]),
            Opcode::ExtractValue(vec![1]),
            Opcode::InsertValue(vec![0]),
            Opcode::Call(info()),
            Opcode::Invoke(info()),
            Opcode::LandingPad { cleanup: false, clauses: vec![] },
            Opcode::LandingPad { cleanup: true, clauses: vec![] },
        ];
        for (a, x) in ops.iter().enumerate() {
            for (b, y) in ops.iter().enumerate() {
                assert_eq!(x == y, a == b, "{x:?} against {y:?}");
            }
        }
    }
}
