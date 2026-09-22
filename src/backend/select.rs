//! MIR to machine bytes: the instructions this pass writes itself.
//!
//! Port of `qbopt/backend/select.py`. Python's iced `Instruction.create_*`
//! calls are iced's own `Instruction::with*`, so every encoding is the same
//! library's answer. An exception Python lets escape (an immediate iced
//! refuses outside a `try`, an index out of range) panics here with the
//! same message.

use crate::support::hash::HashMap;
use std::sync::LazyLock;

use iced_x86::{
    Code, Decoder, DecoderOptions, Encoder, Instruction, MemoryOperand, Register, RepPrefixKind,
};
use crate::support::hash::IndexMap;

use crate::backend::target;
use crate::frontend::declen::BITNESS;
use crate::legacy::calls as machine;
use crate::model::ir::{self, Loc, Operation, Semantics, Space};
use crate::model::mir;
use crate::support::pyrepr::Repr;

// The register file's own tables, not a copy of them.
pub use crate::backend::target::{AT_WIDTH, WIDTHS};

/// The bytes, and where a relocated displacement ended up inside them.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Emitted {
    pub code: Vec<u8>,
    pub displacement_at: Option<usize>,
    pub immediate_at: Option<usize>,
    /// Where each relocatable field landed, in the order the instructions
    /// were emitted.
    pub fields: Vec<usize>,
    /// Whether a displacement here can be a symbol at all.
    pub symbolic: bool,
}

impl Emitted {
    /// `Emitted(code)`, every other field at its default.
    pub fn new(code: Vec<u8>) -> Self {
        Self { code, displacement_at: None, immediate_at: None, fields: Vec::new(), symbolic: true }
    }

    /// Where this instruction's own relocatable field landed.
    pub fn relocated_at(&self) -> Option<usize> {
        if !self.fields.is_empty() {
            return Some(self.fields[0]);
        }
        if !self.symbolic {
            return None;
        }
        if self.displacement_at.is_some() { self.displacement_at } else { self.immediate_at }
    }

    /// Every field, however this was built.
    pub fn places(&self) -> Vec<usize> {
        if !self.fields.is_empty() {
            return self.fields.clone();
        }
        match self.relocated_at() {
            None => Vec::new(),
            Some(one) => vec![one],
        }
    }
}

/// A register, or a cell: Python's `Register_ | ir.Mem` parameters.
#[derive(Clone, Copy, Debug)]
pub enum RegisterOrCell<'a> {
    Reg(Register),
    Mem(&'a ir::Mem),
}

/// Python's `dict[Register_, Register_]`.
pub type RegisterMap = IndexMap<Register, Register>;
/// Python's `held` dict: an SSA value to the register it was allocated.
pub type HeldMap = IndexMap<u32, Register>;

/// `emit`'s `where`: one map for both sides, or `(into, outof)`.
#[derive(Clone, Copy, Debug)]
pub enum Where<'a> {
    One(&'a RegisterMap),
    Pair(&'a RegisterMap, &'a RegisterMap),
}

// Python iced's `Instruction.create_*`, which raise where Rust's `with*`
// returns `Err`. The `Err` text is the Python exception's.

fn overflow(error: std::num::TryFromIntError) -> String {
    error.to_string()
}

fn i32_of(value: i64) -> Result<i32, String> {
    i32::try_from(value).map_err(overflow)
}

fn create_reg(code: Code, register: Register) -> Result<Instruction, String> {
    Instruction::with1(code, register).map_err(|error| error.to_string())
}

fn create_i32(code: Code, value: i64) -> Result<Instruction, String> {
    Instruction::with1(code, i32_of(value)?).map_err(|error| error.to_string())
}

fn create_u32(code: Code, value: i64) -> Result<Instruction, String> {
    let value = u32::try_from(value).map_err(overflow)?;
    Instruction::with1(code, value).map_err(|error| error.to_string())
}

fn create_mem(code: Code, memory: MemoryOperand) -> Result<Instruction, String> {
    Instruction::with1(code, memory).map_err(|error| error.to_string())
}

fn create_reg_reg(code: Code, one: Register, other: Register) -> Result<Instruction, String> {
    Instruction::with2(code, one, other).map_err(|error| error.to_string())
}

fn create_reg_i32(code: Code, register: Register, value: i64) -> Result<Instruction, String> {
    Instruction::with2(code, register, i32_of(value)?).map_err(|error| error.to_string())
}

fn create_reg_mem(code: Code, register: Register, memory: MemoryOperand) -> Result<Instruction, String> {
    Instruction::with2(code, register, memory).map_err(|error| error.to_string())
}

fn create_mem_reg(code: Code, memory: MemoryOperand, register: Register) -> Result<Instruction, String> {
    Instruction::with2(code, memory, register).map_err(|error| error.to_string())
}

fn create_mem_i32(code: Code, memory: MemoryOperand, value: i64) -> Result<Instruction, String> {
    Instruction::with2(code, memory, i32_of(value)?).map_err(|error| error.to_string())
}

fn create_reg_reg_reg(code: Code, one: Register, two: Register, three: Register) -> Result<Instruction, String> {
    Instruction::with3(code, one, two, three).map_err(|error| error.to_string())
}

fn create_reg_reg_i32(code: Code, one: Register, two: Register, value: i64) -> Result<Instruction, String> {
    Instruction::with3(code, one, two, i32_of(value)?).map_err(|error| error.to_string())
}

fn create_reg_mem_i32(code: Code, one: Register, memory: MemoryOperand, value: i64) -> Result<Instruction, String> {
    Instruction::with3(code, one, memory, i32_of(value)?).map_err(|error| error.to_string())
}

fn create_branch(code: Code, target: i64) -> Result<Instruction, String> {
    let target = u64::try_from(target).map_err(overflow)?;
    Instruction::with_branch(code, target).map_err(|error| error.to_string())
}

/// An exception Python does not catch.
fn raised<T>(made: Result<T, String>) -> T {
    made.unwrap_or_else(|error| panic!("{error}"))
}

fn width_of(register: Register) -> Option<i64> {
    target::WIDTHS.get(&register).copied()
}

/// The bytes, with both constant fields located.
pub fn _assemble(made: &Instruction, at: u64, symbolic: bool) -> Option<Emitted> {
    let mut encoder = Encoder::new(BITNESS);
    if encoder.encode(made, at).is_err() {
        return None;
    }
    let code = encoder.take_buffer();
    let mut decoder = Decoder::with_ip(BITNESS, &code, at, DecoderOptions::NONE);
    if !decoder.can_decode() {
        return None;
    }
    let decoded = decoder.decode();
    let offsets = decoder.get_constant_offsets(&decoded);
    // A displacement is a relocatable field only where the operand names a
    // symbol; a frame slot's displacement is an offset from bp.
    let absolute = offsets.has_displacement() && symbolic;
    let r#where = if absolute {
        Some(offsets.displacement_offset())
    } else if offsets.has_immediate() {
        Some(offsets.immediate_offset())
    } else {
        None
    };
    Some(Emitted {
        displacement_at: if offsets.has_displacement() { Some(offsets.displacement_offset()) } else { None },
        immediate_at: if offsets.has_immediate() { Some(offsets.immediate_offset()) } else { None },
        fields: r#where.into_iter().collect(),
        symbolic,
        code,
    })
}

/// How many bytes the displacement needs.
pub fn _displacement_size(base: Register, value: i64) -> u32 {
    if value == 0 && base != Register::BP {
        return 0;
    }
    if (-128..=127).contains(&value) { 1 } else { 2 }
}

fn memory_operand(base: Register, index: Register, scale: i64, displ: i64, displ_size: u32, seg: Register) -> MemoryOperand {
    MemoryOperand::new(base, index, scale as u32, displ, displ_size, false, seg)
}

/// `what` as an encodable memory operand, and whether it is relocated.
pub fn operand_of(what: &ir::Mem) -> Option<(MemoryOperand, bool)> {
    let addr = what.addr;
    if what.index.is_some() {
        return _scaled_operand(what);
    }
    let Some(addr) = addr else {
        // Encodable where it is reached through a register.
        if [Register::SI, Register::DI, Register::BX, Register::BP].contains(&what.through) {
            let wide = if what.disp_width != 0 {
                what.disp_width
            } else {
                _displacement_size(what.through, what.offset)
            };
            return Some((memory_operand(what.through, Register::None, 1, what.offset, wide, Register::None), false));
        }
        return None;
    };
    match addr.space {
        Space::Segment | Space::External => {
            // Once a pass makes a value of the offset, `through` is where the
            // allocation put it.
            let base = if what.base.is_some() { what.through } else { addr.base };
            Some((memory_operand(base, Register::None, 1, 0, 2, addr.segment), true))
        }
        Space::Frame if addr.base == Register::None => {
            let index = what.index_through;
            if index != Register::None && !_WORD_INDEXES.contains(&index) {
                return None;
            }
            Some((
                memory_operand(
                    Register::BP,
                    index,
                    1,
                    addr.disp,
                    _displacement_size(Register::BP, addr.disp),
                    Register::None,
                ),
                false,
            ))
        }
        Space::Far => {
            // A $DYNAMIC array element: the override is part of the address.
            if addr.segment == Register::None {
                return None;
            }
            let base = if what.base.is_some() { what.through } else { addr.base };
            Some((
                memory_operand(base, Register::None, 1, addr.disp, _displacement_size(base, addr.disp), addr.segment),
                false,
            ))
        }
        Space::Literal => {
            // Two bytes without a base: 16-bit mod=00 r/m=110 is the
            // direct-address form, and mod=01 would mean `[bp+disp8]`.
            let base = if what.base.is_some() { what.through } else { addr.base };
            let wide = if base == Register::None { 2 } else { _displacement_size(base, addr.disp) };
            Some((memory_operand(base, Register::None, 1, addr.disp, wide, addr.segment), false))
        }
        _ => None,
    }
}

pub const _WORD_BASES: [Register; 2] = [Register::BX, Register::BP];
pub const _WORD_INDEXES: [Register; 2] = [Register::SI, Register::DI];

/// `[base+index*scale+disp]`, for a cell no fixup names.
pub fn _scaled_operand(what: &ir::Mem) -> Option<(MemoryOperand, bool)> {
    let addr = what.addr?;
    if !matches!(addr.space, Space::Far | Space::Literal) || what.index_through == Register::None {
        return None;
    }
    if addr.space == Space::Far && addr.segment == Register::None {
        return None;
    }
    let (base, disp) = (what.through, addr.disp);
    let segment = if matches!(addr.space, Space::Far | Space::Literal) { addr.segment } else { Register::None };
    if _WORD_INDEXES.contains(&what.index_through) {
        if what.scale != 1 || !_WORD_BASES.contains(&base) {
            return None;
        }
        let size = _displacement_size(base, disp);
        return Some((memory_operand(base, what.index_through, 1, disp, size, segment), false));
    }
    let size = if base == Register::None {
        4
    } else if disp == 0 && base != Register::EBP {
        0
    } else if (-128..=127).contains(&disp) {
        1
    } else {
        4
    };
    Some((memory_operand(base, what.index_through, what.scale, disp, size, segment), false))
}

/// `mov into, outof`, or None if this cannot name that pair.
pub fn r#move(into: Register, outof: Register, at: u64) -> Option<Emitted> {
    if into == outof {
        return Some(Emitted::new(Vec::new())); // a move to itself is no instruction at all
    }
    let code = if target::WIDE.contains(&into) && target::WIDE.contains(&outof) {
        Code::Mov_r32_rm32
    } else if target::NARROW.contains(&into) && target::NARROW.contains(&outof) {
        Code::Mov_r16_rm16
    } else if target::BYTE.contains(&into) && target::BYTE.contains(&outof) {
        Code::Mov_r8_rm8
    } else {
        return None;
    };
    _assemble(&raised(create_reg_reg(code, into, outof)), at, true)
}

pub const TWO_OPERAND: [&str; 8] = ["add", "adc", "sub", "sbb", "and", "or", "xor", "cmp"];
pub const ONE_OPERAND: [&str; 4] = ["neg", "not", "inc", "dec"];

static CODES: LazyLock<HashMap<String, Code>> =
    LazyLock::new(|| Code::values().map(|code| (format!("{code:?}").to_uppercase(), code)).collect());

/// iced's Code value by its own name, or None where there is no such form.
///
/// Python's `getattr(Code, name)`; iced's Rust names are the same names in
/// another case.
pub fn _code(name: &str) -> Option<Code> {
    CODES.get(name).copied()
}

pub fn _width_of(what: &Loc) -> Option<i64> {
    match what {
        Loc::Reg(one) => width_of(one.register),
        Loc::Imm(one) => {
            if one.width == 2 || one.width == 4 {
                Some(i64::from(one.width))
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn _remapped(register: Register, r#where: Option<&RegisterMap>) -> Register {
    r#where.and_then(|map| map.get(&register)).copied().unwrap_or(register)
}

/// One operand with every register in it remapped.
pub fn _operand(one: &Loc, r#where: Option<&RegisterMap>, held: Option<&HeldMap>) -> Loc {
    // A Held names a value, not a register, and the allocation says which
    // register that is.
    let mut one = one.clone();
    if let Loc::Held(value) = &one {
        let Some(got) = held.and_then(|map| map.get(&value.value)).copied() else {
            return one; // emit() refuses it
        };
        let wide = target::AT_WIDTH
            .get(&ir::root(got))
            .and_then(|widths| widths.get(&i64::from(value.width)))
            .copied()
            .unwrap_or(got);
        one = Loc::Reg(ir::Reg { register: wide, width: value.width });
    }
    let Some(map) = r#where.filter(|map| !map.is_empty()) else {
        return one;
    };
    match one {
        Loc::Reg(reg) => Loc::Reg(ir::Reg { register: _remapped(reg.register, Some(map)), ..reg }),
        Loc::Mem(mut cell) => {
            // `index` is a Held on a cell, which no register map names.
            cell.through = _remapped(cell.through, Some(map));
            if let Some(addr) = cell.addr.as_mut() {
                if addr.base != Register::None {
                    addr.base = _remapped(addr.base, Some(map));
                }
            }
            Loc::Mem(cell)
        }
        Loc::Address(mut cell) => {
            cell.through = _remapped(cell.through, Some(map));
            cell.index = _remapped(cell.index, Some(map));
            if let Some(addr) = cell.addr.as_mut() {
                if addr.base != Register::None {
                    addr.base = _remapped(addr.base, Some(map));
                }
            }
            Loc::Address(cell)
        }
        other => other,
    }
}

/// Signed at its width: a word's -1 arrives as 65535 as often as -1.
pub fn _immediate(value: i64, width: i64) -> i64 {
    if width != 2 && width != 4 {
        return value;
    }
    let sign = 1_i64 << (8 * width - 1);
    ((value & (2 * sign - 1)) ^ sign) - sign
}

/// `mov into, imm`, at the width `into` names.
pub fn load(into: Register, value: i64, at: u64) -> Option<Emitted> {
    let width = width_of(into)?;
    let code = _code(&format!("MOV_R{}_IMM{}", width * 8, width * 8))?;
    let made = create_reg_i32(code, into, _immediate(value, width)).ok()?;
    _assemble(&made, at, true)
}

/// `<name> dest, source`, both registers, at the width they name.
pub fn arith(name: &str, dest: Register, source: Register, at: u64) -> Option<Emitted> {
    if !TWO_OPERAND.contains(&name) {
        return None;
    }
    let width = width_of(dest)?;
    if width_of(source) != Some(width) {
        return None; // a mixed-width operation is a different instruction
    }
    let code = _code(&format!("{}_R{}_RM{}", name.to_uppercase(), width * 8, width * 8))?;
    _assemble(&raised(create_reg_reg(code, dest, source)), at, true)
}

/// Whether the sign-extended one-byte form says the same number.
pub fn fits_in_a_byte(value: i64) -> bool {
    (-128..=127).contains(&value)
}

/// The accumulator at each width.
///
/// Python binds `ACCUMULATOR` twice, `{2: AX, 4: EAX}` and later this; every
/// function reads the global at call time, so only this binding is ever seen.
pub static ACCUMULATOR: LazyLock<IndexMap<i64, Register>> =
    LazyLock::new(|| IndexMap::from_iter([(1, Register::AL), (2, Register::AX), (4, Register::EAX)]));

/// `<name> dest, imm`, in the shortest form the value and register allow.
pub fn arith_imm(name: &str, dest: Register, value: i64, at: u64, relocated: bool) -> Option<Emitted> {
    if !TWO_OPERAND.contains(&name) {
        return None;
    }
    let width = width_of(dest)?;
    let mut shapes: Vec<(String, i64)> = Vec::new();
    let value = _immediate(value, width);
    let upper = name.to_uppercase();
    if fits_in_a_byte(value) && !relocated {
        shapes.push((format!("{upper}_RM{}_IMM8", width * 8), 8));
    }
    if Some(&dest) == ACCUMULATOR.get(&width) {
        let accumulator = if width == 2 { "AX" } else { "EAX" };
        shapes.push((format!("{upper}_{accumulator}_IMM{}", width * 8), width * 8));
    }
    shapes.push((format!("{upper}_RM{}_IMM{}", width * 8, width * 8), width * 8));
    for (shape, _bits) in shapes {
        let Some(code) = _code(&shape) else {
            continue;
        };
        match create_reg_i32(code, dest, value) {
            Ok(made) => return _assemble(&made, at, true),
            Err(_) => continue,
        }
    }
    None
}

/// `neg`, `not`, `inc` or `dec` of one register.
pub fn unary(name: &str, dest: Register, at: u64) -> Option<Emitted> {
    if !ONE_OPERAND.contains(&name) {
        return None;
    }
    let width = width_of(dest)?;
    let upper = name.to_uppercase();
    for shape in [format!("{upper}_R{}", width * 8), format!("{upper}_RM{}", width * 8)] {
        let Some(code) = _code(&shape) else {
            continue;
        };
        match create_reg(code, dest) {
            Ok(made) => return _assemble(&made, at, true),
            Err(_) => continue,
        }
    }
    None
}

/// A register onto the stack, at the width it names.
pub fn push(one: Register, at: u64) -> Option<Emitted> {
    let width = width_of(one)?;
    let code = _code(&format!("PUSH_R{}", width * 8))?;
    _assemble(&raised(create_reg(code, one)), at, true)
}

/// A literal onto the stack, at the width the operand names.
pub fn push_imm(value: i64, width: i64, at: u64, relocated: bool) -> Option<Emitted> {
    let value = _immediate(value, width);
    let mut names = vec![if width == 4 { "PUSHD_IMM32" } else { "PUSH_IMM16" }];
    if fits_in_a_byte(value) && !relocated {
        names.insert(0, if width == 4 { "PUSHD_IMM8" } else { "PUSHW_IMM8" });
    }
    for one in names {
        let Some(code) = _code(one) else {
            continue;
        };
        match create_i32(code, value) {
            Ok(made) => return _assemble(&made, at, true),
            Err(_) => continue,
        }
    }
    None
}

pub static MOFFS_LOAD: LazyLock<IndexMap<i64, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([(1, "MOV_AL_MOFFS8"), (2, "MOV_AX_MOFFS16"), (4, "MOV_EAX_MOFFS32")])
});
pub static MOFFS_STORE: LazyLock<IndexMap<i64, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([(1, "MOV_MOFFS8_AL"), (2, "MOV_MOFFS16_AX"), (4, "MOV_MOFFS32_EAX")])
});

/// The accumulator form, where this is one it applies to.
pub fn _moffs(shape: &IndexMap<i64, &'static str>, register: Register, cell: &ir::Mem, width: i64) -> Option<Code> {
    if Some(&register) != ACCUMULATOR.get(&width) || cell.addr.is_none_or(|addr| addr.base != Register::None) {
        return None;
    }
    _code(shape.get(&width).copied().unwrap_or(""))
}

/// `mov into, [cell]`.
pub fn move_from(into: Register, cell: &ir::Mem, at: u64) -> Option<Emitted> {
    let width = width_of(into);
    let built = operand_of(cell);
    let (Some(width), Some((r#where, relocated))) = (width, built) else {
        return None;
    };
    if i64::from(cell.width) != width {
        return None;
    }
    if let Some(short) = _moffs(&MOFFS_LOAD, into, cell, width) {
        let made = _assemble(&raised(create_reg_mem(short, into, r#where)), at, relocated);
        if made.is_some() {
            return made;
        }
    }
    let code = _code(&format!("MOV_R{}_RM{}", width * 8, width * 8))?;
    _assemble(&raised(create_reg_mem(code, into, r#where)), at, relocated)
}

/// `mov [cell], outof`.
pub fn move_into(cell: &ir::Mem, outof: Register, at: u64) -> Option<Emitted> {
    let width = width_of(outof);
    let built = operand_of(cell);
    let (Some(width), Some((r#where, relocated))) = (width, built) else {
        return None;
    };
    if i64::from(cell.width) != width {
        return None;
    }
    if let Some(short) = _moffs(&MOFFS_STORE, outof, cell, width) {
        let made = _assemble(&raised(create_mem_reg(short, r#where, outof)), at, relocated);
        if made.is_some() {
            return made;
        }
    }
    let code = _code(&format!("MOV_RM{}_R{}", width * 8, width * 8))?;
    _assemble(&raised(create_mem_reg(code, r#where, outof)), at, relocated)
}

/// `mov [cell], imm`, at the cell's own width.
pub fn store_imm(cell: &ir::Mem, value: i64, at: u64) -> Option<Emitted> {
    let built = operand_of(cell);
    if built.is_none() || ![1, 2, 4].contains(&cell.width) {
        return None;
    }
    let width = i64::from(cell.width);
    let code = _code(&format!("MOV_RM{}_IMM{}", width * 8, width * 8))?;
    let (r#where, relocated) = built?;
    let made = create_mem_i32(code, r#where, _immediate(value, width)).ok()?;
    _assemble(&made, at, relocated)
}

/// `<name> dest, [cell]`.
pub fn arith_mem(name: &str, dest: Register, cell: &ir::Mem, at: u64) -> Option<Emitted> {
    let width = width_of(dest);
    let built = operand_of(cell);
    let (Some(width), Some((r#where, relocated))) = (width, built) else {
        return None;
    };
    if !TWO_OPERAND.contains(&name) || i64::from(cell.width) != width {
        return None;
    }
    let code = _code(&format!("{}_R{}_RM{}", name.to_uppercase(), width * 8, width * 8))?;
    _assemble(&raised(create_reg_mem(code, dest, r#where)), at, relocated)
}

/// `push [cell]`, at the cell's own width.
pub fn push_mem(cell: &ir::Mem, at: u64) -> Option<Emitted> {
    let built = operand_of(cell);
    if built.is_none() || ![2, 4].contains(&cell.width) {
        return None;
    }
    let code = _code(&format!("PUSH_RM{}", cell.width * 8))?;
    let (r#where, relocated) = built?;
    _assemble(&raised(create_mem(code, r#where)), at, relocated)
}

/// `<name> target`, a conditional branch to an absolute address.
pub fn branch(name: &str, target: i64, at: u64, short: bool) -> Option<Emitted> {
    let upper = name.to_uppercase();
    let code = _code(&if short { format!("{upper}_REL8_16") } else { format!("{upper}_REL16") })?;
    let made = create_branch(code, target).ok()?;
    _assemble(&made, at, true)
}

/// `jmp target`, near unless the short form is asked for.
pub fn jump(target: i64, at: u64, short: bool) -> Option<Emitted> {
    let code = _code(if short { "JMP_REL8_16" } else { "JMP_REL16" })?;
    let made = create_branch(code, target).ok()?;
    _assemble(&made, at, true)
}

/// `call target`, within this segment.
pub fn call_near(target: i64, at: u64) -> Option<Emitted> {
    let code = _code("CALL_REL16")?;
    let made = create_branch(code, target).ok()?;
    _assemble(&made, at, true)
}

/// `jmp far ptr 0:0`, the far end of a body BC jumps out of.
pub fn jump_far(_at: u64) -> Option<Emitted> {
    Some(Emitted { displacement_at: Some(1), ..Emitted::new(vec![0xEA, 0, 0, 0, 0]) })
}

/// `call far ptr 0:0`, the shape BC emits for every runtime call.
pub fn call_far(_at: u64) -> Option<Emitted> {
    let made = vec![0x9A, 0, 0, 0, 0];
    Some(Emitted { displacement_at: Some(1), ..Emitted::new(made) })
}

/// The forms that carry no address and no target.
pub static BARE: LazyLock<IndexMap<&'static str, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([
        ("wait", "WAIT"),
        ("nop", "NOPW"),
        ("ret", "RETNW"),
        ("retf", "RETFW"),
        ("cwd", "CWD"),
        ("cdq", "CDQ"),
        // PDS /Ot closes a procedure with it: `mov sp,bp` then `pop bp`.
        ("leave", "LEAVEW"),
        // the x87 ones that take no operand at all
        ("fsqrt", "FSQRT"),
        ("fchs", "FCHS"),
        ("fabs", "FABS"),
        ("fld1", "FLD1"),
        ("fldz", "FLDZ"),
        ("fcompp", "FCOMPP"),
        ("sahf", "SAHF"),
    ])
});
/// The x87 control and status words, each against a word of memory.
pub static CONTROL_WORD: LazyLock<IndexMap<&'static str, &'static str>> =
    LazyLock::new(|| IndexMap::from_iter([("fldcw", "FLDCW_M2BYTE"), ("fnstcw", "FNSTCW_M2BYTE")]));

/// An instruction with no operands at all.
pub fn bare(name: &str, at: u64) -> Option<Emitted> {
    let code = _code(BARE.get(name).copied().unwrap_or(""))?;
    _assemble(&Instruction::with(code), at, true)
}

/// `pop into`, at the width it names.
pub fn pop(into: Register, at: u64) -> Option<Emitted> {
    let width = width_of(into)?;
    let code = _code(&format!("POP_R{}", width * 8))?;
    _assemble(&raised(create_reg(code, into)), at, true)
}

/// `retf n`, which is how every BC procedure ends.
pub fn ret_far(popped: i64, at: u64) -> Option<Emitted> {
    let code = _code(if popped != 0 { "RETFW_IMM16" } else { "RETFW" })?;
    let made = if popped != 0 { raised(create_i32(code, popped)) } else { Instruction::with(code) };
    _assemble(&made, at, true)
}

pub fn test_immediate(dest: &Loc, value: i64, at: u64) -> Option<Emitted> {
    let width = match dest {
        Loc::Reg(one) => one.width,
        Loc::Mem(one) => one.width,
        _ => return None,
    };
    if ![1, 2, 4].contains(&width) {
        return None;
    }
    let width = i64::from(width);
    let code = _code(&format!("TEST_RM{}_IMM{}", width * 8, width * 8))?;
    let value = _immediate(value, width);
    match dest {
        Loc::Reg(one) => _assemble(&raised(create_reg_i32(code, one.register, value)), at, true),
        Loc::Mem(cell) => {
            let (r#where, relocated) = operand_of(cell)?;
            _assemble(&raised(create_mem_i32(code, r#where, value)), at, relocated)
        }
        _ => None,
    }
}

/// `cmp <dest>, imm`. Flags are the whole result, so there is no dest.
pub fn compare(dest: &Loc, value: i64, at: u64, relocated: bool) -> Option<Emitted> {
    match dest {
        Loc::Reg(one) if value == 0 && !relocated => {
            // `test reg,reg` asks the same question a byte shorter. Not when
            // relocated: `cmp ax,offset X` arrives here as `cmp ax,0`.
            let made = compare_registers("test", one.register, one.register, at);
            if made.is_some() {
                return made;
            }
            arith_imm("cmp", one.register, value, at, relocated)
        }
        Loc::Reg(one) => arith_imm("cmp", one.register, value, at, relocated),
        Loc::Mem(cell) => arith_into_imm("cmp", cell, value, at, relocated),
        _ => None,
    }
}

pub static FLOAT_SIZED: LazyLock<IndexMap<u32, &'static str>> =
    LazyLock::new(|| IndexMap::from_iter([(4, "M32FP"), (8, "M64FP"), (10, "M80FP")]));
pub static INT_SIZED: LazyLock<IndexMap<u32, &'static str>> =
    LazyLock::new(|| IndexMap::from_iter([(2, "M16INT"), (4, "M32INT"), (8, "M64INT")]));
pub const FLOAT_MEMORY: [&str; 11] =
    ["fld", "fstp", "fst", "fadd", "fsub", "fmul", "fdiv", "fsubr", "fdivr", "fcom", "fcomp"];
pub const INT_MEMORY: [&str; 9] = ["fild", "fistp", "fist", "fiadd", "fisub", "fimul", "fidiv", "fisubr", "fidivr"];

/// An x87 instruction against memory -- `fld [x]`, `fmul [x]`, `fistp [x]`.
pub fn float_memory(name: &str, cell: &ir::Mem, at: u64) -> Option<Emitted> {
    let sized = if INT_MEMORY.contains(&name) {
        &*INT_SIZED
    } else if FLOAT_MEMORY.contains(&name) {
        &*FLOAT_SIZED
    } else {
        return None;
    };
    let built = operand_of(cell);
    let suffix = sized.get(&cell.width);
    let (Some((r#where, relocated)), Some(suffix)) = (built, suffix) else {
        return None;
    };
    let code = _code(&format!("{}_{suffix}", name.to_uppercase()))?;
    _assemble(&raised(create_mem(code, r#where)), at, relocated)
}

/// `<name> [cell], source` -- the accumulate whose destination is memory.
pub fn arith_into(name: &str, cell: &ir::Mem, source: Register, at: u64) -> Option<Emitted> {
    let width = width_of(source);
    let built = operand_of(cell);
    let (Some(width), Some((r#where, relocated))) = (width, built) else {
        return None;
    };
    if !TWO_OPERAND.contains(&name) || i64::from(cell.width) != width {
        return None;
    }
    let code = _code(&format!("{}_RM{}_R{}", name.to_uppercase(), width * 8, width * 8))?;
    _assemble(&raised(create_mem_reg(code, r#where, source)), at, relocated)
}

/// `push wide / pop low / pop high` -- whichever three registers those are.
pub fn restore_of(wide: Register, low: Register, high: Register, at: u64) -> Option<Emitted> {
    if width_of(wide) != Some(4) || width_of(low) != Some(2) || width_of(high) != Some(2) {
        return None;
    }
    let (push, pop) = (_code("PUSH_R32")?, _code("POP_R16")?);
    let mut out = Vec::new();
    for (code, register) in [(push, wide), (pop, low), (pop, high)] {
        let made = _assemble(&raised(create_reg(code, register)), at + out.len() as u64, true)?;
        out.extend(made.code);
    }
    Some(Emitted::new(out))
}

/// calls.py's own restore idiom, built from the encoder above.
pub static RESTORE: LazyLock<IndexMap<i64, Vec<u8>>> = LazyLock::new(|| {
    [
        (0, restore_of(Register::EAX, Register::AX, Register::DX, 0)),
        (1, restore_of(Register::ECX, Register::CX, Register::BX, 0)),
    ]
    .into_iter()
    .filter_map(|(pair, made)| made.map(|made| (pair, made.code)))
    .collect()
});

/// One absorbable runtime call as the instructions that replace it.
///
/// `Ok(Err(reason))` is Python's `str` answer. calls.absorb and its
/// CallSite are BC-decoded and not ported, so this refuses.
pub fn absorbed<S>(_site: &S, _live: ir::Flag, _restore: bool) -> Result<Result<Emitted, String>, String> {
    Err("not yet ported: qbopt.legacy.calls.absorb".to_owned())
}

/// A divide emitted from the operation's own operands.
///
/// `Err(reason)` is Python's `str` answer. calls.assemble's ValueError,
/// which Python does not catch, panics.
pub fn divides(op: &mir::Op, seats: (Register, Register), restore: bool) -> Result<Emitted, String> {
    if op.kind != mir::Kind::Divmod || op.args.len() != 2 {
        return Err(format!("{}: not a divide over two operands", op.name));
    }

    let mut steps: Vec<Instruction> = Vec::new();
    let mut reads: Vec<usize> = Vec::new(); // which steps carry a relocatable field

    for (r#where, one) in [(machine::RESULT, &op.args[0]), (machine::DIVISOR, &op.args[1])] {
        // `where` given whatever this operand is, or why it cannot be.
        match one {
            mir::Arg::Const(constant) => {
                let n = i64::try_from(&constant.n).unwrap_or_else(|_| panic!("out of range integral type conversion attempted"));
                steps.push(raised(create_reg_i32(Code::Mov_r32_imm32, r#where, n)));
                continue;
            }
            mir::Arg::Cell(cell) if cell.r#ref.addr.is_some() => {
                // The accumulator's moffs form has no ModRM byte and is a
                // byte shorter.
                let base = cell.r#ref.addr.expect("checked").base;
                let code = if base == Register::None && r#where == machine::RESULT {
                    Code::Mov_EAX_moffs32
                } else {
                    Code::Mov_r32_rm32
                };
                reads.push(steps.len());
                steps.push(raised(create_reg_mem(code, r#where, machine::relocated_memory(base, Register::None))));
                continue;
            }
            _ => {}
        }
        // A value in a register needs the allocation to say which register.
        return Err(format!("{} is not an operand a divide can read yet", one.repr()));
    }

    steps.push(Instruction::with(Code::Cdq));
    steps.push(raised(create_reg(Code::Idiv_rm32, machine::DIVISOR)));
    // Two moves that happen at once: ordered where one is free, exchanged
    // where neither is.
    let (quotient, remainder) = seats;
    let mut moves: Vec<(Register, Register)> = [(quotient, machine::RESULT), (remainder, Register::EDX)]
        .into_iter()
        .filter(|(into, outof)| into != outof)
        .collect();
    if moves.len() == 2 && moves[0].0 == moves[1].1 && moves[1].0 == moves[0].1 {
        steps.push(raised(create_reg_reg(Code::Xchg_rm32_r32, moves[0].0, moves[0].1)));
    } else {
        // Whichever move nothing else reads out of, first.
        if moves.len() == 2 && moves[0].0 == moves[1].1 {
            moves.reverse();
        }
        for (into, outof) in moves {
            steps.push(raised(create_reg_reg(Code::Mov_r32_rm32, into, outof)));
        }
    }
    if restore {
        steps.extend(machine::restoring());
    }
    let relocated: Vec<(usize, usize)> = reads.iter().map(|&index| (index, index)).collect();
    let made = raised(machine::assemble(&mut steps, &relocated));
    Ok(Emitted { fields: made.relocations.iter().map(|&(r#where, _which)| r#where).collect(), ..Emitted::new(made.code) })
}

/// Which fixup each of an absorbed site's fields names, in the same order.
///
/// calls.absorb is not ported, so this refuses.
pub fn absorbed_fixups<S>(_site: &S, _live: ir::Flag, _restore: bool) -> Result<Vec<usize>, String> {
    Err("not yet ported: qbopt.legacy.calls.absorb".to_owned())
}

/// The idiom that puts a widened value's halves back where BC reads them.
pub fn restore(pair: i64) -> Option<Emitted> {
    RESTORE.get(&pair).map(|made| Emitted::new(made.clone()))
}

/// `idiv` or `div` by a register.
pub fn divide(name: &str, divisor: Register, at: u64) -> Option<Emitted> {
    if name != "idiv" && name != "div" {
        return None;
    }
    let width = width_of(divisor)?;
    let code = _code(&format!("{}_RM{}", name.to_uppercase(), width * 8))?;
    _assemble(&raised(create_reg(code, divisor)), at, true)
}

/// `lea into,[cell]` -- the address as a value, reading no memory.
pub fn address_of(into: Register, cell: &ir::Address, at: u64) -> Option<Emitted> {
    let width = width_of(into)?;
    let code = _code(&format!("LEA_R{}_M", width * 8))?;
    if cell.addr.is_some() && cell.index == Register::None {
        let (built, relocated) = operand_of(&ir::Mem::new(cell.addr, width as u32))?;
        return _assemble(&raised(create_reg_mem(code, into, built)), at, relocated);
    }
    if cell.through == Register::None && cell.index == Register::None {
        return None;
    }
    let size = if cell.disp_width != 0 { cell.disp_width } else { _displacement_size(cell.through, cell.offset) };
    let r#where = memory_operand(cell.through, cell.index, cell.scale, cell.offset, size, Register::None);
    // No address to name, so the displacement is arithmetic and not a symbol.
    _assemble(&raised(create_reg_mem(code, into, r#where)), at, false)
}

/// `cmp a,b` or `test a,b` -- both flags-only, and not the same question.
pub fn compare_registers(name: &str, one: Register, other: Register, at: u64) -> Option<Emitted> {
    if name == "cmp" {
        return arith("cmp", one, other, at);
    }
    if name != "test" {
        return None;
    }
    let width = width_of(one)?;
    if width_of(other) != Some(width) {
        return None;
    }
    let code = _code(&format!("TEST_RM{}_R{}", width * 8, width * 8))?;
    _assemble(&raised(create_reg_reg(code, one, other)), at, true)
}

/// `cmp dest,[cell]`. Flags are the whole result, so there is no dest.
pub fn compare_mem(dest: Register, cell: &ir::Mem, at: u64) -> Option<Emitted> {
    arith_mem("cmp", dest, cell, at)
}

/// The segment registers, which are not values and which an instruction
/// still names.
pub static SEGMENTS: LazyLock<IndexMap<Register, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([
        (Register::CS, "CS"),
        (Register::DS, "DS"),
        (Register::ES, "ES"),
        (Register::SS, "SS"),
        (Register::FS, "FS"),
        (Register::GS, "GS"),
    ])
});

/// `push cs` and its kind, at the width the push actually moves.
pub fn push_segment(one: Register, width: i64, at: u64) -> Option<Emitted> {
    let named = SEGMENTS.get(&one)?;
    let code = _code(&format!("PUSH{}_{named}", if width == 4 { "D" } else { "W" }))?;
    // Named even though the opcode implies it: iced wants the operand.
    _assemble(&raised(create_reg(code, one)), at, true)
}

/// `pop es` and its kind. BC saves es around a far-pointer access.
pub fn pop_segment(one: Register, width: i64, at: u64) -> Option<Emitted> {
    let named = SEGMENTS.get(&one)?;
    if one == Register::CS {
        return None; // popping cs is not an instruction on anything after the 8086
    }
    let code = _code(&format!("POP{}_{named}", if width == 4 { "D" } else { "W" }))?;
    _assemble(&raised(create_reg(code, one)), at, true)
}

/// The string stores BC emits to clear an array.
pub const STRING: [&str; 3] = ["stosb", "stosw", "stosd"];

/// `rep stosw` and its kind, at this pass's own 16-bit address size.
pub fn fill(name: &str, at: u64, repeated: bool) -> Option<Emitted> {
    let make = match name {
        "stosb" => Instruction::with_stosb,
        "stosw" => Instruction::with_stosw,
        "stosd" => Instruction::with_stosd,
        _ => return None,
    };
    let prefix = if repeated { RepPrefixKind::Repe } else { RepPrefixKind::None };
    _assemble(&raised(make(BITNESS, prefix).map_err(|error| error.to_string())), at, true)
}

/// The shifts and rotates.
pub const SHIFTS: [&str; 8] = ["shl", "shr", "sar", "rol", "ror", "rcl", "rcr", "sal"];

/// `shld`/`shrd dest,other,count` -- two registers shifted as one number.
pub fn funnel(name: &str, dest: Register, other: Register, count: Option<i64>, at: u64) -> Option<Emitted> {
    if !["shld", "shrd"].contains(&name) || width_of(dest) != Some(4) || width_of(other) != Some(4) {
        return None;
    }
    let opcode = name.to_uppercase();
    let Some(count) = count else {
        let code = _code(&format!("{opcode}_RM32_R32_CL"))?;
        return _assemble(&raised(create_reg_reg_reg(code, dest, other, Register::CL)), at, true);
    };
    let code = _code(&format!("{opcode}_RM32_R32_IMM8"))?;
    _assemble(&raised(create_reg_reg_i32(code, dest, other, count)), at, true)
}

/// Shift a register or spill cell. `count` of None means by cl.
pub fn shift(name: &str, dest: RegisterOrCell<'_>, count: Option<i64>, at: u64) -> Option<Emitted> {
    if !SHIFTS.contains(&name) {
        return None;
    }
    let built = match dest {
        RegisterOrCell::Mem(cell) => Some(operand_of(cell)?),
        RegisterOrCell::Reg(_) => None,
    };
    let width = match dest {
        RegisterOrCell::Mem(cell) => i64::from(cell.width),
        RegisterOrCell::Reg(register) => width_of(register)?,
    };
    let relocated = built.is_some_and(|(_, relocated)| relocated);
    let upper = name.to_uppercase();
    let Some(count) = count else {
        let code = _code(&format!("{upper}_RM{}_CL", width * 8))?;
        let instruction = match (dest, built) {
            (RegisterOrCell::Mem(_), Some((r#where, _))) => raised(create_mem_reg(code, r#where, Register::CL)),
            (RegisterOrCell::Reg(register), _) => raised(create_reg_reg(code, register, Register::CL)),
            _ => unreachable!("a cell has its operand"),
        };
        return _assemble(&instruction, at, relocated);
    };
    // `shl reg,1` has its own opcode, and iced models the implicit 1 as a
    // real operand, so it is built with the count.
    for shape in [format!("{upper}_RM{}_1", width * 8), format!("{upper}_RM{}_IMM8", width * 8)] {
        if shape.ends_with("_1") && count != 1 {
            continue;
        }
        let Some(code) = _code(&shape) else {
            continue;
        };
        let instruction = match (dest, built) {
            (RegisterOrCell::Mem(_), Some((r#where, _))) => raised(create_mem_i32(code, r#where, count)),
            (RegisterOrCell::Reg(register), _) => raised(create_reg_i32(code, register, count)),
            _ => unreachable!("a cell has its operand"),
        };
        if let Some(made) = _assemble(&instruction, at, relocated) {
            return Some(made);
        }
    }
    None
}

/// The popping x87 arithmetic: `faddp st(1),st(0)` and its kind.
pub const FLOAT_POP: [&str; 6] = ["faddp", "fsubp", "fmulp", "fdivp", "fsubrp", "fdivrp"];
pub const STACK_REGISTERS: [Register; 8] = [
    Register::ST0,
    Register::ST1,
    Register::ST2,
    Register::ST3,
    Register::ST4,
    Register::ST5,
    Register::ST6,
    Register::ST7,
];

/// Python's `STACK_REGISTERS[index]`, which raises past the end.
fn stack_register(index: u32) -> Register {
    *STACK_REGISTERS.get(index as usize).unwrap_or_else(|| panic!("tuple index out of range"))
}

/// `faddp st(i),st(0)` -- the arithmetic that pops its own operand.
pub fn float_pop(name: &str, index: u32, at: u64) -> Option<Emitted> {
    if !FLOAT_POP.contains(&name) || (index as usize) >= STACK_REGISTERS.len() {
        return None;
    }
    let code = _code(&format!("{}_STI_ST0", name.to_uppercase()))?;
    _assemble(&raised(create_reg_reg(code, stack_register(index), Register::ST0)), at, true)
}

fn st_index(one: &Loc) -> Option<u32> {
    match one {
        Loc::St(st) => Some(st.index),
        _ => None,
    }
}

/// Select explicit stack operands without changing their evaluation order.
pub fn float_stack(what: &Semantics, at: u64) -> Option<Emitted> {
    if what.dests.iter().chain(&what.sources).filter_map(st_index).any(|index| index as usize >= STACK_REGISTERS.len()) {
        return None;
    }
    let (dests, sources) = (&what.dests, &what.sources);
    let name = what.name.as_deref();
    // FLOAT_LOAD, "fld", (St(0),), (St(index),)
    let fld = (what.op == Operation::FloatLoad
        && name == Some("fld")
        && dests.len() == 1
        && st_index(&dests[0]) == Some(0)
        && sources.len() == 1)
        .then(|| st_index(&sources[0]))
        .flatten();
    if let Some(index) = fld {
        return _assemble(&raised(create_reg(Code::Fld_sti, stack_register(index))), at, true);
    }
    // EXCHANGE, "fxch", (St(0), St(index)), sources == dests
    let fxch = (what.op == Operation::Exchange
        && name == Some("fxch")
        && dests.len() == 2
        && st_index(&dests[0]) == Some(0)
        && sources == dests)
        .then(|| st_index(&dests[1]))
        .flatten();
    if let Some(index) = fxch {
        return _assemble(&raised(create_reg_reg(Code::Fxch_st0_sti, Register::ST0, stack_register(index))), at, true);
    }
    // FLOAT_ARITH, name, (St(dest),), (left, St(source)) if left == dests[0]
    let arith = (what.op == Operation::FloatArith && dests.len() == 1 && sources.len() == 2 && sources[0] == dests[0])
        .then(|| st_index(&dests[0]).zip(st_index(&sources[1])))
        .flatten();
    if let Some((dest, source)) = arith {
        let name = name.unwrap_or("None");
        if !["fadd", "fsub", "fsubr", "fmul", "fdiv", "fdivr"].contains(&name) || (dest != 0 && source != 0) {
            return None;
        }
        let form = if dest == 0 { "ST0_STI" } else { "STI_ST0" };
        let code = _code(&format!("{}_{form}", name.to_uppercase()))?;
        return _assemble(&raised(create_reg_reg(code, stack_register(dest), stack_register(source))), at, true);
    }
    None
}

/// `idiv [x]` -- the divisor in memory rather than a register.
pub fn divide_mem(name: &str, cell: &ir::Mem, at: u64) -> Option<Emitted> {
    if name != "idiv" && name != "div" {
        return None;
    }
    let built = operand_of(cell);
    if built.is_none() || ![2, 4].contains(&cell.width) {
        return None;
    }
    let (r#where, relocated) = built?;
    let code = _code(&format!("{}_RM{}", name.to_uppercase(), cell.width * 8))?;
    _assemble(&raised(create_mem(code, r#where)), at, relocated)
}

/// The one-operand `imul`/`mul`, whose result is dx:ax and is not encoded.
pub fn multiply(name: &str, source: RegisterOrCell<'_>, at: u64) -> Option<Emitted> {
    if name != "imul" && name != "mul" {
        return None;
    }
    let upper = name.to_uppercase();
    match source {
        RegisterOrCell::Mem(cell) => {
            let built = operand_of(cell);
            if built.is_none() || ![2, 4].contains(&cell.width) {
                return None;
            }
            let (r#where, relocated) = built?;
            let code = _code(&format!("{upper}_RM{}", cell.width * 8))?;
            _assemble(&raised(create_mem(code, r#where)), at, relocated)
        }
        RegisterOrCell::Reg(register) => {
            let width = width_of(register)?;
            let code = _code(&format!("{upper}_RM{}", width * 8))?;
            _assemble(&raised(create_reg(code, register)), at, true)
        }
    }
}

/// `imul eax,ecx` and `imul ax,[x],3` -- the forms that name their result.
pub fn multiply_into(dest: Register, source: RegisterOrCell<'_>, value: Option<i64>, at: u64) -> Option<Emitted> {
    let width = width_of(dest)?;
    let bits = width * 8;
    let source = match source {
        RegisterOrCell::Mem(cell) => {
            let built = operand_of(cell);
            if built.is_none() || i64::from(cell.width) != width {
                return None;
            }
            let (r#where, relocated) = built?;
            let Some(value) = value else {
                let code = _code(&format!("IMUL_R{bits}_RM{bits}"))?;
                return _assemble(&raised(create_reg_mem(code, dest, r#where)), at, relocated);
            };
            let immediate = if fits_in_a_byte(value) { 8 } else { bits };
            let code = _code(&format!("IMUL_R{bits}_RM{bits}_IMM{immediate}"))?;
            return _assemble(&raised(create_reg_mem_i32(code, dest, r#where, value)), at, relocated);
        }
        RegisterOrCell::Reg(register) => register,
    };
    if width_of(source) != Some(width) {
        return None;
    }
    let Some(value) = value else {
        let code = _code(&format!("IMUL_R{bits}_RM{bits}"))?;
        return _assemble(&raised(create_reg_reg(code, dest, source)), at, true);
    };
    let immediate = if fits_in_a_byte(value) { 8 } else { bits };
    let code = _code(&format!("IMUL_R{bits}_RM{bits}_IMM{immediate}"))?;
    _assemble(&raised(create_reg_reg_i32(code, dest, source, value)), at, true)
}

/// Each segment register a far pointer can be loaded into with its offset.
pub static FAR_LOADS: LazyLock<IndexMap<Register, (&'static str, &'static str)>> = LazyLock::new(|| {
    IndexMap::from_iter([
        (Register::ES, ("les", "LES_R16_M1616")),
        (Register::FS, ("lfs", "LFS_R16_M1616")),
        (Register::GS, ("lgs", "LGS_R16_M1616")),
    ])
});

/// `les bx,[cell]`: a far pointer's offset word into `into` and its segment
/// word into `segment`.
pub fn far_load(name: &str, into: Register, segment: Register, cell: &ir::Mem, at: u64) -> Option<Emitted> {
    let (spelled, code_name) = FAR_LOADS.get(&segment).copied().unwrap_or(("", ""));
    let built = operand_of(cell);
    if name != spelled || built.is_none() || cell.width != 4 || width_of(into) != Some(2) || SEGMENTS.contains_key(&into)
    {
        return None;
    }
    let code = _code(code_name)?;
    let (r#where, relocated) = built?;
    _assemble(&raised(create_reg_mem(code, into, r#where)), at, relocated)
}

/// `mov es,[si+2]` and `mov [x],es` -- how a far pointer is loaded.
pub fn move_segment(into: Register, outof: RegisterOrCell<'_>, at: u64) -> Option<Emitted> {
    let both = match outof {
        RegisterOrCell::Reg(outof) if SEGMENTS.contains_key(&into) && SEGMENTS.contains_key(&outof) => Some(outof),
        _ => None,
    };
    if let Some(outof) = both {
        // No `mov sreg,sreg`: through the stack, which writes no flags.
        let pushed = push_segment(outof, 2, at);
        let popped = match &pushed {
            None => None,
            Some(pushed) => pop_segment(into, 2, at + pushed.code.len() as u64),
        };
        let (pushed, popped) = (pushed?, popped?);
        return Some(Emitted::new([pushed.code, popped.code].concat()));
    }
    if SEGMENTS.contains_key(&into) {
        let code = _code("MOV_SREG_RM16")?;
        return match outof {
            RegisterOrCell::Mem(cell) => {
                let (built, relocated) = operand_of(cell)?;
                _assemble(&raised(create_reg_mem(code, into, built)), at, relocated)
            }
            RegisterOrCell::Reg(outof) => _assemble(&raised(create_reg_reg(code, into, outof)), at, true),
        };
    }
    let code = _code("MOV_RM16_SREG")?;
    let RegisterOrCell::Reg(outof) = outof else {
        return None;
    };
    if !SEGMENTS.contains_key(&outof) {
        return None;
    }
    _assemble(&raised(create_reg_reg(code, into, outof)), at, true)
}

/// `pop cs` is 8086-only, so cs is not here.
pub static _POP_SEGMENT: LazyLock<IndexMap<Register, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([
        (Register::DS, "POPW_DS"),
        (Register::ES, "POPW_ES"),
        (Register::SS, "POPW_SS"),
        (Register::FS, "POPW_FS"),
        (Register::GS, "POPW_GS"),
    ])
});

/// `push 0A000h / pop es` -- a constant into a segment register.
pub fn load_segment(into: Register, value: i64, at: u64) -> Option<Emitted> {
    let named = _POP_SEGMENT.get(&into);
    let push = _code("PUSH_IMM16");
    let (Some(named), Some(push)) = (named, push) else {
        return None;
    };
    let pop = _code(named)?;
    let mut out = Vec::new();
    for made in [raised(create_u32(push, value & 0xFFFF)), raised(create_reg(pop, into))] {
        let got = _assemble(&made, at + out.len() as u64, true)?;
        out.extend(got.code);
    }
    Some(Emitted::new(out))
}

/// `mov [bx+2],ds` -- half a far pointer written out.
pub fn store_segment(cell: &ir::Mem, outof: Register, at: u64) -> Option<Emitted> {
    if !SEGMENTS.contains_key(&outof) {
        return None;
    }
    let built = operand_of(cell);
    let code = _code("MOV_RM16_SREG");
    let (Some((r#where, relocated)), Some(code)) = (built, code) else {
        return None;
    };
    _assemble(&raised(create_mem_reg(code, r#where, outof)), at, relocated)
}

/// `add word ptr [bp-16h],4` -- accumulate into memory.
pub fn arith_into_imm(name: &str, cell: &ir::Mem, value: i64, at: u64, relocated: bool) -> Option<Emitted> {
    let built = operand_of(cell);
    if !TWO_OPERAND.contains(&name) || built.is_none() || ![1, 2, 4].contains(&cell.width) {
        return None;
    }
    let (r#where, symbolic) = built?;
    let width = i64::from(cell.width);
    let mut value = _immediate(value, width);
    if width == 1 {
        value = ((value & 0xFF) ^ 0x80) - 0x80;
    }
    let all: Vec<i64> = if fits_in_a_byte(value) && !relocated { vec![8, width * 8] } else { vec![width * 8] };
    for bits in all {
        let Some(code) = _code(&format!("{}_RM{}_IMM{bits}", name.to_uppercase(), width * 8)) else {
            continue;
        };
        match create_mem_i32(code, r#where, value) {
            Ok(made) => return _assemble(&made, at, symbolic),
            Err(_) => continue,
        }
    }
    None
}

/// `xchg cx,ax`, which has a one-byte form against the accumulator.
pub fn exchange(one: Register, other: Register, at: u64) -> Option<Emitted> {
    let width = width_of(one)?;
    if width_of(other) != Some(width) {
        return None;
    }
    let accumulator = if width == 2 { "AX" } else { "EAX" };
    for shape in [format!("XCHG_R{}_{accumulator}", width * 8), format!("XCHG_RM{}_R{}", width * 8, width * 8)] {
        let Some(code) = _code(&shape) else {
            continue;
        };
        if let Some(made) = _assemble(&raised(create_reg_reg(code, one, other)), at, true) {
            return Some(made);
        }
    }
    None
}

/// Exchange a register with a same-width memory cell.
pub fn exchange_mem(register: Register, cell: &ir::Mem, at: u64) -> Option<Emitted> {
    let width = width_of(register);
    let built = operand_of(cell);
    let Some(width) = width.filter(|width| [1, 2, 4].contains(width)) else {
        return None;
    };
    let Some((r#where, relocated)) = built.filter(|_| i64::from(cell.width) == width) else {
        return None;
    };
    let code = _code(&format!("XCHG_RM{}_R{}", width * 8, width * 8))?;
    _assemble(&raised(create_mem_reg(code, r#where, register)), at, relocated)
}

/// `neg`, `not`, `inc` or `dec` of a memory cell.
pub fn unary_mem(name: &str, cell: &ir::Mem, at: u64) -> Option<Emitted> {
    if !ONE_OPERAND.contains(&name) {
        return None;
    }
    let built = operand_of(cell);
    if built.is_none() || ![2, 4].contains(&cell.width) {
        return None;
    }
    let (r#where, relocated) = built?;
    let code = _code(&format!("{}_RM{}", name.to_uppercase(), cell.width * 8))?;
    _assemble(&raised(create_mem(code, r#where)), at, relocated)
}

fn reg_of(one: &Loc) -> Option<Register> {
    match one {
        Loc::Reg(reg) => Some(reg.register),
        _ => None,
    }
}

fn mem_of(one: &Loc) -> Option<&ir::Mem> {
    match one {
        Loc::Mem(cell) => Some(cell),
        _ => None,
    }
}

fn imm_of(one: &Loc) -> Option<i64> {
    match one {
        Loc::Imm(imm) => Some(imm.value),
        _ => None,
    }
}

/// One MIR operation as machine bytes, or None where this cannot say it.
pub fn emit(
    what: &Semantics,
    at: u64,
    r#where: Option<Where<'_>>,
    short: bool,
    relocated: bool,
    held: Option<&HeldMap>,
) -> Option<Emitted> {
    // Once, over every operand, rather than at each register site below.
    let truthy = match r#where {
        None => false,
        Some(Where::One(map)) => !map.is_empty(),
        Some(Where::Pair(..)) => true,
    };
    let remapped;
    let what = if truthy || held.is_some_and(|held| !held.is_empty()) {
        // By side: one map cannot say two things about eax.
        let (into, outof) = match r#where {
            None => (None, None),
            Some(Where::One(map)) => (Some(map), Some(map)),
            Some(Where::Pair(into, outof)) => (Some(into), Some(outof)),
        };
        remapped = Semantics {
            dests: what.dests.iter().map(|one| _operand(one, into, held)).collect(),
            sources: what.sources.iter().map(|one| _operand(one, outof, held)).collect(),
            ..what.clone()
        };
        &remapped
    } else {
        what
    };

    let all = || what.dests.iter().chain(&what.sources);
    // A Held that reached here is one the allocation had no register for.
    if all().any(|one| matches!(one, Loc::Held(_))) {
        return None;
    }
    // A cell whose address value nothing placed.
    if all().any(|one| matches!(one, Loc::Mem(cell) if cell.base.is_some() && cell.through == Register::None)) {
        return None;
    }

    let (dests, sources) = (&what.dests, &what.sources);
    let name = what.name.as_deref().unwrap_or("");
    if matches!(what.op, Operation::FloatLoad | Operation::FloatArith | Operation::Exchange)
        && !sources.is_empty()
        && all().all(|operand| matches!(operand, Loc::St(_)))
    {
        return float_stack(what, at);
    }
    let op = what.op;
    let seg = |register: Register| SEGMENTS.contains_key(&register);

    if op == Operation::Extend
        && matches!(what.name.as_deref(), Some("movsx" | "movzx"))
        && dests.len() == 1
        && sources.len() == 1
    {
        let upper = name.to_uppercase();
        let wide = |register: Register| width_of(register).unwrap_or(0);
        match (&dests[0], &sources[0]) {
            (Loc::Reg(into), Loc::Reg(outof)) => {
                let code = _code(&format!("{upper}_R{}_RM{}", wide(into.register) * 8, wide(outof.register) * 8));
                if let Some(code) = code {
                    if target::WIDTHS[&into.register] > target::WIDTHS[&outof.register] {
                        return _assemble(&raised(create_reg_reg(code, into.register, outof.register)), at, true);
                    }
                }
            }
            (Loc::Reg(into), Loc::Mem(cell)) => {
                let code = _code(&format!("{upper}_R{}_RM{}", wide(into.register) * 8, cell.width * 8));
                let built = operand_of(cell);
                if let (Some(code), Some((r#where, relocated))) = (code, built) {
                    if target::WIDTHS[&into.register] > i64::from(cell.width) {
                        return _assemble(&raised(create_reg_mem(code, into.register, r#where)), at, relocated);
                    }
                }
            }
            _ => {}
        }
        return None;
    }
    if op == Operation::Move && dests.len() == 2 && sources.len() == 1 {
        if let (Some(into), Some(segment), Some(cell)) = (reg_of(&dests[0]), reg_of(&dests[1]), mem_of(&sources[0])) {
            return far_load(name, into, segment, cell, at);
        }
        return None;
    }
    if op == Operation::Move && dests.len() == 1 && sources.len() == 1 {
        return match (&dests[0], &sources[0]) {
            // Before the plain register move, which cannot say a segment
            // register and would refuse `mov ax,es`.
            (Loc::Reg(into), Loc::Mem(cell)) if seg(into.register) => {
                move_segment(into.register, RegisterOrCell::Mem(cell), at)
            }
            (Loc::Mem(cell), Loc::Reg(outof)) if seg(outof.register) => store_segment(cell, outof.register, at),
            (Loc::Reg(into), Loc::Reg(outof)) if seg(into.register) || seg(outof.register) => {
                move_segment(into.register, RegisterOrCell::Reg(outof.register), at)
            }
            // Before the plain immediate load, which would build the
            // `mov sreg,imm` the machine has no encoding for.
            (Loc::Reg(into), Loc::Imm(imm)) if seg(into.register) => load_segment(into.register, imm.value, at),
            (Loc::Reg(into), Loc::Reg(outof)) => r#move(into.register, outof.register, at),
            (Loc::Reg(into), Loc::Imm(imm)) => load(into.register, imm.value, at),
            (Loc::Reg(into), Loc::Mem(cell)) => move_from(into.register, cell, at),
            (Loc::Mem(cell), Loc::Reg(outof)) => move_into(cell, outof.register, at),
            (Loc::Mem(cell), Loc::Imm(imm)) => store_imm(cell, imm.value, at),
            _ => None,
        };
    }
    if op == Operation::Restore && dests.len() == 2 && sources.len() == 1 {
        if let (Some(wide), Some(low), Some(high)) = (reg_of(&sources[0]), reg_of(&dests[0]), reg_of(&dests[1])) {
            return restore_of(wide, low, high, at);
        }
        return None;
    }
    if op == Operation::Funnel && dests.len() == 1 && sources.len() == 3 {
        if let (Some(into), Some(other)) = (reg_of(&dests[0]), reg_of(&sources[1])) {
            if let Some(count) = imm_of(&sources[2]) {
                return funnel(name, into, other, Some(count), at);
            }
            if reg_of(&sources[2]) == Some(Register::CL) {
                return funnel(name, into, other, None, at);
            }
        }
        return None;
    }
    if op == Operation::Binary && SHIFTS.contains(&name) && dests.len() == 1 && sources.len() == 2 {
        let count = imm_of(&sources[1]);
        let by_cl = reg_of(&sources[1]) == Some(Register::CL);
        if let Some(into) = reg_of(&dests[0]) {
            if count.is_some() {
                return shift(name, RegisterOrCell::Reg(into), count, at);
            }
            if by_cl {
                return shift(name, RegisterOrCell::Reg(into), None, at);
            }
        }
        if let Some(cell) = mem_of(&dests[0]) {
            if count.is_some() {
                return shift(name, RegisterOrCell::Mem(cell), count, at);
            }
            if by_cl {
                return shift(name, RegisterOrCell::Mem(cell), None, at);
            }
        }
        return None;
    }
    if op == Operation::Binary && dests.len() == 1 && sources.len() == 2 {
        // BINARY's own rule: sources[0] IS dests[0].
        let (dest, source) = (&dests[0], &sources[1]);
        if let (Some(into), Some(outof)) = (reg_of(dest), reg_of(source)) {
            return arith(name, into, outof, at);
        }
        if let (Some(into), Some(value)) = (reg_of(dest), imm_of(source)) {
            return arith_imm(name, into, value, at, relocated);
        }
        if let (Some(into), Some(cell)) = (reg_of(dest), mem_of(source)) {
            return arith_mem(name, into, cell, at);
        }
        if let (Some(cell), Some(outof)) = (mem_of(dest), reg_of(source)) {
            return arith_into(name, cell, outof, at);
        }
        if let (Some(cell), Some(value)) = (mem_of(dest), imm_of(source)) {
            return arith_into_imm(name, cell, value, at, relocated);
        }
        return None;
    }
    if op == Operation::Unary && dests.len() == 1 && sources.len() == 1 {
        return match &dests[0] {
            Loc::Reg(into) => unary(name, into.register, at),
            Loc::Mem(cell) => unary_mem(name, cell, at),
            _ => None,
        };
    }
    if op == Operation::Push && sources.len() == 1 {
        return match &sources[0] {
            Loc::Reg(one) if seg(one.register) => push_segment(one.register, i64::from(one.width), at),
            Loc::Reg(one) => push(one.register, at),
            Loc::Imm(imm) => push_imm(imm.value, i64::from(imm.width), at, relocated),
            Loc::Mem(cell) => push_mem(cell, at),
            _ => None,
        };
    }
    if let (Operation::Branch, Some(target)) = (op, what.target) {
        return branch(name, target, at, short);
    }
    if let (Operation::Jump, Some(target)) = (op, what.target) {
        return jump(target, at, short);
    }
    if op == Operation::Call && what.indirect && sources.len() == 1 {
        match &sources[0] {
            Loc::Reg(one) if one.width == 2 => {
                return _assemble(&raised(create_reg(Code::Call_rm16, one.register)), at, true);
            }
            Loc::Mem(cell) if cell.width == 2 => {
                if let Some((built, relocated)) = operand_of(cell) {
                    return _assemble(&raised(create_mem(Code::Call_rm16, built)), at, relocated);
                }
            }
            Loc::Mem(cell) if cell.width == 4 => {
                if let Some((built, relocated)) = operand_of(cell) {
                    return _assemble(&raised(create_mem(Code::Call_m1616, built)), at, relocated);
                }
            }
            _ => {}
        }
        return None;
    }
    if op == Operation::Call {
        return match what.target {
            None => call_far(at),
            Some(target) => call_near(target, at),
        };
    }
    if op == Operation::Escape && what.target.is_none() {
        return jump_far(at);
    }
    if op == Operation::FloatLoad
        && matches!(what.name.as_deref(), Some("fldz" | "fld1"))
        && sources.is_empty()
        && dests.len() == 1
        && st_index(&dests[0]) == Some(0)
    {
        let code = if name == "fldz" { Code::Fldz } else { Code::Fld1 };
        return _assemble(&Instruction::with(code), at, true);
    }
    if matches!(op, Operation::FloatLoad | Operation::FloatArith) && !sources.is_empty() {
        if let Some(cell) = mem_of(&sources[sources.len() - 1]) {
            return float_memory(name, cell, at);
        }
        return None;
    }
    if op == Operation::FloatStore && dests.len() == 1 {
        match &dests[0] {
            Loc::Mem(cell) => return float_memory(name, cell, at),
            Loc::St(st) if what.name.as_deref() == Some("fstp") && sources.len() == 1 && st_index(&sources[0]) == Some(0) => {
                return _assemble(&raised(create_reg(Code::Fstp_sti, stack_register(st.index))), at, true);
            }
            _ => {}
        }
        return None;
    }
    if op == Operation::Exchange && dests.len() == 2 {
        if let (Some(one), Some(other)) = (reg_of(&dests[0]), reg_of(&dests[1])) {
            return exchange(one, other, at);
        }
        if let (Some(one), Some(cell)) = (reg_of(&dests[0]), mem_of(&dests[1])) {
            return exchange_mem(one, cell, at);
        }
        if let (Some(cell), Some(one)) = (mem_of(&dests[0]), reg_of(&dests[1])) {
            return exchange_mem(one, cell, at);
        }
        return None;
    }
    if op == Operation::Compare && name.starts_with('f') {
        // st(0) is implicit; what is encoded is the memory operand, if any.
        let memory: Vec<&ir::Mem> = sources.iter().filter_map(mem_of).collect();
        return if memory.is_empty() { bare(name, at) } else { float_memory(name, memory[0], at) };
    }
    if op == Operation::Barrier
        && what.name.as_deref() == Some("fnstsw")
        && dests.as_slice() == [Loc::Reg(ir::Reg { register: Register::AX, width: 2 })]
    {
        return _assemble(&raised(create_reg(Code::Fnstsw_AX, Register::AX)), at, true);
    }
    let control = what.name.as_deref().and_then(|name| CONTROL_WORD.get(name));
    if let (Operation::Barrier, Some(control), 1) = (op, control, dests.len() + sources.len()) {
        if let Some(cell) = all().next().and_then(mem_of).filter(|cell| cell.width == 2) {
            if let (Some((built, relocated)), Some(code)) = (operand_of(cell), _code(control)) {
                return _assemble(&raised(create_mem(code, built)), at, relocated);
            }
        }
        return None;
    }
    if op == Operation::Compare && sources.len() == 2 {
        let named = what.name.as_deref().unwrap_or("cmp");
        return match (&sources[0], &sources[1]) {
            (_, Loc::Imm(imm)) if what.name.as_deref() == Some("test") => test_immediate(&sources[0], imm.value, at),
            (_, Loc::Imm(imm)) if named == "cmp" => compare(&sources[0], imm.value, at, relocated),
            (Loc::Reg(into), Loc::Mem(cell)) if named == "cmp" => compare_mem(into.register, cell, at),
            (Loc::Reg(into), Loc::Reg(outof)) => compare_registers(named, into.register, outof.register, at),
            (Loc::Mem(cell), Loc::Reg(outof)) => {
                // Only `cmp` here: `test` against memory has its own shapes
                // and BC writes none of them.
                if named != "cmp" {
                    return None;
                }
                arith_into("cmp", cell, outof.register, at)
            }
            _ => None,
        };
    }
    if op == Operation::Multiply && dests.len() == 1 && sources.len() >= 2 {
        // One destination is the naming form: `imul eax,ecx`, and with a
        // third source `imul ax,[x],3`.
        let count = if sources.len() > 2 { imm_of(&sources[2]) } else { None };
        if let Some(into) = reg_of(&dests[0]) {
            if let Some(one) = reg_of(&sources[1]) {
                return multiply_into(into, RegisterOrCell::Reg(one), count, at);
            }
            if let Some(cell) = mem_of(&sources[1]) {
                return multiply_into(into, RegisterOrCell::Mem(cell), count, at);
            }
            if let Some(only) = imm_of(&sources[1]) {
                // The product of the first source: `imul cx,[bp-0Eh],2` is
                // not `imul cx,2`.
                if let Some(one) = reg_of(&sources[0]) {
                    return multiply_into(into, RegisterOrCell::Reg(one), Some(only), at);
                }
                if let Some(cell) = mem_of(&sources[0]) {
                    return multiply_into(into, RegisterOrCell::Mem(cell), Some(only), at);
                }
            }
        }
        return None;
    }
    if op == Operation::Multiply && dests.len() == 2 && !sources.is_empty() {
        // Two destinations means the widening form: dx:ax, neither encoded.
        return match &sources[sources.len() - 1] {
            Loc::Reg(one) => multiply(name, RegisterOrCell::Reg(one.register), at),
            Loc::Mem(cell) => multiply(name, RegisterOrCell::Mem(cell), at),
            _ => None,
        };
    }
    if op == Operation::FloatArithPop && !dests.is_empty() {
        return match &dests[0] {
            Loc::St(st) => float_pop(name, st.index, at),
            _ => None,
        };
    }
    if op == Operation::Divide && !sources.is_empty() {
        return match &sources[sources.len() - 1] {
            Loc::Reg(one) => divide(name, one.register, at),
            Loc::Mem(cell) => divide_mem(name, cell, at),
            _ => None,
        };
    }
    if op == Operation::Address && dests.len() == 1 && sources.len() == 1 {
        if let (Some(into), Loc::Address(cell)) = (reg_of(&dests[0]), &sources[0]) {
            return address_of(into, cell, at);
        }
        return None;
    }
    if op == Operation::Fill && matches!(sources.len(), 3 | 4) {
        return fill(name, at, sources.len() == 4);
    }
    if op == Operation::Nothing && name.is_empty() {
        return Some(Emitted::new(Vec::new()));
    }
    if matches!(op, Operation::Extend | Operation::Nothing | Operation::Leave) {
        return bare(name, at);
    }
    if matches!(op, Operation::FloatUnary | Operation::FloatArith) && !all().any(|one| matches!(one, Loc::Mem(_))) {
        // `fsqrt` and its kind name st(0) in both dests and sources, and
        // encode neither.
        return bare(name, at);
    }
    if op == Operation::Pop && dests.len() == 1 {
        return match &dests[0] {
            Loc::Reg(into) if seg(into.register) => pop_segment(into.register, i64::from(into.width), at),
            Loc::Reg(into) => pop(into.register, at),
            Loc::Mem(cell) => {
                let built = operand_of(cell);
                let code =
                    if [2, 4].contains(&cell.width) { _code(&format!("POP_RM{}", cell.width * 8)) } else { None };
                match (built, code) {
                    (Some((built, relocated)), Some(code)) => _assemble(&raised(create_mem(code, built)), at, relocated),
                    _ => None,
                }
            }
            _ => None,
        };
    }
    if op == Operation::Return {
        if sources.is_empty() {
            return if what.name.as_deref() == Some("ret") { bare("ret", at) } else { ret_far(0, at) };
        }
        return match &sources[0] {
            Loc::Imm(imm) => ret_far(imm.value, at),
            _ => None,
        };
    }
    None
}

#[cfg(test)]
mod sweep_support {
    pub use super::*;
    pub use crate::model::ir::{self, Addr, Loc, Space};
    pub use iced_x86::Register as R;

    pub use crate::model::ir::Operation as Op;

    pub fn a(space: Space, disp: i64, index: i64, base: R, segment: R) -> Addr {
        Addr { space, disp, index, base, segment }
    }

    pub fn h(value: u32, width: u32) -> ir::Held {
        ir::Held { value, width }
    }

    pub fn hd(value: u32, width: u32) -> Loc {
        Loc::Held(h(value, width))
    }

    pub fn rg(register: R, width: u32) -> Loc {
        Loc::Reg(ir::Reg { register, width })
    }

    pub fn im(value: i64, width: u32, address: Option<Addr>) -> Loc {
        Loc::Imm(ir::Imm { value, width, address })
    }

    pub fn st(index: u32) -> Loc {
        Loc::St(ir::St { index })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn m(
        addr: Option<Addr>,
        width: u32,
        through: R,
        offset: i64,
        disp_width: u32,
        base: Option<ir::Held>,
        index: Option<ir::Held>,
        scale: i64,
        index_through: R,
    ) -> Loc {
        Loc::Mem(ir::Mem { through, offset, disp_width, base, index, scale, index_through, ..ir::Mem::new(addr, width) })
    }

    pub fn ad(addr: Option<Addr>, through: R, index: R, scale: i64, offset: i64, disp_width: u32) -> Loc {
        Loc::Address(ir::Address { addr, through, index, scale, offset, disp_width })
    }

    pub fn sem(
        op: Op,
        name: Option<&str>,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        target: Option<i64>,
        indirect: bool,
    ) -> Semantics {
        Semantics { op, name: name.map(str::to_owned), dests, sources, target, indirect }
    }

    type Want = Option<(&'static str, Option<usize>, Option<usize>, Vec<usize>, bool)>;

    fn emitted(
        what: &Semantics,
        at: u64,
        short: bool,
        relocated: bool,
        r#where: Option<Vec<(R, R)>>,
        held: Option<Vec<(u32, R)>>,
    ) -> Option<Emitted> {
        let r#where: Option<RegisterMap> = r#where.map(|pairs| pairs.into_iter().collect());
        let held: Option<HeldMap> = held.map(|pairs| pairs.into_iter().collect());
        emit(what, at, r#where.as_ref().map(Where::One), short, relocated, held.as_ref())
    }

    fn hex(code: &[u8]) -> String {
        code.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn check(
        what: Semantics,
        at: u64,
        short: bool,
        relocated: bool,
        r#where: Option<Vec<(R, R)>>,
        held: Option<Vec<(u32, R)>>,
        want: Want,
    ) {
        let got = emitted(&what, at, short, relocated, r#where, held)
            .map(|made| (hex(&made.code), made.displacement_at, made.immediate_at, made.fields, made.symbolic));
        let want = want.map(|(code, displacement, immediate, fields, symbolic)| {
            (code.to_owned(), displacement, immediate, fields, symbolic)
        });
        assert_eq!(got, want, "{what:?} at={at} short={short} relocated={relocated}");
    }

    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn raises(
        what: Semantics,
        at: u64,
        short: bool,
        relocated: bool,
        r#where: Option<Vec<(R, R)>>,
        held: Option<Vec<(u32, R)>>,
        message: &str,
    ) {
        let raised = std::panic::catch_unwind(|| emitted(&what, at, short, relocated, r#where, held));
        let error = raised.expect_err("Python raised here");
        let text = error.downcast_ref::<String>().cloned().unwrap_or_default();
        assert_eq!(text, message, "{what:?}");
    }
}

#[cfg(test)]
#[path = "select_sweep.rs"]
mod sweep;

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
