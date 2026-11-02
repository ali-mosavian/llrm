//! IR to scheduled machine nodes through the shared builder and the 6502's
//! tables.

use llrm_6502::dagisel::function::{Selected, select_function};
use llrm_6502::dagisel::generated::mos_mos6502::MACHINE_OPCODES;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_codegen::backend::selectiondag::isd::ISD;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8";

fn selected(
    body: &str,
    name: &str,
) -> Vec<Vec<&'static str>> {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let blocks = select_function(&module, name, Cpu::Mos6502).expect("selects").blocks;
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

/// A byte add is one `ADD8`: the arguments are read from `rc2` and `rc3`, the
/// result goes to `a`.
#[test]
fn a_byte_add_is_one_instruction() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\n  %r = add i8 %a, %b\n  ret i8 %r\n}";
    assert_eq!(selected(ir, "f"), [["ADD8", "RET"]]);
}

/// Hand-read: a word argument is one value in a pair, the add is `ADD16`, and
/// the result leaves as two bytes. Before the target's calling hook the builder
/// joined each argument from its bytes.
#[test]
fn a_word_add_joins_and_splits_the_bytes() {
    let ir = "define i16 @f(i16 %a, i16 %b) {\n  %r = add i16 %a, %b\n  ret i16 %r\n}";
    assert_eq!(selected(ir, "f"), [["ADD16", "EXTRACT_SUBREG", "EXTRACT_SUBREG", "RET"]]);
}

/// With every argument in a register the call is `JSR` between the stack
/// adjustments. Before the call node and the pseudos were in the `.td`,
/// `CallSeqEnd` could not be selected.
#[test]
fn a_call_is_a_jsr_between_the_call_sequence_pseudos() {
    let ir = "declare i8 @g(i8, i16)\ndefine i8 @f(i8 %a, i16 %b) {\n  %r = call i8 @g(i8 %a, i16 %b)\n  ret i8 %r\n}";
    assert_eq!(selected(ir, "f"), [["ADJCALLSTACKDOWN", "JSR", "ADJCALLSTACKUP", "RET"]]);
}

/// `a < b` unsigned: `CMP8`, then the branch to the false block on carry set
/// and a jump to the true one. Before `br` had a pattern the unconditional
/// branch could not be selected.
#[test]
fn an_unsigned_compare_branches_on_carry() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\nentry:\n  %c = icmp ult i8 %a, %b\n  br i1 %c, label %t, label %f\nt:\n  ret i8 1\nf:\n  ret i8 2\n}";
    assert_eq!(selected(ir, "f"), [vec!["CMP8", "BCS", "JMP"], vec!["LDI8", "RET"], vec!["LDI8", "RET"]]);
}

/// Known gap (shared builder): an aggregate passed or returned by value is
/// refused while a target states its calling convention through the hook; the
/// 6502 passes aggregates by pointer.
#[test]
fn aggregates_by_value_are_refused() {
    for (ir, name) in [
        ("define i8 @f({ i8, i8 } %s) {\n  %r = extractvalue { i8, i8 } %s, 0\n  ret i8 %r\n}", "f"),
        ("define { i8, i8 } @g() {\n  ret { i8, i8 } zeroinitializer\n}", "g"),
    ] {
        let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
        let module = llrm_mir::parse::module(&text).expect("parses");
        assert!(select_function(&module, name, Cpu::Mos6502).is_err(), "@{name} should be refused");
    }
}
