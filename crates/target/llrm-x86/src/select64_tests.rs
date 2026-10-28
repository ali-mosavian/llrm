//! The emitter in 64-bit mode. Each expected byte string is what GNU as /
//! clang's integrated assembler makes of the line in its comment.

use super::sweep_support::{im, rg, sem};
use super::*;

fn bytes(what: &Semantics) -> Vec<u8> {
    emit_in(64, what, 0, None, false, false, None).expect("the emitter encodes it").code
}

#[test]
fn push_and_pop_a_quadword_register() {
    // push %rbx; push %r12
    let push = |register| sem(Operation::Push, Some("push"), vec![], vec![rg(register, 8)], None, false);
    assert_eq!(bytes(&push(RegId::RBX)), [0x53]);
    assert_eq!(bytes(&push(RegId::R12)), [0x41, 0x54]);
    // pop %rbx; pop %r12
    let pop = |register| sem(Operation::Pop, Some("pop"), vec![rg(register, 8)], vec![], None, false);
    assert_eq!(bytes(&pop(RegId::RBX)), [0x5b]);
    assert_eq!(bytes(&pop(RegId::R12)), [0x41, 0x5c]);
}

#[test]
fn ret_is_a_near_return_without_an_operand_size_prefix() {
    // ret: it was `66 c3` (retw), which pops a word-sized return address' worth
    // of nothing sensible.
    assert_eq!(bytes(&sem(Operation::Return, Some("ret"), vec![], vec![], None, false)), [0xc3]);
}

#[test]
fn a_quadword_move_takes_rex_w() {
    // mov %rax,%rbx (the 8b form, as for dwords); mov $0x2a,%eax; mov %r8,%r9
    let mov = |into, from| sem(Operation::Move, Some("mov"), vec![rg(into, 8)], vec![rg(from, 8)], None, false);
    assert_eq!(bytes(&mov(RegId::RBX, RegId::RAX)), [0x48, 0x8b, 0xd8]);
    assert_eq!(bytes(&mov(RegId::R9, RegId::R8)), [0x4d, 0x8b, 0xc8]);
    let load = sem(Operation::Move, Some("mov"), vec![rg(RegId::EAX, 4)], vec![im(42, 4, None)], None, false);
    assert_eq!(bytes(&load), [0xb8, 0x2a, 0, 0, 0]);
}

#[test]
fn a_quadword_constant_is_loaded_in_the_shortest_form() {
    let load =
        |value| sem(Operation::Move, Some("mov"), vec![rg(RegId::RAX, 8)], vec![im(value, 8, None)], None, false);
    // mov $0x2a,%eax (zero-extends); mov $-1,%rax; movabs $0x123456789,%rax
    assert_eq!(bytes(&load(42)), [0xb8, 0x2a, 0, 0, 0]);
    assert_eq!(bytes(&load(-1)), [0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(bytes(&load(0x1_2345_6789)), [0x48, 0xb8, 0x89, 0x67, 0x45, 0x23, 0x01, 0, 0, 0]);
}

#[test]
fn a_quadword_add_takes_a_sign_extended_dword() {
    let add = |value| {
        sem(
            Operation::Binary,
            Some("add"),
            vec![rg(RegId::RAX, 8)],
            vec![rg(RegId::RAX, 8), im(value, 8, None)],
            None,
            false,
        )
    };
    // add $0x10,%rax; add $0x1000,%rax; add $0x1000,%rbx
    assert_eq!(bytes(&add(16)), [0x48, 0x83, 0xc0, 0x10]);
    assert_eq!(bytes(&add(0x1000)), [0x48, 0x05, 0x00, 0x10, 0, 0]);
}

fn frame(
    disp: i64,
    width: u32,
) -> Loc {
    Loc::Mem(ir::Mem {
        through: RegId::RBP,
        disp_width: 2,
        ..ir::Mem::new(Some(llrm_lir::Addr::new(Space::Frame, disp)), width)
    })
}

#[test]
fn a_frame_cell_is_addressed_through_rbp() {
    // mov %rax,-0x8(%rbp); mov %rax,-0x90(%rbp); mov -0x4(%rbp),%ecx
    let store = |disp| sem(Operation::Move, Some("mov"), vec![frame(disp, 8)], vec![rg(RegId::RAX, 8)], None, false);
    assert_eq!(bytes(&store(-8)), [0x48, 0x89, 0x45, 0xf8]);
    assert_eq!(bytes(&store(-0x90)), [0x48, 0x89, 0x85, 0x70, 0xff, 0xff, 0xff]);
    let load = sem(Operation::Move, Some("mov"), vec![rg(RegId::ECX, 4)], vec![frame(-4, 4)], None, false);
    assert_eq!(bytes(&load), [0x8b, 0x4d, 0xfc]);
}

#[test]
fn a_dword_widens_to_a_quadword() {
    let extend =
        |name, into, from| sem(Operation::Extend, Some(name), vec![rg(into, 8)], vec![rg(from, 4)], None, false);
    // movslq %ecx,%rax; mov %ecx,%eax (zero extension); movslq %r9d,%r10
    assert_eq!(bytes(&extend("movsx", RegId::RAX, RegId::ECX)), [0x48, 0x63, 0xc1]);
    assert_eq!(bytes(&extend("movzx", RegId::RAX, RegId::ECX)), [0x8b, 0xc1]);
    assert_eq!(bytes(&extend("movsx", RegId::R10, RegId::R9D)), [0x4d, 0x63, 0xd1]);
}

#[test]
fn a_quadword_push_of_a_constant() {
    // push $0 was `pushw $0` (66 6a 00): a word, which left the stack
    // misaligned for the call that followed.
    let push = |value| sem(Operation::Push, Some("push"), vec![], vec![im(value, 8, None)], None, false);
    assert_eq!(bytes(&push(0)), [0x6a, 0x00]);
    assert_eq!(bytes(&push(0x1234)), [0x68, 0x34, 0x12, 0x00, 0x00]);
}

#[test]
fn a_conditional_move_names_its_condition() {
    // cmovl %ecx,%eax; cmovne %rdx,%rax; cmovae %r9,%r8
    let cmov = |name, into, from, width| {
        sem(Operation::Select, Some(name), vec![rg(into, width)], vec![rg(into, width), rg(from, width)], None, false)
    };
    assert_eq!(bytes(&cmov("cmovl", RegId::EAX, RegId::ECX, 4)), [0x0f, 0x4c, 0xc1]);
    assert_eq!(bytes(&cmov("cmovne", RegId::RAX, RegId::RDX, 8)), [0x48, 0x0f, 0x45, 0xc2]);
    assert_eq!(bytes(&cmov("cmovae", RegId::R8, RegId::R9, 8)), [0x4d, 0x0f, 0x43, 0xc1]);
}

#[test]
fn a_dword_inc_and_dec_take_the_modrm_form() {
    // dec %esi; inc %ecx: 0x48+r and 0x40+r are REX prefixes here. `dec %esi`
    // stopped the assembler ("ack: Semantics(... 'dec' ...)") and with it the
    // compile of ackermann, fib and heap.
    let one =
        |name, register| sem(Operation::Unary, Some(name), vec![rg(register, 4)], vec![rg(register, 4)], None, false);
    assert_eq!(bytes(&one("dec", RegId::ESI)), [0xff, 0xce]);
    assert_eq!(bytes(&one("inc", RegId::ECX)), [0xff, 0xc1]);
}

#[test]
fn a_quadword_multiply_by_an_immediate_takes_imm32() {
    // imul $251,%rax,%rax: the immediate is 32 bits sign-extended, not 8 bytes
    // (unsig stopped the assembler on it).
    let imul = sem(
        Operation::Multiply,
        Some("imul"),
        vec![rg(RegId::RAX, 8)],
        vec![rg(RegId::RAX, 8), im(251, 8, None)],
        None,
        false,
    );
    assert_eq!(bytes(&imul), [0x48, 0x69, 0xc0, 0xfb, 0, 0, 0]);
}

#[test]
fn a_quadword_divide_and_negate_take_a_memory_operand() {
    // idiv qword ptr [rbp]; neg qword ptr [rbp]: the one-operand memory forms
    // took words and dwords only, and the assembler stopped on `main` of
    // tests/llc divs.
    use super::sweep_support::{R, a, m};
    let cell = || m(Some(a(llrm_lir::Space::Frame, 0, 0, R::None, R::None)), 8, R::RBP, 0, 0, None, None, 1, R::None);
    let idiv = sem(
        Operation::Divide,
        Some("idiv"),
        vec![rg(RegId::RAX, 8), rg(RegId::RDX, 8)],
        vec![rg(RegId::RDX, 8), rg(RegId::RAX, 8), cell()],
        None,
        false,
    );
    assert_eq!(bytes(&idiv), [0x48, 0xf7, 0x7d, 0x00]);
    let neg = sem(Operation::Unary, Some("neg"), vec![cell()], vec![cell()], None, false);
    assert_eq!(bytes(&neg), [0x48, 0xf7, 0x5d, 0x00]);
}

/// `movzx esi, byte [bp + rdx - 20]` (a frame array's element, the frame
/// token a word register) panicked in iced's encoder (mixed address sizes)
/// when a pass priced it before the frame layout named the token; it is
/// unencodable there, not a panic.
#[test]
fn a_frame_cell_with_a_wide_index_is_not_encodable_before_the_frame_is_named() {
    let cell = ir::Mem {
        through: RegId::BP,
        index: Some(llrm_lir::Held { value: 1, width: 8 }),
        index_through: RegId::RDX,
        scale: 1,
        disp_width: 2,
        ..ir::Mem::new(Some(llrm_lir::Addr::new(Space::Frame, -20)), 1)
    };
    let load = sem(Operation::Extend, Some("movzx"), vec![rg(RegId::ESI, 4)], vec![Loc::Mem(cell)], None, false);
    assert!(emit_in(64, &load, 0, None, false, false, None).is_none());
}

#[test]
fn a_quadword_cell_is_compared_and_added_to_with_an_immediate() {
    // cmpq $0,-0x8(%rbp); cmpq $0x1000,-0x8(%rbp); addq $1,-0x8(%rbp). At -O0
    // these had no form and the compile stopped ("greedy"-free: the emitter
    // returned nothing for the instruction).
    let op = |name, value| {
        let cell = frame(-8, 8);
        if name == "cmp" {
            sem(Operation::Compare, Some(name), vec![], vec![cell, im(value, 8, None)], None, false)
        } else {
            sem(Operation::Binary, Some(name), vec![cell.clone()], vec![cell, im(value, 8, None)], None, false)
        }
    };
    assert_eq!(bytes(&op("cmp", 0)), [0x48, 0x83, 0x7d, 0xf8, 0x00]);
    assert_eq!(bytes(&op("cmp", 0x1000)), [0x48, 0x81, 0x7d, 0xf8, 0x00, 0x10, 0, 0]);
    assert_eq!(bytes(&op("add", 1)), [0x48, 0x83, 0x45, 0xf8, 0x01]);
}

#[test]

fn a_scalar_double_add_is_the_sse2_form() {
    // addsd %xmm1,%xmm0; addss %xmm9,%xmm8. Without an SSE row the emitter has
    // no form to encode.
    let add = |name, into, from, width| {
        sem(Operation::Binary, Some(name), vec![rg(into, width)], vec![rg(into, width), rg(from, width)], None, false)
    };
    assert_eq!(
        bytes(&add("addsd", RegId::from(Register::XMM0), RegId::from(Register::XMM1), 8)),
        [0xf2, 0x0f, 0x58, 0xc1]
    );
    assert_eq!(
        bytes(&add("addss", RegId::from(Register::XMM8), RegId::from(Register::XMM9), 4)),
        [0xf3, 0x45, 0x0f, 0x58, 0xc1]
    );
}

#[test]
fn the_other_scalar_sse_forms_encode() {
    let x = |n: u8| RegId::from(Register::XMM0 + u32::from(n));
    let conv = |name, dest, d: u32, source, s: u32| {
        sem(Operation::Extend, Some(name), vec![rg(dest, d)], vec![rg(source, s)], None, false)
    };
    // cvttsd2si %xmm0,%rax; cvtsi2sd %rax,%xmm0; cvtss2sd %xmm1,%xmm0
    assert_eq!(bytes(&conv("cvttsd2si", RegId::RAX, 8, x(0), 8)), [0xf2, 0x48, 0x0f, 0x2c, 0xc0]);
    assert_eq!(bytes(&conv("cvtsi2sd", x(0), 8, RegId::RAX, 8)), [0xf2, 0x48, 0x0f, 0x2a, 0xc0]);
    assert_eq!(bytes(&conv("cvtss2sd", x(0), 8, x(1), 4)), [0xf3, 0x0f, 0x5a, 0xc1]);
    // ucomisd %xmm1,%xmm0 (two sources, no destination); xorps %xmm0,%xmm0
    let compare = sem(Operation::Compare, Some("ucomisd"), vec![], vec![rg(x(0), 8), rg(x(1), 8)], None, false);
    assert_eq!(bytes(&compare), [0x66, 0x0f, 0x2e, 0xc1]);
    let xor = sem(Operation::Binary, Some("xorps"), vec![rg(x(0), 16)], vec![rg(x(0), 16), rg(x(0), 16)], None, false);
    assert_eq!(bytes(&xor), [0x0f, 0x57, 0xc0]);
}

/// The allocator names a copy between xmm registers `mov`, which has no xmm
/// form: `printf("%f", a + b)` stopped at encode on `mov xmm0, xmm2`. It is
/// `movaps` (`X86InstrInfo::copyPhysReg`).
#[test]
fn a_copy_between_xmm_registers_is_movaps() {
    let x = |n: u8| RegId::from(Register::XMM0 + u32::from(n));
    let copy = sem(Operation::Move, Some("mov"), vec![rg(x(0), 16)], vec![rg(x(2), 16)], None, false);
    assert_eq!(bytes(&copy), [0x0f, 0x28, 0xc2]);
}

/// A spill of a `double` to its 8-byte slot was `movaps`, which writes 16
/// bytes and faults on a slot that is not 16-aligned. It is `movsd`/`movss`
/// (`MOVSDmr`, `MOVSSmr`), as the pin's spill of `FR64`/`FR32`.
#[test]
fn an_xmm_spill_and_reload_move_the_scalar_not_the_register() {
    let x = RegId::from(Register::XMM0);
    let store = sem(Operation::Move, Some("mov"), vec![frame(-8, 8)], vec![rg(x, 16)], None, false);
    assert_eq!(bytes(&store), [0xf2, 0x0f, 0x11, 0x45, 0xf8]);
    let load = sem(Operation::Move, Some("mov"), vec![rg(x, 16)], vec![frame(-4, 4)], None, false);
    assert_eq!(bytes(&load), [0xf3, 0x0f, 0x10, 0x45, 0xfc]);
}

/// A `double` negated or cleared is `xorps` over a value of 8 bytes: the
/// encoder looked for `Xorps_xmm_xmmm64` and found no form.
#[test]
fn a_bitwise_xmm_operation_on_a_scalar_is_the_whole_register_form() {
    let x = RegId::from(Register::XMM0);
    let xor = sem(Operation::Binary, Some("xorps"), vec![rg(x, 8)], vec![rg(x, 8), rg(x, 8)], None, false);
    assert_eq!(bytes(&xor), [0x0f, 0x57, 0xc0]);
}

#[test]
fn a_scalar_held_sixteen_bytes_wide_reads_what_its_mnemonic_says() {
    // The xmm class holds a double in 16 bytes; addsd still reads 64 bits (F2
    // 0F 58), not xmmm128 (no such form).
    let x = |n: u32| RegId::from(Register::XMM0 + n);
    let add = sem(Operation::Binary, Some("addsd"), vec![rg(x(0), 16)], vec![rg(x(0), 16), rg(x(1), 16)], None, false);
    assert_eq!(bytes(&add), [0xf2, 0x0f, 0x58, 0xc1]);
    let widen = sem(Operation::Extend, Some("cvtss2sd"), vec![rg(x(0), 16)], vec![rg(x(1), 16)], None, false);
    assert_eq!(bytes(&widen), [0xf3, 0x0f, 0x5a, 0xc1]);
}

#[test]
fn the_scalar_double_move_is_not_the_string_copy() {
    // movsd %xmm1,%xmm0 is F2 0F 10 C1; the name `movsd` is the string copy's,
    // so the move is `movsd_x`.
    let x = |n: u32| RegId::from(Register::XMM0 + n);
    let mov = sem(Operation::Move, Some("movsd_x"), vec![rg(x(0), 16)], vec![rg(x(1), 16)], None, false);
    assert_eq!(bytes(&mov), [0xf2, 0x0f, 0x10, 0xc1]);
}

/// `movq rax, xmm0` was `Semantics(... name='movq' ...)` with no encoding,
/// so an object file of a float/integer bitcast (`fconst`) failed.
#[test]
fn a_move_between_an_integer_and_an_xmm_register_encodes() {
    let x = RegId::from(Register::XMM0);
    let to_xmm = sem(Operation::Extend, Some("movq_x"), vec![rg(x, 16)], vec![rg(RegId::RDI, 8)], None, false);
    assert_eq!(bytes(&to_xmm), [0x66, 0x48, 0x0f, 0x6e, 0xc7]);
    let from_xmm = sem(Operation::Extend, Some("movd"), vec![rg(RegId::EAX, 4)], vec![rg(x, 16)], None, false);
    assert_eq!(bytes(&from_xmm), [0x66, 0x0f, 0x7e, 0xc0]);
}

/// `cvttss2si rax, [slot]` had no encoding: an object file of a float read
/// from the frame and converted (`fconv`) failed.
#[test]
fn a_float_to_integer_convert_reads_a_frame_cell() {
    let load = sem(Operation::Extend, Some("cvttss2si"), vec![rg(RegId::RAX, 8)], vec![frame(-4, 4)], None, false);
    assert_eq!(bytes(&load), [0xf3, 0x48, 0x0f, 0x2c, 0x45, 0xfc]);
}

/// `cvtsi2ss` from memory was looked up as `CVTSI2SS_XMM_XMMM32` (the
/// xmm-source form) and not encoded: `int_compared_with_float.c` and
/// `wide_frame.c` were refused at m64.
#[test]
fn an_integer_to_float_convert_reads_its_source_from_memory() {
    let from = |width| {
        let cell = ir::Mem { through: RegId::RDI, ..ir::Mem::new(None, width) };
        sem(
            Operation::Extend,
            Some("cvtsi2ss"),
            vec![rg(RegId::from(Register::XMM0), 16)],
            vec![Loc::Mem(cell)],
            None,
            false,
        )
    };
    // cvtsi2ss (%rdi),%xmm0 and cvtsi2ssq (%rdi),%xmm0
    assert_eq!(bytes(&from(4)), [0xf3, 0x0f, 0x2a, 0x07]);
    assert_eq!(bytes(&from(8)), [0xf3, 0x48, 0x0f, 0x2a, 0x07]);
}

/// `btc`, `bts` and `btr` of a quadword register and a bit number had no form:
/// a mask of one high bit at `-Os` (`x ^ 1<<62`) stopped `--isel=dag` on
/// `BTC64ri8` (sat_builtins.c).
#[test]
fn a_quadword_bit_change_of_a_constant_bit_encodes() {
    for (name, expected) in [
        ("btc", [0x48, 0x0f, 0xba, 0xff, 0x3e]),
        ("bts", [0x48, 0x0f, 0xba, 0xef, 0x3e]),
        ("btr", [0x48, 0x0f, 0xba, 0xf7, 0x3e]),
    ] {
        let what = sem(
            Operation::Binary,
            Some(name),
            vec![rg(RegId::RDI, 8)],
            vec![rg(RegId::RDI, 8), im(62, 1, None)],
            None,
            false,
        );
        assert_eq!(bytes(&what), expected, "{name}");
    }
}
