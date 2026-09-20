use std::fmt;

pub const FORMAT_VERSION: u32 = 1;

macro_rules! entity_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

entity_id!(ModuleId);
entity_id!(TypeId);
entity_id!(FunctionId);
entity_id!(CallableId);
entity_id!(DataId);
entity_id!(BlockId);
entity_id!(InstructionId);
entity_id!(ValueId);
entity_id!(PlaceId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dialect {
    Qbasic11,
    Qb45,
    Pds71,
    Vbdos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeProfile {
    Qb45,
    Pds71,
    Vbdos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetProfile {
    I386RealMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrayOrder {
    ColumnMajor,
    RowMajor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatMode {
    Inline,
    Alternate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub version: u32,
    pub dialect: Dialect,
    pub runtime: RuntimeProfile,
    pub target: TargetProfile,
    pub array_order: ArrayOrder,
    pub float_mode: FloatMode,
    pub modules: Vec<Module>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub id: ModuleId,
    pub name: String,
    pub types: Vec<Type>,
    pub functions: Vec<Function>,
    pub data: Vec<DataObject>,
    pub callables: Vec<Callable>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Void,
    Boolean,
    Integer,
    Float,
    Array,
    Pointer,
    Opaque,
}

impl TypeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Array => "array",
            Self::Pointer => "pointer",
            Self::Opaque => "opaque",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AddressKind {
    None,
    Near,
    Far,
    Huge,
    Code,
    Segment,
}

impl AddressKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Near => "near",
            Self::Far => "far",
            Self::Huge => "huge",
            Self::Code => "code",
            Self::Segment => "segment",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatEvaluation {
    None,
    Binary32,
    Binary64,
    Extended80,
}

impl FloatEvaluation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Binary32 => "binary32",
            Self::Binary64 => "binary64",
            Self::Extended80 => "extended80",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Type {
    pub id: TypeId,
    pub name: String,
    pub kind: TypeKind,
    pub width: usize,
    pub signed: Option<bool>,
    pub evaluation: FloatEvaluation,
    pub element: Option<TypeId>,
    pub bounds: Vec<(i64, i64)>,
    pub address: AddressKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Storage {
    Local,
    Parameter,
    Static,
    Module,
    Common,
    External,
}

impl Storage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Parameter => "parameter",
            Self::Static => "static",
            Self::Module => "module",
            Self::Common => "common",
            Self::External => "external",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Linkage {
    Internal,
    External,
}

impl Linkage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::External => "external",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub id: PlaceId,
    pub name: String,
    pub type_id: TypeId,
    pub storage: Storage,
    pub offset: isize,
    pub symbol: DataId,
    pub extent: usize,
    pub address: AddressKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub id: ValueId,
    pub type_id: TypeId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConstantValue {
    Integer(i64),
    Real(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    Value(ValueId),
    Constant {
        type_id: TypeId,
        value: ConstantValue,
    },
    Place(PlaceId),
    Element {
        place: PlaceId,
        indices: Vec<Operand>,
    },
    Projection {
        place: PlaceId,
        indices: Vec<Operand>,
        offset: usize,
        type_id: TypeId,
    },
    Indirect {
        base: ValueId,
        offset: usize,
        type_id: TypeId,
        volatile: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Opcode {
    Copy,
    Load,
    Store,
    Address,
    OffsetPointer,
    PointerOffset,
    PointerSegment,
    Concat,
    Convert,
    SignExtend,
    ZeroExtend,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    DivideRemainder,
    And,
    Or,
    Xor,
    ShiftLeft,
    ShiftRight,
    ShiftRightArithmetic,
    Negate,
    Not,
    Equal,
    NotEqual,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
    StringEqual,
    StringNotEqual,
    StringLessThan,
    StringLessEqual,
    StringGreaterThan,
    StringGreaterEqual,
    FloatAdd,
    FloatSubtract,
    FloatMultiply,
    FloatDivide,
    FloatNegate,
    FloatAbsolute,
    FloatSquareRoot,
    FloatSine,
    FloatCosine,
    FloatArctangent,
    FloatLog2,
    FloatExp2,
    Call,
}

impl Opcode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Load => "load",
            Self::Store => "store",
            Self::Address => "address",
            Self::OffsetPointer => "ptr_offset",
            Self::PointerOffset => "pointer_offset",
            Self::PointerSegment => "pointer_segment",
            Self::Concat => "concat",
            Self::Convert => "convert",
            Self::SignExtend => "sign_extend",
            Self::ZeroExtend => "zero_extend",
            Self::Add => "add",
            Self::Subtract => "sub",
            Self::Multiply => "mul",
            Self::Divide => "div",
            Self::Remainder => "rem",
            Self::DivideRemainder => "divmod",
            Self::And => "and",
            Self::Or => "or",
            Self::Xor => "xor",
            Self::ShiftLeft => "shl",
            Self::ShiftRight => "shr",
            Self::ShiftRightArithmetic => "sar",
            Self::Negate => "neg",
            Self::Not => "not",
            Self::Equal => "eq",
            Self::NotEqual => "ne",
            Self::LessThan => "lt",
            Self::LessEqual => "le",
            Self::GreaterThan => "gt",
            Self::GreaterEqual => "ge",
            Self::StringEqual => "string_eq",
            Self::StringNotEqual => "string_ne",
            Self::StringLessThan => "string_lt",
            Self::StringLessEqual => "string_le",
            Self::StringGreaterThan => "string_gt",
            Self::StringGreaterEqual => "string_ge",
            Self::FloatAdd => "fadd",
            Self::FloatSubtract => "fsub",
            Self::FloatMultiply => "fmul",
            Self::FloatDivide => "fdiv",
            Self::FloatNegate => "fneg",
            Self::FloatAbsolute => "fabs",
            Self::FloatSquareRoot => "fsqrt",
            Self::FloatSine => "fsin",
            Self::FloatCosine => "fcos",
            Self::FloatArctangent => "fatan",
            Self::FloatLog2 => "flog2",
            Self::FloatExp2 => "fexp2",
            Self::Call => "call",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub id: InstructionId,
    pub opcode: Opcode,
    pub results: Vec<ValueId>,
    pub operands: Vec<Operand>,
    pub callee: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        condition: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    Switch {
        selector: Operand,
        cases: Vec<(i64, BlockId)>,
        default: BlockId,
    },
    Return(Option<Operand>),
    Unreachable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub id: BlockId,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackCleanup {
    Caller,
    Callee,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallDistance {
    Near,
    Far,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallAbi {
    pub instruction: InstructionId,
    pub order: Vec<usize>,
    pub cleanup: StackCleanup,
    pub distance: CallDistance,
    pub callee: Option<CallableId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub type_id: TypeId,
    pub by_value: bool,
    pub segmented: bool,
    pub array: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callable {
    pub id: CallableId,
    pub name: String,
    pub result_type: Option<TypeId>,
    pub parameters: Vec<Parameter>,
    pub defined: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcedureAbi {
    pub cleanup: StackCleanup,
    pub distance: CallDistance,
    pub parameter_bytes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub id: FunctionId,
    pub name: String,
    pub result_type: TypeId,
    pub values: Vec<Value>,
    pub places: Vec<Place>,
    pub blocks: Vec<Block>,
    pub entry: BlockId,
    pub parameters: Vec<ValueId>,
    pub abi: ProcedureAbi,
    pub calls: Vec<CallAbi>,
    pub error_handler: Option<BlockId>,
    pub error_handler_local: bool,
    pub external_entries: Vec<BlockId>,
    pub linkage: Linkage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataRelocation {
    pub at: usize,
    pub target: DataId,
    pub addend: isize,
    pub address: AddressKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataObject {
    pub id: DataId,
    pub name: String,
    pub bytes: Vec<u8>,
    pub readonly: bool,
    pub relocations: Vec<DataRelocation>,
    pub linkage: Linkage,
    pub address: AddressKind,
}
