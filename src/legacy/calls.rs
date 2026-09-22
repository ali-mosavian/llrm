//! Port of `qbopt/legacy/calls.py`: the runtime calls, and what they can be
//! replaced with.
//!
//! Comparison pushes its left operand first; multiply, divide and remainder
//! push it second. Getting it backwards is a different answer, not a crash.

use std::sync::LazyLock;

use iced_x86::{
    BlockEncoder, BlockEncoderOptions, Code, Decoder, DecoderOptions, IcedError, Instruction, InstructionBlock,
    MemoryOperand, Register,
};

use crate::analysis::flags::{ALL, Flag};
use crate::frontend::blocks::Block;
use crate::frontend::declen::{BITNESS, Insn, to_signed};
use crate::frontend::stack::{PUSH_BYTES, frames};
use crate::legacy::lift::Emitted;
pub use crate::legacy::lift::relocated_memory;
use crate::objectfile::module::{Addr, Module, Space};
use crate::support::hash::IndexMap;
use crate::support::pyrepr::Repr;

pub const COMPARE: &str = "B$CPI4";
pub const MULTIPLY: &str = "B$MUI4";
pub const DIVIDE: &str = "B$DVI4";
/// The routine that answers a long remainder.
pub const REMAINDER: &str = "B$RMI4";

// B$CMI4 returns flags meant for an *unsigned* jcc; ABSORBED's CMP_R32_RM32
// is only right for B$CPI4. It is not in LEFT_FIRST and must stay out.

/// True where the left operand is pushed first.
pub static LEFT_FIRST: LazyLock<IndexMap<&'static str, bool>> =
    LazyLock::new(|| IndexMap::from_iter([(COMPARE, true), (MULTIPLY, false), (DIVIDE, false), (REMAINDER, false)]));

/// How many long arguments this routine takes, or None if absorption does
/// not handle it at all.
pub fn _arity(name: Option<&str>) -> Option<i64> {
    match name {
        Some(name) if LEFT_FIRST.contains_key(name) => Some(2),
        _ => None,
    }
}

// one dword per argument under VBDOS /G3, two words everywhere else
pub const PUSHES: [Code; 2] = [Code::Push_rm16, Code::Push_rm32];
pub const CONSTANTS: [Code; 4] = [Code::Pushw_imm8, Code::Pushd_imm8, Code::Push_imm16, Code::Pushd_imm32];
pub const WIDE_PUSHES: [Code; 3] = [Code::Push_rm32, Code::Pushd_imm8, Code::Pushd_imm32];

// iced's immediate() is unsigned: an imm8 source comes back as a 64-bit
// rendering of its sign extension, an imm16/imm32 at its own width.
pub static CONSTANT_WIDTH: LazyLock<IndexMap<Code, usize>> = LazyLock::new(|| {
    IndexMap::from_iter([(Code::Pushw_imm8, 8), (Code::Pushd_imm8, 8), (Code::Push_imm16, 2), (Code::Pushd_imm32, 4)])
});

/// `class Kind(StrEnum)`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    /// a bare displacement, whose address is a fixup
    Static,
    /// an immediate pushed straight to the stack
    Constant,
}

impl Kind {
    pub const fn value(self) -> &'static str {
        match self {
            Kind::Static => "static",
            Kind::Constant => "constant",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Operand {
    pub kind: Kind,
    pub addr: Option<Addr>,
    /// where the displacement field was, so its fixup can be reused
    pub at: Option<usize>,
    pub value: i64,
    /// instructions it took to push
    pub length: usize,
}

impl Operand {
    /// `Operand(kind)` with every other field defaulted.
    pub const fn new(kind: Kind) -> Self {
        Operand { kind, addr: None, at: None, value: 0, length: 0 }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CallSite {
    /// the call instruction
    pub at: usize,
    pub end: usize,
    /// where the first push begins -- the call itself, if consume is set
    pub start: usize,
    pub name: String,
    /// in the order they reach the stack -- empty if consume is set
    pub pushed: Vec<Operand>,
    /// the raw pushes stack.py found for this call, when match() could not:
    /// consume() pops every byte instead of reloading from an address
    pub consume: Vec<Insn>,
}

impl CallSite {
    /// (left, right), whichever way this routine takes them.
    pub fn operands(&self) -> (Operand, Operand) {
        let [first, second] = <[Operand; 2]>::try_from(self.pushed.clone())
            .unwrap_or_else(|pushed| panic!("expected 2 operands, got {}", pushed.len()));
        if LEFT_FIRST[self.name.as_str()] { (first, second) } else { (second, first) }
    }
}

/// An exception Python does not catch.
fn made(result: Result<Instruction, IcedError>) -> Instruction {
    result.unwrap_or_else(|error| panic!("{error}"))
}

/// Python's `create_*_i32` refuses an int outside i32 with OverflowError.
fn i32_of(value: i64) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| panic!("OverflowError: {value} out of i32 range"))
}

/// A pushed operand at a fixed address, or an array element indexed by si/di.
///
/// A scaled-index push has no base but an index, and would slip past the
/// base whitelist, so it is refused on its own.
pub fn static_at(module: &Module, insn: &Insn) -> Option<Operand> {
    if !PUSHES.contains(&insn.code()) || insn.disp_at.is_none() || insn.memory_index() != Register::None {
        return None;
    }
    if ![Register::None, Register::SI, Register::DI].contains(&insn.memory_base()) {
        return None;
    }
    let disp_at = insn.disp_at?;
    let mut addr = *module.operands.get(&(disp_at as i64))?;
    if addr.space != Space::Segment {
        return None;
    }
    if insn.memory_base() != Register::None {
        addr = Addr { base: insn.memory_base(), ..addr };
    }
    Some(Operand { addr: Some(addr), at: Some(disp_at), length: 1, ..Operand::new(Kind::Static) })
}

pub fn constant_at(insn: &Insn) -> Option<Operand> {
    if !CONSTANTS.contains(&insn.code()) || insn.imm_at.is_none() {
        return None;
    }
    // a negative constant is otherwise a huge unsigned value, which iced's
    // own i32 builder refuses in absorb() -- found by tools/fuzzcheck.py
    Some(Operand {
        value: to_signed(insn.insn.immediate(0), CONSTANT_WIDTH[&insn.code()]),
        length: 1,
        ..Operand::new(Kind::Constant)
    })
}

/// An INTEGER literal, widened to the LONG a `byval` parameter takes.
///
/// `mov ax,imm16 / cwd / push dx / push ax`: four instructions where a
/// runtime call's own small-constant form is one.
pub fn widened_constant_at(reached: &[Insn], last: usize) -> Option<Operand> {
    if last < 3 {
        return None;
    }
    let (mov_ax, cwd, push_dx, push_ax) = (&reached[last - 3], &reached[last - 2], &reached[last - 1], &reached[last]);
    if !(mov_ax.code() == Code::Mov_r16_imm16
        && mov_ax.insn.op0_register() == Register::AX
        && cwd.code() == Code::Cwd
        && cwd.at == mov_ax.end()
        && push_dx.code() == Code::Push_r16
        && push_dx.insn.op0_register() == Register::DX
        && push_dx.at == cwd.end()
        && push_ax.code() == Code::Push_r16
        && push_ax.insn.op0_register() == Register::AX
        && push_ax.at == push_dx.end())
    {
        return None;
    }
    Some(Operand { value: to_signed(mov_ax.insn.immediate(1), 2), length: 4, ..Operand::new(Kind::Constant) })
}

/// The long argument whose pushes end at `reached[last]`, or None.
///
/// One dword, or two words high first. The `+2` on the word form is the
/// lifter's pair discipline and fails the same silent way if dropped.
pub fn one_operand(module: &Module, reached: &[Insn], last: usize) -> Option<Operand> {
    let insn = &reached[last];
    if WIDE_PUSHES.contains(&insn.code()) {
        return static_at(module, insn).or_else(|| constant_at(insn));
    }

    let Some(low) = static_at(module, insn).or_else(|| constant_at(insn)) else {
        return widened_constant_at(reached, last);
    };
    if last == 0 {
        return None;
    }
    let high = static_at(module, &reached[last - 1]).or_else(|| constant_at(&reached[last - 1]));
    let high = high.filter(|_| reached[last - 1].end() == insn.at)?;
    if low.kind != high.kind {
        return None;
    }
    if low.kind == Kind::Static {
        if low.addr.is_none() || high.addr != low.addr.map(|addr| addr.plus(2)) {
            return None;
        }
        return Some(Operand { length: 2, ..low });
    }
    Some(Operand { value: (high.value << 16) | (low.value & 0xFFFF), length: 2, ..low })
}

/// The call at `reached[index]` with its arguments, or None.
///
/// Every long argument the routine takes, nothing in between, and every
/// push adjacent to the next.
pub fn r#match(module: &Module, reached: &[Insn], index: usize) -> Option<CallSite> {
    let call = &reached[index];
    let name = module.calls.get(&(call.at as i64))?;
    if !LEFT_FIRST.contains_key(name.as_str()) {
        return None;
    }
    let arity = 2;

    let mut found: Vec<Operand> = Vec::new();
    let mut last = index as i64 - 1;
    while found.len() < arity && last >= 0 {
        let here = last as usize;
        if reached[here].end() != if found.is_empty() { call.at } else { reached[here + 1].at } {
            return None;
        }
        let operand = one_operand(module, reached, here)?;
        last -= operand.length as i64;
        found.push(operand);
    }
    if found.len() != arity {
        return None;
    }

    found.reverse();
    Some(CallSite {
        at: call.at,
        end: call.end(),
        start: reached[(last + 1) as usize].at,
        name: name.clone(),
        pushed: found,
        consume: Vec::new(),
    })
}

/// A frame's raw pushes, classified the way match() classifies its own.
///
/// The pushes are contiguous with each other but not necessarily with the
/// call -- lngmix spills a result between them. Empty where anything does
/// not fit.
pub fn _classified(module: &Module, pushed: &[Insn], name: &str) -> Vec<Operand> {
    if !LEFT_FIRST.contains_key(name) {
        return Vec::new();
    }
    let arity = 2;
    let mut found: Vec<Operand> = Vec::new();
    let mut last = pushed.len() as i64 - 1;
    while found.len() < arity && last >= 0 {
        let here = last as usize;
        if here + 1 < pushed.len() && pushed[here].end() != pushed[here + 1].at {
            return Vec::new();
        }
        let Some(operand) = one_operand(module, pushed, here) else {
            return Vec::new();
        };
        last -= operand.length as i64;
        found.push(operand);
    }
    if found.len() != arity || last != -1 {
        return Vec::new();
    }
    found.reverse();
    found
}

/// Every call whose arguments are known: classified where match() sees
/// them, popped from the stack where only frames() does.
pub fn sites(module: &Module, reached: &[Insn], blocks: &[Block]) -> Vec<CallSite> {
    let mut found = Vec::new();
    let mut handled: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for (index, insn) in reached.iter().enumerate() {
        if !module.calls.contains_key(&(insn.at as i64)) {
            continue;
        }
        if let Some(site) = r#match(module, reached, index) {
            found.push(site);
            handled.insert(insn.at);
        }
    }
    for block in blocks {
        for frame in frames(block, &module.calls, &|name| _arity(Some(name))) {
            if handled.contains(&frame.call.at) {
                continue;
            }
            let name = module.calls[&(frame.call.at as i64)].clone();
            found.push(CallSite {
                at: frame.call.at,
                end: frame.call.end(),
                start: frame.call.at,
                pushed: _classified(module, &frame.pushed, &name),
                name,
                consume: frame.pushed,
            });
        }
    }
    found
}

// B$CPI4 rebuilds its answer through lahf/sahf. A 386 cmp leaves the signed
// and equality flags the following jcc wants, but CF, PF and AF are the
// runtime's synthesis, so a site whose CF is read afterwards is left alone.
// Where the high words are equal B$CPI4's OF is left from the low-word
// compare and its jl/jge answer backwards; this cmp is right there and the
// runtime is not (suite/cmpof.bas, configs.DIVERGES).
pub const SYNTHESISED: Flag = Flag(Flag::CF.0 | Flag::PF.0 | Flag::AF.0);

pub static ABSORBED: LazyLock<IndexMap<&'static str, Code>> =
    LazyLock::new(|| IndexMap::from_iter([(COMPARE, Code::Cmp_r32_rm32), (MULTIPLY, Code::Imul_r32_rm32)]));

// Divide and remainder are C's: one idiv, no test of the divisor. BC's
// error 11 on `x / 0` does not survive, deliberately.
pub const DIVIDES: [&str; 2] = [DIVIDE, REMAINDER];

/// What the runtime returns a long in, as ax:dx.
pub const RESULT: Register = Register::EAX;

// Where a divide leaves the answer its own name does not promise. Not edx:
// the restore overwrites it. ebx: every one of these sites clobbers it.
pub const OTHER: Register = Register::EBX;

// What a divide's second operand is loaded into: the emitter and anything
// selecting a divide from an operation's own operands make the same choice.
pub const DIVISOR: Register = Register::ECX;

// B$CPI4 preserves every register (helpi4.asm: save-list <AX>, touches no
// cx/dx/bx), so an absorbed compare wraps its scratch eax in push/pop.

/// Where a Kind.STATIC operand's value lives, read at codegen time.
///
/// static_at always gives a relocated SEGMENT address; anything else is a
/// bug here, so this raises.
pub fn relocated_addr(operand: &Operand) -> Addr {
    match operand.addr {
        Some(addr) if addr.space == Space::Segment => addr,
        _ => panic!("a {} operand has no relocated address", operand.kind.value()),
    }
}

pub fn memory_of(operand: &Operand) -> MemoryOperand {
    relocated_memory(relocated_addr(operand).base, Register::None)
}

pub fn load_of(operand: &Operand) -> Instruction {
    if operand.kind == Kind::Constant {
        return made(Instruction::with2(Code::Mov_r32_imm32, RESULT, i32_of(operand.value)));
    }
    let base = relocated_addr(operand).base;
    // the moffs form has no ModRM byte and so cannot carry an index register
    let code = if base != Register::None { Code::Mov_r32_rm32 } else { Code::Mov_EAX_moffs32 };
    made(Instruction::with2(code, RESULT, relocated_memory(base, Register::None)))
}

pub const fn fits_in_a_byte(value: i64) -> bool {
    -128 <= value && value < 128
}

// A multiply by a constant the 386 can do without multiplying, as gcc and
// clang pick on i386. Only where nothing reads the flags afterwards, which
// absorb() has already established for every MULTIPLY site.
pub static SCALES: LazyLock<IndexMap<i64, u32>> = LazyLock::new(|| IndexMap::from_iter([(3, 2), (5, 4), (9, 8)]));

/// One instruction that multiplies RESULT by `value`, or None.
///
/// Negative and zero are refused rather than special-cased.
pub fn _without_multiplying(value: i64) -> Option<Instruction> {
    if value <= 0 {
        return None;
    }
    if value & (value - 1) == 0 && value != 1 {
        let bit_length = 64 - value.leading_zeros();
        return Some(made(Instruction::with2(Code::Shl_rm32_imm8, RESULT, (bit_length - 1) as i32)));
    }
    if let Some(&scale) = SCALES.get(&value) {
        return Some(made(Instruction::with2(
            Code::Lea_r32_m,
            RESULT,
            MemoryOperand::new(RESULT, RESULT, scale, 0, 0, false, Register::None),
        )));
    }
    None
}

pub fn apply_to(name: &str, operand: &Operand) -> Instruction {
    if operand.kind != Kind::Constant {
        return made(Instruction::with2(ABSORBED[name], RESULT, memory_of(operand)));
    }
    // the sign-extended byte forms are two or three bytes shorter
    let short = fits_in_a_byte(operand.value);
    if name == COMPARE {
        let code = if short { Code::Cmp_rm32_imm8 } else { Code::Cmp_EAX_imm32 };
        return made(Instruction::with2(code, RESULT, i32_of(operand.value)));
    }
    if name == MULTIPLY {
        if let Some(cheaper) = _without_multiplying(operand.value) {
            return cheaper;
        }
    }
    let code = if short { Code::Imul_r32_rm32_imm8 } else { Code::Imul_r32_rm32_imm32 };
    made(Instruction::with3(code, RESULT, RESULT, i32_of(operand.value)))
}

/// The call replaced by 386 instructions, or why it cannot be.
///
/// A compare wraps eax in push/pop because B$CPI4 changes no register.
/// `restore=false` drops the trailing high-half restore for a site whose
/// following code lift.tail() proved widens against it. `Err` is Python's
/// `str` answer.
pub fn absorb(site: &CallSite, live: Flag, restore: bool) -> Result<Emitted, String> {
    if !site.consume.is_empty() && site.pushed.is_empty() {
        // Popped only where nothing classified the pushes: reloading a
        // classified frame site is dividing()'s case, not this one's.
        return consume(site, live, restore);
    }
    if DIVIDES.contains(&site.name.as_str()) {
        return dividing(site, live, restore);
    }
    if !ABSORBED.contains_key(site.name.as_str()) {
        return Err(format!("{} is not absorbed", site.name));
    }
    if site.name == COMPARE && !(live & SYNTHESISED).is_empty() {
        return Err(format!(
            "the site's {} comes from the runtime, not from a comparison",
            (live & SYNTHESISED).repr()
        ));
    }
    if site.name == MULTIPLY && !(live & ALL).is_empty() {
        // imul sets the flags where the runtime left whatever it happened to
        return Err(format!("something reads {} after the multiply", (live & ALL).repr()));
    }

    let (left, right) = site.operands();
    // x*x (or x==x): the same address read twice is one load
    let same_address = left.kind == Kind::Static && right.kind == Kind::Static && left.addr == right.addr;

    let mut steps: Vec<Instruction> = Vec::new();
    let mut relocated: Vec<(usize, usize)> = Vec::new();
    let mut add = |insn: Instruction| -> usize {
        steps.push(insn);
        steps.len() - 1
    };

    if site.name == COMPARE {
        add(made(Instruction::with1(Code::Push_r32, RESULT)));
    }

    let r#where = add(load_of(&left));
    if let (Kind::Static, Some(at)) = (left.kind, left.at) {
        relocated.push((r#where, at));
    }

    if same_address {
        add(made(Instruction::with2(ABSORBED[site.name.as_str()], RESULT, RESULT)));
    } else {
        let r#where = add(apply_to(&site.name, &right));
        if let (Kind::Static, Some(at)) = (right.kind, right.at) {
            relocated.push((r#where, at));
        }
    }

    if site.name == COMPARE {
        // pop does not touch the flags the cmp above just set
        add(made(Instruction::with1(Code::Pop_r32, RESULT)));
    } else if restore {
        // a multiply leaves a value, and BC reads its high half from dx
        for insn in restoring() {
            add(insn);
        }
    }

    Ok(assemble(&mut steps, &relocated).unwrap_or_else(|error| panic!("{error}")))
}

/// Encode a block whose branches name one another by instruction index.
///
/// `relocated` is Python's dict in insertion order. `Err` is Python's
/// ValueError.
pub fn assemble(steps: &mut [Instruction], relocated: &[(usize, usize)]) -> Result<Emitted, String> {
    for (index, insn) in steps.iter_mut().enumerate() {
        insn.set_ip(index as u64);
    }
    let code = BlockEncoder::encode(BITNESS, InstructionBlock::new(steps, 0), BlockEncoderOptions::NONE)
        .map_err(|error| error.to_string())?
        .code_buffer;

    let mut decoder = Decoder::with_ip(BITNESS, &code, 0, DecoderOptions::NONE);
    let mut placed = Vec::new();
    while decoder.can_decode() {
        let insn = decoder.decode();
        placed.push((insn.ip() as usize, decoder.get_constant_offsets(&insn)));
    }
    if placed.len() != steps.len() {
        return Err(format!("encoded {} instructions from {}", placed.len(), steps.len()));
    }
    let relocations = relocated
        .iter()
        .map(|&(index, field)| (placed[index].0 + placed[index].1.displacement_offset(), field))
        .collect();
    Ok(Emitted { code, relocations })
}

/// Put the high half back where BC reads it, through the stack.
pub fn restoring() -> Vec<Instruction> {
    vec![
        made(Instruction::with1(Code::Push_r32, RESULT)),
        made(Instruction::with1(Code::Pop_r16, Register::AX)),
        made(Instruction::with1(Code::Pop_r16, Register::DX)),
    ]
}

/// One argument's worth of pushes per group, deepest first, or None if
/// they do not split cleanly into 4-byte arguments.
///
/// Byte-counted from the top: frames() guarantees the total, not that a
/// word pair stays adjacent to its own other half.
pub fn grouped(pushed: &[Insn]) -> Option<Vec<Vec<Insn>>> {
    let mut groups: Vec<Vec<Insn>> = Vec::new();
    let mut remaining: Vec<Insn> = pushed.to_vec();
    while !remaining.is_empty() {
        let mut have = 0;
        let mut take: Vec<Insn> = Vec::new();
        while have < 4 && !remaining.is_empty() {
            let insn = remaining.pop().expect("checked");
            have += PUSH_BYTES.get(&insn.code()).copied().unwrap_or(0);
            take.push(insn);
        }
        if have != 4 {
            return None;
        }
        take.reverse();
        groups.push(take);
    }
    groups.reverse();
    Some(groups)
}

// bx is never a target below and never holds a value this pass preserves
// across the call, so it is always free as scratch.

/// One argument, off the real stack and into `target`: four bytes in dword
/// layout either way BC pushed them.
pub fn popped_into(target: Register) -> Instruction {
    made(Instruction::with1(Code::Pop_r32, target))
}

// Which register each pop lands in, topmost group to deepest -- forced by
// the real stack, not by LEFT_FIRST. COMPARE is never popped: see
// compare_consume().
pub static CONSUME_TARGETS: LazyLock<IndexMap<&'static str, (Register, Register)>> = LazyLock::new(|| {
    IndexMap::from_iter([
        (MULTIPLY, (Register::EAX, Register::ECX)),
        (DIVIDE, (Register::EAX, Register::ECX)),
        (REMAINDER, (Register::EAX, Register::ECX)),
    ])
});

/// A popped compare, without popping: B$CPI4 changes no register, and its
/// arguments come off the stack as its callee-cleanup would remove them.
///
/// bp stands in as a frame pointer, edx holds one side of the cmp; both
/// are saved and put back. Nothing is read below sp, where an interrupt may
/// have written: bp is parked in the dead argument space before sp moves.
pub fn compare_consume() -> Emitted {
    let bp = |displ: i64| MemoryOperand::new(Register::BP, Register::None, 1, displ, 1, false, Register::None);
    let mut steps: Vec<Instruction> = vec![
        made(Instruction::with1(Code::Push_r16, Register::BP)),
        made(Instruction::with1(Code::Push_r32, Register::EDX)),
        made(Instruction::with2(Code::Mov_r16_rm16, Register::BP, Register::SP)),
        // +6 pushed ahead of the arguments puts left at +10 and right at +6
        made(Instruction::with2(Code::Mov_r32_rm32, Register::EDX, bp(10))),
        made(Instruction::with2(Code::Cmp_r32_rm32, Register::EDX, bp(6))),
        // everything from here on must leave the flags alone
        made(Instruction::with2(Code::Mov_r16_rm16, Register::DX, bp(4))),
        // +12 is the top of the left argument, already read into edx
        made(Instruction::with2(Code::Mov_rm16_r16, bp(12), Register::DX)),
        made(Instruction::with2(Code::Mov_r32_rm32, Register::EDX, bp(0))),
        made(Instruction::with2(Code::Lea_r16_m, Register::SP, bp(12))),
        made(Instruction::with1(Code::Pop_r16, Register::BP)),
    ];
    assemble(&mut steps, &[]).unwrap_or_else(|error| panic!("{error}"))
}

/// A call whose arguments only the stack knows, popped rather than reloaded.
///
/// Every byte pushed comes back off: reloading one operand while leaving
/// its push would leak four bytes of stack per call.
pub fn consume(site: &CallSite, live: Flag, restore: bool) -> Result<Emitted, String> {
    if site.name == COMPARE && !(live & SYNTHESISED).is_empty() {
        return Err(format!(
            "the site's {} comes from the runtime, not from a comparison",
            (live & SYNTHESISED).repr()
        ));
    }
    if site.name != COMPARE && !(live & ALL).is_empty() {
        return Err(format!("something reads {} after it", (live & ALL).repr()));
    }

    let Some(groups) = grouped(&site.consume) else {
        return Err(format!("{}'s pushes do not split cleanly into 4-byte arguments", site.name));
    };
    let arity = _arity(Some(&site.name));
    if arity != Some(groups.len() as i64) {
        let said = arity.map_or_else(|| "None".to_owned(), |arity| arity.to_string());
        return Err(format!("{} takes {} arguments, not {}", site.name, said, groups.len()));
    }

    if site.name == COMPARE {
        return Ok(compare_consume());
    }

    // one dword per argument, popped topmost to deepest
    let (first, second) = CONSUME_TARGETS[site.name.as_str()];
    let mut steps: Vec<Instruction> = [first, second].into_iter().map(popped_into).collect();

    // A divisor that is a constant power of two needs no idiv. grouped() is
    // deepest first, so the divisor is groups[0].
    let divisor = if DIVIDES.contains(&site.name.as_str()) && groups.len() == 2 { Some(&groups[0]) } else { None };
    let known = divisor.filter(|divisor| divisor.len() == 1).and_then(|divisor| constant_at(&divisor[0]));
    let shift = known.and_then(|known| _power_of_two(known.value));

    if site.name == MULTIPLY {
        steps.push(made(Instruction::with2(Code::Imul_r32_rm32, Register::EAX, Register::ECX)));
    } else if let Some(shift) = shift {
        steps.extend(dividing_by_a_power_of_two(&site.name, shift, Register::ECX));
    } else if DIVIDES.contains(&site.name.as_str()) {
        steps.push(Instruction::with(Code::Cdq));
        steps.push(made(Instruction::with1(Code::Idiv_rm32, Register::ECX)));
        steps.extend(keeping_the_other(&site.name));
        if site.name == REMAINDER {
            steps.push(made(Instruction::with2(Code::Mov_r32_rm32, RESULT, Register::EDX)));
        }
    }
    if restore {
        steps.extend(restoring());
    }
    Ok(assemble(&mut steps, &[]).unwrap_or_else(|error| panic!("{error}")))
}

/// The move that puts a divide's unasked-for answer somewhere it lasts,
/// straight after the idiv.
pub fn keeping_the_other(name: &str) -> Vec<Instruction> {
    if name == DIVIDE {
        return vec![made(Instruction::with2(Code::Mov_r32_rm32, OTHER, Register::EDX))];
    }
    if name == REMAINDER {
        return vec![made(Instruction::with2(Code::Mov_r32_rm32, OTHER, RESULT))];
    }
    Vec::new()
}

/// Where this site leaves the answer its own name does not promise.
pub fn other_result(site: &CallSite) -> Option<Register> {
    if DIVIDES.contains(&site.name.as_str()) { Some(OTHER) } else { None }
}

/// n where value is 2**n, for 1 <= n <= 31, else None.
pub fn _power_of_two(value: i64) -> Option<u32> {
    if value <= 1 || value & (value - 1) != 0 {
        return None;
    }
    let n = 64 - value.leading_zeros() - 1;
    if (1..=31).contains(&n) { Some(n) } else { None }
}

/// RESULT divided by 2**n, or its remainder, without an idiv.
///
/// The bias makes the shift truncate towards zero as idiv does:
///
///     mov  scratch,eax / sar scratch,31 / shr scratch,32-n
///     add  eax,scratch / sar eax,n
pub fn dividing_by_a_power_of_two(name: &str, n: u32, scratch: Register) -> Vec<Instruction> {
    let n = n as i32;
    let mut steps = vec![
        made(Instruction::with2(Code::Mov_r32_rm32, scratch, RESULT)),
        made(Instruction::with2(Code::Sar_rm32_imm8, scratch, 31)),
        made(Instruction::with2(Code::Shr_rm32_imm8, scratch, 32 - n)),
    ];
    if name == REMAINDER {
        steps.push(made(Instruction::with2(Code::Add_r32_rm32, scratch, RESULT)));
        steps.push(made(Instruction::with2(Code::And_rm32_imm32, scratch, i32_of(-(1_i64 << n)))));
        steps.push(made(Instruction::with2(Code::Sub_r32_rm32, RESULT, scratch)));
        return steps;
    }
    steps.push(made(Instruction::with2(Code::Add_r32_rm32, RESULT, scratch)));
    steps.push(made(Instruction::with2(Code::Sar_rm32_imm8, RESULT, n)));
    steps
}

/// A long divide, as C compiles one: `mov eax,[a] / mov ecx,[b] / cdq /
/// idiv ecx`, and the remainder from edx. No test of the divisor.
pub fn dividing(site: &CallSite, live: Flag, restore: bool) -> Result<Emitted, String> {
    if !(live & ALL).is_empty() {
        return Err(format!(
            "something reads {} after it, and idiv leaves the flags undefined",
            (live & ALL).repr()
        ));
    }

    let (left, right) = site.operands();
    let divisor = Register::ECX;
    let mut steps: Vec<Instruction> = Vec::new();
    let mut relocated: Vec<(usize, usize)> = Vec::new();
    let mut add = |insn: Instruction| -> usize {
        steps.push(insn);
        steps.len() - 1
    };

    let r#where = add(load_of(&left));
    if let (Kind::Static, Some(at)) = (left.kind, left.at) {
        relocated.push((r#where, at));
    }

    // The shift form computes one answer where a divide hands back two
    // (keeping_the_other), so a relocated divisor is never shifted here.
    if right.kind == Kind::Constant {
        add(made(Instruction::with2(Code::Mov_r32_imm32, divisor, i32_of(right.value))));
    } else {
        let r#where = add(made(Instruction::with2(Code::Mov_r32_rm32, divisor, memory_of(&right))));
        if let Some(at) = right.at {
            relocated.push((r#where, at));
        }
    }

    add(Instruction::with(Code::Cdq));
    add(made(Instruction::with1(Code::Idiv_rm32, divisor)));
    for insn in keeping_the_other(&site.name) {
        add(insn);
    }
    if site.name == REMAINDER {
        add(made(Instruction::with2(Code::Mov_r32_rm32, RESULT, Register::EDX)));
    }
    if restore {
        for insn in restoring() {
            add(insn);
        }
    }

    Ok(assemble(&mut steps, &relocated).unwrap_or_else(|error| panic!("{error}")))
}

#[cfg(test)]
#[path = "calls_tests.rs"]
mod tests;
