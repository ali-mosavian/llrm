//! Parser-private typed syntax. Name and storage resolution consumes this;
//! these nodes never cross the common-HIR boundary.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeName {
    Integer,
    Long,
    Single,
    Double,
    String,
    Named(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bound {
    pub lower: Option<Expr>,
    pub upper: Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub type_name: Option<TypeName>,
    pub array: bool,
    pub bounds: Vec<Bound>,
    pub fixed_length: Option<Expr>,
    pub shared: bool,
    pub dynamic: bool,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub declaration: Declaration,
    pub by_value: bool,
    pub segmented: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcedureKind {
    Sub,
    Function,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitTarget {
    Sub,
    Function,
    Do,
    For,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileMode {
    Input,
    Output,
    Append,
    Binary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrintSeparator {
    Comma,
    Semicolon,
    End,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintItem {
    pub value: Expr,
    pub separator: PrintSeparator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Procedure {
    pub name: String,
    pub alias: Option<String>,
    pub cdecl: bool,
    pub kind: ProcedureKind,
    pub parameters: Vec<Parameter>,
    pub result: Option<TypeName>,
    pub body: Vec<Statement>,
    pub declaration: bool,
    pub is_static: bool,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Integer(i64, TypeName),
    Real(String, TypeName),
    String(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unary {
    Positive,
    Negative,
    Not,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Binary {
    Imp,
    Eqv,
    Xor,
    Or,
    And,
    Eq,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Add,
    Subtract,
    Modulo,
    IntegerDivide,
    Multiply,
    Divide,
    Power,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResumeTarget {
    Current,
    Next,
    Label(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Literal(Literal, Span),
    Name(String, Span),
    Apply {
        name: String,
        arguments: Vec<Expr>,
        span: Span,
    },
    Index {
        base: Box<Expr>,
        indices: Vec<Expr>,
        span: Span,
    },
    Field {
        base: Box<Expr>,
        name: String,
        span: Span,
    },
    Unary {
        op: Unary,
        operand: Box<Expr>,
        span: Span,
    },
    Binary {
        op: Binary,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
}

impl Eq for Expr {}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(_, span) | Self::Name(_, span) => *span,
            Self::Apply { span, .. }
            | Self::Index { span, .. }
            | Self::Field { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. } => *span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    DefType {
        type_name: TypeName,
        ranges: Vec<(char, char)>,
        span: Span,
    },
    TypeDecl {
        name: String,
        fields: Vec<Declaration>,
        span: Span,
    },
    Dim(Vec<Declaration>),
    Redim(Vec<Declaration>),
    Erase(Vec<Expr>),
    Const {
        name: String,
        value: Expr,
        span: Span,
    },
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    Label(String, Span),
    Goto(String, Span),
    CallOrGoto(String, Span),
    If {
        condition: Expr,
        then_branch: Vec<Statement>,
        else_branch: Vec<Statement>,
        span: Span,
    },
    For {
        counter: Expr,
        start: Expr,
        end: Expr,
        step: Option<Expr>,
        body: Vec<Statement>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Vec<Statement>,
        span: Span,
    },
    Do {
        pre: Option<(bool, Expr)>,
        post: Option<(bool, Expr)>,
        body: Vec<Statement>,
        span: Span,
    },
    Select {
        selector: Expr,
        arms: Vec<(Vec<Expr>, Vec<Statement>)>,
        otherwise: Vec<Statement>,
        span: Span,
    },
    Call {
        name: String,
        arguments: Vec<Expr>,
        explicit: bool,
        span: Span,
    },
    Comment(Span),
    OptionExplicit(Span),
    OptionBase(i64, Span),
    Exit(ExitTarget, Span),
    DefSeg {
        value: Option<Expr>,
        span: Span,
    },
    OnError {
        label: String,
        local: bool,
        span: Span,
    },
    Resume {
        target: ResumeTarget,
        span: Span,
    },
    Open {
        path: Expr,
        mode: FileMode,
        file: Expr,
        span: Span,
    },
    Close {
        files: Vec<Expr>,
        span: Span,
    },
    LineInput {
        file: Expr,
        destination: Expr,
        span: Span,
    },
    FileTransfer {
        write: bool,
        file: Expr,
        position: Option<Expr>,
        target: Expr,
        span: Span,
    },
    Seek {
        file: Expr,
        position: Expr,
        span: Span,
    },
    Print {
        file: Option<Expr>,
        items: Vec<PrintItem>,
        span: Span,
    },
    Input {
        file: Option<Expr>,
        destinations: Vec<Expr>,
        span: Span,
    },
    Data {
        values: Vec<Expr>,
        span: Span,
    },
    Read {
        destinations: Vec<Expr>,
        span: Span,
    },
    Runtime {
        name: String,
        arguments: Vec<Expr>,
        span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    pub statements: Vec<Statement>,
    pub procedures: Vec<Procedure>,
}
