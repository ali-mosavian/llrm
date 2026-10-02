//! Port of `qbopt/hir/model.py`: typed source semantics shared by source
//! frontends.
//!
//! This is intentionally not an AST and not another optimizer.  Frontends
//! finish name and type resolution before constructing it; `hir::lower`
//! turns it directly into the existing MIR.

use std::fmt;

use llrm_support::pyrepr::{self, Repr};

pub const SCHEMA_VERSION: i64 = 5;

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

impl RuntimeProfile {
    /// Whether RESUME clears ERL. Measured through BC's route: QB 4.5's
    /// BCOM45 prints the handled line after RESUME, PDS 7.1's and VBDOS's,
    /// whose error state is kept in the user frame, print 0.
    pub fn resume_clears_erl(self) -> bool {
        self != Self::Qb45
    }
}

str_enum!(Dialect {
    Qbasic11("QBASIC11") = "qbasic11",
    Qb45("QB45") = "qb45",
    Pds71("PDS71") = "pds71",
    Vbdos("VBDOS") = "vbdos",
    Quickr("QUICKR") = "quickr",
    Nib("NIB") = "nib",
    C("C") = "c",
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

// Whether a float value must be rounded to its declared format where the
// language says so, or may keep the machine's precision until it is stored.
str_enum!(FloatSemantics {
    Declared("DECLARED") = "declared",
    Machine("MACHINE") = "machine",
});

// Who frames a procedure: the runtime (BASIC's B$ENRA, which zeroes its
// locals), or the procedure itself wherever the runtime needs no frame of
// its own, its HIR storing zero to each local that needs it.
str_enum!(Frames {
    Runtime("RUNTIME") = "runtime",
    Own("OWN") = "own",
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
// caller passes last, which comes back in `ax` (BASIC's, the default); the
// near address of the callee's own copy, in `ax` (Microsoft C's, which
// BASIC's CDECL is); or in `st(0)`, as Borland C and Pascal return it.
str_enum!(FloatReturn {
    Pointer("POINTER") = "pointer",
    Address("ADDRESS") = "address",
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
    // A parameter in the memory it was passed in, `symbol` its value:
    // `offset` bytes from a variadic function's first variadic argument, or
    // from the first register an interrupt handler saved.
    Parameter("PARAMETER") = "parameter",
    Static("STATIC") = "static",
    Module("MODULE") = "module",
    Common("COMMON") = "common",
    External("EXTERNAL") = "external",
});

str_enum!(DataLinkage {
    Internal("INTERNAL") = "internal",
    External("EXTERNAL") = "external",
    // Defined here, visible to other modules.
    Exported("EXPORTED") = "exported",
    // Internal, and nameless: a literal.
    Private("PRIVATE") = "private",
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
    /// The value holding the offset of that object's first byte, where the
    /// frontend knows it: the pointer's offset plus `offset` is then that
    /// value plus a non-negative offset inside the object, and the object
    /// ends inside its segment.
    pub origin: Option<i64>,
    /// The descriptor place owning the far allocation the access stays
    /// inside, where the frontend knows it: disjoint from every place.
    pub allocation: Option<i64>,
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
    // A place's address; with no operand, the address of the function `callee` names.
    Address("ADDRESS") = "address",
    PtrOffset("PTR_OFFSET") = "ptr_offset",
    // The bytes between two huge pointers into one object.
    PtrDiff("PTR_DIFF") = "ptr_diff",
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
    // The language promises operands[0], a condition, holds here: passes may
    // rely on it, as on LLVM's `llvm.assume`. No result.
    Assume("ASSUME") = "assume",
    // Calls `callee`; with none, the function operands[0] points to.
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
    /// The source line of the statement it belongs to, where known.
    pub line: Option<i64>,
}

impl Instruction {
    /// Python's `Instruction(id, op, results, operands)` with the remaining
    /// defaults.
    pub fn new(id: i64, op: Op, results: Vec<i64>, operands: Vec<Operand>) -> Self {
        Self { id, op, results, operands, callee: None, pure: false, asm: None, line: None }
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
    /// The name it links by, where not the language's own for `name`.
    pub symbol: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcedureAbi {
    pub cleanup: StackCleanup,
    pub distance: CallDistance,
    pub parameter_bytes: i64,
    pub float_return: FloatReturn,
    /// It takes arguments past its parameters, as C's `...` does.
    pub variadic: bool,
}

impl ProcedureAbi {
    /// The parameter a floating `result` is stored through: the last, when
    /// it points at the result's type and the ABI returns floats by pointer.
    pub fn result_destination(&self, result: &Type, parameters: &[&Type]) -> Option<usize> {
        let last = parameters.len().checked_sub(1)?;
        let points = parameters[last].kind == TypeKind::Pointer && parameters[last].element == Some(result.id);
        (points && self.float_return.leaves(FloatReturn::Pointer, result)).then_some(last)
    }
}

impl CallAbi {
    /// Whether the callee returns a floating `result` through the
    /// destination this call passes last, the pointer to it coming back.
    pub fn returns_through(&self, result: &Type) -> bool {
        self.float_return.leaves(FloatReturn::Pointer, result)
    }

    /// Whether the callee returns a floating `result` as the address of its
    /// own copy.
    pub fn returns_address(&self, result: &Type) -> bool {
        self.float_return.leaves(FloatReturn::Address, result)
    }
}

impl FloatReturn {
    /// Whether a `result` of this ABI leaves as `how` says: only a float's does.
    fn leaves(self, how: FloatReturn, result: &Type) -> bool {
        self == how && result.kind == TypeKind::Float
    }
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
    /// The name it links by, where not its own.
    pub symbol: Option<String>,
}

/// The debug vocabulary, as MIR's metadata spells it.
pub use llrm_mir::debuginfo::{Kind as DebugKind, Reach as DebugReach, Scalar as DebugScalar};

/// A source type, as a debugger shows it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugType {
    pub id: i64,
    pub kind: DebugKind,
    pub name: String,
    pub target: Option<i64>,
    pub size: i64,
    pub reach: DebugReach,
    pub members: Vec<DebugMember>,
}

/// A structure's field, or a procedure's parameter by its type alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugMember {
    pub name: String,
    pub r#type: i64,
    pub offset: i64,
    /// A bit field's first bit in the unit at `offset`, and its width.
    pub bit_start: Option<i64>,
    pub bit_width: Option<i64>,
}

/// A parameter: the function's `argument`th, hidden ones counted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugParameter {
    pub argument: i64,
    pub name: String,
    pub r#type: i64,
}

/// A variable: a place of the function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugVariable {
    pub place: i64,
    pub name: String,
    pub r#type: i64,
}

/// A function as a debugger names it: its procedure type, its source
/// parameters, and its variables; `module` the module's own code, whose
/// variables are the module's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugFunction {
    pub function: i64,
    pub module: bool,
    pub name: String,
    pub r#type: i64,
    pub parameters: Vec<DebugParameter>,
    pub variables: Vec<DebugVariable>,
}

/// A variable in data: `offset` bytes into a data object; `function` the
/// one declaring it, None for the module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugGlobal {
    pub function: Option<i64>,
    pub object: i64,
    pub offset: i64,
    pub name: String,
    pub r#type: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Debug {
    pub types: Vec<DebugType>,
    pub functions: Vec<DebugFunction>,
    pub globals: Vec<DebugGlobal>,
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
            symbol: None,
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
    // The segment the frontend places it in. Its alignment is a stated fact.
    pub segment: Option<String>,
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
            segment: None,
        }
    }
}

/// A class of the language's type-based aliasing, as LLVM's `!tbaa` type
/// nodes: an access as one of `types` aliases only accesses in this class,
/// an ancestor or a descendant. A class with no parent is a root.
#[derive(Clone, Debug, PartialEq)]
pub struct AliasClass {
    pub name: String,
    pub parent: Option<String>,
    pub types: Vec<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub id: i64,
    pub name: String,
    pub types: Vec<Type>,
    pub functions: Vec<Function>,
    pub data: Vec<DataObject>,
    pub callables: Vec<Callable>,
    pub alias_classes: Vec<AliasClass>,
    /// What the language promises, as the frontend stated it.
    pub facts: Vec<crate::facts::Stated>,
    /// `-g`: what a debugger names and how it reads it.
    pub debug: Option<Debug>,
    /// Each source line's BASIC line number, where a statement table
    /// reports one: (line, number).
    pub line_numbers: Vec<(i64, i64)>,
}

impl Module {
    pub fn new(id: i64, name: &str, types: Vec<Type>, functions: Vec<Function>) -> Self {
        Self { id, name: name.to_owned(), types, functions, data: Vec::new(), callables: Vec::new(), alias_classes: Vec::new(), facts: Vec::new(), debug: None, line_numbers: Vec::new() }
    }

    /// The rows of the statement table, in source order; none without one.
    pub fn statements(&self) -> Result<Vec<Statement>, String> {
        let objects: Vec<&DataObject> = self.data.iter().filter(|one| one.name == STATEMENT_TABLE).collect();
        let object = match objects[..] {
            [] => return Ok(Vec::new()),
            [one] => one,
            _ => return Err("more than one statement table".to_owned()),
        };
        if object.linkage != DataLinkage::Internal || !object.readonly || !object.relocations.is_empty() || object.address != AddressKind::Near || object.bytes.len() % 14 != 0 {
            return Err("the statement table has an invalid storage contract".to_owned());
        }
        let bytes: Vec<u8> = object.bytes.iter().map(|&one| one as u8).collect();
        let word = |at: usize, size: usize| bytes[at..at + size].iter().rev().fold(0i64, |sum, &byte| (sum << 8) | i64::from(byte));
        Ok((0..bytes.len()).step_by(14).map(|at| Statement { function: word(at, 4), block: word(at + 4, 4), instruction: word(at + 8, 4), line: word(at + 12, 2) }).collect())
    }

    /// Whether an error `function` raises lands in it: it has a handler of
    /// its own, or the module body's ON ERROR GOTO takes every procedure's
    /// error on that procedure's frame.
    pub fn lands_errors(&self, function: &Function) -> bool {
        function.error_handler.is_some() || self.functions.iter().any(|one| one.error_handler.is_some() && !one.error_handler_local)
    }
}

/// The internal data object whose rows are where RESUME may continue.
pub const STATEMENT_TABLE: &str = "$qb$statementTable";

/// A statement the runtime may RESUME at: its function, the block it
/// starts, its first instruction and its BASIC line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Statement {
    pub function: i64,
    pub block: i64,
    pub instruction: i64,
    pub line: i64,
}

/// A runtime cell only a reference naming it reaches, and the routines
/// that write it.
#[derive(Clone, Debug, PartialEq)]
pub struct CellWriters {
    pub cell: String,
    pub routines: Vec<String>,
}

/// What the runtime the program links against promises of its routines,
/// which their declarations state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimePromises {
    /// The routines that may run the program's own code; any other runs
    /// none. None: every one may.
    pub calling_back: Option<Vec<String>>,
    /// Each named cell's writers.
    pub writers: Vec<CellWriters>,
    /// The routines that raise no error.
    pub nounwind: Vec<String>,
    /// The routines that only read what their pointer arguments reach and
    /// keep none of them, by their own names: C's strlen.
    pub reads_arguments: Vec<String>,
}

impl RuntimePromises {
    /// The promises of a runtime whose `calling_back` routines may run the
    /// program's code, whose named cells `writers` write, and whose
    /// `nounwind` routines raise no error.
    pub fn of<'a, 'b, 'c, W: IntoIterator<Item = &'b str>>(
        calling_back: impl IntoIterator<Item = &'a str>,
        writers: impl IntoIterator<Item = (&'b str, W)>,
        nounwind: impl IntoIterator<Item = &'c str>,
    ) -> Self {
        Self {
            calling_back: Some(calling_back.into_iter().map(str::to_owned).collect()),
            writers: writers.into_iter().map(|(cell, routines)| CellWriters { cell: cell.to_owned(), routines: routines.into_iter().map(str::to_owned).collect() }).collect(),
            nounwind: nounwind.into_iter().map(str::to_owned).collect(),
            reads_arguments: Vec::new(),
        }
    }

    /// The named cells `routine` writes; none where it may run the
    /// program's code.
    pub fn writes(&self, routine: &str) -> Option<Vec<String>> {
        if self.calling_back.as_ref()?.iter().any(|one| one == routine) {
            return None;
        }
        Some(self.writers.iter().filter(|one| one.routines.iter().any(|writer| writer == routine)).map(|one| one.cell.clone()).collect())
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
    pub float_semantics: FloatSemantics,
    /// A frame's locals start zeroed; false where the language leaves them
    /// indeterminate.
    pub zeroed_locals: bool,
    pub frames: Frames,
    pub promises: RuntimePromises,
    /// The functions code outside the program calls whatever their
    /// linkage: the runtime's way into it.
    pub entries: Vec<String>,
    /// The registers, by their 16-bit whole, a call keeps where no runtime
    /// contract says otherwise: the calling convention's.
    pub preserved: Vec<String>,
    /// The segment of the constants the compiler makes; None: the default
    /// data segment.
    pub constant_segment: Option<String>,
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
            float_semantics: FloatSemantics::Declared,
            zeroed_locals: true,
            frames: Frames::Runtime,
            promises: RuntimePromises::default(),
            entries: Vec::new(),
            preserved: Vec::new(),
            constant_segment: None,
        }
    }
}
