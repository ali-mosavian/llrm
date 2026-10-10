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

use crate::abi::machine::Machine;
use crate::backend::classes::RegisterClasses;
use crate::backend::registerinfo;
use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::support::hash::IndexMap;
use crate::support::pyset::PySet;

// ---------------------------------------------------------------- registers

/// Where an operand has to live: one register, or any of a set.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Need {
    pub r#where: BTreeSet<Register>,
}

impl Need {
    /// The register, where there is only one it can be.
    pub fn fixed(&self) -> Option<Register> {
        if self.r#where.len() == 1 { self.r#where.iter().next().copied() } else { None }
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
    pub fn new(
        side: &str,
        index: usize,
    ) -> Self {
        Self { side: side.to_owned(), index }
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
    what.dests.iter().chain(&what.sources).any(positional_place) || what.name.as_deref().unwrap_or("").starts_with('f')
}

/// Whether `xchg` takes these two operands: a general register or a memory
/// cell each, not both memory, and no segment register.
pub fn exchangeable(
    one: &Loc,
    other: &Loc,
) -> bool {
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
        Loc::Reg(reg) if registerinfo::is_code_segment(reg.register) => None,
        other => pushed_width(other),
    }
}

/// Whether an x87 comparison reaches the flags through AX: `fnstsw ax; sahf`
/// follows it, so nothing may live in AX across it.
pub fn status_through_ax(what: &Semantics) -> bool {
    what.op == Operation::Compare
        && what.sources.iter().any(|one| positional_place(one) || matches!(one, Loc::Held(held) if held.width == 10))
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
pub fn reads(
    what: &Semantics,
    classes: &RegisterClasses,
) -> IndexMap<Register, Need> {
    let mut out = IndexMap::default();
    for (r#where, register) in classes.requirements(what) {
        if r#where.side == "source" {
            out.insert(register, Need { r#where: BTreeSet::from([register]) });
        }
    }
    for one in what.dests.iter().chain(&what.sources) {
        // `getattr(one, "through")`, `getattr(one, "index_through")` and
        // `getattr(getattr(one, "addr"), "base")`, each None where absent.
        let (through, index_through, base) = match one {
            Loc::Mem(mem) => (Some(mem.through), Some(mem.index_through), mem.addr.as_ref().map(|addr| addr.base)),
            Loc::Address(address) => (Some(address.through), None, address.addr.as_ref().map(|addr| addr.base)),
            _ => (None, None, None),
        };
        for r#where in [through, index_through, base].into_iter().flatten() {
            if r#where != Register::None {
                out.insert(_root(r#where), Need { r#where: classes.addressing.iter().map(|x| _root(*x)).collect() });
            }
        }
    }
    out
}

/// Registers this operation writes whether or not it names them.
pub fn writes(
    what: &Semantics,
    classes: &RegisterClasses,
) -> IndexMap<Register, Need> {
    let mut out = IndexMap::default();
    if _on_the_stack(what) {
        return out;
    }
    for (r#where, register) in classes.requirements(what) {
        if r#where.side == "dest" {
            out.insert(register, Need { r#where: BTreeSet::from([register]) });
        }
    }
    out
}

pub static WIDE: LazyLock<PySet<Register>> = LazyLock::new(|| integer_of(4).into_iter().collect());
pub static NARROW: LazyLock<PySet<Register>> = LazyLock::new(|| integer_of(2).into_iter().collect());
// The byte halves. BC reaches for them to clear a high byte and to read one
// byte of an array.
pub static BYTE: LazyLock<PySet<Register>> = LazyLock::new(|| integer_of(1).into_iter().collect());

/// The integer registers `bytes` wide, by iced's number.
fn integer_of(bytes: i64) -> Vec<Register> {
    let mut found: Vec<Register> = registerinfo::entries()
        .filter(|(_, one)| one.classes & registerinfo::class::INT != 0 && i64::from(one.bits / 8) == bytes)
        .map(|(register, _)| register)
        .collect();
    found.sort_by_key(|one| *one as usize);
    found.dedup();
    found
}

/// Every integer register (the 8, 16 and 32-bit views), wide ones first, each
/// width by iced's number: the order the allocator's tables have always been
/// walked in.
pub fn integer_registers() -> impl Iterator<Item = Register> {
    static ALL: LazyLock<Vec<Register>> = LazyLock::new(|| [4, 2, 1].into_iter().flat_map(integer_of).collect());
    ALL.iter().copied()
}

/// Whether `register` is an integer register: one the tables name by width.
pub fn integer(register: Register) -> bool {
    registerinfo::in_class(register, registerinfo::class::INT)
}

/// The same register named at the width an operand needs.
pub fn named(
    register: Register,
    width: i64,
) -> Register {
    registerinfo::view(registerinfo::root(register), width as u32 * 8).unwrap_or(register)
}

/// The registers an operand may take, in the order to try them.
///
/// LLVM's `AllocationOrder`. `None` means the operand said nothing.
pub fn order(
    r#where: Option<&BTreeSet<Register>>,
    segments: &Segments,
    classes: &RegisterClasses,
) -> Vec<Register> {
    let Some(r#where) = r#where else {
        return classes.available.clone();
    };
    let wanted: BTreeSet<Register> = r#where.iter().map(|one| ir::root(*one)).collect();
    if !wanted.is_empty() && wanted.is_subset(&SEGMENTS) {
        return segments.selectors.iter().copied().filter(|one| wanted.contains(one)).collect();
    }
    classes.available.iter().copied().filter(|one| wanted.contains(one)).collect()
}

// The segment registers. Operands, not allocatable.
pub static SEGMENTS: LazyLock<BTreeSet<Register>> = LazyLock::new(|| {
    registerinfo::entries()
        .filter(|(_, one)| one.classes & registerinfo::class::SEGMENT != 0)
        .map(|(id, _)| id)
        .collect()
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
    /// The register an access without a prefix reads.
    fn data_register() -> Register {
        registerinfo::data_segment().expect("the target's register file names no data segment")
    }

    /// Every segment register, the order a selector is placed in: the far
    /// pointer's, then the ones with no meaning of their own, then the data,
    /// stack and code ones.
    fn preference() -> Vec<Register> {
        let meant = [registerinfo::data_segment(), registerinfo::stack_segment(), registerinfo::code_segment()];
        let far = registerinfo::far_segment();
        far.into_iter()
            .chain(SEGMENTS.iter().copied().filter(|one| Some(*one) != far && !meant.contains(&Some(*one))))
            .chain(meant.into_iter().flatten())
            .collect()
    }

    pub fn of(machine: &Machine) -> Self {
        // A flat machine has no selector to place: DS is only what string
        // operations read.
        let Some(segments) = machine.segments.as_ref() else {
            return Self { selectors: Vec::new(), data: Self::data_register(), through: None, huge_shift: None };
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
            selectors: Self::preference()
                .into_iter()
                .filter(|one| !reserved.iter().any(|name| named(one, name)))
                .collect(),
            data: register(&segments.data),
            through: segments.stack_is_data.then(|| register(&segments.stack)),
            huge_shift: machine.huge_shift(),
        }
    }
}

/// The built-in machine's, for the tests of this crate.
#[cfg(test)]
pub static BUILT_IN: LazyLock<Segments> = LazyLock::new(|| Segments::of(&llrm_x86_m16::machine::BUILT_IN));

/// The segment registers of `machine::BASIC`, for the tests of this crate.
#[cfg(test)]
pub static BASIC: LazyLock<Segments> = LazyLock::new(|| Segments::of(&llrm_x86_m16::machine::BASIC));

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
    named_for_data_group(what.name.as_deref().unwrap_or(""))
}

/// Whether an instruction of this mnemonic is a string instruction, an x87 one,
/// a trap or a wait. Prefixes are taken off by hand: `str::trim_start_matches`
/// with a `&str` pattern builds a substring searcher for each call, which was
/// 2.4% of a large module's compile, asked of every instruction at every
/// rebuild of the allocator's facts.
fn named_for_data_group(name: &str) -> bool {
    fn without<'a>(
        mut text: &'a str,
        prefix: &str,
    ) -> &'a str {
        while let Some(rest) = text.strip_prefix(prefix) {
            text = rest;
        }
        text
    }
    let bare = without(without(without(name, "rep "), "repe "), "repne ");
    ["movs", "lods", "cmps", "outs", "stos", "scas", "ins", "xlat", "int", "wait", "fwait"]
        .iter()
        .any(|one| bare.starts_with(one))
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

/// Whether the register is a position in a stack, where an exchange is an
/// effect and no pass may rename or drop it: the description's class.
pub fn positional(register: Register) -> bool {
    crate::backend::registerinfo::in_class(register, crate::backend::registerinfo::class::POSITIONAL)
}

/// Whether the operand is a positional register.
pub fn positional_place(place: &Loc) -> bool {
    matches!(place, Loc::Reg(one) if positional(one.register))
}

/// Whether this is a register this target describes at all.
pub fn known(register: Register) -> bool {
    integer(register) || SEGMENTS.contains(&register)
}

/// How wide this register is, or None where the target does not say.
pub fn width_of(register: Register) -> Option<i64> {
    if SEGMENTS.contains(&register) { Some(2) } else { registerinfo::bytes(register).filter(|_| integer(register)) }
}

// ------------------------------------------------------------ subregisters

/// Which bytes of its root this register names.
pub fn lanes(register: Register) -> i64 {
    registerinfo::lanes(register)
}

/// Whether writing one can be seen by reading the other.
///
/// The same root is not enough: al and ah share eax and share no byte.
pub fn overlaps(
    one: Register,
    other: Register,
) -> bool {
    if ir::root(one) != ir::root(other) {
        return false;
    }
    lanes(one) & lanes(other) != 0
}

/// This register's own name, lowercase.
pub fn name_of(register: Register) -> String {
    if let Some(name) = registerinfo::name(register) {
        return name.to_owned();
    }
    // Python's `iced_x86.Register` defines no `DontUse*` member, so those have
    // no name here.
    let spelled = format!("{register:?}");
    if spelled.starts_with("DontUse") { (register as i64).to_string() } else { spelled.to_lowercase() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mnemonic test as it was written with `trim_start_matches`.
    fn named_as_it_was(name: &str) -> bool {
        let bare = name.trim_start_matches("rep ").trim_start_matches("repe ").trim_start_matches("repne ");
        ["movs", "lods", "cmps", "outs", "stos", "scas", "ins", "xlat", "int", "wait", "fwait"]
            .iter()
            .any(|one| bare.starts_with(one))
            || name.starts_with('f')
    }

    /// Taking the prefixes off by hand must leave every mnemonic where it was:
    /// repeated prefixes, one prefix after another, and a prefix that is
    /// not one.
    #[test]
    fn test_a_mnemonic_is_told_as_a_string_or_x87_instruction_as_it_always_was() {
        let bases = [
            "mov",
            "movsb",
            "movsw",
            "lodsb",
            "cmpsw",
            "outsb",
            "stosw",
            "scasb",
            "insb",
            "xlatb",
            "int",
            "int3",
            "into",
            "wait",
            "fwait",
            "fld",
            "fstp",
            "f2xm1",
            "add",
            "ret",
            "call",
            "rep",
            "rep movsb",
            "",
            "i",
            "in",
        ];
        let prefixes = [
            "",
            "rep ",
            "repe ",
            "repne ",
            "rep rep ",
            "repe rep ",
            "rep repe ",
            "repne repe ",
            "rep repne ",
            "repeat ",
            "repn ",
            "REP ",
        ];
        for prefix in prefixes {
            for base in bases {
                let name = format!("{prefix}{base}");
                assert_eq!(named_for_data_group(&name), named_as_it_was(&name), "{name:?}");
            }
        }
    }

    /// What the allocator may hand out for an operand that reaches memory: the
    /// addressing registers it holds values in.
    fn bases(classes: &RegisterClasses) -> Vec<Register> {
        let addressing: BTreeSet<Register> = classes.addressing.iter().map(|x| ir::root(*x)).collect();
        classes.available.iter().copied().filter(|one| addressing.contains(one)).collect()
    }

    /// `Segments::of` panicked ("a segmented machine") on a flat machine, so no
    /// flat target reached the allocator; a flat machine places no
    /// selector.
    #[test]
    fn test_a_flat_machine_has_no_selector_to_place() {
        let flat =
            Machine::parse("addressing = \"flat\"\nsegment_end_faults = false\nfar_bss = false\n", "486").unwrap();
        let segments = Segments::of(&flat);
        assert!(segments.selectors.is_empty() && segments.through.is_none() && segments.huge_shift.is_none());
    }

    /// The allocator's bases are the encodable ones less the frame register: a
    /// new frame rule changes one place.
    #[test]
    fn test_the_allocators_bases_are_the_encodable_ones_but_the_frame() {
        let encodable: BTreeSet<Register> = llrm_x86_m16::ENCODABLE_BASES.into_iter().collect();
        let held: BTreeSet<Register> = encodable.iter().copied().filter(|&one| one != llrm_x86_m16::FRAME).collect();
        assert_eq!(RegisterClasses::m16().word_bases, held);
        assert!(crate::backend::select::_WORD_BASES.iter().all(|one| encodable.contains(one)));
    }

    /// x86 arithmetic is two-address: the spill model charges the copy of a
    /// first operand that stays live.
    #[test]
    fn test_the_machine_says_its_arithmetic_is_two_address() {
        use llrm_mir::target::Machine;
        assert!(llrm_x86_m16::Dos::default().two_address());
    }

    /// The spill model counts the registers an address may use as the allocator
    /// restricts to.
    #[test]
    fn test_the_spill_models_address_registers_are_the_allocators() {
        use llrm_mir::target::Machine;
        let restricted: BTreeSet<Register> = {
            let classes = RegisterClasses::m16();
            classes.word_bases.union(&classes.word_indexes).copied().collect()
        };
        assert_eq!(llrm_x86_m16::Dos::default().address_registers(), restricted.len() as i64);
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
        let wanted = RegisterClasses::m16().requirements(&repeated);
        let at = |side: &str, index: usize| wanted.get(&Occurrence::new(side, index)).copied();
        assert_eq!(
            (at("source", 0), at("source", 1), at("source", 2)),
            (Some(Register::ECX), Some(Register::ESI), Some(Register::EDI))
        );
        assert_eq!((at("source", 3), at("source", 4)), (Some(Register::FS), Some(Register::ES)));
        assert_eq!(
            (at("dest", 1), at("dest", 2), at("dest", 3)),
            (Some(Register::ESI), Some(Register::EDI), Some(Register::ECX))
        );
        let single = semantics(
            Operation::Copy,
            "movsw",
            vec![Loc::Mem(ir::Mem::new(None, 0)), held(5), held(6)],
            vec![held(2), held(3), reg(Register::DS, 2), held(8)],
        );
        let wanted = RegisterClasses::m16().requirements(&single);
        let at = |side: &str, index: usize| wanted.get(&Occurrence::new(side, index)).copied();
        assert_eq!(
            (at("source", 0), at("source", 1), at("source", 2), at("source", 3)),
            (Some(Register::ESI), Some(Register::EDI), None, Some(Register::ES))
        );
        assert_eq!(at("dest", 3), None);
        assert!(
            reads(&repeated, &crate::backend::classes::RegisterClasses::m16()).contains_key(&Register::ECX)
                && writes(&repeated, &crate::backend::classes::RegisterClasses::m16()).contains_key(&Register::ECX)
        );
    }

    fn reg(
        register: Register,
        width: u32,
    ) -> Loc {
        Loc::Reg(ir::Reg { register, width })
    }

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    /// `add ax,[c]` is ax at two moments, not two places.
    #[test]
    fn test_lir_says_a_two_address_operand_is_one_register() {
        let ax = reg(Register::AX, 2);
        let cell = Loc::Mem(ir::Mem::new(None, 2));

        let what = semantics(Operation::Binary, "add", vec![ax.clone()], vec![ax.clone(), cell.clone()]);
        assert_eq!(tied(&what), Some(Register::EAX), "the destination and the first source are one register");

        let apart = semantics(Operation::Binary, "add", vec![ax.clone()], vec![reg(Register::BX, 2), cell]);
        assert_eq!(tied(&apart), None, "and a three-operand form ties nothing");

        let st = Loc::st(0);
        let on_stack = semantics(Operation::FloatArith, "fadd", vec![st.clone()], vec![st]);
        assert_eq!(tied(&on_stack), None, "x87 shares no register with the rest");
    }

    /// Legal and assignable are different questions: bp addresses a frame
    /// slot and is also the frame pointer.
    #[test]
    fn test_the_requirements_table_says_what_the_encoding_permits() {
        assert!(RegisterClasses::m16().addressing.contains(&Register::BP), "a frame slot is reached through bp");
        assert!(!RegisterClasses::m16().addressing.contains(&Register::DX), "`[dx+0Ah]` has no encoding");
        let classes = RegisterClasses::m16();
        assert!(!bases(&classes).is_empty(), "and the assignable set is not empty");
        assert!(bases(&classes).iter().all(|one| classes.available.contains(one)));
        assert!(!bases(&classes).iter().any(|one| *one == Register::EBP), "bp is the frame pointer");
    }

    /// al and ah share eax and share no byte.
    #[test]
    fn test_segment_registers_follow_the_machine() {
        assert_eq!(BUILT_IN.selectors, [Register::ES, Register::FS, Register::GS]);
        assert_eq!((BUILT_IN.data, BUILT_IN.through), (Register::DS, None));
        let joined = crate::abi::machine::Machine {
            segments: llrm_x86_m16::machine::BUILT_IN
                .segments
                .clone()
                .map(|segments| crate::abi::machine::Segments { stack_is_data: true, ..segments }),
            ..llrm_x86_m16::machine::BUILT_IN.clone()
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
            vec![Loc::Held(ir::Held { value: 1, width: 2 }), Loc::Held(ir::Held { value: 2, width: 2 })],
            vec![Loc::Mem(ir::Mem { base: Some(ir::Held { value: 3, width: 2 }), ..ir::Mem::new(None, 4) })],
        );
        let load = Insn::new(0x10, Some((0x10, 0x13)), Some(what.clone()), vec![1, 2], vec![3]);
        let body = LirBody::new(
            "far",
            0x10,
            vec![LirBlock::new(0x10, vec![Arc::new(load)])],
            IndexMap::default(),
            IndexMap::default(),
        );

        assert!(!RegisterClasses::m16().requirements(&what).contains_key(&Occurrence::new("dest", 1)));
        assert_eq!(
            crate::backend::regclass::classes(
                &body,
                &BTreeSet::new(),
                &BUILT_IN,
                &crate::backend::classes::RegisterClasses::m16()
            )[&2],
            BUILT_IN.selectors.iter().copied().collect::<BTreeSet<_>>()
        );
    }

    /// A string op's segment operand was a general value, so a selector made
    /// for it crowded the general registers: examples/logfile.nib grew a
    /// spill (+2 bytes, +2 memory operands). It is confined to the segment
    /// registers, as a far access's selector is.
    #[test]
    fn test_a_string_ops_segment_operand_is_confined_to_a_segment_register() {
        use std::sync::Arc;

        use crate::model::lir::{Insn, LirBlock, LirBody};

        let held = |value| Loc::Held(ir::Held { value, width: 2 });
        let copy = semantics(
            Operation::Copy,
            "movsd",
            vec![Loc::Mem(ir::Mem::new(None, 0)), held(5), held(6)],
            vec![held(1), held(2), Loc::Reg(ir::Reg { register: Register::DS, width: 2 }), held(3)],
        );
        let body = LirBody::new(
            "string",
            0x10,
            vec![LirBlock::new(
                0x10,
                vec![Arc::new(Insn::new(0x10, Some((0x10, 0x11)), Some(copy), vec![5, 6], vec![1, 2, 3]))],
            )],
            IndexMap::default(),
            IndexMap::default(),
        );
        let classes = crate::backend::regclass::classes(
            &body,
            &BTreeSet::new(),
            &BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        );
        assert_eq!(classes.get(&3), Some(&BUILT_IN.selectors.iter().copied().collect::<BTreeSet<_>>()));
        assert!(!classes.contains_key(&1), "premise: the offsets stay general");
    }

    /// nbody's FLD pointer was allocated to AX, which cannot address 16-bit
    /// memory.
    #[test]
    fn test_x87_memory_operands_still_need_address_registers() {
        let what = semantics(
            Operation::FloatLoad,
            "fld",
            vec![Loc::st(0)],
            vec![Loc::Mem(ir::Mem { through: Register::SI, ..ir::Mem::new(None, 4) })],
        );
        let want: BTreeSet<Register> = RegisterClasses::m16().addressing.iter().map(|x| ir::root(*x)).collect();
        assert_eq!(reads(&what, &crate::backend::classes::RegisterClasses::m16())[&Register::ESI].r#where, want);
    }

    /// The set-ordered tables, `NAMES` and `name_of`, as CPython builds them.
    #[test]
    #[allow(deprecated)]
    fn tables_match_python() {
        let rows = [37, 38, 39, 40, 41, 42, 43, 44, 21, 22, 23, 24, 25, 26, 27, 28, 1, 2, 3, 4, 5, 6, 7, 8];
        let walked: Vec<i64> = integer_registers().map(|one| one as i64).collect();
        assert_eq!(walked, rows);
        let masks = [15, 15, 15, 15, 15, 15, 15, 15, 3, 3, 3, 3, 3, 3, 3, 3, 1, 1, 1, 1, 2, 2, 2, 2];
        let lanes_of: Vec<(i64, i64)> = integer_registers().map(|one| (one as i64, lanes(one))).collect();
        assert_eq!(lanes_of, rows.into_iter().zip(masks).collect::<Vec<_>>());
        let by_root: Vec<(i64, Vec<(i64, i64)>)> = rows[..8]
            .iter()
            .map(|root| {
                let root = Register::values().find(|one| *one as i64 == *root).unwrap();
                let views = [4_i64, 2, 1]
                    .into_iter()
                    .filter_map(|width| registerinfo::view(root, width as u32 * 8).map(|one| (width, one as i64)))
                    .collect();
                (root as i64, views)
            })
            .collect();
        assert_eq!(
            by_root,
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
        assert_eq!(bases(&RegisterClasses::m16()), [Register::EBX, Register::ESI, Register::EDI]);
        let python = "none al cl dl bl ah ch dh bh spl bpl sil dil r8l r9l r10l r11l r12l r13l r14l r15l ax cx dx bx sp bp si di r8w r9w r10w r11w r12w r13w r14w r15w eax ecx edx ebx esp ebp esi edi r8d r9d r10d r11d r12d r13d r14d r15d rax rcx rdx rbx rsp rbp rsi rdi r8 r9 r10 r11 r12 r13 r14 r15 eip rip es cs ss ds fs gs xmm0 xmm1 xmm2 xmm3 xmm4 xmm5 xmm6 xmm7 xmm8 xmm9 xmm10 xmm11 xmm12 xmm13 xmm14 xmm15 xmm16 xmm17 xmm18 xmm19 xmm20 xmm21 xmm22 xmm23 xmm24 xmm25 xmm26 xmm27 xmm28 xmm29 xmm30 xmm31 ymm0 ymm1 ymm2 ymm3 ymm4 ymm5 ymm6 ymm7 ymm8 ymm9 ymm10 ymm11 ymm12 ymm13 ymm14 ymm15 ymm16 ymm17 ymm18 ymm19 ymm20 ymm21 ymm22 ymm23 ymm24 ymm25 ymm26 ymm27 ymm28 ymm29 ymm30 ymm31 zmm0 zmm1 zmm2 zmm3 zmm4 zmm5 zmm6 zmm7 zmm8 zmm9 zmm10 zmm11 zmm12 zmm13 zmm14 zmm15 zmm16 zmm17 zmm18 zmm19 zmm20 zmm21 zmm22 zmm23 zmm24 zmm25 zmm26 zmm27 zmm28 zmm29 zmm30 zmm31 k0 k1 k2 k3 k4 k5 k6 k7 bnd0 bnd1 bnd2 bnd3 cr0 cr1 cr2 cr3 cr4 cr5 cr6 cr7 cr8 cr9 cr10 cr11 cr12 cr13 cr14 cr15 dr0 dr1 dr2 dr3 dr4 dr5 dr6 dr7 dr8 dr9 dr10 dr11 dr12 dr13 dr14 dr15 st0 st1 st2 st3 st4 st5 st6 st7 mm0 mm1 mm2 mm3 mm4 mm5 mm6 mm7 tr0 tr1 tr2 tr3 tr4 tr5 tr6 tr7 tmm0 tmm1 tmm2 tmm3 tmm4 tmm5 tmm6 tmm7";
        let rust: Vec<String> = Register::values().take(249).map(name_of).collect();
        assert_eq!(rust.join(" "), python);
        assert_eq!(name_of(Register::DontUse0), "249");
    }

    fn pins(what: &Semantics) -> Vec<(String, usize, Register)> {
        RegisterClasses::m16()
            .requirements(what)
            .into_iter()
            .map(|(place, register)| (place.side, place.index, register))
            .collect()
    }

    fn held(value: u32) -> Loc {
        Loc::Held(ir::Held { value, width: 2 })
    }

    /// `les`, `lds`, `lfs` and `lgs` take the same operands and differ in the
    /// selector register: which one the instruction is follows from the
    /// register, so none pins it. Reading them as `d1=es` made every far
    /// load need ES.
    #[test]
    fn test_a_register_that_picks_a_form_of_a_family_is_no_requirement() {
        for name in ["les", "lds", "lfs", "lgs"] {
            let what = semantics(Operation::Move, name, vec![held(1), held(2)], vec![Loc::Mem(ir::Mem::new(None, 4))]);
            assert!(RegisterClasses::m16().requirements(&what).is_empty(), "{name}: {:?}", pins(&what));
        }
    }

    /// A divide reads the pair high half first, and writes the quotient then
    /// the remainder.
    #[test]
    fn test_a_divide_pins_the_pair_dx_before_ax() {
        let what = semantics(Operation::Divide, "idiv", vec![held(1), held(2)], vec![held(3), held(4), held(5)]);
        assert_eq!(
            pins(&what),
            [
                ("dest".into(), 0, Register::EAX),
                ("dest".into(), 1, Register::EDX),
                ("source".into(), 0, Register::EDX),
                ("source".into(), 1, Register::EAX)
            ]
        );
        let narrow = semantics(Operation::Divide, "div", vec![held(1)], vec![held(3), held(4)]);
        assert!(pins(&narrow).is_empty(), "a divide into one register names no pair");
    }

    /// `rep movs` reads its count in cx and the pointers in si and di, leaves
    /// them past the cells, and reads the source override in fs and the
    /// destination in es.
    #[test]
    fn test_a_rep_movs_pins_its_pointers_count_and_segments() {
        let what = semantics(
            Operation::Copy,
            "movsd",
            vec![Loc::Mem(ir::Mem::new(None, 0)), held(5), held(6), held(7)],
            vec![held(1), held(2), held(3), held(4), held(8)],
        );
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

    /// A shift by anything but a literal counts from cl: a literal count pins
    /// nothing.
    #[test]
    fn test_a_shift_by_cl_pins_the_count() {
        let by = |count: Loc| semantics(Operation::Binary, "shl", vec![held(1)], vec![held(1), count]);
        assert_eq!(pins(&by(held(2))), [("source".into(), 1, Register::ECX)]);
        assert!(pins(&by(Loc::Imm(ir::Imm { value: 3, width: 1, address: None }))).is_empty());
    }

    // ----------------------------------------------- what a funnel shift and a
    // sign extension pin

    fn funnel_of(
        count: Loc,
        name: &str,
    ) -> Semantics {
        let low = Loc::Reg(ir::Reg { register: Register::EAX, width: 4 });
        Semantics {
            name: Some(name.to_owned()),
            dests: vec![low.clone()],
            sources: vec![low, Loc::Reg(ir::Reg { register: Register::EDX, width: 4 }), count],
            ..Semantics::new(Operation::Funnel)
        }
    }

    fn imm(
        value: i64,
        width: u32,
    ) -> Loc {
        Loc::Imm(ir::Imm { value, width, address: None })
    }

    fn rg(
        register: Register,
        width: u32,
    ) -> Loc {
        Loc::Reg(ir::Reg { register, width })
    }

    #[test]
    fn test_a_funnel_shift_is_two_address_in_its_low_half() {
        assert_eq!(tied(&funnel_of(imm(16, 1), "shrd")), Some(Register::EAX));
    }

    #[test]
    fn test_a_funnel_shift_by_a_register_takes_its_count_in_cl() {
        let dynamic = reads(&funnel_of(rg(Register::CL, 1), "shrd"), &RegisterClasses::m16());
        assert!(dynamic.get(&Register::ECX).is_some_and(|need| need.fixed() == Some(Register::ECX)));
        assert!(!reads(&funnel_of(imm(16, 1), "shrd"), &RegisterClasses::m16()).contains_key(&Register::EAX));
        assert!(
            writes(&funnel_of(imm(16, 1), "shrd"), &RegisterClasses::m16()).is_empty(),
            "shrd writes only what it names"
        );
    }

    /// A sign extension between explicit operands pins nothing.
    #[test]
    fn test_a_signed_word_extension_pins_no_register() {
        let what = Semantics {
            name: Some("movsx".to_owned()),
            dests: vec![rg(Register::EBX, 4)],
            sources: vec![rg(Register::SI, 2)],
            ..Semantics::new(Operation::Extend)
        };
        assert!(RegisterClasses::m16().requirements(&what).is_empty());
    }
}

#[cfg(test)]
mod positional_tests {
    use super::*;

    /// The set comes from the descriptions' `positional` class (build.rs): the
    /// eight stack registers, no general register and no segment register.
    #[test]
    fn test_positional_is_the_stack_registers_the_descriptions_name() {
        let stack: Vec<Register> = Register::values().filter(|one| positional(*one)).collect();
        assert_eq!(stack.len(), 8);
        assert!(stack.iter().all(|one| one.is_st()));
        assert!(![Register::EAX, Register::AL, Register::DS, Register::None].iter().any(|one| positional(*one)));
    }
}
