use std::collections::BTreeMap;
use std::collections::BTreeSet;
/// The generic name a tuple type `(A, B)` applies.
pub const TUPLE: &str = "tuple";
/// A function type `fn(A, B) -> R`, applied to its parameters then its result.
pub const FUNCTION: &str = "fn";

/// An array has one to this many dimensions.
pub const MAX_RANK: usize = 4;

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
    Addr,
    Bool,
    Void,
    I64,
    Fixed {
        storage: FixedStorage,
        fraction: u8,
        declaration: u16,
    },
    /// A payload-free enum: its tag, `u8` or `u16` wide, and its HIR type.
    Enum {
        type_id: u32,
        width: u8,
    },
    /// `vec[T]`: a near pointer to its elements, and its HIR type.
    Vector {
        type_id: u32,
    },
    /// `dict[K, V]`: a near pointer to its slots, and its HIR type.
    Dictionary {
        type_id: u32,
    },
    /// A `bits` struct: its backing integer's width, and its HIR type.
    Bits {
        type_id: u32,
        width: u8,
    },
    /// `fn(A) -> R`: which of the functions converted to it, and its HIR type.
    Function {
        type_id: u32,
    },
    /// `*far [mut] T` or `*near [mut] T`: its width, 4 or 2, and its HIR type.
    Pointer {
        type_id: u32,
        width: u8,
        mutable: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FixedStorage {
    I16,
    I32,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TypeAnnotation {
    Value(TypeSpec),
    /// `[T]`, or `[T, rank]` for a ranked view.
    Slice { element: TypeSpec, rank: u8 },
    /// `[T; d0, d1, ...]`, row-major.
    Array { element: TypeSpec, dims: Vec<u32> },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TypeSpec {
    Primitive(TypeName),
    Named(String),
    /// A generic type given its arguments: `Option[i16]`, `Result[Level, LoadError]`.
    Applied {
        name: String,
        args: Vec<TypeAnnotation>,
    },
}

impl TypeSpec {
    /// The spelling a generic instance is registered under.
    pub fn text(&self) -> String {
        match self {
            Self::Primitive(type_name) => format!("{type_name:?}").to_lowercase(),
            Self::Named(name) => name.clone(),
            Self::Applied { name, args } if name.starts_with('&') => {
                let space = if name == "&mut" { " " } else { "" };
                format!("{name}{space}{}", args.iter().map(TypeAnnotation::text).collect::<String>())
            }
            Self::Applied { name, args } if name == FUNCTION => {
                let texts: Vec<_> = args.iter().map(TypeAnnotation::text).collect();
                let (result, parameters) = texts.split_last().expect("a function type has a result");
                format!("fn({}) -> {result}", parameters.join(", "))
            }
            Self::Applied { name, args } => {
                let args: Vec<_> = args.iter().map(TypeAnnotation::text).collect();
                format!("{name}[{}]", args.join(", "))
            }
        }
    }
}

impl TypeAnnotation {
    pub fn text(&self) -> String {
        match self {
            Self::Value(spec) => spec.text(),
            Self::Slice { element, rank: 1 } => format!("&[{}]", element.text()),
            Self::Slice { element, rank } => format!("&[{}, {rank}]", element.text()),
            Self::Array { element, dims } => {
                let dims: Vec<_> = dims.iter().map(u32::to_string).collect();
                format!("{}[{}]", element.text(), dims.join(", "))
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Module {
    pub imports: Vec<Import>,
    /// The declarations marked `pub`: other modules may name them.
    pub public: BTreeSet<String>,
    pub fixed_types: Vec<FixedType>,
    pub consts: Vec<Const>,
    pub structs: Vec<Struct>,
    pub enums: Vec<Enum>,
    pub protocols: Vec<Protocol>,
    pub functions: Vec<Function>,
    /// Functions another object defines, called through a foreign ABI.
    pub externs: Vec<Extern>,
    /// The functions `export` exposes, each with its foreign ABI.
    pub exports: BTreeMap<String, Abi>,
    /// The language's library methods, compiled only where called.
    pub library: Vec<Function>,
}

/// A foreign calling convention (section 15).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Abi {
    Cdecl16,
    /// Arguments pushed first to last; the callee removes them.
    Pascal16,
}

impl Abi {
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "cdecl16" => Some(Self::Cdecl16),
            "pascal16" => Some(Self::Pascal16),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Cdecl16 => "cdecl16",
            Self::Pascal16 => "pascal16",
        }
    }

    /// The object symbol of `name`: C's `_name`, Pascal's `NAME`.
    pub fn symbol(self, name: &str) -> String {
        match self {
            Self::Cdecl16 => format!("_{name}"),
            Self::Pascal16 => name.to_ascii_uppercase(),
        }
    }

    pub fn callee_cleans(self) -> bool {
        self == Self::Pascal16
    }
}

/// A function declared in an `extern "abi":` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Extern {
    pub abi: Abi,
    /// The object symbol: `@link_name`, else the one its ABI gives the name.
    pub symbol: String,
    /// Its header; the body is empty.
    pub function: Function,
}

/// `const NAME: T = value`, its value folded to a literal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Const {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

/// `import a.b` or `import a.b as c`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    /// The module's absolute name, `a.b`.
    pub module: String,
    /// The name it is known by here: its alias, or the full name.
    pub name: String,
    pub span: Span,
}

/// A structural requirement: the methods a type must have.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Protocol {
    pub name: String,
    /// Each method's name and parameter count, `self` included.
    pub methods: Vec<(String, usize)>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaParameter {
    pub name: String,
    pub type_: Option<TypeSpec>,
}

/// A function's type parameter, as in `fn emit[W: Writer]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericParameter {
    pub name: String,
    pub bound: Option<String>,
}

/// A tagged union. A variant's positional payload fields are named `_0`, `_1`, ...
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Enum {
    pub name: String,
    /// Type parameters, as in `enum Option[T]`.
    pub generics: Vec<String>,
    /// The declared tag type, as in `enum Mode: u8`, and its width in bits:
    /// `u2` is stored as a `u8` and takes two bits of a `bits` struct.
    pub backing: Option<(TypeName, u32)>,
    pub variants: Vec<Variant>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<StructField>,
    /// An explicit tag, as in `active = 10`.
    pub tag: Option<i64>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixedType {
    pub name: String,
    pub type_name: TypeName,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Struct {
    pub name: String,
    /// Type parameters, as in `struct Pair[A, B]`.
    pub generics: Vec<String>,
    /// The backing integer of a `bits struct Name: u8`.
    pub bits: Option<TypeName>,
    /// `@repr("c16", pack=N)`: no field is aligned to more than N bytes.
    pub pack: Option<u32>,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructField {
    pub name: String,
    /// Declared `mut`: writable in place (section 5).
    pub mutable: bool,
    pub type_spec: TypeSpec,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub name: String,
    /// Type parameters: the function is a template, instantiated per use.
    pub generics: Vec<GenericParameter>,
    pub parameters: Vec<Parameter>,
    pub result: TypeAnnotation,
    pub body: Vec<Statement>,
    pub span: Span,
}

impl Function {
    /// A method takes `self` first, and only a value calls it (section 4);
    /// any other function in a type's namespace is called on the type.
    pub fn is_method(&self) -> bool {
        self.parameters.first().is_some_and(|one| one.name == "self")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub type_: ParameterType,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParameterType {
    /// Taken by value: a scalar is copied, an aggregate moves.
    Owned(TypeAnnotation),
    Borrowed {
        mutable: bool,
        target: TypeAnnotation,
    },
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
    /// `unsafe:`: its body may call foreign functions and take raw pointers.
    Unsafe {
        body: Vec<Statement>,
        span: Span,
    },
    /// `yield value`: the consuming loop's body runs with the value.
    Yield {
        value: Expr,
        span: Span,
    },
    /// `let (q, r) = value`: a pattern binds the value's parts. One that may
    /// not match has an `else:` block, which must leave.
    Destructure {
        pattern: Pattern,
        value: Expr,
        otherwise: Option<Vec<Statement>>,
        span: Span,
    },
    Assign {
        target: AssignTarget,
        operation: Option<BinaryOp>,
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
    /// A local `const`: desugaring puts its literal where its name is read.
    Const(Const),
    /// A local `fn`: desugaring makes it a module function with a hidden name.
    Function(Box<Function>),
    Match {
        subject: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// `with name = value:`: `name` is dropped when the block ends.
    With {
        mutable: bool,
        name: String,
        value: Expr,
        body: Vec<Statement>,
        span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Statement>,
    pub span: Span,
}

/// One pattern language serves `match`, `let`, and `for` (draft section 6).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Wildcard(Span),
    Binding(String, Span),
    /// An integer, character, or boolean literal.
    Literal(Expr),
    /// `.name(fields...)`, or `Enum.name(fields...)`.
    Variant {
        enum_name: Option<String>,
        name: String,
        fields: Vec<Pattern>,
        span: Span,
    },
    /// `Point(x, y)`.
    Struct {
        name: String,
        fields: Vec<Pattern>,
        span: Span,
    },
    /// `(a, b)`.
    Tuple(Vec<Pattern>, Span),
    /// `[a, *rest, z]`: the elements before and after the one starred
    /// binding, which views what lies between them.
    Sequence {
        before: Vec<Pattern>,
        rest: Option<Box<Pattern>>,
        after: Vec<Pattern>,
        span: Span,
    },
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Self::Wildcard(span) | Self::Binding(_, span) | Self::Tuple(_, span) => *span,
            Self::Literal(expression) => expression.span(),
            Self::Variant { span, .. } | Self::Struct { span, .. } | Self::Sequence { span, .. } => *span,
        }
    }
}

/// One clause of a comprehension or generator expression; they nest left to right.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Clause {
    /// `for [case] pattern in [&[mut]] iterable`, or `for name in start..end`.
    For {
        pattern: Pattern,
        refutable: bool,
        mode: IterationMode,
        iterable: Expr,
        end: Option<Expr>,
        span: Span,
    },
    If(Expr),
}

impl Clause {
    /// The one `for name in iterable` among `clauses`, the rest being `if`s:
    /// the source bounds how many items there are.
    pub fn source(clauses: &[Clause]) -> Option<(&str, IterationMode, &Expr)> {
        let mut loops = clauses.iter().filter(|one| matches!(one, Self::For { .. }));
        match (loops.next(), loops.next()) {
            (
                Some(Self::For {
                    pattern: Pattern::Binding(name, _),
                    refutable: false,
                    mode,
                    iterable,
                    end: None,
                    ..
                }),
                None,
            ) => Some((name, *mode, iterable)),
            _ => None,
        }
    }

    /// A lone `for name in iterable`: exactly one item per source item.
    pub fn simple(clauses: &[Clause]) -> Option<(&str, IterationMode, &Expr)> {
        Self::source(clauses).filter(|_| clauses.len() == 1)
    }

    /// `clauses` as nested loops and conditions around `body`.
    pub fn loops(clauses: &[Clause], body: Vec<Statement>) -> Vec<Statement> {
        let Some((first, rest)) = clauses.split_first() else {
            return body;
        };
        let inner = Self::loops(rest, body);
        match first {
            Self::If(condition) => vec![Statement::If {
                condition: condition.clone(),
                then_branch: inner,
                else_branch: Vec::new(),
                span: condition.span(),
            }],
            Self::For {
                pattern,
                refutable,
                mode,
                iterable,
                end,
                span,
            } => {
                vec![Statement::for_pattern(
                    pattern,
                    *refutable,
                    *mode,
                    iterable.clone(),
                    end.clone(),
                    inner,
                    *span,
                )]
            }
        }
    }

    fn walk_mut<E>(
        clauses: &mut [Clause],
        visit: &mut impl FnMut(&mut Expr) -> Result<(), E>,
    ) -> Result<(), E> {
        for clause in clauses {
            match clause {
                Self::If(condition) => condition.walk_mut(visit)?,
                Self::For { iterable, end, .. } => {
                    iterable.walk_mut(visit)?;
                    if let Some(end) = end {
                        end.walk_mut(visit)?;
                    }
                }
            }
        }
        Ok(())
    }
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
    Index { base: String, indices: Vec<Expr> },
    Member { base: Expr, field: String },
    /// `*pointer`.
    Deref(Expr),
}

impl AssignTarget {
    /// The place `expression` names.
    pub fn of(expression: Expr) -> Result<Self, &'static str> {
        match expression {
            Expr::Name(name, _) => Ok(Self::Name(name)),
            Expr::Index { base, indices, .. } => match *base {
                Expr::Name(base, _) => Ok(Self::Index { base, indices }),
                _ => Err("assignment target must be a named place"),
            },
            Expr::Member { base, field, .. } => Ok(Self::Member { base: *base, field }),
            Expr::Unary { op: UnaryOp::Deref, operand, .. } => Ok(Self::Deref(*operand)),
            _ => Err("expression is not assignable"),
        }
    }
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Self::Bind { span, .. }
            | Self::Unsafe { span, .. }
            | Self::Yield { span, .. }
            | Self::Destructure { span, .. }
            | Self::Assign { span, .. }
            | Self::Return { span, .. }
            | Self::If { span, .. }
            | Self::While { span, .. }
            | Self::For { span, .. }
            | Self::ForRange { span, .. }
            | Self::Match { span, .. }
            | Self::With { span, .. }
            | Self::Break(span)
            | Self::Continue(span) => *span,
            Self::Const(one) => one.span,
            Self::Function(one) => one.span,
            Self::Expr(expression) => expression.span(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Negative,
    Not,
    Complement,
    /// `*pointer`, the place a raw pointer points to.
    Deref,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    FloorDivide,
    Remainder,
    Equal,
    NotEqual,
    Is,
    IsNot,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    And,
    Or,
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
    /// `(a, b)`: an anonymous struct of its elements.
    Tuple(Vec<Expr>, Span),
    /// `|x: i16| x * 2`: compiled where it is called.
    Lambda {
        parameters: Vec<LambdaParameter>,
        body: Box<Expr>,
        span: Span,
    },
    /// `[value] * n`, one count per rank, outermost first.
    Repeat {
        value: Box<Expr>,
        counts: Vec<Expr>,
        span: Span,
    },
    /// `T(value)`.
    Conversion {
        target: TypeName,
        value: Box<Expr>,
        span: Span,
    },
    Comprehension {
        element: Box<Expr>,
        clauses: Vec<Clause>,
        span: Span,
    },
    Generator {
        element: Box<Expr>,
        clauses: Vec<Clause>,
        span: Span,
    },
    DictComprehension {
        key: Box<Expr>,
        value: Box<Expr>,
        clauses: Vec<Clause>,
        span: Span,
    },
    /// `{key: value, ...}`, or `{}`.
    Dict(Vec<(Expr, Expr)>, Span),
    Boolean(bool, Span),
    Name(String, Span),
    /// `base[i]`, or `base[i, j, ...]` for a ranked array.
    Index {
        base: Box<Expr>,
        indices: Vec<Expr>,
        span: Span,
    },
    Slice {
        base: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        span: Span,
    },
    Member {
        base: Box<Expr>,
        field: String,
        span: Span,
    },
    /// `Point(x=0, y=0)`, recognized after parsing.
    StructLiteral {
        name: String,
        fields: Vec<(String, Expr, Span)>,
        span: Span,
    },
    Borrow {
        mutable: bool,
        operand: Box<Expr>,
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
        /// `f[T](...)`: type arguments given, not inferred.
        type_arguments: Vec<TypeSpec>,
        arguments: Vec<Expr>,
        span: Span,
    },
    /// `name=value`, only as a call argument.
    NamedArgument {
        name: String,
        value: Box<Expr>,
        span: Span,
    },
    /// `operand?`: unwrap success or return the failure.
    Try {
        operand: Box<Expr>,
        span: Span,
    },
    /// `condition ? then : otherwise`.
    Conditional {
        condition: Box<Expr>,
        then: Box<Expr>,
        otherwise: Box<Expr>,
        span: Span,
    },
    MethodCall {
        receiver: Box<Expr>,
        name: String,
        type_arguments: Vec<TypeSpec>,
        arguments: Vec<Expr>,
        span: Span,
    },
    /// `.name(payload)`, `Enum.name(payload)`, or a payload-free `.name`.
    Variant {
        enum_name: Option<String>,
        name: String,
        arguments: Vec<Expr>,
        span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FStringPart {
    Text(Vec<u8>),
    Value(Expr, Format),
}

/// The field `{value:code}` fills, from a code `[-|0][width][x|b|o]`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Format {
    pub width: u8,
    pub radix: u8,
    pub zero: bool,
    pub left: bool,
}

impl Default for Format {
    fn default() -> Self {
        Self {
            width: 0,
            radix: 10,
            zero: false,
            left: false,
        }
    }
}

impl Format {
    pub fn parse(code: &str) -> Option<Self> {
        let (left, code) = code
            .strip_prefix('-')
            .map_or((false, code), |rest| (true, rest));
        let (zero, code) = code
            .strip_prefix('0')
            .map_or((false, code), |rest| (true, rest));
        let (digits, radix) = match code.as_bytes().last() {
            Some(b'x') => (&code[..code.len() - 1], 16),
            Some(b'b') => (&code[..code.len() - 1], 2),
            Some(b'o') => (&code[..code.len() - 1], 8),
            _ => (code, 10),
        };
        let width = if digits.is_empty() {
            0
        } else {
            digits.parse().ok()?
        };
        let format = Self {
            width,
            radix,
            zero,
            left,
        };
        (!(left && zero) && format != Self::default()).then_some(format)
    }
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Self::Integer(_, span)
            | Self::Float(_, span)
            | Self::Character(_, span)
            | Self::String(_, span)
            | Self::Array(_, span)
            | Self::Tuple(_, span)
            | Self::Lambda { span, .. }
            | Self::Boolean(_, span)
            | Self::Name(_, span) => *span,
            Self::FString { span, .. }
            | Self::Comprehension { span, .. }
            | Self::Generator { span, .. }
            | Self::DictComprehension { span, .. }
            | Self::Dict(_, span)
            | Self::Index { span, .. }
            | Self::Slice { span, .. }
            | Self::Member { span, .. }
            | Self::StructLiteral { span, .. }
            | Self::Borrow { span, .. }
            | Self::Repeat { span, .. }
            | Self::Conversion { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Call { span, .. }
            | Self::NamedArgument { span, .. }
            | Self::Conditional { span, .. }
            | Self::Try { span, .. }
            | Self::Variant { span, .. }
            | Self::MethodCall { span, .. } => *span,
        }
    }
}

impl Statement {
    /// Calls `visit` on this statement, then on each in the blocks it holds.
    pub fn each_mut(&mut self, visit: &mut impl FnMut(&mut Statement)) {
        visit(self);
        for block in self.blocks_mut() {
            block.iter_mut().for_each(|one| one.each_mut(visit));
        }
    }

    /// The blocks the statement holds.
    pub fn blocks_mut(&mut self) -> Vec<&mut Vec<Statement>> {
        match self {
            Self::If { then_branch, else_branch, .. } => vec![then_branch, else_branch],
            Self::While { body, .. }
            | Self::For { body, .. }
            | Self::ForRange { body, .. }
            | Self::With { body, .. }
            | Self::Unsafe { body, .. }
            | Self::Destructure { otherwise: Some(body), .. } => vec![body],
            Self::Match { arms, .. } => arms.iter_mut().map(|arm| &mut arm.body).collect(),
            _ => Vec::new(),
        }
    }

    /// `for pattern in iterable: body`, as a loop over one name. A pattern
    /// that is not a name takes apart a hidden one; a refutable `case`
    /// pattern skips the items it does not match.
    pub fn for_pattern(
        pattern: &Pattern,
        refutable: bool,
        mode: IterationMode,
        iterable: Expr,
        end: Option<Expr>,
        body: Vec<Statement>,
        span: Span,
    ) -> Statement {
        let (name, body) = match pattern {
            Pattern::Binding(name, _) if !refutable => (name.clone(), body),
            _ => {
                let name = format!("$item{}_{}", span.line, span.column);
                let item = Expr::Name(name.clone(), span);
                let body = if refutable {
                    let skip = MatchArm {
                        pattern: Pattern::Wildcard(span),
                        body: vec![Statement::Continue(span)],
                        span,
                    };
                    vec![Statement::Match {
                        subject: item,
                        arms: vec![
                            MatchArm {
                                pattern: pattern.clone(),
                                body,
                                span,
                            },
                            skip,
                        ],
                        span,
                    }]
                } else {
                    [
                        vec![Statement::Destructure {
                            pattern: pattern.clone(),
                            value: item,
                            otherwise: None,
                            span,
                        }],
                        body,
                    ]
                    .concat()
                };
                (name, body)
            }
        };
        match end {
            Some(end) => Statement::ForRange {
                name,
                start: iterable,
                end,
                body,
                span,
            },
            None => Statement::For {
                mode,
                name,
                iterable,
                body,
                span,
            },
        }
    }

    /// The statement's own expressions, not those of the blocks it holds.
    pub fn own_expressions_mut(&mut self) -> Vec<&mut Expr> {
        match self {
            Self::Bind { value, .. }
            | Self::Yield { value, .. }
            | Self::Destructure { value, .. }
            | Self::Expr(value)
            | Self::With { value, .. } => {
                vec![value]
            }
            Self::Assign { target, value, .. } => {
                let mut own = match target {
                    AssignTarget::Name(_) => Vec::new(),
                    AssignTarget::Index { indices, .. } => indices.iter_mut().collect(),
                    AssignTarget::Member { base, .. } | AssignTarget::Deref(base) => vec![base],
                };
                own.push(value);
                own
            }
            Self::Return { value, .. } => value.iter_mut().collect(),
            Self::If { condition, .. } | Self::While { condition, .. } => vec![condition],
            Self::For { iterable, .. } => vec![iterable],
            Self::ForRange { start, end, .. } => vec![start, end],
            Self::Match { subject, .. } => vec![subject],
            Self::Break(_) | Self::Continue(_) | Self::Unsafe { .. } | Self::Const(_) | Self::Function(_) => Vec::new(),
        }
    }

    /// Calls `visit` on every expression in the statement, innermost first.
    pub fn walk_mut<E>(
        &mut self,
        visit: &mut impl FnMut(&mut Expr) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Bind { value, .. } | Self::Yield { value, .. } | Self::Expr(value) => value.walk_mut(visit),
            Self::Destructure { value, otherwise, .. } => {
                value.walk_mut(visit)?;
                otherwise.as_deref_mut().map_or(Ok(()), |body| walk_body(body, visit))
            }
            Self::Assign { target, value, .. } => {
                match target {
                    AssignTarget::Name(_) => {}
                    AssignTarget::Index { indices, .. } => {
                        for index in indices {
                            index.walk_mut(visit)?;
                        }
                    }
                    AssignTarget::Member { base, .. } | AssignTarget::Deref(base) => base.walk_mut(visit)?,
                }
                value.walk_mut(visit)
            }
            Self::Return { value, .. } => value.as_mut().map_or(Ok(()), |one| one.walk_mut(visit)),
            Self::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                condition.walk_mut(visit)?;
                walk_body(then_branch, visit)?;
                walk_body(else_branch, visit)
            }
            Self::While {
                condition, body, ..
            } => {
                condition.walk_mut(visit)?;
                walk_body(body, visit)
            }
            Self::For { iterable, body, .. } => {
                iterable.walk_mut(visit)?;
                walk_body(body, visit)
            }
            Self::ForRange {
                start, end, body, ..
            } => {
                start.walk_mut(visit)?;
                end.walk_mut(visit)?;
                walk_body(body, visit)
            }
            Self::With { value, body, .. } => {
                value.walk_mut(visit)?;
                walk_body(body, visit)
            }
            Self::Unsafe { body, .. } => walk_body(body, visit),
            Self::Match { subject, arms, .. } => {
                subject.walk_mut(visit)?;
                arms.iter_mut()
                    .try_for_each(|arm| walk_body(&mut arm.body, visit))
            }
            Self::Break(_) | Self::Continue(_) | Self::Const(_) | Self::Function(_) => Ok(()),
        }
    }
}

fn walk_body<E>(
    body: &mut [Statement],
    visit: &mut impl FnMut(&mut Expr) -> Result<(), E>,
) -> Result<(), E> {
    body.iter_mut()
        .try_for_each(|statement| statement.walk_mut(visit))
}

impl Expr {
    /// Calls `visit` on this expression and every one inside it, innermost first.
    pub fn walk_mut<E>(
        &mut self,
        visit: &mut impl FnMut(&mut Expr) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Integer(..)
            | Self::Float(..)
            | Self::Character(..)
            | Self::String(..)
            | Self::Boolean(..)
            | Self::Name(..) => {}
            Self::FString { parts, .. } => {
                for part in parts {
                    if let FStringPart::Value(value, _) = part {
                        value.walk_mut(visit)?;
                    }
                }
            }
            Self::Dict(entries, _) => {
                for (key, value) in entries {
                    key.walk_mut(visit)?;
                    value.walk_mut(visit)?;
                }
            }
            Self::Array(items, _) | Self::Tuple(items, _) => {
                for item in items {
                    item.walk_mut(visit)?;
                }
            }
            Self::Repeat { value, counts, .. } => {
                value.walk_mut(visit)?;
                for count in counts {
                    count.walk_mut(visit)?;
                }
            }
            Self::Conversion { value, .. }
            | Self::NamedArgument { value, .. }
            | Self::Borrow { operand: value, .. }
            | Self::Unary { operand: value, .. }
            | Self::Try { operand: value, .. }
            | Self::Member { base: value, .. } => value.walk_mut(visit)?,
            // A lambda's body is visited where it is inlined, in its own scope.
            Self::Lambda { .. } => {}
            Self::Comprehension {
                element, clauses, ..
            }
            | Self::Generator {
                element, clauses, ..
            } => {
                Clause::walk_mut(clauses, visit)?;
                element.walk_mut(visit)?;
            }
            Self::DictComprehension {
                key,
                value,
                clauses,
                ..
            } => {
                Clause::walk_mut(clauses, visit)?;
                key.walk_mut(visit)?;
                value.walk_mut(visit)?;
            }
            Self::Index { base, indices, .. } => {
                base.walk_mut(visit)?;
                for index in indices {
                    index.walk_mut(visit)?;
                }
            }
            Self::Slice {
                base, start, end, ..
            } => {
                base.walk_mut(visit)?;
                for bound in [start, end].into_iter().flatten() {
                    bound.walk_mut(visit)?;
                }
            }
            Self::StructLiteral { fields, .. } => {
                for (_, value, _) in fields {
                    value.walk_mut(visit)?;
                }
            }
            Self::Binary { left, right, .. } => {
                left.walk_mut(visit)?;
                right.walk_mut(visit)?;
            }
            Self::Call { arguments, .. } | Self::Variant { arguments, .. } => {
                for argument in arguments {
                    argument.walk_mut(visit)?;
                }
            }
            Self::MethodCall {
                receiver,
                arguments,
                ..
            } => {
                receiver.walk_mut(visit)?;
                for argument in arguments {
                    argument.walk_mut(visit)?;
                }
            }
            Self::Conditional {
                condition,
                then,
                otherwise,
                ..
            } => {
                condition.walk_mut(visit)?;
                then.walk_mut(visit)?;
                otherwise.walk_mut(visit)?;
            }
        }
        visit(self)
    }
}
