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
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    // The pipeline sees an atomic access as a call; the builder reads it back.
    llrm_mir::atomic::hidden(&mut module).expect("hides");
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

/// An array indexed by a sign- or zero-extended `i32` was a `SBFMXri`/`ORRWrs`
/// and an `LDRWroX` (two instructions); the pin folds the extend: `LDRWroW`.
#[test]
fn an_extended_32_bit_index_is_a_register_offset_with_an_extend() {
    let load = |extend: &str| {
        format!(
            "define i32 @f(ptr %p, i32 %i) {{\n  %w = {extend} i32 %i to i64\n  %a = getelementptr i32, ptr %p, i64 %w\n  %v = load i32, ptr %a, align 4\n  ret i32 %v\n}}"
        )
    };
    assert_eq!(selected(&load("sext")).unwrap(), [["LDRWroW", "RET_ReallyLR"]]);
    assert_eq!(selected(&load("zext")).unwrap(), [["LDRWroW", "RET_ReallyLR"]]);
}

/// A branch on `a == 0 || b == 0` was one `SUBS`/`CCMP`/`Bcc` chain: the DAG path had no TTI,
/// so the pin's AArch64 `getJumpConditionMergingParams` was never asked. The pin splits it
/// into two compare-and-branches (`CBZW`, `CBNZW`).
#[test]
fn a_branch_on_two_zero_tests_is_two_compare_and_branches() {
    let ir = "declare void @g()\ndeclare void @h()\ndefine void @f(i32 %a, i32 %b) {\n  %x = icmp eq i32 %a, 0\n  %y = icmp eq i32 %b, 0\n  %o = or i1 %x, %y\n  br i1 %o, label %t, label %e\nt:\n  call void @g()\n  ret void\ne:\n  call void @h()\n  ret void\n}";
    let blocks = selected(ir).unwrap();
    assert_eq!(blocks[0], ["CBZW", "B"], "{blocks:?}");
    assert_eq!(blocks[1], ["CBNZW", "B"], "{blocks:?}");
}

/// A byte result tested against zero was `ANDSWri` and `CBZW`: the compare was lowered to flags
/// (and the `and` made flag-setting) before the branch that only wanted `CBZ` read it. The pin
/// branches on the plain `ANDWri`.
#[test]
fn a_byte_result_tested_for_zero_is_an_and_and_cbz() {
    let ir = "declare i8 @g8(ptr)\ndeclare void @one()\ndefine void @f(ptr %s) {\n  %r = call i8 @g8(ptr %s)\n  %z = icmp eq i8 %r, 0\n  br i1 %z, label %a, label %b\na:\n  call void @one()\n  ret void\nb:\n  ret void\n}";
    let blocks = selected(ir).unwrap();
    assert_eq!(blocks[0], ["ADJCALLSTACKDOWN", "BL", "ADJCALLSTACKUP", "ANDWri", "CBNZW", "B"], "{blocks:?}");
}

/// A branch on an `i1` argument whose true edge falls through was `ORNWrr` and `TBNZW`: the
/// builder's `xor c, 1` was tested for a set bit. The pin inverts the condition: `TBZW`.
#[test]
fn a_branch_on_a_flag_argument_to_the_next_block_is_a_single_tbz() {
    let ir = "declare void @u()\ndefine void @f(i1 %c) {\nentry:\n  br i1 %c, label %t, label %e\nt:\n  call void @u()\n  ret void\ne:\n  ret void\n}";
    let blocks = selected(ir).unwrap();
    assert_eq!(blocks[0], ["TBZW", "B"], "{blocks:?}");
}

/// `a == 65 && b == 66` on two bytes was `EOR`/`EOR`/`ORR` over widened loads: x86's
/// `convertSetCCLogicToBitwiseLogic` ran for AArch64. The pin keeps byte loads and a `CCMP`.
#[test]
fn two_byte_equalities_are_a_compare_and_a_conditional_compare() {
    let ir = "define i32 @f(ptr %p) {\n  %a = load i8, ptr %p, align 1\n  %c = icmp eq i8 %a, 65\n  %q = getelementptr inbounds i8, ptr %p, i64 1\n  %b = load i8, ptr %q, align 1\n  %d = icmp eq i8 %b, 66\n  %r = and i1 %c, %d\n  %z = zext i1 %r to i32\n  ret i32 %z\n}";
    assert_eq!(
        selected(ir).unwrap(),
        [["LDRBBui", "SUBSWri", "LDRBBui", "MOVi32imm", "CCMPWr", "CSINCWr", "RET_ReallyLR"]]
    );
}

/// `select c, d, false` of two byte compares is `and c, freeze(d)`; the frozen byte load stayed a
/// word load with a mask (`LDURWi`, `ANDWri`). The pin narrows it: `LDRBBui`.
#[test]
fn a_select_of_two_byte_equalities_keeps_the_byte_loads() {
    let ir = "define i32 @f(ptr %p) {\n  %a = load i8, ptr %p, align 1\n  %c = icmp eq i8 %a, 65\n  %q = getelementptr inbounds i8, ptr %p, i64 1\n  %b = load i8, ptr %q, align 1\n  %d = icmp eq i8 %b, 66\n  %r = select i1 %c, i1 %d, i1 false\n  %z = zext i1 %r to i32\n  ret i32 %z\n}";
    // the pin's opcodes; the scheduler may place the constant's `MOVi32imm` differently
    let mut ops = selected(ir).unwrap().remove(0);
    ops.sort_unstable();
    let mut want = vec!["LDRBBui", "SUBSWri", "LDRBBui", "MOVi32imm", "CCMPWr", "CSINCWr", "RET_ReallyLR"];
    want.sort_unstable();
    assert_eq!(ops, want);
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

/// Calls: `ADJCALLSTACKDOWN`, the stack stores, `BL` or `BLR`, `ADJCALLSTACKUP`;
/// names are the pin's `-stop-after=finalize-isel` for the same IR.
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
    assert_eq!(got, ["ADJCALLSTACKDOWN", "ADJCALLSTACKUP", "BL", "MOVi32imm", "MOVi32imm", "RET_ReallyLR", "STRBBui", "STRWui", "STRXui"]);
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

/// `sdiv i32 %x, 5` is the pin's widened multiply-add and shifts, not `SDIVWr`:
/// i32 has neither MULHS nor SMUL_LOHI, so BuildSDIV widens to the legal i64 mul.
/// Only i8/i16 widened, so every i32 divide by a constant cost a `SDIVWr`.
#[test]
fn sdiv_by_a_constant_widens_to_a_multiply_add() {
    let ir = "define i32 @f(i32 %a) {\n  %r = sdiv i32 %a, 5\n  ret i32 %r\n}";
    let got = selected(ir).unwrap().remove(0);
    assert!(!got.contains(&"SDIVWr") && got.contains(&"SMADDLrrr"), "{got:?}");
}

/// `(a + 3) * 5` is the pin's `a * 5 + 15`, an `ADDWrs` then `ADDWri 15`. The
/// combiner lacked visitMUL's distribution, so the add of 3 came first.
#[test]
fn a_multiply_of_an_add_with_a_constant_distributes() {
    let ir = "define i32 @f(i32 %a) {\n  %s = add i32 %a, 3\n  %r = mul i32 %s, 5\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["ADDWrs", "ADDWri", "RET_ReallyLR"]]);
}

/// `ashr (trunc (lshr x, 32)), 3` is one 64-bit `SBFMXri` as the pin's visitSRA folds
/// it. Without the fold a `UBFMXri` for the shift came first and the sign fill
/// was taken from the wrong end.
#[test]
fn an_arithmetic_shift_of_a_truncated_shift_is_one_signed_bitfield_move() {
    let ir = "define i32 @f(i64 %a) {\n  %h = lshr i64 %a, 32\n  %t = trunc i64 %h to i32\n  %r = ashr i32 %t, 3\n  ret i32 %r\n}";
    let got = selected(ir).unwrap().remove(0);
    assert!(got.contains(&"SBFMXri") && !got.contains(&"UBFMXri"), "{got:?}");
}

/// A multiply by a power of two beside an int-float conversion is one fixed-point
/// `SCVTF`/`FCVTZS` as the pin's `SelectCVTFixedPos*` selects it. The pattern
/// answered false, so each cost an `FMOVDi` and an `FMULDrr`.
#[test]
fn a_multiply_by_a_power_of_two_beside_a_conversion_is_a_fixed_point_convert() {
    let up = "define double @f(i64 %x) {\n  %d = sitofp i64 %x to double\n  %r = fmul double %d, 2.500000e-01\n  ret double %r\n}";
    assert_eq!(selected(up).unwrap(), [["SCVTFSXDri", "RET_ReallyLR"]]);
    let down = "define i64 @f(double %d) {\n  %m = fmul double %d, 1.600000e+01\n  %r = fptosi double %m to i64\n  ret i64 %r\n}";
    assert_eq!(selected(down).unwrap(), [["FCVTZSSXDri", "RET_ReallyLR"]]);
}

/// Two stores to `@a` and `@a+8` share one `MOVaddr`, as the pin's
/// `performGlobalAddressCombine` leaves them. The offset used to fold into each
/// symbol, so each store paid its own `ADRP`.
#[test]
fn stores_at_two_offsets_of_one_global_share_a_movaddr() {
    let ir = "@a = global [4 x i64] zeroinitializer, align 8\n\
define void @f() {\n  store i64 1234567890123, ptr @a, align 8\n  store i64 9876543210987, ptr getelementptr inbounds ([4 x i64], ptr @a, i64 0, i64 1), align 8\n  ret void\n}";
    assert_eq!(selected(ir).unwrap(), [["MOVaddr", "MOVi64imm", "STRXui", "MOVi64imm", "STRXui", "RET_ReallyLR"]]);
}

/// `store double 1.0` is an `i64` constant store as the pin's replaceStoreOfFPConstant
/// makes it: `MOVi64imm` and `STRXui`, not an `FMOVDi` and an `STRDui`.
#[test]
fn a_store_of_a_double_constant_is_an_integer_store() {
    let ir = "@d = global double 0.0, align 8\ndefine void @f() {\n  store double 1.0, ptr @d, align 8\n  ret void\n}";
    assert_eq!(selected(ir).unwrap(), [["ADRP", "MOVi64imm", "STRXui", "RET_ReallyLR"]]);
}

/// `shl a, (b & 31)` is one `LSLVWr`: the pin's tryShiftAmountMod drops the mask the
/// register shift applies itself. An `ANDWri` came first.
#[test]
fn a_shift_by_a_masked_amount_drops_the_mask() {
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %s = and i32 %b, 31\n  %r = shl i32 %a, %s\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["LSLVWr", "RET_ReallyLR"]]);
}

/// `a - (a >u b ? b : 0)` is `SUBS` and one `CSEL` of the difference and `a`, as the pin's
/// performConditionalSubCombine makes it. A second `SUBSWrr` for the subtract came first.
#[test]
fn a_subtract_of_a_conditional_operand_reuses_the_compare_difference() {
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %gt = icmp ugt i32 %a, %b\n  %s = select i1 %gt, i32 %b, i32 0\n  %r = sub i32 %a, %s\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["SUBSWrr", "CSELWr", "RET_ReallyLR"]]);
}

/// `(a << 1) == b` compares with the shift folded into the right operand, `SUBSWrs`, as the
/// pin's getAArch64Cmp swaps the operands to put it there. A `UBFMWri` came first.
#[test]
fn a_compare_folds_a_shifted_operand_by_swapping_sides() {
    let ir = "define i1 @f(i32 %a, i32 %b) {\n  %s = shl i32 %a, 1\n  %c = icmp eq i32 %s, %b\n  ret i1 %c\n}";
    assert_eq!(selected(ir).unwrap(), [["SUBSWrs", "CSINCWr", "RET_ReallyLR"]]);
}

/// `(x + 32768) u< 65536` is `sext_inreg x, i16 == x`, one `SUBSXrx` with an `sxth` operand, as
/// the pin's optimizeSetCCOfSignedTruncationCheck unfolds it. It stayed an add and a compare.
#[test]
fn a_signed_truncation_check_is_a_sign_extending_compare() {
    let ir = "define i1 @f(i64 %x) {\n  %a = add i64 %x, 32768\n  %c = icmp ult i64 %a, 65536\n  ret i1 %c\n}";
    assert_eq!(selected(ir).unwrap(), [["EXTRACT_SUBREG", "SUBSXrx", "CSINCWr", "RET_ReallyLR"]]);
}

/// Darwin passes every variadic argument on the stack, widened to 8 bytes: the
/// pin's `-stop-after=finalize-isel` for `printf(s, i32, float, i8 signext)` has
/// three `STR`s and no register argument but the format. The DAG path once put
/// the `i32` in `w1`, and every `printf` of a value printed zero.
#[test]
fn variadic_arguments_go_on_the_stack_as_eight_bytes() {
    let ir = "declare i32 @printf(ptr, ...)\n\
define i32 @f(ptr %s, i32 %x, float %y, i8 %z) {\n  %r = call i32 (ptr, ...) @printf(ptr %s, i32 %x, float %y, i8 signext %z)\n  ret i32 %r\n}";
    let mut got = selected(ir).unwrap().remove(0);
    got.retain(|name| !matches!(*name, "IMPLICIT_DEF" | "INSERT_SUBREG"));
    got.sort_unstable();
    assert_eq!(got, ["ADJCALLSTACKDOWN", "ADJCALLSTACKUP", "BL", "FCVTDSr", "RET_ReallyLR", "SBFMXri", "STRDui", "STRXui", "STRXui"]);
}

/// An `sret` parameter is in `x8`, not `x0`; a `byval` one is in memory. The hook
/// placed both as plain pointers, so `scale(sret %0, ptr %1, i64)` read its source
/// from `x0` while the caller passed the result slot in `x8`. They are refused.
#[test]
fn sret_and_byval_arguments_are_refused_not_placed_as_pointers() {
    let sret = "%t = type { i64, i64, i64 }\n\
define void @f(ptr sret(%t) %r, ptr %s) {\n  store i64 1, ptr %r\n  ret void\n}";
    assert!(selected(sret).expect_err("refused").contains("moves it"));
    let call = "%t = type { i64, i64, i64 }\ndeclare void @g(ptr sret(%t), ptr)\n\
define void @f(ptr %r, ptr %s) {\n  call void @g(ptr sret(%t) %r, ptr %s)\n  ret void\n}";
    assert!(selected(call).expect_err("refused").contains("moves it"));
}

/// Scalar floats: arithmetic, compares, conversions, loads and stores select the
/// pin's opcodes; names are its `-stop-after=finalize-isel` for the same IR.
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
        // the pin's InstrEmitter turns `COPY_TO_REGCLASS` into a cross-class `COPY`
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
        let ir = format!("define i32 @f(double %a, double %b) {{\n  %c = fcmp {pred} double %a, %b\n  %r = zext i1 %c to i32\n  ret i32 %r\n}}");
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
    assert_eq!(add("float", "0x3FB99999A0000000").unwrap(), [["MOVi32imm", "COPY_TO_REGCLASS", "FADDSrr", "RET_ReallyLR"]]);
    let why = add("double", "3.14159").expect_err("a pool constant is refused");
    assert!(why.contains("ConstantPool"), "{why}");
}

/// `memcpy` once reached the x86 `rep movs` expansion and panicked with `no arm64
/// register rcx`, which killed the whole compile in shadow mode. A target with no
/// string instructions refuses it.
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

    /// `performGlobalAddressCombine`: every user of `@a` adds a constant, so the smallest, 16,
    /// goes into the symbol and the stores are at 0 and 8 from one `ADRP @a+16`. Folding each
    /// offset in the builder gave two pages, `@a+16` and `@a+24`.
    #[test]
    fn a_global_used_only_at_offsets_has_the_smallest_in_its_symbol() {
        let mf = machine("@a = global [8 x i64] zeroinitializer, align 8\n\
define void @f(i64 %i) {\n  %p = getelementptr inbounds [8 x i64], ptr @a, i64 0, i64 2\n  %q = getelementptr inbounds [8 x i64], ptr @a, i64 0, i64 3\n  store i64 %i, ptr %p, align 8\n  store i64 %i, ptr %q, align 8\n  ret void\n}");
        let pages: Vec<i64> = mf.blocks[0]
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::I(Inst::Adr { page: true, target: llrm_arm64::mc::inst::Target::Symbol { addend, .. }, .. }) => Some(*addend),
                _ => None,
            })
            .collect();
        assert_eq!(pages, [16], "{:?}", mf.blocks[0].ops);
    }

    /// `LDRWroW` had no row in the emitter: a sign-extended index could not be emitted.
    #[test]
    fn a_register_offset_with_an_extend_emits() {
        machine("define i32 @f(ptr %p, i32 %i) {\n  %w = sext i32 %i to i64\n  %a = getelementptr i32, ptr %p, i64 %w\n  %v = load i32, ptr %a, align 4\n  ret i32 %v\n}");
    }

    /// Real loops tested a 64-bit count with `CBZX`, a widening multiply with
    /// `UMADDLrrr`, and an alloca with lifetime markers: the emitter had no row for
    /// the first two and refused the third, so shadow mode fell back to the hand selector.
    #[test]
    fn cbz_x_umaddl_and_lifetime_markers_emit() {
        machine("define i64 @f(i64 %a) {\nentry:\n  %c = icmp eq i64 %a, 0\n  br i1 %c, label %x, label %y\nx:\n  ret i64 1\ny:\n  ret i64 2\n}");
        machine("define i64 @f(i32 %a, i32 %b) {\n  %x = zext i32 %a to i64\n  %y = zext i32 %b to i64\n  %r = mul i64 %x, %y\n  ret i64 %r\n}");
        machine("declare void @llvm.lifetime.start.p0(i64, ptr)\ndeclare void @llvm.lifetime.end.p0(i64, ptr)\n\
define i32 @f(i32 %a) {\n  %p = alloca i32\n  call void @llvm.lifetime.start.p0(i64 4, ptr %p)\n  store i32 %a, ptr %p\n  %v = load i32, ptr %p\n  call void @llvm.lifetime.end.p0(i64 4, ptr %p)\n  ret i32 %v\n}");
    }
}

/// `a + sext(b)` is one `ADDXrx` (`add x0, x0, w1, sxtw`) in the pin. Without
/// `SelectArithExtendedRegister` it was `SBFMXri` and an `ADDXrr`.
#[test]
fn an_add_of_a_sign_extended_argument_is_addxrx() {
    let ir = "define i64 @f(i64 %a, i32 %b) {\n  %e = sext i32 %b to i64\n  %r = add i64 %a, %e\n  ret i64 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["ADDXrx", "RET_ReallyLR"]]);
}

/// `a + ((b >> 4) & 0x0ffffff0)` is `UBFMWri b, 8, 31` and `ADDWrs a, _, lsl 4`
/// in the pin (`SelectShiftedRegisterFromAnd`). Without it the shift, the mask
/// and the add were three instructions.
#[test]
fn a_masked_shift_folds_into_a_shifted_register_add() {
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %s = lshr i32 %b, 4\n  %m = and i32 %s, 268435440\n  %r = add i32 %a, %m\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["UBFMWri", "ADDWrs", "RET_ReallyLR"]]);
}

/// `mul` by a constant that is a power of two plus or minus one, optionally
/// shifted, is shifts and adds in the pin (`performMulCombine`): 5 is
/// `ADDWrs a, a, lsl 2`, 7 is `UBFMWri` and `SUBSWrr`, -7 is `SUBSWrs`, 12 is
/// `ADDWrs` and `UBFMWri`. Without the combine each was a `MOVi32imm` and a `MADDWrrr`.
#[test]
fn multiplies_by_power_of_two_neighbours_select_shifts_and_adds() {
    for (c, expected) in [
        (5, vec!["ADDWrs", "RET_ReallyLR"]),
        (7, vec!["UBFMWri", "SUBSWrr", "RET_ReallyLR"]),
        (-7, vec!["SUBSWrs", "RET_ReallyLR"]),
        (12, vec!["ADDWrs", "UBFMWri", "RET_ReallyLR"]),
    ] {
        let ir = format!("define i32 @f(i32 %a) {{\n  %r = mul i32 %a, {c}\n  ret i32 %r\n}}");
        assert_eq!(selected(&ir).unwrap(), [expected], "mul {c}");
    }
}

/// `(a & c) ^ c` is `a`'s complement masked by `c`: `MOVi32imm c` and one
/// `BICWrr` in the pin (`visitXOR`). Without the fold it was `ANDWri` and `EORWri`.
#[test]
fn xor_of_a_masked_value_with_its_mask_is_a_bic() {
    let ir = "define i32 @f(i32 %a) {\n  %m = and i32 %a, 61680\n  %r = xor i32 %m, 61680\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["MOVi32imm", "BICWrr", "RET_ReallyLR"]]);
}

/// A stack-passed `i8` sign-extended is one `LDRSBXui` in the pin
/// (`sext_inreg (extload)` is a `sextload`); without the fold it was `LDRBBui`
/// and an extended-register `ADDXrx`.
#[test]
fn a_sign_extended_stack_byte_is_one_sign_extending_load() {
    let params = (0..8).map(|i| format!("i64 %a{i}")).chain(["i8 %b".to_owned()]).collect::<Vec<_>>().join(", ");
    let ir = format!("define i64 @f({params}) {{\n  %e = sext i8 %b to i64\n  ret i64 %e\n}}");
    assert_eq!(selected(&ir).unwrap(), [["LDRSBXui", "RET_ReallyLR"]]);
}

/// Two adjacent byte stores of constants are one `STRHHui` in the pin; without
/// the truncating-store merge they were two `STRBBui`.
#[test]
fn adjacent_constant_byte_stores_merge_into_a_halfword_store() {
    let ir = "define void @f(ptr %p) {\n  %b = getelementptr i8, ptr %p, i64 1\n  store i8 1, ptr %p, align 1\n  store i8 0, ptr %b, align 1\n  ret void\n}";
    assert_eq!(selected(ir).unwrap(), [["MOVi32imm", "STRHHui", "RET_ReallyLR"]]);
}

/// `select (x > 3 && y < 9), x, y` is `SUBSWri`, `CCMPWi` and one `CSELWr` in
/// the pin (`emitConjunction`); without it each compare made its own `CSINC`
/// and an `ANDS` joined them.
#[test]
fn an_and_of_two_compares_is_a_compare_and_a_conditional_compare() {
    let ir = "define i32 @f(i32 %x, i32 %y) {\n  %a = icmp sgt i32 %x, 3\n  %b = icmp slt i32 %y, 9\n  %c = and i1 %a, %b\n  %r = select i1 %c, i32 %x, i32 %y\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["SUBSWri", "CCMPWi", "CSELWr", "RET_ReallyLR"]]);
}

/// `select (x == 72 || x == 79), 2, 1` is `SUBSWri`, `CCMPWr` and a `CSINCWr` of
/// the constants (AArch64's `convertSelectOfConstantsToMath` is false). The
/// combiner made it `add (zext cond), 1`, which hid the `or` of compares from
/// the conditional-compare chain.
#[test]
fn a_select_of_constants_on_an_or_of_compares_stays_a_select() {
    let ir = "define i32 @f(i32 %x) {\n  %a = icmp eq i32 %x, 72\n  %b = icmp eq i32 %x, 79\n  %c = or i1 %a, %b\n  %r = select i1 %c, i32 2, i32 1\n  ret i32 %r\n}";
    let mut got = selected(ir).unwrap();
    got[0].sort();
    assert_eq!(got[0], ["CCMPWr", "CSINCWr", "MOVi32imm", "MOVi32imm", "RET_ReallyLR", "SUBSWri"], "{got:?}");
}

/// A branch on `a < b || freeze(c < d)` is a load, `SUBSWrr`, `CCMPWr`
/// and one `Bcc` (the pin pushes the freeze onto the compare's operands, which leaves a tree
/// of compares). With the freeze left on top it was a `CSINC` per compare, an `ORR`
/// and a `TBNZ`.
#[test]
fn a_branch_on_an_or_with_a_frozen_compare_is_a_conditional_compare_chain() {
    let ir = "@v = global i32 0\ndeclare void @g()\ndeclare void @h()\ndefine void @f(i32 %a, i32 %b, i32 %c, i32 %d) {\n  %l = load i32, ptr @v\n  %x = icmp slt i32 %a, %b\n  %y = icmp slt i32 %c, %l\n  %fy = freeze i1 %y\n  %o = or i1 %x, %fy\n  br i1 %o, label %t, label %e\nt:\n  call void @g()\n  ret void\ne:\n  call void @h()\n  ret void\n}";
    let got = selected(ir).unwrap();
    assert_eq!(got[0], ["ADRP", "LDRWui", "SUBSWrr", "CCMPWr", "Bcc", "B"], "{got:?}");
}

/// `tail call @g(a)` is `TCRETURNdi` and no `BL` in the pin; without tail
/// calls it was `ADJCALLSTACKDOWN`, `BL`, `ADJCALLSTACKUP` and `RET`.
#[test]
fn a_tail_call_with_register_arguments_is_tcreturn() {
    let ir = "declare i32 @g(i32)\ndefine i32 @f(i32 %a) {\n  %r = tail call i32 @g(i32 %a)\n  ret i32 %r\n}";
    assert_eq!(selected(ir).unwrap(), [["TCRETURNdi"]]);
}

/// `x * -2` was refused (`Legalize(Unsupported Shl i8 "promote operand")`): the multiply fold made
/// the shift with an `i8` amount, which AArch64 cannot promote. 3 corpus functions were refused.
#[test]
fn a_multiply_by_a_negated_power_of_two_is_a_negated_shift() {
    let ir = "define i32 @f(i32 %a) {\n  %r = mul i32 %a, -2\n  ret i32 %r\n}";
    assert!(selected(ir).is_ok(), "{:?}", selected(ir));
}

/// A signed compare of two `i8` values was refused (`Legalize(Unsupported Shl i8 "promote operand")`):
/// the setcc promotion sign-extended in the register with an `i8` shift amount.
#[test]
fn a_signed_i8_compare_promotes_with_the_targets_shift_amount_type() {
    let ir = "define i1 @f(i8 %x) {\n  %r = icmp sgt i8 %x, -3\n  ret i1 %r\n}";
    assert!(selected(ir).is_ok(), "{:?}", selected(ir));
}

/// `umax i8` was refused (`Legalize(Unsupported UMax i8 "promote result")`): the type legalizer had no
/// promotion for the min/max nodes (loop_with_many_global_pages @text_write).
#[test]
fn an_i8_unsigned_max_is_promoted() {
    let ir = "declare i8 @llvm.umax.i8(i8, i8)\ndefine i8 @f(i8 %a, i8 %b) {\n  %r = call i8 @llvm.umax.i8(i8 %a, i8 %b)\n  ret i8 %r\n}";
    assert!(selected(ir).is_ok(), "{:?}", selected(ir));
}

/// `sdiv i32 x, 5` took `ADDWrr` of two shifted halves; the pin folds the sign bit of the 64-bit product
/// into one `ADDXrs ..., lsr 63` (`trunc_isWorthFoldingALU` was mapped to `never`). signed_divide @by_five, @by_seven.
#[test]
fn a_signed_divide_by_five_adds_the_shifted_product_in_64_bits() {
    let ir = "define i32 @f(i32 %x) {\n  %r = sdiv i32 %x, 5\n  ret i32 %r\n}";
    let ops = selected(ir).unwrap().concat();
    assert!(ops.contains(&"ADDXrs".into()) && !ops.contains(&"ADDWrr".into()) && !ops.contains(&"UBFMXri".into()), "{ops:?}");
}

/// `sum_to`'s closed form kept `add (zext n), 0x1FFFFFFFF` and a mask of the product (`MOVi64imm`, `ADDXrr`);
/// the pin demands only the low bits of the truncated result, so the constant is -1 (`SUBSXri 1`) and the mask goes.
#[test]
fn a_truncated_product_demands_only_its_low_bits() {
    let ir = "define i32 @f(i32 %n) {\n  %a = zext i32 %n to i33\n  %b = add nsw i33 %a, -1\n  %c = add i32 %n, -2\n  %d = zext i32 %c to i33\n  %m = mul i33 %b, %d\n  %s = lshr i33 %m, 1\n  %t = trunc i33 %s to i32\n  ret i32 %t\n}";
    let ops = selected(ir).unwrap().concat();
    assert!(!ops.contains(&"MOVi64imm".into()), "{ops:?}");
}

/// A byte loaded in one block and masked in the next kept its `ANDWri 255`: the pin records the loaded value's
/// known bits at the `CopyToReg` (`ComputeLiveOutVRegInfo`) and asserts them at the other block's `CopyFromReg`.
#[test]
fn a_value_known_zero_extended_in_another_block_needs_no_mask() {
    let ir = "define i32 @f(ptr %p, i1 %c) {\nentry:\n  %b = load i8, ptr %p\n  %z = zext i8 %b to i32\n  br i1 %c, label %t, label %e\nt:\n  %m = and i32 %z, 255\n  ret i32 %m\ne:\n  ret i32 %z\n}";
    let blocks = selected(ir).unwrap();
    assert!(!blocks.concat().contains(&"ANDWri".into()), "{blocks:?}");
}

/// A byte loaded in one block and compared in the next was exported any-extended, so the other block masked it
/// (`ANDWri 255`): the pin exports a value `isZExtFree` holds for (a narrow load) zero-extended
/// (slp_compares @under_or).
#[test]
fn a_loaded_byte_is_exported_zero_extended() {
    let ir = "define i1 @f(ptr %p, i1 %c) {\nentry:\n  %b = load i8, ptr %p\n  br i1 %c, label %t, label %e\nt:\n  %r = icmp eq i8 %b, 66\n  ret i1 %r\ne:\n  ret i1 0\n}";
    let blocks = selected(ir).unwrap();
    assert!(!blocks.concat().contains(&"ANDWri".into()), "{blocks:?}");
}

/// `and (cset a), (cset b)` was `ANDWrr` of two `cset`s; the pin knows a `CSEL 0, 1` has only its low bit set
/// (`computeKnownBitsForTargetNode`), so `topbitsallzero` holds and the `and` is one `CSELWr` (slp_compares @stored_between).
#[test]
fn an_and_of_a_cset_and_a_compare_result_is_a_csel() {
    let ir = "declare void @g()\ndefine i32 @f(i8 %a, i8 %b, i8 %c, i8 %d) {\n  %p = icmp eq i8 %a, 65\n  %q = icmp eq i8 %b, 66\n  %x = select i1 %p, i1 %q, i1 false\n  call void @g()\n  %r = icmp eq i8 %c, 67\n  %s = icmp eq i8 %d, 68\n  %y = select i1 %r, i1 %s, i1 false\n  %z = and i1 %x, %y\n  %e = zext i1 %z to i32\n  ret i32 %e\n}";
    let ops = selected(ir).unwrap().concat();
    assert!(ops.contains(&"CSELWr".into()) && !ops.contains(&"ANDWrr".into()), "{ops:?}");
}

/// `store i32 4, p+16` then `store ptr p+16, ...`: the pin writes the first as a pre-indexed store
/// (`STRWpre`) and stores the writeback address, where we kept the `add` (`ADDXri`) beside a plain `STRWui`
/// (list_optsize @main).
#[test]
fn a_store_whose_address_is_used_again_is_pre_indexed() {
    let ir = "@g = internal global [4 x i64] zeroinitializer, align 8\ndefine void @f(ptr %p) {\n  %a = getelementptr inbounds i8, ptr %p, i64 16\n  store i32 4, ptr %a, align 8\n  store ptr %a, ptr %p, align 8\n  ret void\n}";
    let ops = selected(ir).unwrap().concat();
    assert!(ops.contains(&"STRWpre".into()), "{ops:?}");
}

/// A volatile `store double 1.0e300` was refused (`Legalize(custom lowering of ConstantPool)`): the combine
/// skipped every volatile store, and the pin turns it into an `i64` store when that store is legal
/// (`replaceStoreOfFPConstant`, float_constant_store @set).
#[test]
fn a_volatile_double_constant_store_is_an_integer_store() {
    let ir = "@d = global double 0.0\ndefine void @f() {\n  store volatile double 1.0e300, ptr @d, align 8\n  ret void\n}";
    let ops = selected(ir).unwrap().concat();
    assert!(ops.contains(&"STRXui".into()), "{ops:?}");
}

/// A call returning `{ i64, i1 }` was refused (`an aggregate result with a target's calling hook`): the
/// hook path took one result value, so the struct was never split per leaf (agg_phi @retry).
#[test]
fn a_call_returning_a_struct_is_split_per_leaf() {
    let ir = "declare { i64, i1 } @step(i64)\ndefine i64 @f(i64 %s) {\n  %c = call { i64, i1 } @step(i64 %s)\n  %v = extractvalue { i64, i1 } %c, 0\n  ret i64 %v\n}";
    let ops = selected(ir).unwrap().concat();
    assert!(ops.contains(&"BL".into()), "{ops:?}");
}

/// `select %a, 16, 8` beside `%a | %b` kept a second use on `%a`'s compare, so the or was two `CSINC`s where
/// the pin has one `CCMP`: the pin turns a select on a compare into `select_cc` before legalizing, which
/// frees the compare (conditional_compare @shared_leaf).
#[test]
fn a_select_on_a_compare_frees_it_for_a_conditional_compare() {
    let ir = "define i64 @f(i32 %x, ptr %p) {\n  %a = icmp eq i32 %x, 72\n  %b = icmp eq i32 %x, 79\n  %c = or i1 %a, %b\n  %r = select i1 %c, i64 2, i64 1\n  store i64 %r, ptr %p\n  %s = select i1 %a, i64 16, i64 8\n  ret i64 %s\n}";
    let ops = selected(ir).unwrap().concat();
    assert!(ops.contains(&"CCMPWr".into()) || ops.contains(&"CCMPWi".into()), "{ops:?}");
}

/// A branch taken when `a < 1 || b < a || c > d` is false kept the or as a `CSINC` and a `TBZ` where the
/// pin has a `CCMP` chain and one `Bcc`: the inverted branch (`xor c, 1`) hid the tree from the chain
/// (loop_with_many_global_pages @text_view).
#[test]
fn an_inverted_branch_on_an_or_of_compares_is_a_ccmp_chain() {
    let ir = "@rows = global i8 0\ndeclare void @g()\ndefine void @f(i32 %a, i32 %b, i32 %c) {\n  %l = load i8, ptr @rows\n  %d = zext i8 %l to i32\n  %p = icmp slt i32 %a, 1\n  %q = icmp slt i32 %b, %a\n  %r = icmp sgt i32 %b, %d\n  %o = or i1 %p, %q\n  %t = select i1 %o, i1 true, i1 %r\n  %n = xor i1 %t, true\n  br i1 %n, label %ok, label %bad\nbad:\n  call void @g()\n  br label %ok\nok:\n  ret void\n}";
    let ops = selected(ir).unwrap().concat();
    assert!(ops.contains(&"CCMPWr".into()) && !ops.contains(&"TBZW".into()) && !ops.contains(&"TBNZW".into()), "{ops:?}");
}

/// `a == 0 || b > x || c > y` kept its written order, so the `CCMP` chain started at the last compare
/// (`SUBSWrr`); the pin pairs the two `>` first (`reassociateOps`' compare-pair rule) and the chain
/// starts at `a == 0` (`SUBSWri`) (loop_with_many_global_pages @text_locate_cursor).
#[test]
fn an_or_of_compares_pairs_those_of_one_predicate() {
    let ir = "declare void @g()\ndefine void @f(i32 %a, i32 %b, i32 %c, i32 %x, i32 %y) {\n  %p = icmp eq i32 %a, 0\n  %q = icmp sgt i32 %b, %x\n  %r = icmp sgt i32 %c, %y\n  %o = or i1 %p, %q\n  %t = or i1 %o, %r\n  br i1 %t, label %bad, label %ok\nbad:\n  call void @g()\n  br label %ok\nok:\n  ret void\n}";
    let ops = selected(ir).unwrap().concat();
    assert_eq!(ops.first(), Some(&"SUBSWri"), "{ops:?}");
}

/// A load whose address `p+16` is stored again kept the `ADDXri` beside a plain `LDRWui`; the pin writes
/// `LDRWpre` (tryIndexedLoad) and stores the writeback; the same load at `p` with `p+16` after is `LDRWpost`.
#[test]
fn a_load_whose_address_is_used_again_is_indexed() {
    let pre = "define i32 @f(ptr %p, ptr %q) {\n  %a = getelementptr inbounds i8, ptr %p, i64 16\n  %v = load i32, ptr %a, align 4\n  store ptr %a, ptr %q, align 8\n  ret i32 %v\n}";
    let ops = selected(pre).unwrap().concat();
    assert!(ops.contains(&"LDRWpre".into()), "{ops:?}");
    let post = "define i32 @f(ptr %p, ptr %q) {\n  %v = load i32, ptr %p, align 4\n  %a = getelementptr inbounds i8, ptr %p, i64 16\n  store ptr %a, ptr %q, align 8\n  ret i32 %v\n}";
    let ops = selected(post).unwrap().concat();
    assert!(ops.contains(&"LDRWpost".into()), "{ops:?}");
}

/// The instruction an atomic access selects follows its ordering (the pin's `llc -mcpu=apple-m1`):
/// `ldr`/`ldapr`/`ldar`, `str`/`stlr`, and the LSE `ldadd`/`swp` family with their `a`, `l`, `al`
/// suffixes. The ordering is the node's own (`MemInfo::ordering`); a selector that lost it picks
/// the monotonic form for all of them, which is a correct-looking wrong program.
#[test]
fn an_atomic_access_selects_the_instruction_of_its_ordering() {
    let one = |ir: String| selected(&ir).unwrap();
    let load = |o: &str| one(format!("define i32 @f(ptr %p) {{\n  %a = load atomic i32, ptr %p {o}, align 4\n  ret i32 %a\n}}"));
    assert_eq!(load("monotonic"), [["LDRWui", "RET_ReallyLR"]]);
    assert_eq!(load("acquire"), [["LDAPRW", "RET_ReallyLR"]]);
    assert_eq!(load("seq_cst"), [["LDARW", "RET_ReallyLR"]]);
    let store = |o: &str| one(format!("define void @f(ptr %p, i32 %v) {{\n  store atomic i32 %v, ptr %p {o}, align 4\n  ret void\n}}"));
    assert_eq!(store("monotonic"), [["STRWui", "RET_ReallyLR"]]);
    assert_eq!(store("release"), [["STLRW", "RET_ReallyLR"]]);
    assert_eq!(store("seq_cst"), [["STLRW", "RET_ReallyLR"]]);
    for (op, family) in [("add", "LDADD"), ("xchg", "SWP")] {
        let rmw = |o: &str| {
            one(format!("define i32 @f(ptr %p, i32 %v) {{\n  %a = atomicrmw {op} ptr %p, i32 %v {o}, align 4\n  ret i32 %a\n}}"))
        };
        for (ordering, suffix) in
            [("monotonic", ""), ("acquire", "A"), ("release", "L"), ("acq_rel", "AL"), ("seq_cst", "AL")]
        {
            let name: &'static str = Box::leak(format!("{family}{suffix}W").into_boxed_str());
            assert_eq!(rmw(ordering), [[name, "RET_ReallyLR"]], "{op} {ordering}");
        }
    }
}

/// LSE has `ldadd` and `ldclr` but no `ldsub` or `ldand`: a fetch-sub is a negation and an `ldadd`,
/// a fetch-and a complement (`orn wzr`) and an `ldclr` (the pin's `ExpandNode` and
/// `LowerATOMIC_LOAD_AND`). Both were refused (`convert node to libcall`, `custom lowering of
/// AtomicLoadAnd`).
#[test]
fn a_fetch_sub_negates_and_adds_and_a_fetch_and_complements_and_clears() {
    let rmw = |op: &str, ordering: &str| {
        selected(&format!(
            "define i32 @f(ptr %p, i32 %v) {{\n  %a = atomicrmw {op} ptr %p, i32 %v {ordering}, align 4\n  ret i32 %a\n}}"
        ))
    };
    assert_eq!(rmw("sub", "monotonic").unwrap(), [["SUBSWrr", "LDADDW", "RET_ReallyLR"]]);
    assert_eq!(rmw("and", "release").unwrap(), [["ORNWrr", "LDCLRLW", "RET_ReallyLR"]]);
}

/// An atomic access narrower than a register (`i8`, `i16`) was refused (`Legalize(Unsupported
/// AtomicSwap i8 "promote result")`): the type legalizer keeps the memory type and promotes the
/// value and result (`PromoteIntRes_Atomic0/1`, `PromoteIntOp_ATOMIC_STORE`).
#[test]
fn a_narrow_atomic_access_keeps_its_memory_type() {
    let ir = |body: &str, ret: &str, args: &str| format!("define {ret} @f(ptr %p{args}) {{\n  {body}\n}}");
    let xchg = ir("%a = atomicrmw xchg ptr %p, i8 %v acquire, align 1\n  ret i8 %a", "i8", ", i8 %v");
    assert_eq!(selected(&xchg).unwrap(), [["SWPAB", "RET_ReallyLR"]]);
    let add = ir("%a = atomicrmw add ptr %p, i16 %v seq_cst, align 2\n  ret i16 %a", "i16", ", i16 %v");
    assert_eq!(selected(&add).unwrap(), [["LDADDALH", "RET_ReallyLR"]]);
    let load = ir("%a = load atomic i8, ptr %p acquire, align 1\n  ret i8 %a", "i8", "");
    assert_eq!(selected(&load).unwrap(), [["LDAPRB", "RET_ReallyLR"]]);
    let store = ir("store atomic i16 %v, ptr %p release, align 2\n  ret void", "void", ", i16 %v");
    assert_eq!(selected(&store).unwrap(), [["STLRH", "RET_ReallyLR"]]);
    let sub = ir("%a = atomicrmw sub ptr %p, i8 %v monotonic, align 1\n  ret i8 %a", "i8", ", i8 %v");
    assert_eq!(selected(&sub).unwrap(), [["SUBSWrr", "LDADDB", "RET_ReallyLR"]]);
}

/// A `cmpxchg` was refused (`an atomic other than a load, store or read-modify-write`). The pin
/// selects one CAS of its success ordering, then `SUBSWrr` of the old and expected values and
/// `CSINCWr` for the flag (`ExpandNode` of `ATOMIC_CMP_SWAP_WITH_SUCCESS`).
#[test]
fn a_cmpxchg_selects_the_cas_of_its_ordering_and_a_flag_from_the_compare() {
    let ir = |order: &str, ty: &str| {
        format!("define i1 @f(ptr %p, {ty} %a, {ty} %b) {{\n  %r = cmpxchg ptr %p, {ty} %a, {ty} %b {order} monotonic\n  %s = extractvalue {{ {ty}, i1 }} %r, 1\n  ret i1 %s\n}}")
    };
    for (order, cas) in [("monotonic", "CASW"), ("acquire", "CASAW"), ("release", "CASLW"), ("acq_rel", "CASALW"), ("seq_cst", "CASALW")] {
        assert_eq!(selected(&ir(order, "i32")).unwrap(), [[cas, "SUBSWrr", "CSINCWr", "RET_ReallyLR"]], "{order}");
    }
}

/// A `fence` was refused (no `ATOMIC_FENCE` node). The pin selects `DMB ishld` for an acquire
/// fence, `DMB ish` for the others, and `MEMBARRIER` for a single-thread one.
#[test]
fn a_fence_selects_a_dmb_by_its_ordering() {
    let ir = |scope: &str, order: &str| format!("define void @f() {{\n  fence {scope}{order}\n  ret void\n}}");
    for (order, expect) in [("acquire", "DMB"), ("release", "DMB"), ("seq_cst", "DMB")] {
        assert_eq!(selected(&ir("", order)).unwrap(), [[expect, "RET_ReallyLR"]], "{order}");
    }
    assert_eq!(selected(&ir("syncscope(\"singlethread\") ", "seq_cst")).unwrap(), [["MEMBARRIER", "RET_ReallyLR"]]);
}

/// A `zeroext` i8 argument passed on as `zeroext` got an `ANDWri` (the incoming register carried no
/// `AssertZext`); the pin passes the register as it is.
#[test]
fn a_zeroext_argument_passed_on_needs_no_mask() {
    let ir = "declare void @draw(i8 zeroext)\ndefine void @f(i8 zeroext %a) {\n  tail call void @draw(i8 zeroext %a)\n  ret void\n}";
    assert_eq!(selected(ir).unwrap(), [["TCRETURNdi"]]);
}
