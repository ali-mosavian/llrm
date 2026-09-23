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
use crate::support::hash::IndexMap;

use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::model::mir;
use crate::support::pyset::PySet;

// ---------------------------------------------------------------- registers

pub static ADDRESSING: LazyLock<BTreeSet<Register>> =
    LazyLock::new(|| BTreeSet::from([Register::BX, Register::BP, Register::SI, Register::DI]));
// `[bx+si]`: a word base and a word index are each confined to their half.
pub static WORD_BASES: LazyLock<BTreeSet<Register>> =
    LazyLock::new(|| BTreeSet::from([Register::BX]));
pub static WORD_INDEXES: LazyLock<BTreeSet<Register>> =
    LazyLock::new(|| BTreeSet::from([Register::SI, Register::DI]));

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
/// The one place those are written down; `reads` and `writes` read it too.
pub fn requirements(what: &Semantics) -> IndexMap<Occurrence, Register> {
    let mut out = IndexMap::default();
    if _on_the_stack(what) {
        return out;
    }
    // The widening forms name neither half: the product and the dividend
    // are both dx:ax, low first.
    if [Operation::Multiply, Operation::Divide].contains(&what.op) && what.dests.len() != 1 {
        out.insert(Occurrence::new("dest", 0), Register::EAX);
        out.insert(Occurrence::new("dest", 1), Register::EDX);
        // A multiply reads the accumulator. A divide reads the pair, high
        // first as `ir.DIVIDE_PAIR` has it.
        if what.op == Operation::Divide {
            out.insert(Occurrence::new("source", 0), Register::EDX);
            out.insert(Occurrence::new("source", 1), Register::EAX);
        } else {
            out.insert(Occurrence::new("source", 0), Register::EAX);
        }
    }
    // A repeated string fill reads value, count, address and segment, then
    // leaves di past the last cell and cx empty.  A single store has no count
    // source or result: value, address and segment are its three sources.
    if what.op == Operation::Fill && what.sources.len() == 4 {
        out.insert(Occurrence::new("source", 0), Register::EAX);
        out.insert(Occurrence::new("source", 1), Register::ECX);
        out.insert(Occurrence::new("source", 2), Register::EDI);
        if matches!(what.sources[3], Loc::Held(_)) {
            out.insert(Occurrence::new("source", 3), Register::ES);
        }
        if what.dests.len() == 3 {
            out.insert(Occurrence::new("dest", 1), Register::EDI);
            out.insert(Occurrence::new("dest", 2), Register::ECX);
        }
    } else if what.op == Operation::Fill && what.sources.len() == 3 {
        out.insert(Occurrence::new("source", 0), Register::EAX);
        out.insert(Occurrence::new("source", 1), Register::EDI);
        if matches!(what.sources[2], Loc::Held(_)) {
            out.insert(Occurrence::new("source", 2), Register::ES);
        }
        if what.dests.len() == 2 {
            out.insert(Occurrence::new("dest", 1), Register::EDI);
        }
    }
    if what.op == Operation::Extend && matches!(what.name.as_deref(), Some("cwd" | "cdq")) {
        out.insert(Occurrence::new("source", 0), Register::EAX);
        out.insert(Occurrence::new("dest", 0), Register::EDX);
    }
    // A shift or rotate by anything but a literal counts from cl, asked of
    // the operand's shape: an unplaced value names no register.
    let counted =
        _COUNTED.contains(&what.name.as_deref().unwrap_or("")) || what.op == Operation::Funnel;
    if counted && what.sources.len() > 1 {
        let count = &what.sources[what.sources.len() - 1];
        if !matches!(count, Loc::Imm(_)) {
            out.insert(
                Occurrence::new("source", what.sources.len() - 1),
                Register::ECX,
            );
        }
    }
    out
}

const _COUNTED: [&str; 8] = ["shl", "sal", "shr", "sar", "rol", "ror", "rcl", "rcr"];

fn _root(register: Register) -> Register {
    ir::root(register)
}

/// A shift whose count is a register takes it in cl and says so.
///
/// A funnel shift counts from cl and from nowhere else.
pub fn _shifted(what: &Semantics) -> bool {
    if what.op == Operation::Funnel {
        return what.sources.len() == 3 && matches!(what.sources[2], Loc::Reg(_));
    }
    ["shl", "shr", "sar", "rol", "ror", "rcl", "rcr"].contains(&what.name.as_deref().unwrap_or(""))
        && what
            .sources
            .iter()
            .any(|one| matches!(one, Loc::Reg(reg) if _root(reg.register) == Register::ECX))
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
pub const AVAILABLE: [Register; 6] = mir::TRACKED;

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
pub fn order(r#where: Option<&BTreeSet<Register>>) -> Vec<Register> {
    let Some(r#where) = r#where else {
        return AVAILABLE.to_vec();
    };
    let wanted: BTreeSet<Register> = r#where.iter().map(|one| ir::root(*one)).collect();
    if !wanted.is_empty() && wanted.is_subset(&SEGMENTS) {
        return SELECTORS
            .into_iter()
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

// The ones a selector value may be placed in. DS is DGROUP, SS the stack and
// CS the code; ES is BC's, and FS and GS are the 386's.
pub const SELECTORS: [Register; 3] = [Register::ES, Register::FS, Register::GS];
// One far load per selector: its selector result is in the class, not pinned,
// and the rewriter spells the instruction for the register it was given.
pub static FAR_LOADS: LazyLock<IndexMap<Register, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([(Register::ES, "les"), (Register::FS, "lfs"), (Register::GS, "lgs")])
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
            crate::backend::allocate::classes(&body, &BTreeSet::new())[&2],
            BTreeSet::from(SELECTORS)
        );
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
}
