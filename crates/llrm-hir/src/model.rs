//! Port of `qbopt/hir/model.py`: typed source semantics shared by source
//! frontends.
//!
//! This is intentionally not an AST and not another optimizer.  Frontends
//! finish name and type resolution before constructing it; `hir::lower`
//! turns it directly into the existing MIR.

use std::fmt;

use llrm_support::pyrepr::{self, Repr};

pub const SCHEMA_VERSION: i64 = 1;

/// A Python `StrEnum`: members, their values, `str()` and `repr()`.
macro_rules! str_enum {
    ($name:ident { $($variant:ident($member:literal) = $value:literal,)* }) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name {
            $($variant,)*
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];
            pub const VALUES: &'static [&'static str] = &[$($value,)*];

            pub const fn value(self) -> &'static str {
                match self {
                    $(Self::$variant => $value,)*
                }
            }

            pub const fn member(self) -> &'static str {
                match self {
                    $(Self::$variant => $member,)*
                }
            }

            /// `Class(value)`.
            pub fn from_value(value: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|one| one.value() == value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.value())
            }
        }

        impl Repr for $name {
            fn repr(&self) -> String {
                pyrepr::str_enum(stringify!($name), self.member(), self.value())
            }
        }
    };
}

str_enum!(RuntimeProfile {
    Qb45("QB45") = "qb45",
    Pds71("PDS71") = "pds71",
    Vbdos("VBDOS") = "vbdos",
    Freestanding("FREESTANDING") = "freestanding",
});

str_enum!(Dialect {
    Qbasic11("QBASIC11") = "qbasic11",
    Qb45("QB45") = "qb45",
    Pds71("PDS71") = "pds71",
    Vbdos("VBDOS") = "vbdos",
    Quickr("QUICKR") = "quickr",
    Nib("NIB") = "nib",
});

str_enum!(TargetProfile {
    I386RealMode("I386_REAL_MODE") = "i386-real-mode",
});

str_enum!(ArrayOrder {
    ColumnMajor("COLUMN_MAJOR") = "column-major",
    RowMajor("ROW_MAJOR") = "row-major",
});

str_enum!(FloatMode {
    Inline("INLINE") = "inline",
    Alternate("ALTERNATE") = "alternate",
});

str_enum!(TypeKind {
    Void("VOID") = "void",
    Boolean("BOOLEAN") = "boolean",
    Integer("INTEGER") = "integer",
    Float("FLOAT") = "float",
    Array("ARRAY") = "array",
    Pointer("POINTER") = "pointer",
    Opaque("OPAQUE") = "opaque",
});

str_enum!(AddressKind {
    None("NONE") = "none",
    Near("NEAR") = "near",
    Far("FAR") = "far",
    Huge("HUGE") = "huge",
    Code("CODE") = "code",
    // A 16-bit protected/real-mode segment selector, without an offset.
    Segment("SEGMENT") = "segment",
});

str_enum!(FloatEvaluation {
    None("NONE") = "none",
    Binary32("BINARY32") = "binary32",
    Binary64("BINARY64") = "binary64",
    Extended80("EXTENDED80") = "extended80",
});

str_enum!(StackCleanup {
    Caller("CALLER") = "caller",
    Callee("CALLEE") = "callee",
});

// Where a procedure's float result goes: stored through a near pointer the
// caller passes last, which comes back in `ax` (BASIC's, the default), or
// in `st(0)`, as C and Pascal return it.
str_enum!(FloatReturn {
    Pointer("POINTER") = "pointer",
    Register("REGISTER") = "register",
});

str_enum!(CallDistance {
    Near("NEAR") = "near",
    Far("FAR") = "far",
    // Entered by INT or an IRQ, left by `iret`.
    Interrupt("INTERRUPT") = "interrupt",
});

#[derive(Clone, Debug, PartialEq)]
pub struct Type {
    pub id: i64,
    pub name: String,
    pub kind: TypeKind,
    pub width: i64,
    pub signed: Option<bool>,
    pub evaluation: FloatEvaluation,
    pub element: Option<i64>,
    pub rank: i64,
    pub bounds: Vec<(i64, i64)>,
    pub address: AddressKind,
}

impl Type {
    /// Python's four-required-field construction with every default.
    pub fn new(id: i64, name: &str, kind: TypeKind, width: i64) -> Self {
        Self {
            id,
            name: name.to_owned(),
            kind,
            width,
            signed: None,
            evaluation: FloatEvaluation::None,
            element: None,
            rank: 0,
            bounds: Vec::new(),
            address: AddressKind::None,
        }
    }
}

str_enum!(Storage {
    Local("LOCAL") = "local",
    Parameter("PARAMETER") = "parameter",
    Static("STATIC") = "static",
    Module("MODULE") = "module",
    Common("COMMON") = "common",
    External("EXTERNAL") = "external",
});

str_enum!(DataLinkage {
    Internal("INTERNAL") = "internal",
    External("EXTERNAL") = "external",
});

str_enum!(FunctionLinkage {
    Internal("INTERNAL") = "internal",
    External("EXTERNAL") = "external",
});

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub id: i64,
    pub name: String,
    pub r#type: i64,
    pub storage: Storage,
    pub offset: i64,
    pub symbol: i64,
    pub extent: Option<i64>,
    pub address: AddressKind,
    pub volatile: bool,
}

impl Place {
    /// Python's five-required-field construction with every default.
    pub fn new(id: i64, name: &str, r#type: i64, storage: Storage, offset: i64) -> Self {
        Self {
            id,
            name: name.to_owned(),
            r#type,
            storage,
            offset,
            symbol: 0,
            extent: None,
            address: AddressKind::Near,
            volatile: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    pub id: i64,
    pub r#type: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValueRef {
    pub value: i64,
}

/// Python's `int | float`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Constant {
    pub r#type: i64,
    pub value: Number,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaceRef {
    pub place: i64,
}

/// `indices` holds `ValueRef | Constant` operands only.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrayElement {
    pub place: i64,
    pub indices: Vec<Operand>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedPlace {
    pub place: i64,
    pub indices: Vec<Operand>,
    pub offset: i64,
    pub r#type: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IndirectPlace {
    pub base: i64,
    pub offset: i64,
    pub r#type: i64,
    pub volatile: bool,
    /// The language promises the access stays inside one object.
    pub inbounds: bool,
    /// The value holding the offset of that object's first byte, where the
    /// frontend knows it: the pointer's offset plus `offset` is then that
    /// value plus a non-negative offset inside the object, and the object
    /// ends inside its segment.
    pub origin: Option<i64>,
}

str_enum!(DescriptorField {
    Length("LENGTH") = "length",
    Capacity("CAPACITY") = "capacity",
});

#[derive(Clone, Debug, PartialEq)]
pub struct DescriptorPlace {
    pub base: i64,
    pub field: DescriptorField,
    pub r#type: i64,
}

impl DescriptorPlace {
    /// The field's offset from the base pointer, whose pointee is
    /// `pointee`: a scoped view (`$slice[..]`) holds its length then its
    /// capacity, and a heap string's header, the same two, precedes its data.
    pub fn offset(&self, pointee: Option<&Type>) -> i64 {
        let view = pointee.is_some_and(|one| one.kind == TypeKind::Opaque && one.name.starts_with("$slice["));
        match (view, self.field) {
            (true, DescriptorField::Length) => 0,
            (true, DescriptorField::Capacity) => 2,
            (false, DescriptorField::Length) => -4,
            (false, DescriptorField::Capacity) => -2,
        }
    }
}

/// Python's `type Operand = ValueRef | Constant | ...`.
#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    ValueRef(ValueRef),
    Constant(Constant),
    PlaceRef(PlaceRef),
    ArrayElement(ArrayElement),
    ProjectedPlace(ProjectedPlace),
    IndirectPlace(IndirectPlace),
    DescriptorPlace(DescriptorPlace),
}

impl Operand {
    /// `ValueRef(value)`.
    pub fn value_ref(value: i64) -> Self {
        Self::ValueRef(ValueRef { value })
    }

    /// `Constant(type, value)` with an integer value.
    pub fn constant(r#type: i64, value: i64) -> Self {
        Self::Constant(Constant { r#type, value: Number::Int(value) })
    }

    /// `PlaceRef(place)`.
    pub fn place_ref(place: i64) -> Self {
        Self::PlaceRef(PlaceRef { place })
    }
}

str_enum!(Op {
    Copy("COPY") = "copy",
    Load("LOAD") = "load",
    Store("STORE") = "store",
    Address("ADDRESS") = "address",
    PtrOffset("PTR_OFFSET") = "ptr_offset",
    PointerSegment("POINTER_SEGMENT") = "pointer_segment",
    PointerOffset("POINTER_OFFSET") = "pointer_offset",
    Concat("CONCAT") = "concat",
    Convert("CONVERT") = "convert",
    // A float to an integer, rounded toward zero; CONVERT rounds as the environment does.
    Truncate("TRUNCATE") = "truncate",
    SignExtend("SIGN_EXTEND") = "sign_extend",
    ZeroExtend("ZERO_EXTEND") = "zero_extend",
    Add("ADD") = "add",
    Sub("SUB") = "sub",
    Mul("MUL") = "mul",
    // Fixed-point scaling stays semantic through MIR.  Expanding fixed i32
    // here into generic i64 arithmetic loses that both inputs are narrow and
    // makes the target legalize a 32x32 product as an arbitrary 64x64 one.
    FixedMul("FIXED_MUL") = "fixed_mul",
    FixedDiv("FIXED_DIV") = "fixed_div",
    Div("DIV") = "div",
    Rem("REM") = "rem",
    Divmod("DIVMOD") = "divmod",
    Udiv("UDIV") = "udiv",
    Urem("UREM") = "urem",
    Udivmod("UDIVMOD") = "udivmod",
    And("AND") = "and",
    Or("OR") = "or",
    Xor("XOR") = "xor",
    Shl("SHL") = "shl",
    Shr("SHR") = "shr",
    Sar("SAR") = "sar",
    Neg("NEG") = "neg",
    Not("NOT") = "not",
    Eq("EQ") = "eq",
    Ne("NE") = "ne",
    Lt("LT") = "lt",
    Le("LE") = "le",
    Gt("GT") = "gt",
    Ge("GE") = "ge",
    Below("BELOW") = "below",
    BelowEq("BELOW_EQ") = "beloweq",
    Above("ABOVE") = "above",
    AboveEq("ABOVE_EQ") = "aboveeq",
    StringEq("STRING_EQ") = "string_eq",
    StringNe("STRING_NE") = "string_ne",
    StringLt("STRING_LT") = "string_lt",
    StringLe("STRING_LE") = "string_le",
    StringGt("STRING_GT") = "string_gt",
    StringGe("STRING_GE") = "string_ge",
    Fadd("FADD") = "fadd",
    Fsub("FSUB") = "fsub",
    Fmul("FMUL") = "fmul",
    Fdiv("FDIV") = "fdiv",
    Fneg("FNEG") = "fneg",
    Fabs("FABS") = "fabs",
    Fsqrt("FSQRT") = "fsqrt",
    Fsin("FSIN") = "fsin",
    Fcos("FCOS") = "fcos",
    Fatan("FATAN") = "fatan",
    Flog2("FLOG2") = "flog2",
    Fexp2("FEXP2") = "fexp2",
    // A float rounded to an integral float as the environment rounds: x87 FRNDINT.
    Fround("FROUND") = "fround",
    // An I/O port: port_in reads a byte from operands[0]; port_out writes
    // operands[1], a byte, to operands[0]. Both are observable and ordered.
    PortIn("PORT_IN") = "port_in",
    PortOut("PORT_OUT") = "port_out",
    Call("CALL") = "call",
    // Inline machine code: operands go into its input registers, results
    // come out of its output registers. `Instruction.asm` says which.
    Asm("ASM") = "asm",
});

/// An inline block's code and constraints, registers named by their 16-bit whole.
#[derive(Clone, Debug, PartialEq)]
pub struct Asm {
    pub code: Vec<i64>,
    // One per operand, in order.
    pub inputs: Vec<String>,
    // One per result, in order.
    pub outputs: Vec<String>,
    // Registers it changes besides its outputs; `flags` among them.
    pub clobbers: Vec<String>,
    // Whether it reads or writes memory.
    pub memory: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub id: i64,
    pub op: Op,
    pub results: Vec<i64>,
    pub operands: Vec<Operand>,
    pub callee: Option<String>,
    pub pure: bool,
    pub asm: Option<Asm>,
}

impl Instruction {
    /// Python's `Instruction(id, op, results, operands)` with the remaining
    /// defaults.
    pub fn new(id: i64, op: Op, results: Vec<i64>, operands: Vec<Operand>) -> Self {
        Self { id, op, results, operands, callee: None, pure: false, asm: None }
    }
}

str_enum!(TerminatorKind {
    Jump("JUMP") = "jump",
    Branch("BRANCH") = "branch",
    Switch("SWITCH") = "switch",
    Return("RETURN") = "return",
    Unreachable("UNREACHABLE") = "unreachable",
});

#[derive(Clone, Debug, PartialEq)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub operands: Vec<Operand>,
    pub targets: Vec<i64>,
    pub cases: Vec<(i64, i64)>,
}

impl Terminator {
    pub fn new(kind: TerminatorKind, operands: Vec<Operand>, targets: Vec<i64>) -> Self {
        Self { kind, operands, targets, cases: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub id: i64,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
    // The frontend expects this block never to run; see mir.MirBlock.cold.
    pub cold: bool,
}

impl Block {
    pub fn new(id: i64, instructions: Vec<Instruction>, terminator: Terminator) -> Self {
        Self { id, instructions, terminator, cold: false }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallAbi {
    pub instruction: i64,
    pub order: Vec<i64>,
    pub cleanup: StackCleanup,
    pub distance: CallDistance,
    pub callee: Option<i64>,
    pub float_return: FloatReturn,
}

/// One resolved language procedure symbol; calls refer to its stable id.
#[derive(Clone, Debug, PartialEq)]
pub struct Callable {
    pub id: i64,
    pub name: String,
    pub result_type: Option<i64>,
    pub parameter_types: Vec<i64>,
    pub by_value: Vec<bool>,
    pub segmented: Vec<bool>,
    pub arrays: Vec<bool>,
    pub defined: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcedureAbi {
    pub cleanup: StackCleanup,
    pub distance: CallDistance,
    pub parameter_bytes: i64,
    pub float_return: FloatReturn,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub id: i64,
    pub name: String,
    pub result_type: i64,
    pub values: Vec<Value>,
    pub places: Vec<Place>,
    pub blocks: Vec<Block>,
    pub entry: i64,
    pub parameters: Vec<i64>,
    pub abi: Option<ProcedureAbi>,
    pub calls: Vec<CallAbi>,
    pub error_handler: Option<i64>,
    pub error_handler_local: bool,
    pub external_entries: Vec<i64>,
    pub linkage: FunctionLinkage,
}

impl Function {
    /// Python's seven-required-field construction with every default.
    pub fn new(
        id: i64,
        name: &str,
        result_type: i64,
        values: Vec<Value>,
        places: Vec<Place>,
        blocks: Vec<Block>,
        entry: i64,
    ) -> Self {
        Self {
            id,
            name: name.to_owned(),
            result_type,
            values,
            places,
            blocks,
            entry,
            parameters: Vec::new(),
            abi: None,
            calls: Vec::new(),
            error_handler: None,
            error_handler_local: false,
            external_entries: Vec::new(),
            linkage: FunctionLinkage::External,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataRelocation {
    pub at: i64,
    pub target: i64,
    pub addend: i64,
    pub address: AddressKind,
    /// `target` is a callable, whose code this addresses, not data.
    pub code: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataObject {
    pub id: i64,
    pub name: String,
    pub bytes: Vec<i64>,
    pub readonly: bool,
    pub relocations: Vec<DataRelocation>,
    pub linkage: DataLinkage,
    // Placement class, independently of mutability. Near objects participate
    // in DGROUP; far/huge objects live in a separately addressed segment.
    pub address: AddressKind,
    // False when no code takes its address, this module's or another's: only
    // a reference naming it reaches it.
    pub addressed: bool,
}

impl DataObject {
    pub fn new(id: i64, name: &str, bytes: Vec<i64>) -> Self {
        Self {
            id,
            name: name.to_owned(),
            bytes,
            readonly: false,
            relocations: Vec::new(),
            linkage: DataLinkage::Internal,
            address: AddressKind::Near,
            addressed: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub id: i64,
    pub name: String,
    pub types: Vec<Type>,
    pub functions: Vec<Function>,
    pub data: Vec<DataObject>,
    pub callables: Vec<Callable>,
}

impl Module {
    pub fn new(id: i64, name: &str, types: Vec<Type>, functions: Vec<Function>) -> Self {
        Self { id, name: name.to_owned(), types, functions, data: Vec::new(), callables: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub dialect: Dialect,
    pub runtime: RuntimeProfile,
    pub modules: Vec<Module>,
    pub schema: i64,
    pub target: TargetProfile,
    pub array_order: ArrayOrder,
    pub float_mode: FloatMode,
}

impl Program {
    pub fn new(dialect: Dialect, runtime: RuntimeProfile, modules: Vec<Module>) -> Self {
        Self {
            dialect,
            runtime,
            modules,
            schema: SCHEMA_VERSION,
            target: TargetProfile::I386RealMode,
            array_order: ArrayOrder::ColumnMajor,
            float_mode: FloatMode::Inline,
        }
    }
}
