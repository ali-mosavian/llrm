//! The spike's syntax trees. Where the grammar cannot tell two readings
//! apart, one node keeps the written shape and a later pass picks.

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(String),
    Char(u8),
    Str(Vec<u8>),
    FStr(Vec<u8>),
    Bool(bool),
    Name(String),
    /// `i16`, `bool`, ... written where an expression stands: a cast head or a
    /// type argument.
    Primitive(&'static str),
    /// `.name` or `.name(args)`.
    Variant(String, Option<Vec<Arg>>),
    Paren(Box<Expr>),
    Tuple(Vec<Expr>),
    List(Vec<Expr>),
    Dict(Vec<(Expr, Expr)>),
    Comprehension(Box<Expr>, Option<Box<Expr>>, Vec<Clause>),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
    Compare(Box<Expr>, Vec<(&'static str, Expr)>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Lambda(Vec<(String, Option<Type>)>, Box<Expr>),
    Try(Box<Expr>),
    /// Deferred: a call, a cast, a struct or variant construction.
    Apply(Box<Expr>, Vec<Arg>),
    /// Deferred: an index, a slice, a generic instantiation or an array type.
    Subscript(Box<Expr>, Vec<Index>),
    /// Deferred: a field, a method, a module member or a qualified variant.
    Field(Box<Expr>, String),
    /// A type only a type can spell: `fn(i16) -> i16`.
    TypeOnly(Box<Type>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Index {
    At(Expr),
    Slice(Option<Expr>, Option<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Clause {
    pub filter: bool,
    pub refutable: bool,
    pub pattern: Option<Pattern>,
    pub source: Expr,
    pub end: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Primitive(&'static str),
    Path(Vec<String>),
    Tuple(Vec<Type>),
    Function(Vec<Type>, Box<Type>),
    Borrow(bool, Box<Type>),
    /// `&[T]` or `&[T, n]`.
    Slice(Box<Type>, Option<i64>),
    /// `*[near|far|huge] [mut] T`; the qualifier is a name until a pass checks
    /// it.
    Pointer(Option<String>, bool, Box<Type>),
    /// Deferred: `Name[T, U]` generic arguments or `T[4]`, `T[N]` dimensions.
    Applied(Box<Type>, Vec<TypeArg>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeArg {
    Type(Type),
    Number(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wild,
    Name(String),
    Int(i64),
    Char(u8),
    Bool(bool),
    Tuple(Vec<Pattern>),
    /// `Name(..)` or `a.b.Name(..)`: a struct or a qualified variant.
    Constructor(Vec<String>, Vec<Pattern>),
    /// `a.b`: a qualified variant without a payload.
    Path(Vec<String>),
    Variant(String, Vec<Pattern>),
    Sequence(Vec<Pattern>),
    /// `*name` or `*_` inside a sequence.
    Rest(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block(pub Vec<Stmt>);

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        mutable: bool,
        pattern: Pattern,
        ty: Option<Type>,
        value: Expr,
        otherwise: Option<Block>,
    },
    Const(Const),
    Function(Function),
    Unsafe(Block),
    Asm,
    Loop(Block),
    Return(Option<Expr>),
    Yield(Expr),
    Match(Expr, Vec<(Pattern, Block)>),
    With {
        mutable: bool,
        name: String,
        value: Expr,
        body: Block,
    },
    If(Expr, Block, Option<Block>),
    While(Expr, Block),
    /// A leading `&` or `&mut` of `source` is the iteration mode, read by a
    /// later pass.
    For {
        refutable: bool,
        pattern: Pattern,
        source: Expr,
        end: Option<Expr>,
        body: Block,
    },
    Break,
    Continue,
    /// Deferred: the left side is any expression until a pass checks it is a
    /// place.
    Assign(Expr, &'static str, Expr),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Const {
    pub name: String,
    pub ty: Option<Type>,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: Vec<String>,
    pub generics: Vec<(String, Option<Type>)>,
    pub params: Vec<Param>,
    pub ret: Type,
    /// `None` for an `@extern` declaration.
    pub body: Option<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub name: String,
    pub args: Vec<Arg>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub mutable: bool,
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<(Option<String>, Type)>,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Function { far: bool, function: Function },
    Struct { name: String, params: Vec<String>, fields: Vec<Field> },
    Bits { name: String, width: &'static str, fields: Vec<Field> },
    Enum { name: String, params: Vec<String>, tag: Option<String>, variants: Vec<Variant> },
    Protocol { name: String, params: Vec<String>, methods: Vec<Function> },
    Const(Const),
    Var { huge: bool, name: String, ty: Type, value: Expr },
    Fixed { name: String, base: &'static str, fraction: i64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub attrs: Vec<Attr>,
    pub public: bool,
    pub decl: Decl,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub path: Vec<String>,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub imports: Vec<Import>,
    pub items: Vec<Item>,
}
