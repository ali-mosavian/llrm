#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub end_column: usize,
}

impl Span {
    pub const fn new(line: usize, column: usize, end_column: usize) -> Self {
        Self {
            line,
            column,
            end_column,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TypeName {
    Char,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
    String,
    Bool,
    Void,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeAnnotation {
    Scalar(TypeName),
    Array { element: TypeSpec, length: u32 },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TypeSpec {
    Primitive(TypeName),
    Named(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    pub structs: Vec<Struct>,
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructField {
    pub name: String,
    pub type_spec: TypeSpec,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub result: TypeName,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub type_name: TypeName,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    Bind {
        mutable: bool,
        name: String,
        annotation: Option<TypeAnnotation>,
        value: Expr,
        span: Span,
    },
    Assign {
        target: AssignTarget,
        value: Expr,
        span: Span,
    },
    Expr(Expr),
    Return {
        value: Option<Expr>,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Vec<Statement>,
        else_branch: Vec<Statement>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Vec<Statement>,
        span: Span,
    },
    For {
        mode: IterationMode,
        name: String,
        iterable: Expr,
        body: Vec<Statement>,
        span: Span,
    },
    ForRange {
        name: String,
        start: Expr,
        end: Expr,
        body: Vec<Statement>,
        span: Span,
    },
    Break(Span),
    Continue(Span),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IterationMode {
    Value,
    Shared,
    Mutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssignTarget {
    Name(String),
    Index { base: String, index: Expr },
    Member { base: Expr, field: String },
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Self::Bind { span, .. }
            | Self::Assign { span, .. }
            | Self::Return { span, .. }
            | Self::If { span, .. }
            | Self::While { span, .. }
            | Self::For { span, .. }
            | Self::ForRange { span, .. }
            | Self::Break(span)
            | Self::Continue(span) => *span,
            Self::Expr(expression) => expression.span(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Negative,
    Not,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Is,
    IsNot,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Integer(i64, Span),
    Float(String, Span),
    Character(u8, Span),
    String(Vec<u8>, Span),
    FString {
        parts: Vec<FStringPart>,
        span: Span,
    },
    Array(Vec<Expr>, Span),
    Boolean(bool, Span),
    Name(String, Span),
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Member {
        base: Box<Expr>,
        field: String,
        span: Span,
    },
    StructLiteral {
        name: String,
        fields: Vec<(String, Expr, Span)>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Call {
        name: String,
        arguments: Vec<Expr>,
        span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FStringPart {
    Text(Vec<u8>),
    Value(Expr),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Self::Integer(_, span)
            | Self::Float(_, span)
            | Self::Character(_, span)
            | Self::String(_, span)
            | Self::Array(_, span)
            | Self::Boolean(_, span)
            | Self::Name(_, span) => *span,
            Self::FString { span, .. }
            | Self::Index { span, .. }
            | Self::Member { span, .. }
            | Self::StructLiteral { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Call { span, .. } => *span,
        }
    }
}
