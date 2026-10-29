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

/// `AArch64TargetLowering` makes `ctpop` Custom, which is not translated.
/// `NoHooks` would silently expand it; the block is refused instead.
#[test]
fn a_custom_operation_is_refused_not_expanded() {
    let ir = "declare i32 @llvm.ctpop.i32(i32)\ndefine i32 @f(i32 %a) {\n  %r = call i32 @llvm.ctpop.i32(i32 %a)\n  ret i32 %r\n}";
    let why = selected(ir).expect_err("refused");
    assert!(why.contains("custom lowering"), "{why}");
}

/// Loads and stores select the pin's addressing forms: scaled offset (`ui`),
/// unscaled (`UR`) and register offset (`roX`); names are the pin's
/// `-stop-after=finalize-isel`.
#[test]
fn loads_and_stores_select_the_pins_addressing_forms() {
    let one = |ret: &str, params: &str, body: &str| {
        let ir = format!("define {ret} @f({params}) {{\n{body}\n}}");
        selected(&ir).unwrap()
    };
    let load = |name: &'static str| [vec![name, "RET_ReallyLR"]];
    assert_eq!(one("i32", "ptr %p", "  %r = load i32, ptr %p\n  ret i32 %r"), load("LDRWui"));
    let gep = "  %q = getelementptr i64, ptr %p, i64 3\n  %r = load i64, ptr %q\n  ret i64 %r";
    assert_eq!(one("i64", "ptr %p", gep), load("LDRXui"));
    let gep = "  %q = getelementptr i8, ptr %p, i64 -9\n  %r = load i64, ptr %q\n  ret i64 %r";
    assert_eq!(one("i64", "ptr %p", gep), load("LDURXi"));
    let zext = "  %v = load i8, ptr %p\n  %r = zext i8 %v to i32\n  ret i32 %r";
    assert_eq!(one("i32", "ptr %p", zext), load("LDRBBui"));
    let sext = "  %v = load i8, ptr %p\n  %r = sext i8 %v to i32\n  ret i32 %r";
    assert_eq!(one("i32", "ptr %p", sext), load("LDRSBWui"));
    let sext = "  %v = load i32, ptr %p\n  %r = sext i32 %v to i64\n  ret i64 %r";
    assert_eq!(one("i64", "ptr %p", sext), load("LDRSWui"));
    let indexed = "  %q = getelementptr i64, ptr %p, i64 %i\n  %r = load i64, ptr %q\n  ret i64 %r";
    assert_eq!(one("i64", "ptr %p, i64 %i", indexed), load("LDRXroX"));
    let store = "  store i32 %v, ptr %p\n  ret void";
    assert_eq!(one("void", "ptr %p, i32 %v", store), [vec!["STRWui", "RET_ReallyLR"]]);
    let half = "  %t = trunc i32 %v to i16\n  store i16 %t, ptr %p\n  ret void";
    assert_eq!(one("void", "ptr %p, i32 %v", half), [vec!["STRHHui", "RET_ReallyLR"]]);
}

/// Globals: `ADRP` + `LDRWui` with a page offset, `MOVaddr` for the address,
/// `LOADgot` for a symbol from another object; names are the pin's.
#[test]
fn global_addresses_select_adrp_movaddr_and_loadgot() {
    let head = "@g = global [4 x i32] zeroinitializer\n@e = external global i32\n";
    let one = |ret: &str, body: &str| selected(&format!("{head}define {ret} @f() {{\n{body}\n}}")).unwrap();
    assert_eq!(one("i32", "  %v = load i32, ptr @g\n  ret i32 %v"), [["ADRP", "LDRWui", "RET_ReallyLR"]]);
    assert_eq!(one("ptr", "  ret ptr @g"), [["MOVaddr", "RET_ReallyLR"]]);
    assert_eq!(one("i32", "  %v = load i32, ptr @e\n  ret i32 %v"), [["LOADgot", "LDRWui", "RET_ReallyLR"]]);
}

/// Integer compares and selects: `SUBS`/`ADDS` and `CSEL`/`CSINC` on the flags,
/// as the pin's `-stop-after=finalize-isel` has them for the same IR.
#[test]
fn integer_compares_and_selects_select_subs_and_csel() {
    let one = |ret: &str, params: &str, body: &str| {
        selected(&format!("define {ret} @f({params}) {{\n{body}\n}}")).unwrap()
    };
    let zext = |pred: &str, rhs: &str| format!("  %c = icmp {pred} i32 %a, {rhs}\n  %r = zext i1 %c to i32\n  ret i32 %r");
    let want = |first: &'static str, second: &'static str| [vec![first, second, "RET_ReallyLR"]];
    assert_eq!(one("i32", "i32 %a, i32 %b", &zext("eq", "%b")), want("SUBSWrr", "CSINCWr"));
    assert_eq!(one("i32", "i32 %a", &zext("ugt", "5")), want("SUBSWri", "CSINCWr"));
    assert_eq!(one("i32", "i32 %a", &zext("slt", "4097")), want("SUBSWri", "CSINCWr"));
    assert_eq!(one("i32", "i32 %a", &zext("eq", "-5")), want("ADDSWri", "CSINCWr"));
    let select = "  %c = icmp slt i32 %a, %b\n  %r = select i1 %c, i32 %a, i32 %b\n  ret i32 %r";
    assert_eq!(one("i32", "i32 %a, i32 %b", select), want("SUBSWrr", "CSELWr"));
    let wide = "  %p = icmp sge i64 %a, 0\n  %r = select i1 %p, i64 %b, i64 %c\n  ret i64 %r";
    assert_eq!(one("i64", "i64 %a, i64 %b, i64 %c", wide), want("ADDSXri", "CSELXr"));
}

/// Loops: the exit test is a `CBNZ`, a `TBNZ` of the sign or a masked bit, or
/// flags and `Bcc`; names per block are the pin's `-stop-after=finalize-isel`.
#[test]
fn loop_exits_select_cbnz_tbnz_and_bcc() {
    let looped = |step: &str, test: &str| {
        let ir = format!(
            "define i32 @f(i32 %n) {{\nentry:\n  br label %h\nh:\n  %i = phi i32 [%n, %entry], [%j, %h]\n  {step}\n{test}\n  br i1 %c, label %h, label %x\nx:\n  ret i32 %j\n}}"
        );
        selected(&ir).unwrap()
    };
    let cbnz = looped("%j = sub i32 %i, 3", "  %c = icmp ne i32 %j, 0");
    assert_eq!(cbnz, [vec![], vec!["SUBSWri", "CBNZW", "B"], vec!["RET_ReallyLR"]]);
    let tbnz = looped("%j = add i32 %i, 7", "  %m = and i32 %j, 4\n  %c = icmp ne i32 %m, 0");
    assert_eq!(tbnz, [vec![], vec!["ADDWri", "TBNZW", "B"], vec!["RET_ReallyLR"]]);
    let sign = looped("%j = sub i32 %i, 7", "  %c = icmp slt i32 %j, 0");
    assert_eq!(sign, [vec![], vec!["SUBSWri", "TBNZW", "B"], vec!["RET_ReallyLR"]]);
    let bcc = looped("%j = add i32 %i, 1", "  %c = icmp ult i32 %j, %n");
    assert_eq!(bcc, [vec![], vec!["ADDWri", "SUBSWrr", "Bcc", "B"], vec!["RET_ReallyLR"]]);
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
