//! `-g`: the debug information of an object, once. The backend builds an
//! [`Info`] from the frontends' facts and the code's layout; each format's
//! writer (CodeView, DWARF, Turbo Debugger) reads it and encodes it its own
//! way. A fact a writer cannot say is its to refuse, never to drop.
//!
//! Symbols are indices into `Object::symbols`, code is a section index and
//! an offset, registers are the target's register-file names.

/// A type, by its index in [`Info::types`].
pub type TypeId = usize;
/// A file, by its index in [`Info::files`].
pub type FileId = usize;
/// A symbol, by its index in `Object::symbols`.
pub type SymbolId = usize;

/// The compiler whose reader's habits a CodeView writer follows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Producer {
    #[default]
    Native,
    /// QuickBASIC 4.5's CodeView: its own references to scalars and `STRING * n`.
    Qb45,
}

/// A register of the target, with its number in each debug format that gives it one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Register {
    pub name: String,
    pub bits: u32,
    pub dwarf: Option<u16>,
    pub codeview: Option<u16>,
}

/// The debug format a writer is asked for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Format {
    /// The object format's own: CodeView for OMF, DWARF for ELF and Mach-O.
    #[default]
    Default,
    CodeView,
    Dwarf { version: u16 },
    TurboDebugger,
}

/// The source language, which a debugger reads values by.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Language {
    #[default]
    Unknown,
    C,
    Basic,
    Nib,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Info {
    pub format: Format,
    pub language: Language,
    pub producer: Producer,
    /// The register a [`Location::Frame`] is relative to.
    pub frame_register: String,
    /// The target's register file, so a writer needs no target.
    pub registers: Vec<Register>,
    pub files: Vec<File>,
    /// The module's code, from its first function to its last byte.
    pub code: Vec<Range>,
    pub types: Vec<Type>,
    /// In the order the code is laid out.
    pub functions: Vec<Function>,
    /// Variables of the module, in data.
    pub globals: Vec<Variable>,
    pub lines: Vec<Line>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChecksumKind {
    Md5,
    Sha1,
    Sha256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct File {
    pub name: String,
    pub checksum: Option<(ChecksumKind, Vec<u8>)>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Scalar {
    Void,
    Bool { bytes: u8 },
    Char,
    Int { bytes: u8, signed: bool },
    Float { bytes: u8 },
    Currency,
    /// BASIC's variable-length STRING: a near or a far descriptor.
    BasicString { far: bool },
}

/// How far a pointer reaches.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Reach {
    Near,
    Far,
    Huge,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Field {
    pub name: String,
    pub r#type: TypeId,
    pub offset: u32,
    /// A bit field's first bit in the unit at `offset`, and its width.
    pub bits: Option<(u8, u8)>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Enumerator {
    pub name: String,
    pub value: i64,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    Scalar(Scalar),
    /// BASIC's `STRING * n`.
    FixedString(u32),
    /// `bytes` of its elements in place, as C lays one out; None: BASIC's, whose bounds are its
    /// descriptor's.
    Array { element: TypeId, bytes: Option<u32> },
    Struct { name: String, bytes: u32, fields: Vec<Field> },
    Enum { name: String, underlying: TypeId, enumerators: Vec<Enumerator> },
    /// A pointer of `bytes` (the offset's width, a far one's too).
    Pointer { target: TypeId, bytes: u8, reach: Reach },
    /// A parameter passed by reference.
    Reference(TypeId),
    Typedef { name: String, target: TypeId },
    Qualified { target: TypeId, constant: bool, volatile: bool },
    /// `result` None returns nothing: void. `convention` is the calling convention's own name.
    Procedure { result: Option<TypeId>, parameters: Vec<TypeId>, convention: Option<String> },
}

/// `length` bytes of section `section` from `offset`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Range {
    pub section: usize,
    pub offset: usize,
    pub length: usize,
}

/// Where a value is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Location {
    /// `disp` bytes from the frame register once the prologue has set it up.
    Frame { disp: i64 },
    /// A register of the target's register file, by name.
    Register(String),
    /// Where it is over each range of the code, the ranges not overlapping.
    List(Vec<(Range, Location)>),
    /// `disp` bytes into the data `symbol` names.
    Static { symbol: SymbolId, disp: i64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Parameter,
    Local,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variable {
    pub name: String,
    pub r#type: TypeId,
    pub kind: Kind,
    pub location: Location,
}

/// A lexical scope inside a function.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Block {
    pub ranges: Vec<Range>,
    pub variables: Vec<Variable>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    /// As the source names it.
    pub name: String,
    pub symbol: SymbolId,
    /// Its [`Type::Procedure`].
    pub r#type: TypeId,
    pub ranges: Vec<Range>,
    /// Where the body starts and ends, as offsets from the function's first range: after the
    /// prologue, and where the epilogue begins.
    pub body: Option<(usize, usize)>,
    /// Called and returned from far.
    pub far: bool,
    /// The module's own code, whose variables are the module's.
    pub module: bool,
    /// Parameters first, in order, then the locals.
    pub variables: Vec<Variable>,
    pub blocks: Vec<Block>,
}

/// The first instruction at `offset` of section `section` is source line `line` of `file`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Line {
    pub section: usize,
    pub offset: usize,
    pub file: FileId,
    pub line: u32,
    /// 0: none.
    pub column: u32,
}

impl Info {
    /// The bytes a value of `ty` occupies, where the type says: None for a type that has no size
    /// of its own (void, a procedure, BASIC's array whose bounds are a descriptor's).
    pub fn size_of(&self, ty: TypeId) -> Option<u64> {
        Some(match self.types.get(ty)? {
            Type::Scalar(scalar) => match *scalar {
                Scalar::Void => return None,
                Scalar::Bool { bytes } | Scalar::Int { bytes, .. } | Scalar::Float { bytes } => u64::from(bytes),
                Scalar::Char => 1,
                Scalar::Currency => 8,
                Scalar::BasicString { far } => if far { 4 } else { 2 },
            },
            Type::FixedString(bytes) => u64::from(*bytes),
            Type::Array { bytes, .. } => u64::from((*bytes)?),
            Type::Struct { bytes, .. } => u64::from(*bytes),
            Type::Enum { underlying, .. } => return self.size_of(*underlying),
            Type::Pointer { bytes, .. } => u64::from(*bytes),
            Type::Reference(_) => return None,
            Type::Typedef { target, .. } | Type::Qualified { target, .. } => return self.size_of(*target),
            Type::Procedure { .. } => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typedef_of_an_enum_of_a_byte_is_a_byte() {
        let info = Info {
            types: vec![
                Type::Scalar(Scalar::Int { bytes: 1, signed: false }),
                Type::Enum { name: "e".into(), underlying: 0, enumerators: vec![Enumerator { name: "a".into(), value: 0 }] },
                Type::Typedef { name: "t".into(), target: 1 },
            ],
            ..Info::default()
        };
        assert_eq!(info.size_of(2), Some(1));
    }

    #[test]
    fn a_basic_array_has_no_size_of_its_own() {
        let info = Info { types: vec![Type::Scalar(Scalar::Int { bytes: 2, signed: true }), Type::Array { element: 0, bytes: None }], ..Info::default() };
        assert_eq!(info.size_of(1), None);
    }
}
