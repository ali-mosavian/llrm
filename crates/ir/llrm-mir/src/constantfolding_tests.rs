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
/// day MIR gains one, so its translation is owed then.
#[test]
fn mir_has_no_binary_constant_expressions_and_no_inrange() {
    for expr in [
        "sub (i64 ptrtoint (ptr @g to i64), i64 1)",
        "add (i64 ptrtoint (ptr @g to i64), i64 1)",
        "xor (i64 ptrtoint (ptr @g to i64), i64 1)",
        "select (i1 true, i64 1, i64 2)",
    ] {
        let text = format!("@g = global i32 0\ndefine void @f() {{\n  %x = add i64 {expr}, 1\n  ret void\n}}\n");
        assert!(rejects(&text), "{expr}");
    }
    assert!(rejects(
        "@g = global [2 x i32] zeroinitializer\n@h = global ptr getelementptr inrange(0, 8) ([2 x i32], ptr @g, i64 0, i64 1)\n"
    ));
}

#[test]
fn mir_has_no_ptrtoaddr_and_no_bytes() {
    assert!(rejects("define i64 @f(ptr %p) {\n  %x = ptrtoaddr ptr %p to i64\n  ret i64 %x\n}\n"));
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
