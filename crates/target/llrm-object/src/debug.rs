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

/// The CodeView 4 a unit's records are written as: the frontend states it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Dialect {
    /// What BASIC's compilers write (BC, QuickBASIC's), which BASIC's records match.
    #[default]
    Bc,
    /// The standard form, as C7-era tools write it.
    Cv4,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Info {
    pub format: Format,
    pub language: Language,
    /// Which CodeView 4 an OMF object carries, where the frontend says.
    pub dialect: Dialect,
    pub producer: Producer,
    /// The register a [`Location::Frame`] is relative to.
    pub frame_register: String,
    /// The register that stands for the return address in call frame information (the target's `pc`
    /// class); empty where the target numbers none.
    pub return_register: String,
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
    /// A scalar the source spells `name` (`unsigned long`): laid out as `scalar`, which only a format that names
    /// base types (DWARF) tells apart from the plain one.
    Basic { name: String, scalar: Scalar },
    /// BASIC's `STRING * n`.
    FixedString(u32),
    /// `bytes` of its elements in place, as C lays one out; None: BASIC's, whose bounds are its
    /// descriptor's.
    Array { element: TypeId, bytes: Option<u32> },
    /// A struct or, with `union`, a union: a type a member may reach again (a pointer to itself), by its
    /// index, which may be a later one.
    Struct { name: String, bytes: u32, fields: Vec<Field>, union: bool },
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

impl Location {
    /// Where the value is from the last range to the end of `scope`: what a format whose records name one place for a
    /// whole scope can say of it. A list with no range that reaches the end (a register parameter, there only until
    /// the body starts) has none.
    pub fn settled(&self, scope: &[Range]) -> Option<&Location> {
        let Location::List(entries) = self else { return Some(self) };
        let end = scope.last().map(|last| (last.section, last.offset + last.length))?;
        entries.iter().find(|(range, _)| (range.section, range.offset + range.length) == end).map(|(_, location)| location)
    }
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
    /// How to find the caller's frame from each place in the code; empty where the code could not be
    /// followed (a writer then says nothing, rather than something wrong).
    pub frame: Vec<FrameRow>,
}

/// From `offset` bytes into a function's first range, until the next row: the canonical frame address is
/// `cfa_offset` past register `cfa_register`, and each register in `saved` is in memory at that address
/// plus its offset (negative: below it).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameRow {
    pub offset: usize,
    pub cfa_register: String,
    pub cfa_offset: i64,
    pub saved: Vec<(String, i64)>,
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

impl Type {
    /// The types this one names, which a writer that cannot write one cannot write this one for.
    pub fn references(&self) -> Vec<TypeId> {
        match self {
            Type::Scalar(_) | Type::Basic { .. } | Type::FixedString(_) => Vec::new(),
            Type::Array { element, .. } => vec![*element],
            Type::Struct { fields, .. } => fields.iter().map(|field| field.r#type).collect(),
            Type::Enum { underlying, .. } => vec![*underlying],
            Type::Pointer { target, .. } | Type::Reference(target) | Type::Typedef { target, .. } | Type::Qualified { target, .. } => vec![*target],
            Type::Procedure { result, parameters, .. } => result.iter().chain(parameters).copied().collect(),
        }
    }
}

impl Info {
    /// The bytes a value of `ty` occupies, where the type says: None for a type that has no size
    /// of its own (void, a procedure, BASIC's array whose bounds are a descriptor's).
    pub fn size_of(&self, ty: TypeId) -> Option<u64> {
        Some(match self.types.get(ty)? {
            Type::Scalar(scalar) | Type::Basic { scalar, .. } => match *scalar {
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

#[cfg(test)]
mod settled_tests {
    use super::*;

    fn range(offset: usize, length: usize) -> Range {
        Range { section: 0, offset, length }
    }

    /// Where a value settles is the entry that reaches the end of the scope: a parameter in its register until the
    /// function stores it settles in its cell; one in a register only until the body starts, or removed, settles
    /// nowhere; a plain place is its own.
    #[test]
    fn a_value_settles_where_its_last_range_reaches_the_end_of_the_scope() {
        let scope = [range(0, 32)];
        let homed = Location::List(vec![(range(0, 10), Location::Register("eax".into())), (range(10, 22), Location::Frame { disp: -4 })]);
        assert_eq!(homed.settled(&scope), Some(&Location::Frame { disp: -4 }));
        let entry_only = Location::List(vec![(range(0, 7), Location::Register("eax".into()))]);
        assert_eq!(entry_only.settled(&scope), None);
        assert_eq!(Location::List(Vec::new()).settled(&scope), None);
        assert_eq!(Location::Frame { disp: 8 }.settled(&scope), Some(&Location::Frame { disp: 8 }));
        let whole = Location::List(vec![(range(0, 32), Location::Register("esi".into()))]);
        assert_eq!(whole.settled(&scope), Some(&Location::Register("esi".into())));
    }
}
