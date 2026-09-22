//! Port of `qbopt/legacy/calls.py`: what `asm` reaches through `absorb` --
//! the site and operand records, and the instructions that replace a call.
//!
//! Not ported (the raise's matching half): `PUSHES`, `WIDE_PUSHES`,
//! `static_at`, `widened_constant_at`, `one_operand`, `match`, `_classified`,
//! `sites`. An exception Python lets escape panics with its message.

use std::fmt;
use std::sync::LazyLock;

use iced_x86::{
    BlockEncoder, BlockEncoderOptions, Code, Decoder, DecoderOptions, Instruction, InstructionBlock, MemoryOperand,
    Register,
};

use crate::analysis::flags::{ALL, Flag};
use crate::frontend::declen::{BITNESS, Insn, to_signed};
use crate::frontend::stack::PUSH_BYTES;
use crate::legacy::lift::Emitted;
pub use crate::legacy::lift::relocated_memory;
use crate::objectfile::module::{Addr, Space};
use crate::support::hash::IndexMap;
use crate::support::pyrepr::Repr;

pub const COMPARE: &str = "B$CPI4";
pub const MULTIPLY: &str = "B$MUI4";
pub const DIVIDE: &str = "B$DVI4";
/// The routine that answers a long remainder.
pub const REMAINDER: &str = "B$RMI4";

/// True where the left operand is pushed first. Uniform across the
/// compilers, opposite between comparison and the arithmetic routines.
pub static LEFT_FIRST: LazyLock<IndexMap<&'static str, bool>> = LazyLock::new(|| {
    IndexMap::from_iter([(COMPARE, true), (MULTIPLY, false), (DIVIDE, false), (REMAINDER, false)])
});

/// How many long arguments this routine takes, or None if it is not one
/// absorption knows how to handle at all.
pub fn _arity(name: Option<&str>) -> Option<usize> {
    match name {
        Some(name) if LEFT_FIRST.contains_key(name) => Some(2),
        _ => None,
    }
}

pub const CONSTANTS: [Code; 4] = [Code::Pushw_imm8, Code::Pushd_imm8, Code::Push_imm16, Code::Pushd_imm32];

/// iced's immediate is unsigned; `to_signed` at this width undoes it.
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

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Kind::Static => "static",
            Kind::Constant => "constant",
        })
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
    /// `Operand(kind)`, every other field at its default.
    pub fn new(kind: Kind) -> Self {
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
    /// the raw pushes stack.py found for this call, when match()'s own
    /// contiguous-and-classifiable scan could not
    pub consume: Vec<Insn>,
}

impl CallSite {
    /// (left, right), whichever way this routine takes them.
    pub fn operands(&self) -> (&Operand, &Operand) {
        let [first, second] = self.pushed.as_slice() else {
            panic!("not enough values to unpack (expected 2, got {})", self.pushed.len());
        };
        if LEFT_FIRST[self.name.as_str()] { (first, second) } else { (second, first) }
    }
}

pub fn constant_at(insn: &Insn) -> Option<Operand> {
    if !CONSTANTS.contains(&insn.code()) || insn.imm_at.is_none() {
        return None;
    }
    // A negative constant pushed straight to the stack is otherwise a huge
    // unsigned value, which iced's i32 builder refuses.
    Some(Operand {
        value: to_signed(insn.insn.immediate(0), CONSTANT_WIDTH[&insn.code()]),
        length: 1,
        ..Operand::new(Kind::Constant)
    })
}

pub const SYNTHESISED: Flag = Flag(Flag::CF.0 | Flag::PF.0 | Flag::AF.0);

pub static ABSORBED: LazyLock<IndexMap<&'static str, Code>> =
    LazyLock::new(|| IndexMap::from_iter([(COMPARE, Code::Cmp_r32_rm32), (MULTIPLY, Code::Imul_r32_rm32)]));

/// Divide and remainder are C's: one idiv, no test of the divisor.
pub const DIVIDES: [&str; 2] = [DIVIDE, REMAINDER];

/// What the runtime returns a long in, as ax:dx.
pub const RESULT: Register = Register::EAX;
/// Where a divide leaves the answer its own name does not promise: every
/// such site already declares ebx clobbered.
pub const OTHER: Register = Register::EBX;
/// What a divide's second operand is loaded into.
pub const DIVISOR: Register = Register::ECX;

fn raised<T, E: fmt::Display>(made: Result<T, E>) -> T {
    made.unwrap_or_else(|error| panic!("{error}"))
}

fn i32_of(value: i64) -> i32 {
    raised(i32::try_from(value))
}

/// Where a Kind.STATIC operand's value lives, once it is read at codegen
/// time rather than reused from wherever it was pushed.
pub fn relocated_addr(operand: &Operand) -> Addr {
    match operand.addr {
        Some(addr) if addr.space == Space::Segment => addr,
        _ => panic!("a {} operand has no relocated address", operand.kind),
    }
}

pub fn memory_of(operand: &Operand) -> MemoryOperand {
    relocated_memory(relocated_addr(operand).base, Register::None)
}

pub fn load_of(operand: &Operand) -> Instruction {
    if operand.kind == Kind::Constant {
        return raised(Instruction::with2(Code::Mov_r32_imm32, RESULT, i32_of(operand.value)));
    }
    let base = relocated_addr(operand).base;
    // the moffs form has no ModRM byte and so no way to carry an index register
    let code = if base != Register::None { Code::Mov_r32_rm32 } else { Code::Mov_EAX_moffs32 };
    raised(Instruction::with2(code, RESULT, relocated_memory(base, Register::None)))
}

pub fn fits_in_a_byte(value: i64) -> bool {
    (-128..128).contains(&value)
}

/// A multiply by a constant the 386 can do without multiplying.
pub static SCALES: LazyLock<IndexMap<i64, u32>> = LazyLock::new(|| IndexMap::from_iter([(3, 2), (5, 4), (9, 8)]));

fn bit_length(value: i64) -> i64 {
    i64::from(64 - value.unsigned_abs().leading_zeros())
}

/// One instruction that multiplies RESULT by `value`, or None.
pub fn _without_multiplying(value: i64) -> Option<Instruction> {
    if value <= 0 {
        return None;
    }
    if value & (value - 1) == 0 && value != 1 {
        return Some(raised(Instruction::with2(Code::Shl_rm32_imm8, RESULT, i32_of(bit_length(value) - 1))));
    }
    if let Some(&scale) = SCALES.get(&value) {
        return Some(raised(Instruction::with2(
            Code::Lea_r32_m,
            RESULT,
            MemoryOperand::with_base_index_scale(RESULT, RESULT, scale),
        )));
    }
    None
}

pub fn apply_to(name: &str, operand: &Operand) -> Instruction {
    if operand.kind != Kind::Constant {
        return raised(Instruction::with2(ABSORBED[name], RESULT, memory_of(operand)));
    }
    // the sign-extended byte forms are two or three bytes shorter
    let short = fits_in_a_byte(operand.value);
    if name == COMPARE {
        let code = if short { Code::Cmp_rm32_imm8 } else { Code::Cmp_EAX_imm32 };
        return raised(Instruction::with2(code, RESULT, i32_of(operand.value)));
    }
    if name == MULTIPLY {
        if let Some(cheaper) = _without_multiplying(operand.value) {
            return cheaper;
        }
    }
    let code = if short { Code::Imul_r32_rm32_imm8 } else { Code::Imul_r32_rm32_imm32 };
    raised(Instruction::with3(code, RESULT, RESULT, i32_of(operand.value)))
}

/// The call replaced by 386 instructions, or why it cannot be.
///
/// `Err(reason)` is Python's `str` answer.
pub fn absorb(site: &CallSite, live: Flag, restore: bool) -> Result<Emitted, String> {
    if !site.consume.is_empty() && site.pushed.is_empty() {
        // Popped rather than reloaded, and only where nothing classified
        // the pushes.
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
    // x*x (or x==x): the same address read twice is one load, not two.
    let same_address = left.kind == Kind::Static && right.kind == Kind::Static && left.addr == right.addr;

    let mut steps: Vec<Instruction> = Vec::new();
    let mut relocated: IndexMap<usize, usize> = IndexMap::default();

    if site.name == COMPARE {
        steps.push(raised(Instruction::with1(Code::Push_r32, RESULT)));
    }

    steps.push(load_of(left));
    if left.kind == Kind::Static {
        if let Some(at) = left.at {
            relocated.insert(steps.len() - 1, at);
        }
    }

    if same_address {
        steps.push(raised(Instruction::with2(ABSORBED[site.name.as_str()], RESULT, RESULT)));
    } else {
        steps.push(apply_to(&site.name, right));
        if right.kind == Kind::Static {
            if let Some(at) = right.at {
                relocated.insert(steps.len() - 1, at);
            }
        }
    }

    if site.name == COMPARE {
        // pop does not touch the flags the cmp above just set
        steps.push(raised(Instruction::with1(Code::Pop_r32, RESULT)));
    } else if restore {
        // a multiply leaves a value, and BC reads its high half from dx
        steps.extend(restoring());
    }

    Ok(raised(assemble(&mut steps, &relocated.into_iter().collect::<Vec<_>>())))
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
        raised(Instruction::with1(Code::Push_r32, RESULT)),
        raised(Instruction::with1(Code::Pop_r16, Register::AX)),
        raised(Instruction::with1(Code::Pop_r16, Register::DX)),
    ]
}

/// One argument's worth of pushes per group, deepest first, or None if
/// they do not split cleanly into 4-byte arguments.
pub fn grouped(pushed: &[Insn]) -> Option<Vec<Vec<Insn>>> {
    let mut groups: Vec<Vec<Insn>> = Vec::new();
    let mut remaining: Vec<Insn> = pushed.to_vec();
    while !remaining.is_empty() {
        let mut have = 0;
        let mut take: Vec<Insn> = Vec::new();
        while have < 4 {
            let Some(insn) = remaining.pop() else { break };
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

/// One argument, off the real stack and into `target`.
pub fn popped_into(target: Register) -> Instruction {
    raised(Instruction::with1(Code::Pop_r32, target))
}

/// Which physical register each pop lands in, topmost group to deepest.
pub static CONSUME_TARGETS: LazyLock<IndexMap<&'static str, (Register, Register)>> = LazyLock::new(|| {
    IndexMap::from_iter([
        (MULTIPLY, (Register::EAX, Register::ECX)),
        (DIVIDE, (Register::EAX, Register::ECX)),
        (REMAINDER, (Register::EAX, Register::ECX)),
    ])
});

/// A popped compare, without popping: B$CPI4 changes no register at all,
/// and its two arguments have to come off the stack as a real call's
/// callee-cleanup would remove them.
pub fn compare_consume() -> Emitted {
    let at = |displacement: i64| MemoryOperand::with_base_displ_size(Register::BP, displacement, 1);
    let mut steps: Vec<Instruction> = vec![
        raised(Instruction::with1(Code::Push_r16, Register::BP)),
        raised(Instruction::with1(Code::Push_r32, Register::EDX)),
        raised(Instruction::with2(Code::Mov_r16_rm16, Register::BP, Register::SP)),
        // +6 pushed ahead of the arguments (bp, then edx) puts left at +10
        // and right, the topmost original argument, at +6
        raised(Instruction::with2(Code::Mov_r32_rm32, Register::EDX, at(10))),
        raised(Instruction::with2(Code::Cmp_r32_rm32, Register::EDX, at(6))),
        // everything from here on must leave the flags alone
        raised(Instruction::with2(Code::Mov_r16_rm16, Register::DX, at(4))),
        // +12 is the top two bytes of the left argument's own four -- already
        // read into edx above, so overwriting them here is safe
        raised(Instruction::with2(Code::Mov_rm16_r16, at(12), Register::DX)),
        raised(Instruction::with2(Code::Mov_r32_rm32, Register::EDX, at(0))),
        raised(Instruction::with2(Code::Lea_r16_m, Register::SP, at(12))),
        raised(Instruction::with1(Code::Pop_r16, Register::BP)),
    ];
    raised(assemble(&mut steps, &[]))
}

/// A call whose arguments only the stack knows, popped rather than reloaded.
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
    if arity != Some(groups.len()) {
        let said = arity.map_or_else(|| "None".to_owned(), |one| one.to_string());
        return Err(format!("{} takes {said} arguments, not {}", site.name, groups.len()));
    }

    if site.name == COMPARE {
        return Ok(compare_consume());
    }

    // each argument is exactly one dword, popped topmost to deepest
    let (first, second) = CONSUME_TARGETS[site.name.as_str()];
    let mut steps: Vec<Instruction> = vec![popped_into(first), popped_into(second)];

    // grouped() returns deepest first, and CONSUME_TARGETS pops topmost
    // first, so the divisor is groups[0] and the dividend groups[-1].
    let divides = DIVIDES.contains(&site.name.as_str());
    let divisor = if divides && groups.len() == 2 { Some(&groups[0]) } else { None };
    let known = match divisor {
        Some(divisor) if divisor.len() == 1 => constant_at(&divisor[0]),
        _ => None,
    };
    let shift = known.and_then(|known| _power_of_two(known.value));

    if site.name == MULTIPLY {
        steps.push(raised(Instruction::with2(Code::Imul_r32_rm32, Register::EAX, Register::ECX)));
    } else if let Some(shift) = shift {
        steps.extend(dividing_by_a_power_of_two(&site.name, shift, Register::ECX));
    } else if divides {
        steps.push(Instruction::with(Code::Cdq));
        steps.push(raised(Instruction::with1(Code::Idiv_rm32, Register::ECX)));
        steps.extend(keeping_the_other(&site.name));
        if site.name == REMAINDER {
            steps.push(raised(Instruction::with2(Code::Mov_r32_rm32, RESULT, Register::EDX)));
        }
    }
    if restore {
        steps.extend(restoring());
    }
    Ok(raised(assemble(&mut steps, &[])))
}

/// The move that puts a divide's unasked-for answer somewhere it lasts.
pub fn keeping_the_other(name: &str) -> Vec<Instruction> {
    if name == DIVIDE {
        return vec![raised(Instruction::with2(Code::Mov_r32_rm32, OTHER, Register::EDX))];
    }
    if name == REMAINDER {
        return vec![raised(Instruction::with2(Code::Mov_r32_rm32, OTHER, RESULT))];
    }
    Vec::new()
}

/// Where this site leaves the answer its own name does not promise.
pub fn other_result(site: &CallSite) -> Option<Register> {
    DIVIDES.contains(&site.name.as_str()).then_some(OTHER)
}

/// n where value is 2**n, for 1 <= n <= 31, else None.
pub fn _power_of_two(value: i64) -> Option<i64> {
    if value <= 1 || value & (value - 1) != 0 {
        return None;
    }
    let n = bit_length(value) - 1;
    (1..=31).contains(&n).then_some(n)
}

/// RESULT divided by 2**n, or its remainder, without an idiv.
pub fn dividing_by_a_power_of_two(name: &str, n: i64, scratch: Register) -> Vec<Instruction> {
    let mut steps = vec![
        raised(Instruction::with2(Code::Mov_r32_rm32, scratch, RESULT)),
        raised(Instruction::with2(Code::Sar_rm32_imm8, scratch, 31)),
        raised(Instruction::with2(Code::Shr_rm32_imm8, scratch, i32_of(32 - n))),
    ];
    if name == REMAINDER {
        steps.push(raised(Instruction::with2(Code::Add_r32_rm32, scratch, RESULT)));
        steps.push(raised(Instruction::with2(Code::And_rm32_imm32, scratch, i32_of(-(1_i64 << n)))));
        steps.push(raised(Instruction::with2(Code::Sub_r32_rm32, RESULT, scratch)));
        return steps;
    }
    steps.push(raised(Instruction::with2(Code::Add_r32_rm32, RESULT, scratch)));
    steps.push(raised(Instruction::with2(Code::Sar_rm32_imm8, RESULT, i32_of(n))));
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
    let mut relocated: IndexMap<usize, usize> = IndexMap::default();

    steps.push(load_of(left));
    if left.kind == Kind::Static {
        if let Some(at) = left.at {
            relocated.insert(steps.len() - 1, at);
        }
    }

    if right.kind == Kind::Constant {
        steps.push(raised(Instruction::with2(Code::Mov_r32_imm32, divisor, i32_of(right.value))));
    } else {
        steps.push(raised(Instruction::with2(Code::Mov_r32_rm32, divisor, memory_of(right))));
        if let Some(at) = right.at {
            relocated.insert(steps.len() - 1, at);
        }
    }

    steps.push(Instruction::with(Code::Cdq));
    steps.push(raised(Instruction::with1(Code::Idiv_rm32, divisor)));
    steps.extend(keeping_the_other(&site.name));
    if site.name == REMAINDER {
        steps.push(raised(Instruction::with2(Code::Mov_r32_rm32, RESULT, Register::EDX)));
    }
    if restore {
        steps.extend(restoring());
    }

    Ok(raised(assemble(&mut steps, &relocated.into_iter().collect::<Vec<_>>())))
}

#[cfg(test)]
#[path = "calls_tests.rs"]
mod tests;
