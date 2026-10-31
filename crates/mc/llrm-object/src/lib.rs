//! The object the backend lays out, which one writer per format writes. See
//! `docs/formats/object-model.md`.

pub mod assembler;

/// Which debug format a [`Role::Debug`] section belongs to, so a writer needs
/// no name to tell them apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugSection {
    /// A `.debug_*` section of DWARF.
    Dwarf,
    /// CodeView's symbols: OMF's `$$SYMBOLS`, COFF's `.debug$S`.
    CodeViewSymbols,
    /// CodeView's types: OMF's `$$TYPES`, COFF's `.debug$T`.
    CodeViewTypes,
}

/// What a section holds, which each format spells its own way.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Text,
    ROData,
    Data,
    /// Uninitialised: its image is zeros, and a format with a section for it
    /// stores none.
    Bss,
    /// The linker's stack: concatenated with the other objects' stacks.
    Stack,
    /// A format's debug section, which only that format's writer makes.
    Debug(DebugSection),
    /// Pointers to the functions the loader calls before `main`, in order
    /// (`llvm.global_ctors`): `__mod_init_func`, `.init_array`.
    InitFunctions,
    /// The unwind information of the functions (`.eh_frame`), which only the
    /// format's unwind writer makes.
    Unwind,
}

/// What a fixup patches (LLVM's `MCFixupKind`): an open number. The generic
/// kinds are below `FIRST_TARGET`; a target's own are above it. The kinds from
/// `FIRST_TARGET` are the targets'; only the segmented-addressing pair stays
/// here, which OMF and the debug writers share.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FixupKind(pub u16);

/// What a fixup kind says of the field it patches (LLVM's `MCFixupKindInfo`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixupKindInfo {
    pub name: &'static str,
    /// Bits from the field's first byte to the bits the fixup patches.
    pub offset: u8,
    /// Bits the fixup patches; the field is `bits / 8` bytes (rounded up).
    pub bits: u8,
}

impl FixupKind {
    pub const NONE: Self = Self(4000);
    pub const DATA_1: Self = Self(4001);
    pub const DATA_2: Self = Self(4002);
    pub const DATA_4: Self = Self(4003);
    pub const DATA_8: Self = Self(4004);
    /// COFF: the 1-based index of the target's section (`FK_SecRel_2`).
    pub const SECREL_2: Self = Self(4005);
    /// COFF: the target's offset within its section (`FK_SecRel_4`).
    pub const SECREL_4: Self = Self(4006);
    /// A target's kinds start here and are its own (`FIRST_TARGET + n`), so two
    /// targets reuse the numbers.
    pub const FIRST_TARGET: Self = Self(4010);

    /// The kinds' table; a target's replace it (each target crate states its
    /// own `INFOS`).
    pub fn info(self) -> FixupKindInfo {
        let (name, bits) = match self {
            Self::DATA_1 => ("FK_Data_1", 8),
            Self::DATA_2 => ("FK_Data_2", 16),
            Self::DATA_4 => ("FK_Data_4", 32),
            Self::DATA_8 => ("FK_Data_8", 64),
            Self::SECREL_2 => ("FK_SecRel_2", 16),
            Self::SECREL_4 => ("FK_SecRel_4", 32),
            _ => ("none", 0),
        };
        FixupKindInfo { name, offset: 0, bits }
    }

    /// Field bytes this kind patches.
    pub fn width(self) -> usize {
        usize::from(self.info().bits).div_ceil(8)
    }
}

/// The part of a [`Reloc`] that does not say where it is or what it names:
/// kind, whether it is relative to the place, the bytes between the field's
/// start and that place, and the specifier. What the backend builds before it
/// places a fixup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fix {
    pub kind: FixupKind,
    /// The value is relative to the place (`MCFixup::PCRel`).
    pub pc_rel: bool,
    /// The bytes from the field's start to the place a pc-relative value is
    /// relative to: an x86 call's is the field's width, the field's end. LLVM
    /// folds it into the addend; llrm keeps it apart and the writers add it
    /// back, choosing by the total: coff64 writes `REL32` for every value (the
    /// distance past the end goes into the field) and Mach-O picks `SIGNED_n`
    /// from `-(total + 4)`, so `cmpb $1, x(%rip)` and `lea x-1(%rip)` are one
    /// object as in LLVM.
    pub pc_bias: usize,
    /// The value's specifier (`S_ABS`, `S_GOT`).
    pub spec: u8,
}

impl Fix {
    pub const S_ABS: u8 = 0;
    pub const S_GOT: u8 = 1;
    /// COFF: the target's address relative to the image base
    /// (`VK_COFF_IMGREL32`).
    pub const S_IMGREL: u8 = 2;
    pub const SECTION_INDEX: Fix = Fix::plain(FixupKind::SECREL_2);

    /// An absolute fixup of `kind`.
    pub const fn plain(kind: FixupKind) -> Fix {
        Fix { kind, pc_rel: false, pc_bias: 0, spec: Fix::S_ABS }
    }

    /// A fixup of `kind` relative to the place `pc_bias` bytes past the field's
    /// start.
    pub const fn relative(
        kind: FixupKind,
        pc_bias: usize,
    ) -> Fix {
        Fix { kind, pc_rel: true, pc_bias, spec: Fix::S_ABS }
    }

    /// The target's address, `width` bytes.
    pub const fn abs(width: usize) -> Fix {
        Fix::plain(match width {
            1 => FixupKind::DATA_1,
            2 => FixupKind::DATA_2,
            4 => FixupKind::DATA_4,
            8 => FixupKind::DATA_8,
            _ => FixupKind::NONE,
        })
    }

    /// The target's address minus the place `from` bytes past the field's
    /// start.
    pub const fn pc_rel(
        width: usize,
        from: usize,
    ) -> Fix {
        Fix::relative(Fix::abs(width).kind, from)
    }

    /// COFF: the target's address minus the image base, `width` bytes.
    pub const fn image_relative(width: usize) -> Fix {
        Fix { spec: Fix::S_IMGREL, ..Fix::abs(width) }
    }

    /// COFF: the target's offset within its section, `width` bytes.
    pub const fn section_offset(width: usize) -> Fix {
        Fix::plain(match width {
            4 => FixupKind::SECREL_4,
            _ => FixupKind::NONE,
        })
    }
}

/// Writes `value` into the `width`-byte field at `at` of a section's `image`
/// (LLVM's `applyFixup` for a data fixup), and refuses what the field cannot
/// hold: a pc-relative value is signed, any other may be signed or unsigned.
pub fn apply_fixup(
    image: &mut [u8],
    at: usize,
    width: usize,
    pc_rel: bool,
    value: i64,
    section: &str,
) -> Result<(), Unsupported> {
    let fits = match width {
        8 => true,
        4 => i32::try_from(value).is_ok() || (!pc_rel && u32::try_from(value).is_ok()),
        2 => i16::try_from(value).is_ok() || (!pc_rel && u16::try_from(value).is_ok()),
        _ => i8::try_from(value).is_ok() || (!pc_rel && u8::try_from(value).is_ok()),
    };
    if !fits {
        return Err(Unsupported(format!("{section}: a value of {value} does not fit its {width}-byte field")));
    }
    image[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]);
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    /// An index into `Object::symbols`.
    Symbol(usize),
    /// An index into `Object::omf_groups`.
    OmfGroup(usize),
    /// The start of a section, by its index in `Object::sections`: what a debug
    /// section refers to another one by.
    Section(usize),
}

/// One field of a section's image, filled when the object is linked. The image
/// holds zeros there.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reloc {
    pub at: usize,
    pub kind: FixupKind,
    /// The value is relative to the place (`MCFixup::PCRel`).
    pub pc_rel: bool,
    /// See [`Fix::spec`].
    pub spec: u8,
    pub target: Target,
    /// What is added to the target's address, less the distance from the
    /// field's start to the place a pc-relative value is relative to
    /// (`S + A - P`); sign-extended from the field's width.
    pub addend: i64,
}

impl Reloc {
    pub fn new(
        at: usize,
        fix: Fix,
        target: Target,
        addend: i64,
    ) -> Self {
        // As LLVM: the bias of a pc-relative value is part of the addend
        // (`S + A - P`, `A = addend - pc_bias`); a writer adds back the part
        // its format keeps in the field.
        let addend = if fix.pc_rel { addend - fix.pc_bias as i64 } else { addend };
        Reloc { at, kind: fix.kind, pc_rel: fix.pc_rel, spec: fix.spec, target, addend }
    }

    /// Field bytes this relocation patches.
    pub fn width(&self) -> usize {
        self.kind.width()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    pub name: String,
    pub role: Role,
    /// Addressed within the program's one data group or flat space. A section
    /// reached by its own selector alone, paragraph aligned, is not.
    pub near: bool,
    /// The widest alignment an item in it asks for, in bytes.
    pub align: usize,
    pub image: Vec<u8>,
    /// `[start, end)` of the image that holds data; the rest is zeros that need
    /// not be stored.
    pub spans: Vec<[usize; 2]>,
    pub relocs: Vec<Reloc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Binding {
    Public,
    /// Exported, but a strong definition of the name elsewhere replaces it
    /// (`weak`, `linkonce`).
    Weak,
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
    /// ELF `st_size`: the bytes of a function's body, not the padding before
    /// it; 0 when unknown.
    pub size: usize,
    /// OMF: the group a symbol this object does not define is addressed in.
    pub group: Option<usize>,
}

impl Symbol {
    /// A name the compiler gave what the source did not name (`G$n`, an unnamed
    /// global): the pin's `.L` names, which an ELF symbol table leaves out.
    pub fn is_private(&self) -> bool {
        self.name.starts_with("G$")
    }
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
    /// The width of a near offset, in bits: 16 is real mode's segmented x86.
    pub bits: u8,
    pub sections: Vec<Section>,
    /// Defined symbols in the order they were defined, then the undefined ones
    /// in the order they are declared.
    pub symbols: Vec<Symbol>,
    pub omf_groups: Vec<OmfGroup>,
    /// OMF: the code sections written as COMDAT records, one function each,
    /// which the linker places in a segment of its own and drops where
    /// nothing refers to them. Each has one public symbol at offset 0
    /// that names it. Indices into `sections`.
    pub omf_comdat: Vec<usize>,
    /// How to find each function's caller (`.eh_frame`'s source), whether or
    /// not `-g` is on. Empty where the format has none or nothing declared it.
    pub frames: Vec<Frame>,
}

/// From `offset` bytes into a function's first range, until the next row: the
/// canonical frame address is `cfa_offset` past register `cfa_register`, and
/// each register in `saved` is in memory at that address plus its offset
/// (negative: below it).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameRow {
    pub offset: usize,
    pub cfa_register: String,
    pub cfa_offset: i64,
    pub saved: Vec<(String, i64)>,
}

/// A function's unwind rows: its code's place and how the canonical frame
/// address and the saved registers change along it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub section: usize,
    pub offset: usize,
    pub length: usize,
    pub rows: Vec<FrameRow>,
    /// The directives that declared the rows, in order (`.cfi_*`), for the
    /// formats that encode them instead of the rows (Mach-O's compact unwind).
    pub cfi: Vec<assembler::Cfi>,
    /// The prologue as `.seh_*` directives state it (Win64), each with the
    /// offset after its instruction, for the formats that encode them.
    pub seh: Vec<(usize, Seh)>,
}

/// One Win64 prologue directive (LLVM's `.seh_pushreg`, `.seh_stackalloc`,
/// `.seh_setframe`, `.seh_savereg`, `.seh_savexmm`). Registers are hardware
/// numbers (`xmm` by its own number).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Seh {
    PushReg(u8),
    StackAlloc(u64),
    /// The frame register, `offset` bytes above `rsp`.
    SetFrame(u8, u64),
    SaveReg(u8, u64),
    SaveXmm(u8, u64),
}

/// What a target says of the object files it writes, so the code that holds an
/// [`Object`] names no writer crate (LLVM's `MCObjectTargetWriter` dispatch, in
/// one place per target).
pub trait ObjectTarget {
    /// `object` as a file of `format` (`"elf"`, `"coff"`, `"macho"`); a format
    /// the target has no writer for is [`Unsupported`].
    fn write_object(
        &self,
        format: &str,
        object: &Object,
    ) -> Result<Vec<u8>, Unsupported>;

    /// What the target says of `format`'s assembly and debug conventions
    /// (LLVM's `MCAsmInfo`); `None` for a format it does not write.
    fn asm_info(
        &self,
        format: &str,
    ) -> Option<AsmInfo>;
}

/// What an object format says of the code it holds, so the code that lays it
/// out names no format.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AsmInfo {
    /// A function's preferred alignment is `.prefalign` (no more than its body
    /// fills), not `.p2align`.
    pub pref_align: bool,
    /// A host's linker and C name the runtime's `B$X` as `B_X`: the object is
    /// not DOS's.
    pub hosted: bool,
    /// How the format says where a function's caller is
    /// (`MCAsmInfo::ExceptionsType`, `getExceptionHandlingType`).
    pub unwind: UnwindKind,
    /// One debug section's offset into another is relocated (ELF, COFF, OMF);
    /// Mach-O's is the offset itself, which dsymutil reads in the object's own
    /// sections (`MCAsmInfo::doesDwarfUseRelocationsAcrossSections`).
    pub dwarf_relocs_across_sections: bool,
}

/// The unwind information a format carries for a function.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UnwindKind {
    /// None: DOS's OMF has no place for it.
    #[default]
    None,
    /// `.eh_frame` (ELF).
    EhFrame,
    /// Mach-O's `__compact_unwind` with `__eh_frame` for what it cannot encode.
    Compact,
    /// `.pdata` and `.xdata` (Win64 COFF).
    Win64,
}

/// What a writer cannot say. A writer returns it; it never writes something
/// near.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unsupported(pub String);

impl std::fmt::Display for Unsupported {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unsupported {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three containers each had their own range check, and Mach-O's refused a
    /// negative absolute 32-bit field the others took: one check now,
    /// signed where pc-relative, either where not.
    #[test]
    fn a_field_takes_what_its_width_holds() {
        let mut image = [0u8; 8];
        assert!(apply_fixup(&mut image, 0, 4, false, -2, "t").is_ok() && image[..4] == [0xFE, 0xFF, 0xFF, 0xFF]);
        assert!(apply_fixup(&mut image, 0, 4, false, 0xFFFF_FFFF, "t").is_ok());
        assert!(apply_fixup(&mut image, 0, 4, true, 0xFFFF_FFFF, "t").unwrap_err().0.contains("4-byte"));
        assert!(apply_fixup(&mut image, 0, 2, false, 70_000, "t").is_err());
        assert!(apply_fixup(&mut image, 0, 8, true, i64::MIN, "t").is_ok());
    }

    /// A kind without a width entry patched zero bytes, so `Reloc::width` and
    /// the writers' field sizes were silently 0.
    #[test]
    fn every_kind_a_fix_builds_patches_its_field() {
        let widths = [
            (Fix::abs(1), 1),
            (Fix::abs(2), 2),
            (Fix::abs(4), 4),
            (Fix::abs(8), 8),
            (Fix::pc_rel(4, 5), 4),
            (Fix::section_offset(4), 4),
        ];
        for (fix, width) in widths {
            assert_eq!(fix.kind.width(), width, "{fix:?}");
            let reloc = Reloc::new(0, fix, Target::Symbol(0), 0);
            assert_eq!((reloc.kind, reloc.pc_rel, reloc.spec), (fix.kind, fix.pc_rel, fix.spec));
        }
    }

    /// `Fix::abs` of an unsupported width has no kind, never a wrong one.
    #[test]
    fn an_unsupported_width_is_none() {
        assert_eq!(Fix::abs(3).kind, FixupKind::NONE);
        assert_eq!(FixupKind::NONE.width(), 0);
    }

    /// The bias of a pc-relative value is part of the addend, as LLVM's `S + A
    /// - P` has it; each writer adds back what its format keeps in the
    /// field. The five writers each read the bias their own way before
    /// (COFF baked 4 into the field, Mach-O picked `SIGNED_n` from it, ELF
    /// subtracted it).
    #[test]
    fn the_bias_of_a_pc_relative_reloc_is_in_its_addend() {
        let addend = |fix: Fix| Reloc::new(0, fix, Target::Symbol(0), 10).addend;
        assert_eq!(addend(Fix::relative(FixupKind::DATA_4, 4)), 6);
        assert_eq!(addend(Fix::relative(FixupKind::DATA_4, 5)), 5);
        assert_eq!(addend(Fix::plain(FixupKind::DATA_4)), 10);
    }
}
