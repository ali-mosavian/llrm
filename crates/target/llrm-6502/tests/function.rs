//! IR to scheduled machine nodes through the shared builder and the 6502's tables.

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
    let blocks = select_function(&module, name, Cpu::Mos6502).expect("selects");
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

/// A byte add is one `ADD8`: the arguments are read from `rc2` and `rc3`, the result goes to `a`.
#[test]
fn a_byte_add_is_one_instruction() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\n  %r = add i8 %a, %b\n  ret i8 %r\n}";
    assert_eq!(selected(ir, "f"), [["ADD8", "RET"]]);
}

/// Hand-read: each word argument is its two bytes joined into a pair (`REG_SEQUENCE`), the add is
/// `ADD16`, and the result leaves as two bytes. Before the join pattern each argument cost a
/// zero-extend, a `SHL16` by 8 and an `OR16` (eight shift steps in the emitted code).
#[test]
fn a_word_add_joins_and_splits_the_bytes() {
    let ir = "define i16 @f(i16 %a, i16 %b) {\n  %r = add i16 %a, %b\n  ret i16 %r\n}";
    assert_eq!(
        selected(ir, "f"),
        [["REG_SEQUENCE", "REG_SEQUENCE", "ADD16", "EXTRACT_SUBREG", "EXTRACT_SUBREG", "RET"]]
    );
}

/// With every argument in a register the call is `JSR` between the stack adjustments. Before the
/// call node and the pseudos were in the `.td`, `CallSeqEnd` could not be selected.
#[test]
fn a_call_is_a_jsr_between_the_call_sequence_pseudos() {
    let ir = "declare i8 @g(i8, i16)\ndefine i8 @f(i8 %a, i16 %b) {\n  %r = call i8 @g(i8 %a, i16 %b)\n  ret i8 %r\n}";
    assert_eq!(selected(ir, "f"), [["ADJCALLSTACKDOWN", "JSR", "ADJCALLSTACKUP", "RET"]]);
}

/// `a < b` unsigned: `CMP8`, then the branch to the false block on carry set and a jump to the true
/// one. Before `br` had a pattern the unconditional branch could not be selected.
#[test]
fn an_unsigned_compare_branches_on_carry() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\nentry:\n  %c = icmp ult i8 %a, %b\n  br i1 %c, label %t, label %f\nt:\n  ret i8 1\nf:\n  ret i8 2\n}";
    assert_eq!(selected(ir, "f"), [vec!["CMP8", "BCS", "JMP"], vec!["LDI8", "RET"], vec!["LDI8", "RET"]]);
}

/// Known gap (plan row S2): the shared builder returns a value in the registers the way x86 does,
/// so a long (A:X:rc2:rc3) is refused until the builder calls the target's `lower_return`.
#[test]
fn a_long_result_is_refused_until_the_builder_calls_the_target() {
    let text = format!(
        "target datalayout = \"{DATALAYOUT}\"\ndefine i32 @f(i32 %a, i32 %b) {{\n  %r = add i32 %a, %b\n  ret i32 %r\n}}"
    );
    let module = llrm_mir::parse::module(&text).expect("parses");
    let refused = select_function(&module, "f", Cpu::Mos6502).err().expect("refused");
    assert_eq!(refused, "result in registers the value does not fill");
}
