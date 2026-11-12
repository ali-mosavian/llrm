//! The Z80's register file as the allocator sees it.
//!
//! The classes are `GR8` (the bytes `b c d e h l a`, `a` last: the 8-bit ALU
//! works on it), `ACC` (`a`), `GR16` (the pairs `bc de hl`), `HLR` (`hl`) and
//! `IY`. `ix` is the frame and `sp` the stack. A call keeps nothing but
//! `ix` and `sp` (`calling.toml`), so every allocatable register is
//! caller-saved.

use llrm_codegen::model::ir::Loc;
use llrm_codegen::model::lir::Insn;
use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Target;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::expand::lanes;
use crate::{REGISTER_INFO, reg};

/// A virtual register's class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    Gr8,
    Acc,
    Gr16,
    Hlr,
    /// `de` and `bc` alone: the fixed operands of `ldir`.
    Der,
    Bcr,
    Iy,
    /// Unconstrained: any allocatable register.
    Any,
}

pub type MFunction = llrm_spike_mf::MFunction<Class>;

const BYTES: [RegId; 7] = [reg::B, reg::C, reg::D, reg::E, reg::H, reg::L, reg::A];
const PAIRS: [RegId; 3] = [reg::BC, reg::DE, reg::HL];

impl Class {
    fn is_byte(self) -> bool {
        matches!(self, Class::Gr8 | Class::Acc)
    }
}

/// The Z80 behind the allocator.
pub struct Z80Registers;

impl Z80Registers {
    /// The bytes a value of `class` takes.
    pub fn bytes_of(class: Class) -> usize {
        if class.is_byte() { 1 } else { 2 }
    }
}

thread_local! {
    /// Set while a function is allocated a second time, after the first
    /// allocation failed or to price the other choice: whether the copies from
    /// the argument registers are left unjoined, instead of what the
    /// convention says (`Target::joins_pinned_copies`).
    pub static UNJOINED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    /// Set while a function is allocated to price the other choice: no spilled
    /// read is taken from its cell (a `push` and a `pop` of the slot can be
    /// smaller than `ld d(ix), r` and `op a, d(ix)`).
    pub static NO_FOLD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl Target for Z80Registers {
    /// The pin joins a copy with a physical register only when it is reserved
    /// (`canJoinPhys`); an argument pair is not, so the copy stays and the
    /// word can spill instead of holding its pair to its last use. The
    /// convention states whether the z80 leaves them
    /// (`pinned_copies_unjoined`).
    fn joins_pinned_copies() -> bool {
        !UNJOINED.with(std::cell::Cell::get).unwrap_or(crate::abi::z80().pinned_copies_unjoined)
    }

    type Class = Class;

    fn meet(
        one: Class,
        other: Class,
    ) -> Option<Class> {
        match (one, other) {
            (one, two) if one == two => Some(one),
            (Class::Any, one) | (one, Class::Any) => Some(one),
            (Class::Gr8, Class::Acc) | (Class::Acc, Class::Gr8) => Some(Class::Acc),
            (Class::Gr16, Class::Hlr) | (Class::Hlr, Class::Gr16) => Some(Class::Hlr),
            (Class::Gr16, Class::Der) | (Class::Der, Class::Gr16) => Some(Class::Der),
            (Class::Gr16, Class::Bcr) | (Class::Bcr, Class::Gr16) => Some(Class::Bcr),
            _ => None,
        }
    }

    fn any() -> Class {
        Class::Any
    }

    fn candidates(class: Class) -> Vec<RegId> {
        match class {
            Class::Gr8 => BYTES.to_vec(),
            Class::Acc => vec![reg::A],
            Class::Gr16 => PAIRS.to_vec(),
            Class::Hlr => vec![reg::HL],
            Class::Der => vec![reg::DE],
            Class::Bcr => vec![reg::BC],
            Class::Iy => vec![reg::IY],
            Class::Any => BYTES.iter().chain(&PAIRS).chain(&[reg::IY]).copied().collect(),
        }
    }

    /// The register of `class`'s width in `root`: a byte class takes the low
    /// byte of a pair.
    fn view(
        root: RegId,
        class: Class,
    ) -> RegId {
        if class.is_byte() && REGISTER_INFO.bytes(root) == Some(2) {
            return REGISTER_INFO.view(root, 8).unwrap_or(root);
        }
        root
    }

    fn root(register: RegId) -> RegId {
        REGISTER_INFO.root(register)
    }

    fn operand_class(
        name: &str,
        index: usize,
    ) -> Option<Class> {
        crate::mfunc::operand_class(name, index)
    }

    fn lane_moves(inst: &Inst) -> Option<(usize, Vec<(usize, u32, u32)>)> {
        matches!(inst.kind, Kind::Op("PACK")).then(|| (0, vec![(1, 0, 1), (2, 1, 1)]))
    }

    // DEVIATE 3b-1 (owner greedy-subreg, exit 2026-11-12): both off until the
    // split editor copies by lane. Lowering alone: +505 lines at -O2; with
    // joins `mul_constants` -O2 summed a stale byte.
    fn lowers_lanes() -> bool {
        false
    }

    fn read_lanes(inst: &Inst) -> Option<(usize, u32, u32)> {
        match inst.kind {
            Kind::Op("BIT7_16" | "HI") => Some((1, 1, 1)),
            Kind::Op("LO") => Some((1, 0, 1)),
            _ => None,
        }
    }

    fn joins_lanes() -> bool {
        false
    }

    fn read_part(
        inst: &Inst,
        operand: usize,
        register: RegId,
    ) -> RegId {
        // the sign test of a pair, and the byte halves taken from one, read a
        // single byte
        let half = match inst.kind {
            Kind::Op("BIT7_16" | "HI") if operand == 1 => 1,
            Kind::Op("LO") if operand == 1 => 0,
            _ => return register,
        };
        lanes(register).map_or(register, |lanes| [lanes.0, lanes.1][half])
    }

    /// A literal in a byte register costs `ld r, n` (7 T, 2 B) against the
    /// copy `ld r, r'` (4 T, 1 B), a word `ld rr, nn` (10 T, 3 B) against two
    /// copies (8 T, 2 B): making it again loses to the copy, except a zero in
    /// `a`, which is `xor a` (4 T, 1 B). Unpriced, `fletcher` -O2 made its zero
    /// in the loop every trip.
    fn literal_move_wins(
        literal: &Insn,
        _source: &[RegId],
        destination: &[RegId],
        size: bool,
    ) -> bool {
        let Some(what) = literal.what.as_ref() else { return false };
        let ([Loc::Held(into)], [Loc::Imm(imm)]) = (what.dests.as_slice(), what.sources.as_slice()) else {
            return false;
        };
        if into.width == 1 && imm.value == 0 && destination == [reg::A] {
            return true;
        }
        let cost = |form: &str| crate::timing::cost(form).map_or((u32::MAX, u32::MAX), |c| (c.bytes, c.taken));
        let (copy, load) = if into.width == 1 {
            (cost("ld b;c"), cost("ld b;n"))
        } else {
            let (bytes, taken) = cost("ld b;c");
            ((2 * bytes, 2 * taken), cost("ld bc;nn"))
        };
        if size { load.0 <= copy.0 } else { load.1 <= copy.1 }
    }

    // `reads_flags_set_just_before` stays false: every copy, spill and reload
    // the allocator inserts is an `ld`, which leaves the flags alone, so
    // nothing needs to be kept off the window between a compare and its branch.
    // (A far frame slot is reached by `add iy, sp`, which sets the carry: the
    // expander keeps the flags across it, `Printer::iy_to_stack`.)

    fn reserved(root: RegId) -> bool {
        matches!(root, r if r == reg::SP || r == reg::IX || r == reg::FLAGS)
    }

    fn caller_saved() -> Vec<RegId> {
        vec![reg::A, reg::BC, reg::DE, reg::HL, reg::IY]
    }

    /// A pair and its bytes share storage; two bytes of one pair do not.
    fn overlaps(
        one: RegId,
        other: RegId,
    ) -> bool {
        REGISTER_INFO.root(one) == REGISTER_INFO.root(other)
            && REGISTER_INFO.lanes(one) & REGISTER_INFO.lanes(other) != 0
    }

    /// `ld (ix+d),r` for a byte, two of them for a pair: `ld` only, so a spill
    /// never touches the flags.
    fn spill_store(
        class: Class,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        let name = if class.is_byte() { "SPILL8" } else { "SPILL16" };
        Some(Inst { may_store: true, ..Inst::new(Kind::Op(name), vec![Operand::Frame(slot), Operand::Use(register)]) })
    }

    /// Worse rows (owner greedy-subreg, exit 2026-12-05; net -1083 B O2, -2636 B Os): O2 mem_length_boundaries_m32 +12,
    /// sat_sub_u64_shared_compare +3, switch_peeled_table +4; Os mem_length_boundaries_m32 +1, sat_sub_u64_shared_compare
    /// +1, rep_tails +2, wide_shift_by_one +3. Allocation shifts after a remade address; clocks unpriced.
    fn remats_symbol_addresses() -> bool {
        true
    }

    fn spills_lanes() -> bool {
        true
    }

    /// `LEAFI` puts the address of a frame object in a register.
    fn frame_address(inst: &Inst) -> Option<i32> {
        if inst.kind != Kind::Op("LEAFI") {
            return None;
        }
        inst.operands.iter().find_map(|o| if let Operand::Frame(f) = o { Some(*f) } else { None })
    }

    /// A load of a frame object the call before it filled is made again where
    /// it is read (`call_result_buffers`).
    fn is_buffer_load(name: &str) -> bool {
        matches!(name, "LD16rf" | "LD8rf")
    }

    fn spill_store_lane(
        class: Class,
        register: Reg,
        slot: i32,
        offset: u32,
    ) -> Option<Inst> {
        // a lane is a byte whatever class the value of the pair stands in
        let _ = class;
        Some(Inst {
            may_store: true,
            ..Inst::new(
                Kind::Op("SPILL8L"),
                vec![Operand::Frame(slot), Operand::Use(register), Operand::Imm(i64::from(offset))],
            )
        })
    }

    /// `foldMemoryOperand` (`InlineSpiller::foldMemoryOperand`,
    /// `X86InstrInfo::foldMemoryOperandImpl`): the second source of a byte
    /// ALU op or compare, or of the word `and`/`or`/`xor`, is read from its
    /// frame cell (`and a, d(ix)`), not loaded into a register first.
    fn fold_spill(
        inst: &Inst,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        // a half taken from a spilled pair is that half's byte of the cell
        if let (Kind::Op(op @ ("LO" | "HI")), [Operand::Def(byte), Operand::Use(pair)]) = (inst.kind, &inst.operands[..])
            && *pair == register
            && byte != pair
            && !NO_FOLD.with(std::cell::Cell::get)
        {
            return Some(Inst {
                may_load: true,
                ..Inst::new(
                    Kind::Op("RELOAD8L"),
                    vec![Operand::Def(*byte), Operand::Frame(slot), Operand::Imm(i64::from(op == "HI"))],
                )
            });
        }
        let (Kind::Op("ADD8r" | "SUB8r" | "AND8r" | "OR8r" | "XOR8r" | "CP8r" | "AND16" | "OR16" | "XOR16"), Some(Operand::Use(second))) =
            (inst.kind, inst.operands.get(2))
        else {
            return None;
        };
        let once = inst.operands.iter().filter(|o| matches!(o, Operand::Def(r) | Operand::Use(r) if *r == register)).count() == 1;
        if *second != register || !once || NO_FOLD.with(std::cell::Cell::get) {
            return None;
        }
        let mut folded = inst.clone();
        folded.operands[2] = Operand::Frame(slot);
        folded.may_load = true;
        Some(folded)
    }

    fn spill_load(
        class: Class,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        let name = if class.is_byte() { "RELOAD8" } else { "RELOAD16" };
        Some(Inst { may_load: true, ..Inst::new(Kind::Op(name), vec![Operand::Def(register), Operand::Frame(slot)]) })
    }
}

const CONDITIONAL: [(&str, &str); 2] = [("JPNZ", "JPZ"), ("JPNC", "JPC")];

impl llrm_spike_mf::Jump for Z80Registers {
    fn jump(block: u32) -> Inst {
        let mut inst = Inst::new(Kind::Op("JP"), vec![Operand::Block(block)]);
        inst.terminator = true;
        inst.barrier = true;
        inst
    }

    /// `reverseBranchCondition`: `jp nz` and `jp z` swap, as do `jp nc` and `jp
    /// c`.
    fn reverse(branch: &Inst) -> Option<Inst> {
        let Kind::Op(name) = branch.kind else { return None };
        let opposite = CONDITIONAL
            .iter()
            .find_map(
                |&(one, two)| match () {
                    _ if one == name => Some(two),
                    _ if two == name => Some(one),
                    _ => None,
                },
            )?;
        Some(Inst { kind: Kind::Op(opposite), ..branch.clone() })
    }
}
