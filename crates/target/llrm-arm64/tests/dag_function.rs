//! A function from IR through the DAG path with the AArch64 table.

use llrm_arm64::dag::function::{Selected, select_function};
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_dagisel::generated::aarch64::aarch64_apple_m1::MACHINE_OPCODES;

const DATALAYOUT: &str = "e-m:o-i64:64-i128:128-n32:64-S128";

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

fn selected(body: &str) -> Result<Vec<Vec<&'static str>>, String> {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    Ok(select_function(&module, "f")?.iter().map(names).collect())
}

/// The pin's finalize-isel for `f(a, b) = a + b` is `ADDWrr` and `RET_ReallyLR`
/// (`COPY`s of `w0`, `w1` and to `w0` are not machine opcodes of the table).
#[test]
fn an_add_of_two_arguments_is_addwrr_and_ret() {
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["ADDWrr", "RET_ReallyLR"]]);
}

/// The pin's `llc -stop-after=finalize-isel` selects `sub i32` as `SUBSWrr`
/// (implicit-def dead $nzcv), not `SUBWrr`: the expected name comes from the pin.
#[test]
fn i64_add_and_i32_sub_select_as_the_pin_does() {
    let ir = "define i64 @f(i64 %a, i64 %b) {\n  %r = add i64 %a, %b\n  ret i64 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["ADDXrr", "RET_ReallyLR"]]);
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %r = sub i32 %a, %b\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["SUBSWrr", "RET_ReallyLR"]]);
}

/// More than eight integer arguments would need stack slots, which the
/// convention does not translate: the block is refused, not mis-placed.
#[test]
fn a_ninth_argument_is_refused() {
    let params = (0..9).map(|i| format!("i32 %a{i}")).collect::<Vec<_>>().join(", ");
    let ir = format!("define i32 @f({params}) {{\n  ret i32 %a8\n}}");
    let why = selected(&ir).expect_err("refused");
    assert!(why.contains("x0..x7"), "{why}");
}

/// `AArch64TargetLowering` makes `setcc` Custom, which this slice does not
/// translate. `NoHooks` would silently expand it; the block is refused instead.
#[test]
fn a_custom_operation_is_refused_not_expanded() {
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %c = icmp slt i32 %a, %b\n  %r = select i1 %c, i32 %a, i32 %b\n  ret i32 %r\n}";
    let why = selected(ir).expect_err("refused");
    assert!(why.contains("custom lowering"), "{why}");
}

mod emitted {
    use llrm_arm64::dag::emit::emit;
    use llrm_arm64::dag::function::select_function;
    use llrm_arm64::mc::inst::{Inst, Width};
    use llrm_arm64::mc::visit::Reg;
    use llrm_arm64::mfunc::Op;

    use super::DATALAYOUT;

    fn machine(body: &str) -> llrm_arm64::mfunc::MFunc {
        let text = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
        let module = llrm_mir::parse::module(&text).expect("parses");
        emit("f", &select_function(&module, "f").expect("selects")).expect("emits")
    }

    /// `f(a, b) = a + b`: both arguments copied out of `w0`/`w1`, one `add`,
    /// the result copied into `w0`, a return reading `w0`. The hand selector
    /// makes the same ops (`isel/call.rs` `copy_in`, `ret`).
    #[test]
    fn an_add_is_two_live_in_copies_an_add_a_result_copy_and_a_return() {
        let mf = machine("define i32 @f(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}");
        let ops = &mf.blocks[0].ops;
        assert_eq!(ops.len(), 5, "{ops:?}");
        assert!(matches!(&ops[0], Op::Copy { src: Reg::Gpr(g), .. } if g.0 == 0), "{ops:?}");
        assert!(matches!(&ops[1], Op::Copy { src: Reg::Gpr(g), .. } if g.0 == 1), "{ops:?}");
        assert!(matches!(&ops[2], Op::I(Inst::AddSubShifted { width: Width::W, amount: 0, .. })), "{ops:?}");
        assert!(matches!(&ops[3], Op::Copy { dst: Reg::Gpr(g), .. } if g.0 == 0), "{ops:?}");
        assert!(matches!(&ops[4], Op::Return { uses, .. } if uses.len() == 1), "{ops:?}");
    }
}
