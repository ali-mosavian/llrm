//! IR to scheduled machine nodes through the shared builder and the Z80's
//! tables.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_z80::dagisel::function::{Selected, select_function};
use llrm_z80::dagisel::generated::z80_z80::MACHINE_OPCODES;
use llrm_z80::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16";

fn selected(
    body: &str,
    name: &str,
) -> Vec<Vec<&'static str>> {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let blocks = select_function(&module, name, Cpu::Z80).expect("selects").blocks;
    blocks.iter().map(names).collect()
}

/// The machine nodes in order. A unit lists its glued nodes last first.
fn names(one: &Selected) -> Vec<&'static str> {
    one.units
        .iter()
        .flat_map(|unit| unit.nodes.iter().rev().copied())
        .filter_map(|node| match one.dag.nodes()[node as usize].opcode {
            ISD::Machine(code) => MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map(|(_, name)| *name),
            _ => None,
        })
        .collect()
}

#[test]
fn a_byte_add_is_one_add_on_a() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\n  %r = add i8 %a, %b\n  ret i8 %r\n}";
    assert_eq!(selected(ir, "f"), [["ADD8r", "RET"]]);
}

#[test]
fn a_word_add_is_add_hl() {
    let ir = "define i16 @f(i16 %a, i16 %b) {\n  %r = add i16 %a, %b\n  ret i16 %r\n}";
    let blocks = selected(ir, "f");
    assert!(blocks[0].contains(&"ADD16"), "{blocks:?}");
}

/// With every argument in a register the call is `CALL` between the stack
/// adjustments. Before the call node and the pseudos were in the `.td`,
/// `CallSeqEnd` could not be selected.
#[test]
fn a_call_is_a_call_between_the_call_sequence_pseudos() {
    let ir = "declare i8 @g(i8, i16)\ndefine i8 @f(i8 %a, i16 %b) {\n  %r = call i8 @g(i8 %a, i16 %b)\n  ret i8 %r\n}";
    assert_eq!(selected(ir, "f"), [["ADJCALLSTACKDOWN", "CALL", "ADJCALLSTACKUP", "RET"]]);
}

/// `a < b` unsigned: `CP8r`, then a jump to the false block on no carry and a
/// jump to the true one.
#[test]
fn an_unsigned_compare_branches_on_carry() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\nentry:\n  %c = icmp ult i8 %a, %b\n  br i1 %c, label %t, label %f\nt:\n  ret i8 1\nf:\n  ret i8 2\n}";
    let blocks = selected(ir, "f");
    assert_eq!(blocks[0], ["CP8r", "JPNC", "JP"]);
}

/// Word logic is one pseudo, expanded to the two bytes' ops when emitted.
/// Before it, `and i16` was a custom lowering whose own `or` join hung the
/// legalizer.
#[test]
fn a_word_and_is_one_word_pseudo() {
    let ir = "define i16 @f(i16 %a, i16 %b) {\n  %r = and i16 %a, %b\n  ret i16 %r\n}";
    let blocks = selected(ir, "f");
    assert!(blocks[0].contains(&"AND16"), "{blocks:?}");
}

/// Two bytes joined into a word are the two halves of one pair, not an `or` of
/// extensions.
#[test]
fn two_bytes_joined_are_a_pair_not_an_or() {
    let ir = "define i16 @f(i8 %a, i8 %b, i16 %w) {\n  %x = zext i8 %a to i16\n  %y = zext i8 %b to i16\n  %h = shl i16 %y, 8\n  %r = or i16 %x, %h\n  %s = add i16 %r, %w\n  ret i16 %s\n}";
    let blocks = selected(ir, "f");
    assert!(blocks[0].contains(&"REG_SEQUENCE") && !blocks[0].contains(&"OR16"), "{blocks:?}");
}

/// `max` is a compare and a conditional move, byte or word. Before the
/// `SETCC`/`CMOV` nodes, `max8` and `max16` refused with
/// `CannotSelect(Select)`.
#[test]
fn a_select_on_a_compare_is_a_conditional_move() {
    let byte =
        "define i8 @f(i8 %a, i8 %b) {\n  %c = icmp ugt i8 %a, %b\n  %r = select i1 %c, i8 %a, i8 %b\n  ret i8 %r\n}";
    assert_eq!(selected(byte, "f"), [["CP8r", "CMOV8", "RET"]]);
    let word = "define i16 @g(i16 %a, i16 %b) {\n  %c = icmp ult i16 %a, %b\n  %r = select i1 %c, i16 %a, i16 %b\n  ret i16 %r\n}";
    assert!(selected(word, "g")[0].contains(&"CMOV16"));
}

/// A signed compare is the unsigned one on sign-flipped operands: an `xor 0x80`
/// on each side.
#[test]
fn a_signed_byte_compare_flips_both_sign_bits() {
    let ir =
        "define i8 @f(i8 %a, i8 %b) {\n  %c = icmp slt i8 %a, %b\n  %r = select i1 %c, i8 %a, i8 %b\n  ret i8 %r\n}";
    let names = &selected(ir, "f")[0];
    assert_eq!(names.iter().filter(|name| **name == "XOR8i").count(), 2, "{names:?}");
}

/// A byte read and widened, and a word read, through a pointer: the byte load
/// with a zero high byte, and the word load. Before them `scmp`, `crc16` and
/// `sum16` of the bench refused with `CannotSelect(Load)`.
#[test]
fn loads_through_a_pointer_widen_a_byte_and_read_a_word() {
    let widened = "define i16 @f(ptr %p) {\n  %b = load i8, ptr %p\n  %w = zext i8 %b to i16\n  ret i16 %w\n}";
    let blocks = selected(widened, "f");
    assert_eq!(blocks, [["LD8rm", "LD8ri", "RET"]]);
    let word = "define i16 @g(ptr %p) {\n  %w = load i16, ptr %p\n  ret i16 %w\n}";
    assert!(selected(word, "g")[0].contains(&"LD16rm"));
}

/// Known gap: an argument beyond the registers is refused, not misplaced (the
/// builder's hook cannot return an error, so it panics).
#[test]
#[should_panic(expected = "Stack")]
fn a_fourth_word_argument_is_refused_until_pushes_exist() {
    let ir = "define i16 @f(i16 %a, i16 %b, i16 %c, i16 %d) {\n  ret i16 %d\n}";
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let _ = select_function(&module, "f", Cpu::Z80);
}
