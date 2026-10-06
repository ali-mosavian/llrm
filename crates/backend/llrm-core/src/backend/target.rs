//! Port of `qbopt/backend/target.py`: what the machine has, and what each
//! instruction requires.
//!
//! LLVM's `TargetRegisterInfo` and `TargetInstrInfo` in one module; the two
//! halves are the two sections below. A register class is the unit an
//! allocator works in: 16-bit addressing reaches memory through bx, bp, si
//! and di and nothing else.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use iced_x86::Register;
use llrm_x86_code16::instructions;

use crate::abi::machine::{self, Machine};
use crate::support::hash::IndexMap;

use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::support::pyset::PySet;

// ---------------------------------------------------------------- registers

pub static ADDRESSING: LazyLock<BTreeSet<Register>> =
    LazyLock::new(|| BTreeSet::from([Register::BX, Register::BP, Register::SI, Register::DI]));
// `[bx+si]`: a word base and a word index are each confined to their half.
pub static WORD_BASES: LazyLock<BTreeSet<Register>> = LazyLock::new(|| llrm_x86_code16::word_bases().into_iter().collect());
pub static WORD_INDEXES: LazyLock<BTreeSet<Register>> = LazyLock::new(|| llrm_x86_code16::WORD_INDEXES.into());

/// Where an operand has to live: one register, or any of a set.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Need {
    pub r#where: BTreeSet<Register>,
}

impl Need {
    /// The register, where there is only one it can be.
    pub fn fixed(&self) -> Option<Register> {
        if self.r#where.len() == 1 {
            self.r#where.iter().next().copied()
        } else {
            None
        }
    }
}

/// One operand of one instruction, by side and position.
///
/// A requirement is about an operand, not about the register it happens to
/// name: `imul`'s dx:ax is a fact about the first and second destination.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Occurrence {
    pub side: String, // "dest" or "source"
    pub index: usize,
}

impl Occurrence {
    pub fn new(side: &str, index: usize) -> Self {
        Self {
            side: side.to_owned(),
            index,
        }
    }
}

/// Every operand this instruction requires in one particular register.
///
/// The one place those are written down: the `fixed` column of the form that
/// takes this instruction's operands (`x86.instr`); `reads` and `writes` read it
/// too. An operand that is an immediate is no register: a shift's count or an
/// `in`'s port written as one pins nothing, and a segment register is pinned
/// only while it is a held value, a placed one being where it is.
pub fn requirements(what: &Semantics) -> IndexMap<Occurrence, Register> {
    let mut out = IndexMap::default();
    if _on_the_stack(what) {
        return out;
    }
    let Some(name) = what.name.as_deref() else { return out };
    let key = (name.to_owned(), what.op.as_str(), what.dests.len(), what.sources.len());
    let Some(pins) = PINS.get(&key) else { return out };
    for (side, index, register) in pins {
        let places = match side {
            instructions::Side::Dest => &what.dests,
            instructions::Side::Source => &what.sources,
        };
        let pinned = match &places[*index] {
            Loc::Imm(_) => false,
            Loc::Held(_) => true,
            _ => !SEGMENTS.contains(register),
        };
        if pinned {
            out.insert(Occurrence::new(if *side == instructions::Side::Dest { "dest" } else { "source" }, *index), *register);
        }
    }
    out
}

/// Each form's pins, by the mnemonic, the operation and the operand counts that
/// pick it out. A pin that tells the members of a family apart (`les`, `lds`, `lfs`
/// and `lgs` take the same operands and differ in the selector register) is a
/// choice the allocator makes, not a requirement, and is left out.
static PINS: LazyLock<std::collections::HashMap<(String, &'static str, usize, usize), Vec<(instructions::Side, usize, Register)>>> = LazyLock::new(|| {
    let operations: std::collections::HashMap<&str, &'static str> = Operation::ALL.iter().map(|op| (op.as_str(), op.as_str())).collect();
    let mut pins = std::collections::HashMap::new();
    for form in instructions::FORMS.iter() {
        let chosen = |side: instructions::Side, index: usize, root: &str| {
            instructions::FORMS.iter().any(|other| {
                other.operation == form.operation
                    && other.dests == form.dests
                    && other.sources == form.sources
                    && other.fixed.iter().any(|(s, i, r)| *s == side && *i == index && r != root)
            })
        };
        let required = form.fixed.iter().filter(|(side, index, root)| !chosen(*side, *index, root)).map(|(side, index, root)| (*side, *index, root_register(root))).collect();
        pins.entry((form.name.clone(), operations[form.operation.as_str()], form.dests.len(), form.sources.len())).or_insert(required);
    }
    pins
});

/// The register a form's `fixed` column names by its root: `ax` is EAX.
fn root_register(root: &str) -> Register {
    match root {
        "ax" => Register::EAX,
        "bx" => Register::EBX,
        "cx" => Register::ECX,
        "dx" => Register::EDX,
        "si" => Register::ESI,
        "di" => Register::EDI,
        "bp" => Register::EBP,
        "sp" => Register::ESP,
        "es" => Register::ES,
        "ds" => Register::DS,
        "fs" => Register::FS,
        "gs" => Register::GS,
        other => unreachable!("x86.instr names no register `{other}`"),
    }
}

fn _root(register: Register) -> Register {
    ir::root(register)
}

/// Whether this is an x87 operation, which shares no register with the rest.
///
/// `ir` models `fdivp` as a DIVIDE, and the widening rule claimed it reads
/// dx:ax.
pub fn _on_the_stack(what: &Semantics) -> bool {
    what.dests
        .iter()
        .chain(&what.sources)
        .any(|one| matches!(one, Loc::St(_)))
        || what.name.as_deref().unwrap_or("").starts_with('f')
}

/// Whether `xchg` takes these two operands: a general register or a memory
/// cell each, not both memory, and no segment register.
pub fn exchangeable(one: &Loc, other: &Loc) -> bool {
    let exchanged = |place: &Loc| match place {
        Loc::Reg(reg) => !SEGMENTS.contains(&reg.register),
        Loc::Mem(_) => true,
        _ => false,
    };
    exchanged(one) && exchanged(other) && !(matches!(one, Loc::Mem(_)) && matches!(other, Loc::Mem(_)))
}

/// The width `push` carries this place at: a register of a word or more, a
/// memory cell of a word or more, or a frame cell, which owns at least a word.
pub fn pushed_width(place: &Loc) -> Option<u32> {
    match place {
        Loc::Reg(reg) if reg.width >= 2 => Some(reg.width),
        Loc::Mem(cell) if matches!(cell.width, 2 | 4) => Some(cell.width),
        Loc::Mem(cell) if cell.width == 1 && cell.in_frame() => Some(2),
        _ => None,
    }
}

/// The width `pop` fills this place at: what `push` carries, but not `cs`.
pub fn popped_width(place: &Loc) -> Option<u32> {
    match place {
        Loc::Reg(reg) if reg.register == Register::CS => None,
        other => pushed_width(other),
    }
}

/// Whether an x87 comparison reaches the flags through AX: `fnstsw ax; sahf`
/// follows it, so nothing may live in AX across it.
pub fn status_through_ax(what: &Semantics) -> bool {
    what.op == Operation::Compare && what.sources.iter().any(|one| matches!(one, Loc::St(_)) || matches!(one, Loc::Held(held) if held.width == 10))
}

/// The register a two-address instruction reads and writes as one.
///
/// `add ax,[c]` is one register at two moments; x86 says so by naming the
/// same operand twice.
pub fn tied(what: &Semantics) -> Option<Register> {
    if _on_the_stack(what) || what.dests.is_empty() || what.sources.is_empty() {
        return None;
    }
    if let (Loc::Reg(into), Loc::Reg(outof)) = (&what.dests[0], &what.sources[0]) {
        if _root(into.register) == _root(outof.register) {
            return Some(_root(into.register));
        }
    }
    None
}

/// Registers this operation reads whether or not it names them, keyed on
/// the root it reads.
pub fn reads(what: &Semantics) -> IndexMap<Register, Need> {
    let mut out = IndexMap::default();
    for (r#where, register) in requirements(what) {
        if r#where.side == "source" {
            out.insert(
                register,
                Need {
                    r#where: BTreeSet::from([register]),
                },
            );
        }
    }
    for one in what.dests.iter().chain(&what.sources) {
        // `getattr(one, "through")`, `getattr(one, "index_through")` and
        // `getattr(getattr(one, "addr"), "base")`, each None where absent.
        let (through, index_through, base) = match one {
            Loc::Mem(mem) => (
                Some(mem.through),
                Some(mem.index_through),
                mem.addr.as_ref().map(|addr| addr.base),
            ),
            Loc::Address(address) => (
                Some(address.through),
                None,
                address.addr.as_ref().map(|addr| addr.base),
            ),
            _ => (None, None, None),
        };
        for r#where in [through, index_through, base].into_iter().flatten() {
            if r#where != Register::None {
                out.insert(
                    _root(r#where),
                    Need {
                        r#where: ADDRESSING.iter().map(|x| _root(*x)).collect(),
                    },
                );
            }
        }
    }
    out
}

/// Registers this operation writes whether or not it names them.
pub fn writes(what: &Semantics) -> IndexMap<Register, Need> {
    let mut out = IndexMap::default();
    if _on_the_stack(what) {
        return out;
    }
    for (r#where, register) in requirements(what) {
        if r#where.side == "dest" {
            out.insert(
                register,
                Need {
                    r#where: BTreeSet::from([register]),
                },
            );
        }
    }
    out
}

// Every register a value may be placed in: `mir.TRACKED`, what the raise
// follows.
pub const AVAILABLE: [Register; 6] = llrm_x86_code16::GENERAL;

// What this may hand out for an operand that reaches memory, which is not
// what the encoding permits: bp is a legal base and also the frame pointer.
pub static BASES: LazyLock<Vec<Register>> = LazyLock::new(|| {
    let addressing: BTreeSet<Register> = ADDRESSING.iter().map(|x| ir::root(*x)).collect();
    AVAILABLE
        .into_iter()
        .filter(|one| addressing.contains(one))
        .collect()
});

pub static WIDE: LazyLock<PySet<Register>> = LazyLock::new(|| {
    [
        Register::EAX,
        Register::ECX,
        Register::EDX,
        Register::EBX,
        Register::ESI,
        Register::EDI,
        Register::EBP,
        Register::ESP,
    ]
    .into_iter()
    .collect()
});
pub static NARROW: LazyLock<PySet<Register>> = LazyLock::new(|| {
    [
        Register::AX,
        Register::CX,
        Register::DX,
        Register::BX,
        Register::SI,
        Register::DI,
        Register::BP,
        Register::SP,
    ]
    .into_iter()
    .collect()
});
// The byte halves. BC reaches for them to clear a high byte and to read one
// byte of an array.
pub static BYTE: LazyLock<PySet<Register>> = LazyLock::new(|| {
    [
        Register::AL,
        Register::CL,
        Register::DL,
        Register::BL,
        Register::AH,
        Register::CH,
        Register::DH,
        Register::BH,
    ]
    .into_iter()
    .collect()
});

// The width each register names, and the register file at each width.
// Built from the three rows rather than from ir.ROOT, which has no
// byte-wide entries.
pub static WIDTHS: LazyLock<IndexMap<Register, i64>> = LazyLock::new(|| {
    let mut widths = IndexMap::default();
    for (_row, _size) in [(&*WIDE, 4), (&*NARROW, 2), (&*BYTE, 1)] {
        for _one in _row.iter() {
            widths.insert(*_one, _size);
        }
    }
    widths
});
pub static AT_WIDTH: LazyLock<IndexMap<Register, IndexMap<i64, Register>>> = LazyLock::new(|| {
    let mut at_width: IndexMap<Register, IndexMap<i64, Register>> = IndexMap::default();
    for (_row, _size) in [(&*WIDE, 4), (&*NARROW, 2), (&*BYTE, 1)] {
        for _one in _row.iter() {
            // setdefault, not assignment: al and ah both root to eax, and
            // the later one resolved a width-1 value to `ah`.
            at_width
                .entry(ir::root(*_one))
                .or_default()
                .entry(_size)
                .or_insert(*_one);
        }
    }
    at_width
});

/// The same register named at the width an operand needs.
pub fn named(register: Register, width: i64) -> Register {
    AT_WIDTH
        .get(&ir::root(register))
        .and_then(|widths| widths.get(&width))
        .copied()
        .unwrap_or(register)
}

/// The registers an operand may take, in the order to try them.
///
/// LLVM's `AllocationOrder`. `None` means the operand said nothing.
pub fn order(r#where: Option<&BTreeSet<Register>>, segments: &Segments) -> Vec<Register> {
    let Some(r#where) = r#where else {
        return AVAILABLE.to_vec();
    };
    let wanted: BTreeSet<Register> = r#where.iter().map(|one| ir::root(*one)).collect();
    if !wanted.is_empty() && wanted.is_subset(&SEGMENTS) {
        return segments
            .selectors
            .iter()
            .copied()
            .filter(|one| wanted.contains(one))
            .collect();
    }
    AVAILABLE
        .into_iter()
        .filter(|one| wanted.contains(one))
        .collect()
}

// The segment registers. Operands, not allocatable.
pub static SEGMENTS: LazyLock<BTreeSet<Register>> = LazyLock::new(|| {
    BTreeSet::from([
        Register::ES,
        Register::CS,
        Register::SS,
        Register::DS,
        Register::FS,
        Register::GS,
    ])
});

/// The segment registers as the machine's program model assigns them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segments {
    /// Where a selector value may be placed, most preferred first: every one
    /// the program model does not reserve. Where the stack reaches the data
    /// group, the data segment register is free between the points that need
    /// it (`needs_data_group`), so it comes last.
    pub selectors: Vec<Register>,
    /// The one every access without a prefix reads.
    pub data: Register,
    /// The one that reaches the data group while `data` holds something
    /// else: the stack's, where the stack lives in the data group.
    pub through: Option<Register>,
    /// The selector stride a huge pointer takes per carried 64K, as a shift:
    /// the machine's (`Machine::huge_shift`), where it states one.
    pub huge_shift: Option<u32>,
}

impl Segments {
    pub fn of(machine: &Machine) -> Self {
        // A flat machine has no selector to place: DS is only what string operations read.
        let Some(segments) = machine.segments.as_ref() else {
            return Self { selectors: Vec::new(), data: Register::DS, through: None, huge_shift: None };
        };
        let named = |one: &Register, name: &String| name.eq_ignore_ascii_case(crate::backend::select::SEGMENTS[one]);
        let register = |name: &String| {
            *crate::backend::select::SEGMENTS
                .keys()
                .find(|one| named(one, name))
                .unwrap_or_else(|| panic!("the machine's {name} is not a segment register"))
        };
        let mut reserved = vec![&segments.stack, &segments.code];
        if !segments.stack_is_data {
            reserved.push(&segments.data);
        }
        Self {
            selectors: [Register::ES, Register::FS, Register::GS, Register::DS, Register::SS, Register::CS]
                .into_iter()
                .filter(|one| !reserved.iter().any(|name| named(one, name)))
                .collect(),
            data: register(&segments.data),
            through: segments.stack_is_data.then(|| register(&segments.stack)),
            huge_shift: machine.huge_shift(),
        }
    }
}

/// The built-in machine's.
pub static BUILT_IN: LazyLock<Segments> = LazyLock::new(|| Segments::of(&machine::BUILT_IN));

/// The segment registers of `machine::BASIC`.
pub static BASIC: LazyLock<Segments> = LazyLock::new(|| Segments::of(&machine::BASIC));

/// Whether `one` needs the data segment register to hold the data group: it
/// calls, returns, traps or is opaque; it is an x87 instruction, whose
/// emulator fixup spells the segment itself; or it is a string instruction,
/// which reads the data segment without naming it.
pub fn needs_data_group(one: &crate::model::lir::Insn) -> bool {
    let Some(what) = &one.what else {
        return true;
    };
    if !one.clobbers.is_empty() && what.op == Operation::Nothing {
        return true;
    }
    if matches!(
        what.op,
        Operation::Call
            | Operation::Return
            | Operation::Escape
            | Operation::Barrier
            | Operation::Data
            | Operation::FloatLoad
            | Operation::FloatStore
            | Operation::FloatArith
            | Operation::FloatArithPop
            | Operation::FloatUnary
    ) {
        return true;
    }
    let name = what.name.as_deref().unwrap_or("");
    let bare = name.trim_start_matches("rep ").trim_start_matches("repe ").trim_start_matches("repne ");
    ["movs", "lods", "cmps", "outs", "stos", "scas", "ins", "xlat", "int", "wait", "fwait"].iter().any(|one| bare.starts_with(one))
        || name.starts_with('f')
}
// One far load per selector: its selector result is in the class, not pinned,
// and the rewriter spells the instruction for the register it was given.
pub static FAR_LOADS: LazyLock<IndexMap<Register, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([(Register::ES, "les"), (Register::FS, "lfs"), (Register::GS, "lgs"), (Register::DS, "lds")])
});

pub fn far_load(what: &Semantics) -> bool {
    what.op == Operation::Move
        && what.name.as_deref().is_some_and(|name| FAR_LOADS.values().any(|one| *one == name))
        && what.dests.len() == 2
}

/// Whether this is a register this target describes at all.
pub fn known(register: Register) -> bool {
    WIDTHS.contains_key(&register) || SEGMENTS.contains(&register)
}

/// How wide this register is, or None where the target does not say.
pub fn width_of(register: Register) -> Option<i64> {
    if SEGMENTS.contains(&register) {
        Some(2)
    } else {
        WIDTHS.get(&register).copied()
    }
}

// ------------------------------------------------------------ subregisters

// Which bytes of its root each register is, as a mask: LLVM's lane masks,
// with four lanes. `ir.ROOT` answers "same register file entry", not "same
// bytes", and al and ah are where those differ.
pub static LANES: LazyLock<IndexMap<Register, i64>> = LazyLock::new(|| {
    let mut lanes = IndexMap::default();
    for (_row, _mask) in [(&*WIDE, 0b1111), (&*NARROW, 0b0011)] {
        for _one in _row.iter() {
            lanes.insert(*_one, _mask);
        }
    }
    for _one in BYTE.iter() {
        let high = [Register::AH, Register::CH, Register::DH, Register::BH].contains(_one);
        lanes.insert(*_one, if high { 0b0010 } else { 0b0001 });
    }
    lanes
});

/// Which bytes of its root this register names.
pub fn lanes(register: Register) -> i64 {
    LANES.get(&register).copied().unwrap_or(0b1111)
}

/// Whether writing one can be seen by reading the other.
///
/// The same root is not enough: al and ah share eax and share no byte.
pub fn overlaps(one: Register, other: Register) -> bool {
    if ir::root(one) != ir::root(other) {
        return false;
    }
    lanes(one) & lanes(other) != 0
}

// Each register by the name a human writes, which is what runtime.py's own
// contracts are keyed on. Python's `iced_x86.Register` defines no
// `DontUse*` member, so those have no name here either.
pub static NAMES: LazyLock<IndexMap<Register, String>> = LazyLock::new(|| {
    let mut names: Vec<(String, Register)> = Register::values()
        .map(|register| (format!("{register:?}").to_uppercase(), register))
        .filter(|(name, _)| !name.starts_with("DONTUSE"))
        .collect();
    // `dir(Register)` is sorted by name.
    names.sort();
    names
        .into_iter()
        .map(|(name, register)| (register, name.to_lowercase()))
        .collect()
});

/// This register's own name, lowercase.
pub fn name_of(register: Register) -> String {
    NAMES
        .get(&register)
        .cloned()
        .unwrap_or_else(|| (register as i64).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Segments::of` panicked ("a segmented machine") on a flat machine, so no flat
    /// target reached the allocator; a flat machine places no selector.
    #[test]
    fn test_a_flat_machine_has_no_selector_to_place() {
        let flat = Machine::parse("addressing = \"flat\"\nsegment_end_faults = false\nfar_bss = false\ncpu = \"486\"\n", &["486"]).unwrap();
        let segments = Segments::of(&flat);
        assert!(segments.selectors.is_empty() && segments.through.is_none() && segments.huge_shift.is_none());
    }

    /// The allocator's bases are the encodable ones less the frame register: a new frame rule changes one place.
    #[test]
    fn test_the_allocators_bases_are_the_encodable_ones_but_the_frame() {
        let encodable: BTreeSet<Register> = llrm_x86_code16::ENCODABLE_BASES.into_iter().collect();
        let held: BTreeSet<Register> = encodable.iter().copied().filter(|&one| one != llrm_x86_code16::FRAME).collect();
        assert_eq!(*WORD_BASES, held);
        assert!(crate::backend::select::_WORD_BASES.iter().all(|one| encodable.contains(one)));
    }

    /// x86 arithmetic is two-address: the spill model charges the copy of a first operand that stays live.
    #[test]
    fn test_the_machine_says_its_arithmetic_is_two_address() {
        use llrm_mir::target::Machine;
        assert!(llrm_x86_code16::Dos::default().two_address());
    }

    /// The spill model counts the registers an address may use as the allocator restricts to.
    #[test]
    fn test_the_spill_models_address_registers_are_the_allocators() {
        use llrm_mir::target::Machine;
        let restricted: BTreeSet<Register> = WORD_BASES.union(&WORD_INDEXES).copied().collect();
        assert_eq!(llrm_x86_code16::Dos::default().address_registers(), restricted.len() as i64);
    }

    /// A string move reads cx cells from ds:si to es:di and leaves si, di
    /// and cx past them: the rep form pins all three and both segments (the
    /// source's as an fs override), the single one has no count.
    #[test]
    fn test_a_string_move_names_its_registers() {
        let held = |value: u32| Loc::Held(ir::Held { value, width: 2 });
        let repeated = semantics(
            Operation::Copy,
            "movsw",
            vec![Loc::Mem(ir::Mem::new(None, 0)), held(5), held(6), held(7)],
            vec![held(1), held(2), held(3), held(4), held(8)],
        );
        let wanted = requirements(&repeated);
        let at = |side: &str, index: usize| wanted.get(&Occurrence::new(side, index)).copied();
        assert_eq!((at("source", 0), at("source", 1), at("source", 2)), (Some(Register::ECX), Some(Register::ESI), Some(Register::EDI)));
        assert_eq!((at("source", 3), at("source", 4)), (Some(Register::FS), Some(Register::ES)));
        assert_eq!((at("dest", 1), at("dest", 2), at("dest", 3)), (Some(Register::ESI), Some(Register::EDI), Some(Register::ECX)));
        let single = semantics(
            Operation::Copy,
            "movsw",
            vec![Loc::Mem(ir::Mem::new(None, 0)), held(5), held(6)],
            vec![held(2), held(3), reg(Register::DS, 2), held(8)],
        );
        let wanted = requirements(&single);
        let at = |side: &str, index: usize| wanted.get(&Occurrence::new(side, index)).copied();
        assert_eq!((at("source", 0), at("source", 1), at("source", 2), at("source", 3)), (Some(Register::ESI), Some(Register::EDI), None, Some(Register::ES)));
        assert_eq!(at("dest", 3), None);
        assert!(reads(&repeated).contains_key(&Register::ECX) && writes(&repeated).contains_key(&Register::ECX));
    }

    fn reg(register: Register, width: u32) -> Loc {
        Loc::Reg(ir::Reg { register, width })
    }

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
        Semantics {
            name: Some(name.to_owned()),
            dests,
            sources,
            ..Semantics::new(op)
        }
    }

    /// `add ax,[c]` is ax at two moments, not two places.
    #[test]
    fn test_lir_says_a_two_address_operand_is_one_register() {
        let ax = reg(Register::AX, 2);
        let cell = Loc::Mem(ir::Mem::new(None, 2));

        let what = semantics(
            Operation::Binary,
            "add",
            vec![ax.clone()],
            vec![ax.clone(), cell.clone()],
        );
        assert_eq!(
            tied(&what),
            Some(Register::EAX),
            "the destination and the first source are one register"
        );

        let apart = semantics(
            Operation::Binary,
            "add",
            vec![ax.clone()],
            vec![reg(Register::BX, 2), cell],
        );
        assert_eq!(tied(&apart), None, "and a three-operand form ties nothing");

        let st = Loc::St(ir::St { index: 0 });
        let on_stack = semantics(Operation::FloatArith, "fadd", vec![st.clone()], vec![st]);
        assert_eq!(
            tied(&on_stack),
            None,
            "x87 shares no register with the rest"
        );
    }

    /// Legal and assignable are different questions: bp addresses a frame
    /// slot and is also the frame pointer.
    #[test]
    fn test_the_requirements_table_says_what_the_encoding_permits() {
        assert!(
            ADDRESSING.contains(&Register::BP),
            "a frame slot is reached through bp"
        );
        assert!(
            !ADDRESSING.contains(&Register::DX),
            "`[dx+0Ah]` has no encoding"
        );
        assert!(!BASES.is_empty(), "and the assignable set is not empty");
        assert!(BASES.iter().all(|one| AVAILABLE.contains(one)));
        assert!(
            !BASES.iter().any(|one| *one == Register::EBP),
            "bp is the frame pointer"
        );
    }

    /// al and ah share eax and share no byte.
    #[test]
    fn test_segment_registers_follow_the_machine() {
        assert_eq!(BUILT_IN.selectors, [Register::ES, Register::FS, Register::GS]);
        assert_eq!((BUILT_IN.data, BUILT_IN.through), (Register::DS, None));
        let joined = crate::abi::machine::Machine {
            segments: machine::BUILT_IN.segments.clone().map(|segments| crate::abi::machine::Segments { stack_is_data: true, ..segments }),
            ..machine::BUILT_IN.clone()
        };
        let joined = Segments::of(&joined);
        assert_eq!(joined.selectors, [Register::ES, Register::FS, Register::GS, Register::DS]);
        assert_eq!((joined.data, joined.through), (Register::DS, Some(Register::SS)));
    }

    #[test]
    fn test_a_register_names_which_bytes_of_its_root_it_is() {
        assert!(!overlaps(Register::AL, Register::AH));
        assert!(overlaps(Register::AL, Register::AX));
        assert!(overlaps(Register::AH, Register::EAX));
        assert!(!overlaps(Register::AL, Register::BL));
    }

    /// QCport's pl_game_reset selected LES before allocation, but the selector
    /// result was assigned BX; fresh OMF emission then refused the impossible
    /// `les ax:bx,[di+table]` form.
    ///
    /// The selector result is in the segment-register class rather than pinned
    /// to ES: allocation picks one and the rewriter spells les, lfs or lgs.
    #[test]
    fn test_far_load_confines_its_selector_result_to_a_segment_register() {
        use std::sync::Arc;

        use crate::model::lir::{Insn, LirBlock, LirBody};

        let what = semantics(
            Operation::Move,
            "les",
            vec![
                Loc::Held(ir::Held { value: 1, width: 2 }),
                Loc::Held(ir::Held { value: 2, width: 2 }),
            ],
            vec![Loc::Mem(ir::Mem {
                base: Some(ir::Held { value: 3, width: 2 }),
                ..ir::Mem::new(None, 4)
            })],
        );
        let load = Insn::new(0x10, Some((0x10, 0x13)), Some(what.clone()), vec![1, 2], vec![3]);
        let body = LirBody::new(
            "far",
            0x10,
            vec![LirBlock::new(0x10, vec![Arc::new(load)])],
            IndexMap::default(),
            IndexMap::default(),
        );

        assert!(!requirements(&what).contains_key(&Occurrence::new("dest", 1)));
        assert_eq!(
            crate::backend::regclass::classes(&body, &BTreeSet::new(), &BUILT_IN)[&2],
            BUILT_IN.selectors.iter().copied().collect::<BTreeSet<_>>()
        );
    }

    /// A string op's segment operand was a general value, so a selector made for
    /// it crowded the general registers: examples/logfile.nib grew a spill (+2 bytes,
    /// +2 memory operands). It is confined to the segment registers, as a far
    /// access's selector is.
    #[test]
    fn test_a_string_ops_segment_operand_is_confined_to_a_segment_register() {
        use std::sync::Arc;

        use crate::model::lir::{Insn, LirBlock, LirBody};

        let held = |value| Loc::Held(ir::Held { value, width: 2 });
        let copy = semantics(Operation::Copy, "movsd", vec![Loc::Mem(ir::Mem::new(None, 0)), held(5), held(6)], vec![held(1), held(2), Loc::Reg(ir::Reg { register: Register::DS, width: 2 }), held(3)]);
        let body = LirBody::new("string", 0x10, vec![LirBlock::new(0x10, vec![Arc::new(Insn::new(0x10, Some((0x10, 0x11)), Some(copy), vec![5, 6], vec![1, 2, 3]))])], IndexMap::default(), IndexMap::default());
        let classes = crate::backend::regclass::classes(&body, &BTreeSet::new(), &BUILT_IN);
        assert_eq!(classes.get(&3), Some(&BUILT_IN.selectors.iter().copied().collect::<BTreeSet<_>>()));
        assert!(!classes.contains_key(&1), "premise: the offsets stay general");
    }

    /// nbody's FLD pointer was allocated to AX, which cannot address 16-bit memory.
    #[test]
    fn test_x87_memory_operands_still_need_address_registers() {
        let what = semantics(
            Operation::FloatLoad,
            "fld",
            vec![Loc::St(ir::St { index: 0 })],
            vec![Loc::Mem(ir::Mem {
                through: Register::SI,
                ..ir::Mem::new(None, 4)
            })],
        );
        let want: BTreeSet<Register> = ADDRESSING.iter().map(|x| ir::root(*x)).collect();
        assert_eq!(reads(&what)[&Register::ESI].r#where, want);
    }

    /// The set-ordered tables, `NAMES` and `name_of`, as CPython builds them.
    #[test]
    #[allow(deprecated)]
    fn tables_match_python() {
        let values = |pairs: Vec<(Register, i64)>| {
            pairs
                .into_iter()
                .map(|(k, v)| (k as i64, v))
                .collect::<Vec<_>>()
        };
        let rows = [
            37, 38, 39, 40, 41, 42, 43, 44, 21, 22, 23, 24, 25, 26, 27, 28, 1, 2, 3, 4, 5, 6, 7, 8,
        ];
        let widths: Vec<i64> = WIDTHS.keys().map(|one| *one as i64).collect();
        assert_eq!(widths, rows);
        let lanes = values(LANES.iter().map(|(k, v)| (*k, *v)).collect());
        let masks = [
            15, 15, 15, 15, 15, 15, 15, 15, 3, 3, 3, 3, 3, 3, 3, 3, 1, 1, 1, 1, 2, 2, 2, 2,
        ];
        assert_eq!(lanes, rows.into_iter().zip(masks).collect::<Vec<_>>());
        let at_width: Vec<(i64, Vec<(i64, i64)>)> = AT_WIDTH
            .iter()
            .map(|(root, widths)| {
                (
                    *root as i64,
                    widths.iter().map(|(w, r)| (*w, *r as i64)).collect(),
                )
            })
            .collect();
        assert_eq!(
            at_width,
            [
                (37, vec![(4, 37), (2, 21), (1, 1)]),
                (38, vec![(4, 38), (2, 22), (1, 2)]),
                (39, vec![(4, 39), (2, 23), (1, 3)]),
                (40, vec![(4, 40), (2, 24), (1, 4)]),
                (41, vec![(4, 41), (2, 25)]),
                (42, vec![(4, 42), (2, 26)]),
                (43, vec![(4, 43), (2, 27)]),
                (44, vec![(4, 44), (2, 28)]),
            ]
        );
        assert_eq!(*BASES, [Register::EBX, Register::ESI, Register::EDI]);
        let python = "none al cl dl bl ah ch dh bh spl bpl sil dil r8l r9l r10l r11l r12l r13l r14l r15l ax cx dx bx sp bp si di r8w r9w r10w r11w r12w r13w r14w r15w eax ecx edx ebx esp ebp esi edi r8d r9d r10d r11d r12d r13d r14d r15d rax rcx rdx rbx rsp rbp rsi rdi r8 r9 r10 r11 r12 r13 r14 r15 eip rip es cs ss ds fs gs xmm0 xmm1 xmm2 xmm3 xmm4 xmm5 xmm6 xmm7 xmm8 xmm9 xmm10 xmm11 xmm12 xmm13 xmm14 xmm15 xmm16 xmm17 xmm18 xmm19 xmm20 xmm21 xmm22 xmm23 xmm24 xmm25 xmm26 xmm27 xmm28 xmm29 xmm30 xmm31 ymm0 ymm1 ymm2 ymm3 ymm4 ymm5 ymm6 ymm7 ymm8 ymm9 ymm10 ymm11 ymm12 ymm13 ymm14 ymm15 ymm16 ymm17 ymm18 ymm19 ymm20 ymm21 ymm22 ymm23 ymm24 ymm25 ymm26 ymm27 ymm28 ymm29 ymm30 ymm31 zmm0 zmm1 zmm2 zmm3 zmm4 zmm5 zmm6 zmm7 zmm8 zmm9 zmm10 zmm11 zmm12 zmm13 zmm14 zmm15 zmm16 zmm17 zmm18 zmm19 zmm20 zmm21 zmm22 zmm23 zmm24 zmm25 zmm26 zmm27 zmm28 zmm29 zmm30 zmm31 k0 k1 k2 k3 k4 k5 k6 k7 bnd0 bnd1 bnd2 bnd3 cr0 cr1 cr2 cr3 cr4 cr5 cr6 cr7 cr8 cr9 cr10 cr11 cr12 cr13 cr14 cr15 dr0 dr1 dr2 dr3 dr4 dr5 dr6 dr7 dr8 dr9 dr10 dr11 dr12 dr13 dr14 dr15 st0 st1 st2 st3 st4 st5 st6 st7 mm0 mm1 mm2 mm3 mm4 mm5 mm6 mm7 tr0 tr1 tr2 tr3 tr4 tr5 tr6 tr7 tmm0 tmm1 tmm2 tmm3 tmm4 tmm5 tmm6 tmm7";
        let rust: Vec<String> = Register::values().take(249).map(name_of).collect();
        assert_eq!(rust.join(" "), python);
        assert_eq!(name_of(Register::DontUse0), "249");
    }


    fn pins(what: &Semantics) -> Vec<(String, usize, Register)> {
        requirements(what).into_iter().map(|(place, register)| (place.side, place.index, register)).collect()
    }

    fn held(value: u32) -> Loc {
        Loc::Held(ir::Held { value, width: 2 })
    }

    /// `les`, `lds`, `lfs` and `lgs` take the same operands and differ in the selector
    /// register: which one the instruction is follows from the register, so none pins it.
    /// Reading them as `d1=es` made every far load need ES.
    #[test]
    fn test_a_register_that_picks_a_form_of_a_family_is_no_requirement() {
        for name in ["les", "lds", "lfs", "lgs"] {
            let what = semantics(Operation::Move, name, vec![held(1), held(2)], vec![Loc::Mem(ir::Mem::new(None, 4))]);
            assert!(requirements(&what).is_empty(), "{name}: {:?}", pins(&what));
        }
    }

    /// A divide reads the pair high half first, and writes the quotient then the remainder.
    #[test]
    fn test_a_divide_pins_the_pair_dx_before_ax() {
        let what = semantics(Operation::Divide, "idiv", vec![held(1), held(2)], vec![held(3), held(4), held(5)]);
        assert_eq!(
            pins(&what),
            [("dest".into(), 0, Register::EAX), ("dest".into(), 1, Register::EDX), ("source".into(), 0, Register::EDX), ("source".into(), 1, Register::EAX)]
        );
        let narrow = semantics(Operation::Divide, "div", vec![held(1)], vec![held(3), held(4)]);
        assert!(pins(&narrow).is_empty(), "a divide into one register names no pair");
    }

    /// `rep movs` reads its count in cx and the pointers in si and di, leaves them past the
    /// cells, and reads the source override in fs and the destination in es.
    #[test]
    fn test_a_rep_movs_pins_its_pointers_count_and_segments() {
        let what = semantics(Operation::Copy, "movsd", vec![Loc::Mem(ir::Mem::new(None, 0)), held(5), held(6), held(7)], vec![held(1), held(2), held(3), held(4), held(8)]);
        assert_eq!(
            pins(&what),
            [
                ("source".into(), 0, Register::ECX),
                ("source".into(), 1, Register::ESI),
                ("source".into(), 2, Register::EDI),
                ("source".into(), 3, Register::FS),
                ("source".into(), 4, Register::ES),
                ("dest".into(), 1, Register::ESI),
                ("dest".into(), 2, Register::EDI),
                ("dest".into(), 3, Register::ECX),
            ]
        );
    }

    /// A shift by anything but a literal counts from cl: a literal count pins nothing.
    #[test]
    fn test_a_shift_by_cl_pins_the_count() {
        let by = |count: Loc| semantics(Operation::Binary, "shl", vec![held(1)], vec![held(1), count]);
        assert_eq!(pins(&by(held(2))), [("source".into(), 1, Register::ECX)]);
        assert!(pins(&by(Loc::Imm(ir::Imm { value: 3, width: 1, address: None }))).is_empty());
    }

}
