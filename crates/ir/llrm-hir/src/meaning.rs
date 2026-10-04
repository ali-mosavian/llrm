//! What a runtime routine computes, as its description states it: the
//! meaning MIR expands into a body a call can be replaced by.

/// How a runtime keeps its strings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Form {
    /// A descriptor in the data group, its length and data pointer at offsets.
    Near,
    /// Bytes in a far segment, behind a header.
    Far,
}

/// Where a string descriptor keeps its length and its data pointer, and how
/// large it is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Descriptor {
    pub length: i64,
    pub data: i64,
    pub size: i64,
}

/// A routine's parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Parameter {
    /// A pointer to a string descriptor.
    String,
    /// A 16-bit integer.
    Int,
}

/// A binary operation of a routine's meaning, on integers; `Add` also
/// advances a data pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arithmetic {
    Add,
    Sub,
    Min,
    Max,
    Eq,
    Lt,
    Or,
}

/// A value a routine computes from its parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Int(i64),
    Param(usize),
    /// The length of the string parameter.
    Length(usize),
    /// The address of the string parameter's first byte.
    Data(usize),
    /// The byte at an address.
    Byte(Box<Expr>),
    Binary(Arithmetic, Box<Expr>, Box<Expr>),
}

/// What a routine returns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Returns {
    /// A 16-bit value.
    Value(Expr),
    /// A string made of a length and a data address: the runtime hands back
    /// a temporary descriptor.
    View { length: Expr, data: Expr },
}

/// What one runtime routine computes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Meaning {
    pub routine: String,
    pub parameters: Vec<Parameter>,
    pub result: Returns,
    /// The string parameters the routine frees where the runtime allocated
    /// them: a copy of it stands in only where no actual is such a temporary.
    pub releases: Vec<usize>,
    /// The condition under which it raises the error, and the error's number.
    pub check: Option<(Expr, i64)>,
}
