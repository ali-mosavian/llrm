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

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::{successors, two_address};
use llrm_spike_mf::spill::allocate_spilling;
use llrm_spike_mf::{
    Block, Inst, Jump, Kind, MFunction, Operand, Reg, Target, branch_fold, forward_reloads, machine_sink_in_place,
    remove_identity_copies, remove_redundant_copies_between, reschedule_kills,
};

use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::hooks::Cpu;
use crate::emit::{MInst, Operand as Machine, address, spill};
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

/// The zero-page byte of a pair: lane 0 is the low byte.
fn lane(
    pair: RegId,
    lane: u8,
) -> RegId {
    rc(2 * (pair.index() as u8 - reg::RS0.index() as u8) + lane)
}

fn is_cell(register: RegId) -> bool {
    register.index() >= reg::RC0.index()
}

/// The target; `STACK` keeps spills on the soft stack (a reentrant function)
/// instead of in static cells, which costs `y` at each spill.
pub struct Mos<const STACK: bool>;

impl<const STACK: bool> Target for Mos<STACK> {
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

    /// In stack mode a spill is `ldy #offset; sta (rc0),y`: it overwrites `y`.
    fn reads_flags_set_just_before(inst: &Inst) -> bool {
        inst.reads_flags && (inst.terminator || inst.kind == Kind::Op("CMOV"))
    }

    fn moves_lanes(inst: &Inst) -> bool {
        matches!(inst.kind, Kind::Op("PACK" | "UNPACK"))
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
    bytes: HashMap<String, Reg>,
    /// Word cells used as pointers: one pair register, not two bytes.
    pointers: HashSet<String>,
    /// Pointers written as two bytes, packed into their pair after the
    /// instruction.
    packs: Vec<(Reg, Reg, Reg)>,
    /// Cells defined once, by an immediate load: they fold into the user.
    constants: HashMap<String, i64>,
    /// The block each label names.
    block_of: HashMap<String, u32>,
    /// How many times each cell or register has been defined so far.
    versions: HashMap<String, u32>,
    /// The compare whose flags the next branch reads, and the ones saved to
    /// cells for later.
    pending: Option<Compare>,
    saved: HashMap<String, Compare>,
    /// Compares are redone where the flags are read: nothing keeps `P` alive.
    /// The block instructions go to.
    current: usize,
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
}

impl Builder {
    fn fresh(&mut self) -> Reg {
        self.function.classes.push(Class::ANY);
        Reg::Virt(self.function.classes.len() as u32 - 1)
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
        if let Some(r) = self.bytes.get(name) {
            return Ok(*r);
        }
        let r = self.fresh();
        self.bytes.insert(name.to_owned(), r);
        Ok(r)
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
        self.pointers.contains(name) || matches!(name, "rs1" | "rs2" | "rs3")
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
            Reg::Phys(p) if is_cell(p) => Ok(()),
            Reg::Phys(_) => Err(Unsupported("a register as a memory operand".into())),
        }
    }

    /// `name` is defined here.
    fn define(
        &mut self,
        name: &str,
    ) {
        *self.versions.entry(name.to_owned()).or_default() += 1;
    }

    fn version(
        &self,
        name: &str,
    ) -> u32 {
        self.versions.get(name).copied().unwrap_or(0)
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

    /// `COPY a, lhs; CMP a, rhs` again, as the flags' producer: the operands
    /// must hold what they held at the first compare.
    ///
    /// `code` is the condition its reader tests (`cond`): MI and PL against a
    /// word zero read the sign of the high byte, and the low byte and the
    /// borrow cannot change it.
    fn redo_compare(
        &mut self,
        code: i64,
    ) -> Result<(), Unsupported> {
        let Compare { a, b, word, zero } =
            self.pending.clone().ok_or_else(|| Unsupported("flags with no compare".into()))?;
        if self.version(&a.0) != a.1 || self.version(&b.0) != b.1 {
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
            self.copy(acc, ah);
            let mut test = Inst::new(
                Kind::Op("CMP"),
                vec![Operand::Def(Reg::Phys(reg::FLAGS)), Operand::Use(acc), Operand::Imm(0)],
            );
            test.sets_flags = true;
            self.push(test);
            return Ok(());
        }
        let (lhs, rhs) = if word {
            let (al, ah) = self.read_word(&a.0)?;
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
            ([al, ah], rhs)
        } else {
            let lhs = self.byte(&a.0)?;
            let rhs = match self.constants.get(&b.0) {
                Some(value) => Operand::Imm(*value & 0xFF),
                None => {
                    let r = self.byte(&b.0)?;
                    self.require(r, Class::MEM)?;
                    Operand::Use(r)
                }
            };
            ([lhs, lhs], [rhs.clone(), rhs])
        };
        self.copy(acc, lhs[0]);
        let mut cmp =
            Inst::new(Kind::Op("CMP"), vec![Operand::Def(Reg::Phys(reg::FLAGS)), Operand::Use(acc), rhs[0].clone()]);
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
        inst.sets_flags |= matches!(inst.kind, Kind::Op("LDI" | "LDS"));
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
        let acc = Reg::Phys(reg::A);
        self.define(d);
        let (d, a) = (self.byte(d)?, self.byte(a)?);
        let second = match self.constants.get(b) {
            Some(value) => Operand::Imm(*value & 0xFF),
            None => {
                let b = self.byte(b)?;
                self.require(b, Class::MEM)?;
                Operand::Use(b)
            }
        };
        if is_identity(mnemonic, &second) {
            self.copy(d, a);
            return Ok(());
        }
        if let Some(carry) = carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        self.copy(acc, a);
        self.push(Inst::new(Kind::Op(mnemonic), vec![Operand::Def(acc), Operand::Use(acc), second]));
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
        if let Some(carry) = carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        for ((d, a), b) in [(dl, al), (dh, ah)].into_iter().zip(second) {
            if is_identity(mnemonic, &b) {
                self.copy(d, a);
                continue;
            }
            self.copy(acc, a);
            self.push(Inst::new(Kind::Op(mnemonic), vec![Operand::Def(acc), Operand::Use(acc), b]));
            self.copy(d, acc);
        }
        Ok(())
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
/// A pointer stays one through a copy and through an add of a constant 1..=255
/// (`p++`, `p += k`), in both directions, so a pointer walked round a loop is
/// bumped in its pair instead of unpacked, added and packed every trip.
fn pointer_cells(
    items: &[Item],
    constants: &HashMap<String, i64>,
) -> HashSet<String> {
    let mut out = pointer_bases(items);
    let constant = |cell: &String| {
        let low = constants.get(cell).copied();
        let high = constants.get(&format!("{cell}+1")).copied();
        matches!((low, high), (Some(1..=255), Some(0)))
    };
    // a cell read or written as plain bytes anywhere stays a word cell
    let mut plain: HashSet<String> = HashSet::new();
    for item in items {
        let Item::Inst(MInst { opcode, operands }) = item else { continue };
        for (at, operand) in operands.iter().enumerate() {
            let Machine::Reg(cell) = operand else { continue };
            let word = cell.strip_suffix("+1").unwrap_or(cell);
            let pointer_use = cell == word
                && match *opcode {
                    op::COPY16 => true,
                    op::ADD16 => matches!(&operands[2], Machine::Reg(b) if constant(b)),
                    op::LDP8 | op::LDP16 | op::STP8 | op::STP16 | op::LDPI8 | op::STPI8 | op::LDPZ | op::STPZ => {
                        at == 1
                    }
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
            let Item::Inst(MInst { opcode, operands }) = item else { continue };
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
    let mut out = HashSet::new();
    for item in items {
        let Item::Inst(MInst { opcode, operands }) = item else { continue };
        let at = match *opcode {
            op::LDP8 | op::LDP16 | op::STP8 | op::STP16 | op::LDPI8 | op::STPI8 | op::LDPZ | op::STPZ => 1,
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
    let mut defs: HashMap<String, usize> = HashMap::new();
    let mut values = HashMap::new();
    for item in items {
        let Item::Inst(MInst { opcode, operands }) = item else { continue };
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
    // a byte copied once from a constant is that constant (the zero high half
    // of a widened byte)
    for item in items {
        let Item::Inst(MInst { opcode: op::COPY8, operands }) = item else { continue };
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
    // A block starts at a label that follows an instruction (or the first).
    let mut labels: Vec<Vec<String>> = vec![Vec::new()];
    let mut block_of = HashMap::new();
    let mut after_inst = false;
    for item in items {
        match item {
            Item::Label(l) => {
                if after_inst {
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
        function: MFunction::new(name_of_function),
        bytes: HashMap::new(),
        pointers: pointer_cells(items, &constants(items)),
        packs: Vec::new(),
        constants: constants(items),
        block_of,
        versions: HashMap::new(),
        pending: None,
        saved: HashMap::new(),
        current: 0,
    };
    for id in 0..labels.len() {
        b.function.blocks.push(Block { id: id as u32, ..Block::default() });
    }
    let mut after_inst = false;
    for item in items {
        let inst = match item {
            Item::Label(_) => {
                if after_inst {
                    b.current += 1;
                    after_inst = false;
                    b.pending = None;
                }
                continue;
            }
            // the flags are not kept across the phi copies: a compare is redone
            // at its branch
            Item::Text(text) if matches!(text.as_str(), "  php\n" | "  plp\n") => continue,
            Item::Text(_) => return Err(Unsupported("raw text".into())),
            Item::Inst(inst) => inst,
        };
        after_inst = true;
        let o = &inst.operands;
        match inst.opcode {
            op::COPY8 => {
                b.define(name(&o[0])?);
                let (d, s) = (b.byte(name(&o[0])?)?, b.byte(name(&o[1])?)?);
                b.copy(d, s);
            }
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
            op::ADD16 => b.alu16("ADC", Some("CLC"), name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::SUB16 => b.alu16("SBC", Some("SEC"), name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::AND16 => b.alu16("AND", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::OR16 => b.alu16("ORA", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::XOR16 => b.alu16("EOR", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::SHL8 | op::SRL8 | op::SRA8 => {
                let step = match inst.opcode {
                    op::SHL8 => "ASL",
                    op::SRL8 => "LSR",
                    _ => "ASR",
                };
                let acc = Reg::Phys(reg::A);
                b.define(name(&o[0])?);
                let (d, a) = (b.byte(name(&o[0])?)?, b.byte(name(&o[1])?)?);
                b.copy(acc, a);
                b.shift_a(step, immediate(&o[2])?);
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
            op::LDAX | op::STAX => {
                let Machine::Label(symbol) = &o[1] else { return Err(Unsupported("an indexed access".into())) };
                let (acc, x) = (Reg::Phys(reg::A), Reg::Phys(reg::X));
                let index = b.byte(name(&o[2])?)?;
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
                });
            }
            op::CMPZ16 => {
                let (a, c) = (name(&o[1])?, name(&o[2])?);
                b.pending = Some(Compare {
                    a: (a.to_owned(), b.version(a)),
                    b: (c.to_owned(), b.version(c)),
                    word: true,
                    zero: true,
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
            op::SETCC8 => {
                b.redo_compare(immediate(&o[1])?)?;
                b.define(name(&o[0])?);
                let d = b.byte(name(&o[0])?)?;
                let mut set = Inst::new(
                    Kind::Op("CMOV"),
                    vec![
                        Operand::Def(Reg::Phys(reg::A)),
                        Operand::Imm(0),
                        Operand::Imm(1),
                        Operand::Imm(immediate(&o[1])?),
                        Operand::Use(Reg::Phys(reg::FLAGS)),
                    ],
                );
                set.reads_flags = true;
                b.push(set);
                b.copy(d, Reg::Phys(reg::A));
            }
            // 8 or more: one byte moves across, shifted by the rest of the
            // count
            code @ (op::SHL16 | op::SRL16 | op::SRA16) if (8..16).contains(&immediate(&o[2])?) => {
                let rest = immediate(&o[2])? - 8;
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
                    _ => {
                        shift(&mut b, ah, dl, "ASR", rest);
                        shift(&mut b, ah, dh, "ASR", 7);
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
            op::CMP16 => {
                let (a, c) = (name(&o[1])?, name(&o[2])?);
                b.pending = Some(Compare {
                    a: (a.to_owned(), b.version(a)),
                    b: (c.to_owned(), b.version(c)),
                    word: true,
                    zero: false,
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
            code @ (op::BEQ | op::BNE | op::BCC | op::BCS | op::BMI | op::BPL) => {
                let condition = [op::BEQ, op::BNE, op::BCC, op::BCS, op::BMI, op::BPL].iter().position(|c| *c == code);
                b.redo_compare(condition.map_or(-1, |c| c as i64))?;
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
            op::CMOV8 => {
                b.redo_compare(immediate(&o[3])?)?;
                b.define(name(&o[0])?);
                let d = b.byte(name(&o[0])?)?;
                let value = |b: &mut Builder, operand: &Machine| -> Result<Operand, Unsupported> {
                    match b.constants.get(name(operand)?) {
                        Some(v) => Ok(Operand::Imm(*v & 0xFF)),
                        None => {
                            let r = b.byte(name(operand)?)?;
                            b.require(r, Class(Class::X | Class::Y | Class::M))?;
                            Ok(Operand::Use(r))
                        }
                    }
                };
                let (if_false, if_true) = (value(&mut b, &o[1])?, value(&mut b, &o[2])?);
                let mut cmov = Inst::new(
                    Kind::Op("CMOV"),
                    vec![
                        Operand::Def(Reg::Phys(reg::A)),
                        if_false,
                        if_true,
                        Operand::Imm(immediate(&o[3])?),
                        Operand::Use(Reg::Phys(reg::FLAGS)),
                    ],
                );
                cmov.reads_flags = true;
                b.push(cmov);
                b.copy(d, Reg::Phys(reg::A));
            }
            // d := cc ? t : f, as a pair `n` that starts as `f` (a copy the
            // allocator joins with `f`'s register when it can) and takes `t`
            // on the taken path
            op::CMOV16 => {
                let (fl, fh) = b.read_word(name(&o[1])?)?;
                let (tl, th) = b.read_word(name(&o[2])?)?;
                let (nl, nh) = (b.fresh(), b.fresh());
                b.copy(nl, fl);
                b.copy(nh, fh);
                b.redo_compare(immediate(&o[3])?)?;
                for r in [nl, nh, tl, th] {
                    b.require(r, Class(Class::X | Class::Y | Class::M))?;
                }
                let (dl, dh) = b.write_word(name(&o[0])?)?;
                let mut operands = vec![Operand::Def(nl), Operand::Def(nh), Operand::Def(Reg::Phys(reg::A))];
                operands.extend([nl, nh, tl, th].map(Operand::Use));
                operands.extend([Operand::Imm(immediate(&o[3])?), Operand::Use(Reg::Phys(reg::FLAGS))]);
                let mut cmov = Inst::new(Kind::Op("CMOV16"), operands);
                cmov.reads_flags = true;
                b.push(cmov);
                b.copy(dl, nl);
                b.copy(dh, nh);
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
            op::UMUL8X8 => {
                let (dl, dh) = b.write_word(name(&o[0])?)?;
                let (mut x, mut y) = (b.byte(name(&o[1])?)?, b.byte(name(&o[2])?)?);
                // a product goes either way round: the operand that arrives in
                // `rc3` is the second
                if b.arrives_in(x) == Some(rc(3)) || b.arrives_in(y) == Some(rc(2)) {
                    (x, y) = (y, x);
                }
                b.copy(Reg::Phys(rc(2)), x);
                b.copy(Reg::Phys(rc(3)), y);
                b.call("__umulqihi3");
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
    remove_dead(&mut b.function);
    narrow_remainder_calls(&mut b.function);
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
/// need `a`, and those of the rest.
fn results(insts: &[Inst]) -> Vec<RegId> {
    let mut out = Vec::new();
    for inst in insts.iter().rev() {
        match (inst.kind, inst.operands.first()) {
            (Kind::Copy, Some(Operand::Def(Reg::Phys(p)))) => out.push(*p),
            (Kind::Op("ADJSP"), _) => {}
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

fn remove_dead(function: &mut MFunction<Class>) {
    loop {
        let mut read: HashSet<u32> = HashSet::new();
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
    let effects = |inst: &Inst| {
        let mut defs = Vec::new();
        let mut reads = Vec::new();
        for operand in &inst.operands {
            match operand {
                Operand::Def(r) => defs.push(at(*r)),
                Operand::Use(r) => reads.push(at(*r)),
                _ => {}
            }
        }
        // a call writes every register it does not preserve, results included
        if inst.call {
            defs.extend(Mos::<false>::caller_saved());
        }
        (defs, reads)
    };
    let mut live_in: Vec<HashSet<RegId>> = vec![HashSet::new(); blocks];
    let mut after: Vec<Vec<HashSet<RegId>>> =
        function.blocks.iter().map(|b| vec![HashSet::new(); b.insts.len()]).collect();
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
    out: &mut String,
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
    out: &mut String,
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
                    Some(s) => move_byte(out, d, s, false, None),
                    None => {
                        out.push_str("  pla\n");
                        move_byte(out, d, reg::A, false, None);
                    }
                }
            }
            None => {
                // blocked: either a move needs `a` as scratch while another
                // reads it, or only cycles of cells are left. Park the value
                // on the stack and read it from there (two moves at most, so
                // one parked value).
                let free = |i: usize, pending: &[(RegId, Option<RegId>)]| {
                    !pending.iter().enumerate().any(|(j, (_, s))| j != i && *s == Some(pending[i].0))
                };
                let parked = if (0..pending.len()).any(|i| free(i, &pending)) {
                    out.push_str("  pha\n");
                    reg::A
                } else {
                    let (d, _) = pending[0];
                    writeln!(out, "  lda {}\n  pha", register_name(d)).unwrap();
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
    out: &mut String,
    to: RegId,
    from: RegId,
    a_live: bool,
    spare: Option<&str>,
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
        // x to y or y to x: through `a`
        (true, true) => through_a(out, a_live, None, &format!("  t{s}a\n  ta{d}\n")),
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
) -> Result<(String, Vec<(String, usize)>), String> {
    let before = IMAGINARY.with(|n| n.replace(zero_page.clamp(BASE_CELLS, MOST_CELLS)));
    let level = TUNING.with(|t| t.replace(tuning));
    let result = emit_on(function, labels, stack, cmos);
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
) -> Result<(String, Vec<(String, usize)>), String> {
    let run = |function: MFunction<Class>, sink: bool, share: bool| {
        if stack {
            emit_with::<true>(function, labels, cmos, sink, share)
        } else {
            emit_with::<false>(function, labels, cmos, sink, share)
        }
    };
    let (code, slots, sunk, shared) = run(function.clone(), true, true)?;
    if !sunk && !shared {
        return Ok((code, slots));
    }
    let cpu = if cmos { Cpu::W65c02 } else { Cpu::Mos6502 };
    let price = |code: &str, slots: &[(String, usize)]| {
        crate::timing::estimate(cpu, code)
            .map(|priced| (priced.bytes + slots.iter().map(|(_, n)| *n as u32).sum::<u32>(), priced.cycles))
    };
    // each change is kept only where it is no larger and no slower than the
    // function without it
    let mut best = (code, slots);
    let mut variants = vec![(false, false)];
    if sunk && shared {
        variants.extend([(true, false), (false, true)]);
    }
    for (sink, share) in variants {
        let Ok((other, other_slots, _, _)) = run(function.clone(), sink, share) else { continue };
        if let (Ok(kept), Ok(candidate)) = (price(&best.0, &best.1), price(&other, &other_slots)) {
            if kept.0 > candidate.0 || kept.1 > candidate.1 {
                best = (other, other_slots);
            }
        }
    }
    Ok(best)
}

/// `rs0 += n` (the frame, `n` as a signed 16-bit amount).
fn adjust(
    out: &mut String,
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
    let at = usize::from(entry.first().is_some_and(|inst| inst.kind == Kind::Op("ADJSP")));
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

/// Whether the emitted line leaves the zero and sign flags describing what `a`
/// now holds.
fn sets_nz_of_a(line: &str) -> bool {
    let line = line.trim();
    let (mnemonic, operand) = line.split_once(' ').unwrap_or((line, ""));
    match mnemonic {
        "lda" | "txa" | "tya" | "tax" | "tay" | "pla" | "and" | "ora" | "eor" | "adc" | "sbc" => true,
        "asl" | "lsr" | "rol" | "ror" | "inc" | "dec" => operand == "a",
        _ => false,
    }
}

fn emit_with<const STACK: bool>(
    mut function: MFunction<Class>,
    labels: &[Vec<String>],
    cmos: bool,
    sink: bool,
    share: bool,
) -> Result<(String, Vec<(String, usize)>, bool, bool), String> {
    // an instruction goes to the one path that reads it; a bump of a pointer is
    // then done in place, and the read of the old pointer that follows it
    // goes first
    let sunk = sink && machine_sink_in_place::<Mos<STACK>>(&mut function);
    let shared = share && crate::sharecmp::share_compares(&mut function);
    reschedule_kills::<Mos<STACK>>(&mut function);
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
    let after = live_after(&function, &at);
    let slot_name = |slot: i32| format!("s_{}_{}", function.name, slot);
    let mut out = String::new();
    let mut made = 0;
    let mut pending: Option<i64> = None;
    let target = |operand: &Operand| match operand {
        Operand::Block(id) => labels[*id as usize][0].clone(),
        other => unreachable!("{other:?} is not a block"),
    };
    for (b, block) in function.blocks.iter().enumerate() {
        for label in &labels[block.id as usize] {
            writeln!(out, "{label}:").unwrap();
        }
        if b == 0 && spilled > 0 && !has_frame {
            adjust(&mut out, -spilled);
        }
        let mut repeated = 0;
        for (i, inst) in block.insts.iter().enumerate() {
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
            match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, _) if is_pair(regs[0]) || is_pair(regs[1]) => {
                    let moves: Vec<_> = (0..2).map(|n| (lane(regs[0], n), lane(regs[1], n))).collect();
                    parallel_move(&mut out, &moves, a_live);
                }
                (Kind::Copy, _) => move_byte(&mut out, regs[0], regs[1], a_live, spare),
                (Kind::Op("UNPACK"), _) => {
                    parallel_move(&mut out, &[(regs[0], lane(regs[2], 0)), (regs[1], lane(regs[2], 1))], a_live);
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
                (Kind::Op("LDPX"), _) => writeln!(out, "  lda ({}),y", register_name(lane(regs[1], 0))).unwrap(),
                (Kind::Op("STPX"), _) => writeln!(out, "  sta ({}),y", register_name(lane(regs[1], 0))).unwrap(),
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
                // n := t when cc, n already holding f: the branch skips the
                // moves on the other path
                (Kind::Op("CMOV16"), [.., Operand::Imm(code), _]) => {
                    made += 1;
                    let done = format!("_cmd{made}");
                    let skip = ["bne", "beq", "bcs", "bcc", "bpl", "bmi"][*code as usize];
                    writeln!(out, "  {skip} {done}").unwrap();
                    parallel_move(&mut out, &[(regs[0], regs[5]), (regs[1], regs[6])], false);
                    writeln!(out, "{done}:").unwrap();
                }
                (Kind::Op("JSR"), [Operand::Symbol(callee, _), ..]) => writeln!(out, "  jsr {callee}").unwrap(),
                // return address, then target-1, then rts into the callee; its
                // own rts lands on the label
                (Kind::Op("JSRI"), _) => {
                    made += 1;
                    let (lo, hi) = (register_name(lane(regs[0], 0)), register_name(lane(regs[0], 1)));
                    let push_low = if cmos { "phx" } else { "txa\n  pha" };
                    writeln!(
                        out,
                        "  lda #>(_ci{made}-1)\n  pha\n  lda #<(_ci{made}-1)\n  pha\n  lda {lo}\n  sec\n  sbc #1\n  tax\n  lda {hi}\n  sbc #0\n  pha\n  {push_low}\n  rts\n_ci{made}:"
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
                    match spare {
                        Some(index) if pays => {
                            made += 1;
                            writeln!(out, "  ld{index} #{run}\n_sh{made}:\n  {first} {one}\n  {second} {two}\n  de{index}\n  bne _sh{made}").unwrap();
                            repeated = run - 1;
                        }
                        _ => writeln!(out, "  {first} {one}\n  {second} {two}").unwrap(),
                    }
                }
                (Kind::Op("SRA1"), _) => {
                    let (lo, hi) = (register_name(regs[1]), register_name(regs[3]));
                    writeln!(out, "  lda {hi}\n  cmp #$80\n  ror {hi}\n  ror {lo}").unwrap()
                }
                (Kind::Op("CMPZ"), [_, _, _, _, low, high]) => {
                    made += 1;
                    let load = |out: &mut String, r: RegId| match r {
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
                (Kind::Op(step @ ("INC" | "DEC")), _) => {
                    let (inc, r) = (step == "INC", regs[0]);
                    let (short, long) = if inc { ("in", "adc #1") } else { ("de", "sbc #1") };
                    if r == reg::X || r == reg::Y {
                        writeln!(out, "  {short}{}", register_name(r)).unwrap();
                    } else if r != reg::A {
                        writeln!(out, "  {short}c {}", register_name(r)).unwrap();
                    } else if cmos {
                        writeln!(out, "  {short}c a").unwrap();
                    } else {
                        writeln!(out, "  {}\n  {long}", if inc { "clc" } else { "sec" }).unwrap();
                    }
                }
                (Kind::Op("CLC"), _) => out.push_str("  clc\n"),
                (Kind::Op("SEC"), _) => out.push_str("  sec\n"),
                (Kind::Op(mnemonic @ ("ADC" | "SBC" | "AND" | "ORA" | "EOR")), [_, _, second, ..]) => {
                    let text = match second {
                        Operand::Imm(value) => format!("#{value}"),
                        Operand::Frame(slot) => slot_name(*slot),
                        _ => register_name(regs[2]),
                    };
                    writeln!(out, "  {} {text}", mnemonic.to_lowercase()).unwrap();
                }
                (Kind::Op("RTS"), _) => {
                    // on the stack the frame is released last: the result
                    // copies after the epilogue may still reload spills
                    let release = pending.take().or((spilled > 0 && !has_frame).then_some(spilled));
                    if let Some(n) = release {
                        out.push_str("  pha\n");
                        adjust(&mut out, n);
                        out.push_str("  pla\n");
                    }
                    out.push_str("  rts\n")
                }
                (Kind::Op("ADJSP"), [_, Operand::Imm(n)]) => {
                    let n = i64::from(*n as u16 as i16);
                    if n > 0 && spilled > 0 {
                        pending = Some(n + spilled);
                    } else {
                        adjust(&mut out, if n < 0 { n - spilled } else { n });
                    }
                }
                (Kind::Op("LEA"), [_, _, _, Operand::Imm(n)]) => {
                    let n = i64::from(*n as u16);
                    let n = if n >= old { n + spilled } else { n };
                    writeln!(out, "  clc\n  lda rc0\n  adc #{}", n & 0xFF).unwrap();
                    move_byte(&mut out, regs[0], reg::A, false, None);
                    writeln!(out, "  lda rc1\n  adc #{}", (n >> 8) & 0xFF).unwrap();
                    move_byte(&mut out, regs[1], reg::A, false, None);
                }
                // against zero, a load or an ALU result has already set the
                // flags a zero or sign test reads
                (Kind::Op("CMP"), [_, _, Operand::Imm(0)])
                    if block.insts.get(i + 1).is_some_and(reads_zero_or_sign_only)
                        && out.lines().last().is_some_and(sets_nz_of_a) => {}
                (Kind::Op("CMP"), [_, _, second]) => {
                    let text = match second {
                        Operand::Imm(value) => format!("#{value}"),
                        Operand::Frame(slot) => slot_name(*slot),
                        _ => register_name(regs[2]),
                    };
                    writeln!(out, "  cmp {text}").unwrap();
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
                // a := cc ? if_true : if_false, the branch first: loads set N
                // and Z
                (Kind::Op("CMOV"), [_, if_false, if_true, Operand::Imm(code), _]) => {
                    made += 1;
                    let (taken, done) = (format!("_cmt{made}"), format!("_cmd{made}"));
                    let branch = ["beq", "bne", "bcc", "bcs", "bmi", "bpl"][*code as usize];
                    let load = |out: &mut String, value: &Operand, at: Option<RegId>| match (value, at) {
                        (Operand::Imm(v), _) => writeln!(out, "  lda #{v}").unwrap(),
                        // a value already in `a`
                        (_, Some(r)) if r == reg::A => {}
                        (_, Some(r)) if r == reg::X => out.push_str("  txa\n"),
                        (_, Some(r)) if r == reg::Y => out.push_str("  tya\n"),
                        (_, Some(r)) => writeln!(out, "  lda {}", register_name(r)).unwrap(),
                        (_, None) => unreachable!(),
                    };
                    let mut read =
                        inst.operands.iter().filter_map(|o| if let Operand::Use(r) = o { Some(at(*r)) } else { None });
                    let false_at = matches!(if_false, Operand::Use(_)).then(|| read.next()).flatten();
                    let true_at = matches!(if_true, Operand::Use(_)).then(|| read.next()).flatten();
                    // `a` holds the compare's left side: `lda lhs; cmp rhs`
                    // just before
                    let held = i.checked_sub(2).and_then(|first| match (&block.insts[first], &block.insts[i - 1]) {
                        (copy, compare) if compare.kind == Kind::Op("CMP") => match (copy.kind, &copy.operands[..]) {
                            (Kind::Copy, [Operand::Def(Reg::Phys(a)), Operand::Use(lhs)]) if *a == reg::A => {
                                Some(at(*lhs))
                            }
                            _ => None,
                        },
                        _ => None,
                    });
                    let opposite = ["bne", "beq", "bcs", "bcc", "bpl", "bmi"][*code as usize];
                    if held.is_some() && held == true_at {
                        // the value to keep is already in `a`: skip the load of
                        // the other
                        writeln!(out, "  {branch} {done}").unwrap();
                        load(&mut out, if_false, false_at);
                    } else if held.is_some() && held == false_at {
                        writeln!(out, "  {opposite} {done}").unwrap();
                        load(&mut out, if_true, true_at);
                    } else if matches!(code, 2 | 3) {
                        // a load leaves the carry alone: the value for a true
                        // condition goes in first,
                        // and the branch skips the other
                        load(&mut out, if_true, true_at);
                        writeln!(out, "  {branch} {done}").unwrap();
                        load(&mut out, if_false, false_at);
                    } else {
                        writeln!(out, "  {branch} {taken}").unwrap();
                        load(&mut out, if_false, false_at);
                        writeln!(out, "  jmp {done}\n{taken}:").unwrap();
                        load(&mut out, if_true, true_at);
                    }
                    writeln!(out, "{done}:").unwrap();
                }
                // a slot is a byte, or two for a pair
                // a callee-saved cell on the hardware stack
                // (see `save_preserved`); the restore keeps the result in `a`
                // safe in `y`, which no result uses
                (Kind::Op("PUSHC"), _) => {
                    if a_live {
                        return Err("a is live where a callee-saved cell is pushed".into());
                    }
                    writeln!(out, "  lda {}\n  pha", register_name(regs[0])).unwrap();
                }
                (Kind::Op("CSAVE"), _) => {
                    if after[b][i].contains(&reg::Y) || after[b][i].contains(&reg::X) || a_live {
                        return Err("a register is live where the callee-saved cells are saved".into());
                    }
                    writeln!(out, "  ldy #{}\n  jsr __csave", regs.len()).unwrap();
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
    }
    let slots: Vec<(String, usize)> =
        if STACK { Vec::new() } else { slots.iter().map(|(slot, class)| (slot_name(*slot), size_of(class))).collect() };
    Ok((own_labels(&crate::emit::relax_bra(&relax(&drop_reloads(&out))), &function.name), slots, sunk, shared))
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

/// A conditional branch reaches 127 bytes. An instruction is at most 3 bytes,
/// so one whose target is within 40 instruction lines stays and any other
/// becomes the opposite branch over a `jmp`.
///
/// DEVIATE: line counting, not sizes (as `emit::relax_bra`). Exit: the shared
/// branch relaxation.
/// `sta m` then `lda m` (likewise `x` and `y`) of a zero-page or spill cell:
/// the register still holds it, so the load goes. It also set the zero and sign
/// flags, so it stays when a branch reads them before anything sets them again.
pub fn drop_reloads(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let cell = |operand: &str| operand.starts_with("rc") || operand.starts_with("s_");
    let sets_flags = |mnemonic: &str| {
        matches!(
            mnemonic,
            "lda"
                | "ldx"
                | "ldy"
                | "adc"
                | "sbc"
                | "cmp"
                | "cpx"
                | "cpy"
                | "and"
                | "ora"
                | "eor"
                | "asl"
                | "lsr"
                | "rol"
                | "ror"
                | "inc"
                | "dec"
                | "inx"
                | "iny"
                | "dex"
                | "dey"
                | "tax"
                | "tay"
                | "txa"
                | "tya"
                | "pla"
                | "bit"
                | "jsr"
                | "rts"
        )
    };
    let flags_unread_after = |from: usize| {
        for line in &lines[from..] {
            let mnemonic = line.trim().split(' ').next().unwrap_or("");
            if !line.starts_with("  ") {
                return false;
            }
            if sets_flags(mnemonic) {
                return true;
            }
            if !matches!(
                mnemonic,
                "sta" | "stx" | "sty" | "stz" | "clc" | "sec" | "pha" | "nop"
            ) {
                return false;
            }
        }
        false
    };
    let mut out = String::with_capacity(text.len());
    for (at, line) in lines.iter().enumerate() {
        let reload = at > 0
            && lines[at - 1].starts_with("  st")
            && line.starts_with("  ld")
            && lines[at - 1].trim()[2..] == line.trim()[2..]
            && line.trim().split(' ').nth(1).is_some_and(cell)
            && flags_unread_after(at + 1);
        if !reload {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn relax(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let is_instruction = |l: &str| l.starts_with("  ") && !l.trim_start().starts_with('.');
    let mut out = String::new();
    let mut far = 0;
    for (at, line) in lines.iter().enumerate() {
        let parts: Vec<&str> = line.trim_start().split(' ').collect();
        let opposite = match parts[..] {
            ["beq", _] => "bne",
            ["bne", _] => "beq",
            ["bcc", _] => "bcs",
            ["bcs", _] => "bcc",
            ["bmi", _] => "bpl",
            ["bpl", _] => "bmi",
            _ => {
                writeln!(out, "{line}").unwrap();
                continue;
            }
        };
        let label = format!("{}:", parts[1]);
        let near = lines
            .iter()
            .position(|l| *l == label)
            .is_some_and(
                |to| {
                    let (from, until) = if to > at { (at + 1, to) } else { (to, at) };
                    lines[from..until].iter().filter(|l| is_instruction(l)).count() <= 40
                },
            );
        if near {
            writeln!(out, "{line}").unwrap();
        } else {
            far += 1;
            writeln!(out, "  {opposite} _far{far}\n  jmp {}\n_far{far}:", parts[1]).unwrap();
        }
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
                eprintln!("in bb{at}.{i}: {:?} {:?}{}{}", inst.kind, inst.operands, if inst.sets_flags { " SF" } else { "" }, if inst.reads_flags { " RF" } else { "" });
            }
        }
    }
    if spike_asked() || !wanted {
        return allocate_spilling::<Mos<STACK>>(function, 0);
    }
    allocate_greedy::<Mos<STACK>>(function)
}

/// `function` allocated by greedy: converted, its copies joined by the
/// coalescer, allocated with the spills written into it, and written back.
fn allocate_greedy<T: Target<Class = Class>>(
    function: &mut MFunction<Class>
) -> Result<(llrm_spike_mf::regalloc::Assignment, Vec<(i32, Class)>), String> {
    use llrm_codegen::greedy::shadow::{Mode, decide_with_split};
    use llrm_lir::registers::Regs;
    let regs = Regs(&crate::REGISTER_INFO);
    let mut converted = llrm_spike_mf::tolir::to_lir::<T>(function, regs)?;
    if std::env::var_os("LLRM_NO_COALESCE").is_none() {
        converted.coalesce(regs, TUNING.with(|t| t.get()) == Tuning::SIZE);
    }
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs, &order);
    let mut frame = llrm_spike_mf::tolir::SpikeFrame::of(function);
    let decided = decide_with_split(&inputs, Mode::Inline { frame: &mut frame }, true)
        .ok_or_else(|| format!("{}: greedy refused", function.name))?;
    if decided.spill_failed || decided.report.stats.failed > 0 {
        return Err(format!("{}: greedy could not allocate", function.name));
    }
    let back = llrm_spike_mf::tolir::from_lir::<T>(function, &converted, &decided, regs)?;
    *function = back.function;
    if std::env::var("LLRM_6502_DUMP").is_ok_and(|name| name == function.name) {
        for (at, block) in function.blocks.iter().enumerate() {
            for (i, inst) in block.insts.iter().enumerate() {
                eprintln!("bb{at}.{i}: {:?} {:?}{}{}", inst.kind, inst.operands, if inst.sets_flags { " SF" } else { "" }, if inst.reads_flags { " RF" } else { "" });
            }
        }
    }
    if std::env::var_os("LLRM_6502_CHECK").is_some() {
        llrm_spike_mf::regalloc::GraphColoring::<T>::by_frequency(false).check(function, &back.assignment)?;
    }
    Ok((back.assignment, back.slots))
}
