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
/// (implicit-def dead $nzcv), not `SUBWrr`: the expected name comes from the
/// pin.
#[test]
fn i64_add_and_i32_sub_select_as_the_pin_does() {
    let ir = "define i64 @f(i64 %a, i64 %b) {\n  %r = add i64 %a, %b\n  ret i64 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["ADDXrr", "RET_ReallyLR"]]);
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %r = sub i32 %a, %b\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["SUBSWrr", "RET_ReallyLR"]]);
}

/// The ninth integer argument and a byte after it are loads from the caller's
/// outgoing area (`LDRWui`, `LDRBBui` of fixed objects), as in the pin; a
/// float argument is refused, not mis-placed.
#[test]
fn stack_arguments_are_loads_and_a_vector_is_refused() {
    let params = (0..9).map(|i| format!("i32 %a{i}")).chain(["i8 %b".to_owned()]).collect::<Vec<_>>().join(", ");
    let ir = format!("define i32 @f({params}) {{\n  %z = zext i8 %b to i32\n  %r = add i32 %a8, %z\n  ret i32 %r\n}}");
    assert_eq!(selected(&ir).unwrap(), [["LDRBBui", "LDRWui", "ADDWrr", "RET_ReallyLR"]]);
    let why = selected("define <4 x i32> @f(<4 x i32> %a) {\n  ret <4 x i32> %a\n}").expect_err("refused");
    assert!(why.contains("vector"), "{why}");
}

/// Calls: `ADJCALLSTACKDOWN`, the stack stores, `BL` or `BLR`,
/// `ADJCALLSTACKUP`; names are the pin's `-stop-after=finalize-isel` for the
/// same IR.
#[test]
fn calls_select_adjcallstack_bl_and_blr() {
    let decls = "declare i32 @h(i32, i32)\ndeclare i64 @many(i64, i64, i64, i64, i64, i64, i64, i64, i64, i8, i32)\n";
    let one = |body: &str| selected(&format!("{decls}{body}")).unwrap();
    let direct = "define i32 @f(i32 %a, i32 %b) {\n  %r = call i32 @h(i32 %a, i32 %b)\n  ret i32 %r\n}";
    assert_eq!(one(direct), [["ADJCALLSTACKDOWN", "BL", "ADJCALLSTACKUP", "RET_ReallyLR"]]);
    let indirect = "define void @f(ptr %p) {\n  call void %p()\n  ret void\n}";
    assert_eq!(one(indirect), [["ADJCALLSTACKDOWN", "BLR", "ADJCALLSTACKUP", "RET_ReallyLR"]]);
    let stacked = "define i64 @f(i64 %a) {\n  %r = call i64 @many(i64 %a, i64 %a, i64 %a, i64 %a, i64 %a, i64 %a, i64 %a, i64 %a, i64 %a, i8 3, i32 4)\n  ret i64 %r\n}";
    let mut got = one(stacked).remove(0);
    got.sort_unstable();
    assert_eq!(
        got,
        [
            "ADJCALLSTACKDOWN",
            "ADJCALLSTACKUP",
            "BL",
            "MOVi32imm",
            "MOVi32imm",
            "RET_ReallyLR",
            "STRBBui",
            "STRWui",
            "STRXui"
        ]
    );
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
    let one =
        |ret: &str, params: &str, body: &str| selected(&format!("define {ret} @f({params}) {{\n{body}\n}}")).unwrap();
    let zext =
        |pred: &str, rhs: &str| format!("  %c = icmp {pred} i32 %a, {rhs}\n  %r = zext i1 %c to i32\n  ret i32 %r");
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

/// Logic, shifts, multiply, divide and the integer extensions select the pin's
/// opcodes; names are its `-stop-after=finalize-isel` for the same IR.
#[test]
fn integer_alu_selects_the_pins_opcodes() {
    let cases: &[(&str, &str, &str, &[&str])] = &[
        ("i32", "i32 %a, i32 %b", "and i32 %a, %b", &["ANDWrr"]),
        ("i32", "i32 %a", "and i32 %a, 255", &["ANDWri"]),
        ("i32", "i32 %a, i32 %b", "or i32 %a, %b", &["ORRWrr"]),
        ("i64", "i64 %a", "xor i64 %a, 65280", &["EORXri"]),
        ("i32", "i32 %a", "xor i32 %a, -1", &["ORNWrr"]),
        ("i32", "i32 %a, i32 %b", "shl i32 %a, %b", &["LSLVWr"]),
        ("i32", "i32 %a", "shl i32 %a, 3", &["UBFMWri"]),
        ("i64", "i64 %a", "lshr i64 %a, 5", &["UBFMXri"]),
        ("i32", "i32 %a, i32 %b", "ashr i32 %a, %b", &["ASRVWr"]),
        ("i32", "i32 %a, i32 %b", "mul i32 %a, %b", &["MADDWrrr"]),
        ("i32", "i32 %a, i32 %b", "sdiv i32 %a, %b", &["SDIVWr"]),
        ("i64", "i64 %a, i64 %b", "udiv i64 %a, %b", &["UDIVXr"]),
        ("i32", "i32 %a, i32 %b", "srem i32 %a, %b", &["SDIVWr", "MADDWrrr", "SUBSWrr"]),
        ("i64", "i32 %a", "sext i32 %a to i64", &["IMPLICIT_DEF", "INSERT_SUBREG", "SBFMXri"]),
        ("i64", "i32 %a", "zext i32 %a to i64", &["ORRWrs", "SUBREG_TO_REG"]),
        ("i32", "i8 %a", "zext i8 %a to i32", &["ANDWri"]),
        ("i32", "i16 %a", "sext i16 %a to i32", &["SBFMWri"]),
        ("i32", "i32 %a", "sub i32 0, %a", &["SUBSWrr"]),
        ("i32", "i32 %a, i32 %b, i32 %c", "add i32 %a, %c", &["ADDWrr"]),
    ];
    for &(ret, params, op, want) in cases {
        let ir = format!("define {ret} @f({params}) {{\n  %r = {op}\n  ret {ret} %r\n}}");
        let mut expected: Vec<&str> = want.to_vec();
        expected.push("RET_ReallyLR");
        assert_eq!(selected(&ir).unwrap_or_else(|why| panic!("{op}: {why}")), [expected], "{op}");
    }
}

/// Darwin passes every variadic argument on the stack, widened to 8 bytes: the
/// pin's `-stop-after=finalize-isel` for `printf(s, i32, float, i8 signext)`
/// has three `STR`s and no register argument but the format. The DAG path once
/// put the `i32` in `w1`, and every `printf` of a value printed zero.
#[test]
fn variadic_arguments_go_on_the_stack_as_eight_bytes() {
    let ir = "declare i32 @printf(ptr, ...)\n\
define i32 @f(ptr %s, i32 %x, float %y, i8 %z) {\n  %r = call i32 (ptr, ...) @printf(ptr %s, i32 %x, float %y, i8 signext %z)\n  ret i32 %r\n}";
    let mut got = selected(ir).unwrap().remove(0);
    got.retain(|name| !matches!(*name, "IMPLICIT_DEF" | "INSERT_SUBREG"));
    got.sort_unstable();
    assert_eq!(
        got,
        [
            "ADJCALLSTACKDOWN",
            "ADJCALLSTACKUP",
            "BL",
            "FCVTDSr",
            "RET_ReallyLR",
            "SBFMXri",
            "STRDui",
            "STRXui",
            "STRXui"
        ]
    );
}

/// An `sret` parameter is in `x8`, not `x0`; a `byval` one is in memory. The
/// hook placed both as plain pointers, so `scale(sret %0, ptr %1, i64)` read
/// its source from `x0` while the caller passed the result slot in `x8`. They
/// are refused.
#[test]
fn sret_and_byval_arguments_are_refused_not_placed_as_pointers() {
    let sret = "%t = type { i64, i64, i64 }\n\
define void @f(ptr sret(%t) %r, ptr %s) {\n  store i64 1, ptr %r\n  ret void\n}";
    assert!(selected(sret).expect_err("refused").contains("moves it"));
    let call = "%t = type { i64, i64, i64 }\ndeclare void @g(ptr sret(%t), ptr)\n\
define void @f(ptr %r, ptr %s) {\n  call void @g(ptr sret(%t) %r, ptr %s)\n  ret void\n}";
    assert!(selected(call).expect_err("refused").contains("moves it"));
}

/// A call with a `byval` argument is refused by a target whose hook does not
/// pass it by reference: the check on `byval_by_reference` alone stands between
/// such a call and a pointer passed where the bytes belong (the 6502 hook,
/// which does copy, places `byval` through `places_attribute`).
#[test]
fn a_byval_call_is_refused_by_a_target_without_by_reference() {
    let call = "%t = type { i64, i64, i64 }\ndeclare void @g(ptr byval(%t))\n\
define void @f(ptr %s) {\n  call void @g(ptr byval(%t) %s)\n  ret void\n}";
    assert!(selected(call).expect_err("refused").contains("byval"));
}

/// Scalar floats: arithmetic, compares, conversions, loads and stores select
/// the pin's opcodes; names are its `-stop-after=finalize-isel` for the same
/// IR.
#[test]
fn scalar_floats_select_the_pins_opcodes() {
    let cases: &[(&str, &str, &str, &[&str])] = &[
        ("float", "float %a, float %b", "fadd float %a, %b", &["FADDSrr"]),
        ("double", "double %a, double %b", "fmul double %a, %b", &["FMULDrr"]),
        ("double", "double %a, double %b", "fsub double %a, %b", &["FSUBDrr"]),
        ("double", "double %a, double %b", "fdiv double %a, %b", &["FDIVDrr"]),
        ("double", "double %a", "fneg double %a", &["FNEGDr"]),
        ("double", "i32 %a", "sitofp i32 %a to double", &["SCVTFUWDri"]),
        ("double", "i64 %a", "uitofp i64 %a to double", &["UCVTFUXDri"]),
        ("i32", "double %a", "fptosi double %a to i32", &["FCVTZSUWDr"]),
        ("float", "double %a", "fptrunc double %a to float", &["FCVTSDr"]),
        ("double", "float %a", "fpext float %a to double", &["FCVTDSr"]),
        ("double", "ptr %p", "load double, ptr %p", &["LDRDui"]),
        // the pin's InstrEmitter turns `COPY_TO_REGCLASS` into a
        // cross-class `COPY`
        ("i32", "float %a", "bitcast float %a to i32", &["COPY_TO_REGCLASS"]),
    ];
    for &(ret, params, op, want) in cases {
        let ir = format!("define {ret} @f({params}) {{\n  %r = {op}\n  ret {ret} %r\n}}");
        let mut expected: Vec<&str> = want.to_vec();
        expected.push("RET_ReallyLR");
        assert_eq!(selected(&ir).unwrap_or_else(|why| panic!("{op}: {why}")), [expected], "{op}");
    }
    let store = "define void @f(ptr %p, float %a) {\n  store float %a, ptr %p\n  ret void\n}";
    assert_eq!(selected(store).unwrap(), [["STRSui", "RET_ReallyLR"]]);
    let compare = |pred: &str| {
        let ir = format!(
            "define i32 @f(double %a, double %b) {{\n  %c = fcmp {pred} double %a, %b\n  %r = zext i1 %c to i32\n  ret i32 %r\n}}"
        );
        selected(&ir).unwrap_or_else(|why| panic!("{pred}: {why}"))
    };
    assert_eq!(compare("olt"), [["FCMPDrr", "CSINCWr", "RET_ReallyLR"]]);
    assert_eq!(compare("one"), [["FCMPDrr", "CSINCWr", "CSINCWr", "RET_ReallyLR"]]);
    let select = "define double @f(double %a, double %b) {\n  %c = fcmp ogt double %a, %b\n  %r = select i1 %c, double %a, double %b\n  ret double %r\n}";
    assert_eq!(selected(select).unwrap(), [["FCMPDrr", "FCSELDrrr", "RET_ReallyLR"]]);
}

/// Float constants: an `fmov` immediate, zero, or two `mov`s and an `fmov`
/// are selected as the pin does; a constant that needs a pool is refused,
/// not mis-materialised.
#[test]
fn float_constants_are_fmov_immediates_or_mov_pairs_and_a_pool_is_refused() {
    let add = |ty: &str, constant: &str| {
        selected(&format!("define {ty} @f({ty} %a) {{\n  %r = fadd {ty} %a, {constant}\n  ret {ty} %r\n}}"))
    };
    assert_eq!(add("double", "4.0").unwrap(), [["FMOVDi", "FADDDrr", "RET_ReallyLR"]]);
    assert_eq!(add("double", "0.0").unwrap(), [["FMOVD0", "FADDDrr", "RET_ReallyLR"]]);
    assert_eq!(
        add("float", "0x3FB99999A0000000").unwrap(),
        [["MOVi32imm", "COPY_TO_REGCLASS", "FADDSrr", "RET_ReallyLR"]]
    );
    let why = add("double", "3.14159").expect_err("a pool constant is refused");
    assert!(why.contains("ConstantPool"), "{why}");
}

/// `memcpy` once reached the x86 `rep movs` expansion and panicked with `no
/// arm64 register rcx`, which killed the whole compile in shadow mode. A target
/// with no string instructions refuses it.
#[test]
fn memcpy_is_refused_not_expanded_with_x86_registers() {
    let ir = "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
define void @f(ptr %d, ptr %s) {\n  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 100, i1 false)\n  ret void\n}";
    let why = selected(ir).expect_err("refused");
    assert!(why.contains("mem"), "{why}");
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

    /// Real loops tested a 64-bit count with `CBZX`, a widening multiply with
    /// `UMADDLrrr`, and an alloca with lifetime markers: the emitter had no row
    /// for the first two and refused the third, so shadow mode fell back to
    /// the hand selector.
    #[test]
    fn cbz_x_umaddl_and_lifetime_markers_emit() {
        machine(
            "define i64 @f(i64 %a) {\nentry:\n  %c = icmp eq i64 %a, 0\n  br i1 %c, label %x, label %y\nx:\n  ret i64 1\ny:\n  ret i64 2\n}",
        );
        machine(
            "define i64 @f(i32 %a, i32 %b) {\n  %x = zext i32 %a to i64\n  %y = zext i32 %b to i64\n  %r = mul i64 %x, %y\n  ret i64 %r\n}",
        );
        machine(
            "declare void @llvm.lifetime.start.p0(i64, ptr)\ndeclare void @llvm.lifetime.end.p0(i64, ptr)\n\
define i32 @f(i32 %a) {\n  %p = alloca i32\n  call void @llvm.lifetime.start.p0(i64 4, ptr %p)\n  store i32 %a, ptr %p\n  %v = load i32, ptr %p\n  call void @llvm.lifetime.end.p0(i64 4, ptr %p)\n  ret i32 %v\n}",
        );
    }
}
