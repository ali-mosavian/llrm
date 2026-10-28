//! A target's register file as its description states it (`registers.regs`),
//! asked through queries, as LLVM's generated `MCRegisterInfo` is: the width,
//! root, lane, name and classes of a register, and the view of a root at a
//! width. The target generates its `Info` from its description; the backend
//! reads the one the driver bound. A `RegId` is iced's `Register` until the
//! newtype.

use iced_x86::Register;

/// A register's id: a number in the table of the target being compiled
/// (`Info`). The x86 targets number theirs as iced does, so the encoder and
/// decoder convert; any other target numbers its own file. Only the target's
/// `Info` says what an id means: `Debug` and the x86 helpers below read an id
/// as iced's.
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegId(u8);

impl std::fmt::Debug for RegId {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        self.iced().fmt(formatter)
    }
}

include!(concat!(env!("OUT_DIR"), "/reg_consts.rs"));

impl RegId {
    /// The register iced names, for the encoder and the decoder.
    #[inline]
    pub const fn new(register: Register) -> Self {
        RegId(register as u8)
    }

    /// The id numbered `number` in a target's own register file.
    #[inline]
    pub const fn from_raw(number: u8) -> Self {
        RegId(number)
    }

    /// The register iced numbers so; `None` for a number iced has no register
    /// for, which only a non-x86 target uses.
    #[inline]
    pub fn iced(self) -> Register {
        Register::try_from(usize::from(self.0)).unwrap_or(Register::None)
    }

    /// Every register iced names.
    #[inline]
    pub fn values() -> impl Iterator<Item = RegId> + DoubleEndedIterator + ExactSizeIterator {
        Register::values().map(RegId::new)
    }

    /// The register of iced's number `index`.
    #[inline]
    pub fn from_index(index: usize) -> RegId {
        RegId(u8::try_from(index).expect("a register id below 256"))
    }

    /// The dword register of this one's family (see [`crate::root::root`]):
    /// x86's numbering, which an x86 target's ids have; another target's
    /// `Info` says its own roots.
    #[inline]
    pub fn root(self) -> RegId {
        RegId(ROOT_OF[self.0 as usize])
    }

    /// iced's number of the register: the index into the tables.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn full_register32(self) -> RegId {
        RegId::new(self.iced().full_register32())
    }

    #[inline]
    pub fn full_register(self) -> RegId {
        RegId::new(self.iced().full_register())
    }

    #[inline]
    pub fn size(self) -> usize {
        self.iced().size()
    }

    #[inline]
    pub fn number(self) -> usize {
        self.iced().number()
    }

    #[inline]
    pub fn is_gpr(self) -> bool {
        self.iced().is_gpr()
    }

    #[inline]
    pub fn is_gpr8(self) -> bool {
        self.iced().is_gpr8()
    }

    #[inline]
    pub fn is_gpr16(self) -> bool {
        self.iced().is_gpr16()
    }

    #[inline]
    pub fn is_gpr32(self) -> bool {
        self.iced().is_gpr32()
    }

    #[inline]
    pub fn is_gpr64(self) -> bool {
        self.iced().is_gpr64()
    }

    #[inline]
    pub fn is_segment_register(self) -> bool {
        self.iced().is_segment_register()
    }

    #[inline]
    pub fn is_st(self) -> bool {
        self.iced().is_st()
    }

    #[inline]
    pub fn is_xmm(self) -> bool {
        self.iced().is_xmm()
    }
}

impl From<Register> for RegId {
    #[inline]
    fn from(register: Register) -> Self {
        RegId::new(register)
    }
}

impl From<RegId> for Register {
    #[inline]
    fn from(register: RegId) -> Self {
        register.iced()
    }
}

/// The classes a description may give a register, one bit each, in this order.
/// A name not here is refused where the table is generated.
pub const CLASSES: [&str; 22] = [
    "base",
    "byte",
    "code_segment",
    "data_segment",
    "far_segment",
    "frame",
    "gpr",
    "index",
    "int",
    "pc",
    "positional",
    "reserved",
    "segment",
    "stack",
    "stack_segment",
    "string_segment",
    "x87",
    "rfp",
    "fpr",
    "zero",
    "xmm",
    "tail_call",
];

/// The class bits.
pub mod class {
    pub const BASE: u32 = 1 << 0;
    pub const BYTE: u32 = 1 << 1;
    pub const CODE_SEGMENT: u32 = 1 << 2;
    pub const DATA_SEGMENT: u32 = 1 << 3;
    pub const FAR_SEGMENT: u32 = 1 << 4;
    pub const FRAME: u32 = 1 << 5;
    pub const GPR: u32 = 1 << 6;
    pub const INDEX: u32 = 1 << 7;
    /// An integer register: the 8, 16 and 32-bit views the width tables name.
    pub const INT: u32 = 1 << 8;
    pub const PC: u32 = 1 << 9;
    /// A position in a stack: an exchange of it is an effect and it is no
    /// register a pass renames or removes.
    pub const POSITIONAL: u32 = 1 << 10;
    pub const RESERVED: u32 = 1 << 11;
    /// A segment register: an operand, not allocatable.
    pub const SEGMENT: u32 = 1 << 12;
    pub const STACK: u32 = 1 << 13;
    pub const STACK_SEGMENT: u32 = 1 << 14;
    pub const STRING_SEGMENT: u32 = 1 << 15;
    /// LLVM's RFP classes: FP0..FP6, the x87 registers greedy allocates and
    /// the stackifier maps to stack positions. Named by iced's MM0..MM6,
    /// the physical x87 registers, and never encoded.
    pub const RFP: u32 = 1 << 17;
    pub const X87: u32 = 1 << 16;
    /// A floating-point or vector register.
    pub const FPR: u32 = 1 << 18;
    /// A register that reads as zero and ignores writes.
    pub const ZERO: u32 = 1 << 19;
    /// An SSE register: scalar `float` and `double` live in its low lanes.
    pub const XMM: u32 = 1 << 20;
    /// A register an indirect sibling call may take its callee's address in
    /// (the pin's `GR32_TC`, `GR64_TC`), in the order it is given out.
    pub const TAIL_CALL: u32 = 1 << 21;
}

/// What LIR calls the frame register and the stack pointer, whatever the
/// target: a listing and an object spell them as the target has them
/// (`FrameRegisters::spelled`).
pub const FRAME: RegId = RegId::BP;
pub const STACK: RegId = RegId::SP;

/// One register of the file.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// The register itself.
    pub id: RegId,
    pub name: &'static str,
    pub bits: u32,
    pub root: RegId,
    /// The bit offset inside the root.
    pub lane: u32,
    /// `class` bits.
    pub classes: u32,
}

/// A target's register file: an entry by iced's number, and each register at
/// each width by its root.
/// What a row of `x86.instr` says of its instruction, as a target's build.rs
/// writes it out: the registers it uses without naming and the flags it
/// touches.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub reads: &'static [&'static str],
    pub writes: &'static [&'static str],
    /// What each dest, then each source, may be: `r`, `m`, `i`, `a` or `s`, as
    /// `x86.instr` spells them.
    pub kinds: &'static [&'static str],
    /// The bits of the operation, where it has the one (`stosb`'s 8).
    pub width: u32,
    /// `(source, dest)`: the source is the dest's register, whatever the
    /// semantics name.
    pub ties: &'static [(usize, usize)],
    /// `(is a dest, operand, root)`: the operand is that register when it is
    /// one.
    pub pins: &'static [(bool, usize, &'static str)],
    pub flags_read: u32,
    pub flags_written: u32,
}

/// A target's rows of `name` with `dests` destinations and `sources` sources.
pub type Rows = fn(&str, usize, usize) -> &'static [Row];

/// What a query works out once from the file: each width's integer registers,
/// and all of them.
pub struct Cache {
    integers: [std::sync::OnceLock<Vec<RegId>>; 4],
    xmm_bytes: std::sync::OnceLock<u32>,
    rfp: std::sync::OnceLock<Vec<RegId>>,
}

impl Cache {
    pub const fn new() -> Self {
        Self {
            integers: [
                std::sync::OnceLock::new(),
                std::sync::OnceLock::new(),
                std::sync::OnceLock::new(),
                std::sync::OnceLock::new(),
            ],
            xmm_bytes: std::sync::OnceLock::new(),
            rfp: std::sync::OnceLock::new(),
        }
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Info {
    pub table: [Option<Entry>; 256],
    /// The root the description gives the class `frame`, and `stack`.
    pub frame: RegId,
    pub stack: RegId,
    /// The far-pointer load that fills each segment register: `(es, "les")`.
    pub loads: &'static [(RegId, &'static str)],
    /// The registers a pass may borrow for a moment, most preferred first.
    pub scratch: &'static [RegId],
    /// The 8-bit registers in allocation order, the high bytes among them
    /// (the GR8 class); empty where the target has none.
    pub bytes: &'static [RegId],
    /// The segment register each address space of a pair kind means, where the
    /// target has segments: what an access without a prefix reads, the stack's,
    /// the code's, and the one a far pointer's selector is loaded into.
    pub data_segment: Option<RegId>,
    pub stack_segment: Option<RegId>,
    pub code_segment: Option<RegId>,
    pub far_segment: Option<RegId>,
    /// The widths the file states (0 pads), and each root's register at each:
    /// `views[root as usize][column of the width]`.
    pub widths: [u32; 8],
    /// The effect rows of the target's instructions (`MCInstrDesc`'s implicit
    /// uses and defs): the register file and the instruction table are one
    /// description of the machine.
    pub effects: Rows,
    /// Worked out on first ask.
    pub cache: Cache,
    pub views: [[Option<RegId>; 8]; 256],
}

impl Info {
    /// The entry for `register`, if the description lists it.
    pub fn get(
        &self,
        register: RegId,
    ) -> Option<&Entry> {
        self.table.get(register.index()).and_then(Option::as_ref)
    }

    /// Whether the description lists `register`.
    pub fn known(
        &self,
        register: RegId,
    ) -> bool {
        self.get(register).is_some()
    }

    /// The width of `register`, in bytes.
    pub fn bytes(
        &self,
        register: RegId,
    ) -> Option<i64> {
        self.get(register).map(|one| i64::from(one.bits / 8))
    }

    /// The register `register` is a view of (itself for a root, and for one the
    /// description does not list).
    pub fn root(
        &self,
        register: RegId,
    ) -> RegId {
        self.get(register).map_or(register, |one| one.root)
    }

    /// Which of a root's four bytes `register` names, one bit each: the lane
    /// mask.
    pub fn lanes(
        &self,
        register: RegId,
    ) -> i64 {
        self.get(register).map_or(0b1111, |one| {
            let width = (one.bits / 8).min(4);
            ((1_i64 << width) - 1) << (one.lane / 8)
        })
    }

    /// `register`'s own name, lowercase.
    pub fn name(
        &self,
        register: RegId,
    ) -> Option<&'static str> {
        self.get(register).map(|one| one.name)
    }

    /// Whether the description gives `register` every class in `mask`.
    pub fn in_class(
        &self,
        register: RegId,
        mask: u32,
    ) -> bool {
        self.get(register).is_some_and(|one| one.classes & mask == mask)
    }

    /// The register of `root` that is `bits` wide: the first by iced's number
    /// where several share it (AL and AH are both EAX's byte; AL is the one
    /// named).
    pub fn view(
        &self,
        root: RegId,
        bits: u32,
    ) -> Option<RegId> {
        let column = self.widths.iter().position(|one| *one == bits && bits != 0)?;
        self.views.get(root.index())?[column]
    }

    /// Whether `register` is a view of the frame register's root.
    pub fn is_frame(
        &self,
        register: RegId,
    ) -> bool {
        self.root(register) == self.frame
    }

    /// Whether `register` is a view of the stack pointer's root.
    pub fn is_stack(
        &self,
        register: RegId,
    ) -> bool {
        self.root(register) == self.stack
    }

    /// The mnemonic that loads a far pointer's offset and `segment`.
    pub fn load_form(
        &self,
        segment: RegId,
    ) -> Option<&'static str> {
        self.loads.iter().find(|(one, _)| *one == segment).map(|(_, name)| *name)
    }

    /// The segment register `form` loads, where it is a far-pointer load.
    pub fn loaded_by(
        &self,
        form: &str,
    ) -> Option<RegId> {
        self.loads.iter().find(|(_, name)| *name == form).map(|(one, _)| *one)
    }

    /// Whether the register is a position in a stack, where an exchange is an
    /// effect and no pass may rename or drop it.
    pub fn positional(
        &self,
        register: RegId,
    ) -> bool {
        self.in_class(register, class::POSITIONAL)
    }

    /// Whether writing one register can be seen by reading the other: the
    /// same root is not enough, `al` and `ah` share `eax` and no byte.
    pub fn overlaps(
        &self,
        one: RegId,
        other: RegId,
    ) -> bool {
        self.root(one) == self.root(other) && self.lanes(one) & self.lanes(other) != 0
    }

    /// Whether `register` is a segment register.
    pub fn is_segment(
        &self,
        register: RegId,
    ) -> bool {
        self.in_class(register, class::SEGMENT)
    }

    pub fn is_data_segment(
        &self,
        register: RegId,
    ) -> bool {
        self.data_segment == Some(register)
    }

    pub fn is_stack_segment(
        &self,
        register: RegId,
    ) -> bool {
        self.stack_segment == Some(register)
    }

    pub fn is_code_segment(
        &self,
        register: RegId,
    ) -> bool {
        self.code_segment == Some(register)
    }

    /// The data segment, for code that exists only where the target has
    /// address spaces of a pair kind: a target without them never reaches it.
    pub fn data(&self) -> RegId {
        self.data_segment.expect("the target's register file names no data segment")
    }

    pub fn stack_segment_register(&self) -> RegId {
        self.stack_segment.expect("the target's register file names no stack segment")
    }

    pub fn far(&self) -> RegId {
        self.far_segment.expect("the target's register file names no far segment")
    }

    /// Whether `form` loads a far pointer into a segment register an address
    /// may be held in: any but the data segment, which is the default.
    pub fn loads_a_selector(
        &self,
        form: &str,
    ) -> bool {
        self.loaded_by(form).is_some_and(|segment| !self.is_data_segment(segment))
    }

    /// The segment an address through `base` reads without a prefix: the
    /// stack's through the stack pointer or the frame register, the data
    /// segment's through any other. None where the target has no segments.
    pub fn default_segment(
        &self,
        base: RegId,
    ) -> Option<RegId> {
        if self.is_stack(base) || self.is_frame(base) { self.stack_segment } else { self.data_segment }
    }

    /// Whether a register `offset_bytes` wide can hold the offset of an
    /// address read through a segment: a base or index of the description that
    /// is neither the frame register nor the stack pointer, which select their
    /// own segment.
    pub fn holds_a_segment_offset(
        &self,
        register: RegId,
        offset_bytes: i64,
    ) -> bool {
        let root = self.root(register);
        self.bytes(register) == Some(offset_bytes)
            && (self.in_class(root, class::BASE) || self.in_class(root, class::INDEX))
            && !self.is_frame(register)
            && !self.is_stack(register)
    }

    /// Whether `register` is an integer register: one the tables name by
    /// width.
    pub fn integer(
        &self,
        register: RegId,
    ) -> bool {
        self.in_class(register, class::INT)
    }

    /// The integer registers `bytes` wide, by iced's number.
    pub fn integer_of(
        &self,
        bytes: i64,
    ) -> &[RegId] {
        let slot = match bytes {
            1 => 1,
            2 => 2,
            4 => 3,
            _ => return &[],
        };
        self.cache
            .integers[slot]
            .get_or_init(
                || {
                    let mut found: Vec<RegId> = self
                        .entries()
                        .filter(|(_, one)| one.classes & class::INT != 0 && i64::from(one.bits / 8) == bytes)
                        .map(|(register, _)| register)
                        .collect();
                    found.sort_by_key(|one| one.index());
                    found.dedup();
                    found
                },
            )
    }

    /// The registers of the RFP class (FP0..FP6, the x87 registers greedy
    /// allocates), by iced's number: the order they are allocated in.
    pub fn rfp_registers(&self) -> &[RegId] {
        self.cache.rfp.get_or_init(|| {
            let mut found: Vec<RegId> =
                self.entries().filter(|(_, one)| one.classes & class::RFP != 0).map(|(register, _)| register).collect();
            found.sort_by_key(|one| one.index());
            found.dedup();
            found
        })
    }

    /// Every integer register (the 8, 16 and 32-bit views), wide ones first,
    /// each width by iced's number: the order the allocator's tables have
    /// always been walked in.
    pub fn integer_registers(&self) -> &[RegId] {
        self.cache.integers[0]
            .get_or_init(|| [4, 2, 1].into_iter().flat_map(|bytes| self.integer_of(bytes).to_vec()).collect())
    }

    /// The same register named at the width an operand needs.
    pub fn named(
        &self,
        register: RegId,
        bytes: i64,
    ) -> RegId {
        self.view(self.root(register), bytes as u32 * 8).unwrap_or(register)
    }

    /// The 8-bit allocation order (`@bytes`), empty where the target has none.
    pub fn byte_order(&self) -> &'static [RegId] {
        self.bytes
    }

    /// The segment registers, by iced's number.
    pub fn segments(&self) -> impl Iterator<Item = RegId> + '_ {
        self.entries().filter(|(_, one)| one.classes & class::SEGMENT != 0).map(|(id, _)| id)
    }

    /// The bytes of the target's SSE registers, 0 where it has none: a value
    /// that wide is held in one.
    pub fn xmm_bytes(&self) -> u32 {
        *self
            .cache
            .xmm_bytes
            .get_or_init(
                || self.entries().find(|(_, one)| one.classes & class::XMM != 0).map_or(0, |(_, one)| one.bits / 8),
            )
    }

    /// How wide this register is, or None where the target does not say: a
    /// segment register is a word.
    pub fn width_of(
        &self,
        register: RegId,
    ) -> Option<i64> {
        if self.is_segment(register) { Some(2) } else { self.bytes(register).filter(|_| self.integer(register)) }
    }

    /// Whether this is a register the target describes at all.
    pub fn described(
        &self,
        register: RegId,
    ) -> bool {
        self.integer(register) || self.is_segment(register) || self.in_class(register, class::XMM)
    }

    /// The entries in iced's number order, with their registers.
    pub fn entries(&self) -> impl Iterator<Item = (RegId, &Entry)> {
        self.table.iter().flatten().map(|entry| (entry.id, entry))
    }
}

/// A target's register file, carried by what a compile holds (a body, a pass,
/// the classes, the segments) so that every query asks the file of the target
/// being compiled. Two are equal when they are the same file.
#[derive(Clone, Copy)]
pub struct Regs(pub &'static Info);

impl std::ops::Deref for Regs {
    type Target = Info;

    fn deref(&self) -> &Info {
        self.0
    }
}

impl std::fmt::Debug for Regs {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(formatter, "Regs({:p})", self.0)
    }
}

impl PartialEq for Regs {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for Regs {}

/// A set of registers, a bit each: the register file is 256 ids, so the set is
/// four words held in place, where a `BTreeSet<RegId>` allocated a node and
/// walked a tree for each of the 85,000 registers `masm::_roots` inserted in a
/// d_faces -O2 compile. Iteration is ascending, `BTreeSet`'s order.
#[derive(Clone, Copy, Default, Eq, PartialEq, Hash)]
pub struct RegSet([u64; 4]);

impl RegSet {
    #[must_use]
    pub const fn new() -> Self {
        Self([0; 4])
    }

    /// Whether `register` was not in the set.
    pub fn insert(
        &mut self,
        register: RegId,
    ) -> bool {
        let at = register.index();
        let fresh = self.0[at / 64] & (1 << (at % 64)) == 0;
        self.0[at / 64] |= 1 << (at % 64);
        fresh
    }

    #[must_use]
    pub fn contains(
        &self,
        register: &RegId,
    ) -> bool {
        let at = register.index();
        self.0[at / 64] & (1 << (at % 64)) != 0
    }

    /// The registers in either set.
    #[must_use]
    #[inline]
    pub fn union(
        self,
        other: &RegSet,
    ) -> Self {
        Self(std::array::from_fn(|at| self.0[at] | other.0[at]))
    }

    /// `other`'s registers added to this set, in place
    /// (`BitVector::operator|=`).
    #[inline]
    pub fn union_with(
        &mut self,
        other: &RegSet,
    ) {
        for (word, more) in self.0.iter_mut().zip(other.0) {
            *word |= more;
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.iter().map(|word| word.count_ones() as usize).sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0 == [0; 4]
    }

    /// The members, ascending.
    pub fn iter(&self) -> impl Iterator<Item = RegId> + '_ {
        self.0
            .iter()
            .enumerate()
            .flat_map(
                |(at, word)| {
                    let mut left = *word;
                    std::iter::from_fn(move || {
                        (left != 0).then(|| {
                            let bit = left.trailing_zeros() as usize;
                            left &= left - 1;
                            RegId::from_index(at * 64 + bit)
                        })
                    })
                },
            )
    }
}

impl FromIterator<RegId> for RegSet {
    fn from_iter<I: IntoIterator<Item = RegId>>(registers: I) -> Self {
        let mut set = Self::new();
        set.extend(registers);
        set
    }
}

impl Extend<RegId> for RegSet {
    fn extend<I: IntoIterator<Item = RegId>>(
        &mut self,
        registers: I,
    ) {
        for register in registers {
            self.insert(register);
        }
    }
}

impl std::fmt::Debug for RegSet {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        formatter.debug_set().entries(self.iter()).finish()
    }
}

/// A register as its number, which is how every dump names it.
impl llrm_support::pyrepr::Repr for RegId {
    fn repr(&self) -> String {
        self.iced().repr()
    }
}

impl llrm_support::pyset::PyHash for RegId {
    fn py_hash(&self) -> i64 {
        self.iced().py_hash()
    }
}
