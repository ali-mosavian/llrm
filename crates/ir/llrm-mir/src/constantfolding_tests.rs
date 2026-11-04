//! `constant_fold_instruction` against what LLVM answered on the same IR: the
//! lines of `tests/cf/fold/NAME.expected` are `tools/llvmport/constfold_dump.sh
//! NAME.ll`, and `NAME.deviates` lists the lines llrm answers differently (a
//! DEVIATE of the `constantfolding` header or a gap of `constfold`). The set
//! of lines that differ must be exactly that list: a fold that closes a gap
//! fails here until its line is removed.

use std::collections::BTreeSet;

use crate::constantfolding_dump::dump;
use crate::parse;

macro_rules! cases {
    ($($name:ident),*) => {
        const CASES: &[(&str, &str, &str, &str)] = &[$((
            stringify!($name),
            include_str!(concat!("../tests/cf/fold/", stringify!($name), ".ll")),
            include_str!(concat!("../tests/cf/fold/", stringify!($name), ".expected")),
            include_str!(concat!("../tests/cf/fold/", stringify!($name), ".deviates")),
        )),*];
    };
}

cases!(gep, load, loadbe, cast, icmp, fcmp, intrinsic, misc, deviate, regress);

fn answer(line: &str) -> &str {
    line.splitn(4, ' ').nth(3).expect("four fields")
}

#[test]
fn llvm_and_llrm_fold_the_same_but_the_listed_lines() {
    for (name, text, expected, deviates) in CASES {
        let ours = dump(text).expect("parses");
        let (ours, expected): (Vec<_>, Vec<_>) = (ours.lines().collect(), expected.lines().collect());
        assert_eq!(ours.len(), expected.len(), "{name}: instructions with constant operands");
        let mut differing = BTreeSet::new();
        for (mine, llvm) in ours.iter().zip(&expected) {
            if mine == llvm {
                continue;
            }
            let mut fields = llvm.splitn(3, ' ');
            let (function, n) = (fields.next().expect("function"), fields.next().expect("n"));
            assert!(
                answer(mine) == "-",
                "{name}: llrm folds where LLVM does not or differently:\n  llrm: {mine}\n  llvm: {llvm}"
            );
            differing.insert(format!("{function} {n}"));
        }
        let listed: BTreeSet<String> =
            deviates.lines().map(|line| line.splitn(3, ' ').take(2).collect::<Vec<_>>().join(" ")).collect();
        assert_eq!(differing, listed, "{name}: lines that differ against {name}.deviates");
    }
}

/// An oracle that can only say one thing proves nothing.
#[test]
fn every_expected_file_has_two_distinct_answers() {
    for (name, _, expected, _) in CASES {
        let answers: BTreeSet<&str> = expected.lines().map(answer).collect();
        assert!(answers.len() >= 2, "{name}.expected has only {answers:?}");
        assert!(answers.iter().any(|one| *one != "-"), "{name}.expected folds nothing");
    }
}

fn rejects(text: &str) -> bool {
    parse::module(text).is_err()
}

/// The constructs of `ConstantFolding.cpp` MIR cannot say: a test fails the
/// day MIR gains one, so its translation is owed then. `add`, `sub` and `xor`
/// are said (`ConstantExpr::Binary`; torture loop-2d), and fold as the
/// instructions they are.
#[test]
fn mir_has_no_select_constant_expression_and_no_inrange() {
    let text =
        "@g = global i32 0\ndefine void @f() {\n  %x = add i64 select (i1 true, i64 1, i64 2), 1\n  ret void\n}\n";
    assert!(rejects(text));
    assert!(rejects(
        "@g = global [2 x i32] zeroinitializer\n@h = global ptr getelementptr inrange(0, 8) ([2 x i32], ptr @g, i64 0, i64 1)\n"
    ));
}

#[test]
fn mir_reads_add_sub_and_xor_of_an_address_as_constant_expressions() {
    for op in ["add", "sub", "xor"] {
        let text = format!(
            "@g = global i32 0\ndefine i64 @f() {{\n  %x = add i64 {op} (i64 ptrtoint (ptr @g to i64), i64 1), 1\n  ret i64 %x\n}}\n"
        );
        let module = parse::module(&text).unwrap_or_else(|why| panic!("{op}: {why:?}"));
        let printed = crate::print::module(&module);
        assert!(printed.contains(&format!("{op} (i64 ptrtoint (ptr @g to i64), i64 1)")), "{printed}");
    }
}

#[test]
fn mir_reads_ptrtoaddr_as_ptrtoint_and_has_no_bytes() {
    let module = parse::module("define i64 @f(ptr %p) {\n  %x = ptrtoaddr ptr %p to i64\n  ret i64 %x\n}\n")
        .expect("ptrtoaddr is read");
    let text = crate::print::module(&module);
    assert!(text.contains("ptrtoint ptr %p to i64") && !text.contains("ptrtoaddr"), "{text}");
    assert!(rejects("define void @f(b8 %p) {\n  ret void\n}\n"));
}

/// The parser ate `externally_initialized`, so a load of such a constant folded
/// to its initializer where LLVM (`hasDefinitiveInitializer`) keeps the load.
#[test]
fn a_load_of_an_externally_initialized_constant_is_kept() {
    let text = "@g = externally_initialized constant i32 5\n@h = constant i32 6\ndefine void @f() {\n  %x = load i32, ptr @g\n  %y = load i32, ptr @h\n  ret void\n}\n";
    let out = dump(text).expect("parses");
    assert!(out.contains("f 0 load -"), "{out}");
    assert!(out.contains("load i32 6"), "{out}");
}

#[test]
fn mir_has_no_lane_operations_and_no_half() {
    assert!(rejects("define void @f(<4 x i32> %a) {\n  %x = extractelement <4 x i32> %a, i32 0\n  ret void\n}\n"));
    assert!(rejects("define half @f(half %a) {\n  ret half %a\n}\n"));
}

/// `DataLayout` ignored `ni:`, so a GEP of null in a non-integral address space
/// folded to an `inttoptr`, which LLVM never introduces there. Space 0 still
/// does.
#[test]
fn a_gep_in_a_non_integral_address_space_is_not_an_inttoptr() {
    let text = "target datalayout = \"ni:4\"\ndefine void @f() {\n  %x = getelementptr i8, ptr addrspace(4) null, i64 50\n  %y = getelementptr i8, ptr null, i64 50\n  ret void\n}\n";
    let out = dump(text).expect("parses");
    assert!(!out.contains("addrspace(4) inttoptr"), "{out}");
    assert!(out.contains("inttoptr (i64 50"), "{out}");
}

/// A huge pointer's packed bits are not an address: `6000:0000 + 70000` folded
/// to the integer `6001:1170`, which is 16 bytes past the start, not 70000 (a
/// carry past the offset goes 0x1000 into the selector). A huge space marked
/// non-integral keeps the GEP.
#[test]
fn a_gep_of_a_huge_pointer_is_not_added_as_an_integer() {
    let text = |datalayout: &str| {
        format!(
            "target datalayout = \"{datalayout}\"\ndefine ptr addrspace(3) @f() {{\n  %p = getelementptr i8, ptr addrspace(3) inttoptr (i32 1610612736 to ptr addrspace(3)), i32 70000\n  ret ptr addrspace(3) %p\n}}\n"
        )
    };
    let flat = dump(&text("e-p:16:16-p3:32:16:32")).expect("parses");
    let kept = dump(&text("e-p:16:16-p3:32:16:32-ni:3")).expect("parses");
    assert!(flat.contains("inttoptr (i32 1610682736"), "{flat}");
    assert!(!kept.contains("1610682736"), "{kept}");
}

/// far3.bas (a PEEK/POKE loop on `SADD(a$)` after `DEF SEG = VARSEG(a$)`)
/// printed the string unchanged and n = 0 at -O1: the pair near -> selector ->
/// far was folded to near -> far, which keeps the offset of the global (the
/// selector drops it, the far pointer back has segment:0), so every address was
/// off by the global's own. A pair through a space the datalayout marks
/// non-integral (`ni:2`, the target's selector space) stays two casts.
#[test]
fn a_pair_of_casts_through_a_non_integral_space_is_not_one_cast() {
    let text = |datalayout: &str| {
        format!(
            "target datalayout = \"{datalayout}\"

@g = global i16 0

define ptr addrspace(1) @f() {{
b0:
  %far = addrspacecast ptr addrspace(2) addrspacecast (ptr @g to ptr addrspace(2)) to ptr addrspace(1)
  ret ptr addrspace(1) %far
}}
"
        )
    };
    let folded = dump(&text("e-p:16:16-p1:32:16:16:16-p2:16:16")).expect("parses");
    let kept = dump(&text("e-p:16:16-p1:32:16:16:16-p2:16:16-ni:2")).expect("parses");
    assert_ne!(folded, kept, "the selector space composed as an integral one");
}

/// A far pointer is non-integral by its width (bits beyond its index), and
/// the gate of the pair fold named that: a pair through a far middle, near ->
/// far -> huge, stopped folding (25 objects of the sweep moved with it). Only
/// a space `ni:` names, the selector alone, is no point on the way.
#[test]
fn a_pair_through_a_far_middle_still_folds_when_the_selector_space_is_ni() {
    let text = |datalayout: &str| {
        format!(
            "target datalayout = \"{datalayout}\"

@g = global i16 0

define ptr addrspace(3) @f() {{
b0:
  %huge = addrspacecast ptr addrspace(1) addrspacecast (ptr @g to ptr addrspace(1)) to ptr addrspace(3)
  ret ptr addrspace(3) %huge
}}
"
        )
    };
    let base = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32";
    let marked = dump(&text(&format!("{base}-ni:2"))).expect("parses");
    assert!(!marked.contains("addrspace(1)"), "the far middle stayed: {marked}");
}

/// A load through `segment:0` of a global's selector read the global's own
/// initializer at the offset (`@k[3]`, 100) where the address is DS:3.
#[test]
fn a_load_through_a_selector_cast_is_not_folded_from_the_global_it_came_from() {
    let text = |datalayout: &str| {
        format!(
            "target datalayout = \"{datalayout}\"

@k = constant [4 x i8] c\"abcd\"

define i8 @f() {{
b0:
  %v = load i8, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) addrspacecast (ptr @k to ptr addrspace(2)) to ptr addrspace(1)), i16 3)
  ret i8 %v
}}
"
        )
    };
    let base = "e-p:16:16-p1:32:16:16:16-p2:16:16";
    assert!(dump(&text(base)).expect("parses").contains("100"), "premise: an integral middle space folds");
    assert!(!dump(&text(&format!("{base}-ni:2"))).expect("parses").contains("100"));
}

/// `log(10.0)` in C is the host libm's double. Constant folding had no float
/// arm, so `llvm.log` of a constant stayed a call.
#[test]
fn a_libm_intrinsic_of_a_constant_folds_in_a_function_that_evaluates_in_double() {
    let text = "declare double @llvm.log.f64(double)
declare double @llvm.floor.f64(double)

define double @f() {
b0:
  %v = call double @llvm.log.f64(double 1.0e1)
  ret double %v
}
";
    let folded = dump(text).expect("parses");
    assert!(folded.contains(&format!("f 0 call double 0x{:016X}", 10f64.ln().to_bits())), "{folded}");
}

/// BASIC's `LOG` is computed in 80 bits (FLDLN2; FXCH; FYL2X): a double
/// constant is a different last digit (tests/run/qb/log_exp_digits). A
/// function the HIR lowering made carries `"llrm-fp-eval"="x87-extended"` and
/// keeps the call; the rounding group is exact at any width and folds anyway.
#[test]
fn a_libm_intrinsic_in_an_extended_precision_function_is_left_but_floor_folds() {
    let text = "declare double @llvm.log.f64(double)
declare double @llvm.floor.f64(double)

define double @f() \"llrm-fp-eval\"=\"x87-extended\" {
b0:
  %v = call double @llvm.log.f64(double 1.0e1)
  %w = call double @llvm.floor.f64(double 2.5)
  %r = fadd double %v, %w
  ret double %r
}
";
    let folded = dump(text).expect("parses");
    assert!(folded.contains("f 0 call -\n"), "the log is left: {folded}");
    assert!(folded.contains("f 1 call double 0x4000000000000000"), "the floor folds: {folded}");
}

/// What `dump` answers for each instruction of `@f`, in order.
fn answers_of(text: &str) -> Vec<String> {
    dump(text).expect("parses").lines().map(|line| answer(line).to_owned()).collect()
}

/// An FP binary fold is exact or refused
/// (docs/working/fp-constant-fold-plan.md): `0.1 + 0.2` rounds, and x87
/// evaluates it wider than f32 or f64 do, so a fold in the format could print a
/// different number than the target computes.
#[test]
fn an_fp_binary_op_folds_only_when_its_result_is_exact_in_the_format() {
    let got = answers_of(
        "define void @f() {
  %a = fadd double 1.0, 2.0
  %b = fadd double 0.1, 0.2
  %c = fadd float 1.5, 2.25
  %d = fadd float 0x3FB99999A0000000, 0x3FC99999A0000000
  %e = fmul double 1.5, 4.0
  %g = fdiv double 1.0, 4.0
  %h = fdiv double 1.0, 3.0
  %i = frem double 7.5, 2.0
  %j = fsub float 1.0, 1.0
  %k = fadd nsz double 1.0, 2.0
  %l = fdiv double 1.0, 0.0
  ret void
}
",
    );
    let folds: Vec<bool> = got.iter().map(|one| one != "-").collect();
    assert_eq!(folds, [true, false, true, false, true, true, false, true, true, false, false], "{got:?}");
}

/// DEVIATE 9: the pin leaves `lrint` of a constant; an integral one is exact
/// in every rounding mode and folds, a fractional one waits for the mode.
/// fpemu's constant `CLNG`s were `fld; fistp` at m16 -Os (1,378 B, main's
/// 1,231).
#[test]
fn an_lrint_of_an_integral_constant_folds_and_a_fractional_one_does_not() {
    let text = "define void @f() {\n  %a = call i32 @llvm.lrint.i32.f64(double 1.049600e+06)\n  %b = call i32 @llvm.lrint.i32.f64(double 2.5)\n  %c = call i32 @llvm.lrint.i32.f64(double 3.000000e+10)\n  %d = call i64 @llvm.llrint.i64.f64(double -4.0)\n  ret void\n}\ndeclare i32 @llvm.lrint.i32.f64(double)\ndeclare i64 @llvm.llrint.i64.f64(double)\n";
    let folded = dump(text).unwrap();
    let lines: Vec<&str> = folded.lines().collect();
    assert_eq!(lines, ["f 0 call i32 1049600", "f 1 call -", "f 2 call -", "f 3 call i64 18446744073709551612"]);
}
