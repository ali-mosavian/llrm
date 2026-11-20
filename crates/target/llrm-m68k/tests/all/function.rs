//! IR to scheduled machine nodes through the shared builder, against the pin's
//! `llc -mtriple=m68k-linux-gnu -mcpu=M68000 -relocation-model=static -O2
//! -stop-after=finalize-isel`.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_m68k::dagisel::function::{Selected, select_function};
use llrm_m68k::dagisel::generated::m68k_m68000::MACHINE_OPCODES;
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn selected(
    body: &str,
    name: &str,
) -> Vec<Vec<&'static str>> {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let blocks = select_function(&module, name, Cpu::M68000).expect("selects");
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

/// The pin: `MOV32rp` of the first argument, `ADD32dp` folding the second,
/// `RET`. Before the builder took the target's call nodes, the IR path ended
/// at x86's `RET_GLUE` and the stack pointer name `esp`, which m68k lacks.
#[test]
fn an_add_of_two_arguments_from_ir() {
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}";
    assert_eq!(selected(ir, "f"), [["MOV32rp", "ADD32dp", "RET"]]);
}

/// The first argument is the lowest in memory (`move.l (4,%sp), %d0` is `a`):
/// the pin's fixed objects are `-1` at offset 0 and `-2` at offset 4. The
/// description first said `left-to-right`, which put `a` highest.
#[test]
fn the_first_argument_is_at_the_lowest_address() {
    let text = format!(
        "target datalayout = \"{DATALAYOUT}\"\ndefine i32 @f(i32 %a, i32 %b) {{\n  %r = sub i32 %a, %b\n  ret i32 %r\n}}"
    );
    let module = llrm_mir::parse::module(&text).expect("parses");
    let blocks = select_function(&module, "f", Cpu::M68000).expect("selects");
    let offsets = &blocks[0].dag.fixed_offsets;
    assert_eq!((offsets[&-1], offsets[&-2]), (0, 4));
}

const CALLS: &str = "declare i32 @g(i32, i8)\ndeclare ptr @h(ptr)\n\
define i32 @c(i32 %a, i8 %b) {\n  %r = call i32 @g(i32 %a, i8 %b)\n  ret i32 %r\n}\n\
define ptr @p(ptr %x) {\n  %r = call ptr @h(ptr %x)\n  ret ptr %r\n}";

/// The pin: the byte argument is zero-extended by its load (`MOVZXd32p8`), both
/// arguments are stored at the stack pointer, the call is `CALLb`.
#[test]
fn a_call_stores_its_arguments_and_calls() {
    assert_eq!(
        selected(CALLS, "c"),
        [["ADJCALLSTACKDOWN", "MOVZXd32p8", "MOV32pr", "MOV32rp", "MOV32jr", "CALLb", "ADJCALLSTACKUP", "RET"]]
    );
}

/// A pointer result is read from `a0` and returned in it: the pin's `COPY $a0`
/// and `RET 0, $a0`. Machine nodes alone cannot show the register; the call
/// lowering test does.
#[test]
fn a_pointer_call_has_the_same_shape() {
    assert_eq!(selected(CALLS, "p"), [["ADJCALLSTACKDOWN", "MOV32rp", "MOV32jr", "CALLb", "ADJCALLSTACKUP", "RET"]]);
}

/// The loop: constants, the add with an immediate, `CMP32dr` and `Blt8` with
/// `BRA8` (the pin's `bb.1.loop`).
#[test]
fn a_counted_loop_compares_and_branches() {
    let ir = "define i32 @sum(i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [0, %entry], [%i1, %loop]\n  %s = phi i32 [0, %entry], [%s1, %loop]\n  %s1 = add i32 %s, %i\n  %i1 = add i32 %i, 1\n  %c = icmp slt i32 %i1, %n\n  br i1 %c, label %loop, label %exit\nexit:\n  ret i32 %s1\n}";
    assert_eq!(
        selected(ir, "sum"),
        [vec!["MOVI32ri", "MOV32rp"], vec!["ADD32dr", "ADD32di", "CMP32dr", "Blt8", "BRA8"], vec!["RET"]]
    );
}

/// `ret ptr` leaves in `a0` (`RetCC_M68k_C`), not `d0`: the convention names a
/// register for a pointer result and the builder reads it. Before, a pointer
/// was returned by its width, in `d0`.
#[test]
fn a_pointer_is_returned_in_a0_from_ir() {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{CALLS}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let blocks = select_function(&module, "p", Cpu::M68000).expect("selects");
    let dag = &blocks[0].dag;
    let ret = dag
        .nodes()
        .iter()
        .find(|node| {
            matches!(
                node.opcode,
                ISD::Machine(code) if MACHINE_OPCODES.iter().any(|(v, n)| u32::from(*v) == code && *n == "RET")
            )
        })
        .expect("a RET");
    let register = ret.ops.iter().map(|op| (dag.opcode(*op), dag.node(*op).data)).collect::<Vec<_>>();
    use llrm_codegen::backend::selectiondag::NodeData;
    assert!(
        register.iter().any(|(_, data)| matches!(data, NodeData::PhysReg(r) if *r == llrm_m68k::reg::A0)),
        "{register:?}"
    );
}
