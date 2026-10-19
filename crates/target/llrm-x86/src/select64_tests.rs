//! The emitter in 64-bit mode. Each expected byte string is what GNU as / clang's integrated assembler
//! makes of the line in its comment.

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
    // ret: it was `66 c3` (retw), which pops a word-sized return address' worth of nothing sensible.
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
    let load = |value| sem(Operation::Move, Some("mov"), vec![rg(RegId::RAX, 8)], vec![im(value, 8, None)], None, false);
    // mov $0x2a,%eax (zero-extends); mov $-1,%rax; movabs $0x123456789,%rax
    assert_eq!(bytes(&load(42)), [0xb8, 0x2a, 0, 0, 0]);
    assert_eq!(bytes(&load(-1)), [0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(bytes(&load(0x1_2345_6789)), [0x48, 0xb8, 0x89, 0x67, 0x45, 0x23, 0x01, 0, 0, 0]);
}

#[test]
fn a_quadword_add_takes_a_sign_extended_dword() {
    let add = |value| sem(Operation::Binary, Some("add"), vec![rg(RegId::RAX, 8)], vec![rg(RegId::RAX, 8), im(value, 8, None)], None, false);
    // add $0x10,%rax; add $0x1000,%rax; add $0x1000,%rbx
    assert_eq!(bytes(&add(16)), [0x48, 0x83, 0xc0, 0x10]);
    assert_eq!(bytes(&add(0x1000)), [0x48, 0x05, 0x00, 0x10, 0, 0]);
}
