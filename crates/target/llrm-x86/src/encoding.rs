//! Bytes of x86 machine code per instruction form, written once for every
//! consumer that prices code before registers exist (the selector, the
//! inliner's size view, loop strength reduction's size model).
//!
//! The first group (`register_bytes` ...) is the selector's, by operand width
//! against the target's default size. The rest is x86-64's, by [`Width`] and
//! [`Mem`].
//!
//! A form's length is prefix + opcode + ModRM + SIB + displacement + immediate
//! (Intel SDM vol. 2, 2.1). REX.W is counted from the operand width; REX.R/X/B
//! (registers r8-r15, `sil`/`dil`) are unknown before allocation, so each form
//! takes `rex_extra` for a pessimistic price. Forms with a short accumulator
//! encoding (`add eax,imm32`) are priced at the general one. Every price is
//! checked against iced's encoder in the tests.

fn prefix_bytes(
    width: i64,
    operand: i64,
) -> i64 {
    i64::from(width != operand)
}

/// Bytes of `op r, r` on `width`-byte registers: the opcode and ModRM.
pub fn register_bytes(
    width: i64,
    operand: i64,
) -> i64 {
    prefix_bytes(width, operand) + 2
}

/// Bytes of a register copy: `mov r, r'`, no prefix on a byte.
pub fn copy_move_bytes(
    width: i64,
    operand: i64,
) -> i64 {
    if width == 1 { 2 } else { register_bytes(width, operand) }
}

/// Bytes of loading `number` into a `width`-byte register: `xor r32, r32`
/// for zero (the pin's `MOV32r0`; a dword in real mode takes the prefix),
/// `mov r8, imm8` for a byte, else the opcode and an immediate of the
/// register's width. A qword is `mov r32, imm32` where `number` is
/// unsigned, REX.W and a dword immediate where it is sign-extended, else
/// the whole eight; a symbol's address is a `lea` off `rip`.
pub fn literal_move_bytes(
    number: i64,
    address: bool,
    width: i64,
    operand: i64,
) -> i64 {
    if !address && number == 0 {
        return 2 + i64::from(operand == 2 && width > 2);
    }
    match width {
        1 => 2,
        8 if address => 7,
        8 if u32::try_from(number).is_ok() => 5,
        8 if i32::try_from(number).is_ok() => 7,
        8 => 10,
        _ => prefix_bytes(width, operand) + 1 + width,
    }
}

/// Bytes of a shift of a `width`-byte register by `count`: `D1` for one,
/// `C1` with a byte count otherwise.
pub fn shift_bytes(
    count: i64,
    width: i64,
    operand: i64,
) -> i64 {
    prefix_bytes(width, operand) + if count == 1 { 2 } else { 3 }
}

/// Bytes of `imul r, r, number` on `width`-byte registers: a byte immediate
/// where `number` fits one, else the operand's width.
pub fn imul_immediate_bytes(
    number: i64,
    width: i64,
    operand: i64,
) -> i64 {
    prefix_bytes(width, operand) + 2 + if (-128..=127).contains(&number) { 1 } else { width }
}

/// An operand's width.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Width {
    B8,
    B16,
    B32,
    B64,
}

/// A memory operand's base.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Base {
    /// A register other than rsp, rbp, r12, r13.
    Reg,
    /// rsp or r12: a SIB byte is needed.
    Rsp,
    /// rbp or r13: a displacement is needed.
    Rbp,
    /// `[rip+disp32]`: no index.
    Rip,
    /// `[disp32]` or `[index*scale+disp32]`: SIB and a 32-bit displacement.
    None,
}

/// A memory operand as far as its length goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mem {
    pub base: Base,
    pub index: bool,
    pub disp: i64,
}

impl Mem {
    pub fn new(
        base: Base,
        index: bool,
        disp: i64,
    ) -> Mem {
        debug_assert!(!(base == Base::Rip && index), "rip-relative has no index");
        Mem { base, index, disp }
    }
}

fn fits_byte(value: i64) -> bool {
    (-128..=127).contains(&value)
}

/// The legacy and REX prefixes: 66h for 16 bits, REX for 64 bits or
/// `rex_extra`.
fn prefix(
    width: Width,
    rex_extra: bool,
) -> i64 {
    i64::from(width == Width::B16) + i64::from(width == Width::B64 || rex_extra)
}

/// An immediate of `width` that is sign-extended from a byte where it fits.
fn immediate(
    width: Width,
    value: i64,
) -> i64 {
    match width {
        Width::B8 => 1,
        _ if fits_byte(value) => 1,
        Width::B16 => 2,
        _ => 4,
    }
}

/// ModRM, SIB and displacement of a memory operand.
pub fn mem(m: &Mem) -> i64 {
    match m.base {
        Base::Rip => 1 + 4,
        Base::None => 1 + 1 + 4,
        Base::Reg | Base::Rsp | Base::Rbp => {
            let sib = i64::from(m.index || m.base == Base::Rsp);
            let disp = if m.disp == 0 && m.base != Base::Rbp {
                0
            } else if fits_byte(m.disp) {
                1
            } else {
                4
            };
            1 + sib + disp
        }
    }
}

/// `add`, `sub`, `and`, `or`, `xor`, `cmp`, `adc`, `sbb`, `test`, `mov`:
/// register to register.
pub fn alu_rr(
    width: Width,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra) + 2
}

/// The same with an immediate (`test` has no byte form: see [`test_ri`]).
pub fn alu_ri(
    width: Width,
    imm: i64,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra) + 2 + immediate(width, imm)
}

/// `test r,imm`: `F7 /0`, the immediate is the operand's width (a 64-bit one
/// is 32 bits).
pub fn test_ri(
    width: Width,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra)
        + 2
        + match width {
            Width::B8 => 1,
            Width::B16 => 2,
            _ => 4,
        }
}

/// `mov r,[m]`.
pub fn load(
    width: Width,
    m: &Mem,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra) + 1 + mem(m)
}

/// `mov [m],r`.
pub fn store(
    width: Width,
    m: &Mem,
    rex_extra: bool,
) -> i64 {
    load(width, m, rex_extra)
}

/// `mov [m],imm`: the immediate is the operand's width, a 64-bit one 32 bits.
pub fn store_imm(
    width: Width,
    m: &Mem,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra)
        + 1
        + mem(m)
        + match width {
            Width::B8 => 1,
            Width::B16 => 2,
            _ => 4,
        }
}

/// `lea r,[m]`.
pub fn lea(
    width: Width,
    m: &Mem,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra) + 1 + mem(m)
}

/// `shl`, `shr`, `sar`, `rol`, `ror` by 1, an immediate or `cl`.
pub fn shift(
    width: Width,
    by_immediate: bool,
    count: i64,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra) + 2 + i64::from(by_immediate && count != 1)
}

/// `imul r,r`.
pub fn imul_rr(
    width: Width,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra) + 3
}

/// `imul r,r,imm`.
pub fn imul_ri(
    width: Width,
    imm: i64,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra) + 2 + immediate(width, imm)
}

/// `idiv r`, `div r`, `neg r`, `not r`, `mul r` (`F7 /n`), `inc r`, `dec r`
/// (`FF /n`).
pub fn unary(
    width: Width,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra) + 2
}

/// `cdq` or `cqo`.
pub fn sign_extend_accumulator(width: Width) -> i64 {
    1 + i64::from(width == Width::B64)
}

/// `movzx`, `movsx` and `movsxd` from `from` to `to`; a zero extension from
/// 32 to 64 bits is `mov r32,r32`.
pub fn extend(
    from: Width,
    to: Width,
    signed: bool,
    rex_extra: bool,
) -> i64 {
    match (from, to, signed) {
        (Width::B32, Width::B64, true) => 3,
        (Width::B32, Width::B64, false) => alu_rr(Width::B32, rex_extra),
        // A zero extension to 64 bits is the 32-bit `movzx`.
        (_, Width::B64, false) => 3 + i64::from(rex_extra),
        _ => prefix(to, rex_extra) + 3,
    }
}

/// `cmovcc r,r`.
pub fn cmov(
    width: Width,
    rex_extra: bool,
) -> i64 {
    prefix(width, rex_extra) + 3
}

/// `setcc r8`.
pub fn setcc(rex_extra: bool) -> i64 {
    3 + i64::from(rex_extra)
}

/// `push r`, `pop r`.
pub fn push_pop(rex_extra: bool) -> i64 {
    1 + i64::from(rex_extra)
}

/// `jcc rel8`, `jmp rel8`.
pub const JUMP_SHORT: i64 = 2;
/// `jcc rel32`.
pub const JCC_NEAR: i64 = 6;
/// `jmp rel32`.
pub const JMP_NEAR: i64 = 5;
/// `call rel32`.
pub const CALL: i64 = 5;
/// `ret`.
pub const RET: i64 = 1;

/// `call [m]`, `jmp [m]`.
pub fn indirect_through_memory(
    m: &Mem,
    rex_extra: bool,
) -> i64 {
    i64::from(rex_extra) + 1 + mem(m)
}

/// A scalar SSE operation, register to register: prefix, `0F`, opcode, ModRM
/// (`addss`, `mulsd`, `cvtsi2sd`, `movss`).
pub fn sse_rr(rex_extra: bool) -> i64 {
    4 + i64::from(rex_extra)
}

/// A scalar SSE operation with a memory operand (`movsd xmm,[m]`).
pub fn sse_rm(
    m: &Mem,
    rex_extra: bool,
) -> i64 {
    3 + i64::from(rex_extra) + mem(m)
}

/// `cvtsi2sd xmm,r64` and the other conversions with a 64-bit integer side:
/// REX.W after the mandatory prefix.
pub const SSE_CONVERT_WIDE: i64 = 5;

/// The bytes loop strength reduction orders formulae by at x86-64, read off
/// the table: the displacement a base register gains, an add, a step and a
/// setup `lea` on 64-bit registers, and the exit test's `cmp`.
pub fn lsr_size_model_64() -> llrm_mir::tti::LsrSizeModel {
    use llrm_mir::tti::{LsrSizeModel, OpCost};
    let bytes = |n: i64| OpCost::bytes(n as u32);
    let reg = Mem::new(Base::Reg, false, 0);
    let past_register = |m: Mem| mem(&m) - mem(&reg);
    LsrSizeModel {
        disp_global: bytes(past_register(Mem::new(Base::Rip, false, 0))),
        disp_frame: bytes(past_register(Mem::new(Base::Rsp, false, 8))),
        disp_short: bytes(past_register(Mem::new(Base::Reg, false, 8))),
        disp_long: bytes(past_register(Mem::new(Base::Reg, false, 4096))),
        base_add: bytes(lea(Width::B64, &Mem::new(Base::Reg, true, 0), false)),
        step,
        constant_start: Some(constant_start),
        setup: bytes(lea(Width::B64, &Mem::new(Base::Reg, false, 8), false)),
        setup_cap: 4,
        size_only: true,
        exit_compare,
    }
}

/// The operand width a counter of `bits` is held at.
fn counter_width(bits: u32) -> Width {
    match bits {
        0..=8 => Width::B8,
        9..=16 => Width::B16,
        17..=32 => Width::B32,
        _ => Width::B64,
    }
}

/// `inc` or `dec` for a step of one, `add r,imm` for another constant, `add
/// r,r` for a register's.
fn step(
    bits: u32,
    constant: Option<i64>,
) -> u32 {
    let width = counter_width(bits);
    (match constant {
        Some(1 | -1) => unary(width, false),
        Some(imm) => alu_ri(width, imm, false),
        None => alu_rr(width, false),
    }) as u32
}

/// `xor r32,r32` for zero, else the `mov` of the value.
fn constant_start(
    bits: u32,
    value: i64,
) -> u32 {
    literal_move_bytes(value, false, i64::from(bits.div_ceil(8).next_power_of_two().min(8)), 4) as u32
}

/// A `cmp` of a `bits`-wide counter against an immediate, or a register.
fn exit_compare(
    bits: u32,
    immediate: Option<i64>,
) -> u32 {
    let width = counter_width(bits);
    (match immediate {
        Some(imm) => alu_ri(width, imm, false),
        None => alu_rr(width, false),
    }) as u32
}

#[cfg(test)]
mod tests {
    use iced_x86::{Code, Encoder, Instruction, MemoryOperand, Register};

    use super::*;

    fn length(instruction: Instruction) -> i64 {
        let mut encoder = Encoder::new(64);
        encoder.encode(&instruction, 0x1000).unwrap_or_else(|error| panic!("{instruction}: {error}")) as i64
    }

    fn memory(m: &Mem) -> MemoryOperand {
        let base = match m.base {
            Base::Reg => Register::RAX,
            Base::Rsp => Register::RSP,
            Base::Rbp => Register::RBP,
            Base::Rip => Register::RIP,
            Base::None => Register::None,
        };
        let index = if m.index { Register::RCX } else { Register::None };
        let scale = if m.index { 4 } else { 1 };
        let disp = if m.base == Base::Rip { 0x2000 + m.disp } else { m.disp };
        // The displacement size the assembler is told (8: a 32-bit one in a
        // 64-bit address); iced adds the one rbp and r13 need.
        let size = match m.base {
            Base::Rip | Base::None => 8,
            _ if disp == 0 => 0,
            _ if (-128..=127).contains(&disp) => 1,
            _ => 8,
        };
        MemoryOperand::new(base, index, scale, disp, size, false, Register::None)
    }

    /// Every addressing form, with a displacement of each size.
    fn memories() -> Vec<Mem> {
        let mut all = Vec::new();
        for base in [Base::Reg, Base::Rsp, Base::Rbp, Base::None] {
            for index in [false, true] {
                for disp in [0, 8, -128, 127, 128, 4096] {
                    all.push(Mem::new(base, index, disp));
                }
            }
        }
        all.push(Mem::new(Base::Rip, false, 16));
        all
    }

    fn reg(
        width: Width,
        n: u8,
    ) -> Register {
        match (width, n) {
            (Width::B8, 0) => Register::AL,
            (Width::B8, _) => Register::BL,
            (Width::B16, 0) => Register::AX,
            (Width::B16, _) => Register::BX,
            (Width::B32, 0) => Register::EAX,
            (Width::B32, _) => Register::EBX,
            (Width::B64, 0) => Register::RAX,
            (Width::B64, _) => Register::RBX,
        }
    }

    const WIDTHS: [Width; 4] = [Width::B8, Width::B16, Width::B32, Width::B64];

    fn rm_r(
        width: Width,
        m: &MemoryOperand,
        load: bool,
    ) -> Instruction {
        let r = reg(width, 1);
        let code = match (width, load) {
            (Width::B8, true) => Code::Mov_r8_rm8,
            (Width::B8, false) => Code::Mov_rm8_r8,
            (Width::B16, true) => Code::Mov_r16_rm16,
            (Width::B16, false) => Code::Mov_rm16_r16,
            (Width::B32, true) => Code::Mov_r32_rm32,
            (Width::B32, false) => Code::Mov_rm32_r32,
            (Width::B64, true) => Code::Mov_r64_rm64,
            (Width::B64, false) => Code::Mov_rm64_r64,
        };
        if load { Instruction::with2(code, r, *m).unwrap() } else { Instruction::with2(code, *m, r).unwrap() }
    }

    /// The prefix is for the size that is not the default: a dword in real
    /// mode, a word when flat.
    #[test]
    fn the_operand_size_prefix_is_for_the_size_that_is_not_the_default() {
        assert_eq!(
            (register_bytes(2, 2), register_bytes(4, 2), register_bytes(2, 4), register_bytes(4, 4)),
            (2, 3, 3, 2)
        );
        assert_eq!((shift_bytes(1, 4, 4), shift_bytes(3, 2, 4), imul_immediate_bytes(446, 4, 4)), (2, 4, 6));
        assert_eq!((copy_move_bytes(1, 4), copy_move_bytes(8, 4), copy_move_bytes(4, 4)), (2, 3, 2));
        // recgcd -Os m64: a zero is the 2-byte `xor`, not a copy's REX
        // byte.
        assert_eq!((literal_move_bytes(0, false, 8, 4), register_bytes(8, 4)), (2, 3));
        assert_eq!(
            (
                literal_move_bytes(0, false, 4, 2),
                literal_move_bytes(7, false, 2, 2),
                literal_move_bytes(7, false, 4, 2)
            ),
            (3, 3, 6)
        );
        assert_eq!(
            (
                literal_move_bytes(7, false, 4, 4),
                literal_move_bytes(7, false, 2, 4),
                literal_move_bytes(7, false, 1, 4)
            ),
            (5, 4, 2)
        );
        assert_eq!(
            (
                literal_move_bytes(7, false, 8, 4),
                literal_move_bytes(-1, false, 8, 4),
                literal_move_bytes(1 << 40, false, 8, 4)
            ),
            (5, 7, 10)
        );
    }

    #[test]
    fn alu_register_to_register() {
        for w in WIDTHS {
            let code = match w {
                Width::B8 => Code::Add_rm8_r8,
                Width::B16 => Code::Add_rm16_r16,
                Width::B32 => Code::Add_rm32_r32,
                Width::B64 => Code::Add_rm64_r64,
            };
            let i = Instruction::with2(code, reg(w, 0), reg(w, 1)).unwrap();
            assert_eq!(alu_rr(w, false), length(i), "{w:?}");
        }
        // `mov r8d,eax`: REX.B.
        let i = Instruction::with2(Code::Mov_rm32_r32, Register::R8D, Register::EAX).unwrap();
        assert_eq!(alu_rr(Width::B32, true), length(i));
    }

    #[test]
    fn alu_immediate_byte_and_full() {
        for (w, imm, code) in [
            (Width::B8, 5, Code::Add_rm8_imm8),
            (Width::B16, 5, Code::Add_rm16_imm8),
            (Width::B16, 1000, Code::Add_rm16_imm16),
            (Width::B32, 5, Code::Add_rm32_imm8),
            (Width::B32, 1000, Code::Add_rm32_imm32),
            (Width::B64, -128, Code::Add_rm64_imm8),
            (Width::B64, 1000, Code::Add_rm64_imm32),
        ] {
            let r = reg(w, 1);
            let i = Instruction::with2(code, r, imm as i32).unwrap();
            assert_eq!(alu_ri(w, imm, false), length(i), "{w:?} {imm}");
        }
    }

    #[test]
    fn test_immediate_has_no_byte_form() {
        for (w, code) in [
            (Width::B8, Code::Test_rm8_imm8),
            (Width::B16, Code::Test_rm16_imm16),
            (Width::B32, Code::Test_rm32_imm32),
            (Width::B64, Code::Test_rm64_imm32),
        ] {
            let i = Instruction::with2(code, reg(w, 1), 5).unwrap();
            assert_eq!(test_ri(w, false), length(i), "{w:?}");
        }
    }

    #[test]
    fn load_and_store_every_addressing_form() {
        for m in memories() {
            let operand = memory(&m);
            for w in WIDTHS {
                for is_load in [true, false] {
                    let i = rm_r(w, &operand, is_load);
                    let got = if is_load { load(w, &m, false) } else { store(w, &m, false) };
                    assert_eq!(got, length(i), "{w:?} {m:?} load={is_load}");
                }
            }
        }
    }

    #[test]
    fn store_immediate_every_addressing_form() {
        for m in memories() {
            let operand = memory(&m);
            for (w, code) in [
                (Width::B8, Code::Mov_rm8_imm8),
                (Width::B16, Code::Mov_rm16_imm16),
                (Width::B32, Code::Mov_rm32_imm32),
                (Width::B64, Code::Mov_rm64_imm32),
            ] {
                let i = Instruction::with2(code, operand, 9).unwrap();
                assert_eq!(store_imm(w, &m, false), length(i), "{w:?} {m:?}");
            }
        }
    }

    #[test]
    fn lea_every_addressing_form() {
        for m in memories() {
            let operand = memory(&m);
            for (w, code, r) in
                [(Width::B32, Code::Lea_r32_m, Register::EBX), (Width::B64, Code::Lea_r64_m, Register::RBX)]
            {
                let i = Instruction::with2(code, r, operand).unwrap();
                assert_eq!(lea(w, &m, false), length(i), "{w:?} {m:?}");
            }
        }
    }

    #[test]
    fn extra_rex_adds_one_byte() {
        let m = Mem::new(Base::Reg, false, 8);
        let i = Instruction::with2(Code::Mov_r32_rm32, Register::R9D, MemoryOperand::with_base_displ(Register::RAX, 8))
            .unwrap();
        assert_eq!(load(Width::B32, &m, true), length(i));
    }

    #[test]
    fn shifts() {
        let i = Instruction::with2(Code::Shl_rm32_1, Register::EBX, 1).unwrap();
        assert_eq!(shift(Width::B32, true, 1, false), length(i));
        let i = Instruction::with2(Code::Shl_rm32_imm8, Register::EBX, 3).unwrap();
        assert_eq!(shift(Width::B32, true, 3, false), length(i));
        let i = Instruction::with2(Code::Shl_rm64_imm8, Register::RBX, 3).unwrap();
        assert_eq!(shift(Width::B64, true, 3, false), length(i));
        let i = Instruction::with2(Code::Shl_rm32_CL, Register::EBX, Register::CL).unwrap();
        assert_eq!(shift(Width::B32, false, 0, false), length(i));
    }

    #[test]
    fn multiplies_and_unary() {
        let i = Instruction::with2(Code::Imul_r32_rm32, Register::EAX, Register::EBX).unwrap();
        assert_eq!(imul_rr(Width::B32, false), length(i));
        let i = Instruction::with2(Code::Imul_r64_rm64, Register::RAX, Register::RBX).unwrap();
        assert_eq!(imul_rr(Width::B64, false), length(i));
        let i = Instruction::with3(Code::Imul_r32_rm32_imm8, Register::EAX, Register::EBX, 5).unwrap();
        assert_eq!(imul_ri(Width::B32, 5, false), length(i));
        let i = Instruction::with3(Code::Imul_r32_rm32_imm32, Register::EAX, Register::EBX, 1000).unwrap();
        assert_eq!(imul_ri(Width::B32, 1000, false), length(i));
        let i = Instruction::with3(Code::Imul_r64_rm64_imm32, Register::RAX, Register::RBX, 1000).unwrap();
        assert_eq!(imul_ri(Width::B64, 1000, false), length(i));
        let i = Instruction::with1(Code::Idiv_rm32, Register::EBX).unwrap();
        assert_eq!(unary(Width::B32, false), length(i));
        let i = Instruction::with1(Code::Idiv_rm64, Register::RBX).unwrap();
        assert_eq!(unary(Width::B64, false), length(i));
        assert_eq!(sign_extend_accumulator(Width::B32), length(Instruction::with(Code::Cdq)));
        assert_eq!(sign_extend_accumulator(Width::B64), length(Instruction::with(Code::Cqo)));
    }

    #[test]
    fn extensions() {
        let cases = [
            (
                Width::B8,
                Width::B32,
                true,
                Instruction::with2(Code::Movsx_r32_rm8, Register::EAX, Register::BL).unwrap(),
            ),
            (
                Width::B8,
                Width::B32,
                false,
                Instruction::with2(Code::Movzx_r32_rm8, Register::EAX, Register::BL).unwrap(),
            ),
            (
                Width::B16,
                Width::B32,
                true,
                Instruction::with2(Code::Movsx_r32_rm16, Register::EAX, Register::BX).unwrap(),
            ),
            (
                Width::B16,
                Width::B32,
                false,
                Instruction::with2(Code::Movzx_r32_rm16, Register::EAX, Register::BX).unwrap(),
            ),
            (Width::B8, Width::B16, true, Instruction::with2(Code::Movsx_r16_rm8, Register::AX, Register::BL).unwrap()),
            (
                Width::B8,
                Width::B64,
                true,
                Instruction::with2(Code::Movsx_r64_rm8, Register::RAX, Register::BL).unwrap(),
            ),
            (
                Width::B16,
                Width::B64,
                true,
                Instruction::with2(Code::Movsx_r64_rm16, Register::RAX, Register::BX).unwrap(),
            ),
            (
                Width::B32,
                Width::B64,
                true,
                Instruction::with2(Code::Movsxd_r64_rm32, Register::RAX, Register::EBX).unwrap(),
            ),
            (
                Width::B32,
                Width::B64,
                false,
                Instruction::with2(Code::Mov_r32_rm32, Register::EAX, Register::EBX).unwrap(),
            ),
            (
                Width::B8,
                Width::B64,
                false,
                Instruction::with2(Code::Movzx_r32_rm8, Register::EAX, Register::BL).unwrap(),
            ),
            (
                Width::B16,
                Width::B64,
                false,
                Instruction::with2(Code::Movzx_r32_rm16, Register::EAX, Register::BX).unwrap(),
            ),
        ];
        for (from, to, signed, instruction) in cases {
            assert_eq!(extend(from, to, signed, false), length(instruction), "{from:?}->{to:?} signed={signed}");
        }
    }

    #[test]
    fn conditionals_and_stack() {
        let i = Instruction::with2(Code::Cmove_r32_rm32, Register::EAX, Register::EBX).unwrap();
        assert_eq!(cmov(Width::B32, false), length(i));
        let i = Instruction::with2(Code::Cmove_r64_rm64, Register::RAX, Register::RBX).unwrap();
        assert_eq!(cmov(Width::B64, false), length(i));
        let i = Instruction::with1(Code::Sete_rm8, Register::AL).unwrap();
        assert_eq!(setcc(false), length(i));
        let i = Instruction::with1(Code::Push_r64, Register::RBX).unwrap();
        assert_eq!(push_pop(false), length(i));
        let i = Instruction::with1(Code::Pop_r64, Register::R12).unwrap();
        assert_eq!(push_pop(true), length(i));
    }

    #[test]
    fn jumps_calls_and_returns() {
        let i = Instruction::with_branch(Code::Je_rel8_64, 0x1010).unwrap();
        assert_eq!(JUMP_SHORT, length(i));
        let i = Instruction::with_branch(Code::Je_rel32_64, 0x1100).unwrap();
        assert_eq!(JCC_NEAR, length(i));
        let i = Instruction::with_branch(Code::Jmp_rel32_64, 0x1100).unwrap();
        assert_eq!(JMP_NEAR, length(i));
        let i = Instruction::with_branch(Code::Call_rel32_64, 0x1100).unwrap();
        assert_eq!(CALL, length(i));
        assert_eq!(RET, length(Instruction::with(Code::Retnq)));
        for m in memories() {
            let i = Instruction::with1(Code::Call_rm64, memory(&m)).unwrap();
            assert_eq!(indirect_through_memory(&m, false), length(i), "{m:?}");
        }
    }

    #[test]
    fn scalar_sse() {
        let i = Instruction::with2(Code::Addss_xmm_xmmm32, Register::XMM0, Register::XMM1).unwrap();
        assert_eq!(sse_rr(false), length(i));
        let i = Instruction::with2(Code::Mulsd_xmm_xmmm64, Register::XMM0, Register::XMM9).unwrap();
        assert_eq!(sse_rr(true), length(i));
        for m in memories() {
            let i = Instruction::with2(Code::Movsd_xmm_xmmm64, Register::XMM0, memory(&m)).unwrap();
            assert_eq!(sse_rm(&m, false), length(i), "{m:?}");
        }
        let i = Instruction::with2(Code::Cvtsi2sd_xmm_rm64, Register::XMM0, Register::RBX).unwrap();
        assert_eq!(SSE_CONVERT_WIDE, length(i));
    }

    #[test]
    fn the_selectors_prices_match_the_encoder() {
        for (w, width, code) in [(Width::B16, 2, Code::Add_rm16_r16), (Width::B32, 4, Code::Add_rm32_r32)] {
            let i = Instruction::with2(code, reg(w, 0), reg(w, 1)).unwrap();
            assert_eq!(register_bytes(width, 4), length(i), "{w:?}");
        }
        let cases = [
            (0, 4, Instruction::with2(Code::Xor_r32_rm32, Register::EAX, Register::EAX).unwrap()),
            (7, 4, Instruction::with2(Code::Mov_r32_imm32, Register::EBX, 7).unwrap()),
            (7, 2, Instruction::with2(Code::Mov_r16_imm16, Register::BX, 7).unwrap()),
            (7, 1, Instruction::with2(Code::Mov_r8_imm8, Register::BL, 7).unwrap()),
            (1000, 8, Instruction::with2(Code::Mov_r32_imm32, Register::EBX, 1000).unwrap()),
            (-1, 8, Instruction::with2(Code::Mov_rm64_imm32, Register::RBX, -1).unwrap()),
            (1 << 40, 8, Instruction::with2(Code::Mov_r64_imm64, Register::RBX, 1u64 << 40).unwrap()),
        ];
        for (number, width, instruction) in cases {
            assert_eq!(literal_move_bytes(number, false, width, 4), length(instruction), "{number} x{width}");
        }
        let i = Instruction::with2(Code::Shl_rm32_1, Register::EBX, 1).unwrap();
        assert_eq!(shift_bytes(1, 4, 4), length(i));
        let i = Instruction::with2(Code::Shl_rm32_imm8, Register::EBX, 3).unwrap();
        assert_eq!(shift_bytes(3, 4, 4), length(i));
        let i = Instruction::with3(Code::Imul_r32_rm32_imm32, Register::EAX, Register::EBX, 446).unwrap();
        assert_eq!(imul_immediate_bytes(446, 4, 4), length(i));
        let i = Instruction::with3(Code::Imul_r32_rm32_imm8, Register::EAX, Register::EBX, 5).unwrap();
        assert_eq!(imul_immediate_bytes(5, 4, 4), length(i));
    }

    #[test]
    fn the_lsr_model_prices_a_counter_as_the_encoder_does() {
        let m = lsr_size_model_64();
        // -O1 keeps the pin's order: its 11 cells that differ with the model
        // trade bytes for up to +20% instructions (tile).
        assert!(m.size_only);
        let cases = [
            ((m.step)(32, Some(1)), Instruction::with1(Code::Inc_rm32, Register::EBX).unwrap()),
            ((m.step)(32, Some(-1)), Instruction::with1(Code::Dec_rm32, Register::EBX).unwrap()),
            ((m.step)(64, Some(1)), Instruction::with1(Code::Inc_rm64, Register::RBX).unwrap()),
            ((m.step)(16, Some(1)), Instruction::with1(Code::Inc_rm16, Register::BX).unwrap()),
            ((m.step)(32, Some(4)), Instruction::with2(Code::Add_rm32_imm8, Register::EBX, 4).unwrap()),
            ((m.step)(64, Some(1000)), Instruction::with2(Code::Add_rm64_imm32, Register::RBX, 1000).unwrap()),
            ((m.step)(64, None), Instruction::with2(Code::Add_rm64_r64, Register::RBX, Register::RCX).unwrap()),
            (
                (m.constant_start.unwrap())(32, 0),
                Instruction::with2(Code::Xor_r32_rm32, Register::EBX, Register::EBX).unwrap(),
            ),
            ((m.constant_start.unwrap())(32, 8), Instruction::with2(Code::Mov_r32_imm32, Register::EBX, 8).unwrap()),
            ((m.constant_start.unwrap())(16, 8), Instruction::with2(Code::Mov_r16_imm16, Register::BX, 8).unwrap()),
            ((m.exit_compare)(32, Some(8)), Instruction::with2(Code::Cmp_rm32_imm8, Register::EBX, 8).unwrap()),
            ((m.exit_compare)(16, Some(8)), Instruction::with2(Code::Cmp_rm16_imm8, Register::BX, 8).unwrap()),
            ((m.exit_compare)(64, Some(1000)), Instruction::with2(Code::Cmp_rm64_imm32, Register::RBX, 1000).unwrap()),
            ((m.exit_compare)(32, None), Instruction::with2(Code::Cmp_rm32_r32, Register::EBX, Register::ECX).unwrap()),
        ];
        for (priced, instruction) in cases {
            assert_eq!(i64::from(priced), length(instruction), "{instruction}");
        }
        // A value a register's width holds: the 64-bit start below 2^32 is
        // the zero-extending dword `mov`.
        assert_eq!(
            i64::from((m.constant_start.unwrap())(64, 8)),
            length(Instruction::with2(Code::Mov_r32_imm32, Register::EBX, 8).unwrap())
        );
    }
}
