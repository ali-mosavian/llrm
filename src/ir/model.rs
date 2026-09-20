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

entity_id!(TypeId);
entity_id!(GlobalId);
entity_id!(FunctionId);
entity_id!(BlockId);
entity_id!(InstructionId);
entity_id!(ValueId);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    pub name: String,
    pub types: Vec<Type>,
    pub globals: Vec<Global>,
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Type {
    pub id: TypeId,
    pub kind: TypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Void,
    Integer { bits: u16 },
    Float(FloatKind),
    Pointer { address_space: AddressSpace },
    Array { element: TypeId, length: u64 },
    Structure { fields: Vec<TypeId>, packed: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatKind {
    Binary32,
    Binary64,
    Extended80,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressSpace {
    Generic,
    NearData,
    FarData,
    HugeData,
    Code,
    Segment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Global {
    pub id: GlobalId,
    pub name: String,
    pub type_id: TypeId,
    pub linkage: Linkage,
    pub constant: bool,
    pub initializer: Option<Constant>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Linkage {
    Internal,
    External,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Constant {
    Integer(i128),
    Float(String),
    Null,
    Undefined,
    Bytes(Vec<u8>),
    Aggregate(Vec<TypedConstant>),
    GlobalAddress { global: GlobalId, addend: i64 },
    FunctionAddress(FunctionId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedConstant {
    pub type_id: TypeId,
    pub value: Constant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub id: FunctionId,
    pub name: String,
    pub signature: Signature,
    pub linkage: Linkage,
    pub attributes: Vec<FunctionAttribute>,
    pub parameters: Vec<Value>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature {
    pub result: TypeId,
    pub parameters: Vec<TypeId>,
    pub variadic: bool,
    pub calling_convention: CallingConvention,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallingConvention {
    C,
    Basic,
    Runtime,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FunctionAttribute {
    NoReturn,
    NoUnwind,
    ReadOnly,
    AlwaysInline,
    NeverInline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub id: ValueId,
    pub type_id: TypeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub id: BlockId,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Instruction {
    pub id: InstructionId,
    pub results: Vec<Value>,
    pub kind: InstructionKind,
}

impl Instruction {
    pub fn effects(&self) -> Effects {
        match &self.kind {
            InstructionKind::Load { volatile, .. } => Effects {
                memory: MemoryEffects::Read,
                may_trap: true,
                observable: *volatile,
            },
            InstructionKind::Store { .. } => Effects {
                memory: MemoryEffects::Write,
                may_trap: true,
                observable: true,
            },
            InstructionKind::Call { effects, .. } => *effects,
            InstructionKind::Intrinsic { intrinsic, .. } => intrinsic.effects(),
            InstructionKind::Binary {
                op:
                    BinaryOp::SignedDivide
                    | BinaryOp::UnsignedDivide
                    | BinaryOp::SignedRemainder
                    | BinaryOp::UnsignedRemainder,
                ..
            } => Effects {
                memory: MemoryEffects::None,
                may_trap: true,
                observable: false,
            },
            InstructionKind::Phi { .. }
            | InstructionKind::Unary { .. }
            | InstructionKind::Binary { .. }
            | InstructionKind::Compare { .. }
            | InstructionKind::Cast { .. }
            | InstructionKind::GetElementPointer { .. }
            | InstructionKind::Select { .. } => Effects::NONE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionKind {
    Phi {
        incoming: Vec<PhiIncoming>,
    },
    Unary {
        op: UnaryOp,
        operand: Operand,
    },
    Binary {
        op: BinaryOp,
        left: Operand,
        right: Operand,
    },
    Compare {
        predicate: ComparePredicate,
        left: Operand,
        right: Operand,
    },
    Cast {
        op: CastOp,
        operand: Operand,
        to: TypeId,
    },
    Load {
        address: Operand,
        alignment: u32,
        volatile: bool,
    },
    Store {
        address: Operand,
        value: Operand,
        alignment: u32,
        volatile: bool,
    },
    GetElementPointer {
        base: Operand,
        indices: Vec<Operand>,
    },
    Select {
        condition: Operand,
        then_value: Operand,
        else_value: Operand,
    },
    Call {
        callee: Callee,
        arguments: Vec<Operand>,
        effects: Effects,
    },
    Intrinsic {
        intrinsic: Intrinsic,
        arguments: Vec<Operand>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhiIncoming {
    pub predecessor: BlockId,
    pub value: Operand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operand {
    Value(ValueId),
    Constant(TypedConstant),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Negate,
    Not,
    FloatNegate,
    FloatAbsolute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    SignedDivide,
    UnsignedDivide,
    SignedRemainder,
    UnsignedRemainder,
    And,
    Or,
    Xor,
    ShiftLeft,
    LogicalShiftRight,
    ArithmeticShiftRight,
    FloatAdd,
    FloatSubtract,
    FloatMultiply,
    FloatDivide,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparePredicate {
    Equal,
    NotEqual,
    SignedLessThan,
    SignedLessEqual,
    SignedGreaterThan,
    SignedGreaterEqual,
    UnsignedLessThan,
    UnsignedLessEqual,
    UnsignedGreaterThan,
    UnsignedGreaterEqual,
    OrderedEqual,
    OrderedNotEqual,
    OrderedLessThan,
    OrderedLessEqual,
    OrderedGreaterThan,
    OrderedGreaterEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastOp {
    Truncate,
    SignExtend,
    ZeroExtend,
    IntegerToFloat,
    FloatToInteger,
    FloatExtend,
    FloatTruncate,
    PointerToInteger,
    IntegerToPointer,
    Bitcast,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Callee {
    Direct(FunctionId),
    Indirect(Operand),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryEffects {
    None,
    Read,
    Write,
    ReadWrite,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Effects {
    pub memory: MemoryEffects,
    pub may_trap: bool,
    pub observable: bool,
}

impl Effects {
    pub const NONE: Self = Self {
        memory: MemoryEffects::None,
        may_trap: false,
        observable: false,
    };

    pub const fn is_pure(self) -> bool {
        matches!(self.memory, MemoryEffects::None) && !self.may_trap && !self.observable
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intrinsic {
    MemoryCopy,
    MemoryMove,
    MemorySet,
    SquareRoot,
    Sine,
    Cosine,
    Arctangent,
    Log2,
    Exp2,
    Trap,
}

impl Intrinsic {
    pub const fn effects(self) -> Effects {
        match self {
            Self::MemoryCopy | Self::MemoryMove => Effects {
                memory: MemoryEffects::ReadWrite,
                may_trap: true,
                observable: true,
            },
            Self::MemorySet => Effects {
                memory: MemoryEffects::Write,
                may_trap: true,
                observable: true,
            },
            Self::Trap => Effects {
                memory: MemoryEffects::None,
                may_trap: true,
                observable: true,
            },
            Self::SquareRoot
            | Self::Sine
            | Self::Cosine
            | Self::Arctangent
            | Self::Log2
            | Self::Exp2 => Effects::NONE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        condition: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    Switch {
        selector: Operand,
        cases: Vec<(i128, BlockId)>,
        default: BlockId,
    },
    Return(Option<Operand>),
    Unreachable,
}
