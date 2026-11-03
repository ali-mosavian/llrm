//! `-g`: the debug information of an object, once. The backend builds an
//! [`Info`] from the frontends' facts and the code's layout; each format's
//! writer (CodeView, DWARF, Turbo Debugger) reads it and encodes it its own
//! way. A fact a writer cannot say is its to refuse, never to drop.
//!
//! Symbols are indices into `Object::symbols`, code is a section index and
//! an offset, registers are the target's register-file names.

use llrm_object::{Object, ObjectTarget, Unsupported};

/// An object and the debug information that describes it: what the backend
/// builds and a [`DebugObjectTarget`] writes. The information is the
/// object's, not a field of it, so `llrm-object` names no debug model.
#[derive(Clone, Debug)]
pub struct Described {
    pub object: Object,
    pub debug: Option<Info>,
}

impl From<Object> for Described {
    fn from(object: Object) -> Self {
        Described { object, debug: None }
    }
}

impl std::ops::Deref for Described {
    type Target = Object;

    fn deref(&self) -> &Object {
        &self.object
    }
}

impl std::ops::DerefMut for Described {
    fn deref_mut(&mut self) -> &mut Object {
        &mut self.object
    }
}

/// A debug format's writer: DWARF, CodeView 13 (LLVM's `MCDwarf` and
/// `CodeViewContext`, which a target's object writer calls in turn). It makes
/// the sections of the object's debug information; OMF's CodeView 4 and Turbo
/// Debugger records are the container's own and are not sections.
pub trait DebugWriter {
    /// `object` with the sections that say `info`, which the format's
    /// container then writes like any other.
    fn expanded(
        &self,
        object: &Object,
        info: &Info,
    ) -> Result<Object, Unsupported>;
}

/// `object` with `debug` expanded by `writer`, untouched where it has none.
pub fn expanded<'a>(
    object: &'a Object,
    debug: Option<&Info>,
    writer: &dyn DebugWriter,
) -> Result<std::borrow::Cow<'a, Object>, Unsupported> {
    match debug {
        Some(info) => writer.expanded(object, info).map(std::borrow::Cow::Owned),
        None => Ok(std::borrow::Cow::Borrowed(object)),
    }
}

/// A target's object writers when they also write debug information (LLVM's
/// `MCObjectWriter` with the debug sections its `MCStreamer` emitted).
pub trait DebugObjectTarget: ObjectTarget {
    /// `object` as a file of `format`, with `debug` expanded into the debug
    /// sections the format's writer makes.
    fn write_described(
        &self,
        format: &str,
        object: &Object,
        debug: Option<&Info>,
    ) -> Result<Vec<u8>, Unsupported>;

    /// What the debug writer `asked` for in object `format` can say; the
    /// format's own pick where `asked` is the default.
    fn debug_caps(
        &self,
        format: &str,
        asked: Format,
    ) -> DebugCaps;
}

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
    /// QuickBASIC 4.5's CodeView: its own references to scalars and `STRING *
    /// n`.
    Qb45,
}

/// A register of the target, with its number in each debug format that gives it
/// one.
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
    Dwarf {
        version: u16,
    },
    TurboDebugger,
}

/// What a debug writer can say, which decides what the compiler keeps for it
/// (LLVM's one `DbgValueHistory` feeds writers that differ here).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DebugCaps {
    /// It says where a value is over a range of code. CodeView 4, its older
    /// BASIC dialect and Turbo Debugger's records name one place for a whole
    /// scope.
    pub location_ranges: bool,
    /// It places a frame cell from the canonical frame address, which DWARF's
    /// call frame information gives at every address, so `-g` keeps no frame
    /// register for it.
    pub cfa_locations: bool,
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
    /// What BASIC's compilers write (BC, QuickBASIC's), which BASIC's records
    /// match.
    #[default]
    Bc,
    /// The standard form, as C7-era tools write it.
    Cv4,
}

/// What a [`Location::Frame`]'s displacement is measured from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FrameBase {
    /// The frame register, which the code keeps for it.
    #[default]
    Register,
    /// The frame register as the code would set it: `bias` bytes below the
    /// canonical frame address (the return address and the saved register),
    /// `far_bias` for a function called far, whose return address is longer.
    /// The code keeps none; a debugger finds the cell from the caller's frame,
    /// which call frame information gives at every address.
    Cfa { bias: i64, far_bias: i64 },
}

/// What a debug writer needs to know of the machine, so it reads no
/// [`llrm_object::Arch`]. The target says it (`Info::target`); a made-up one
/// needs no variant of `Arch`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugTarget {
    /// The width of a near address: a DWARF address, and the size of a
    /// reference.
    pub address_bytes: u8,
    /// Whether an address is a segment and an offset (16-bit real mode).
    pub segmented: bool,
    /// CodeView's machine number for `S_COMPILE3`, where the target has one.
    pub codeview_machine: Option<u16>,
    /// The fixup kind for a segment's selector, where addresses are segmented
    /// (`Info::target`'s to say: the object crate names no target's kind).
    pub segment_base: Option<llrm_object::FixupKind>,
    /// The fixup kind for a 16:16 pointer, whose offset is the field's first
    /// two bytes.
    pub far_pointer: Option<llrm_object::FixupKind>,
    /// The register file with its DWARF and CodeView numbers
    /// (`MCRegisterInfo`'s `mapLLVMRegToDWARFReg`), the frame register and the
    /// return address's column.
    pub registers: std::sync::Arc<RegisterFile>,
    /// A parameter whose register was reused is told by what the register held
    /// at entry (`DW_OP_entry_value`), as the pin does for this target.
    pub entry_values: bool,
    /// The registers a call passes its arguments in, in order; empty where the
    /// target does not describe call sites.
    pub call_arguments: Vec<String>,
    /// The registers a call leaves as they were.
    pub call_preserved: Vec<String>,
}

/// The target's registers as a debug writer numbers them: three flavours of
/// number in LLVM (`X86RegisterInfo.td`), two here.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegisterFile {
    pub registers: Vec<Register>,
    /// The register a [`Location::Frame`] is relative to.
    pub frame: String,
    /// The register that stands for the return address in call frame
    /// information (the target's `pc` class); empty where the target numbers
    /// none.
    pub return_register: String,
}

impl RegisterFile {
    pub fn named(
        &self,
        name: &str,
    ) -> Option<&Register> {
        self.registers.iter().find(|one| one.name == name)
    }

    /// The DWARF number of `name`.
    pub fn dwarf(
        &self,
        name: &str,
    ) -> Option<u16> {
        self.named(name).and_then(|one| one.dwarf)
    }

    /// The CodeView number of `name`.
    pub fn codeview(
        &self,
        name: &str,
    ) -> Option<u16> {
        self.named(name).and_then(|one| one.codeview)
    }
}

impl DebugTarget {
    /// This machine with the register file `frame`, `return_register` and
    /// `registers` make.
    pub fn with_registers(
        self,
        frame: &str,
        return_register: &str,
        registers: Vec<Register>,
    ) -> Self {
        let file = RegisterFile { registers, frame: frame.to_owned(), return_register: return_register.to_owned() };
        Self { registers: std::sync::Arc::new(file), ..self }
    }

    /// What the width of a near offset says; the target adds what it knows
    /// (`Info::target`).
    pub fn of(bits: u8) -> Self {
        Self {
            address_bytes: bits / 8,
            segmented: bits == 16,
            codeview_machine: None,
            segment_base: None,
            far_pointer: None,
            registers: std::sync::Arc::default(),
            entry_values: false,
            call_arguments: Vec::new(),
            call_preserved: Vec::new(),
        }
    }
}

/// How the information is written, as against what it says: the format asked
/// for and the habits of the compiler whose reader follows it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DebugOptions {
    pub format: Format,
    /// Which CodeView 4 an OMF object carries, where the frontend says.
    pub dialect: Dialect,
    pub producer: Producer,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Info {
    /// The machine, where the target says so; unset, [`DebugTarget::of`] the
    /// object's architecture.
    pub target: Option<DebugTarget>,
    pub options: DebugOptions,
    pub language: Language,
    /// What a [`Location::Frame`] is measured from: that register, or the
    /// canonical frame address.
    pub frame_base: FrameBase,
    pub files: Vec<File>,
    /// The module's code, from its first function to its last byte.
    pub code: Vec<Range>,
    pub types: Vec<Type>,
    /// In the order the code is laid out.
    pub functions: Vec<Function>,
    /// Functions the code calls and the unit does not define.
    pub declarations: Vec<Declaration>,
    /// Variables of the module, in data.
    pub globals: Vec<Variable>,
    pub lines: Vec<Line>,
    /// Rows of `lines` that are not the start of a statement, by (section,
    /// offset): the rest are (`is_stmt`).
    pub not_stmt: std::collections::BTreeSet<(usize, usize)>,
}

impl Info {
    /// The target's registers; none where the target is unset.
    pub fn registers(&self) -> &RegisterFile {
        static NONE: std::sync::OnceLock<RegisterFile> = std::sync::OnceLock::new();
        self.target.as_ref().map_or_else(|| NONE.get_or_init(RegisterFile::default), |one| &*one.registers)
    }

    /// `file` as the target's registers, on the target as it stands or, where
    /// there is none, on a machine of `bits`.
    pub fn set_registers(
        &mut self,
        bits: u8,
        file: RegisterFile,
    ) {
        *self.registers_mut(bits) = file;
    }

    /// The target's registers to edit, on a machine of `bits` where there is
    /// no target yet.
    pub fn registers_mut(
        &mut self,
        bits: u8,
    ) -> &mut RegisterFile {
        let target = self.target.get_or_insert_with(|| DebugTarget::of(bits));
        std::sync::Arc::make_mut(&mut target.registers)
    }

    /// The machine this information is written for.
    pub fn target_of(
        &self,
        bits: u8,
    ) -> DebugTarget {
        self.target.clone().unwrap_or_else(|| DebugTarget::of(bits))
    }
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
    Bool {
        bytes: u8,
    },
    Char,
    Int {
        bytes: u8,
        signed: bool,
    },
    Float {
        bytes: u8,
    },
    Currency,
    /// BASIC's variable-length STRING: a near or a far descriptor.
    BasicString {
        far: bool,
    },
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
    /// A scalar the source spells `name` (`unsigned long`): laid out as
    /// `scalar`, which only a format that names base types (DWARF) tells
    /// apart from the plain one.
    Basic {
        name: String,
        scalar: Scalar,
    },
    /// BASIC's `STRING * n`.
    FixedString(u32),
    /// `bytes` of its elements in place, as C lays one out; None: BASIC's,
    /// whose bounds are its descriptor's.
    Array {
        element: TypeId,
        bytes: Option<u32>,
    },
    /// BASIC's array of `element`: a variable of it is `descriptor`, a struct
    /// the program keeps the bounds and the data pointer in.
    Described {
        element: TypeId,
        descriptor: TypeId,
    },
    /// A struct or, with `union`, a union: a type a member may reach again (a
    /// pointer to itself), by its index, which may be a later one.
    Struct {
        name: String,
        bytes: u32,
        fields: Vec<Field>,
        union: bool,
    },
    Enum {
        name: String,
        underlying: TypeId,
        enumerators: Vec<Enumerator>,
    },
    /// A pointer of `bytes` (the offset's width, a far one's too).
    Pointer {
        target: TypeId,
        bytes: u8,
        reach: Reach,
    },
    /// A parameter passed by reference.
    Reference(TypeId),
    Typedef {
        name: String,
        target: TypeId,
    },
    Qualified {
        target: TypeId,
        constant: bool,
        volatile: bool,
    },
    /// `result` None returns nothing: void. `convention` is the calling
    /// convention's own name.
    Procedure {
        result: Option<TypeId>,
        parameters: Vec<TypeId>,
        convention: Option<String>,
    },
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
    /// `disp` bytes from the register `register` names: a cell reached through
    /// the stack pointer where the code keeps no frame register (the
    /// displacement changes as the stack pointer does, so it is told over the
    /// ranges it holds for).
    Relative { register: String, disp: i64 },
    /// The value itself, which is in no place.
    Constant(i64),
    /// The value of the place plus a constant (`DW_OP_plus_uconst, n,
    /// DW_OP_stack_value`), which is in no place.
    Offset(Box<Location>, i64),
    /// What the register held when the function was entered
    /// (`DW_OP_entry_value`), which is in no place now.
    EntryValue(String),
    /// What the operations make of the arguments, which are in no place of
    /// their own: a register, a frame cell or a constant each, with the bytes
    /// of the value a cell holds, pushed by [`ExprOp::Arg`]. The value itself
    /// where `value`, else the place it addresses.
    Expression { args: Vec<(Location, u32)>, ops: Vec<ExprOp>, value: bool },
    /// Its bytes in pieces, from the first: each piece's size in bytes and
    /// where it is; none for a piece that is nowhere.
    Pieces(Vec<(u32, Option<Location>)>),
}

/// One operation of a [`Location::Expression`], on a stack.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExprOp {
    /// Push the expression's `n`th argument (`DW_OP_LLVM_arg`).
    Arg(u32),
    Constu(u64),
    Consts(i64),
    PlusUconst(u64),
    Plus,
    Minus,
    Mul,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Shra,
    Not,
    Neg,
    Deref,
}

impl Location {
    /// Where the value is from the last range to the end of `scope`: what a
    /// format whose records name one place for a whole scope can say of it.
    /// A list with no range that reaches the end (a register parameter, there
    /// only until the body starts) has none.
    pub fn settled(
        &self,
        scope: &[Range],
    ) -> Option<&Location> {
        let Location::List(entries) = self else { return Some(self) };
        let end = scope.last().map(|last| (last.section, last.offset + last.length))?;
        entries
            .iter()
            .find(|(range, _)| (range.section, range.offset + range.length) == end)
            .map(|(_, location)| location)
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
    /// The call this block is the inlined body of; none for a lexical block.
    pub inlined: Option<Inlined>,
}

/// A function's body inlined into another: which function, and the call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inlined {
    /// As the source names the callee.
    pub name: String,
    /// The call's place (`DW_AT_call_file`, `_line`, `_column`).
    pub file: FileId,
    pub line: u32,
    pub column: u32,
}

impl Block {
    /// The one range from the block's first byte to its last, for a format
    /// that has no list of ranges: it takes in code between the pieces that is
    /// not the block's. None where the pieces are in more than one section (or
    /// there are none).
    pub fn hull(&self) -> Option<Range> {
        let first = self.ranges.first()?;
        if self.ranges.iter().any(|one| one.section != first.section) {
            return None;
        }
        let start = self.ranges.iter().map(|one| one.offset).min()?;
        let end = self.ranges.iter().map(|one| one.offset + one.length).max()?;
        Some(Range { section: first.section, offset: start, length: end - start })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    /// As the source names it.
    pub name: String,
    pub symbol: SymbolId,
    /// Its [`Type::Procedure`].
    pub r#type: TypeId,
    pub ranges: Vec<Range>,
    /// Where the body starts and ends, as offsets from the function's first
    /// range: after the prologue, and where the epilogue begins.
    pub body: Option<(usize, usize)>,
    /// Called and returned from far.
    pub far: bool,
    /// The module's own code, whose variables are the module's.
    pub module: bool,
    /// Parameters first, in order, then the locals.
    pub variables: Vec<Variable>,
    pub blocks: Vec<Block>,
    /// How to find the caller's frame from each place in the code; empty where
    /// the code could not be followed (a writer then says nothing, rather
    /// than something wrong).
    pub frame: Vec<FrameRow>,
    /// Every call of the body is in [`Function::calls`]
    /// (`DW_AT_call_all_calls`).
    pub all_calls: bool,
    /// The calls, in the order of the code (`DW_TAG_call_site`).
    pub calls: Vec<CallSite>,
}

/// A function the unit calls and does not define (a `DISubprogram`
/// declaration).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub name: String,
    /// Its [`Type::Procedure`].
    pub r#type: TypeId,
    pub external: bool,
}

/// The function a call reaches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallOrigin {
    /// A function of [`Info::declarations`].
    Declaration(usize),
    /// A function of [`Info::functions`].
    Function(usize),
}

/// What an argument register holds at a call, in the caller's terms
/// (`DW_AT_call_value`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallValue {
    Constant(i64),
    /// The value the register had when the caller was entered
    /// (`DW_OP_entry_value`).
    Entry(String),
    /// The value of a register the call preserves, plus `offset`.
    Register {
        name: String,
        offset: i64,
    },
}

/// An argument register of a call and what it holds
/// (`DW_TAG_call_site_parameter`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallParameter {
    pub register: String,
    pub value: CallValue,
}

/// A call: where it returns to, what it calls, and what its arguments hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallSite {
    /// The return address: the first byte after the call.
    pub section: usize,
    pub offset: usize,
    pub origin: Option<CallOrigin>,
    pub parameters: Vec<CallParameter>,
}

pub use llrm_object::FrameRow;

/// The first instruction at `offset` of section `section` is source line `line`
/// of `file`.
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
    /// The types this one names, which a writer that cannot write one cannot
    /// write this one for.
    pub fn references(&self) -> Vec<TypeId> {
        match self {
            Type::Scalar(_) | Type::Basic { .. } | Type::FixedString(_) => Vec::new(),
            Type::Array { element, .. } => vec![*element],
            Type::Described { element, descriptor } => vec![*element, *descriptor],
            Type::Struct { fields, .. } => fields.iter().map(|field| field.r#type).collect(),
            Type::Enum { underlying, .. } => vec![*underlying],
            Type::Pointer { target, .. }
            | Type::Reference(target)
            | Type::Typedef { target, .. }
            | Type::Qualified { target, .. } => vec![*target],
            Type::Procedure { result, parameters, .. } => result.iter().chain(parameters).copied().collect(),
        }
    }
}

impl Info {
    /// The bytes a value of `ty` occupies, where the type says: None for a type
    /// that has no size of its own (void, a procedure, BASIC's array whose
    /// bounds are a descriptor's).
    pub fn size_of(
        &self,
        ty: TypeId,
    ) -> Option<u64> {
        Some(match self.types.get(ty)? {
            Type::Scalar(scalar) | Type::Basic { scalar, .. } => match *scalar {
                Scalar::Void => return None,
                Scalar::Bool { bytes } | Scalar::Int { bytes, .. } | Scalar::Float { bytes } => u64::from(bytes),
                Scalar::Char => 1,
                Scalar::Currency => 8,
                Scalar::BasicString { far } => {
                    if far {
                        4
                    } else {
                        2
                    }
                }
            },
            Type::FixedString(bytes) => u64::from(*bytes),
            Type::Array { bytes, .. } => u64::from((*bytes)?),
            Type::Described { descriptor, .. } => return self.size_of(*descriptor),
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

    /// The format asked for is not a fact of the program: changing it must
    /// leave every fact `Info` states as it was.
    #[test]
    fn the_options_are_apart_from_the_facts() {
        let facts = Info { language: Language::C, types: vec![Type::Scalar(Scalar::Char)], ..Info::default() };
        let asked = Info {
            options: DebugOptions { format: Format::Dwarf { version: 4 }, dialect: Dialect::Cv4, ..Default::default() },
            ..facts.clone()
        };
        assert_ne!(asked, facts);
        assert_eq!(DebugOptions::default(), facts.options);
        assert_eq!(Info { options: DebugOptions::default(), ..asked }, facts);
    }

    #[test]
    fn a_typedef_of_an_enum_of_a_byte_is_a_byte() {
        let info = Info {
            types: vec![
                Type::Scalar(Scalar::Int { bytes: 1, signed: false }),
                Type::Enum {
                    name: "e".into(),
                    underlying: 0,
                    enumerators: vec![Enumerator { name: "a".into(), value: 0 }],
                },
                Type::Typedef { name: "t".into(), target: 1 },
            ],
            ..Info::default()
        };
        assert_eq!(info.size_of(2), Some(1));
    }

    #[test]
    fn a_basic_array_has_no_size_of_its_own() {
        let info = Info {
            types: vec![Type::Scalar(Scalar::Int { bytes: 2, signed: true }), Type::Array { element: 0, bytes: None }],
            ..Info::default()
        };
        assert_eq!(info.size_of(1), None);
    }
}

#[cfg(test)]
mod settled_tests {
    use super::*;

    fn range(
        offset: usize,
        length: usize,
    ) -> Range {
        Range { section: 0, offset, length }
    }

    /// Where a value settles is the entry that reaches the end of the scope: a
    /// parameter in its register until the function stores it settles in
    /// its cell; one in a register only until the body starts, or removed,
    /// settles nowhere; a plain place is its own.
    #[test]
    fn a_value_settles_where_its_last_range_reaches_the_end_of_the_scope() {
        let scope = [range(0, 32)];
        let homed = Location::List(vec![
            (range(0, 10), Location::Register("eax".into())),
            (range(10, 22), Location::Frame { disp: -4 }),
        ]);
        assert_eq!(homed.settled(&scope), Some(&Location::Frame { disp: -4 }));
        let entry_only = Location::List(vec![(range(0, 7), Location::Register("eax".into()))]);
        assert_eq!(entry_only.settled(&scope), None);
        assert_eq!(Location::List(Vec::new()).settled(&scope), None);
        assert_eq!(Location::Frame { disp: 8 }.settled(&scope), Some(&Location::Frame { disp: 8 }));
        let whole = Location::List(vec![(range(0, 32), Location::Register("esi".into()))]);
        assert_eq!(whole.settled(&scope), Some(&Location::Register("esi".into())));
    }
}

#[cfg(test)]
mod writer_tests {
    use llrm_object::{Role, Section};

    use super::*;

    struct Marks;

    impl DebugWriter for Marks {
        fn expanded(
            &self,
            object: &Object,
            _: &Info,
        ) -> Result<Object, Unsupported> {
            let mut made = object.clone();
            made.sections.push(Section {
                name: ".marked".into(),
                role: Role::Data,
                near: true,
                align: 1,
                image: vec![1],
                spans: vec![[0, 1]],
                relocs: Vec::new(),
            });
            Ok(made)
        }
    }

    /// A format reaches the container through its writer alone: the writer runs
    /// where the object has debug information and never where it has none.
    #[test]
    fn a_writer_expands_an_object_with_information_and_leaves_one_without() {
        let object = Object {
            name: "t".into(),
            bits: 32,
            sections: Vec::new(),
            symbols: Vec::new(),
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        };
        let with = expanded(&object, Some(&Info::default()), &Marks).unwrap();
        let without = expanded(&object, None, &Marks).unwrap();
        assert_eq!((with.sections.len(), without.sections.len()), (1, 0));
    }
}
