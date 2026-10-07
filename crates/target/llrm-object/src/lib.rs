//! The object the backend lays out, which one writer per format writes. See
//! `docs/architecture/object-model.md`.

pub mod debug;

/// The machine an object's code is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Arch {
    /// 16-bit x86: real mode.
    I8086,
    /// 32-bit x86.
    I386,
    /// x86-64.
    X8664,
}

impl Arch {
    /// The width of a near offset, in bits.
    pub fn bits(self) -> u32 {
        match self {
            Arch::I8086 => 16,
            Arch::I386 => 32,
            Arch::X8664 => 64,
        }
    }
}

/// What a section holds, which each format spells its own way.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Text,
    ROData,
    Data,
    /// Uninitialised: its image is zeros, and a format with a section for it stores none.
    Bss,
    /// The linker's stack: concatenated with the other objects' stacks.
    Stack,
    /// A format's debug section, which only that format's writer makes.
    Debug,
}

/// The field a relocation patches, and what the value is relative to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Kind {
    /// The target's address, `width` bytes.
    Abs { width: usize },
    /// The target's address minus the place `from` bytes past the field's start: an x86 call's
    /// is `width`, the field's end.
    PcRel { width: usize, from: usize },
    /// A call or jump's target, `width` bytes relative to the field's end: what `PcRel { width,
    /// from: width }` says, for the formats that tell a branch from a data reference (Mach-O's
    /// `BRANCH`, which a linker may send through a stub).
    Branch { width: usize },
    /// COFF: the 1-based index of the target's section, 2 bytes (`IMAGE_REL_*_SECTION`).
    SectionIndex,
    /// COFF: the target's offset within its section, `width` bytes (`IMAGE_REL_*_SECREL`).
    SectionOffset { width: usize },
    /// OMF: the selector of the target's segment.
    SegmentBase,
    /// OMF: a 16:16 pointer, whose offset is the field's first two bytes.
    FarPointer,
}

impl Kind {
    /// Field bytes this kind patches.
    pub fn width(self) -> usize {
        match self {
            Kind::Abs { width } | Kind::PcRel { width, .. } | Kind::Branch { width } => width,
            Kind::SectionIndex | Kind::SegmentBase => 2,
            Kind::SectionOffset { width } => width,
            Kind::FarPointer => 4,
        }
    }

    pub fn relative(self) -> bool {
        matches!(self, Kind::PcRel { .. } | Kind::Branch { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    /// An index into `Object::symbols`.
    Symbol(usize),
    /// An index into `Object::omf_groups`.
    OmfGroup(usize),
}

/// One field of a section's image, filled when the object is linked. The image holds zeros there.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reloc {
    pub at: usize,
    pub kind: Kind,
    pub target: Target,
    /// What is added to the target's address; sign-extended from the field's width.
    pub addend: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    pub name: String,
    pub role: Role,
    /// Addressed within the program's one data group or flat space. A section reached by its own
    /// selector alone, paragraph aligned, is not.
    pub near: bool,
    /// The widest alignment an item in it asks for, in bytes.
    pub align: usize,
    pub image: Vec<u8>,
    /// `[start, end)` of the image that holds data; the rest is zeros that need not be stored.
    pub spans: Vec<[usize; 2]>,
    pub relocs: Vec<Reloc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Binding {
    Public,
    Local,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Definition {
    Defined { section: usize, offset: usize },
    Undefined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub binding: Binding,
    pub definition: Definition,
    /// OMF: the group a symbol this object does not define is addressed in.
    pub group: Option<usize>,
}

/// An OMF group: segments addressed by one selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OmfGroup {
    pub name: String,
    /// Indices into `Object::sections`.
    pub members: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Object {
    /// The source's name.
    pub name: String,
    pub arch: Arch,
    pub sections: Vec<Section>,
    /// Defined symbols in the order they were defined, then the undefined ones in the order they
    /// are declared.
    pub symbols: Vec<Symbol>,
    pub omf_groups: Vec<OmfGroup>,
    pub debug: Option<debug::Info>,
}

/// What a writer cannot say. A writer returns it; it never writes something near.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unsupported(pub String);

impl std::fmt::Display for Unsupported {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unsupported {}
