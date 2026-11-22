//! The atomic read-modify-writes the pin selects (`LOCK_ADD32mr`, `LXADD32`,
//! `LCMPXCHG32`...) as the emitter encodes them. Each byte string is what the
//! pin's assembler makes of the line in its comment; `lock` is `F0`, after an
//! operand-size prefix (`66 f0`), before REX.

use super::sweep_support::{im, rg, sem};
use super::*;

type R = RegId;

fn cell(
    bits: u32,
    disp: i64,
    width: u32,
) -> Loc {
    let through = if bits == 64 { R::RBP } else { R::EBP };
    Loc::Mem(ir::Mem { through, disp_width: 2, ..ir::Mem::new(Some(llrm_lir::Addr::new(Space::Frame, disp)), width) })
}

fn encoded(
    bits: u32,
    what: Semantics,
) -> Vec<u8> {
    emit_in(bits, &Semantics { lock: true, ..what }, 0, None, false, false, None).expect("the emitter encodes it").code
}

/// `lock add %reg, [cell]` and the other four accumulates.
fn accumulate(
    name: &str,
    cell: Loc,
    source: Loc,
) -> Semantics {
    sem(Operation::Binary, Some(name), vec![cell.clone()], vec![cell, source], None, false)
}

fn unary(
    name: &str,
    cell: Loc,
) -> Semantics {
    sem(Operation::Unary, Some(name), vec![cell.clone()], vec![cell], None, false)
}

fn xadd(
    cell: Loc,
    register: Loc,
) -> Semantics {
    sem(Operation::Binary, Some("xadd"), vec![cell.clone(), register.clone()], vec![cell, register], None, false)
}

fn cmpxchg(
    cell: Loc,
    swap: Loc,
    accumulator: Loc,
) -> Semantics {
    sem(
        Operation::Binary,
        Some("cmpxchg"),
        vec![cell.clone(), accumulator.clone()],
        vec![cell, swap, accumulator],
        None,
        false,
    )
}

/// Without the prefix a locked `add` of a cell was the plain `add` (`01 5d
/// e4`): a counter two threads bumped lost updates.
#[test]
fn a_locked_accumulate_into_a_cell_is_the_prefix_then_the_instruction_at_m32() {
    let ebx = || rg(R::EBX, 4);
    let at32 = |name, value| encoded(32, accumulate(name, cell(32, -28, 4), value));
    // lock addl %ebx,-0x1c(%ebp); lock subl; lock andl; lock orl; lock xorl
    assert_eq!(at32("add", ebx()), [0xf0, 0x01, 0x5d, 0xe4]);
    assert_eq!(at32("sub", ebx()), [0xf0, 0x29, 0x5d, 0xe4]);
    assert_eq!(at32("and", ebx()), [0xf0, 0x21, 0x5d, 0xe4]);
    assert_eq!(at32("or", ebx()), [0xf0, 0x09, 0x5d, 0xe4]);
    assert_eq!(at32("xor", ebx()), [0xf0, 0x31, 0x5d, 0xe4]);
    // lock addl $5,-0x1c(%ebp); lock addl $1000,-0x1c(%ebp)
    assert_eq!(at32("add", im(5, 4, None)), [0xf0, 0x83, 0x45, 0xe4, 0x05]);
    assert_eq!(at32("add", im(1000, 4, None)), [0xf0, 0x81, 0x45, 0xe4, 0xe8, 0x03, 0, 0]);
    // lock incl -0x1c(%ebp); lock decl -0x1c(%ebp)
    assert_eq!(encoded(32, unary("inc", cell(32, -28, 4))), [0xf0, 0xff, 0x45, 0xe4]);
    assert_eq!(encoded(32, unary("dec", cell(32, -28, 4))), [0xf0, 0xff, 0x4d, 0xe4]);
}

/// The operand-size prefix comes before `lock`: `66 f0`, not `f0 66`.
#[test]
fn a_locked_word_op_puts_the_operand_size_prefix_before_lock_at_m32() {
    // lock addw %si,-0x2a(%ebp); lock addb %al,-0x2b(%ebp)
    assert_eq!(encoded(32, accumulate("add", cell(32, -42, 2), rg(R::SI, 2))), [0x66, 0xf0, 0x01, 0x75, 0xd6]);
    assert_eq!(encoded(32, accumulate("add", cell(32, -43, 1), rg(R::AL, 1))), [0xf0, 0x00, 0x45, 0xd5]);
    // lock xaddw %si,-0x2a(%ebp); lock cmpxchgw %dx,-0x2a(%ebp)
    assert_eq!(encoded(32, xadd(cell(32, -42, 2), rg(R::SI, 2))), [0x66, 0xf0, 0x0f, 0xc1, 0x75, 0xd6]);
    assert_eq!(
        encoded(32, cmpxchg(cell(32, -42, 2), rg(R::DX, 2), rg(R::AX, 2))),
        [0x66, 0xf0, 0x0f, 0xb1, 0x55, 0xd6]
    );
}

/// `xadd` and `cmpxchg` were not forms: a selected `LXADD32` stopped the
/// compile.
#[test]
fn a_locked_xadd_and_cmpxchg_encode_at_m32() {
    // lock xaddl %ebx,-0x1c(%ebp); lock xaddb %al,-0x2b(%ebp)
    assert_eq!(encoded(32, xadd(cell(32, -28, 4), rg(R::EBX, 4))), [0xf0, 0x0f, 0xc1, 0x5d, 0xe4]);
    assert_eq!(encoded(32, xadd(cell(32, -43, 1), rg(R::AL, 1))), [0xf0, 0x0f, 0xc0, 0x45, 0xd5]);
    // lock cmpxchgl %ecx,-0x1c(%ebp); lock cmpxchgb %cl,-0x2b(%ebp)
    assert_eq!(encoded(32, cmpxchg(cell(32, -28, 4), rg(R::ECX, 4), rg(R::EAX, 4))), [0xf0, 0x0f, 0xb1, 0x4d, 0xe4]);
    assert_eq!(encoded(32, cmpxchg(cell(32, -43, 1), rg(R::CL, 1), rg(R::AL, 1))), [0xf0, 0x0f, 0xb0, 0x4d, 0xd5]);
}

/// REX comes after `lock`, `lock` after the operand-size prefix, and a quadword
/// is REX.W.
#[test]
fn a_locked_op_at_m64_puts_lock_before_rex() {
    let at64 = |what| encoded(64, what);
    // lock addq %rcx,-0x18(%rbp); lock addl %r10d,-0xc(%rbp); lock addw
    // %dx,-0x1a(%rbp)
    assert_eq!(at64(accumulate("add", cell(64, -24, 8), rg(R::RCX, 8))), [0xf0, 0x48, 0x01, 0x4d, 0xe8]);
    assert_eq!(at64(accumulate("add", cell(64, -12, 4), rg(R::R10D, 4))), [0xf0, 0x44, 0x01, 0x55, 0xf4]);
    assert_eq!(at64(accumulate("add", cell(64, -26, 2), rg(R::DX, 2))), [0x66, 0xf0, 0x01, 0x55, 0xe6]);
    // lock subq; lock andq; lock orq; lock xorq (all %rcx, -0x18(%rbp))
    for (name, opcode) in [("sub", 0x29), ("and", 0x21), ("or", 0x09), ("xor", 0x31)] {
        assert_eq!(at64(accumulate(name, cell(64, -24, 8), rg(R::RCX, 8))), [0xf0, 0x48, opcode, 0x4d, 0xe8]);
    }
    // lock addq $5,-0x18(%rbp); lock incq; lock decq
    assert_eq!(at64(accumulate("add", cell(64, -24, 8), im(5, 8, None))), [0xf0, 0x48, 0x83, 0x45, 0xe8, 0x05]);
    assert_eq!(at64(unary("inc", cell(64, -24, 8))), [0xf0, 0x48, 0xff, 0x45, 0xe8]);
    assert_eq!(at64(unary("dec", cell(64, -24, 8))), [0xf0, 0x48, 0xff, 0x4d, 0xe8]);
    // lock xaddq %r9; lock xaddw %r8w; lock xaddb %dil
    assert_eq!(at64(xadd(cell(64, -24, 8), rg(R::R9, 8))), [0xf0, 0x4c, 0x0f, 0xc1, 0x4d, 0xe8]);
    assert_eq!(at64(xadd(cell(64, -26, 2), rg(R::R8W, 2))), [0x66, 0xf0, 0x44, 0x0f, 0xc1, 0x45, 0xe6]);
    assert_eq!(at64(xadd(cell(64, -27, 1), rg(R::DIL, 1))), [0xf0, 0x40, 0x0f, 0xc0, 0x7d, 0xe5]);
    // lock cmpxchgq %rcx; lock cmpxchgl %r10d; lock cmpxchgw %dx; lock cmpxchgb
    // %sil
    assert_eq!(at64(cmpxchg(cell(64, -24, 8), rg(R::RCX, 8), rg(R::RAX, 8))), [0xf0, 0x48, 0x0f, 0xb1, 0x4d, 0xe8]);
    assert_eq!(at64(cmpxchg(cell(64, -12, 4), rg(R::R10D, 4), rg(R::EAX, 4))), [0xf0, 0x44, 0x0f, 0xb1, 0x55, 0xf4]);
    assert_eq!(at64(cmpxchg(cell(64, -26, 2), rg(R::DX, 2), rg(R::AX, 2))), [0x66, 0xf0, 0x0f, 0xb1, 0x55, 0xe6]);
    assert_eq!(at64(cmpxchg(cell(64, -27, 1), rg(R::SIL, 1), rg(R::AL, 1))), [0xf0, 0x40, 0x0f, 0xb0, 0x75, 0xe5]);
}

/// A `lock` no arm applies is a refusal: a locked register-to-register `add`
/// was encoded without the prefix, an atomic that was not.
#[test]
fn a_lock_on_a_register_destination_is_refused() {
    let plain =
        sem(Operation::Binary, Some("add"), vec![rg(R::EAX, 4)], vec![rg(R::EAX, 4), rg(R::ECX, 4)], None, false);
    assert!(emit_in(32, &Semantics { lock: true, ..plain.clone() }, 0, None, false, false, None).is_none());
    assert!(emit_in(32, &plain, 0, None, false, false, None).is_some());
}

/// The unlocked `add` of a cell is still the plain instruction.
#[test]
fn an_unlocked_accumulate_has_no_prefix() {
    let what = accumulate("add", cell(32, -28, 4), rg(R::EBX, 4));
    assert_eq!(emit_in(32, &what, 0, None, false, false, None).unwrap().code, [0x01, 0x5d, 0xe4]);
}
