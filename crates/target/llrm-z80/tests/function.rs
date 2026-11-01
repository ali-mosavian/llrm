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

/// Word logic is the bytes' ops joined (no 16-bit `and` exists).
#[test]
fn a_word_and_is_two_byte_ands() {
    let ir = "define i16 @f(i16 %a, i16 %b) {\n  %r = and i16 %a, %b\n  ret i16 %r\n}";
    let blocks = selected(ir, "f");
    assert_eq!(blocks[0].iter().filter(|name| **name == "AND8r").count(), 2, "{blocks:?}");
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
