//! The register class of each value: the set of registers an instruction's
//! operands allow it to be in.
//!
//! # How it works
//! A value is confined when an instruction names it where only some registers
//! will do: a byte operand (the byte-addressable registers), a word address
//! base or index (BX/BP, SI/DI, or any of them, as the cell's shape needs), the
//! selector of a far access (a segment register), or the register a far load
//! writes. Each instruction states this by `contribution`, a list of `Item`s: a
//! `Restrict` (value, `Kind`, `Role`), or a note that the value is read as a
//! `Numeric` or `Selecting`, as half of an address `Pair`, or as a `Far`
//! selector. The sets are the target's (`RegisterClasses`,
//! `Segments::selectors`); the instruction only says which one applies.
//!
//! `classes` intersects every restriction on a value. Then four steps refine
//! it:
//!
//! 1. A value read as a selector and never as a number takes the selector
//! registers. 2. The selector a far load writes takes them as well. 3. A
//! `[base+index]` pair with scale 1 can be spelled either way round, so each
//! connected component of such pairs is two-coloured (base side and index
//! side). The colouring is chosen by fewest base-register clobbers, then by
//! preferred indexes, then fewest bases; a component that is not bipartite, or
//! cannot take either colouring, keeps the spelling in the source. The
//! clobbers are read from the caller's intervals and masks, or, where it holds
//! none, from the intervals of the pairs' values alone, found when a component
//! has two colourings to choose between. 4. After
//! coalescing, a phi's result and arguments are one web and take the web's
//! class where its members agree. A member with a class of its own keeps it.
//!
//! ```text
//!  mov al, [bx+si]       al : Bytes          bx : base, si : index
//!  mov es, x ; mov [es:di], 0                x  : selectors
//!  mov ax, [p+q]         p, q a Pair: p base / q index, or the reverse
//! ```
//!
//! `classes_with(.., optimistic = true)` is the checker's reading of a body the
//! coalescer has not merged: the two sides of a phi edge's copy also join, and
//! address classes are read through webs. `classes_given` takes intervals and
//! masks a caller already has. `confining_uses` lists each confining read
//! (`Use`), for `constrain`. `Scan` keeps per-value counts of what
//! instructions say, so a rewrite takes the counts of the instructions it lost
//! and gained and recomputes only the values they name.
//!
//! `violations` checks a body against its classes, per register file (general,
//! selector): at each point it asks whether the live values fit the file
//! (`Crowded`) and whether the values an instruction acts on can have distinct
//! registers of their classes, a bipartite matching (`Unmatched`). A body with
//! none can be coloured with copies; one with some cannot, whatever the
//! allocator does. `LLRM_DEBUG=regclass` runs it after the phases from
//! `ssaspill` to `coalesce`.
//!
//! # Example
//! A far pointer's segment `v1` enters a loop through the phi `v2`, which a far
//! load reads as the selector of `[es:v3]`:
//!
//! ```lir
//! === body
//!     block 0x0000
//!       0x0001  v1 := mov 0x1
//!       0x0002  v3 := mov 0x2
//!       0x0003  jmp
//!     block 0x0001
//!       v2 := phi 0x0000:v1 0x0001:v2
//!       0x0004  v4 := mov [[es:r0+0x0] v3@unplaced]
//!       0x0005  jne
//!     block 0x0002
//!       0x0006  ret
//! ```
//!
//! | value | class            | why                       |
//! |-------|------------------|---------------------------|
//! | v1    | {ES, FS, GS}     | the web of the phi `v2`   |
//! | v2    | {ES, FS, GS}     | read only as a selector   |
//! | v3    | {BX, BP, SI, DI} | address base              |
//! | v4    | none             | a plain loaded word       |
//!
//! From unit test `test_a_segment_only_a_phi_reads_has_the_class_of_its_web` in
//! `regclass.rs`; the classes are what `classes` returns for the m16 register
//! sets.
//!
//! # Runs
//! Not a pass: queried on demand, by `coalesce`, `constrain`, `allocate`, at
//! every -O level. `LLRM_DEBUG=regclass` also checks each body after the phases
//! from `ssaspill` to `coalesce`.
//!
//! # Provides
//! One answer to "which registers may this value be in", owned here (#441).
//! Read by [`crate::backend::ssaspill`], [`crate::backend::allocate`],
//! [`crate::coalescer::pass`], [`crate::backend::constrain`],
//! and `target.rs`.
//!
//! # Needs
//! The target's register sets ([`crate::backend::classes`],
//! [`crate::backend::target`]) and, for the pair colouring, the body's
//! intervals and clobber masks (`allocate`).

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::analysis::intervals as ranges;
use crate::backend::classes::RegisterClasses;
use crate::backend::masks::{_clobbered, _masks};
use crate::backend::target::{self, Segments};
use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::{HashSet, IndexMap, IndexSet};

pub type Classes = IndexMap<u32, BTreeSet<RegId>>;

fn _restrict(
    out: &mut Classes,
    value: u32,
    choices: &BTreeSet<RegId>,
) {
    let now: BTreeSet<RegId> = match out.get(&value) {
        Some(had) => had.intersection(choices).copied().collect(),
        None => choices.clone(),
    };
    out.insert(value, now);
}

/// The register class each value is confined to, where it is confined.
pub fn classes(
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
) -> Classes {
    classes_with(body, prefer_indexes, segments, registers, false)
}

/// `MachineRegisterInfo::recomputeRegClass` (MachineRegisterInfo.cpp, called
/// from `LiveRangeEdit::calculateRegClassAndHint`, LiveRangeEdit.cpp#L426-L430,
/// and SplitKit.cpp#L628): the registers `value` is confined to by the
/// instructions that name it, which a value made from another by the spiller or
/// a split does not inherit. None where its instructions confine it to nothing.
///
/// DEVIATE CF-1: the m16 `[base+index]` pairs colour base {BX} and index
/// {SI,DI}; the pin has no such model. A pair member keeps its side.
/// DEVIATE CF-4: the answer comes from `classes` over the whole body, so a call
/// costs a body; a caller that makes many values asks `classes` once.
///
/// No caller yet: splitkit's spiller and split sites recompute per value
/// (`reclass_orders`, #3668); this whole-body entry is for the base/index pair
/// members they leave to the parent, if a miss shows.
pub fn recompute_class(
    body: &LirBody,
    value: u32,
    segments: &Segments,
    registers: &RegisterClasses,
) -> Option<BTreeSet<RegId>> {
    classes(body, &BTreeSet::new(), segments, registers).get(&value).cloned()
}

/// `classes`; `optimistic` is the checker's reading of a body the coalescer has
/// not merged yet: the two sides of a phi edge's copy also join, and address
/// classes are read through webs. An allocator cannot take that, the two sides
/// being values of their own, and SsaSpill does not price it yet.
pub fn classes_with(
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
    optimistic: bool,
) -> Classes {
    collected(body, prefer_indexes, segments, registers, optimistic, None, None)
}

/// What a caller that has already found the body's intervals and clobber masks
/// gives `classes`, which reads them for its `[word+word]` roles and would
/// number the body and find both again.
pub struct Found<'a> {
    pub live: &'a dyn crate::backend::live::Ranges,
    pub masks: &'a crate::backend::masks::Masks,
}

/// `classes`, given the body's intervals and masks.
pub fn classes_given(
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
    found: &Found,
) -> Classes {
    collected(body, prefer_indexes, segments, registers, false, None, Some(found))
}

/// How an instruction names a value that confines it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Byte,
    Base,
    Index,
    Xmm,
}

/// One read of a value that confines it: where it is read, and the registers
/// that read allows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Use {
    pub block: usize,
    pub insn: usize,
    pub value: u32,
    pub class: BTreeSet<RegId>,
    pub role: Role,
    /// The instruction writes the value there, rather than reads it.
    pub defining: bool,
    /// A half of an address pair the roles left registers for: it confines the
    /// value to the addressing registers, and only `distinct_classes`' cold
    /// reads use it.
    pub spare: bool,
}

/// Every read that confines a value on its own, a byte operand or an address
/// base or index. A value whose uses share no register is one the classes leave
/// with none.
pub fn confining_uses(
    body: &LirBody,
    segments: &Segments,
    registers: &RegisterClasses,
) -> Vec<Use> {
    let mut uses = Vec::new();
    collected(body, &BTreeSet::new(), segments, registers, false, Some(&mut uses), None);
    uses
}

/// `confining_uses` and `classes` of one scan.
pub fn confining(
    body: &LirBody,
    segments: &Segments,
    registers: &RegisterClasses,
) -> (Vec<Use>, Classes) {
    let mut uses = Vec::new();
    let found = collected(body, &BTreeSet::new(), segments, registers, false, Some(&mut uses), None);
    (uses, found)
}

/// Which of the sets of registers an operand confines a value to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Kind {
    WordIndexes,
    Addressing,
    WordBases,
    Bytes,
    /// A 16-byte value: scalar `float` and `double` live in an SSE register.
    Xmm,
}

impl Kind {
    const ALL: [Kind; 5] = [Kind::WordIndexes, Kind::Addressing, Kind::WordBases, Kind::Bytes, Kind::Xmm];

    fn registers<'a>(
        self,
        registers: &'a RegisterClasses,
    ) -> &'a BTreeSet<RegId> {
        match self {
            Kind::WordIndexes => &registers.word_indexes,
            Kind::Addressing => &registers.addressing,
            Kind::WordBases => &registers.word_bases,
            Kind::Bytes => &registers.byte_words,
            Kind::Xmm => &registers.xmm_set,
        }
    }
}

/// What one instruction says of the values it names, apart from the others.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Item {
    Restrict {
        value: u32,
        kind: Kind,
        role: Role,
        defining: bool,
    },
    /// Read as a number, not as a segment selector.
    Numeric(u32),
    /// Read as a segment selector.
    Selecting(u32),
    /// A base and an index that may be either way round.
    Pair(u32, u32),
    /// The selector of a far load.
    Far(u32),
}

/// The sets of registers an operand confines a value to, and the registers a
/// value may take at all: what `RegisterClasses` holds, without the instruction
/// forms.
#[derive(Clone)]
pub struct Sets {
    pub available: Vec<RegId>,
    kinds: [BTreeSet<RegId>; 5],
}

impl Sets {
    pub fn of(registers: &RegisterClasses) -> Self {
        Self { available: registers.available.clone(), kinds: Kind::ALL.map(|kind| kind.registers(registers).clone()) }
    }
}

/// What the instructions that name a value say of the registers it may take.
pub enum Confinement {
    /// The reads are not all the ones that confine by themselves (a segment
    /// selector, a base and an index that may be either way round): the
    /// value keeps the class it had.
    Unknown,
    /// Nothing says.
    Free,
    /// The registers every confining read allows.
    Only(BTreeSet<RegId>),
}

/// `MRI.recomputeRegClass`: the class of `value` from the instructions that
/// name it alone.
pub fn confinement_of<'a>(
    regs: Regs,
    insns: impl Iterator<Item = &'a crate::model::lir::Insn>,
    value: u32,
    sets: &Sets,
) -> Confinement {
    let mut only: Option<BTreeSet<RegId>> = None;
    let mut items = Vec::new();
    for one in insns {
        items.clear();
        contribution(regs, one, &mut items);
        for item in &items {
            match *item {
                Item::Restrict { value: named, kind, .. } if named == value => {
                    let allowed = &sets.kinds[Kind::ALL.iter().position(|k| *k == kind).expect("a kind")];
                    only = Some(match only {
                        Some(so_far) => so_far.intersection(allowed).copied().collect(),
                        None => allowed.clone(),
                    });
                }
                Item::Selecting(named) | Item::Far(named) if named == value => return Confinement::Unknown,
                Item::Pair(base, index) if base == value || index == value => return Confinement::Unknown,
                _ => {}
            }
        }
        if let Some(what) = &one.what {
            if target::far_load(regs, what) && matches!(&what.dests[1], Loc::Held(held) if held.value == value) {
                return Confinement::Unknown;
            }
        }
    }
    only.map_or(Confinement::Free, Confinement::Only)
}

/// Whether the form of `what` fills operand `at` with an xmm register (its row
/// has `x` there and no `r`), as LLVM's operand class `FR64`/`VR128` says. A
/// width of the xmm register does not: a 16-byte integer pair is not one. An
/// instruction with no row (a copy) is read by the width.
fn xmm_filled(
    regs: Regs,
    what: &crate::model::ir::Semantics,
    width: u32,
    at: usize,
    xmm: &mut std::cell::OnceCell<Option<u32>>,
) -> bool {
    if regs.xmm_bytes() == 0 {
        return false;
    }
    match xmm.get_or_init(|| llrm_x86::effects::xmm_operands(&regs.effects, what)) {
        Some(mask) => mask >> at & 1 != 0,
        None => width == regs.xmm_bytes(),
    }
}

/// The items of `one`, appended.
pub fn contribution(
    regs: Regs,
    one: &crate::model::lir::Insn,
    items: &mut Vec<Item>,
) {
    // A value an instruction takes from or leaves in an xmm register is held
    // in one: LLVM gives the vreg copied from a physical register that
    // register's class. Its only reads may be copies, which name no class.
    for (held, register) in one.requires.iter().chain(&one.delivers) {
        if regs.in_class(*register, llrm_lir::registers::class::XMM) {
            let defining = one.delivers.iter().any(|(other, _)| other.value == held.value);
            items.push(Item::Restrict { value: held.value, kind: Kind::Xmm, role: Role::Xmm, defining });
        }
    }
    let Some(what) = &one.what else {
        return;
    };
    // A string op's segment operands are selectors, as a far access's are.
    let segments: &[Loc] = match (what.op, what.sources.len()) {
        (Operation::Copy, 4 | 5) => &what.sources[what.sources.len() - 2..],
        (Operation::Fill, 3 | 4) => &what.sources[what.sources.len() - 1..],
        _ => &[],
    };
    for place in segments {
        if let Loc::Held(held) = place {
            items.push(Item::Selecting(held.value));
        }
    }
    // Which operands the form fills with an xmm register, found once and only
    // if a register operand asks.
    let mut xmm = std::cell::OnceCell::new();
    for (at, place) in what.dests.iter().chain(&what.sources).enumerate() {
        if let Loc::Mem(cell) = place {
            if let Some(selector) = cell.selector {
                items.push(Item::Selecting(selector.value));
            }
            if let Some(base) = cell.base {
                items.push(Item::Numeric(base.value));
            }
        }
        if let Loc::Held(held) = place {
            if (held.width != 2 || !_SEGMENT_OPERANDS.contains(&what.op))
                && !segments.iter().any(|one| matches!(one, Loc::Held(other) if other.value == held.value))
            {
                items.push(Item::Numeric(held.value));
            }
        }
        if let Loc::Mem(cell) = place {
            if let (Some(base), None) = (cell.base, cell.index) {
                if base.width == 2 {
                    let kind = if cell.addr.is_some_and(|addr| addr.space == Space::Frame) {
                        Kind::WordIndexes
                    } else {
                        Kind::Addressing
                    };
                    items.push(Item::Restrict { value: base.value, kind, role: Role::Base, defining: false });
                }
            }
            if let Some(index) = cell.index {
                items.push(Item::Numeric(index.value));
                if index.width == 2 {
                    if cell.base.is_some_and(|base| base.width == 2) && cell.scale == 1 {
                        items.push(Item::Pair(cell.base.expect("checked").value, index.value));
                    } else {
                        items.push(Item::Restrict {
                            value: index.value,
                            kind: Kind::WordIndexes,
                            role: Role::Index,
                            defining: false,
                        });
                        if let Some(base) = cell.base {
                            items.push(Item::Restrict {
                                value: base.value,
                                kind: Kind::WordBases,
                                role: Role::Base,
                                defining: false,
                            });
                        }
                    }
                }
            }
        }
        if let Loc::Held(held) = place {
            if held.width == 1 {
                items.push(Item::Restrict {
                    value: held.value,
                    kind: Kind::Bytes,
                    role: Role::Byte,
                    defining: what.dests.contains(place),
                });
            }
            if xmm_filled(regs, what, held.width, at, &mut xmm) {
                items.push(Item::Restrict {
                    value: held.value,
                    kind: Kind::Xmm,
                    role: Role::Xmm,
                    defining: what.dests.contains(place),
                });
            }
        }
    }
    if target::far_load(regs, what) {
        if let Loc::Held(held) = &what.dests[1] {
            items.push(Item::Far(held.value));
        }
    }
}

/// What the instructions of a body say of its values, counted, so that a body
/// made from another by a rewrite takes the counts of the instructions it
/// lost and gained and works out the class of the values they name only.
#[derive(Clone, Default)]
pub struct Scan {
    counts: crate::support::hash::HashMap<u32, Counts>,
    pairs: crate::support::hash::HashMap<(u32, u32), u32>,
    /// Each value's class from its counts, before the roles of word addresses
    /// and the webs.
    pre: Classes,
}

#[derive(Clone, Copy, Default)]
struct Counts {
    kinds: [u32; 5],
    selecting: u32,
    numeric: u32,
    far: u32,
}

impl Counts {
    fn is_empty(&self) -> bool {
        self.kinds == [0; 5] && self.selecting == 0 && self.numeric == 0 && self.far == 0
    }
}

impl Scan {
    /// The counts of `body`'s instructions.
    pub fn of(
        body: &LirBody,
        registers: &RegisterClasses,
        segments: &Segments,
    ) -> Self {
        let regs = body.regs();
        let mut scan = Scan::default();
        let mut touched = Vec::new();
        for one in body.blocks.iter().flat_map(|block| &block.insns) {
            scan.count(regs, one, true, &mut touched);
        }
        scan.classify(touched, registers, segments);
        scan
    }

    /// These counts with the instructions in `gone` taken out and those in
    /// `added` put in.
    pub fn after(
        self,
        gone: &[std::sync::Arc<crate::model::lir::Insn>],
        added: &[std::sync::Arc<crate::model::lir::Insn>],
        registers: &RegisterClasses,
        segments: &Segments,
    ) -> Self {
        let regs = segments.registers;
        let mut scan = self;
        let mut touched = Vec::new();
        for one in gone {
            scan.count(regs, one, false, &mut touched);
        }
        for one in added {
            scan.count(regs, one, true, &mut touched);
        }
        scan.classify(touched, registers, segments);
        scan
    }

    /// Where the counts of `value` are held, for a test that a rewrite's scan
    /// is edited where it stands and not copied.
    #[cfg(test)]
    pub fn counts_address(
        &self,
        value: u32,
    ) -> Option<usize> {
        self.counts.get(&value).map(|counts| std::ptr::from_ref(counts) as usize)
    }

    fn count(
        &mut self,
        regs: Regs,
        one: &crate::model::lir::Insn,
        put: bool,
        touched: &mut Vec<u32>,
    ) {
        let mut items = Vec::new();
        contribution(regs, one, &mut items);
        for item in items {
            let (value, slot): (u32, fn(&mut Counts) -> &mut u32) = match item {
                Item::Restrict { value, kind, .. } => (
                    value,
                    match kind {
                        Kind::WordIndexes => |c| &mut c.kinds[0],
                        Kind::Addressing => |c| &mut c.kinds[1],
                        Kind::WordBases => |c| &mut c.kinds[2],
                        Kind::Bytes => |c| &mut c.kinds[3],
                        Kind::Xmm => |c| &mut c.kinds[4],
                    },
                ),
                Item::Numeric(value) => (value, |c| &mut c.numeric),
                Item::Selecting(value) => (value, |c| &mut c.selecting),
                Item::Far(value) => (value, |c| &mut c.far),
                Item::Pair(base, index) => {
                    let count = self.pairs.entry((base, index)).or_insert(0);
                    if put {
                        *count += 1;
                    } else {
                        *count -= 1;
                        if *count == 0 {
                            self.pairs.remove(&(base, index));
                        }
                    }
                    continue;
                }
            };
            let counts = self.counts.entry(value).or_default();
            if put {
                *slot(counts) += 1;
            } else {
                *slot(counts) -= 1;
            }
            touched.push(value);
        }
    }

    fn classify(
        &mut self,
        mut touched: Vec<u32>,
        registers: &RegisterClasses,
        segments: &Segments,
    ) {
        touched.sort_unstable();
        touched.dedup();
        let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
        for value in touched {
            let counts = self.counts[&value];
            if counts.is_empty() {
                self.counts.remove(&value);
            }
            let mut sets: Vec<&BTreeSet<RegId>> = Kind::ALL
                .iter()
                .zip(counts.kinds)
                .filter(|(_, n)| *n > 0)
                .map(|(kind, _)| kind.registers(registers))
                .collect();
            if (counts.selecting > 0 && counts.numeric == 0) || counts.far > 0 {
                sets.push(&selectors);
            }
            match sets.split_first() {
                None => {
                    self.pre.swap_remove(&value);
                }
                Some((first, rest)) => {
                    let class: BTreeSet<RegId> =
                        first.iter().copied().filter(|one| rest.iter().all(|set| set.contains(one))).collect();
                    self.pre.insert(value, class);
                }
            }
        }
    }

    /// `classes_given` of the body these counts are of.
    pub fn classes(
        &self,
        body: &LirBody,
        prefer_indexes: &BTreeSet<u32>,
        segments: &Segments,
        registers: &RegisterClasses,
        found: &Found,
    ) -> Classes {
        let mut out = self.pre.clone();
        let mut pairs: Vec<(u32, u32)> = self.pairs.keys().copied().collect();
        pairs.sort_unstable();
        _word_address_roles(&pairs, &mut out, body, prefer_indexes, segments, registers, Some(found));
        let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
        let (selecting, numeric) = (
            |value: u32| self.counts.get(&value).is_some_and(|c| c.selecting > 0),
            |value: u32| self.counts.get(&value).is_some_and(|c| c.numeric > 0),
        );
        _through_webs(body, &selecting, &numeric, &selectors, selector_bytes(segments), false, &mut out);
        _through_xmm_copies(body, registers, &mut out);
        out
    }
}

thread_local! {
    /// Scans of a body for the classes its reads confine values to, counted
    /// for the test that fixes how many one allocation makes.
    #[cfg(test)]
    pub(crate) static SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A set of register numbers as bits: `collected` inserts one for most
/// operands of the body, which a `BTreeSet` made 2.8 M Ir of d_faces.
#[derive(Default)]
pub(crate) struct Bits(Vec<u64>);

impl Bits {
    pub(crate) fn insert(
        &mut self,
        value: u32,
    ) {
        let word = value as usize / 64;
        if word >= self.0.len() {
            self.0.resize(word + 1, 0);
        }
        self.0[word] |= 1 << (value % 64);
    }

    pub(crate) fn contains(
        &self,
        value: u32,
    ) -> bool {
        self.0.get(value as usize / 64).is_some_and(|word| word >> (value % 64) & 1 == 1)
    }

    /// The members `other` lacks, ascending as a `BTreeSet` difference is.
    pub(crate) fn without<'a>(
        &'a self,
        other: &'a Bits,
    ) -> impl Iterator<Item = u32> + 'a {
        self.0
            .iter()
            .enumerate()
            .flat_map(
                move |(at, &word)| {
                    let mut left = word & !other.0.get(at).copied().unwrap_or(0);
                    std::iter::from_fn(move || {
                        (left != 0).then(|| {
                            let bit = left.trailing_zeros();
                            left &= left - 1;
                            at as u32 * 64 + bit
                        })
                    })
                },
            )
    }
}

fn collected(
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
    optimistic: bool,
    mut uses: Option<&mut Vec<Use>>,
    found: Option<&Found>,
) -> Classes {
    #[cfg(test)]
    SCANS.with(|one| one.set(one.get() + 1));
    let regs = body.regs();
    let mut out: Classes = IndexMap::default();
    let mut selecting = Bits::default();
    let mut numeric = Bits::default();
    let mut word_pairs: Vec<(u32, u32)> = Vec::new();
    // Where each pair is read, for the caller that asks for the reads.
    let mut pair_sites: Vec<(usize, usize, u32, u32)> = Vec::new();
    let _scan = llrm_support::debug::span("classes scan");
    let mut items = Vec::new();
    for (at, block) in body.blocks.iter().enumerate() {
        for (position, one) in block.insns.iter().enumerate() {
            items.clear();
            contribution(regs, one, &mut items);
            for item in &items {
                match *item {
                    Item::Restrict { value, kind, role, defining } => {
                        let choices = kind.registers(registers);
                        // The reads are kept only for a caller that asks for
                        // them.
                        if let Some(uses) = uses.as_deref_mut() {
                            uses.push(Use {
                                block: at,
                                insn: position,
                                value,
                                class: choices.clone(),
                                role,
                                defining,
                                spare: false,
                            });
                        }
                        _restrict(&mut out, value, choices);
                    }
                    Item::Numeric(value) => {
                        numeric.insert(value);
                    }
                    Item::Selecting(value) => {
                        selecting.insert(value);
                    }
                    Item::Pair(base, index) => {
                        word_pairs.push((base, index));
                        if uses.is_some() {
                            pair_sites.push((at, position, base, index));
                        }
                    }
                    Item::Far(_) => {}
                }
            }
        }
    }
    drop(_scan);
    let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
    for value in selecting.without(&numeric) {
        _restrict(&mut out, value, &selectors);
    }
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if let Some(what) = &one.what {
            if target::far_load(regs, what) {
                if let Loc::Held(held) = &what.dests[1] {
                    _restrict(&mut out, held.value, &selectors);
                }
            }
        }
    }
    llrm_support::debug::timed("classes word roles", || {
        _word_address_roles(&word_pairs, &mut out, body, prefer_indexes, segments, registers, found)
    });
    // A half of a pair whose roles left it no register: the read as the pair
    // spells it confines it to the registers of its side, which share none
    // with its other reads. Kept for `distinct_classes`, which gives such a
    // read a copy of its own.
    if let Some(uses) = uses.as_deref_mut() {
        for (block, insn, base, index) in pair_sites {
            for (value, role, kind) in [(base, Role::Base, Kind::WordBases), (index, Role::Index, Kind::WordIndexes)] {
                let none = out.get(&value).is_some_and(BTreeSet::is_empty);
                uses.push(Use {
                    block,
                    insn,
                    value,
                    class: if none {
                        kind.registers(registers).clone()
                    } else {
                        registers.word_bases.union(&registers.word_indexes).copied().collect()
                    },
                    role,
                    defining: false,
                    spare: !none,
                });
            }
        }
    }
    llrm_support::debug::timed("classes webs", || {
        _through_webs(
            body,
            &|value| selecting.contains(value),
            &|value| numeric.contains(value),
            &selectors,
            selector_bytes(segments),
            optimistic,
            &mut out,
        )
    });
    _through_xmm_copies(body, registers, &mut out);
    out
}

/// A plain `mov` of one value to another is one register class
/// (`COPY` between vregs of one class): a value only copied to or from an
/// xmm-confined one is held in an xmm register too. The allocator's copies
/// around a value pinned in `xmm0` (a `double` call result passed on to the
/// return) are such moves, and took a general register for it. A `movd` or
/// `movq` between classes has its own name.
fn _through_xmm_copies(
    body: &LirBody,
    registers: &RegisterClasses,
    out: &mut Classes,
) {
    if registers.xmm_set.is_empty() {
        return;
    }
    let mut web: IndexMap<u32, u32> = IndexMap::default();
    fn find(
        web: &mut IndexMap<u32, u32>,
        value: u32,
    ) -> u32 {
        let up = *web.entry(value).or_insert(value);
        if up == value {
            return value;
        }
        let top = find(web, up);
        web.insert(value, top);
        top
    }
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        let Some(what) = &one.what else { continue };
        if let (Operation::Move, Some("mov"), [Loc::Held(into)], [Loc::Held(from)], None) =
            (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice(), one.group)
        {
            if into.value != from.value {
                let (a, b) = (find(&mut web, into.value), find(&mut web, from.value));
                web.insert(a, b);
            }
        }
    }
    let mut members: IndexMap<u32, Vec<u32>> = IndexMap::default();
    for value in web.keys().copied().collect::<Vec<_>>() {
        let top = find(&mut web, value);
        members.entry(top).or_default().push(value);
    }
    for list in members.values() {
        if list.iter().any(|value| out.get(value).is_some_and(|class| *class == registers.xmm_set)) {
            for value in list {
                out.entry(*value).or_insert_with(|| registers.xmm_set.clone());
            }
        }
    }
}

/// A phi's result and arguments, and the two sides of a phi edge's copy, are
/// one value once the coalescer has merged them: each takes the class of the
/// web. A value that only a phi or such a copy reads, as one a loop reads
/// through its header phi is, has no class of its own.
/// How many bytes a segment selector is: the width of the segment registers the
/// program model places selectors in.
fn selector_bytes(segments: &Segments) -> Option<i64> {
    segments.selectors.first().and_then(|register| segments.registers.width_of(*register))
}

fn _through_webs(
    body: &LirBody,
    selecting: &dyn Fn(u32) -> bool,
    numeric: &dyn Fn(u32) -> bool,
    selectors: &BTreeSet<RegId>,
    selector_bytes: Option<i64>,
    optimistic: bool,
    out: &mut Classes,
) {
    let mut web: IndexMap<u32, u32> = IndexMap::default();
    fn find(
        web: &mut IndexMap<u32, u32>,
        value: u32,
    ) -> u32 {
        let up = *web.entry(value).or_insert(value);
        if up == value {
            return value;
        }
        let top = find(web, up);
        web.insert(value, top);
        top
    }
    let mut of_phis: HashSet<u32> = HashSet::default();
    for phi in body.blocks.iter().flat_map(|block| &block.phis) {
        of_phis.insert(phi.result);
        for (_, value) in &phi.incoming {
            of_phis.insert(*value);
            let (one, other) = (find(&mut web, phi.result), find(&mut web, *value));
            web.insert(one, other);
        }
    }
    // A selector carried round a loop by its phi is a plain `mov` to the
    // register of the phi and another back: one value, which a segment
    // register can hold the whole way. A copy off a call's result is not
    // one: the word is in the call's register, and the web would ask a
    // segment register of it.
    let carries = |one: &Insn| {
        one.group.is_none()
            && one.what.as_ref().is_some_and(|what| {
                matches!(
                    (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()),
                    (Operation::Move, Some("mov"), [Loc::Held(into)], [Loc::Held(from)])
                        if Some(i64::from(into.width)) == selector_bytes && into.width == from.width
                )
            })
    };
    for one in body
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter(|one| if one.group.is_some() { optimistic } else { carries(one) })
    {
        if let ([into], [from]) = (one.defines.as_slice(), one.uses.as_slice()) {
            if one.group.is_none() && !of_phis.contains(into) && !of_phis.contains(from) {
                continue;
            }
            let (a, b) = (find(&mut web, *into), find(&mut web, *from));
            web.insert(a, b);
        }
    }
    let mut members: IndexMap<u32, Vec<u32>> = IndexMap::default();
    for value in web.keys().copied().collect::<Vec<_>>() {
        let top = find(&mut web, value);
        members.entry(top).or_default().push(value);
    }
    for list in members.values().filter(|list| list.len() > 1) {
        // The web has a class where its members agree; a member that has one of
        // its own keeps it, as the allocator copies between members of
        // different classes.
        let mut agreed: Option<BTreeSet<RegId>> = None;
        let mut agree = true;
        for value in list {
            if let Some(mine) = out.get(value) {
                match &agreed {
                    Some(so_far) if so_far != mine => agree = false,
                    Some(_) => {}
                    None => agreed = Some(mine.clone()),
                }
            }
        }
        if agree
            && agreed.is_none()
            && list.iter().any(|value| selecting(*value))
            && !list.iter().any(|value| numeric(*value))
        {
            agreed = Some(selectors.clone());
        }
        // Address classes are the checker's reading only: SsaSpill holds fewer
        // values where it sees them, which the allocator's own spills
        // do not make up for (deedlines COPPER -Os: 112k reloads, 140k with
        // them).
        let wanted = optimistic
            || agreed.as_ref().is_some_and(|class| class.iter().all(|register| selectors.contains(register)));
        if let (true, true, Some(class)) = (agree, wanted, agreed) {
            // A member read as a number (an `xor`) stays a general register:
            // the copies between it and the selectors are real
            // moves.
            let agreed_selectors = class.iter().all(|register| selectors.contains(register));
            let bare: Vec<u32> = list
                .iter()
                .copied()
                .filter(|value| !out.contains_key(value) && !(agreed_selectors && numeric(*value)))
                .collect();
            for value in bare {
                out.insert(value, class.clone());
            }
        }
    }
}

/// Choose BX versus SI/DI for commutative `[word+word]` graphs.
fn _word_address_roles(
    pairs: &[(u32, u32)],
    confined: &mut Classes,
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
    found: Option<&Found>,
) {
    // No pair, no component: nothing below would run, and it numbers the body,
    // finds every interval and builds the masks first.
    if pairs.is_empty() {
        return;
    }
    let mut adjacent: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
    for (base, index) in pairs {
        adjacent.entry(*base).or_default().insert(*index);
        adjacent.entry(*index).or_default().insert(*base);
    }
    let mut unseen: BTreeSet<u32> = adjacent.keys().copied().collect();
    // Only a component with two ways to spell its pairs is priced by what
    // clobbers the word base over its values; the body's intervals and
    // masks are found for the first, if the caller holds none.
    let own = std::cell::OnceCell::new();
    let word_base = *registers.word_bases.iter().next().expect("one word base");

    let base_penalty = |values: &BTreeSet<u32>| -> i64 {
        let (live, masks): (&dyn crate::backend::live::Ranges, &crate::backend::masks::Masks) = match found {
            Some(found) => (found.live, found.masks),
            None => {
                let (live, masks) = own.get_or_init(|| {
                    let numbered = ranges::indexed(body);
                    // The pairs' values only: the rest are never asked.
                    let live = ranges::intervals_where(
                        body,
                        &numbered,
                        &crate::analysis::frequency::Frequency::of(body),
                        &|value| adjacent.contains_key(&value),
                    );
                    (live, _masks(body, &numbered, segments))
                });
                (live, masks)
            }
        };
        values
            .iter()
            .filter_map(|value| live.range(*value))
            .map(|interval| i64::from(_clobbered(interval, word_base, masks, 2)))
            .sum()
    };

    let allowed = |confined: &Classes, values: &BTreeSet<u32>, choices: &BTreeSet<RegId>| -> bool {
        values
            .iter()
            .all(
                |value| match confined.get(value) {
                    Some(had) => had.intersection(choices).next().is_some(),
                    None => !choices.is_empty(),
                },
            )
    };

    let restrict = |confined: &mut Classes, values: &BTreeSet<u32>, choices: &BTreeSet<RegId>| {
        for value in values {
            _restrict(confined, *value, choices);
        }
    };

    while let Some(&seed) = unseen.iter().next() {
        let mut colors: IndexMap<u32, u8> = IndexMap::from_iter([(seed, 0)]);
        let mut work = vec![seed];
        let mut bipartite = true;
        while let Some(value) = work.pop() {
            for other in &adjacent[&value] {
                match colors.get(other) {
                    None => {
                        let color = 1 - colors[&value];
                        colors.insert(*other, color);
                        work.push(*other);
                    }
                    Some(color) if *color == colors[&value] => bipartite = false,
                    Some(_) => {}
                }
            }
        }
        let component: BTreeSet<u32> = colors.keys().copied().collect();
        for value in &component {
            unseen.remove(value);
        }
        let source_spelling = |confined: &mut Classes| {
            for (base, index) in pairs {
                if component.contains(base) {
                    restrict(confined, &BTreeSet::from([*base]), &registers.word_bases);
                    restrict(confined, &BTreeSet::from([*index]), &registers.word_indexes);
                }
            }
        };
        if !bipartite {
            source_spelling(confined);
            continue;
        }
        let sides: (BTreeSet<u32>, BTreeSet<u32>) = (
            colors.iter().filter(|(_value, color)| **color == 0).map(|(value, _)| *value).collect(),
            colors.iter().filter(|(_value, color)| **color != 0).map(|(value, _)| *value).collect(),
        );
        let options: Vec<(&BTreeSet<u32>, &BTreeSet<u32>)> = [(&sides.0, &sides.1), (&sides.1, &sides.0)]
            .into_iter()
            .filter(|(left, right)| {
                allowed(confined, left, &registers.word_bases) && allowed(confined, right, &registers.word_indexes)
            })
            .collect();
        if options.is_empty() {
            source_spelling(confined);
            continue;
        }
        let key = |option: &(&BTreeSet<u32>, &BTreeSet<u32>)| {
            (
                base_penalty(option.0),
                option.0.intersection(prefer_indexes).count(),
                option.0.len(),
                option.0.iter().copied().collect::<Vec<u32>>(),
            )
        };
        // `min` keeps the first of equal keys.
        let mut best = options[0];
        for option in &options[1..] {
            if key(option) < key(&best) {
                best = *option;
            }
        }
        let (bases, indexes) = (best.0.clone(), best.1.clone());
        restrict(confined, &bases, &registers.word_bases);
        restrict(confined, &indexes, &registers.word_indexes);
    }
}

pub(crate) const _SEGMENT_OPERANDS: [Operation; 3] = [Operation::Move, Operation::Push, Operation::Pop];

/// A point of a body at which the values live cannot all sit in registers their
/// classes allow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub block: i64,
    pub position: usize,
    pub file: &'static str,
    pub why: Why,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Why {
    /// More values live than the file has registers.
    Crowded { live: usize, registers: usize },
    /// Values an instruction acts on, which no assignment gives distinct
    /// registers of their classes.
    Unmatched(Vec<u32>),
}

/// Whether every value of `wanted` can take a distinct register of its own set:
/// a bipartite matching.
fn matched(wanted: &[(u32, BTreeSet<RegId>)]) -> bool {
    fn place(
        at: usize,
        wanted: &[(u32, BTreeSet<RegId>)],
        taken: &mut IndexMap<RegId, usize>,
        seen: &mut BTreeSet<RegId>,
    ) -> bool {
        for register in &wanted[at].1 {
            if !seen.insert(*register) {
                continue;
            }
            let holder = taken.get(register).copied();
            if holder.is_none_or(|other| place(other, wanted, taken, seen)) {
                taken.insert(*register, at);
                return true;
            }
        }
        false
    }
    let mut taken: IndexMap<RegId, usize> = IndexMap::default();
    (0..wanted.len()).all(|at| place(at, wanted, &mut taken, &mut BTreeSet::new()))
}

/// The points of `body` at which the values live do not fit the registers their
/// classes allow, per register file: the general registers and the segment
/// registers. A body with none can be coloured, with copies where a value waits
/// in another class's register; one with some cannot, whatever the allocator
/// does. `skip` names the values another pass places (x87, pinned, inputs).
pub fn violations(
    body: &LirBody,
    segments: &Segments,
    registers: &RegisterClasses,
    skip: &BTreeSet<u32>,
) -> Vec<Violation> {
    let confined = classes_with(body, &BTreeSet::new(), segments, registers, true);
    let (_, live_out) = crate::backend::liverows::live(body);
    let general: BTreeSet<RegId> = registers.available.iter().map(|one| crate::backend::masks::_whole(*one)).collect();
    let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
    let files: [(&'static str, &BTreeSet<RegId>); 2] = [("general", &general), ("selector", &selectors)];
    let in_file = |value: u32, file: &BTreeSet<RegId>, general_file: bool| match confined.get(&value) {
        Some(class) => class.iter().any(|one| file.contains(&crate::backend::masks::_whole(*one))),
        None => general_file,
    };
    let class_in = |value: u32, file: &BTreeSet<RegId>| -> BTreeSet<RegId> {
        match confined.get(&value) {
            Some(class) => {
                class.iter().map(|one| crate::backend::masks::_whole(*one)).filter(|one| file.contains(one)).collect()
            }
            None => file.clone(),
        }
    };
    let mut out = Vec::new();
    for block in &body.blocks {
        let mut live: BTreeSet<u32> = live_out[&block.at].iter().filter(|value| !skip.contains(value)).collect();
        for (position, one) in block.insns.iter().enumerate().rev() {
            // Two states per instruction: after it (what is live, its results
            // among them) and before it (what it reads is live, its
            // results not yet): a result takes the register of an operand that
            // dies. The copies of one phi edge are one parallel
            // copy: a point at its ends, not between its copies.
            if one.defines.is_empty()
                && one.uses.is_empty()
                && one.what.as_ref().is_none_or(|what| what.name.as_deref().is_none_or(str::is_empty))
            {
                continue;
            }
            // An instruction that does nothing (a placeholder left where a copy
            // was made unnecessary) does not end the group.
            let real = |other: &&std::sync::Arc<crate::model::lir::Insn>| {
                !(other.defines.is_empty()
                    && other.uses.is_empty()
                    && other.what.as_ref().is_none_or(|what| what.name.as_deref().is_none_or(str::is_empty)))
            };
            let grouped = |other: Option<&std::sync::Arc<crate::model::lir::Insn>>| {
                one.group.is_some() && other.is_some_and(|other| other.group == one.group)
            };
            let next = block.insns.iter().skip(position + 1).find(real);
            let previous = block.insns.iter().take(position).rev().find(real);
            let (inside_after, inside_before) = (grouped(next), grouped(previous));
            let after = live.clone();
            let mut before = live.clone();
            for value in &one.defines {
                before.remove(value);
            }
            before.extend(one.uses.iter().copied().filter(|value| !skip.contains(value)));
            for (state, acting_values, inside) in
                [(&after, &one.defines, inside_after), (&before, &one.uses, inside_before)]
            {
                if inside {
                    continue;
                }
                for (name, file) in files {
                    let members: Vec<u32> =
                        state.iter().copied().filter(|value| in_file(*value, file, name == "general")).collect();
                    if members.len() > file.len() {
                        out.push(Violation {
                            block: block.at,
                            position,
                            file: name,
                            why: Why::Crowded { live: members.len(), registers: file.len() },
                        });
                        continue;
                    }
                    let acting: Vec<(u32, BTreeSet<RegId>)> = acting_values
                        .iter()
                        .copied()
                        .filter(|value| members.contains(value))
                        .collect::<BTreeSet<u32>>()
                        .into_iter()
                        .map(|value| (value, class_in(value, file)))
                        .collect();
                    if !matched(&acting) {
                        out.push(Violation {
                            block: block.at,
                            position,
                            file: name,
                            why: Why::Unmatched(acting.iter().map(|(value, _)| *value).collect()),
                        });
                    }
                }
            }
            live = before;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Addr, Held, Mem, Semantics};
    use crate::model::lir::{Insn, LirBlock, Phi};

    /// `Bits` replaced two `BTreeSet<u32>`: the difference must come out
    /// ascending and across word boundaries, or `_restrict` meets the values
    /// in another order and `out` is built in another order.
    #[test]
    fn bits_give_the_ascending_difference_a_btree_set_gave() {
        let (mut selecting, mut numeric) = (Bits::default(), Bits::default());
        let (mut a, mut b) = (BTreeSet::new(), BTreeSet::new());
        for value in [200u32, 3, 64, 63, 65, 130, 7, 1000, 64] {
            selecting.insert(value);
            a.insert(value);
        }
        for value in [3u32, 65, 999, 1000] {
            numeric.insert(value);
            b.insert(value);
        }
        assert_eq!(selecting.without(&numeric).collect::<Vec<_>>(), a.difference(&b).copied().collect::<Vec<_>>());
        assert!(selecting.contains(1000) && !selecting.contains(999) && !selecting.contains(5000));
    }

    /// A far pointer's segment that a loop reads only through its header phi:
    /// FADETOCOLOR's selectors had no class of their own after SsaSpill and
    /// counted among the general registers (3 over, 63 points after
    /// PhiElimination) while they sat in selector registers.
    #[test]
    fn test_a_segment_only_a_phi_reads_has_the_class_of_its_web() {
        let (entry_segment, segment, base, loaded) = (1, 2, 3, 4);
        let cell = Mem {
            base: Some(Held { value: base, width: 2 }),
            selector: Some(Held { value: segment, width: 2 }),
            ..Mem::new(Some(Addr { segment: (RegId::ES).iced(), ..Addr::new(Space::Far, 0) }), 2)
        };
        let semantics = |op, name: &str, dests, sources, target| Semantics {
            name: Some(name.into()),
            dests,
            sources,
            target,
            ..Semantics::new(op)
        };
        let held = |value| Loc::Held(Held { value, width: 2 });
        let make =
            |at, defines: Vec<u32>, uses: Vec<u32>, what| Arc::new(Insn::new(at, None, Some(what), defines, uses));
        let entry = LirBlock {
            at: 0,
            insns: vec![
                make(
                    1,
                    vec![entry_segment],
                    vec![],
                    semantics(
                        Operation::Move,
                        "mov",
                        vec![held(entry_segment)],
                        vec![Loc::Imm(crate::model::ir::Imm { value: 1, width: 2, address: None })],
                        None,
                    ),
                ),
                make(
                    2,
                    vec![base],
                    vec![],
                    semantics(
                        Operation::Move,
                        "mov",
                        vec![held(base)],
                        vec![Loc::Imm(crate::model::ir::Imm { value: 2, width: 2, address: None })],
                        None,
                    ),
                ),
                make(3, vec![], vec![], semantics(Operation::Jump, "jmp", vec![], vec![], Some(1))),
            ]
            .into(),
            succ: vec![1].into(),
            phis: vec![].into(),
            cold: false,
            align: 0,
            max_skip: 0,
        };
        let looped = LirBlock {
            at: 1,
            insns: vec![
                make(
                    4,
                    vec![loaded],
                    vec![base, segment],
                    semantics(Operation::Move, "mov", vec![held(loaded)], vec![Loc::Mem(cell)], None),
                ),
                make(5, vec![], vec![], semantics(Operation::Branch, "jne", vec![], vec![], Some(1))),
            ]
            .into(),
            succ: vec![1, 2].into(),
            phis: vec![Phi { result: segment, incoming: vec![(0, entry_segment), (1, segment)] }].into(),
            cold: false,
            align: 0,
            max_skip: 0,
        };
        let exit = LirBlock {
            at: 2,
            insns: vec![make(6, vec![], vec![loaded], semantics(Operation::Return, "ret", vec![], vec![], None))]
                .into(),
            succ: vec![].into(),
            phis: vec![].into(),
            cold: false,
            align: 0,
            max_skip: 0,
        };
        let body = LirBody::new(
            "f",
            0,
            vec![entry.into(), looped.into(), exit.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let segments = &target::BUILT_IN;
        let webs = classes(&body, &BTreeSet::new(), segments, &crate::backend::classes::RegisterClasses::m16());
        assert_eq!(webs.get(&entry_segment), webs.get(&segment));
        assert!(webs.get(&entry_segment).is_some());
    }
    /// A reload made for a stored word: the spiller gave it its parent's class,
    /// {EBX} (ls.c `_ls_animate@3`, `[abs+516 + %53 + v16] := mov %51`), where
    /// its instructions only store it. Recomputed, it is confined to
    /// nothing and the base keeps the address class.
    #[test]
    fn test_a_stored_word_recomputes_to_no_class_and_its_base_keeps_one() {
        let (base, word) = (1, 2);
        let held = |value| Loc::Held(Held { value, width: 2 });
        let semantics = |op, name: &str, dests, sources| Semantics {
            name: Some(name.into()),
            dests,
            sources,
            ..Semantics::new(op)
        };
        let imm = |value| Loc::Imm(crate::model::ir::Imm { value, width: 2, address: None });
        let make =
            |at, defines: Vec<u32>, uses: Vec<u32>, what| Arc::new(Insn::new(at, None, Some(what), defines, uses));
        let cell = Mem { base: Some(Held { value: base, width: 2 }), ..Mem::new(None, 2) };
        let block = LirBlock {
            at: 0,
            insns: vec![
                make(1, vec![base], vec![], semantics(Operation::Move, "mov", vec![held(base)], vec![imm(2)])),
                make(2, vec![word], vec![], semantics(Operation::Move, "mov", vec![held(word)], vec![imm(3)])),
                make(
                    3,
                    vec![],
                    vec![base, word],
                    semantics(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![held(word)]),
                ),
                make(4, vec![], vec![], semantics(Operation::Return, "ret", vec![], vec![])),
            ]
            .into(),
            succ: vec![].into(),
            phis: vec![].into(),
            cold: false,
            align: 0,
            max_skip: 0,
        };
        let body = LirBody::new("f", 0, vec![block.into()], IndexMap::default(), IndexMap::default());
        let registers = crate::backend::classes::RegisterClasses::m16();
        let of = |value| recompute_class(&body, value, &target::BUILT_IN, &registers);
        assert_eq!(of(word), None);
        assert!(of(base).is_some_and(|class| class.contains(&RegId::BX) && !class.contains(&RegId::AX)));
    }

    /// What the form says fills an operand, not the register's width: an
    /// `addsd` operand held 8 bytes got no xmm restriction (the value was
    /// free to take a general register), and a 16-byte operand of an
    /// integer `add` was confined to xmm.
    #[test]
    fn test_an_operand_is_xmm_by_its_form_not_its_width() {
        let regs = Regs(&llrm_x86_m64::REGISTER_INFO);
        let restricted = |name: &str, width: u32| {
            let held = |value| Loc::Held(Held { value, width });
            let what = Semantics {
                name: Some(name.into()),
                dests: vec![held(1)],
                sources: vec![held(1), held(2)],
                ..Semantics::new(Operation::Binary)
            };
            let mut items = Vec::new();
            contribution(regs, &Insn::new(1, None, Some(what), vec![1], vec![1, 2]), &mut items);
            items.iter().any(|item| matches!(item, Item::Restrict { kind: Kind::Xmm, .. }))
        };
        assert!(restricted("addsd", 8), "an xmm operand held 8 bytes");
        assert!(restricted("addsd", 16));
        assert!(!restricted("add", 16), "a 16-byte integer operand");
    }
}

/// Values the spiller leaves alone: x87 values, pinned ones, and the body's
/// inputs.
pub(crate) fn untouchable(body: &LirBody) -> BTreeSet<u32> {
    let mut out = floating(body);
    out.extend(body.pins.keys().copied().chain(body.inputs.iter().copied()));
    out
}

/// Values no general register holds: x87 values and wider ones, and every phi
/// web they join.
pub(crate) fn floating(body: &LirBody) -> BTreeSet<u32> {
    let regs = body.regs();
    let mut out: BTreeSet<u32> = BTreeSet::new();
    for one in body.insns() {
        if one.what.as_ref().is_some_and(|what| target::_on_the_stack(regs, what) || what.op.is_x87()) {
            out.extend(one.uses.iter().chain(&one.defines).copied());
        }
        // A value no general register holds: a float, whatever reads it.
        for place in one.what.iter().flat_map(|what| what.dests.iter().chain(&what.sources)) {
            for held in crate::model::ir::values(place) {
                if held.width > 4 {
                    out.insert(held.value);
                }
            }
        }
    }
    // A phi joins its result and arguments: the web is as wide as its widest
    // member.
    loop {
        let mut grew = false;
        for block in &body.blocks {
            for phi in &block.phis {
                let web: Vec<u32> =
                    std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value)).collect();
                if web.iter().any(|value| out.contains(value)) {
                    for value in web {
                        grew |= out.insert(value);
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    out
}

/// Each far cell whose selector is also read as a number, or pinned to a
/// general register, reached through ES: None where no cell is rewritten.
pub fn explicit_selectors_given(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, RegId>>,
    segments: &Segments,
    confined: &Classes,
) -> Option<LirBody> {
    let regs = body.regs();
    let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
    let empty = IndexMap::default();
    let pinned = pinned.unwrap_or(&empty);
    let mut conflicted: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            for place in what.dests.iter().chain(&what.sources) {
                if let Loc::Mem(cell) = place {
                    if let Some(selector) = cell.selector {
                        if confined.get(&selector.value) != Some(&selectors)
                            || !regs.is_segment(*pinned.get(&selector.value).unwrap_or(&regs.far()))
                        {
                            conflicted.insert(selector.value);
                        }
                    }
                }
            }
        }
    }
    if conflicted.is_empty() {
        return None;
    }

    let through_es = |place: &Loc| -> Loc {
        if let Loc::Mem(cell) = place {
            if cell.selector.is_some_and(|selector| conflicted.contains(&selector.value)) {
                let addr = cell.addr.expect("a far cell has an address");
                return Loc::Mem(Mem {
                    selector: None,
                    addr: Some(Addr { segment: regs.far().iced(), ..addr }),
                    ..cell.clone()
                });
            }
        }
        place.clone()
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            let named: Vec<Held> = match &one.what {
                None => Vec::new(),
                Some(what) => what
                    .dests
                    .iter()
                    .chain(&what.sources)
                    .filter_map(|place| match place {
                        Loc::Mem(cell) => cell.selector.filter(|selector| conflicted.contains(&selector.value)),
                        _ => None,
                    })
                    .collect(),
            };
            if named.is_empty() {
                insns.push(Arc::clone(one));
                continue;
            }
            let what = one.what.as_ref().expect("named is empty without semantics");
            let what = Semantics {
                dests: what.dests.iter().map(&through_es).collect(),
                sources: what.sources.iter().map(&through_es).collect(),
                ..what.clone()
            };
            let requires: IndexSet<(Held, RegId)> =
                one.requires.iter().copied().chain(named.iter().map(|held| (*held, regs.far()))).collect();
            let uses: IndexSet<u32> = one.uses.iter().copied().chain(named.iter().map(|held| held.value)).collect();
            let mut made = (**one).clone();
            made.what = Some(what);
            made.requires = requires.into_iter().collect();
            made.uses = uses.into_iter().collect();
            insns.push(Arc::new(made));
        }
        blocks.push(block.with_insns(insns));
    }
    Some(body.with_blocks(blocks))
}
