//! The selected function on virtual registers, allocated by the shared
//! [`llrm_spike_mf`] allocator, and emitted as ca65.
//!
//! The byte cells of `select.rs` become virtual registers; `a`, `x`, `y` and
//! `rcN` stay physical. An accumulator operation is `COPY a, s; ADC a, b;
//! COPY d, a`: a value lives in `a` only where the allocator joins it to the
//! physical register, so two results can be in flight without both wanting it.
//! A word is two bytes, so a pair never needs a class of its own.
//!
//! DEVIATE: a function with an opcode `build` does not know is not handled
//! here; `select::function_text` falls back to the cell path. Exit: every
//! opcode built.

use std::fmt::Write;

use llrm_codegen::model::ir::Loc;
use llrm_codegen::model::lir::Insn;
use llrm_lir::registers::RegId;
use llrm_spike_mf::commute_accumulator;
use llrm_spike_mf::regalloc::{successors, two_address};
use llrm_spike_mf::spill::allocate_spilling;
use llrm_spike_mf::{
    Block, Inst, Jump, Kind, MFunction, Operand, Reg, Target, branch_fold, forward_reloads, machine_sink_in_place,
    remove_identity_copies, remove_redundant_copies_between, reschedule_kills,
};
use llrm_support::hash::{HashMap, HashSet};

use crate::asm::{Asm, Inst as AsmInst, Line, Op as AsmOp};
use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::hooks::Cpu;
use crate::emit::{MInst, Operand as Machine, address, indexed_address, spill};
use crate::reg;
use crate::select::Item;

/// The registers a value may live in, as a set: `A`, `X`, `Y`, and the
/// zero-page bytes (`MEM`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Class(u8);

impl Class {
    const A: u8 = 1;
    const X: u8 = 2;
    const Y: u8 = 4;
    const M: u8 = 8;
    const P: u8 = 16;
    pub const ANY: Class = Class(15);
    /// What `cmp`, `cpx` and `cpy` test as their first operand.
    pub const COMPARED: Class = Class(Class::A | Class::X | Class::Y);
    /// A zero-page pair, for `(zp),y`.
    pub const PAIR: Class = Class(Class::P);
    /// What an ALU instruction reads as its second operand.
    pub const MEM: Class = Class(Class::M);
}

fn rc(n: u8) -> RegId {
    RegId::from_raw(reg::RC0.index() as u8 + n)
}

/// The zero-page bytes the imaginary registers may use on the platform being
/// compiled for (`Platform::zero_page`): `rc0`..`rc15` and, above them, as many
/// scratch cells as it has room for.
const BASE_CELLS: u8 = 16;

/// What a byte of code is worth against a clock, per level (the tuning struct
/// of this target): a size-reducing choice is taken when each byte it saves
/// costs at most `clocks_per_byte` more clocks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tuning {
    pub clocks_per_byte: u32,
}

impl Tuning {
    /// Speed: a byte saved is not worth a clock.
    pub const SPEED: Tuning = Tuning { clocks_per_byte: 1 };
    /// Size: a byte saved is worth up to twelve clocks (a shift loop of 3 bits
    /// is 3 bytes and 16 clocks; three shared callee-saved cells are 9
    /// bytes and 99 clocks).
    pub const SIZE: Tuning = Tuning { clocks_per_byte: 12 };
}

thread_local! {
    /// Whether a function may save its callee-saved cells
    /// with `__csave`/`__crest` (see [`with_shared_saves`]).
    static SHARED_SAVES: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static IMAGINARY: std::cell::Cell<u8> = const { std::cell::Cell::new(BASE_CELLS) };
    static TUNING: std::cell::Cell<Tuning> = const { std::cell::Cell::new(Tuning::SPEED) };
    static SPIKE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static EMITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Whether a long division by a divisor not known to be a byte calls the
    /// speed routine with the pin-structure wide path, and not the one with
    /// the small wide path (see [`with_fast_division`]).
    static FAST_DIVISION: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    /// Whether a long multiply calls `__mulsi3_fast` (see
    /// [`with_fast_multiply`]).
    static FAST_MULTIPLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Whether a long division calls the speed routine at size too (see
    /// [`with_size_fast_division`]).
    static SIZE_FAST_DIVISION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `f` with the long divisions calling the speed routines at size: a module
/// asks for them where the clocks they save are worth the bytes
/// (`select::module_text`).
pub fn with_size_fast_division<R>(
    allowed: bool,
    f: impl FnOnce() -> R,
) -> R {
    let before = SIZE_FAST_DIVISION.with(|a| a.replace(allowed));
    let result = f();
    SIZE_FAST_DIVISION.with(|a| a.set(before));
    result
}

/// `f` with the speed multiply called or not: 19 bytes linked once, a module
/// asks for it only where its multiply sites run (`hotmultiply`,
/// `select::module_text`).
pub fn with_fast_multiply<R>(
    allowed: bool,
    f: impl FnOnce() -> R,
) -> R {
    let before = FAST_MULTIPLY.with(|a| a.replace(allowed));
    let result = f();
    FAST_MULTIPLY.with(|a| a.set(before));
    result
}

/// `f` with the wide speed division allowed or not. Linked once, it costs
/// 141 bytes (`hotdivision`): a module keeps it only where a division site runs
/// often enough to pay that (`select::module_text`).
pub fn with_fast_division<R>(
    allowed: bool,
    f: impl FnOnce() -> R,
) -> R {
    let before = FAST_DIVISION.with(|a| a.replace(allowed));
    let result = f();
    FAST_DIVISION.with(|a| a.set(before));
    result
}

/// `f` with the shared callee-save helpers allowed or not. A function that
/// keeps `n >= 3` callee-saved cells saves `6n - 9` bytes with them, but the
/// runtime that holds them is [`SAVE_HELPERS_BYTES`] a module: only a module
/// whose users together save more than that should link it
/// (`select::module_text`).
pub fn with_shared_saves<R>(
    allowed: bool,
    f: impl FnOnce() -> R,
) -> R {
    let before = SHARED_SAVES.with(|a| a.replace(allowed));
    let result = f();
    SHARED_SAVES.with(|a| a.set(before));
    result
}

/// The bytes `__csave` and `__crest` take once linked.
pub const SAVE_HELPERS_BYTES: i64 = 34;

/// What a function that keeps `n` callee-saved cells saves by calling the
/// helpers: six bytes a cell inline, and two for the result, against eleven.
pub fn shared_save_gain(n: u32) -> i64 {
    6 * i64::from(n) + 2 - 11
}

fn imaginary() -> u8 {
    IMAGINARY.with(|n| n.get())
}

/// The zero-page bytes a value may live in: `rc2`..`rc7`, which a call
/// clobbers, then the platform's extra scratch cells `rc16`.., which a call
/// clobbers too, then `rc8`..`rc15`, which a callee keeps (see [`preserved`]).
/// Scratch comes before the kept cells: using one costs no save and restore.
fn cells() -> impl Iterator<Item = RegId> {
    (2..8).chain(BASE_CELLS..imaginary()).chain(8..BASE_CELLS).map(rc)
}

/// How many bytes a value may live in on the platform with `zero_page` cells
/// (`BASE_CELLS` at least): `a`, `x`, `y` and [`cells`], and how many of those
/// a callee keeps. The register file `Machine::registers`/`call_registers`
/// state to the mid-end, counted from the allocator's own pools.
pub fn register_file(zero_page: u8) -> (i64, i64) {
    let before = IMAGINARY.with(|n| n.replace(zero_page.clamp(BASE_CELLS, MOST_CELLS)));
    let bytes = 3 + cells().count() as i64;
    let kept = cells().filter(|&cell| preserved(cell)).count() as i64;
    IMAGINARY.with(|n| n.set(before));
    (bytes, kept)
}

/// The pairs a pointer may live in: `rs1`..`rs3` over `rc2`..`rc7`, the scratch
/// pairs `rs8`.., then `rs4`..`rs7`.
fn pairs() -> impl Iterator<Item = RegId> {
    let scratch = (BASE_CELLS / 2..imaginary() / 2).map(|n| RegId::from_raw(reg::RS0.index() as u8 + n));
    [reg::RS1, reg::RS2, reg::RS3].into_iter().chain(scratch).chain([reg::RS4, reg::RS5, reg::RS6, reg::RS7])
}

/// A callee-saved zero-page byte or pair (`rc8`..`rc15`, `rs4`..`rs7`): a
/// function that uses one saves it on entry and restores it before each return,
/// so a value in it survives a call.
fn preserved(register: RegId) -> bool {
    let at = register.index();
    (reg::RC8.index()..=reg::RC15.index()).contains(&at) || (reg::RS4.index()..=reg::RS7.index()).contains(&at)
}

fn is_pair(register: RegId) -> bool {
    pairs().any(|one| one == register)
}

/// A pair or the soft stack pointer `rs0`, which no value is allocated to.
fn is_word(register: RegId) -> bool {
    register == reg::RS0 || is_pair(register)
}

/// The zero-page byte of a pair: lane 0 is the low byte.
fn lane(
    pair: RegId,
    lane: u8,
) -> RegId {
    rc(2 * (pair.index() as u8 - reg::RS0.index() as u8) + lane)
}

/// Whether `order` holds a zero-page pair.
fn is_pair_order(order: &[RegId]) -> bool {
    order.iter().any(|register| is_pair(*register))
}

fn is_cell(register: RegId) -> bool {
    register.index() >= reg::RC0.index()
}

/// The target; `STACK` keeps spills on the soft stack (a reentrant function)
/// instead of in static cells, which costs `y` at each spill.
pub struct Mos<const STACK: bool>;

impl<const STACK: bool> Target for Mos<STACK> {
    /// The second source of a byte op on the accumulator is a zero-page cell.
    fn operand_class(
        name: &str,
        index: usize,
    ) -> Option<Class> {
        (index == 2 && matches!(name, "ADC" | "AND" | "ORA" | "EOR")).then_some(Class::MEM)
    }

    /// A copy is priced as the load of a cell into a register, `ldx rc2`: the
    /// join of a value with its argument cell removes that copy.
    fn copy_cost() -> u32 {
        crate::timing::estimate(Cpu::Mos6502, "ldx rc2").map_or(1, |priced| priced.bytes + priced.cycles as u32)
    }

    /// The copy of `from` into `to` in the timing tables: a transfer between
    /// `a`, `x` and `y` (two through `a` between `x` and `y`), a store or a
    /// load of a cell, and `lda`+`sta` between two cells. Greedy hints a
    /// value the register that makes its copies cheapest.
    fn copy_register_cost(
        to: RegId,
        from: RegId,
        size: bool,
    ) -> u32 {
        // Dormant: the hint moves 39 cells worse than the tip (6502-HINT-1
        // follow-up).
        if !llrm_support::env_flag!("LLRM_6502_COPY_HINT") {
            return 0;
        }
        let is_index = |register: RegId| matches!(register, reg::A | reg::X | reg::Y);
        if to == from || !(is_index(to) || is_cell(to)) || !(is_index(from) || is_cell(from)) {
            return 0;
        }
        // bytes when the function is sized; clocks plus 3 per byte otherwise
        // (the project's O2 rate)
        let price = |mnemonic: &str, operand: &str| {
            crate::timing::instruction(Cpu::Mos6502, mnemonic, operand)
                .map_or(0, |cost| if size { cost.bytes } else { cost.cycles as u32 + 3 * cost.bytes })
        };
        let name = |register: RegId| match register {
            reg::A => "a",
            reg::X => "x",
            _ => "y",
        };
        match (is_cell(to), is_cell(from)) {
            (true, true) => price("lda", "rc2") + price("sta", "rc2"),
            (true, false) => price(&format!("st{}", name(from)), "rc2"),
            (false, true) => price(&format!("ld{}", name(to)), "rc2"),
            (false, false) if to == reg::A || from == reg::A => price(&format!("t{}{}", name(from), name(to)), ""),
            (false, false) => 2 * price("tax", ""),
        }
    }

    /// `isAsCheapAsAMove` against the copy it replaces. In a register a literal
    /// is `lda #n` (2 B, 2 clocks) against `tax`/`txa`/`tay`/`tya` (1 B, 2
    /// clocks): the copy stays, as does a tie in clocks; there is no `xor a`.
    /// In a cell the literal is `lda #n` + `sta rc` (4 B, 5 clocks) against
    /// `sta rc` (2 B, 3) from a register or `lda rc` + `sta rc` (4 B, 6) from a
    /// cell: only a cell-to-cell copy loses to it. Unpriced, `-Os` made a zero
    /// again where `txa` was one byte (`shift_int32_variable`, +3 B).
    fn literal_move_wins(
        literal: &Insn,
        source: &[RegId],
        destination: &[RegId],
        size: bool,
    ) -> bool {
        let Some(what) = literal.what.as_ref() else { return false };
        let ([Loc::Held(_)], [Loc::Imm(_)]) = (what.dests.as_slice(), what.sources.as_slice()) else {
            return false;
        };
        let price = |mnemonic: &str, operand: &str| {
            crate::timing::instruction(Cpu::Mos6502, mnemonic, operand)
                .map_or((u32::MAX / 4, u32::MAX / 4), |cost| (cost.bytes, cost.cycles))
        };
        let in_cells =
            |registers: &[RegId]| !registers.is_empty() && registers.iter().all(|register| is_cell(*register));
        let (copy, remat) = if in_cells(destination) {
            let (load, store) = (price("lda", "#0"), price("sta", "rc2"));
            let from = if in_cells(source) { price("lda", "rc2") } else { (0, 0) };
            ((from.0 + store.0, from.1 + store.1), (load.0 + store.0, load.1 + store.1))
        } else {
            (price("tax", ""), price("lda", "#0"))
        };
        if size { remat.0 <= copy.0 } else { remat.1 < copy.1 }
    }

    /// A byte class the compare class narrowed has `any` as its larger class.
    fn larger_order(order: &[RegId]) -> Option<Vec<RegId>> {
        if !order.iter().all(|register| matches!(*register, reg::A | reg::X | reg::Y) || is_cell(*register))
            || is_pair_order(order)
        {
            return None;
        }
        let any = Self::candidates(Class::ANY);
        (any.len() > order.len()).then_some(any)
    }

    /// What `inst` costs with its operand `operand` in `register`: the bytes
    /// and the clocks of its form in the timing tables (a step is 1 byte and 2
    /// clocks in `x`, 2 and 5 on a cell, 3 and 4 in `a`). The operand's other
    /// appearances in the instruction are in `register` too; any other
    /// register is priced where it stands, a virtual one as `a`. An
    /// instruction with no register-dependent form costs 0.
    fn operand_cost(
        inst: &Inst,
        operand: usize,
        register: RegId,
    ) -> u32 {
        let priced = inst.operands.get(operand);
        let regs: Vec<RegId> = inst
            .operands
            .iter()
            .filter_map(|o| match o {
                Operand::Def(r) | Operand::Use(r)
                    if Some(o) == priced
                        || matches!(
                            (o, priced),
                            (Operand::Def(a) | Operand::Use(a), Some(Operand::Def(b) | Operand::Use(b))) if a == b
                        ) =>
                {
                    let _ = r;
                    Some(register)
                }
                Operand::Def(Reg::Phys(r)) | Operand::Use(Reg::Phys(r)) => Some(*r),
                Operand::Def(_) | Operand::Use(_) => Some(reg::A),
                _ => None,
            })
            .collect();
        form_cost(Cpu::Mos6502, inst.kind, &inst.operands, &regs)
            .map_or(0, |priced| priced.bytes + priced.cycles as u32)
    }

    type Class = Class;

    fn meet(
        one: Class,
        other: Class,
    ) -> Option<Class> {
        let both = one.0 & other.0;
        (both != 0).then_some(Class(both))
    }

    fn any() -> Class {
        Class::ANY
    }

    /// Index registers first: moving a value in or out of one is a transfer,
    /// the cheapest copy; `a` last, since most code wants it.
    fn candidates(class: Class) -> Vec<RegId> {
        let mut out = Vec::new();
        if class.0 & Class::X != 0 {
            out.push(reg::X);
        }
        if class.0 & Class::Y != 0 {
            out.push(reg::Y);
        }
        if class.0 & Class::M != 0 {
            out.extend(cells());
        }
        if class.0 & Class::A != 0 {
            out.push(reg::A);
        }
        if class.0 & Class::P != 0 {
            out.extend(pairs());
        }
        out
    }

    fn view(
        root: RegId,
        _: Class,
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        register
    }

    /// `tax`, `tay`, `txa`, `tya` and the loads `lda`, `ldx`, `ldy` set N and
    /// Z; a store sets nothing.
    fn copy_sets_flags(
        to: RegId,
        _: RegId,
    ) -> bool {
        [reg::A, reg::X, reg::Y].contains(&to)
    }

    /// In stack mode a spill is `ldy #offset; sta (rc0),y`: it overwrites `y`.
    fn reads_flags_set_just_before(inst: &Inst) -> bool {
        inst.reads_flags() && inst.terminator
    }

    fn lane_moves(inst: &Inst) -> Option<(usize, Vec<(usize, u32, u32)>)> {
        match inst.kind {
            Kind::Op("PACK") => Some((0, vec![(1, 0, 1), (2, 1, 1)])),
            Kind::Op("UNPACK") => Some((2, vec![(0, 0, 1), (1, 1, 1)])),
            _ => None,
        }
    }

    // DEVIATE 3b-1 (owner greedy-subreg): a pair is built and taken apart by
    // lane, as the pin's sub-register coalescing does.
    fn lowers_lanes() -> bool {
        true
    }

    fn joins_lanes() -> bool {
        true
    }

    fn moves_lanes(inst: &Inst) -> bool {
        matches!(inst.kind, Kind::Op("PACK" | "UNPACK"))
    }

    /// A byte cell: `lda cell; pha` on entry, `pla; sta cell` on exit.
    fn csr_first_use_cost() -> u32 {
        4
    }

    fn csr_cost_scale() -> u32 {
        100
    }

    fn csr_spill_first() -> bool {
        true
    }

    /// A zero-page byte costs more per use than `A`, `X` and `Y`: an
    /// instruction on a cell is a byte longer and a clock slower than the
    /// same on a register, and a transfer is two clocks against three.
    fn register_cost(register: RegId) -> u8 {
        let first = reg::RC0.index();
        u8::from((first..first + usize::from(imaginary())).contains(&register.index()))
    }

    fn spill_clobbers() -> Vec<RegId> {
        if STACK { vec![reg::Y] } else { Vec::new() }
    }

    /// `rc0` and `rc1` are the software stack pointer.
    fn reserved(root: RegId) -> bool {
        root == reg::RC0 || root == reg::RC1
    }

    fn caller_saved() -> Vec<RegId> {
        [reg::A, reg::X, reg::Y].into_iter().chain(cells()).chain(pairs()).filter(|r| !preserved(*r)).collect()
    }

    fn lanes_of(register: RegId) -> Option<[RegId; 2]> {
        is_pair(register).then(|| [lane(register, 0), lane(register, 1)])
    }

    fn overlaps(
        one: RegId,
        other: RegId,
    ) -> bool {
        one == other
            || (is_pair(one) && (lane(one, 0) == other || lane(one, 1) == other))
            || (is_pair(other) && (lane(other, 0) == one || lane(other, 1) == one))
    }

    // saving and restoring a callee-saved cell is five bytes each way; a
    // spilled value costs about three bytes a mention (the reload) over a
    // direct use, and a mention in a loop weighs ten
    fn worth_taking(
        root: RegId,
        first_use: bool,
        weight: u64,
    ) -> bool {
        let bytes = if is_pair(root) { 20 } else { 10 };
        !preserved(root) || !first_use || weight * 2 >= bytes
    }

    fn spill_store(
        _: Class,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        let mut spill = Inst::new(Kind::Op("SPILL"), vec![Operand::Use(register), Operand::Frame(slot)]);
        if STACK {
            // `ldy #offset; sta (rc0),y`: it clobbers `y`, even a spilled `y`
            spill.operands.push(Operand::Def(Reg::Phys(reg::Y)));
            spill.early_clobber = true;
        }
        Some(spill)
    }

    /// The second operand of an ALU op or a compare is a memory operand: a
    /// spilled byte is read from its cell as it is (`ora s_f_3`), not
    /// loaded into a zero-page cell first.
    fn fold_spill(
        inst: &Inst,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        let ([_, _, Operand::Use(second), ..], Kind::Op("ADC" | "SBC" | "AND" | "ORA" | "EOR" | "CMP")) =
            (&inst.operands[..], inst.kind)
        else {
            return None;
        };
        let once =
            inst.operands.iter().filter(|o| matches!(o, Operand::Def(r) | Operand::Use(r) if *r == register)).count()
                == 1;
        if STACK || *second != register || !once {
            return None;
        }
        let mut folded = inst.clone();
        folded.operands[2] = Operand::Frame(slot);
        Some(folded)
    }

    fn spill_load(
        _: Class,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        let mut reload = Inst::new(Kind::Op("RELOAD"), vec![Operand::Def(register), Operand::Frame(slot)]);
        if STACK {
            reload.operands.push(Operand::Def(Reg::Phys(reg::Y)));
        }
        Some(reload)
    }
}

/// An opcode `build` does not know.
#[derive(Debug)]
pub struct Unsupported(pub String);

struct Builder {
    function: MFunction<Class>,
    /// The source place of the item being built (`MInst::origin`); every
    /// instruction it makes takes it.
    origin: u32,
    bytes: HashMap<String, Reg>,
    /// Word cells used as pointers: one pair register, not two bytes.
    pointers: HashSet<String>,
    /// Pointers written as two bytes, packed into their pair after the
    /// instruction.
    packs: Vec<(Reg, Reg, Reg)>,
    /// The half of a pointer cell a byte copy wrote, until the other half
    /// arrives and the pair is packed (a pointer returned in a and x).
    halves: HashMap<String, (bool, Reg)>,
    /// Cells defined once, by an immediate load: they fold into the user.
    constants: HashMap<String, i64>,
    /// Cells loaded once from a global and read once by an add, subtract or
    /// logic op after it with no store between: the op reads the global.
    /// The block each label names.
    block_of: HashMap<String, u32>,
    /// How many times each cell or register has been defined so far.
    versions: HashMap<String, u32>,
    /// High halves of pointer cells written as plain bytes: the pair never sees
    /// them.
    lost_halves: HashSet<String>,
    /// The compare whose flags the next branch reads, and the ones saved to
    /// cells for later.
    pending: Option<Compare>,
    /// The compare of the next branch is already in the block
    /// (`spill::COMPARE`).
    compared: bool,
    saved: HashMap<String, Compare>,
    /// Compares are redone where the flags are read: nothing keeps `P` alive.
    /// The block instructions go to.
    current: usize,
    /// Where the sign splat of a byte cell (all ones or all zeros) is held,
    /// by cell, version and block, with the version of the holder. The splat
    /// of a splat is the cell itself.
    splats: HashMap<(String, u32, usize), (String, u32)>,
}

/// A compare not yet emitted: the operands and the version of each when it
/// was made, so a later redo can tell they have not changed.
#[derive(Clone, Debug)]
struct Compare {
    a: (String, u32),
    b: (String, u32),
    /// Over words: `lda; cmp` on the low bytes, then `lda; sbc` on the high.
    word: bool,
    /// Over words, for Z only: the high bytes are compared when the low are
    /// equal.
    zero: bool,
    /// The right side is this global in memory, read where the flags are made
    /// (`cmp g; sbc g+1`); `b` names nothing. Kept only when no store, call
    /// or saved flags lie between the compare and the end of its block.
    memory: Option<String>,
    /// `memory` is indexed by this byte (`cmp g,x`), which must hold what it
    /// held at the compare.
    index: Option<(String, u32)>,
    /// Over two words: the high pair, subtracted with the borrow of the first.
    wide: Option<((String, u32), (String, u32))>,
}

impl Builder {
    fn fresh(&mut self) -> Reg {
        self.function.classes.push(Class::ANY);
        Reg::Virt(self.function.classes.len() as u32 - 1)
    }

    /// `value` copied to a register of `class`, as LLVM's `InstrEmitter` copies
    /// an operand to the class its instruction needs: the allocator joins the
    /// copy to `value` when `value` is already in the class.
    fn copy_to_class(
        &mut self,
        value: Reg,
        class: Class,
    ) -> Reg {
        self.function.classes.push(class);
        let copy = Reg::Virt(self.function.classes.len() as u32 - 1);
        self.copy(copy, value);
        copy
    }

    /// The register a byte operand names: `a`, `x`, `y`, `rcN`, or a cell.
    fn byte(
        &mut self,
        name: &str,
    ) -> Result<Reg, Unsupported> {
        let named = |n: &str| match n {
            "a" => Some(reg::A),
            "x" => Some(reg::X),
            "y" => Some(reg::Y),
            _ => n.strip_prefix("rc").and_then(|n| n.parse::<u8>().ok()).filter(|n| *n < 16).map(rc),
        };
        if let Some(r) = named(name) {
            return Ok(Reg::Phys(r));
        }
        if !name.starts_with("v_") && !name.starts_with("w_") {
            return Err(Unsupported(format!("operand {name}")));
        }
        if self.lost_halves.contains(name) {
            return Err(Unsupported(format!("{name}, the high half of a pointer cell, is written as a byte")));
        }
        if let Some(r) = self.bytes.get(name) {
            return Ok(*r);
        }
        // A half of a pointer cell lives in its pair: a read of it unpacks
        // the pair. Without this the half was a byte nothing defined
        // (strs -O2: "a unique value" in the coalescer).
        let word = name.strip_suffix("+1").unwrap_or(name);
        if self.pointers.contains(word) {
            let (low, high) = self.read_word(word)?;
            return Ok(if name.ends_with("+1") { high } else { low });
        }
        let r = self.fresh();
        self.bytes.insert(name.to_owned(), r);
        Ok(r)
    }

    /// Whether `name` is a word with a half known to be a constant: an ALU op
    /// takes that half as an immediate, and a step by a constant is its own
    /// form.
    fn has_known_half(
        &self,
        name: &str,
    ) -> bool {
        let base = name.strip_suffix("+1").unwrap_or(name);
        !self.pointers.contains(base)
            && (self.constants.contains_key(base)
                || self.constants.contains_key(&format!("{base}+1"))
                || self.word_constant(base).is_some())
    }

    /// The two bytes of `name` when it is a word loaded once from an
    /// immediate and never a pointer: an ALU op takes them as immediates.
    fn word_constant(
        &self,
        name: &str,
    ) -> Option<[i64; 2]> {
        let name = name.strip_suffix("+1").unwrap_or(name);
        if self.pointers.contains(name) {
            return None;
        }
        Some([*self.constants.get(name)?, *self.constants.get(&format!("{name}+1"))?])
    }

    /// The low and high byte of a word to read: a pointer cell is unpacked
    /// into two new bytes.
    fn read_word(
        &mut self,
        name: &str,
    ) -> Result<(Reg, Reg), Unsupported> {
        if !self.pointers.contains(name) {
            return self.word(name);
        }
        let pair = self.pointer(name)?;
        let (low, high) = (self.fresh(), self.fresh());
        let mut unpack = Inst::new(Kind::Op("UNPACK"), vec![Operand::Def(low), Operand::Def(high), Operand::Use(pair)]);
        unpack.early_clobber = true;
        self.push(unpack);
        Ok((low, high))
    }

    /// `d := s` for bytes, where either may be a half of a pointer cell: a
    /// half read is unpacked from the pair, and the two halves written are
    /// packed into it when the second arrives.
    fn copy_byte(
        &mut self,
        d: &str,
        s: &str,
    ) -> Result<(), Unsupported> {
        let half = |b: &Builder, n: &str| {
            let word = n.strip_suffix("+1").unwrap_or(n);
            b.pointers.contains(word).then(|| (word.to_owned(), n.ends_with("+1")))
        };
        let source = match half(self, s) {
            Some((word, high)) => {
                let (low, top) = self.read_word(&word)?;
                if high { top } else { low }
            }
            None => self.byte(s)?,
        };
        let Some((word, high)) = half(self, d) else {
            let splat = self.splat_of(s).is_some_and(|held| held == s);
            self.define(d);
            if splat {
                self.note_splat(d, d);
            }
            let d = self.byte(d)?;
            self.copy(d, source);
            return Ok(());
        };
        let Some((other_high, other)) = self.halves.remove(&word) else {
            self.halves.insert(word, (high, source));
            return Ok(());
        };
        if other_high == high {
            return Err(Unsupported(format!("half of {word} written twice")));
        }
        let (low, top) = if high { (other, source) } else { (source, other) };
        self.define(&word);
        let pair = self.pointer(&word)?;
        let mut pack = Inst::new(Kind::Op("PACK"), vec![Operand::Def(pair), Operand::Use(low), Operand::Use(top)]);
        pack.early_clobber = true;
        self.push(pack);
        Ok(())
    }

    /// The bytes to write a word through: a pointer cell gets two new bytes
    /// and is packed after the instruction.
    fn write_word(
        &mut self,
        name: &str,
    ) -> Result<(Reg, Reg), Unsupported> {
        self.define(name);
        if !self.pointers.contains(name) {
            return self.word(name);
        }
        let pair = self.pointer(name)?;
        let (low, high) = (self.fresh(), self.fresh());
        self.packs.push((pair, low, high));
        Ok((low, high))
    }

    fn flush_packs(&mut self) {
        for (pair, low, high) in std::mem::take(&mut self.packs) {
            let mut pack = Inst::new(Kind::Op("PACK"), vec![Operand::Def(pair), Operand::Use(low), Operand::Use(high)]);
            pack.early_clobber = true;
            self.push(pack);
        }
    }

    /// The pair register of a pointer cell or of `rsN`.
    fn pointer(
        &mut self,
        name: &str,
    ) -> Result<Reg, Unsupported> {
        if let Some(n) = name.strip_prefix("rs").and_then(|n| n.parse::<u8>().ok()) {
            return match n {
                0 => Ok(Reg::Phys(reg::RS0)),
                1 => Ok(Reg::Phys(reg::RS1)),
                2 => Ok(Reg::Phys(reg::RS2)),
                3 => Ok(Reg::Phys(reg::RS3)),
                _ => Err(Unsupported(format!("pair {name}"))),
            };
        }
        if !self.pointers.contains(name) {
            return Err(Unsupported(format!("{name} is not a pointer cell")));
        }
        if let Some(r) = self.bytes.get(name) {
            return Ok(*r);
        }
        self.function.classes.push(Class::PAIR);
        let r = Reg::Virt(self.function.classes.len() as u32 - 1);
        self.bytes.insert(name.to_owned(), r);
        Ok(r)
    }

    /// Whether `name` is held as a pair register.
    fn in_pair(
        &self,
        name: &str,
    ) -> bool {
        self.pointers.contains(name) || matches!(name, "rs0" | "rs1" | "rs2" | "rs3")
    }

    fn word(
        &mut self,
        name: &str,
    ) -> Result<(Reg, Reg), Unsupported> {
        if let Some(n) = name.strip_prefix("rs").and_then(|n| n.parse::<u8>().ok()).filter(|n| *n < 8) {
            return Ok((Reg::Phys(rc(2 * n)), Reg::Phys(rc(2 * n + 1))));
        }
        let name = name.strip_suffix("+1").unwrap_or(name);
        let low = self.byte(name)?;
        let high = self.byte(&format!("{name}+1"))?;
        Ok((low, high))
    }

    fn require(
        &mut self,
        register: Reg,
        class: Class,
    ) -> Result<(), Unsupported> {
        match register {
            Reg::Virt(v) => {
                let met = Mos::<false>::meet(self.function.classes[v as usize], class)
                    .ok_or_else(|| Unsupported("a value needed in two places".into()))?;
                self.function.classes[v as usize] = met;
                Ok(())
            }
            Reg::Phys(p) if is_cell(p) || p == reg::RS0 || is_pair(p) => Ok(()),
            Reg::Phys(_) => Err(Unsupported("a register as a memory operand".into())),
        }
    }

    /// `name` is defined here.
    fn define(
        &mut self,
        name: &str,
    ) {
        *self.versions.entry(name.to_owned()).or_default() += 1;
        if name.strip_suffix("+1").is_some_and(|word| self.pointers.contains(word)) {
            self.lost_halves.insert(name.to_owned());
        }
    }

    fn version(
        &self,
        name: &str,
    ) -> u32 {
        self.versions.get(name).copied().unwrap_or(0)
    }

    /// `held` (just written) holds the sign splat of `of`.
    fn note_splat(
        &mut self,
        of: &str,
        held: &str,
    ) {
        let value = (held.to_owned(), self.version(held));
        self.splats.insert((of.to_owned(), self.version(of), self.current), value);
    }

    /// The cell that still holds the sign splat of `of`, if one does.
    fn splat_of(
        &self,
        of: &str,
    ) -> Option<String> {
        let (held, version) = self.splats.get(&(of.to_owned(), self.version(of), self.current))?;
        (self.version(held) == *version).then(|| held.clone())
    }

    /// The fixed register an incoming argument's value arrived in, followed
    /// back through copies and the unpacking of a pair into its bytes.
    fn arrives_in(
        &self,
        register: Reg,
    ) -> Option<RegId> {
        let Reg::Virt(mut v) = register else { return None };
        let mut lane_of: Option<u8> = None;
        for _ in 0..8 {
            let def = self
                .function
                .blocks
                .iter()
                .flat_map(|block| &block.insts)
                .find(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(d)) if *d == v)))?;
            let source = match (def.kind, &def.operands[..]) {
                (Kind::Copy, [Operand::Def(_), Operand::Use(source)]) => *source,
                (Kind::Op("UNPACK"), [Operand::Def(low), Operand::Def(_), Operand::Use(source)]) => {
                    lane_of = Some(u8::from(*low != Reg::Virt(v)));
                    *source
                }
                _ => return None,
            };
            match source {
                Reg::Phys(p) => return Some(lane_of.map_or(p, |n| lane(p, n))),
                Reg::Virt(next) => v = next,
            }
        }
        None
    }

    /// `count` single-bit steps of `step` on the accumulator. An arithmetic
    /// shift right by 7 leaves only the sign, all ones or all zeros, which
    /// is cheaper than seven `cmp #$80; ror a`.
    fn shift_a(
        &mut self,
        step: &'static str,
        count: i64,
    ) {
        let acc = Reg::Phys(reg::A);
        if step == "ASR" && count == 7 {
            self.push(Inst::new(Kind::Op("SPLAT"), vec![Operand::Def(acc), Operand::Use(acc)]));
            return;
        }
        // a logical shift by 7 leaves one bit: the carry out of the other end,
        // rotated into a zero
        if let ("LSR" | "ASL", 7) = (step, count) {
            let one = if step == "LSR" { "TOPBIT" } else { "LOWBIT" };
            self.push(Inst::new(Kind::Op(one), vec![Operand::Def(acc), Operand::Use(acc)]));
            return;
        }
        for _ in 0..count {
            self.push(Inst::new(Kind::Op(step), vec![Operand::Def(acc), Operand::Use(acc)]));
        }
    }

    /// The pending compare, if its right side is a global, with that global
    /// loaded into a cell now: what the compare reads from here on cannot
    /// change under a later store or call.
    fn load_compared_global(
        &mut self,
        at: usize,
    ) -> Result<(), Unsupported> {
        let Some(Compare { a, word, memory: Some(symbol), index, .. }) = self.pending.clone() else { return Ok(()) };
        let cell = format!("{}___cmpm{at}", if word { "w" } else { "v" });
        let acc = Reg::Phys(reg::A);
        if let Some((index, _)) = index {
            let (x, i) = (Reg::Phys(reg::X), self.byte(&index)?);
            self.copy(x, i);
            self.define(&cell);
            let d = self.byte(&cell)?;
            self.push(Inst::new(
                Kind::Op("LDAX"),
                vec![Operand::Def(acc), Operand::Symbol(symbol, 0), Operand::Use(x)],
            ));
            self.copy(d, acc);
        } else if word {
            let (low, high) = self.write_word(&cell)?;
            for (d, half) in [low, high].into_iter().zip([symbol.clone(), format!("{symbol}+1")]) {
                self.push(Inst::new(Kind::Op("LDM"), vec![Operand::Def(acc), Operand::Symbol(half, 0)]));
                self.copy(d, acc);
            }
        } else {
            self.define(&cell);
            let d = self.byte(&cell)?;
            self.push(Inst::new(Kind::Op("LDM"), vec![Operand::Def(acc), Operand::Symbol(symbol, 0)]));
            self.copy(d, acc);
        }
        let version = self.version(&cell);
        self.pending =
            Some(Compare { a, b: (cell, version), word, zero: false, memory: None, index: None, wide: None });
        Ok(())
    }

    /// `COPY a, lhs; CMP a, rhs` again, as the flags' producer: the operands
    /// must hold what they held at the first compare.
    ///
    /// `code` is the condition its reader tests (`cond`): MI and PL against a
    /// word zero read the sign of the high byte, and the low byte and the
    /// borrow cannot change it.
    fn redo_compare(
        &mut self,
        code: i64,
        branch: bool,
    ) -> Result<(), Unsupported> {
        let Compare { a, b, word, zero, memory, index, wide } =
            self.pending.clone().ok_or_else(|| Unsupported("flags with no compare".into()))?;
        if self.version(&a.0) != a.1
            || memory.is_none() && self.version(&b.0) != b.1
            || index.as_ref().is_some_and(|(cell, version)| self.version(cell) != *version)
        {
            return Err(Unsupported("a compare's operand changed before its branch".into()));
        }
        let acc = Reg::Phys(reg::A);
        if zero {
            let (al, ah) = self.read_word(&a.0)?;
            // a constant right side is two immediates, not two cells loaded
            // with it
            let rhs = match self.word_constant(&b.0) {
                Some(bytes) => bytes.map(Operand::Imm),
                None => {
                    let (bl, bh) = self.read_word(&b.0)?;
                    for r in [bl, bh] {
                        self.require(r, Class::MEM)?;
                    }
                    [Operand::Use(bl), Operand::Use(bh)]
                }
            };
            for r in [al, ah] {
                self.require(r, Class(Class::X | Class::Y | Class::M))?;
            }
            let mut cmp = Inst::new(
                Kind::Op("CMPZ"),
                vec![
                    Operand::Def(Reg::Phys(reg::FLAGS)),
                    Operand::Def(acc),
                    Operand::Use(al),
                    Operand::Use(ah),
                    rhs[0].clone(),
                    rhs[1].clone(),
                ],
            );
            cmp.sets_flags = true;
            self.push(cmp);
            return Ok(());
        }
        if word && matches!(code, 4 | 5) && self.word_constant(&b.0) == Some([0, 0]) {
            let (_, ah) = self.read_word(&a.0)?;
            let tested = if branch {
                self.copy_to_class(ah, Class::COMPARED)
            } else {
                self.copy(acc, ah);
                acc
            };
            let mut test = Inst::new(
                Kind::Op("CMP"),
                vec![Operand::Def(Reg::Phys(reg::FLAGS)), Operand::Use(tested), Operand::Imm(0)],
            );
            test.sets_flags = true;
            self.push(test);
            return Ok(());
        }
        let (lhs, rhs) = if word {
            let (al, ah) = self.read_word(&a.0)?;
            let rhs = match (&memory, self.word_constant(&b.0)) {
                (Some(symbol), _) => [Operand::Symbol(symbol.clone(), 0), Operand::Symbol(format!("{symbol}+1"), 0)],
                (None, Some(bytes)) => bytes.map(Operand::Imm),
                (None, None) => {
                    let (bl, bh) = self.read_word(&b.0)?;
                    for r in [bl, bh] {
                        self.require(r, Class::MEM)?;
                    }
                    [Operand::Use(bl), Operand::Use(bh)]
                }
            };
            ([al, ah], rhs)
        } else {
            let lhs = self.byte(&a.0)?;
            let rhs = match (&memory, self.constants.get(&b.0)) {
                (Some(symbol), _) if index.is_some() => Operand::Symbol(format!("{symbol},x"), 0),
                (Some(symbol), _) => Operand::Symbol(symbol.clone(), 0),
                (None, Some(value)) => Operand::Imm(*value & 0xFF),
                (None, None) => {
                    let r = self.byte(&b.0)?;
                    self.require(r, Class::MEM)?;
                    Operand::Use(r)
                }
            };
            ([lhs, lhs], [rhs.clone(), rhs])
        };
        // A branch on a byte tests the flags and nothing else: any of `a`,
        // `x`, `y` compares. A select or a set reads `a` as the compare left
        // it, and a word's high byte is subtracted in `a`, so those compare
        // there.
        // `cmp g,x` exists for `a` alone
        let tested = if branch && !word && index.is_none() {
            self.copy_to_class(lhs[0], Class::COMPARED)
        } else {
            self.copy(acc, lhs[0]);
            acc
        };
        let mut cmp =
            Inst::new(Kind::Op("CMP"), vec![Operand::Def(Reg::Phys(reg::FLAGS)), Operand::Use(tested), rhs[0].clone()]);
        if let Some((cell, _)) = &index {
            let (x, i) = (Reg::Phys(reg::X), self.byte(cell)?);
            self.copy(x, i);
            cmp.operands.push(Operand::Use(x));
        }
        cmp.sets_flags = true;
        self.push(cmp);
        if word {
            self.copy(acc, lhs[1]);
            let mut sbc = Inst::new(
                Kind::Op("SBC"),
                vec![Operand::Def(acc), Operand::Use(acc), rhs[1].clone(), Operand::Def(Reg::Phys(reg::FLAGS))],
            );
            sbc.sets_flags = true;
            self.push(sbc);
        }
        if let Some((a1, b1)) = wide {
            if self.version(&a1.0) != a1.1 || self.version(&b1.0) != b1.1 {
                return Err(Unsupported("a compare's operand changed before its branch".into()));
            }
            let (al, ah) = self.read_word(&a1.0)?;
            let rhs = match self.word_constant(&b1.0) {
                Some(bytes) => bytes.map(Operand::Imm),
                None => {
                    let (bl, bh) = self.read_word(&b1.0)?;
                    for r in [bl, bh] {
                        self.require(r, Class::MEM)?;
                    }
                    [Operand::Use(bl), Operand::Use(bh)]
                }
            };
            for (lhs, rhs) in [al, ah].into_iter().zip(rhs) {
                self.copy(acc, lhs);
                let mut sbc = Inst::new(
                    Kind::Op("SBC"),
                    vec![Operand::Def(acc), Operand::Use(acc), rhs, Operand::Def(Reg::Phys(reg::FLAGS))],
                );
                sbc.sets_flags = true;
                self.push(sbc);
            }
        }
        Ok(())
    }

    fn block(
        &self,
        label: &Machine,
    ) -> Result<Operand, Unsupported> {
        match label {
            Machine::Label(l) => self
                .block_of
                .get(l)
                .map(|id| Operand::Block(*id))
                .ok_or_else(|| Unsupported(format!("a branch to {l}, outside the function"))),
            other => Err(Unsupported(format!("operand {other:?}"))),
        }
    }

    /// `jsr callee`: it reads the registers just copied into, and clobbers
    /// what the convention lets a routine clobber.
    fn call(
        &mut self,
        callee: &str,
    ) {
        let arguments = results(&self.function.blocks[self.current].insts);
        let mut jsr = Inst::new(
            Kind::Op("JSR"),
            std::iter::once(Operand::Symbol(callee.to_owned(), 0))
                .chain(arguments.into_iter().map(|r| Operand::Use(Reg::Phys(r))))
                .collect(),
        );
        jsr.call = true;
        self.push(jsr);
    }

    /// `jsr (pointer)`: the same, entering the callee through the pair that
    /// holds its address.
    fn call_through(
        &mut self,
        pointer: Reg,
    ) {
        let arguments = results(&self.function.blocks[self.current].insts);
        let mut jsr = Inst::new(
            Kind::Op("JSRI"),
            std::iter::once(Operand::Use(pointer))
                .chain(arguments.into_iter().map(|r| Operand::Use(Reg::Phys(r))))
                .collect(),
        );
        jsr.call = true;
        self.push(jsr);
    }

    fn push(
        &mut self,
        mut inst: Inst,
    ) {
        // a constant or a symbol's address is cheaper to load again than to
        // keep in a slot
        inst.rematerializable |= matches!(inst.kind, Kind::Op("LDI" | "LDS"));
        // and a load sets N and Z, so it is not run again where a compare's
        // flags are live
        if matches!(inst.kind, Kind::Op("LDI" | "LDS")) {
            inst.sets_flags = true;
        }
        if inst.origin == 0 {
            inst.origin = self.origin;
        }
        self.function.blocks[self.current].insts.push(inst);
    }

    fn copy(
        &mut self,
        to: Reg,
        from: Reg,
    ) {
        self.push(Inst::copy(to, from));
    }

    /// `d := a <mnemonic> b`, through the accumulator; the second operand is
    /// memory or an immediate.
    fn alu(
        &mut self,
        mnemonic: &'static str,
        carry: Option<&'static str>,
        d: &str,
        a: &str,
        b: &str,
    ) -> Result<(), Unsupported> {
        let second = match self.constants.get(b) {
            Some(value) => Operand::Imm(*value & 0xFF),
            None => {
                let b = self.byte(b)?;
                self.require(b, Class::MEM)?;
                Operand::Use(b)
            }
        };
        self.alu_with(mnemonic, carry, d, a, second)
    }

    /// `d := (a + b [+ 1]) >> 1` with the sum's ninth bit: `ADC` leaves it in
    /// C and `ROR` brings it in as bit 7. Nothing may sit between the two.
    fn average(
        &mut self,
        carry: &'static str,
        d: &str,
        a: &str,
        b: &str,
    ) -> Result<(), Unsupported> {
        let acc = Reg::Phys(reg::A);
        let second = match self.constants.get(b) {
            Some(value) => Operand::Imm(*value & 0xFF),
            None => {
                let b = self.byte(b)?;
                self.require(b, Class::MEM)?;
                Operand::Use(b)
            }
        };
        self.define(d);
        let (d, a) = (self.byte(d)?, self.byte(a)?);
        self.push(Inst::new(Kind::Op(carry), vec![]));
        self.copy(acc, a);
        self.push(Inst::new(Kind::Op("ADC"), vec![Operand::Def(acc), Operand::Use(acc), second]));
        self.push(Inst::new(Kind::Op("ROR"), vec![Operand::Def(acc), Operand::Use(acc)]));
        self.copy(d, acc);
        Ok(())
    }

    /// The same with the second operand made.
    fn alu_with(
        &mut self,
        mnemonic: &'static str,
        carry: Option<&'static str>,
        d: &str,
        a: &str,
        second: Operand,
    ) -> Result<(), Unsupported> {
        let acc = Reg::Phys(reg::A);
        self.define(d);
        let (d, a) = (self.byte(d)?, self.byte(a)?);
        if is_identity(mnemonic, &second) {
            self.copy(d, a);
            return Ok(());
        }
        if is_zeroing(mnemonic, &second) {
            self.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(d), Operand::Imm(0)]));
            return Ok(());
        }
        if let Some(carry) = carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        self.copy(acc, a);
        self.push(accumulate(mnemonic, second));
        self.copy(d, acc);
        Ok(())
    }

    /// `d := a op *(p + off)`: `ldy #off; op (p),y`. `y` is written even where
    /// a 65C02 prints `op (p)` for an offset of 0.
    fn alu_pointer(
        &mut self,
        mnemonic: &'static str,
        carry: Option<&'static str>,
        d: &str,
        a: &str,
        pointer: &str,
        off: i64,
    ) -> Result<(), Unsupported> {
        let (acc, y) = (Reg::Phys(reg::A), Reg::Phys(reg::Y));
        let ptr = self.pointer(pointer)?;
        self.require(ptr, Class::PAIR)?;
        self.define(d);
        let (d, a) = (self.byte(d)?, self.byte(a)?);
        if let Some(carry) = carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        self.copy(acc, a);
        let operands =
            vec![Operand::Def(acc), Operand::Use(acc), Operand::Use(ptr), Operand::Imm(off), Operand::Def(y)];
        self.push(Inst::new(Kind::Op(pointer_kind(mnemonic)), operands));
        self.copy(d, acc);
        Ok(())
    }

    /// A folded op whose first operand has a constant (half): the constant has
    /// its own forms (`x & 255`, `n - 1`, an `adc #0`), which an operand in
    /// memory would hide, so the load is made into a cell and the ordinary op
    /// runs on it.
    fn pointer_op_with_constant(
        &mut self,
        fold: &crate::pointer_alu::PointerAlu,
        at: usize,
        d: &str,
        a: &str,
        pointer: &str,
        place: Place,
    ) -> Result<(), Unsupported> {
        let (acc, y) = (Reg::Phys(reg::A), Reg::Phys(reg::Y));
        let ptr = self.pointer(pointer)?;
        self.require(ptr, Class::PAIR)?;
        // the loaded cell is the left operand, as the ordinary form has it,
        // except in a subtraction
        let sub = fold.mnemonic == "SBC";
        if fold.word {
            let cell = format!("w___p{at}");
            let Place::Offset(off) = place else { return Err(Unsupported("an indexed word".into())) };
            let (low, high) = self.write_word(&cell)?;
            for (lane, to) in [low, high].into_iter().enumerate() {
                let operands =
                    vec![Operand::Def(acc), Operand::Use(ptr), Operand::Imm(off + lane as i64), Operand::Def(y)];
                self.push(Inst::new(Kind::Op("LDPYZ"), operands));
                self.copy(to, acc);
            }
            let (left, right) = if sub { (a, cell.as_str()) } else { (cell.as_str(), a) };
            if fold.a == 1 && self.word_step(fold.mnemonic, d, left, right)? {
                return Ok(());
            }
            return self.alu16(fold.mnemonic, fold.carry, d, left, right);
        }
        let cell = format!("v___p{at}");
        self.define(&cell);
        let loaded = self.byte(&cell)?;
        match place {
            Place::Offset(off) => {
                let operands = vec![Operand::Def(acc), Operand::Use(ptr), Operand::Imm(off), Operand::Def(y)];
                self.push(Inst::new(Kind::Op("LDPYZ"), operands));
            }
            Place::Index(index) => {
                let index = self.byte(index)?;
                self.copy(y, index);
                self.push(Inst::new(Kind::Op("LDPX"), vec![Operand::Def(acc), Operand::Use(ptr), Operand::Use(y)]));
            }
        }
        self.copy(loaded, acc);
        // `*p + 1` and `*p - 1` step in place, as `INC8` and `DEC8` do
        let step = match self.constants.get(a).map(|value| *value & 0xFF) {
            Some(1) if fold.mnemonic == "ADC" && fold.a == 1 => Some("INC"),
            Some(255) if fold.mnemonic == "ADC" && fold.a == 1 => Some("DEC"),
            _ => None,
        };
        if let Some(step) = step {
            self.define(d);
            let d = self.byte(d)?;
            self.copy(d, loaded);
            self.push(Inst::new(Kind::Op(step), vec![Operand::Def(d), Operand::Use(d)]));
            return Ok(());
        }
        let (left, right) = if sub { (a, cell.as_str()) } else { (cell.as_str(), a) };
        self.alu(fold.mnemonic, fold.carry, d, left, right)
    }

    /// `d := a op *(p + index)`: the index byte goes to `y`, the op reads
    /// `(p),y`.
    fn alu_pointer_indexed(
        &mut self,
        mnemonic: &'static str,
        carry: Option<&'static str>,
        d: &str,
        a: &str,
        pointer: &str,
        index: &str,
    ) -> Result<(), Unsupported> {
        let (acc, y) = (Reg::Phys(reg::A), Reg::Phys(reg::Y));
        let ptr = self.pointer(pointer)?;
        self.require(ptr, Class::PAIR)?;
        self.define(d);
        let (d, a, index) = (self.byte(d)?, self.byte(a)?, self.byte(index)?);
        if let Some(carry) = carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        self.copy(y, index);
        self.copy(acc, a);
        let kind = Kind::Op(match mnemonic {
            "ADC" => "ADCPX",
            "SBC" => "SBCPX",
            "AND" => "ANDPX",
            "ORA" => "ORAPX",
            _ => "EORPX",
        });
        self.push(Inst::new(kind, vec![Operand::Def(acc), Operand::Use(acc), Operand::Use(ptr), Operand::Use(y)]));
        self.copy(d, acc);
        Ok(())
    }

    /// The same on a word: the carry runs from the low byte to the high, the
    /// high byte of the second operand being at `off + 1`.
    fn alu16_pointer(
        &mut self,
        fold: &crate::pointer_alu::PointerAlu,
        d: &str,
        a: &str,
        pointer: &str,
        off: i64,
    ) -> Result<(), Unsupported> {
        let (acc, y) = (Reg::Phys(reg::A), Reg::Phys(reg::Y));
        let ptr = self.pointer(pointer)?;
        self.require(ptr, Class::PAIR)?;
        let (al, ah) = self.read_word(a)?;
        let (dl, dh) = self.write_word(d)?;
        if let Some(carry) = fold.carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        for (lane, (to, from)) in [(dl, al), (dh, ah)].into_iter().enumerate() {
            self.copy(acc, from);
            let operands = vec![
                Operand::Def(acc),
                Operand::Use(acc),
                Operand::Use(ptr),
                Operand::Imm(off + lane as i64),
                Operand::Def(y),
            ];
            self.push(Inst::new(Kind::Op(pointer_kind(fold.mnemonic)), operands));
            self.copy(to, acc);
        }
        Ok(())
    }

    /// `d := a op g[i]` for a global indexed by a byte: x takes the index, the
    /// op reads `g,x`.
    fn alu_indexed(
        &mut self,
        mnemonic: &'static str,
        carry: Option<&'static str>,
        d: &str,
        a: &str,
        symbol: String,
        index: &str,
    ) -> Result<(), Unsupported> {
        let (acc, x) = (Reg::Phys(reg::A), Reg::Phys(reg::X));
        self.define(d);
        let (d, a, index) = (self.byte(d)?, self.byte(a)?, self.byte(index)?);
        self.copy(x, index);
        if let Some(carry) = carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        self.copy(acc, a);
        let second = Operand::Symbol(format!("{symbol},x"), 0);
        self.push(Inst::new(Kind::Op(mnemonic), vec![Operand::Def(acc), Operand::Use(acc), second, Operand::Use(x)]));
        self.copy(d, acc);
        Ok(())
    }

    /// How many halves of the pair `name` are the identity of `mnemonic` (a
    /// known `0`, or `255` for `AND`).
    fn identity_halves(
        &self,
        mnemonic: &str,
        name: &str,
    ) -> usize {
        let base = name.strip_suffix("+1").unwrap_or(name);
        if self.pointers.contains(base) {
            return 0;
        }
        [base.to_owned(), format!("{base}+1")]
            .iter()
            .filter(|cell| self.constants.get(*cell).is_some_and(|v| is_identity(mnemonic, &Operand::Imm(*v & 0xFF))))
            .count()
    }

    /// `d := a - 1` or `a + 1` on a word: the low byte steps, the high only
    /// when the low wrapped (`lda lo; bne; dec hi; dec lo`, 11 clocks against
    /// 22 for the carry chain; `inc lo; bne; inc hi`; a byte in x or y steps
    /// by inx/iny/dex/dey). Not for a pointer, which has its own pair bump.
    fn word_step(
        &mut self,
        mnemonic: &str,
        d: &str,
        a: &str,
        b: &str,
    ) -> Result<bool, Unsupported> {
        let up = match (mnemonic, self.word_constant(b)) {
            ("ADC", Some([1, 0])) => true,
            ("ADC", Some([255, 255])) | ("SBC", Some([1, 0])) => false,
            _ => return Ok(false),
        };
        if self.in_pair(d) || self.in_pair(a) {
            return Ok(false);
        }
        let (al, ah) = self.read_word(a)?;
        let (dl, dh) = self.write_word(d)?;
        self.copy(dl, al);
        self.copy(dh, ah);
        // `inc` has no accumulator form on the 6502 (`dec` loads through it)
        if up {
            for r in [dl, dh] {
                self.require(r, Class(Class::X | Class::Y | Class::M))?;
            }
        }
        let mut operands = vec![Operand::Def(dl), Operand::Def(dh), Operand::Use(dl), Operand::Use(dh)];
        if !up {
            // the `lda lo` that tests for the borrow: a dead value in `a`, a virtual register so
            // that the sink may move the step to the successor that reads it
            let scratch = self.fresh();
            self.require(scratch, Class(Class::A))?;
            operands.push(Operand::Def(scratch));
        }
        self.push(Inst::new(Kind::Op(if up { "INC16" } else { "DEC16" }), operands));
        Ok(true)
    }

    /// `d := a op g` for a word global: the op reads `g` for the low byte and
    /// `g+1` for the high, as a memory operand.
    fn alu16_mem(
        &mut self,
        mnemonic: &'static str,
        carry: Option<&'static str>,
        d: &str,
        a: &str,
        symbol: String,
    ) -> Result<(), Unsupported> {
        let (al, ah) = self.read_word(a)?;
        let second = [Operand::Symbol(symbol.clone(), 0), Operand::Symbol(format!("{symbol}+1"), 0)];
        let (dl, dh) = self.write_word(d)?;
        let acc = Reg::Phys(reg::A);
        if let Some(carry) = carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        for ((d, a), b) in [(dl, al), (dh, ah)].into_iter().zip(second) {
            self.copy(acc, a);
            self.push(accumulate(mnemonic, b));
            self.copy(d, acc);
        }
        Ok(())
    }

    /// The same on a pair, the carry running from the low byte to the high.
    fn alu16(
        &mut self,
        mnemonic: &'static str,
        carry: Option<&'static str>,
        d: &str,
        a: &str,
        b: &str,
    ) -> Result<(), Unsupported> {
        // a pointer plus a small constant stays a pair, bumped in place
        if mnemonic == "ADC"
            && self.in_pair(d)
            && self.in_pair(a)
            && let Some([k, 0]) = self.word_constant(b)
            && k > 0
        {
            let source = self.pointer(a)?;
            self.define(d);
            let dest = self.pointer(d)?;
            let mut operands = vec![Operand::Def(dest), Operand::Use(source), Operand::Imm(k)];
            if k != 1 {
                operands.push(Operand::Def(Reg::Phys(reg::A)));
            }
            let mut bump = Inst::new(Kind::Op("PADD"), operands);
            bump.ties = vec![(0, 1)];
            self.push(bump);
            return Ok(());
        }
        // a pointer minus a small constant: the low byte steps down, the high
        // on borrow
        if self.in_pair(d)
            && self.in_pair(a)
            && let Some(n) = match (mnemonic, self.word_constant(b)) {
                ("ADC", Some([lo, 255])) if lo > 0 => Some(256 - lo),
                ("SBC", Some([n, 0])) if n > 0 => Some(n),
                _ => None,
            }
        {
            let source = self.pointer(a)?;
            self.define(d);
            let dest = self.pointer(d)?;
            let operands =
                vec![Operand::Def(dest), Operand::Use(source), Operand::Imm(n), Operand::Def(Reg::Phys(reg::A))];
            let mut bump = Inst::new(Kind::Op("PSUB"), operands);
            bump.ties = vec![(0, 1)];
            self.push(bump);
            return Ok(());
        }
        // a pointer plus a page: the high byte steps, in place
        if mnemonic == "ADC"
            && self.in_pair(d)
            && self.in_pair(a)
            && let Some([0, 1]) = self.word_constant(b)
        {
            let source = self.pointer(a)?;
            self.define(d);
            let dest = self.pointer(d)?;
            let mut bump = Inst::new(Kind::Op("PINCH"), vec![Operand::Def(dest), Operand::Use(source)]);
            bump.ties = vec![(0, 1)];
            self.push(bump);
            return Ok(());
        }
        // a commutative op reads the operand with the known identity half as
        // its second
        let (a, b) = if matches!(mnemonic, "EOR" | "ORA" | "AND")
            && self.identity_halves(mnemonic, a) > self.identity_halves(mnemonic, b)
        {
            (b, a)
        } else {
            (a, b)
        };
        let (al, ah) = self.read_word(a)?;
        let second = match self.word_constant(b) {
            Some(bytes) => bytes.map(Operand::Imm),
            None => {
                let (bl, bh) = self.read_word(b)?;
                // a half known to be a constant (the zero of a widened byte) is
                // an immediate
                let bname = b.strip_suffix("+1").unwrap_or(b);
                let known = |halves: &HashMap<String, i64>, half: usize| {
                    let cell = if half == 0 { bname.to_owned() } else { format!("{bname}+1") };
                    halves.get(&cell).copied()
                };
                let mut second = [Operand::Use(bl), Operand::Use(bh)];
                for (half, register) in [bl, bh].into_iter().enumerate() {
                    match known(&self.constants, half).filter(|_| !self.pointers.contains(bname)) {
                        Some(value) => second[half] = Operand::Imm(value & 0xFF),
                        None => self.require(register, Class::MEM)?,
                    }
                }
                second
            }
        };
        let (dl, dh) = self.write_word(d)?;
        let acc = Reg::Phys(reg::A);
        let mut carried = false;
        // the low byte is left as it is, with the carry clear (set) for the
        // high
        let mut low_kept = false;
        for (half, ((d, a), b)) in [(dl, al), (dh, ah)].into_iter().zip(second).enumerate() {
            // the carry is clear (set) going in: adding (subtracting) 0 to the
            // low byte leaves it, and the high byte steps by the constant
            let spent = matches!((mnemonic, carry), ("ADC", Some("CLC")) | ("SBC", Some("SEC")));
            if half == 0 && spent && b == Operand::Imm(0) {
                low_kept = true;
            }
            if is_identity(mnemonic, &b) || low_kept && half == 0 {
                self.copy(d, a);
                continue;
            }
            if is_zeroing(mnemonic, &b) {
                self.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(d), Operand::Imm(0)]));
                continue;
            }
            if half == 1 && low_kept && b == Operand::Imm(1) {
                self.copy(d, a);
                self.push(Inst::new(
                    Kind::Op(if mnemonic == "ADC" { "INC" } else { "DEC" }),
                    vec![Operand::Def(d), Operand::Use(d)],
                ));
                continue;
            }
            if let Some(carry) = carry.filter(|_| !carried) {
                carried = true;
                self.push(Inst::new(Kind::Op(carry), vec![]));
            }
            self.copy(acc, a);
            self.push(accumulate(mnemonic, b));
            self.copy(d, acc);
        }
        Ok(())
    }
}

/// Whether the global a compare reads is the same wherever its flags are
/// read: no store, call or raw text lies between it and the end of its block,
/// and every flags cell it is saved to is loaded back inside that stretch. The
/// flags are made again where they are read, so the read must still see the
/// value the compare saw.
fn memory_holds_until_flags_are_read(
    items: &[Item],
    compare: usize,
) -> bool {
    let end = items[compare + 1..]
        .iter()
        .position(|item| matches!(item, Item::Label(_)))
        .map_or(items.len(), |at| compare + 1 + at);
    let mut saved = Vec::new();
    for item in &items[compare + 1..end] {
        let Item::Inst(inst) = item else { return false };
        let opcode = inst.opcode;
        if opcode == spill::SAVE_FLAGS {
            saved.extend(inst.operands.first());
            continue;
        }
        let (_, stores) = crate::dagisel::generated::instrinfo::memory_flags(opcode);
        if stores || matches!(opcode, op::JSR | op::JSRI | op::STZABS) {
            return false;
        }
    }
    items
        .iter()
        .enumerate()
        .all(
            |(at, item)| match item {
                Item::Inst(inst) if inst.opcode == spill::LOAD_FLAGS => {
                    !inst.operands.first().is_some_and(|cell| saved.contains(&cell)) || compare < at && at < end
                }
                _ => true,
            },
        )
}

/// `mnemonic a, a, second`: the byte op on the accumulator. `ADC`, `AND`, `ORA`
/// and `EOR` may swap their sources (`isCommutable`).
fn accumulate(
    mnemonic: &'static str,
    second: Operand,
) -> Inst {
    let acc = Reg::Phys(reg::A);
    let mut inst = Inst::new(Kind::Op(mnemonic), vec![Operand::Def(acc), Operand::Use(acc), second]);
    inst.commutable = matches!(mnemonic, "ADC" | "AND" | "ORA" | "EOR");
    inst
}

/// Where a folded op's operand lies from its pointer: a constant offset, or a
/// byte index register.
enum Place<'a> {
    Offset(i64),
    Index(&'a str),
}

/// The kind of an ALU op reading `(p),y`.
fn pointer_kind(mnemonic: &str) -> &'static str {
    match mnemonic {
        "ADC" => "ADCPY",
        "SBC" => "SBCPY",
        "AND" => "ANDPY",
        "ORA" => "ORAPY",
        _ => "EORPY",
    }
}

/// A logical op whose immediate leaves the byte as it was: `x ^ 0`, `x | 0`, `x
/// & 255`.
fn is_identity(
    mnemonic: &str,
    second: &Operand,
) -> bool {
    matches!(
        (mnemonic, second),
        ("EOR" | "ORA", Operand::Imm(0)) | ("AND", Operand::Imm(255))
    )
}

/// A logical op whose immediate makes the byte zero whatever it was: `x & 0`
/// (the high half of a zero-extended byte masked by 255 was `lda #0; and #0`).
fn is_zeroing(
    mnemonic: &str,
    second: &Operand,
) -> bool {
    matches!((mnemonic, second), ("AND", Operand::Imm(0)))
}

fn name(operand: &Machine) -> Result<&str, Unsupported> {
    match operand {
        Machine::Reg(name) => Ok(name),
        other => Err(Unsupported(format!("operand {other:?}"))),
    }
}

fn immediate(operand: &Machine) -> Result<i64, Unsupported> {
    match operand {
        Machine::Imm(value) => Ok(*value),
        other => Err(Unsupported(format!("operand {other:?}"))),
    }
}

/// The word cells that are a pointer somewhere: the base of a `(zp),y`
/// access, or the address of a frame object.
///
/// A pointer stays one through a copy and through an add of a constant
/// 1..=255 or -1..=-255 (`p++`, `p += k`, `--p`), in both directions, so a
/// pointer walked round a loop is bumped in its pair instead of unpacked, added
/// and packed every trip.
///
/// DEVIATE (6502-7): pointer-ness is worked out from how the cells are used,
/// and an add of a constant is found by following the cell that holds it; the
/// pin gets both from the register class a pattern constrains an operand to
/// and from an immediate-form pattern. Exit: the class constraint of the
/// greedy PR (#6259) and `ADD16` immediate forms in `mos6502.td`.
fn pointer_cells(
    items: &[Item],
    constants: &HashMap<String, i64>,
) -> HashSet<String> {
    let mut out = pointer_bases(items);
    let constant = |cell: &String| {
        let low = constants.get(cell).copied();
        let high = constants.get(&format!("{cell}+1")).copied();
        matches!(
            (low, high),
            (Some(1..=255), Some(0 | 255)) | (Some(0), Some(1))
        )
    };
    // a cell read or written as plain bytes anywhere stays a word cell
    let mut plain: HashSet<String> = HashSet::default();
    for item in items {
        let Item::Inst(MInst { opcode, operands, .. }) = item else { continue };
        for (at, operand) in operands.iter().enumerate() {
            let Machine::Reg(cell) = operand else { continue };
            let word = cell.strip_suffix("+1").unwrap_or(cell);
            let pointer_use = cell == word
                && match *opcode {
                    op::COPY16 | op::CMP16 | op::CMPZ16 => true,
                    op::ADD16 => matches!(&operands[2], Machine::Reg(b) if constant(b)),
                    op::LDP8 | op::LDP16 | op::STP8 | op::STP16 | op::LDPI8 | op::STPI8 | op::LDPZ | op::STPZ => {
                        at == 1
                    }
                    code if crate::pointer_alu::pointer_alu(code).is_some_and(|fold| at == fold.ptr) => true,
                    op::STZP | op::LEASP16 | op::JSRI => at == 0,
                    _ => false,
                };
            if !pointer_use {
                plain.insert(word.to_owned());
            }
        }
    }
    loop {
        let before = out.len();
        for item in items {
            let Item::Inst(MInst { opcode, operands, .. }) = item else { continue };
            let (d, a) = match (*opcode, &operands[..]) {
                (op::COPY16, [Machine::Reg(d), Machine::Reg(a)]) => (d, a),
                (op::ADD16, [Machine::Reg(d), Machine::Reg(a), Machine::Reg(b)]) if constant(b) => (d, a),
                _ => continue,
            };
            let pointer = |cell: &String| cell.starts_with("w_") && out.contains(cell);
            let tied = (pointer(d) || pointer(a))
                && [d, a].iter().all(|cell| cell.starts_with("w_") && (out.contains(*cell) || !plain.contains(*cell)));
            if tied {
                out.insert(d.clone());
                out.insert(a.clone());
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// The cells that are the base of a pointer access or an address.
fn pointer_bases(items: &[Item]) -> HashSet<String> {
    let mut out = HashSet::default();
    for item in items {
        let Item::Inst(MInst { opcode, operands, .. }) = item else { continue };
        let at = match *opcode {
            op::LDP8 | op::LDP16 | op::STP8 | op::STP16 | op::LDPI8 | op::STPI8 | op::LDPZ | op::STPZ => 1,
            code if crate::pointer_alu::pointer_alu(code).is_some() => {
                crate::pointer_alu::pointer_alu(code).expect("guarded").ptr
            }
            op::STZP | op::LEASP16 | op::JSRI => 0,
            _ => continue,
        };
        if let Some(Machine::Reg(cell)) = operands.get(at)
            && cell.starts_with("w_")
        {
            out.insert(cell.clone());
        }
    }
    out
}

/// The cells defined by exactly one `LDI8`, with their values.
fn constants(items: &[Item]) -> HashMap<String, i64> {
    let mut defs: HashMap<String, usize> = HashMap::default();
    let mut values = HashMap::default();
    for item in items {
        let Item::Inst(MInst { opcode, operands, .. }) = item else { continue };
        let Some(Machine::Reg(d)) = operands.first() else { continue };
        *defs.entry(d.clone()).or_default() += 1;
        if *opcode == op::LDI8
            && let Some(Machine::Imm(value)) = operands.get(1)
        {
            values.insert(d.clone(), *value);
        }
        // a shift by a byte or more leaves one half zero
        if let (op::SHL16 | op::SRL16, Some(Machine::Imm(8..16))) = (*opcode, operands.get(2)) {
            let zero = if *opcode == op::SHL16 { d.clone() } else { format!("{d}+1") };
            *defs.entry(zero.clone()).or_default() += usize::from(*opcode == op::SRL16);
            values.insert(zero, 0);
        }
        if *opcode == op::LDI16
            && let Some(Machine::Imm(value)) = operands.get(1)
        {
            let high = format!("{d}+1");
            *defs.entry(high.clone()).or_default() += 1;
            values.insert(d.clone(), value & 0xFF);
            values.insert(high, (value >> 8) & 0xFF);
        }
    }
    values.retain(|cell, _| {
        let word = cell.strip_suffix("+1").unwrap_or(cell);
        defs[cell] == 1 && defs[word] == 1 && (cell.starts_with("v_") || cell.starts_with("w_"))
    });
    // a word masked by a word whose high byte is zero has a zero high byte (the
    // mask 255 that zero-extends an any-extended byte)
    for item in items {
        let Item::Inst(MInst { opcode: op::AND16, operands, .. }) = item else { continue };
        if let [Machine::Reg(d), _, Machine::Reg(mask)] = &operands[..]
            && defs[d] == 1
            && (d.starts_with("v_") || d.starts_with("w_"))
            && values.get(&format!("{mask}+1")) == Some(&0)
        {
            values.insert(format!("{d}+1"), 0);
        }
    }
    // a byte copied once from a constant is that constant (the zero high half
    // of a widened byte)
    for item in items {
        let Item::Inst(MInst { opcode: op::COPY8, operands, .. }) = item else { continue };
        if let [Machine::Reg(d), Machine::Reg(source)] = &operands[..]
            && defs[d] == 1
            && (d.starts_with("v_") || d.starts_with("w_"))
            && let Some(value) = values.get(source).copied()
        {
            values.insert(d.clone(), value);
        }
    }
    values
}

/// `items` as a machine function on virtual registers, with the labels of each
/// block; or an error naming what is not built.
pub fn build(
    name_of_function: &str,
    items: &[Item],
) -> Result<(MFunction<Class>, Vec<Vec<String>>), Unsupported> {
    build_with_places(name_of_function, items, Vec::new())
}

/// [`build`], with the source places the items' origins number.
pub fn build_with_places(
    name_of_function: &str,
    items: &[Item],
    places: Vec<llrm_codegen::model::lir::SrcLoc>,
) -> Result<(MFunction<Class>, Vec<Vec<String>>), Unsupported> {
    // A block starts at a label that follows an instruction (or the first). The
    // entry block stays apart from a loop header that follows it directly: the
    // entry has no predecessors.
    let mut labels: Vec<Vec<String>> = vec![Vec::new()];
    let mut block_of = HashMap::default();
    let mut after_inst = false;
    for item in items {
        match item {
            Item::Label(l) => {
                if after_inst || labels.len() == 1 && !labels[0].is_empty() {
                    labels.push(Vec::new());
                    after_inst = false;
                }
                block_of.insert(l.clone(), labels.len() as u32 - 1);
                labels.last_mut().expect("a block").push(l.clone());
            }
            _ => after_inst = true,
        }
    }
    let mut b = Builder {
        function: MFunction { places, ..MFunction::new(name_of_function) },
        origin: 0,
        bytes: HashMap::default(),
        pointers: pointer_cells(items, &constants(items)),
        packs: Vec::new(),
        halves: HashMap::default(),
        constants: constants(items),
        block_of,
        versions: HashMap::default(),
        lost_halves: HashSet::default(),
        pending: None,
        compared: false,
        saved: HashMap::default(),
        current: 0,
        splats: HashMap::default(),
    };
    for id in 0..labels.len() {
        b.function.blocks.push(Block { id: id as u32, ..Block::default() });
    }
    let mut after_inst = false;
    let mut entry_labeled = false;
    for (at, item) in items.iter().enumerate() {
        let inst = match item {
            Item::Label(_) => {
                if after_inst || b.current == 0 && entry_labeled {
                    b.current += 1;
                    after_inst = false;
                    b.pending = None;
                }
                entry_labeled = true;
                continue;
            }
            // the flags are not kept across the phi copies: a compare is redone
            // at its branch
            Item::Text(text) if matches!(text.as_str(), "  php\n" | "  plp\n") => continue,
            Item::Text(_) => return Err(Unsupported("raw text".into())),
            Item::Inst(inst) => inst,
        };
        b.origin = inst.origin;
        after_inst = true;
        let o = &inst.operands;
        match inst.opcode {
            op::COPY8 => b.copy_byte(name(&o[0])?, name(&o[1])?)?,
            op::COPY16 => {
                let (d, src) = (name(&o[0])?, name(&o[1])?);
                if b.in_pair(d) && b.in_pair(src) {
                    b.define(d);
                    let (to, from) = (b.pointer(d)?, b.pointer(src)?);
                    b.copy(to, from);
                } else {
                    let (sl, sh) = b.read_word(src)?;
                    let (dl, dh) = b.write_word(d)?;
                    b.copy(dl, sl);
                    b.copy(dh, sh);
                }
            }
            op::LDI8 => {
                b.define(name(&o[0])?);
                let d = b.byte(name(&o[0])?)?;
                b.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(d), Operand::Imm(immediate(&o[1])? & 0xFF)]));
            }
            op::LDI16 if matches!(&o[1], Machine::Label(_)) => {
                let Machine::Label(symbol) = &o[1] else { unreachable!() };
                let (low, high) = b.write_word(name(&o[0])?)?;
                for (d, half) in [(low, 0), (high, 1)] {
                    b.push(Inst::new(Kind::Op("LDS"), vec![Operand::Def(d), Operand::Symbol(symbol.clone(), half)]));
                }
            }
            op::LDI16 => {
                let (low, high) = b.write_word(name(&o[0])?)?;
                let value = immediate(&o[1])?;
                for (d, byte) in [(low, value & 0xFF), (high, (value >> 8) & 0xFF)] {
                    b.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(d), Operand::Imm(byte)]));
                }
            }
            op::ADD8 => b.alu("ADC", Some("CLC"), name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::AVGFLOORU8 | op::AVGCEILU8 => {
                let carry = if inst.opcode == op::AVGFLOORU8 { "CLC" } else { "SEC" };
                b.average(carry, name(&o[0])?, name(&o[1])?, name(&o[2])?)?
            }
            op::ADD8M | op::SUB8M | op::AND8M | op::OR8M | op::XOR8M => {
                let (mnemonic, carry) = match inst.opcode {
                    op::ADD8M => ("ADC", Some("CLC")),
                    op::SUB8M => ("SBC", Some("SEC")),
                    op::AND8M => ("AND", None),
                    op::OR8M => ("ORA", None),
                    _ => ("EOR", None),
                };
                let symbol = address(&o[2], &o[3]).map_err(Unsupported)?;
                b.alu_with(mnemonic, carry, name(&o[0])?, name(&o[1])?, Operand::Symbol(symbol, 0))?
            }
            // a byte op reading `(p),y` (or the 65C02's `(p)`): the load folded
            // into it
            code if crate::pointer_alu::pointer_alu(code).is_some() => {
                let fold = crate::pointer_alu::pointer_alu(code).expect("guarded");
                let (d, a, pointer) = (name(&o[0])?, name(&o[fold.a])?, name(&o[fold.ptr])?);
                let constant_first = if fold.word { b.has_known_half(a) } else { b.constants.contains_key(a) };
                if constant_first {
                    let place = if fold.indexed {
                        Place::Index(name(&o[fold.ptr + 1])?)
                    } else {
                        Place::Offset(immediate(&o[fold.ptr + 1])?)
                    };
                    b.pointer_op_with_constant(&fold, at, d, a, pointer, place)?
                } else if fold.indexed {
                    b.alu_pointer_indexed(fold.mnemonic, fold.carry, d, a, pointer, name(&o[fold.ptr + 1])?)?
                } else if fold.word {
                    let offset = immediate(&o[fold.ptr + 1])?;
                    b.alu16_pointer(&fold, d, a, pointer, offset)?
                } else {
                    b.alu_pointer(fold.mnemonic, fold.carry, d, a, pointer, immediate(&o[fold.ptr + 1])?)?
                }
            }
            op::ADD8MX | op::SUB8MX | op::AND8MX | op::OR8MX | op::XOR8MX => {
                let (mnemonic, carry) = match inst.opcode {
                    op::ADD8MX => ("ADC", Some("CLC")),
                    op::SUB8MX => ("SBC", Some("SEC")),
                    op::AND8MX => ("AND", None),
                    op::OR8MX => ("ORA", None),
                    _ => ("EOR", None),
                };
                let symbol = indexed_address(&o[2], &o[3]).map_err(Unsupported)?;
                b.alu_indexed(mnemonic, carry, name(&o[0])?, name(&o[1])?, symbol, name(&o[4])?)?
            }
            op::SUB8 => b.alu("SBC", Some("SEC"), name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            // one register read and written: `d := a`, then `d` in place
            op::INC8 | op::DEC8 => {
                b.define(name(&o[0])?);
                let (d, a) = (b.byte(name(&o[0])?)?, b.byte(name(&o[1])?)?);
                let step = if inst.opcode == op::INC8 { "INC" } else { "DEC" };
                b.copy(d, a);
                b.push(Inst::new(Kind::Op(step), vec![Operand::Def(d), Operand::Use(d)]));
            }
            op::AND8 => b.alu("AND", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::OR8 => b.alu("ORA", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::XOR8 => b.alu("EOR", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::ADD16 => {
                let (d, a, c) = (name(&o[0])?, name(&o[1])?, name(&o[2])?);
                if !b.word_step("ADC", d, a, c)? {
                    b.alu16("ADC", Some("CLC"), d, a, c)?
                }
            }
            op::SUB16 => {
                let (d, a, c) = (name(&o[0])?, name(&o[1])?, name(&o[2])?);
                if !b.word_step("SBC", d, a, c)? {
                    b.alu16("SBC", Some("SEC"), d, a, c)?
                }
            }
            op::AND16 => b.alu16("AND", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::OR16 => b.alu16("ORA", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::XOR16 => b.alu16("EOR", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::ADD16M | op::SUB16M | op::AND16M | op::OR16M | op::XOR16M => {
                let (mnemonic, carry) = match inst.opcode {
                    op::ADD16M => ("ADC", Some("CLC")),
                    op::SUB16M => ("SBC", Some("SEC")),
                    op::AND16M => ("AND", None),
                    op::OR16M => ("ORA", None),
                    _ => ("EOR", None),
                };
                let symbol = address(&o[2], &o[3]).map_err(Unsupported)?;
                let (d, a) = (name(&o[0])?, name(&o[1])?);
                if !b.has_known_half(a) {
                    b.alu16_mem(mnemonic, carry, d, a, symbol)?
                } else {
                    // a constant half on the left has its own forms (`n - 1`
                    // borrows by branch, a mask keeps the
                    // halves it leaves): the global
                    // is loaded into a cell and the ordinary word op runs
                    let cell = format!("w___m{at}");
                    let acc = Reg::Phys(reg::A);
                    let (low, high) = b.write_word(&cell)?;
                    for (r, half) in [low, high].into_iter().zip([symbol.clone(), format!("{symbol}+1")]) {
                        b.push(Inst::new(Kind::Op("LDM"), vec![Operand::Def(acc), Operand::Symbol(half, 0)]));
                        b.copy(r, acc);
                    }
                    // the loaded cell is the left operand, as the ordinary form
                    // has it, except in a subtraction,
                    // where the load was the right one
                    let (left, right) = if inst.opcode == op::SUB16M { (a, cell.as_str()) } else { (cell.as_str(), a) };
                    if !matches!(inst.opcode, op::ADD16M | op::SUB16M) || !b.word_step(mnemonic, d, left, right)? {
                        b.alu16(mnemonic, carry, d, left, right)?
                    }
                }
            }
            op::SHL8 | op::SRL8 | op::SRA8 => {
                let step = match inst.opcode {
                    op::SHL8 => "ASL",
                    op::SRL8 => "LSR",
                    _ => "ASR",
                };
                let acc = Reg::Phys(reg::A);
                let count = immediate(&o[2])?;
                // the sign of a sign is itself
                let splat = step == "ASR" && count == 7;
                let held = if splat { b.splat_of(name(&o[1])?) } else { None };
                let held = held.filter(|held| held != name(&o[0]).unwrap_or(""));
                let held = held.map(|held| b.byte(&held)).transpose()?;
                b.define(name(&o[0])?);
                let (d, a) = (b.byte(name(&o[0])?)?, b.byte(name(&o[1])?)?);
                match held {
                    Some(held) => b.copy(acc, held),
                    None => {
                        b.copy(acc, a);
                        b.shift_a(step, count);
                    }
                }
                b.copy(d, acc);
                if splat {
                    b.note_splat(name(&o[1])?, name(&o[0])?);
                    b.note_splat(name(&o[0])?, name(&o[0])?);
                }
            }
            // `lsr a` leaves the bit that left in the carry; the operands
            // are `d`, `co`, `a`
            op::SRL1C8 => {
                let acc = Reg::Phys(reg::A);
                b.define(name(&o[0])?);
                let (d, a) = (b.byte(name(&o[0])?)?, b.byte(name(&o[2])?)?);
                b.copy(acc, a);
                b.shift_a("LSR", 1);
                b.copy(d, acc);
            }
            op::STZABS => {
                let symbol = address(&o[0], &o[1]).map_err(Unsupported)?;
                b.push(Inst::new(Kind::Op("STZM"), vec![Operand::Symbol(symbol, 0)]));
            }
            op::LDABS | op::STABS => {
                let symbol = address(&o[1], &o[2]).map_err(Unsupported)?;
                let acc = Reg::Phys(reg::A);
                if inst.opcode == op::LDABS {
                    b.define(name(&o[0])?);
                    let d = b.byte(name(&o[0])?)?;
                    b.push(Inst::new(Kind::Op("LDM"), vec![Operand::Def(acc), Operand::Symbol(symbol, 0)]));
                    b.copy(d, acc);
                } else {
                    let value = b.byte(name(&o[0])?)?;
                    b.copy(acc, value);
                    b.push(Inst::new(Kind::Op("STM"), vec![Operand::Use(acc), Operand::Symbol(symbol, 0)]));
                }
            }
            // a word global: its two halves, g and g+1
            op::LDABS16 | op::STABS16 => {
                let symbol = address(&o[1], &o[2]).map_err(Unsupported)?;
                let acc = Reg::Phys(reg::A);
                let halves = [symbol.clone(), format!("{symbol}+1")];
                if inst.opcode == op::LDABS16 {
                    let (low, high) = b.write_word(name(&o[0])?)?;
                    for (d, half) in [low, high].into_iter().zip(halves) {
                        b.push(Inst::new(Kind::Op("LDM"), vec![Operand::Def(acc), Operand::Symbol(half, 0)]));
                        b.copy(d, acc);
                    }
                } else {
                    let (low, high) = b.read_word(name(&o[0])?)?;
                    for (value, half) in [low, high].into_iter().zip(halves) {
                        b.copy(acc, value);
                        b.push(Inst::new(Kind::Op("STM"), vec![Operand::Use(acc), Operand::Symbol(half, 0)]));
                    }
                }
            }
            op::LDAX | op::STAX => {
                let symbol = indexed_address(&o[1], &o[2]).map_err(Unsupported)?;
                let (acc, x) = (Reg::Phys(reg::A), Reg::Phys(reg::X));
                let index = b.byte(name(&o[3])?)?;
                b.copy(x, index);
                if inst.opcode == op::LDAX {
                    b.define(name(&o[0])?);
                    let d = b.byte(name(&o[0])?)?;
                    b.push(Inst::new(
                        Kind::Op("LDAX"),
                        vec![Operand::Def(acc), Operand::Symbol(symbol.clone(), 0), Operand::Use(x)],
                    ));
                    b.copy(d, acc);
                } else {
                    let value = b.byte(name(&o[0])?)?;
                    b.copy(acc, value);
                    b.push(Inst::new(
                        Kind::Op("STAX"),
                        vec![Operand::Use(acc), Operand::Symbol(symbol.clone(), 0), Operand::Use(x)],
                    ));
                }
            }
            // The flags are made where they are read.
            op::CMP8 => {
                let (a, c) = (name(&o[1])?, name(&o[2])?);
                b.pending = Some(Compare {
                    a: (a.to_owned(), b.version(a)),
                    b: (c.to_owned(), b.version(c)),
                    word: false,
                    zero: false,
                    memory: None,
                    index: None,
                    wide: None,
                });
            }
            // a global on the right of a compare: read at the compare, kept
            // as a memory operand only while it cannot change before the
            // flags are last read
            op::CMP8M | op::CMP16M => {
                let (a, symbol) = (name(&o[1])?, address(&o[2], &o[3]).map_err(Unsupported)?);
                b.pending = Some(Compare {
                    a: (a.to_owned(), b.version(a)),
                    b: (String::new(), 0),
                    word: inst.opcode == op::CMP16M,
                    zero: false,
                    memory: Some(symbol),
                    index: None,
                    wide: None,
                });
                if !memory_holds_until_flags_are_read(items, at) {
                    b.load_compared_global(at)?;
                }
            }
            // a global indexed by a byte on the right: the same, with the index
            // kept in `x`
            op::CMP8MX => {
                let (a, symbol) = (name(&o[1])?, indexed_address(&o[2], &o[3]).map_err(Unsupported)?);
                let index = name(&o[4])?;
                b.pending = Some(Compare {
                    a: (a.to_owned(), b.version(a)),
                    b: (String::new(), 0),
                    word: false,
                    zero: false,
                    memory: Some(symbol),
                    index: Some((index.to_owned(), b.version(index))),
                    wide: None,
                });
                if !memory_holds_until_flags_are_read(items, at) {
                    b.load_compared_global(at)?;
                }
            }
            op::CMPZ16 => {
                let (a, c) = (name(&o[1])?, name(&o[2])?);
                b.pending = Some(Compare {
                    a: (a.to_owned(), b.version(a)),
                    b: (c.to_owned(), b.version(c)),
                    word: true,
                    zero: true,
                    memory: None,
                    index: None,
                    wide: None,
                });
            }
            // the carry out of the first goes into the next: the instructions
            // between move bytes, which leave C alone
            op::ADDO8 => b.alu("ADC", Some("CLC"), name(&o[0])?, name(&o[2])?, name(&o[3])?)?,
            op::SUBO8 => b.alu("SBC", Some("SEC"), name(&o[0])?, name(&o[2])?, name(&o[3])?)?,
            op::ADDOC8 => b.alu("ADC", None, name(&o[0])?, name(&o[2])?, name(&o[3])?)?,
            op::SUBOC8 => b.alu("SBC", None, name(&o[0])?, name(&o[2])?, name(&o[3])?)?,
            op::ADDO16 => b.alu16("ADC", Some("CLC"), name(&o[0])?, name(&o[2])?, name(&o[3])?)?,
            op::SUBO16 => b.alu16("SBC", Some("SEC"), name(&o[0])?, name(&o[2])?, name(&o[3])?)?,
            op::ADDOC16 => b.alu16("ADC", None, name(&o[0])?, name(&o[2])?, name(&o[3])?)?,
            op::SUBOC16 => b.alu16("SBC", None, name(&o[0])?, name(&o[2])?, name(&o[3])?)?,
            code if code == spill::UNREACHABLE => {
                let mut end = Inst::new(Kind::Op("UNREACHABLE"), Vec::new());
                end.terminator = true;
                end.barrier = true;
                b.push(end);
            }
            // C into a byte and back: `lda #0; rol a` and `lsr a`
            code if code == spill::SAVE_CARRY => {
                b.define(name(&o[0])?);
                let d = b.byte(name(&o[0])?)?;
                let acc = Reg::Phys(reg::A);
                b.push(Inst::new(Kind::Op("SAVEC"), vec![Operand::Def(acc)]));
                b.copy(d, acc);
            }
            code if code == spill::LOAD_CARRY => {
                let s = b.byte(name(&o[0])?)?;
                let acc = Reg::Phys(reg::A);
                b.copy(acc, s);
                b.push(Inst::new(Kind::Op("LOADC"), vec![Operand::Def(acc), Operand::Use(acc)]));
            }
            // 8 or more: one byte moves across, shifted by the rest of the
            // count
            code @ (op::SHL16 | op::SRL16 | op::SRA16) if (8..16).contains(&immediate(&o[2])?) => {
                let rest = immediate(&o[2])? - 8;
                let (source, word) = (name(&o[1])?, name(&o[0])?);
                let source = source.strip_suffix("+1").unwrap_or(source);
                let top = format!("{source}+1");
                // the sign of the high byte, if some cell holds it already
                let held = (!b.pointers.contains(source) && !b.pointers.contains(word))
                    .then(|| b.splat_of(&top))
                    .flatten()
                    .filter(|held| held != word && *held != format!("{word}+1"));
                let held = held.map(|held| b.byte(&held)).transpose()?;
                let (al, ah) = b.read_word(name(&o[1])?)?;
                let (dl, dh) = b.write_word(name(&o[0])?)?;
                let acc = Reg::Phys(reg::A);
                let shift = |b: &mut Builder, from: Reg, to: Reg, step: &'static str, count: i64| {
                    b.copy(acc, from);
                    b.shift_a(step, count);
                    b.copy(to, acc);
                };
                match code {
                    op::SHL16 => {
                        shift(&mut b, al, dh, "ASL", rest);
                        b.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(dl), Operand::Imm(0)]));
                    }
                    op::SRL16 => {
                        shift(&mut b, ah, dl, "LSR", rest);
                        b.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(dh), Operand::Imm(0)]));
                    }
                    // the high byte is the sign of `ah`; by 15 the low byte is
                    // too, and one splat is stored twice
                    _ => {
                        if rest != 7 {
                            shift(&mut b, ah, dl, "ASR", rest);
                        }
                        match held {
                            Some(held) => b.copy(acc, held),
                            None => {
                                b.copy(acc, ah);
                                b.shift_a("ASR", 7);
                            }
                        }
                        if rest == 7 {
                            b.copy(dl, acc);
                        }
                        b.copy(dh, acc);
                        if !b.pointers.contains(word) {
                            b.note_splat(&top, &format!("{word}+1"));
                            b.note_splat(&format!("{word}+1"), &format!("{word}+1"));
                            if rest == 7 {
                                b.note_splat(word, word);
                            }
                        }
                    }
                }
            }
            // in place on zero-page bytes: asl/rol, lsr/ror
            code @ (op::SHL16 | op::SRL16 | op::SRA16) => {
                let (al, ah) = b.read_word(name(&o[1])?)?;
                let (dl, dh) = b.write_word(name(&o[0])?)?;
                b.copy(dl, al);
                b.copy(dh, ah);
                for r in [dl, dh] {
                    b.require(r, Class::MEM)?;
                }
                for _ in 0..immediate(&o[2])? {
                    let step = match code {
                        op::SHL16 => Inst::new(
                            Kind::Op("SHL1"),
                            vec![Operand::Def(dl), Operand::Use(dl), Operand::Def(dh), Operand::Use(dh)],
                        ),
                        op::SRL16 => Inst::new(
                            Kind::Op("SRL1"),
                            vec![Operand::Def(dl), Operand::Use(dl), Operand::Def(dh), Operand::Use(dh)],
                        ),
                        _ => Inst::new(
                            Kind::Op("SRA1"),
                            vec![
                                Operand::Def(Reg::Phys(reg::A)),
                                Operand::Def(dl),
                                Operand::Use(dl),
                                Operand::Def(dh),
                                Operand::Use(dh),
                            ],
                        ),
                    };
                    b.push(step);
                }
            }
            // one bit crosses between two words: the carry of a shift of the
            // word that gives it up, rotated into the word that takes it
            code @ (op::FSHL1 | op::FSHR1) => {
                let (al, ah) = b.read_word(name(&o[1])?)?;
                let (bl, bh) = b.read_word(name(&o[2])?)?;
                let (dl, dh) = b.write_word(name(&o[0])?)?;
                let acc = Reg::Phys(reg::A);
                let (kind, kept, gives, first, second) =
                    if code == op::FSHL1 { ("FSHL1", (al, ah), bh, dl, dh) } else { ("FSHR1", (bl, bh), al, dh, dl) };
                b.copy(dl, kept.0);
                b.copy(dh, kept.1);
                for r in [dl, dh] {
                    b.require(r, Class::MEM)?;
                }
                b.push(Inst::new(
                    Kind::Op(kind),
                    vec![
                        Operand::Def(acc),
                        Operand::Use(gives),
                        Operand::Def(first),
                        Operand::Use(first),
                        Operand::Def(second),
                        Operand::Use(second),
                    ],
                ));
            }
            // 32 bits through `__sa` and `__sb` (runtime/mos_rt.s)
            code @ (op::MUL32
            | op::UDIV32
            | op::SDIV32
            | op::UREM32
            | op::SREM32
            | op::SHL32
            | op::SRL32
            | op::SRA32) => {
                let routine = match code {
                    op::MUL32 => "__mulsi3",
                    op::UDIV32 => "__udivsi3",
                    op::SDIV32 => "__divsi3",
                    op::UREM32 => "__umodsi3",
                    op::SREM32 => "__modsi3",
                    op::SHL32 => "__ashlsi3",
                    op::SRL32 => "__lshrsi3",
                    _ => "__ashrsi3",
                };
                // a divisor whose three upper bytes are known zero is a byte
                // (1 to 255): the speed division has an entry that links the
                // byte loop and not the wide path
                let byte_divisor = matches!(code, op::UDIV32 | op::SDIV32 | op::UREM32 | op::SREM32) && {
                    let (low, high) = (name(&o[4])?, name(&o[5])?);
                    [format!("{low}+1"), high.to_owned(), format!("{high}+1")]
                        .iter()
                        .all(|cell| b.constants.get(cell) == Some(&0))
                };
                let routine = if byte_divisor { format!("{routine}_byte") } else { routine.to_owned() };
                let acc = Reg::Phys(reg::A);
                let mut stores = Vec::new();
                for (at, source) in [name(&o[2])?, name(&o[3])?].into_iter().enumerate() {
                    let (low, high) = b.read_word(source)?;
                    stores.push((format!("__sa+{}", 2 * at), low));
                    stores.push((format!("__sa+{}", 2 * at + 1), high));
                }
                if matches!(code, op::SHL32 | op::SRL32 | op::SRA32) {
                    stores.push(("__sb".to_owned(), b.byte(name(&o[4])?)?));
                } else {
                    for (at, source) in [name(&o[4])?, name(&o[5])?].into_iter().enumerate() {
                        let (low, high) = b.read_word(source)?;
                        stores.push((format!("__sb+{}", 2 * at), low));
                        stores.push((format!("__sb+{}", 2 * at + 1), high));
                    }
                }
                for (symbol, value) in stores {
                    b.copy(acc, value);
                    b.push(Inst::new(Kind::Op("STM"), vec![Operand::Use(acc), Operand::Symbol(symbol, 0)]));
                }
                let mut jsr = Inst::new(Kind::Op("JSR"), vec![Operand::Symbol(routine, 0)]);
                jsr.call = true;
                b.push(jsr);
                let (first, second) = (b.write_word(name(&o[0])?)?, b.write_word(name(&o[1])?)?);
                for (n, to) in [first.0, first.1, second.0, second.1].into_iter().enumerate() {
                    let symbol = if n == 0 { "__sa".to_owned() } else { format!("__sa+{n}") };
                    b.push(Inst::new(Kind::Op("LDM"), vec![Operand::Def(acc), Operand::Symbol(symbol, 0)]));
                    b.copy(to, acc);
                }
            }
            // a widening multiply (`__umulhisi3`, `__mulhisi3`): the operands'
            // words in `__sa` and `__sb`, the product in `__sa`
            code @ (op::UMULW16 | op::SMULW16) => {
                // a multiplier whose high byte is known zero is a byte: the
                // entry that walks to its last set
                // bit links only the byte loop
                let byte = b.constants.get(&format!("{}+1", name(&o[3])?)) == Some(&0);
                let routine = match (code == op::UMULW16, byte) {
                    (true, true) => "__umulhisi3_byte",
                    (true, false) => "__umulhisi3",
                    (false, true) => "__mulhisi3_byte",
                    (false, false) => "__mulhisi3",
                };
                let acc = Reg::Phys(reg::A);
                for (to, source) in [("__sa", name(&o[2])?), ("__sb", name(&o[3])?)] {
                    let (low, high) = b.read_word(source)?;
                    for (symbol, value) in [(to.to_owned(), low), (format!("{to}+1"), high)] {
                        b.copy(acc, value);
                        b.push(Inst::new(Kind::Op("STM"), vec![Operand::Use(acc), Operand::Symbol(symbol, 0)]));
                    }
                }
                let mut jsr = Inst::new(Kind::Op("JSR"), vec![Operand::Symbol(routine.to_owned(), 0)]);
                jsr.call = true;
                b.push(jsr);
                let (first, second) = (b.write_word(name(&o[0])?)?, b.write_word(name(&o[1])?)?);
                for (n, to) in [first.0, first.1, second.0, second.1].into_iter().enumerate() {
                    let symbol = if n == 0 { "__sa".to_owned() } else { format!("__sa+{n}") };
                    b.push(Inst::new(Kind::Op("LDM"), vec![Operand::Def(acc), Operand::Symbol(symbol, 0)]));
                    b.copy(to, acc);
                }
            }
            op::CMP32 => {
                let (a, c) = (name(&o[1])?, name(&o[2])?);
                let (a1, c1) = (name(&o[3])?, name(&o[4])?);
                b.pending = Some(Compare {
                    a: (a.to_owned(), b.version(a)),
                    b: (c.to_owned(), b.version(c)),
                    word: true,
                    zero: false,
                    memory: None,
                    index: None,
                    wide: Some(((a1.to_owned(), b.version(a1)), (c1.to_owned(), b.version(c1)))),
                });
            }
            op::CMP16 => {
                let (a, c) = (name(&o[1])?, name(&o[2])?);
                b.pending = Some(Compare {
                    a: (a.to_owned(), b.version(a)),
                    b: (c.to_owned(), b.version(c)),
                    word: true,
                    zero: false,
                    memory: None,
                    index: None,
                    wide: None,
                });
            }
            spill_op if spill_op == spill::SAVE_FLAGS => {
                let compare = b.pending.clone().ok_or_else(|| Unsupported("saved flags with no compare".into()))?;
                b.saved.insert(name(&o[0])?.to_owned(), compare);
            }
            spill_op if spill_op == spill::LOAD_FLAGS => {
                b.pending = Some(
                    b.saved.get(name(&o[0])?).cloned().ok_or_else(|| Unsupported("loaded flags never saved".into()))?,
                );
            }
            spill_op if spill_op == spill::COMPARE => {
                b.redo_compare(immediate(&o[0])?, true)?;
                b.compared = true;
            }
            code @ (op::BEQ | op::BNE | op::BCC | op::BCS | op::BMI | op::BPL) => {
                let condition = [op::BEQ, op::BNE, op::BCC, op::BCS, op::BMI, op::BPL].iter().position(|c| *c == code);
                if !std::mem::take(&mut b.compared) {
                    b.redo_compare(condition.map_or(-1, |c| c as i64), true)?;
                }
                let mnemonic = match code {
                    op::BEQ => "BEQ",
                    op::BNE => "BNE",
                    op::BCC => "BCC",
                    op::BCS => "BCS",
                    op::BMI => "BMI",
                    _ => "BPL",
                };
                let target = b.block(&o[0])?;
                let mut branch = Inst::new(Kind::Op(mnemonic), vec![target, Operand::Use(Reg::Phys(reg::FLAGS))]);
                branch.terminator = true;
                branch.reads_flags = true;
                b.push(branch);
            }
            op::JMP | op::BRA => {
                let target = b.block(&o[0])?;
                let mut jump = Inst::new(Kind::Op(if inst.opcode == op::BRA { "BRA" } else { "JMP" }), vec![target]);
                jump.terminator = true;
                jump.barrier = true;
                b.push(jump);
            }
            // `ldy #off; lda (p),y` is one instruction, so spill code for the
            // pointer cannot land between the two and clobber `y`
            op::LDP8 | op::LDP16 => {
                let ptr = b.pointer(name(&o[1])?)?;
                b.require(ptr, Class::PAIR)?;
                let (acc, y) = (Reg::Phys(reg::A), Reg::Phys(reg::Y));
                let wide = inst.opcode == op::LDP16;
                let off = immediate(&o[2])?;
                let (dl, dh) = if wide {
                    b.write_word(name(&o[0])?)?
                } else {
                    b.define(name(&o[0])?);
                    (b.byte(name(&o[0])?)?, Reg::Phys(reg::A))
                };
                for (part, to) in [(0, dl), (1, dh)].into_iter().take(if wide { 2 } else { 1 }) {
                    b.push(Inst::new(
                        Kind::Op("LDPY"),
                        vec![Operand::Def(acc), Operand::Use(ptr), Operand::Imm(off + part), Operand::Def(y)],
                    ));
                    b.copy(to, acc);
                }
            }
            // 65C02: `lda (p)` / `sta (p)`, no `y`
            op::LDPZ => {
                let ptr = b.pointer(name(&o[1])?)?;
                b.require(ptr, Class::PAIR)?;
                b.define(name(&o[0])?);
                let d = b.byte(name(&o[0])?)?;
                let acc = Reg::Phys(reg::A);
                b.push(Inst::new(Kind::Op("LDP0"), vec![Operand::Def(acc), Operand::Use(ptr)]));
                b.copy(d, acc);
            }
            op::STPZ => {
                let ptr = b.pointer(name(&o[1])?)?;
                b.require(ptr, Class::PAIR)?;
                let value = b.byte(name(&o[0])?)?;
                let acc = Reg::Phys(reg::A);
                b.copy(acc, value);
                b.push(Inst::new(Kind::Op("STP0"), vec![Operand::Use(acc), Operand::Use(ptr)]));
            }
            op::STP8 | op::STP16 => {
                let ptr = b.pointer(name(&o[1])?)?;
                b.require(ptr, Class::PAIR)?;
                let (acc, y) = (Reg::Phys(reg::A), Reg::Phys(reg::Y));
                let wide = inst.opcode == op::STP16;
                let off = immediate(&o[2])?;
                let (sl, sh) =
                    if wide { b.read_word(name(&o[0])?)? } else { (b.byte(name(&o[0])?)?, Reg::Phys(reg::A)) };
                for (part, from) in [(0, sl), (1, sh)].into_iter().take(if wide { 2 } else { 1 }) {
                    b.copy(acc, from);
                    b.push(Inst::new(
                        Kind::Op("STPY"),
                        vec![Operand::Use(acc), Operand::Use(ptr), Operand::Imm(off + part), Operand::Def(y)],
                    ));
                }
            }
            // `lda (p),y` with the index byte in `y`
            op::LDPI8 => {
                let ptr = b.pointer(name(&o[1])?)?;
                b.require(ptr, Class::PAIR)?;
                let index = b.byte(name(&o[2])?)?;
                b.define(name(&o[0])?);
                let d = b.byte(name(&o[0])?)?;
                let (acc, y) = (Reg::Phys(reg::A), Reg::Phys(reg::Y));
                b.copy(y, index);
                b.push(Inst::new(Kind::Op("LDPX"), vec![Operand::Def(acc), Operand::Use(ptr), Operand::Use(y)]));
                b.copy(d, acc);
            }
            op::STPI8 => {
                let ptr = b.pointer(name(&o[1])?)?;
                b.require(ptr, Class::PAIR)?;
                let index = b.byte(name(&o[2])?)?;
                let value = b.byte(name(&o[0])?)?;
                let (acc, y) = (Reg::Phys(reg::A), Reg::Phys(reg::Y));
                b.copy(acc, value);
                b.copy(y, index);
                b.push(Inst::new(Kind::Op("STPX"), vec![Operand::Use(acc), Operand::Use(ptr), Operand::Use(y)]));
            }
            op::STZP => {
                let ptr = b.pointer(name(&o[0])?)?;
                b.require(ptr, Class::PAIR)?;
                let (acc, y) = (Reg::Phys(reg::A), Reg::Phys(reg::Y));
                let off = immediate(&o[1])?;
                b.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(acc), Operand::Imm(0)]));
                if off == 0 {
                    b.push(Inst::new(Kind::Op("STP0"), vec![Operand::Use(acc), Operand::Use(ptr)]));
                } else {
                    b.push(Inst::new(
                        Kind::Op("STPY"),
                        vec![Operand::Use(acc), Operand::Use(ptr), Operand::Imm(off), Operand::Def(y)],
                    ));
                }
            }
            // d := rs0 + n
            op::LEASP16 => {
                let (low, high) = b.write_word(name(&o[0])?)?;
                let n = immediate(&o[1])? as u16;
                b.push(Inst::new(
                    Kind::Op("LEA"),
                    vec![
                        Operand::Def(low),
                        Operand::Def(high),
                        Operand::Def(Reg::Phys(reg::A)),
                        Operand::Imm(i64::from(n)),
                    ],
                ));
            }
            // rs0 := rs0 + n
            op::ADJSP => {
                let n = immediate(&o[0])? as u16;
                b.push(Inst::new(Kind::Op("ADJSP"), vec![Operand::Def(Reg::Phys(reg::A)), Operand::Imm(i64::from(n))]));
            }
            op::JSR => {
                let Machine::Label(callee) = &o[0] else { return Err(Unsupported("a call to a non-label".into())) };
                b.call(callee);
            }
            op::JSRI => {
                let pointer = b.pointer(name(&o[0])?)?;
                b.require(pointer, Class::PAIR)?;
                b.call_through(pointer);
            }
            code @ (op::MUL8 | op::SDIV8 | op::SREM8 | op::SHLV8 | op::SRLV8 | op::SRAV8) => {
                let routine = match code {
                    op::MUL8 => "__mulqi3",
                    op::SDIV8 => "__divqi3",
                    op::SREM8 => "__modqi3",
                    op::SHLV8 => "__ashlqi3",
                    op::SRLV8 => "__lshrqi3",
                    _ => "__ashrqi3",
                };
                b.define(name(&o[0])?);
                let (d, x, y) = (b.byte(name(&o[0])?)?, b.byte(name(&o[1])?)?, b.byte(name(&o[2])?)?);
                b.copy(Reg::Phys(rc(2)), x);
                b.copy(Reg::Phys(rc(3)), y);
                b.call(routine);
                b.copy(d, Reg::Phys(reg::A));
            }
            // the quotient comes back in A (and X), the remainder in rc6 (and
            // rc7): see `runtime/mos_rt.s`
            op::UDIVREM8 => {
                b.define(name(&o[0])?);
                b.define(name(&o[1])?);
                let (q, r) = (b.byte(name(&o[0])?)?, b.byte(name(&o[1])?)?);
                let (x, y) = (b.byte(name(&o[2])?)?, b.byte(name(&o[3])?)?);
                b.copy(Reg::Phys(rc(2)), x);
                b.copy(Reg::Phys(rc(3)), y);
                b.call("__udivmodqi4");
                b.copy(q, Reg::Phys(reg::A));
                b.copy(r, Reg::Phys(rc(6)));
            }
            op::UDIVREM16 => {
                let (xl, xh) = b.read_word(name(&o[2])?)?;
                let (yl, yh) = b.read_word(name(&o[3])?)?;
                let (ql, qh) = b.write_word(name(&o[0])?)?;
                let (rl, rh) = b.write_word(name(&o[1])?)?;
                b.copy(Reg::Phys(rc(2)), xl);
                b.copy(Reg::Phys(rc(3)), xh);
                b.copy(Reg::Phys(rc(4)), yl);
                b.copy(Reg::Phys(rc(5)), yh);
                b.call("__udivhi3");
                b.copy(ql, Reg::Phys(reg::A));
                b.copy(qh, Reg::Phys(reg::X));
                b.copy(rl, Reg::Phys(rc(6)));
                b.copy(rh, Reg::Phys(rc(7)));
            }
            code @ (op::UMUL8X8 | op::SMUL8X8) => {
                let (dl, dh) = b.write_word(name(&o[0])?)?;
                let (mut x, mut y) = (b.byte(name(&o[1])?)?, b.byte(name(&o[2])?)?);
                // a product goes either way round: the operand that arrives in
                // `rc3` is the second
                if b.arrives_in(x) == Some(rc(3)) || b.arrives_in(y) == Some(rc(2)) {
                    (x, y) = (y, x);
                }
                b.copy(Reg::Phys(rc(2)), x);
                b.copy(Reg::Phys(rc(3)), y);
                b.call(if code == op::SMUL8X8 { "__mulqihi3" } else { "__umulqihi3" });
                b.copy(dl, Reg::Phys(reg::A));
                b.copy(dh, Reg::Phys(reg::X));
            }
            code @ (op::MUL16 | op::SDIV16 | op::SREM16 | op::SHLV16 | op::SRLV16 | op::SRAV16) => {
                let routine = match code {
                    op::MUL16 => "__mulhi3",
                    op::SDIV16 => "__divhi3",
                    op::SREM16 => "__modhi3",
                    op::SHLV16 => "__ashlhi3",
                    op::SRLV16 => "__lshrhi3",
                    _ => "__ashrhi3",
                };
                let (mut xl, mut xh) = b.read_word(name(&o[1])?)?;
                let (dl, dh) = b.write_word(name(&o[0])?)?;
                let counted = matches!(code, op::SHLV16 | op::SRLV16 | op::SRAV16);
                let second = if counted { None } else { Some(b.read_word(name(&o[2])?)?) };
                let (mut yl, mut yh) = second.unwrap_or((xl, xh));
                // a product goes either way round: the operand that arrives in
                // `rc4` is the second
                if code == op::MUL16 && (b.arrives_in(xl) == Some(rc(4)) || b.arrives_in(yl) == Some(rc(2))) {
                    (xl, xh, yl, yh) = (yl, yh, xl, xh);
                }
                b.copy(Reg::Phys(rc(2)), xl);
                b.copy(Reg::Phys(rc(3)), xh);
                if counted {
                    let count = b.byte(name(&o[2])?)?;
                    b.copy(Reg::Phys(rc(4)), count);
                } else {
                    b.copy(Reg::Phys(rc(4)), yl);
                    b.copy(Reg::Phys(rc(5)), yh);
                }
                b.call(routine);
                b.copy(dl, Reg::Phys(reg::A));
                b.copy(dh, Reg::Phys(reg::X));
            }
            op::RET => {
                let results = results(&b.function.blocks[b.current].insts);
                let mut ret =
                    Inst::new(Kind::Op("RTS"), results.into_iter().map(|r| Operand::Use(Reg::Phys(r))).collect());
                ret.terminator = true;
                ret.barrier = true;
                b.push(ret);
            }
            other => return Err(Unsupported(format!("opcode {other}"))),
        }
        b.flush_packs();
    }
    if let Some(word) = b.halves.keys().next() {
        return Err(Unsupported(format!("one half of {word} written alone")));
    }
    remove_dead(&mut b.function);
    narrow_remainder_calls(&mut b.function);
    b.function.place_unlocated();
    Ok((b.function, labels))
}

/// The branch with the opposite condition.
fn opposite(kind: Kind) -> Option<Kind> {
    Some(Kind::Op(match kind {
        Kind::Op("BEQ") => "BNE",
        Kind::Op("BNE") => "BEQ",
        Kind::Op("BCC") => "BCS",
        Kind::Op("BCS") => "BCC",
        Kind::Op("BMI") => "BPL",
        Kind::Op("BPL") => "BMI",
        _ => return None,
    }))
}

impl<const STACK: bool> Jump for Mos<STACK> {
    fn jump(block: u32) -> Inst {
        let mut jump = Inst::new(Kind::Op("JMP"), vec![Operand::Block(block)]);
        jump.terminator = true;
        jump.barrier = true;
        jump
    }

    fn reverse(branch: &Inst) -> Option<Inst> {
        Some(Inst { kind: opposite(branch.kind)?, ..branch.clone() })
    }
}

/// The physical registers the run of copies just before the return sets: the
/// result. The frame is released between the copies of a result that does not
/// need `a`, and those of the rest; the split of a word into the bytes of two
/// of them (`UNPACK`, made at each copy) lies between the copies of an argument
/// in a:x, and read as the end of the run it dropped `x` from the call's uses.
fn results(insts: &[Inst]) -> Vec<RegId> {
    let mut out = Vec::new();
    for inst in insts.iter().rev() {
        match (inst.kind, inst.operands.first()) {
            (Kind::Copy, Some(Operand::Def(Reg::Phys(p)))) => out.push(*p),
            (Kind::Op("ADJSP" | "UNPACK"), _) => {}
            _ => break,
        }
    }
    out
}

/// Copies and loads of a value nothing reads.
/// A divide whose quotient nobody reads is the routine that returns the
/// remainder in the result registers: the copies after the call of a
/// divide-remainder (`UDIVREM`) read A and X (or A) for the quotient and
/// `rc6:rc7` (or `rc6`) for the remainder, and the dead ones are gone. With
/// only the remainder's left, `__umodhi3` and `__umodqi3` leave it in A:X and
/// A, where the function returns it or copies it without a load.
fn narrow_remainder_calls(function: &mut MFunction<Class>) {
    for block in &mut function.blocks {
        for at in 0..block.insts.len() {
            let narrow = match &block.insts[at].operands[..] {
                [Operand::Symbol(name, _), ..] if block.insts[at].kind == Kind::Op("JSR") => match name.as_str() {
                    "__udivhi3" => "__umodhi3",
                    "__udivmodqi4" => "__umodqi3",
                    _ => continue,
                },
                _ => continue,
            };
            let results: Vec<(usize, RegId)> = (at + 1..block.insts.len())
                .map_while(|i| match &block.insts[i].operands[..] {
                    [_, Operand::Use(Reg::Phys(from))]
                        if block.insts[i].kind == Kind::Copy && [reg::A, reg::X, rc(6), rc(7)].contains(from) =>
                    {
                        Some((i, *from))
                    }
                    _ => None,
                })
                .collect();
            if results.is_empty() || !results.iter().all(|&(_, from)| from == rc(6) || from == rc(7)) {
                continue;
            }
            block.insts[at].operands[0] = Operand::Symbol(narrow.to_owned(), 0);
            for (i, from) in results {
                let narrowed = if from == rc(6) { reg::A } else { reg::X };
                block.insts[i].operands[1] = Operand::Use(Reg::Phys(narrowed));
            }
        }
    }
}

/// At size, a call of a runtime routine that has a compact variant calls it
/// (`routines.toml`'s `[compact]`).
fn call_compact_routines(function: &mut MFunction<Class>) {
    for inst in function.blocks.iter_mut().flat_map(|block| &mut block.insts) {
        if inst.kind != Kind::Op("JSR") {
            continue;
        }
        if let Some(Operand::Symbol(name, _)) = inst.operands.first_mut()
            && let Some(compact) = crate::routines::compact_variant(name)
        {
            *name = compact.to_owned();
        }
    }
}

/// A carry made into a byte and read back with only moves and loads between
/// (`SAVEC; copy; ..; copy; LOADC`) stays in C: the 32-bit add paid
/// `lda #0; rol a; lsr a` between its two halves. A byte nothing reads goes
/// too.
///
/// Every producer of a carry saves it and every consumer loads it, so that
/// nothing the scheduler or allocator puts between the two can lose it; the
/// moves and loads here do not touch C.
fn keep_carry_in_c(function: &mut MFunction<Class>) {
    let mut reads: HashMap<u32, usize> = HashMap::default();
    for operand in function.blocks.iter().flat_map(|b| &b.insts).flat_map(|i| &i.operands) {
        if let Operand::Use(Reg::Virt(v)) = operand {
            *reads.entry(*v).or_default() += 1;
        }
    }
    let acc = Reg::Phys(reg::A);
    let saved = |insts: &[Inst], at: usize| match (insts.get(at).map(|i| i.kind), insts.get(at + 1)) {
        (Some(Kind::Op("SAVEC")), Some(next)) => match (next.kind, &next.operands[..]) {
            (Kind::Copy, [Operand::Def(Reg::Virt(v)), Operand::Use(from)]) if *from == acc => Some(*v),
            _ => None,
        },
        _ => None,
    };
    let moves = |inst: &Inst| matches!(inst.kind, Kind::Copy | Kind::Op("LDM" | "LDI" | "STM"));
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let Some(cell) = saved(&block.insts, at) else {
                // the byte was dropped as unread: the save is a write of A that
                // the next load overwrites
                let overwritten = block
                    .insts
                    .get(at + 1)
                    .is_some_and(
                        |next| matches!(
                            (next.kind, &next.operands[..]),
                            (Kind::Copy, [Operand::Def(to), Operand::Use(from)]) if *to == acc && *from != acc
                        ),
                    );
                if block.insts[at].kind == Kind::Op("SAVEC") && overwritten {
                    block.insts.remove(at);
                } else {
                    at += 1;
                }
                continue;
            };
            let count = reads.get(&cell).copied().unwrap_or(0);
            let load = (at + 2..block.insts.len().saturating_sub(1)).find(|&j| {
                matches!(
                    (block.insts[j].kind, &block.insts[j].operands[..]),
                    (Kind::Copy, [Operand::Def(to), Operand::Use(Reg::Virt(v))]) if *to == acc && *v == cell
                ) && block.insts[j + 1].kind == Kind::Op("LOADC")
            });
            match (count, load) {
                (0, _) => {
                    block.insts.drain(at..at + 2);
                }
                (1, Some(j)) if block.insts[at + 2..j].iter().all(moves) => {
                    block.insts.drain(j..j + 2);
                    block.insts.drain(at..at + 2);
                }
                _ => at += 1,
            }
        }
    }
}

/// A flag compare of the old value of a word, made after the step that
/// replaced it, is made before the step: `inx; bne; iny` and `inc m; bne; inc
/// m+1` leave the carry as it was (they set N and Z), and a branch on the
/// carry alone (`bcc`, `bcs`) reads what the compare left. The old value then
/// needs no copy to outlive the step, and the copies that were made for it
/// go to the passes after (`remove_redundant_copies_between`).
///
/// DEVIATE (6502-LANE-COPY): the pin folds a flag-setting instruction into
/// its compare in `TargetInstrInfo::optimizeCompareInstr`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PeepholeOptimizer.cpp>)
/// and has no pass that moves a compare above an instruction that leaves
/// the carry alone. Exit 2026-11-20 (a generated flags description of the
/// instruction tables).
///
/// The run `T` after the step `S` is copies, loads and the compare itself,
/// ending at the last instruction that sets the flags, and a `bcc` or `bcs`
/// follows. `T` moves to just before `S` when
/// - it reads nothing `S` writes (the old value, not the new one), and writes
///   nothing `S` reads;
/// - what it writes (the accumulator and the flags) is dead after it, and the
///   flags were not live into `S`;
/// - an earlier copy of what `S` writes is what it reads (otherwise there is
///   nothing to save);
/// - `S` leaves the carry alone: not `clc; adc #1` in `a` (the 65C02's `inc a`
///   does).
pub fn compare_before_step(
    function: &mut MFunction<Class>,
    at: &dyn Fn(Reg) -> RegId,
    cmos: bool,
) {
    let flags = reg::FLAGS;
    let regs = |inst: &Inst, def: bool| -> Vec<RegId> {
        inst.operands
            .iter()
            .filter_map(|o| match o {
                Operand::Def(r) if def => Some(at(*r)),
                Operand::Use(r) if !def => Some(at(*r)),
                _ => None,
            })
            .collect()
    };
    loop {
        let after = live_after(function, at);
        let mut moved = false;
        'blocks: for (b, block) in function.blocks.iter_mut().enumerate() {
            for s in 1..block.insts.len() {
                if !matches!(
                    block.insts[s].kind,
                    Kind::Op("INC16" | "DEC16" | "INC" | "DEC")
                ) {
                    continue;
                }
                let (step_writes, step_reads) = (regs(&block.insts[s], true), regs(&block.insts[s], false));
                // `clc; adc #1` (`sec; sbc #1`) clears the carry the compare
                // sets
                if !cmos && step_writes.contains(&reg::A) && matches!(block.insts[s].kind, Kind::Op("INC" | "DEC")) {
                    continue;
                }
                // the run: copies, loads, compares; `last` is its last flag
                // setter
                let mut last = None;
                let mut end = s + 1;
                while let Some(inst) = block.insts.get(end) {
                    if !matches!(inst.kind, Kind::Copy | Kind::Op("LDI" | "CMP" | "SBC")) {
                        break;
                    }
                    if inst.sets_flags() {
                        last = Some(end);
                    }
                    end += 1;
                }
                let Some(last) = last else { continue };
                // only copies and loads between the last flag setter and a
                // carry branch
                let carry_branch = block.insts.get(end).is_some_and(|i| matches!(i.kind, Kind::Op("BCC" | "BCS")));
                if !carry_branch {
                    continue;
                }
                let run = s + 1..=last;
                let (mut reads, mut writes) = (Vec::new(), Vec::new());
                for inst in &block.insts[run.clone()] {
                    for r in regs(inst, false) {
                        if !writes.contains(&r) {
                            reads.push(r);
                        }
                    }
                    writes.extend(regs(inst, true));
                }
                let sound = !reads.iter().any(|r| step_writes.contains(r))
                    && !writes.iter().any(|r| *r != flags && step_reads.contains(r))
                    && !writes.iter().any(|r| *r != flags && after[b][last].contains(r))
                    && !after[b][s - 1].contains(&flags);
                // an earlier copy of a register the step writes is what the run
                // reads
                let saves = block
                    .insts[..s]
                    .iter()
                    .enumerate()
                    .any(
                        |(k, inst)| matches!(
                            (inst.kind, &inst.operands[..]),
                            (Kind::Copy, [Operand::Def(to), Operand::Use(from)])
                                if reads.contains(&at(*to))
                                    && step_writes.contains(&at(*from))
                                    && block.insts[k + 1..s].iter().all(|i| {
                                        let written = regs(i, true);
                                        !written.contains(&at(*to)) && !written.contains(&at(*from))
                                    })
                        ),
                    );
                if sound && saves {
                    let run: Vec<Inst> = block.insts.drain(run).collect();
                    for (n, inst) in run.into_iter().enumerate() {
                        block.insts.insert(s + n, inst);
                    }
                    moved = true;
                    forward_old_copies(function, at, b, s + (last - s));
                    break 'blocks;
                }
            }
        }
        if !moved {
            return;
        }
    }
}

/// The copies of the old value that the step outlived are no longer needed
/// once the compare has moved above it: a copy `to := from` of a register the
/// step at `step` writes, whose readers before the step are all copies, and
/// that nothing reads after it, goes; the readers take `from` (it still holds
/// the old value there).
fn forward_old_copies(
    function: &mut MFunction<Class>,
    at: &dyn Fn(Reg) -> RegId,
    b: usize,
    step: usize,
) {
    let after = live_after(function, at);
    let block = &mut function.blocks[b];
    let written = |inst: &Inst| -> Vec<RegId> {
        inst.operands.iter().filter_map(|o| if let Operand::Def(r) = o { Some(at(*r)) } else { None }).collect()
    };
    let step_writes = written(&block.insts[step]);
    let mut gone = Vec::new();
    for k in 0..step {
        let (to, from) = match (block.insts[k].kind, &block.insts[k].operands[..]) {
            (Kind::Copy, [Operand::Def(to), Operand::Use(from)]) => (*to, *from),
            _ => continue,
        };
        let (to_at, from_at) = (at(to), at(from));
        // a copy into a register may be free (a reload through x leaves the
        // value there): forward it only to save a store, or to read the
        // register the step leaves (`cpx`)
        if !(is_cell(to_at) || !is_cell(from_at))
            || !step_writes.contains(&from_at)
            || step_writes.contains(&to_at)
            || after[b][step].contains(&to_at)
        {
            continue;
        }
        let between = k + 1..step;
        let reads_to = |inst: &Inst| inst.operands.iter().any(|o| matches!(o, Operand::Use(r) if at(*r) == to_at));
        let untouched = block
            .insts[between.clone()]
            .iter()
            .all(
                |i| {
                    let w = written(i);
                    !w.contains(&to_at)
                        && !w.contains(&from_at)
                        && (!reads_to(i) || i.kind == Kind::Copy || i.kind == Kind::Op("CMP"))
                },
            );
        if !untouched {
            continue;
        }
        for inst in &mut block.insts[between] {
            if inst.kind != Kind::Copy && inst.kind != Kind::Op("CMP") {
                continue;
            }
            for o in &mut inst.operands {
                if matches!(o, Operand::Use(r) if at(*r) == to_at) {
                    *o = Operand::Use(from);
                }
            }
        }
        gone.push(k);
    }
    for k in gone.into_iter().rev() {
        block.insts.remove(k);
    }
}

fn remove_dead(function: &mut MFunction<Class>) {
    loop {
        let mut read: HashSet<u32> = HashSet::default();
        for operand in function.blocks.iter().flat_map(|b| &b.insts).flat_map(|i| &i.operands) {
            if let Operand::Use(Reg::Virt(v)) = operand {
                read.insert(*v);
            }
        }
        let mut changed = false;
        for block in &mut function.blocks {
            let before = block.insts.len();
            block
                .insts
                .retain(
                    |inst| {
                        let pure = matches!(inst.kind, Kind::Copy | Kind::Op("LDI" | "LDS"));
                        !(pure
                            && matches!(
                                inst.operands.first(),
                                Some(Operand::Def(Reg::Virt(v))) if !read.contains(v)
                            ))
                    },
                );
            changed |= block.insts.len() != before;
        }
        if !changed {
            return;
        }
    }
}

/// The ca65 name of an allocated register.
/// The assembly of an instruction whose form depends only on the registers it
/// reads and writes: `inc`/`dec` (one byte in `x` or `y`, a cell in place, two
/// instructions in `a`), an ALU op with its second operand, and a compare,
/// whose opcode follows the register the left side is in. `None` for any other.
fn form_text(
    kind: Kind,
    operands: &[Operand],
    regs: &[RegId],
    cmos: bool,
    slot_name: &dyn Fn(i32) -> String,
) -> Option<String> {
    let second = |at: usize| match &operands[at] {
        Operand::Imm(value) => format!("#{value}"),
        Operand::Symbol(symbol, _) => symbol.clone(),
        Operand::Frame(slot) => slot_name(*slot),
        _ => register_name(regs[at.min(regs.len() - 1)]),
    };
    Some(match kind {
        Kind::Op(step @ ("INC" | "DEC")) => {
            let (inc, r) = (step == "INC", regs[0]);
            let (short, long) = if inc { ("in", "adc #1") } else { ("de", "sbc #1") };
            if r == reg::X || r == reg::Y {
                format!("  {short}{}\n", register_name(r))
            } else if r != reg::A {
                format!("  {short}c {}\n", register_name(r))
            } else if cmos {
                format!("  {short}c a\n")
            } else {
                format!("  {}\n  {long}\n", if inc { "clc" } else { "sec" })
            }
        }
        Kind::Op(mnemonic @ ("ADC" | "SBC" | "AND" | "ORA" | "EOR")) if operands.len() >= 3 => {
            format!("  {} {}\n", mnemonic.to_lowercase(), second(2))
        }
        Kind::Op("CMP") if operands.len() >= 3 => {
            let compare = match regs[1] {
                r if r == reg::X => "cpx",
                r if r == reg::Y => "cpy",
                _ => "cmp",
            };
            format!("  {compare} {}\n", second(2))
        }
        _ => return None,
    })
}

/// What `kind` costs on `cpu` with `operands` in `regs`, from the timing
/// tables: the data a hint or a join weighs (an `inc` is 1 byte and 2 clocks in
/// `x`, 2 and 5 in a cell, 3 and 4 in `a`).
pub fn form_cost(
    cpu: Cpu,
    kind: Kind,
    operands: &[Operand],
    regs: &[RegId],
) -> Option<crate::timing::Estimate> {
    let text = form_text(kind, operands, regs, cpu == Cpu::W65c02, &|slot| format!("s_{slot}"))?;
    crate::timing::estimate(cpu, &text).ok()
}

fn register_name(register: RegId) -> String {
    if register == reg::A {
        "a".into()
    } else if register == reg::X {
        "x".into()
    } else if register == reg::Y {
        "y".into()
    } else {
        format!("rc{}", register.index() - reg::RC0.index())
    }
}

/// Registers live after each instruction of each block, by their final names.
fn live_after(
    function: &MFunction<Class>,
    at: &dyn Fn(Reg) -> RegId,
) -> Vec<Vec<HashSet<RegId>>> {
    let blocks = function.blocks.len();
    let next = successors(function);
    // A pair is its two bytes: a lane moved into a cell the live pair is made
    // of is read through the pair (`UNPACK` into the lane of a pointer).
    let named = |r: Reg, into: &mut Vec<RegId>| {
        let register = at(r);
        into.push(register);
        if is_pair(register) {
            into.extend([lane(register, 0), lane(register, 1)]);
        }
    };
    let effects = |inst: &Inst| {
        let mut defs = Vec::new();
        let mut reads = Vec::new();
        for operand in &inst.operands {
            match operand {
                Operand::Def(r) => named(*r, &mut defs),
                Operand::Use(r) => named(*r, &mut reads),
                _ => {}
            }
        }
        // a call writes every register it does not preserve, results included
        if inst.call {
            defs.extend(Mos::<false>::caller_saved());
        }
        (defs, reads)
    };
    let mut live_in: Vec<HashSet<RegId>> = vec![HashSet::default(); blocks];
    let mut after: Vec<Vec<HashSet<RegId>>> =
        function.blocks.iter().map(|b| vec![HashSet::default(); b.insts.len()]).collect();
    loop {
        let mut changed = false;
        for b in (0..blocks).rev() {
            let mut live: HashSet<RegId> = next[b].iter().flat_map(|n| live_in[*n].iter().copied()).collect();
            for (i, inst) in function.blocks[b].insts.iter().enumerate().rev() {
                after[b][i] = live.clone();
                let (defs, reads) = effects(inst);
                for d in &defs {
                    live.remove(d);
                }
                live.extend(reads);
            }
            if live != live_in[b] {
                live_in[b] = live;
                changed = true;
            }
        }
        if !changed {
            return after;
        }
    }
}

/// A move that needs a register when neither side is one: through `spare`, an
/// index register nothing reads after it (`lda`/`sta` become `ldx`/`stx`), else
/// through `a`, bracketed by `pha`/`pla` when it is live. `body` is a plain
/// `lda`/`sta` pair.
fn through_a(
    out: &mut Asm,
    a_live: bool,
    spare: Option<&str>,
    body: &str,
) {
    match spare.filter(|_| a_live) {
        Some(r) => out.push_str(&body.replace("lda ", &format!("ld{r} ")).replace("sta ", &format!("st{r} "))),
        None if a_live => {
            out.push_str("  pha\n");
            out.push_str(body);
            out.push_str("  pla\n");
        }
        None => out.push_str(body),
    }
}

/// Several byte moves as if at once: no move reads a register an earlier one
/// wrote, and `a` is saved around the sequence when it is live after it. A
/// cycle (a swap of two cells) goes through the stack.
pub fn parallel_move(
    out: &mut Asm,
    moves: &[(RegId, RegId)],
    a_live: bool,
) {
    let mut pending: Vec<(RegId, Option<RegId>)> =
        moves.iter().filter(|(d, s)| d != s).map(|(d, s)| (*d, Some(*s))).collect();
    if pending.is_empty() {
        return;
    }
    let a_is_dest = pending.iter().any(|(d, _)| *d == reg::A);
    let save = a_live && !a_is_dest;
    if save {
        out.push_str("  pha\n");
    }
    let direct = |r: RegId| r == reg::A || r == reg::X || r == reg::Y;
    while !pending.is_empty() {
        let a_wanted = |skip: usize, pending: &[(RegId, Option<RegId>)]| {
            pending.iter().enumerate().any(|(i, (_, s))| i != skip && *s == Some(reg::A))
        };
        let ready = (0..pending.len()).find(|&i| {
            let (d, s) = pending[i];
            let free = !pending.iter().enumerate().any(|(j, (_, other))| j != i && *other == Some(d));
            let scratch = match s {
                Some(s) => !(direct(d) && direct(s) && (d == reg::A || s == reg::A)),
                None => true,
            };
            let clobbers_a = scratch && d != reg::A && s != Some(reg::A);
            // a move into `a` is last: the others may use `a` as scratch
            free && (!clobbers_a || !a_wanted(i, &pending)) && (d != reg::A || pending.len() == 1)
        });
        match ready {
            Some(i) => {
                let (d, s) = pending.remove(i);
                match s {
                    Some(s) => move_byte(out, d, s, false, None, None),
                    None => {
                        out.push_str("  pla\n");
                        move_byte(out, d, reg::A, false, None, None);
                    }
                }
            }
            None => {
                // blocked: either a move needs `a` as scratch while another
                // reads it, or only cycles of cells are left. Park the value
                // on the stack and read it from there (two moves at most, so
                // one parked value).
                let parked = if pending.iter().any(|(_, s)| *s == Some(reg::A)) {
                    out.push_str("  pha\n");
                    reg::A
                } else {
                    // no move reads `a`: a cell another move reads is held up
                    let (d, _) = *pending
                        .iter()
                        .find(|(d, _)| pending.iter().any(|(_, s)| *s == Some(*d)))
                        .expect("a blocked sequence without `a` has a cycle");
                    move_byte(out, reg::A, d, false, None, None);
                    out.push_str("  pha\n");
                    d
                };
                for (_, s) in pending.iter_mut().filter(|(_, s)| *s == Some(parked)) {
                    *s = None;
                }
            }
        }
    }
    if save {
        out.push_str("  pla\n");
    }
}

fn move_byte(
    out: &mut Asm,
    to: RegId,
    from: RegId,
    a_live: bool,
    spare: Option<&str>,
    free_cell: Option<RegId>,
) {
    let (d, s) = (register_name(to), register_name(from));
    if to == from {
        return;
    }
    let reg3 = |r: RegId| r == reg::A || r == reg::X || r == reg::Y;
    match (reg3(to), reg3(from)) {
        (true, true) if to == reg::A || from == reg::A => {
            let mnemonic = if to == reg::A { format!("t{s}a") } else { format!("ta{d}") };
            writeln!(out, "  {mnemonic}").unwrap();
        }
        // x to y or y to x: through `a`; with `a` live, through a cell nothing
        // reads (`sty c; ldx c`, 6 clocks) before the bracket (`pha; tya; tax;
        // pla`, 11)
        (true, true) => match free_cell.filter(|_| a_live) {
            Some(cell) => writeln!(out, "  st{s} {}\n  ld{d} {0}", register_name(cell)).unwrap(),
            None => through_a(out, a_live, None, &format!("  t{s}a\n  ta{d}\n")),
        },
        (true, false) => writeln!(out, "  ld{d} {s}").unwrap(),
        (false, true) => writeln!(out, "  st{s} {d}").unwrap(),
        (false, false) => through_a(out, a_live, spare, &format!("  lda {s}\n  sta {d}\n")),
    }
}

/// The function as ca65 lines (no label), with the zero-page slots the spills
/// used, or an error from the allocator. `cmos` is the 65C02 subtarget: a zero
/// loaded into a cell is `stz`.
///
/// A callee-saved cell costs a save and a restore, so [`Mos::worth_taking`]
/// gives one only to a register that gains that back; the rest are spilled.
///
/// An instruction the sink moved into another block (`machine_sink_in_place`)
/// may cost a jump or a callee-saved cell the allocation then needs, so a
/// function the sink changed is also emitted without it, and the sunk form is
/// kept only if it is no larger and no slower by the timing table
/// (`MachineSink` weighs the move by block frequency; the table is this
/// target's).
pub fn emit(
    function: MFunction<Class>,
    labels: &[Vec<String>],
    stack: bool,
    cmos: bool,
    zero_page: u8,
    tuning: Tuning,
    source: Option<(&str, Option<u32>)>,
) -> Result<(String, Vec<(String, usize)>), String> {
    let before = IMAGINARY.with(|n| n.replace(zero_page.clamp(BASE_CELLS, MOST_CELLS)));
    let level = TUNING.with(|t| t.replace(tuning));
    let result = emit_on(function, labels, stack, cmos, source);
    IMAGINARY.with(|n| n.set(before));
    TUNING.with(|t| t.set(level));
    result
}

/// The most zero-page bytes the register file names (`rc0`..`rc25`).
pub const MOST_CELLS: u8 = 26;

fn emit_on(
    function: MFunction<Class>,
    labels: &[Vec<String>],
    stack: bool,
    cmos: bool,
    source: Option<(&str, Option<u32>)>,
) -> Result<(String, Vec<(String, usize)>), String> {
    let run = |function: MFunction<Class>, sink: bool, share: bool, place: bool| {
        if stack {
            emit_with::<true>(function, labels, cmos, sink, share, place, source)
        } else {
            emit_with::<false>(function, labels, cmos, sink, share, place, source)
        }
    };
    let cpu = if cmos { Cpu::W65c02 } else { Cpu::Mos6502 };
    // a variant is priced on its text before the transfer of known constants,
    // the pass after the layout; what is kept is the text with it
    let price = |code: &str, slots: &[(String, usize)]| {
        crate::timing::estimate(cpu, &without_lines(code))
            .map(|priced| (priced.bytes + slots.iter().map(|(_, n)| *n as u32).sum::<u32>(), priced.cycles))
    };
    // a change of layout is kept where it wins both before the transfer and
    // after it: a byte the pass saves must not tip the choice to a slower
    // layout, nor a layout it would have undone
    let wins = |kept: (&str, &str, &[(String, usize)]), other: (&str, &str, &[(String, usize)])| {
        let better = |a: Result<(u32, u64), String>, b: Result<(u32, u64), String>| matches!((a, b), (Ok(a), Ok(b)) if a.0 > b.0 || a.1 > b.1);
        better(price(kept.1, kept.2), price(other.1, other.2)) && better(price(kept.0, kept.2), price(other.0, other.2))
    };
    let (mut code, mut priced, mut slots, sunk, shared, moved) = run(function.clone(), true, true, true)?;
    // a layout that rotates a loop is kept only where it is no larger and no
    // slower than the order the blocks came in
    let mut place = true;
    if moved
        && let Ok((other, other_priced, other_slots, ..)) = run(function.clone(), true, true, false)
        && wins((&code, &priced, &slots), (&other, &other_priced, &other_slots))
    {
        (code, priced, slots, place) = (other, other_priced, other_slots, false);
    }
    if !sunk && !shared {
        return Ok((code, slots));
    }
    // each change is kept only where it is no larger and no slower than the
    // function without it
    let mut best = (code, priced, slots);
    let mut variants = vec![(false, false)];
    if sunk && shared {
        variants.extend([(true, false), (false, true)]);
    }
    for (sink, share) in variants {
        let Ok((other, other_priced, other_slots, ..)) = run(function.clone(), sink, share, place) else {
            continue;
        };
        if wins((&best.0, &best.1, &best.2), (&other, &other_priced, &other_slots)) {
            best = (other, other_priced, other_slots);
        }
    }
    Ok((best.0, best.2))
}

/// `rs0 += n` (the frame, `n` as a signed 16-bit amount).
fn adjust(
    out: &mut Asm,
    n: i64,
) {
    let n = n as u16;
    writeln!(out, "  clc\n  lda rc0\n  adc #{}\n  sta rc0\n  lda rc1\n  adc #{}\n  sta rc1", n & 0xFF, n >> 8).unwrap();
}

/// The callee-saved bytes the allocation used are stored to a slot of their own
/// on entry and loaded back before each `rts`
/// (`PrologEpilogInserter::spillCalleeSavedRegisters`).
fn save_preserved<const STACK: bool>(
    function: &mut MFunction<Class>,
    assignment: &llrm_spike_mf::regalloc::Assignment,
    slots: &mut Vec<(i32, Class)>,
) -> bool {
    let mut used: Vec<RegId> = Vec::new();
    let named = function
        .blocks
        .iter()
        .flat_map(|b| &b.insts)
        .flat_map(|i| &i.operands)
        .filter_map(
            |o| match o {
                Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) => Some(assignment.0[*v as usize]),
                Operand::Def(Reg::Phys(r)) | Operand::Use(Reg::Phys(r)) => Some(*r),
                _ => None,
            },
        );
    for register in named.collect::<Vec<_>>() {
        let bytes = if is_pair(register) { vec![lane(register, 0), lane(register, 1)] } else { vec![register] };
        for byte in bytes.into_iter().filter(|r| preserved(*r)) {
            if !used.contains(&byte) {
                used.push(byte);
            }
        }
    }
    used.sort_by_key(|r| r.index());
    // the bytes a push will save, for the entry copies below
    let kept = used.clone();
    let mut saves = Vec::new();
    let mut restores = Vec::new();
    // Static slots: each cell goes on the hardware stack, three bytes to save
    // and three to restore against five and five through a slot of its own
    // (and no RAM). The soft stack keeps its slots: a recursive function
    // would run the hardware stack out.
    if !STACK && !used.is_empty() {
        // `rc8`.. up to `n` of them: one call saves and one jump restores
        // (`__csave`, `__crest`), 11 bytes against 6 a cell and 2 for
        // the result, and 57 + 14n clocks more; taken where the level
        // pays that for the bytes
        let n = used.len() as u32;
        let (saved, slower) = (shared_save_gain(n), 57 + 14 * n);
        let shared = SHARED_SAVES.with(|a| a.get())
            && n >= 3
            && used.iter().enumerate().all(|(at, byte)| *byte == rc(8 + at as u8))
            && saved > 0
            && slower <= TUNING.with(|t| t.get()).clocks_per_byte * saved as u32;
        if shared {
            saves.push(Inst::new(Kind::Op("CSAVE"), used.iter().map(|byte| Operand::Use(Reg::Phys(*byte))).collect()));
            let popped = used.iter().rev().map(|byte| Operand::Def(Reg::Phys(*byte))).collect();
            restores.push(Inst::new(Kind::Op("CREST"), popped));
        } else {
            saves.extend(used.iter().map(|byte| Inst::new(Kind::Op("PUSHC"), vec![Operand::Use(Reg::Phys(*byte))])));
            let popped = used.iter().rev().map(|byte| Operand::Def(Reg::Phys(*byte))).collect();
            restores.push(Inst::new(Kind::Op("POPS"), popped));
        }
    }
    for byte in used.into_iter().filter(|_| STACK) {
        let slot = slots.iter().map(|(s, _)| s + 1).max().unwrap_or(0);
        slots.push((slot, Class::MEM));
        saves.push(Mos::<STACK>::spill_store(Class::MEM, Reg::Phys(byte), slot).expect("a spill"));
        restores.push(Mos::<STACK>::spill_load(Class::MEM, Reg::Phys(byte), slot).expect("a reload"));
    }
    if saves.is_empty() {
        return false;
    }
    for block in &mut function.blocks {
        let mut out = Vec::with_capacity(block.insts.len());
        for inst in std::mem::take(&mut block.insts) {
            if inst.kind == Kind::Op("RTS") {
                out.extend(restores.iter().cloned());
            }
            out.push(inst);
        }
        block.insts = out;
    }
    // after the frame is allocated: its slots are addressed from the adjusted
    // stack pointer
    let entry = &mut function.blocks[0].insts;
    let mut at = usize::from(entry.first().is_some_and(|inst| inst.kind == Kind::Op("ADJSP")));
    // an argument that arrives in `a` or `x` is copied out first: a push goes
    // through `a`. Not into a cell the push is about to save: the push would
    // keep the argument, and the caller's value of the cell would be lost.
    while let Some(Inst { kind: Kind::Copy, operands, .. }) = entry.get(at) {
        if !matches!(
            operands.get(1),
            Some(Operand::Use(Reg::Phys(r))) if [reg::A, reg::X].contains(r)
        ) {
            break;
        }
        let into = match operands.first() {
            Some(Operand::Def(Reg::Virt(v))) => Some(assignment.0[*v as usize]),
            Some(Operand::Def(Reg::Phys(r))) => Some(*r),
            _ => None,
        };
        let bytes = into.map_or_else(Vec::new, |r| if is_pair(r) { vec![lane(r, 0), lane(r, 1)] } else { vec![r] });
        if bytes.iter().any(|byte| kept.contains(byte)) {
            break;
        }
        at += 1;
    }
    entry.splice(at..at, saves);
    true
}

/// Whether `inst` tests only the zero or sign flag: `beq`, `bne`, `bmi`, `bpl`,
/// or a select or set whose condition code (the last immediate, before the
/// flags it reads) is one of those.
fn reads_zero_or_sign_only(inst: &Inst) -> bool {
    match inst.kind {
        Kind::Op("BEQ" | "BNE" | "BMI" | "BPL") => true,
        _ if inst.reads_flags => inst
            .operands
            .iter()
            .rev()
            .find_map(|o| if let Operand::Imm(code) = o { Some(*code) } else { None })
            .is_some_and(|code| matches!(code, 0 | 1 | 4 | 5)),
        _ => false,
    }
}

/// The odds of a branch out of a loop, as `BranchProbabilityInfo`'s loop
/// heuristic gives them with no profile: the edge that stays in the innermost
/// loop of its block takes 124 of 128, the edge that leaves it the rest.
fn loop_odds(function: &MFunction<Class>) -> llrm_spike_mf::placement::Odds {
    use llrm_mir::blockfrequency::BranchProbability;
    let cfg = llrm_spike_mf::cfg::Cfg::new(function);
    let mut odds = llrm_spike_mf::placement::Odds::default();
    for (from, succs) in cfg.succs.iter().enumerate() {
        if succs.len() != 2 {
            continue;
        }
        let innermost = cfg
            .loops()
            .iter()
            .filter(|(_, body)| body[from])
            .min_by_key(|(_, body)| body.iter().filter(|b| **b).count());
        let Some((_, body)) = innermost else { continue };
        let leaves: Vec<bool> = succs.iter().map(|&to| !body[to]).collect();
        if leaves[0] == leaves[1] {
            continue;
        }
        for (rank, &to) in succs.iter().enumerate() {
            let share = if leaves[rank] { 4 } else { 124 };
            odds.insert((function.blocks[from].id, function.blocks[to].id), (BranchProbability::new(share, 128), rank));
        }
    }
    odds
}

/// `labels` with a label for each block the folding added, numbered past the
/// source's.
pub fn labels_with_new_blocks(
    labels: &[Vec<String>],
    function: &MFunction<Class>,
) -> Vec<Vec<String>> {
    let mut all = labels.to_vec();
    if let Some(last) = function.blocks.iter().map(|block| block.id as usize).max() {
        all.resize(all.len().max(last + 1), Vec::new());
    }
    // a block made past the source's, whatever order the layout reaches them in
    for block in &function.blocks {
        let id = block.id as usize;
        if all[id].is_empty() {
            all[id].push(format!("{}__tm{id}", function.name));
        }
    }
    all
}

/// How many times this thread ran `emit_with`, a whole function's selection
/// to text (what a test counts to see how many variants a function paid for).
pub fn emit_runs() -> usize {
    EMITS.with(std::cell::Cell::get)
}

fn emit_with<const STACK: bool>(
    mut function: MFunction<Class>,
    labels: &[Vec<String>],
    cmos: bool,
    sink: bool,
    share: bool,
    place: bool,
    source: Option<(&str, Option<u32>)>,
) -> Result<(String, String, Vec<(String, usize)>, bool, bool, bool), String> {
    EMITS.with(|runs| runs.set(runs.get() + 1));
    // an instruction goes to the one path that reads it; a bump of a pointer is
    // then done in place, and the read of the old pointer that follows it
    // goes first
    if TUNING.with(|t| t.get()) == Tuning::SIZE {
        call_compact_routines(&mut function);
    }
    keep_carry_in_c(&mut function);
    let sunk = sink && machine_sink_in_place::<Mos<STACK>>(&mut function);
    let shared = share && crate::sharecmp::share_compares(&mut function);
    if crate::cmpzero::drop_zero_compares(&mut function) {
        remove_dead(&mut function);
    }
    reschedule_kills::<Mos<STACK>>(&mut function);
    commute_accumulator::<Mos<STACK>>(&mut function);
    two_address(&mut function);
    let (assignment, mut slots) = allocate::<STACK>(&mut function)?;
    save_preserved::<STACK>(&mut function, &assignment, &mut slots);
    let size_of = |class: &Class| if class.0 & Class::P != 0 { 2 } else { 1 };
    // On the stack, spills sit above the locals, in the frame: the bytes of
    // the slots (`spilled`) are added to the frame, and a frame object beyond
    // the old frame (`old`: an incoming argument) moves up by them.
    let spilled: i64 = if STACK { slots.iter().map(|(_, class)| size_of(class)).sum::<usize>() as i64 } else { 0 };
    let all = function.blocks.iter().flat_map(|b| &b.insts);
    let old: i64 = all
        .clone()
        .find_map(|i| match (i.kind, &i.operands[..]) {
            (Kind::Op("ADJSP"), [_, Operand::Imm(n)]) if (*n as u16 as i16) < 0 => Some(-i64::from(*n as u16 as i16)),
            _ => None,
        })
        .unwrap_or(0);
    let has_frame = all.clone().any(|i| i.kind == Kind::Op("ADJSP"));
    let slot_offset = |slot: i32| -> i64 {
        old + slots.iter().take_while(|(s, _)| *s != slot).map(|(_, class)| size_of(class) as i64).sum::<i64>()
    };
    let at = |r: Reg| match r {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    };
    compare_before_step(&mut function, &at, cmos);
    remove_identity_copies::<Mos<STACK>>(&mut function, &assignment);
    remove_redundant_copies_between::<Mos<STACK>>(&mut function, &assignment, |_| true, |_| true);
    // a reload of a slot whose register nothing has written is that register; x
    // and y copy through a
    let spill_slot = |inst: &Inst, name: &str| match (inst.kind, &inst.operands[..]) {
        (Kind::Op(op), [Operand::Use(reg) | Operand::Def(reg), Operand::Frame(slot), ..]) if op == name => {
            Some((*slot, *reg))
        }
        _ => None,
    };
    forward_reloads::<Mos<STACK>>(
        &mut function,
        &assignment,
        |inst| spill_slot(inst, "SPILL"),
        |inst| spill_slot(inst, "RELOAD"),
        is_pair,
        |to, from| !matches!((to, from), (reg::X, reg::Y) | (reg::Y, reg::X)),
    );
    remove_redundant_copies_between::<Mos<STACK>>(&mut function, &assignment, |_| true, |_| true);
    branch_fold::<Mos<STACK>>(&mut function);
    // tail merging splits blocks: a block it makes has no label of the source's
    let labels = &labels_with_new_blocks(labels, &function);
    let moved = place && {
        let odds = loop_odds(&function);
        llrm_spike_mf::placement::place_blocks_counting::<Class, Mos<STACK>>(&mut function, &odds, false, true)
    };
    let after = live_after(&function, &at);
    let slot_name = |slot: i32| format!("s_{}_{}", function.name, slot);
    let mut out = Asm::new();
    let mut made = 0;
    let mut pending: Option<i64> = None;
    // A call that returns at once is a jump: the callee's `rts` returns from
    // here. `__csave` keeps the return address, and a frame to release stands
    // between the call and the return.
    //
    // DEVIATE (6502-2): decided at emission, where the pin's `isTailCall` is
    // decided in call lowering (as llvm-mos does). Exit: tail calls in the
    // 6502's call lowering.
    let tail_call = |block: &Block, i: usize, pending: Option<i64>| {
        let named = |inst: &Inst| {
            matches!(
                &inst.operands[..],
                [Operand::Symbol(callee, _), ..] if !callee.starts_with("__csave")
            )
        };
        block.insts[i].kind == Kind::Op("JSR")
            && named(&block.insts[i])
            && block.insts.get(i + 1).is_some_and(|next| next.kind == Kind::Op("RTS"))
            && pending.is_none()
            && !(spilled > 0 && !has_frame)
    };
    let target = |operand: &Operand| match operand {
        Operand::Block(id) => labels[*id as usize][0].clone(),
        other => unreachable!("{other:?} is not a block"),
    };
    let mut starts: Vec<Vec<usize>> = Vec::new();
    for (b, block) in function.blocks.iter().enumerate() {
        for label in &labels[block.id as usize] {
            writeln!(out, "{label}:").unwrap();
        }
        starts.push(Vec::new());
        if b == 0 && spilled > 0 && !has_frame {
            // an argument in `a` that the entry block reads before it writes
            let first = block.insts.first();
            let a_in = first.is_some_and(|inst| {
                let touches =
                    |want: fn(&Operand) -> Option<Reg>| inst.operands.iter().filter_map(want).any(|r| at(r) == reg::A);
                touches(|o| if let Operand::Use(r) = o { Some(*r) } else { None })
                    || (after[0][0].contains(&reg::A)
                        && !touches(|o| if let Operand::Def(r) = o { Some(*r) } else { None }))
            });
            if a_in {
                out.push_str("  pha\n");
            }
            adjust(&mut out, -spilled);
            if a_in {
                out.push_str("  pla\n");
            }
        }
        let mut repeated = 0;
        for (i, inst) in block.insts.iter().enumerate() {
            starts[b].push(out.lines.len());
            out.place = inst.origin;
            // the rest of a run of one-bit shifts a loop has emitted
            if repeated > 0 {
                repeated -= 1;
                continue;
            }
            let a_live = after[b][i].contains(&reg::A);
            let regs: Vec<RegId> = inst
                .operands
                .iter()
                .filter_map(|o| match o {
                    Operand::Def(r) | Operand::Use(r) => Some(at(*r)),
                    _ => None,
                })
                .collect();
            let spare = [(reg::X, "x"), (reg::Y, "y")]
                .into_iter()
                .find(|(r, _)| !after[b][i].contains(r) && !regs.contains(r))
                .map(|(_, name)| name);
            // a cell a call clobbers anyway that nothing holds here
            let holds = |c: RegId, r: RegId| c == r || (is_pair(r) && (lane(r, 0) == c || lane(r, 1) == c));
            let free_cell = (2..8)
                .chain(BASE_CELLS..imaginary())
                .map(rc)
                .find(|c| !after[b][i].iter().any(|r| holds(*c, *r)) && !regs.iter().any(|r| holds(*c, *r)));
            match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, _) if is_word(regs[0]) && is_word(regs[1]) => {
                    let moves: Vec<_> = (0..2).map(|n| (lane(regs[0], n), lane(regs[1], n))).collect();
                    parallel_move(&mut out, &moves, a_live);
                }
                // a word copied into a byte is its low half (`trunc`
                // at -O0); `lane` of a byte register named a cell no pair has
                (Kind::Copy, _) if is_word(regs[1]) => {
                    move_byte(&mut out, regs[0], lane(regs[1], 0), a_live, spare, free_cell);
                }
                (Kind::Copy, _) if is_word(regs[0]) => return Err("a byte copied into a word".into()),
                (Kind::Copy, _) => move_byte(&mut out, regs[0], regs[1], a_live, spare, free_cell),
                // a lane nothing reads is not moved (a word truncated to a
                // byte)
                (Kind::Op("UNPACK"), _) => {
                    let moves: Vec<_> = [(regs[0], lane(regs[2], 0)), (regs[1], lane(regs[2], 1))]
                        .into_iter()
                        .filter(|(to, _)| after[b][i].contains(to))
                        .collect();
                    parallel_move(&mut out, &moves, a_live);
                }
                (Kind::Op("PACK"), _) => {
                    parallel_move(&mut out, &[(lane(regs[0], 0), regs[1]), (lane(regs[0], 1), regs[2])], a_live);
                }
                // d := s + k on a pair: the low byte bumped, the high on carry
                (Kind::Op("PADD"), [_, _, Operand::Imm(k), ..]) => {
                    let (d, s) = (regs[0], regs[1]);
                    parallel_move(&mut out, &[(lane(d, 0), lane(s, 0)), (lane(d, 1), lane(s, 1))], a_live && *k == 1);
                    made += 1;
                    let (low, high) = (register_name(lane(d, 0)), register_name(lane(d, 1)));
                    if *k == 1 {
                        writeln!(out, "  inc {low}\n  bne _pa{made}\n  inc {high}\n_pa{made}:").unwrap();
                    } else {
                        writeln!(
                            out,
                            "  clc\n  lda {low}\n  adc #{k}\n  sta {low}\n  bcc _pa{made}\n  inc {high}\n_pa{made}:"
                        )
                        .unwrap();
                    }
                }
                // d := d + 1 on two cells: the high byte steps when the low
                // wrapped
                (Kind::Op("INC16"), _) => {
                    made += 1;
                    let step = |r| match r {
                        reg::X => "inx".to_owned(),
                        reg::Y => "iny".to_owned(),
                        cell => format!("inc {}", register_name(cell)),
                    };
                    let (low, high) = (step(regs[0]), step(regs[1]));
                    writeln!(out, "  {low}\n  bne _pa{made}\n  {high}\n_pa{made}:").unwrap();
                }
                // d := d - 1 on two cells: the high byte steps when the low was
                // 0
                (Kind::Op("DEC16"), _) => {
                    made += 1;
                    // a byte in x or y is read through a and stepped by dex/dey
                    let read = |r| match r {
                        reg::X => "txa".to_owned(),
                        reg::Y => "tya".to_owned(),
                        cell => format!("lda {}", register_name(cell)),
                    };
                    let step = |r| match r {
                        reg::X => "dex".to_owned(),
                        reg::Y => "dey".to_owned(),
                        cell => format!("dec {}", register_name(cell)),
                    };
                    let (load, low, high) = (read(regs[0]), step(regs[0]), step(regs[1]));
                    writeln!(out, "  {load}\n  bne _pa{made}\n  {high}\n_pa{made}:\n  {low}").unwrap();
                }
                // d := s - k on a pair: the low byte stepped down, the high on
                // borrow
                (Kind::Op("PSUB"), [_, _, Operand::Imm(k), ..]) => {
                    let (d, s) = (regs[0], regs[1]);
                    parallel_move(&mut out, &[(lane(d, 0), lane(s, 0)), (lane(d, 1), lane(s, 1))], false);
                    made += 1;
                    let (low, high) = (register_name(lane(d, 0)), register_name(lane(d, 1)));
                    if *k == 1 {
                        writeln!(out, "  lda {low}\n  bne _pa{made}\n  dec {high}\n_pa{made}:\n  dec {low}").unwrap();
                    } else {
                        writeln!(
                            out,
                            "  sec\n  lda {low}\n  sbc #{k}\n  sta {low}\n  bcs _pa{made}\n  dec {high}\n_pa{made}:"
                        )
                        .unwrap();
                    }
                }
                (Kind::Op("PINCH"), _) => {
                    let (d, s) = (regs[0], regs[1]);
                    parallel_move(&mut out, &[(lane(d, 0), lane(s, 0)), (lane(d, 1), lane(s, 1))], a_live);
                    writeln!(out, "  inc {}", register_name(lane(d, 1))).unwrap();
                }
                (
                    Kind::Op(step @ ("ADCPY" | "SBCPY" | "ANDPY" | "ORAPY" | "EORPY")),
                    [_, _, _, Operand::Imm(off), _],
                ) => {
                    let (op, p) = (step[..3].to_lowercase(), register_name(lane(regs[2], 0)));
                    if cmos && *off == 0 {
                        writeln!(out, "  {op} ({p})").unwrap()
                    } else {
                        writeln!(out, "  ldy #{off}\n  {op} ({p}),y").unwrap()
                    }
                }
                (Kind::Op(step @ ("ADCPX" | "SBCPX" | "ANDPX" | "ORAPX" | "EORPX")), _) => {
                    writeln!(out, "  {} ({}),y", step[..3].to_lowercase(), register_name(lane(regs[2], 0))).unwrap()
                }
                (Kind::Op("LDPX"), _) => writeln!(out, "  lda ({}),y", register_name(lane(regs[1], 0))).unwrap(),
                (Kind::Op("STPX"), _) => writeln!(out, "  sta ({}),y", register_name(lane(regs[1], 0))).unwrap(),
                (Kind::Op("LDPYZ"), [_, _, Operand::Imm(off), _]) => {
                    let p = register_name(lane(regs[1], 0));
                    if cmos && *off == 0 {
                        writeln!(out, "  lda ({p})").unwrap()
                    } else {
                        writeln!(out, "  ldy #{off}\n  lda ({p}),y").unwrap()
                    }
                }
                (Kind::Op("LDPY"), [_, _, Operand::Imm(off), _]) => {
                    writeln!(out, "  ldy #{off}\n  lda ({}),y", register_name(lane(regs[1], 0))).unwrap()
                }
                (Kind::Op("STPY"), [_, _, Operand::Imm(off), _]) => {
                    writeln!(out, "  ldy #{off}\n  sta ({}),y", register_name(lane(regs[1], 0))).unwrap()
                }
                (Kind::Op("LDP0"), _) => writeln!(out, "  lda ({})", register_name(lane(regs[1], 0))).unwrap(),
                (Kind::Op("STP0"), _) => writeln!(out, "  sta ({})", register_name(lane(regs[1], 0))).unwrap(),
                (Kind::Op("LDI"), [_, Operand::Imm(value)]) => {
                    let d = regs[0];
                    if d == reg::A || d == reg::X || d == reg::Y {
                        writeln!(out, "  ld{} #{value}", register_name(d)).unwrap();
                    } else if cmos && *value == 0 {
                        writeln!(out, "  stz {}", register_name(d)).unwrap();
                    } else {
                        through_a(&mut out, a_live, spare, &format!("  lda #{value}\n  sta {}\n", register_name(d)));
                    }
                }
                (Kind::Op("LDS"), [_, Operand::Symbol(symbol, half)]) => {
                    let d = regs[0];
                    let part = if *half == 0 { '<' } else { '>' };
                    if d == reg::A || d == reg::X || d == reg::Y {
                        writeln!(out, "  ld{} #{part}({symbol})", register_name(d)).unwrap();
                    } else {
                        through_a(
                            &mut out,
                            a_live,
                            spare,
                            &format!("  lda #{part}({symbol})\n  sta {}\n", register_name(d)),
                        );
                    }
                }
                (Kind::Op("JSR"), [Operand::Symbol(callee, _), ..]) => {
                    let jump = tail_call(block, i, pending);
                    // the speed variants of the variable long shifts move whole
                    // bytes first: 45 bytes more each, linked once, for 200
                    // clocks fewer per 8 bits. `__mulsi3_fast` (19 bytes more)
                    // is not chosen: a site that never runs or has a constant
                    // multiplier priced as run once still links it. The long
                    // division and remainder start at the quotient's first bit
                    // (compiler-rt's `__udivXi3` does, by `clz`): 107 bytes
                    // more, linked once, for 2000 clocks fewer a call
                    // (loop_interchange -O2 13.9M -> 5.9M).
                    // `_byte` marks the byte-divisor entry of the four
                    // divisions only: a program's own function may end in it
                    let (callee, byte) = match callee.strip_suffix("_byte") {
                        Some(base @ ("__udivsi3" | "__divsi3" | "__umodsi3" | "__modsi3")) => (base, true),
                        _ => (callee.as_str(), false),
                    };
                    let speed = TUNING.with(|t| t.get()).clocks_per_byte <= 1;
                    let division = matches!(callee, "__udivsi3" | "__divsi3" | "__umodsi3" | "__modsi3");
                    let entry = if !speed
                        && !(callee == "__mulsi3" && FAST_MULTIPLY.with(|f| f.get()))
                        && !(division && SIZE_FAST_DIVISION.with(|f| f.get()))
                    {
                        ""
                    } else if division && byte {
                        "_fast_byte"
                    } else if division && !FAST_DIVISION.with(|f| f.get()) {
                        "_fast_small"
                    } else if division
                        || matches!(callee, "__ashlsi3" | "__lshrsi3" | "__ashrsi3")
                        || (callee == "__mulsi3" && FAST_MULTIPLY.with(|f| f.get()))
                    {
                        "_fast"
                    } else {
                        ""
                    };
                    writeln!(out, "  {} {callee}{entry}", if jump { "jmp" } else { "jsr" }).unwrap()
                }
                // return address, then target-1, then rts into the callee; its
                // own rts lands on the label
                (Kind::Op("JSRI"), _) => {
                    made += 1;
                    let (lo, hi) = (register_name(lane(regs[0], 0)), register_name(lane(regs[0], 1)));
                    // an argument in `a` waits in `y`; one in `x` rules `x` out
                    // as the scratch, and the borrow of
                    // target-1 comes from `cmp #1`
                    let (in_a, in_x) = (regs[1..].contains(&reg::A), regs[1..].contains(&reg::X));
                    let (keep, restore) = if in_a { ("  tay\n", "  tya\n") } else { ("", "") };
                    let target = if in_x {
                        format!(
                            "  lda {lo}\n  cmp #1\n  lda {hi}\n  sbc #0\n  pha\n  lda {lo}\n  sec\n  sbc #1\n  pha\n"
                        )
                    } else {
                        let push_low = if cmos { "phx" } else { "txa\n  pha" };
                        format!("  lda {lo}\n  sec\n  sbc #1\n  tax\n  lda {hi}\n  sbc #0\n  pha\n  {push_low}\n")
                    };
                    let name = &function.name;
                    writeln!(
                        out,
                        "{keep}  lda #>(_ci_{name}_{made}-1)\n  pha\n  lda #<(_ci_{name}_{made}-1)\n  pha\n{target}{restore}  rts\n_ci_{name}_{made}:"
                    )
                    .unwrap();
                }
                (Kind::Op("STZM"), [Operand::Symbol(symbol, _)]) => writeln!(out, "  stz {symbol}").unwrap(),
                (Kind::Op("LDM"), [_, Operand::Symbol(symbol, _)]) => writeln!(out, "  lda {symbol}").unwrap(),
                (Kind::Op("STM"), [_, Operand::Symbol(symbol, _)]) => writeln!(out, "  sta {symbol}").unwrap(),
                (Kind::Op("ASL"), _) => out.push_str("  asl a\n"),
                (Kind::Op("LSR"), _) => out.push_str("  lsr a\n"),
                (Kind::Op("ASR"), _) => out.push_str("  cmp #$80\n  ror a\n"),
                // the sign into every bit: the carry out of `asl` is the
                // sign, `sbc #0` turns it into 0 or $FF and `eor` flips it
                (Kind::Op("SPLAT"), _) => out.push_str("  asl a\n  lda #0\n  sbc #0\n  eor #$ff\n"),
                (Kind::Op("ROR"), _) => out.push_str("  ror a\n"),
                // `x >> 7` and `x << 7` of a byte: four bytes, not seven shifts
                (Kind::Op("TOPBIT"), _) => out.push_str("  asl a\n  lda #0\n  rol a\n"),
                (Kind::Op("LOWBIT"), _) => out.push_str("  lsr a\n  lda #0\n  ror a\n"),
                (Kind::Op("LDAX"), [_, Operand::Symbol(symbol, _), _]) => writeln!(out, "  lda {symbol},x").unwrap(),
                (Kind::Op("STAX"), [_, Operand::Symbol(symbol, _), _]) => writeln!(out, "  sta {symbol},x").unwrap(),
                (Kind::Op("SAVEC"), _) => out.push_str("  lda #0\n  rol a\n"),
                (Kind::Op("LOADC"), _) => out.push_str("  lsr a\n"),
                (Kind::Op(step @ ("SHL1" | "SRL1")), _) => {
                    let (first, second) = if step == "SHL1" { ("asl", "rol") } else { ("lsr", "ror") };
                    let (low, high) = (register_name(regs[0]), register_name(regs[2]));
                    let (one, two) = if step == "SHL1" { (low, high) } else { (high, low) };
                    // a run of shifts is shorter as a counted loop (nine bytes,
                    // against four a shift) when an index
                    // register is free to count; it takes 5 clocks a shift and
                    // one more than the run, so it is
                    // chosen where each byte it saves costs no more clocks than
                    // the level's rate
                    let run = block.insts[i..]
                        .iter()
                        .take_while(|next| next.kind == inst.kind && next.operands == inst.operands)
                        .count();
                    let (saved, slower) = (4 * run as i64 - 9, 5 * run as u32 + 1);
                    let pays = saved > 0 && slower <= TUNING.with(|t| t.get()).clocks_per_byte * saved as u32;
                    // the byte that shifts alone rides in `a` for a run: 7
                    // clocks a step where memory is 10 (`asl a; rol hi` against
                    // `asl lo; rol hi`), for the two bytes of the load and
                    // the store; a byte longer under three steps
                    let in_a = run >= if TUNING.with(|t| t.get()).clocks_per_byte > 1 { 5 } else { 3 };
                    match spare {
                        Some(index) if pays => {
                            made += 1;
                            writeln!(out, "  ld{index} #{run}\n_sh{made}:\n  {first} {one}\n  {second} {two}\n  de{index}\n  bne _sh{made}").unwrap();
                            repeated = run - 1;
                        }
                        _ if in_a && !a_live => {
                            writeln!(out, "  lda {one}").unwrap();
                            for _ in 0..run {
                                writeln!(out, "  {first} a\n  {second} {two}").unwrap();
                            }
                            writeln!(out, "  sta {one}").unwrap();
                            repeated = run - 1;
                        }
                        _ => writeln!(out, "  {first} {one}\n  {second} {two}").unwrap(),
                    }
                }
                // `lda give; asl|lsr a` leaves the bit in the carry; the word
                // rotates it in
                (Kind::Op(step @ ("FSHL1" | "FSHR1")), _) => {
                    let (shift, first, second) =
                        if step == "FSHL1" { ("asl", "rol", "rol") } else { ("lsr", "ror", "ror") };
                    match regs[1] {
                        r if r == reg::A => {}
                        r if r == reg::X => out.push_str("  txa\n"),
                        r if r == reg::Y => out.push_str("  tya\n"),
                        r => writeln!(out, "  lda {}", register_name(r)).unwrap(),
                    }
                    writeln!(
                        out,
                        "  {shift} a\n  {first} {}\n  {second} {}",
                        register_name(regs[2]),
                        register_name(regs[4])
                    )
                    .unwrap();
                }
                (Kind::Op("SRA1"), _) => {
                    let (lo, hi) = (register_name(regs[1]), register_name(regs[3]));
                    writeln!(out, "  lda {hi}\n  cmp #$80\n  ror {hi}\n  ror {lo}").unwrap()
                }
                (Kind::Op("CMPZ"), [_, _, _, _, low, high]) => {
                    made += 1;
                    let load = |out: &mut Asm, r: RegId| match r {
                        r if r == reg::X => out.push_str("  txa\n"),
                        r if r == reg::Y => out.push_str("  tya\n"),
                        r => writeln!(out, "  lda {}", register_name(r)).unwrap(),
                    };
                    load(&mut out, regs[2]);
                    match (low, high) {
                        // against zero the loads set the flag: both bytes zero
                        // is `lda lo; ora hi`
                        (Operand::Imm(0), Operand::Imm(0)) if regs[3] != reg::X && regs[3] != reg::Y => {
                            writeln!(out, "  ora {}", register_name(regs[3])).unwrap();
                        }
                        (Operand::Imm(0), Operand::Imm(0)) => {
                            writeln!(out, "  bne _cz{made}").unwrap();
                            load(&mut out, regs[3]);
                            writeln!(out, "_cz{made}:").unwrap();
                        }
                        (Operand::Imm(low), Operand::Imm(high)) => {
                            writeln!(out, "  cmp #{low}\n  bne _cz{made}").unwrap();
                            load(&mut out, regs[3]);
                            writeln!(out, "  cmp #{high}\n_cz{made}:").unwrap();
                        }
                        _ => {
                            writeln!(out, "  cmp {}\n  bne _cz{made}", register_name(regs[4])).unwrap();
                            load(&mut out, regs[3]);
                            writeln!(out, "  cmp {}\n_cz{made}:", register_name(regs[5])).unwrap();
                        }
                    }
                }
                (Kind::Op("INC" | "DEC"), _) => {
                    out.push_str(&form_text(inst.kind, &inst.operands, &regs, cmos, &slot_name).unwrap())
                }
                (Kind::Op("UNREACHABLE"), _) => {}
                (Kind::Op("CLC"), _) => out.push_str("  clc\n"),
                (Kind::Op("SEC"), _) => out.push_str("  sec\n"),
                (Kind::Op("ADC" | "SBC" | "AND" | "ORA" | "EOR"), [_, _, _, ..]) => {
                    out.push_str(&form_text(inst.kind, &inst.operands, &regs, cmos, &slot_name).unwrap())
                }
                (Kind::Op("RTS"), _) => {
                    // on the stack the frame is released last: the result
                    // copies after the epilogue may still reload spills
                    let jumped = i.checked_sub(1).is_some_and(|before| tail_call(block, before, pending));
                    let release = pending.take().or((spilled > 0 && !has_frame).then_some(spilled));
                    if let Some(n) = release {
                        out.push_str("  pha\n");
                        adjust(&mut out, n);
                        out.push_str("  pla\n");
                    }
                    if !jumped {
                        out.push_str("  rts\n");
                    }
                }
                (Kind::Op("ADJSP"), [_, Operand::Imm(n)]) => {
                    let n = i64::from(*n as u16 as i16);
                    if n > 0 && spilled > 0 {
                        pending = Some(n + spilled);
                    } else {
                        // an argument still in `a` (never copied out) waits on
                        // the stack
                        if a_live {
                            out.push_str("  pha\n");
                        }
                        adjust(&mut out, if n < 0 { n - spilled } else { n });
                        if a_live {
                            out.push_str("  pla\n");
                        }
                    }
                }
                (Kind::Op("LEA"), [_, _, _, Operand::Imm(n)]) => {
                    let n = i64::from(*n as u16);
                    let n = if n >= old { n + spilled } else { n };
                    // a low byte of 0 adds nothing and carries nothing
                    if n & 0xFF == 0 {
                        writeln!(out, "  lda rc0").unwrap();
                        move_byte(&mut out, regs[0], reg::A, false, None, None);
                        if n >> 8 == 0 {
                            writeln!(out, "  lda rc1").unwrap();
                        } else {
                            writeln!(out, "  clc\n  lda rc1\n  adc #{}", (n >> 8) & 0xFF).unwrap();
                        }
                    } else {
                        writeln!(out, "  clc\n  lda rc0\n  adc #{}", n & 0xFF).unwrap();
                        move_byte(&mut out, regs[0], reg::A, false, None, None);
                        writeln!(out, "  lda rc1\n  adc #{}", (n >> 8) & 0xFF).unwrap();
                    }
                    move_byte(&mut out, regs[1], reg::A, false, None, None);
                }
                // against zero, a load or an ALU result has already set the
                // flags a zero or sign test reads
                (Kind::Op("CMP"), [_, _, Operand::Imm(0)])
                    if regs[1] == reg::A
                        && block.insts.get(i + 1).is_some_and(reads_zero_or_sign_only)
                        && out.last_inst().is_some_and(crate::peephole::sets_nz_of_a) =>
                {
                    // `iny; tya; bne`: the step set the flags of y itself
                    // DEVIATE (6502-3): looks back at the emitted
                    // instructions. Exit: the peephole on allocated MIR (D-6502
                    // of #6259, see peephole.rs).
                    let n = out.lines.len();
                    let stepped = n >= 2
                        && matches!(
                            (&out.lines[n - 2], &out.lines[n - 1]),
                            (
                                Line::Inst(AsmInst { op: AsmOp::Iny | AsmOp::Dey, .. }),
                                Line::Inst(AsmInst { op: AsmOp::Tya, .. })
                            ) | (
                                Line::Inst(AsmInst { op: AsmOp::Inx | AsmOp::Dex, .. }),
                                Line::Inst(AsmInst { op: AsmOp::Txa, .. })
                            )
                        );
                    if !a_live && stepped {
                        out.pop();
                    }
                }
                // the register the left side is in decides the opcode
                (Kind::Op("CMP"), [_, _, _, ..]) => {
                    out.push_str(&form_text(inst.kind, &inst.operands, &regs, cmos, &slot_name).unwrap())
                }
                (Kind::Op(branch @ ("BEQ" | "BNE" | "BCC" | "BCS" | "BMI" | "BPL")), [to, _]) => {
                    writeln!(out, "  {} {}", branch.to_lowercase(), target(to)).unwrap();
                }
                // a jump to the block that follows is the layout
                (Kind::Op(jump @ ("JMP" | "BRA")), [to @ Operand::Block(id)]) => {
                    if function.blocks.get(b + 1).map(|next| next.id) != Some(*id) {
                        writeln!(out, "  {} {}", jump.to_lowercase(), target(to)).unwrap();
                    }
                }
                // a slot is a byte, or two for a pair
                // a callee-saved cell on the hardware stack
                // (see `save_preserved`); the restore keeps the result in `a`
                // safe in `y`, which no result uses
                (Kind::Op("PUSHC"), _) => {
                    // an argument still in `a` waits in an index register
                    // nothing holds
                    let keep = [(reg::Y, "y"), (reg::X, "x")].into_iter().find(|(r, _)| !after[b][i].contains(r));
                    match (a_live, keep) {
                        (false, _) => writeln!(out, "  lda {}\n  pha", register_name(regs[0])).unwrap(),
                        (true, Some((_, r))) => {
                            writeln!(out, "  ta{r}\n  lda {}\n  pha\n  t{r}a", register_name(regs[0])).unwrap()
                        }
                        (true, None) => return Err("a is live where a callee-saved cell is pushed".into()),
                    }
                }
                (Kind::Op("CSAVE"), _) => {
                    if after[b][i].contains(&reg::Y) || after[b][i].contains(&reg::X) || a_live {
                        // `__csave` changes a, x and y: the same bytes pushed
                        // one at a time, the argument
                        // in `a` waiting in an index register nothing holds
                        let keep = [(reg::Y, "y"), (reg::X, "x")].into_iter().find(|(r, _)| !after[b][i].contains(r));
                        if a_live && keep.is_none() {
                            return Err("a register is live where the callee-saved cells are saved".into());
                        }
                        if let (true, Some((_, r))) = (a_live, keep) {
                            writeln!(out, "  ta{r}").unwrap();
                        }
                        for r in &regs {
                            writeln!(out, "  lda {}\n  pha", register_name(*r)).unwrap();
                        }
                        if let (true, Some((_, r))) = (a_live, keep) {
                            writeln!(out, "  t{r}a").unwrap();
                        }
                    } else {
                        writeln!(out, "  ldy #{}\n  jsr __csave", regs.len()).unwrap();
                    }
                }
                // the restore is the function's return: `jmp` to the routine
                // that pops and returns, unless x holds a result, when the
                // cells are popped here
                (Kind::Op("CREST"), _) if !after[b][i].contains(&reg::X) && !after[b][i].contains(&reg::Y) => {
                    if a_live {
                        out.push_str("  tay\n");
                    }
                    writeln!(out, "  ldx #{}\n  jmp __crest", regs.len()).unwrap();
                    repeated = 1;
                }
                (Kind::Op("CREST"), _) => {
                    if a_live && after[b][i].contains(&reg::Y) {
                        return Err("a and y are live where callee-saved cells are popped".into());
                    }
                    if a_live {
                        out.push_str("  tay\n");
                    }
                    for r in &regs {
                        writeln!(out, "  pla\n  sta {}", register_name(*r)).unwrap();
                    }
                    if a_live {
                        out.push_str("  tya\n");
                    }
                }
                (Kind::Op("POPS"), _) => {
                    if a_live && after[b][i].contains(&reg::Y) {
                        return Err("a and y are live where callee-saved cells are popped".into());
                    }
                    if a_live {
                        out.push_str("  tay\n");
                    }
                    for r in &regs {
                        writeln!(out, "  pla\n  sta {}", register_name(*r)).unwrap();
                    }
                    if a_live {
                        out.push_str("  tya\n");
                    }
                }
                (Kind::Op(kind @ ("SPILL" | "RELOAD")), [_, Operand::Frame(slot), ..]) if STACK => {
                    let register = regs[0];
                    let lanes: Vec<RegId> =
                        if is_pair(register) { vec![lane(register, 0), lane(register, 1)] } else { vec![register] };
                    for (n, byte) in lanes.into_iter().enumerate() {
                        let offset = slot_offset(*slot) + n as i64;
                        if offset > 255 {
                            return Err(format!("{}: a frame beyond 256 bytes", function.name));
                        }
                        writeln!(out, "  ldy #{offset}").unwrap();
                        let name = register_name(byte);
                        let direct = byte == reg::A;
                        let body = match (kind, byte) {
                            ("SPILL", b) if b == reg::A => "  sta (rc0),y\n".to_owned(),
                            ("SPILL", b) if b == reg::X => "  txa\n  sta (rc0),y\n".to_owned(),
                            ("SPILL", _) => format!("  lda {name}\n  sta (rc0),y\n"),
                            (_, b) if b == reg::A => "  lda (rc0),y\n".to_owned(),
                            (_, b) if b == reg::X => "  lda (rc0),y\n  tax\n".to_owned(),
                            (_, _) => format!("  lda (rc0),y\n  sta {name}\n"),
                        };
                        if direct {
                            out.push_str(&body);
                        } else {
                            through_a(&mut out, a_live, None, &body);
                        }
                    }
                }
                (Kind::Op(kind @ ("SPILL" | "RELOAD")), [_, Operand::Frame(slot)]) => {
                    let register = regs[0];
                    let lanes: Vec<RegId> =
                        if is_pair(register) { vec![lane(register, 0), lane(register, 1)] } else { vec![register] };
                    for (n, byte) in lanes.into_iter().enumerate() {
                        let cell = if n == 0 { slot_name(*slot) } else { format!("{}+{n}", slot_name(*slot)) };
                        let direct = byte == reg::A || byte == reg::X || byte == reg::Y;
                        let name = register_name(byte);
                        match (kind, direct) {
                            ("SPILL", true) => writeln!(out, "  st{name} {cell}").unwrap(),
                            ("SPILL", false) => {
                                through_a(&mut out, a_live, spare, &format!("  lda {name}\n  sta {cell}\n"))
                            }
                            (_, true) => writeln!(out, "  ld{name} {cell}").unwrap(),
                            (_, false) => through_a(&mut out, a_live, spare, &format!("  lda {cell}\n  sta {name}\n")),
                        }
                    }
                }
                (other, _) => return Err(format!("{other:?} has no expansion")),
            }
        }
        starts[b].push(out.lines.len());
    }
    if std::env::var_os("LLRM_6502_CHECK_FLAGS").is_some() {
        crate::flagcheck::verify(&function, &starts, &out.lines)?;
    }
    let slots: Vec<(String, usize)> =
        if STACK { Vec::new() } else { slots.iter().map(|(slot, class)| (slot_name(*slot), size_of(class))).collect() };
    let returns_a = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter(|inst| inst.kind == Kind::Op("RTS"))
        .flat_map(|inst| &inst.operands)
        .any(|operand| matches!(operand, Operand::Use(Reg::Phys(r)) if *r == reg::A));
    // `jsr callee` reads `a` when the call lists it among the arguments
    let mut a_callees = std::collections::BTreeSet::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts).filter(|inst| inst.kind == Kind::Op("JSR")) {
        if let [Operand::Symbol(callee, _), arguments @ ..] = &inst.operands[..]
            && arguments.iter().any(|operand| matches!(operand, Operand::Use(Reg::Phys(r)) if *r == reg::A))
        {
            a_callees.insert(callee.clone());
            a_callees.insert(format!("{callee}_fast"));
        }
    }
    let returns_x = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter(|inst| inst.kind == Kind::Op("RTS"))
        .flat_map(|inst| &inst.operands)
        .any(|operand| matches!(operand, Operand::Use(Reg::Phys(r)) if *r == reg::X));
    // `jsr callee` reads `x` when the call lists it among the arguments
    let mut x_callees = std::collections::BTreeSet::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts).filter(|inst| inst.kind == Kind::Op("JSR")) {
        if let [Operand::Symbol(callee, _), arguments @ ..] = &inst.operands[..]
            && arguments.iter().any(|operand| matches!(operand, Operand::Use(Reg::Phys(r)) if *r == reg::X))
        {
            x_callees.insert(callee.clone());
            x_callees.insert(format!("{callee}_fast"));
        }
    }
    let returns_y = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter(|inst| inst.kind == Kind::Op("RTS"))
        .flat_map(|inst| &inst.operands)
        .any(|operand| matches!(operand, Operand::Use(Reg::Phys(r)) if *r == reg::Y));
    // `jsr callee` reads `y` when the call lists it among the arguments
    let mut y_callees = std::collections::BTreeSet::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts).filter(|inst| inst.kind == Kind::Op("JSR")) {
        if let [Operand::Symbol(callee, _), arguments @ ..] = &inst.operands[..]
            && arguments.iter().any(|operand| matches!(operand, Operand::Use(Reg::Phys(r)) if *r == reg::Y))
        {
            y_callees.insert(callee.clone());
            y_callees.insert(format!("{callee}_fast"));
        }
    }
    // the cells a return reads: the bytes of a long result
    let mut result_cells = std::collections::BTreeSet::new();
    for operand in function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter(|inst| inst.kind == Kind::Op("RTS"))
        .flat_map(|inst| &inst.operands)
    {
        if let Operand::Use(Reg::Phys(r)) = operand {
            if is_pair(*r) {
                result_cells.insert(register_name(lane(*r, 0)));
                result_cells.insert(register_name(lane(*r, 1)));
            } else {
                result_cells.insert(register_name(*r));
            }
        }
    }
    let lines_of = source.map(|(file, scope)| (file, scope, &function.places[..]));
    out.cmos = cmos;
    let tails = ((!returns_a, a_callees), (!returns_x, x_callees), (!returns_y, y_callees), result_cells);
    let (priced, text) = finish_pair(out, lines_of, tails);
    Ok((own_labels(&text, &function.name), own_labels(&priced, &function.name), slots, sunk, shared, moved))
}

/// The labels the emitter numbers from 1 in every function (`_pa1`, `_cmt1`,
/// `_far1`) get the function's name, so two functions in one file do not define
/// one label twice.
fn own_labels(
    text: &str,
    name: &str,
) -> String {
    const KINDS: [&str; 6] = ["_pa", "_cmt", "_cmd", "_cz", "_sh", "_far"];
    let mut out = String::with_capacity(text.len() + 64);
    let mut rest = text;
    while let Some(at) = rest.find('_') {
        let (before, here) = rest.split_at(at);
        out.push_str(before);
        let boundary = before.chars().last().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let kind = KINDS
            .iter()
            .find(
                |kind| here.strip_prefix(**kind).is_some_and(|after| after.starts_with(|c: char| c.is_ascii_digit())),
            );
        match kind {
            Some(kind) if boundary => {
                out.push_str(name);
                out.push_str(kind);
                rest = &here[kind.len()..];
            }
            _ => {
                out.push('_');
                rest = &here[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The function's text twice: priced (the transfer of a known constant left
/// out, no line rows) and kept. The peepholes before the first that tells them
/// apart run once.
fn finish_pair(
    mut code: Asm,
    lines_of: Option<(&str, Option<u32>, &[llrm_codegen::model::lir::SrcLoc])>,
    ((no_a_result, a_callees), (no_x_result, x_callees), (no_y_result, y_callees), result_cells): (
        (bool, std::collections::BTreeSet<String>),
        (bool, std::collections::BTreeSet<String>),
        (bool, std::collections::BTreeSet<String>),
        std::collections::BTreeSet<String>,
    ),
) -> (String, String) {
    code.no_a_result = no_a_result;
    code.a_callees = a_callees;
    code.no_x_result = no_x_result;
    code.x_callees = x_callees;
    code.no_y_result = no_y_result;
    code.y_callees = y_callees;
    code.result_cells = result_cells;
    let (mut priced, mut kept) = crate::peephole::run_pair(&code);
    crate::relax::relax(&mut priced);
    crate::relax::relax(&mut kept);
    if let Some((file, scope, places)) = lines_of {
        kept.lines = with_lines(kept.lines, file, scope, places);
    }
    (priced.text(), kept.text())
}

/// `.dbg line` ahead of each instruction whose line is not the one before it
/// (ca65 writes them to the debug file); a label starts a line again. They go
/// in after every pass, so no pass sees them.
fn with_lines(
    lines: Vec<Line>,
    file: &str,
    scope: Option<u32>,
    places: &[llrm_codegen::model::lir::SrcLoc],
) -> Vec<Line> {
    let mut out = Vec::with_capacity(lines.len() * 2);
    let mut shown = None;
    // The function's first row is the line it starts at, before any
    // instruction's (`DwarfDebug::beginFunctionImpl`).
    let mut scope = scope;
    for line in lines {
        match &line {
            Line::Label(_) => shown = None,
            Line::Inst(inst) => {
                if let Some(line) = scope.take() {
                    out.push(Line::Raw(format!("  .dbg line, \"{file}\", {line}")));
                    shown = Some(line);
                }
                // DEVIATE: place 0 (no location) writes no row, so the row
                // before covers it; the pin gives it line 0.
                // Reason: ca65 keeps a `.dbg line` of line 0 as
                // a span-less row (probe 2026-11-05),
                // so it would cover nothing either. Exit: with ca65's handling.
                if let Some(place) = (inst.place != 0).then(|| places.get(inst.place as usize - 1)).flatten()
                    && shown != Some(place.line)
                {
                    out.push(Line::Raw(format!("  .dbg line, \"{file}\", {}", place.line)));
                    shown = Some(place.line);
                }
            }
            Line::Raw(_) => {}
        }
        out.push(line);
    }
    out
}

/// Whether `LLRM_6502_SPIKE` asks for the spike graph-colouring allocator, to
/// measure against.
fn spike_asked() -> bool {
    SPIKE.with(|one| one.get()) || std::env::var_os("LLRM_6502_SPIKE").is_some()
}

/// `f` with the spike allocator doing the allocation: the tests of its spill
/// policy (a slot, not a callee-saved cell) run on it until it is deleted.
pub fn with_spike_allocator<R>(f: impl FnOnce() -> R) -> R {
    let before = SPIKE.with(|one| one.replace(true));
    let result = f();
    SPIKE.with(|one| one.set(before));
    result
}

/// The function allocated: by the shared greedy allocator, which this target
/// moves onto (`Target::register_allocator`), else by the spike allocator.
///
/// A function greedy cannot take is an error, not a quiet fall back to the
/// spike allocator: that hid 600 failures in 300 differential programs.
fn allocate<const STACK: bool>(
    function: &mut MFunction<Class>
) -> Result<(llrm_spike_mf::regalloc::Assignment, Vec<(i32, Class)>), String> {
    // local bisection of a miscompile: `LLRM_6502_GREEDY_ONLY=f,g` gives greedy
    // only those functions; `LLRM_6502_GREEDY_ONLY=?` prints the names
    let only = std::env::var("LLRM_6502_GREEDY_ONLY").ok();
    if only.as_deref() == Some("?") {
        eprintln!("FUNCTION {}", function.name);
    }
    let wanted = only.as_deref().is_none_or(|names| names == "?" || names.split(',').any(|n| n == function.name));
    if std::env::var("LLRM_6502_DUMP").is_ok_and(|name| name == function.name) {
        for (at, block) in function.blocks.iter().enumerate() {
            for (i, inst) in block.insts.iter().enumerate() {
                eprintln!(
                    "in bb{at}.{i}: {:?} {:?}{}{}",
                    inst.kind,
                    inst.operands,
                    if inst.sets_flags() { " SF" } else { "" },
                    if inst.reads_flags() { " RF" } else { "" }
                );
            }
        }
    }
    if spike_asked() || !wanted {
        return allocate_spilling::<Mos<STACK>>(function, 0);
    }
    allocate_greedy::<Mos<STACK>>(function)
}

/// `function` allocated by greedy (`llrm_spike_mf::tolir::allocate_greedy`).
fn allocate_greedy<T: Target<Class = Class>>(
    function: &mut MFunction<Class>
) -> Result<(llrm_spike_mf::regalloc::Assignment, Vec<(i32, Class)>), String> {
    use llrm_lir::registers::Regs;
    let size = TUNING.with(|t| t.get()) == Tuning::SIZE;
    let (assignment, slots) = llrm_spike_mf::tolir::allocate_greedy::<T>(function, Regs(&crate::REGISTER_INFO), size)?;
    if std::env::var("LLRM_6502_DUMP").is_ok_and(|name| name == function.name) {
        for (at, block) in function.blocks.iter().enumerate() {
            for (i, inst) in block.insts.iter().enumerate() {
                eprintln!(
                    "bb{at}.{i}: {:?} {:?}{}{}",
                    inst.kind,
                    inst.operands,
                    if inst.sets_flags() { " SF" } else { "" },
                    if inst.reads_flags() { " RF" } else { "" }
                );
            }
        }
    }
    if std::env::var_os("LLRM_6502_CHECK").is_some() {
        llrm_spike_mf::regalloc::GraphColoring::<T>::by_frequency(false).check(function, &assignment)?;
    }
    Ok((assignment, slots))
}

/// `code` without its `.dbg line` directives, which cost nothing.
fn without_lines(code: &str) -> String {
    code.lines().filter(|line| !line.trim_start().starts_with(".dbg")).map(|line| format!("{line}\n")).collect()
}
