//! CVP against LLVM's `correlated-propagation` on `tests/cvp/**/*.ll`, its
//! work on doubled functions, and the boundary of each DEVIATE of `cvp.rs`.
//!
//! Each `NAME.ll` is run through the pass and must come out as `NAME.after.ll`
//! (written by `LLRM_BLESS=1`); `tools/difftest` read the same pair as
//! identical to `opt`'s answer, bar the blocks `opt` leaves with no
//! predecessor (`llrm-pass` settles them, the pass alone does not remove
//! them here either: they are listed in the `.after.ll`).

use std::path::{Path, PathBuf};

use llrm_mir::target::Neutral;

use crate::cvp::{Cvp, SOLVED};
use crate::testing::parsed;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cvp")
}

/// Every input under `tests/cvp`, with its `.after.ll`.
fn fixtures() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).expect("tests/cvp").flatten() {
            let path = entry.path();
            let name = path.file_name().expect("a name").to_string_lossy().into_owned();
            if path.is_dir() {
                pending.push(path);
            } else if name.ends_with(".ll") && !name.ends_with(".after.ll") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// `text` through the pass under the pass manager, printed. The module must
/// verify but for the intrinsics MIR does not have, which the fixtures under
/// `intrinsics/` use.
fn run(text: &str) -> String {
    let mut module = parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(Cvp);
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let rest: Vec<String> = llrm_mir::verify::verify(&module)
        .into_iter()
        .filter(|one| !one.contains("an intrinsic MIR does not have"))
        .collect();
    assert_eq!(rest, Vec::<String>::new());
    llrm_mir::print::module(&module)
}

#[test]
fn every_fixture_comes_out_as_its_after_file() {
    let fixtures = fixtures();
    assert!(fixtures.len() >= 20, "{} fixtures", fixtures.len());
    for path in fixtures {
        let before = std::fs::read_to_string(&path).expect("readable");
        let after = run(&before);
        let expected = path.with_extension("after.ll");
        if std::env::var_os("LLRM_BLESS").is_some() {
            std::fs::write(&expected, &after).expect("writable");
        }
        let want = std::fs::read_to_string(&expected).unwrap_or_else(|_| panic!("{} has no .after.ll", path.display()));
        assert_eq!(after, want, "{}", path.display());
        assert_ne!(after, llrm_mir::print::module(&parsed(&before)), "{} changed nothing", path.display());
    }
}

/// `n` diamonds in a row, each merging a value bumped one way or the other;
/// `%x` is tested at the top of each. CVP asks the range of every operand.
fn diamonds(n: usize) -> String {
    let mut text = String::from("define i32 @f(i32 %x) {\nentry:\n  br label %l0\n");
    for i in 0..n {
        let prev = if i == 0 { "0".to_owned() } else { format!("%p{}", i - 1) };
        text += &format!(
            "l{i}:\n  %c{i} = icmp ult i32 %x, {}\n  br i1 %c{i}, label %t{i}, label %e{i}\nt{i}:\n  %a{i} = add i32 {prev}, 1\n  br label %m{i}\ne{i}:\n  %b{i} = sub i32 {prev}, 1\n  br label %m{i}\nm{i}:\n  %p{i} = phi i32 [ %a{i}, %t{i} ], [ %b{i}, %e{i} ]\n  br label %l{}\n",
            1000 - i,
            i + 1
        );
    }
    text += &format!("l{n}:\n  ret i32 %p{}\n}}\n", n - 1);
    text
}

/// The `solve` iterations of LVI for one run over `text`.
fn solved(text: &str) -> u64 {
    SOLVED.with(|one| one.set(0));
    run(text);
    SOLVED.with(std::cell::Cell::get)
}

/// A function of N instructions costs O(N) solves: LVI's cache answers each
/// query after the first at a block in O(1), and the pass tells it only of the
/// values it erases. Doubling the function must not more than double the work.
#[test]
fn doubling_the_function_doubles_the_solves() {
    let (small, large) = (solved(&diamonds(60)), solved(&diamonds(120)));
    assert!(small > 100, "{small} solves: the pass asked LVI nothing");
    assert!(large * 10 <= small * 21, "{small} solves for 60 diamonds, {large} for 120");
}

/// LLVM keeps `LazyValueAnalysis` between passes that leave the function as it
/// is. Here each `cvp` and `decide` slot started an empty LVI: nine runs of
/// the same solves on a diamond ladder (+16% of its compile).
#[test]
fn a_second_run_on_an_unchanged_function_solves_nothing() {
    let text = "define i32 @f(i32 %x, i32 %y) {\nentry:\n  %c = icmp ult i32 %x, %y\n  %r = select i1 %c, i32 %x, i32 %y\n  ret i32 %r\n}\n";
    let solves = |passes: usize| {
        let mut module = parsed(text);
        let mut manager = llrm_mir::passes::PassManager::default();
        for _ in 0..passes {
            manager.add(Cvp);
        }
        SOLVED.with(|one| one.set(0));
        manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
        SOLVED.with(std::cell::Cell::get)
    };
    let once = solves(1);
    assert!(once > 0, "the pass asked LVI nothing");
    assert_eq!(solves(2), once);
}

/// LVI takes an `ICmpInst` predicate and MIR holds no floating-point compare
/// of constants for it to fold, so `processCmp` leaves an `fcmp` alone. LLVM
/// answers `ret i1 true`; this fails the day `cvp.rs` folds it.
#[test]
fn an_fcmp_of_constants_is_left_alone() {
    let text = "define i1 @f(float %x) {\nentry:\n  %c = fcmp olt float 1.0, 2.0\n  ret i1 %c\n}\n";
    assert_eq!(run(text), llrm_mir::print::module(&parsed(text)));
}

/// MIR has no operand bundles or lane operations: the `deopt` loop of
/// `processCallSite` and the vector arms have nothing to read. A test fails
/// the day MIR gains one. (`undef` arrived: its CVP/LVI states are owed,
/// #2260.)
#[test]
fn mir_lacks_what_cvp_leaves_out() {
    let rejects = |text: &str| llrm_mir::parse::module(text).is_err();
    assert!(rejects("declare void @g()\ndefine void @f() {\n  call void @g() [ \"deopt\"(i32 1) ]\n  ret void\n}\n"));
    assert!(rejects("define i32 @f(<2 x i32> %a) {\n  %x = extractelement <2 x i32> %a, i32 0\n  ret i32 %x\n}\n"));
}
