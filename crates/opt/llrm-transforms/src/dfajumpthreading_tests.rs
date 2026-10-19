//! DFAJumpThreading against LLVM's `dfa-jump-threading` on
//! `tests/dfajumpthreading/*.ll`.
//!
//! Each `NAME.ll` is run through the pass and must come out as
//! `NAME.after.ll` (written by `LLRM_BLESS=1`); `tools/difftest/difftest.py`
//! reads the same pair against `opt -passes=dfa-jump-threading`.

use std::path::{Path, PathBuf};

use llrm_mir::target::Neutral;

use crate::dfajumpthreading::{DfaJumpThreading, DfaJumpThreadingOptions};
use crate::testing::parsed;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dfajumpthreading")
}

/// Every input under `tests/dfajumpthreading`, with its `.after.ll`.
fn fixtures() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root())
        .expect("tests/dfajumpthreading")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().expect("a name").to_string_lossy().into_owned();
            name.ends_with(".ll") && !name.ends_with(".after.ll")
        })
        .collect();
    found.sort();
    found
}

/// `text` through the pass under the pass manager with `options`, printed; the
/// module must verify.
fn run_with(
    text: &str,
    options: DfaJumpThreadingOptions,
) -> String {
    let mut module = parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(DfaJumpThreading { enabled: true, for_size: false, options });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
    llrm_mir::print::module(&module)
}

fn run(text: &str) -> String {
    run_with(text, DfaJumpThreadingOptions::default())
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(root().join(format!("{name}.ll"))).expect("a fixture")
}

#[test]
fn every_fixture_comes_out_as_its_after_file() {
    let fixtures = fixtures();
    assert!(fixtures.len() >= 5, "{} fixtures", fixtures.len());
    for path in fixtures {
        let before = std::fs::read_to_string(&path).expect("readable");
        let after = run(&before);
        let expected = path.with_extension("after.ll");
        if std::env::var_os("LLRM_BLESS").is_some() {
            std::fs::write(&expected, &after).expect("writable");
        }
        let want = std::fs::read_to_string(&expected).unwrap_or_else(|_| panic!("{} has no .after.ll", path.display()));
        assert_eq!(after, want, "{}", path.display());
    }
}

/// A loop of `n` states, the switch on the state in the loop's header block
/// and each case setting the next state.
fn state_machine(n: usize) -> String {
    let mut text = String::from(
        "define i32 @f(i32 %n) {\nentry:\n  br label %loop\n\nloop:\n  %state = phi i32 [ 0, %entry ], [ %next, %latch ]\n  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]\n  %go = icmp slt i32 %i, %n\n  br i1 %go, label %dispatch, label %done\n\ndispatch:\n  switch i32 %state, label %case.default [\n",
    );
    for k in 0..n {
        text.push_str(&format!("    i32 {k}, label %case{k}\n"));
    }
    text.push_str("  ]\n\n");
    for k in 0..n {
        text.push_str(&format!("case{k}:\n  br label %latch\n\n"));
    }
    text.push_str("case.default:\n  br label %latch\n\nlatch:\n  %next = phi i32 ");
    let arms: Vec<String> = (0..n).map(|k| format!("[ {}, %case{k} ]", (k + 1) % n)).collect();
    text.push_str(&arms.join(", "));
    text.push_str(", [ 0, %case.default ]\n  %i.next = add i32 %i, 1\n  br label %loop\n\ndone:\n  ret i32 %i\n}\n");
    text
}

/// The search for paths looks at 8 states and at 32 in work that grows with
/// them, not with their square: the blocks and phi inputs it looks at, at 4x
/// the states, are at most 4.5x. (From 8 states on the pin's clone rate
/// refuses to thread: the search is what is measured.)
#[test]
fn test_the_path_search_grows_linearly_with_the_states() {
    let steps = |n: usize| {
        let before = crate::dfajumpthreading::path_steps();
        run(&state_machine(n));
        crate::dfajumpthreading::path_steps() - before
    };
    let (small, large) = (steps(8), steps(32));
    assert!(small > 0, "the search is counted");
    assert!(large * 10 <= small * 45, "{small} steps at 8 states, {large} at 32");
}

/// A `select` in the chain that makes the state is DF1: the pin unfolds a
/// valid one into control flow and threads; the switch is left alone here
/// until stage S2.
#[test]
fn test_a_select_that_could_be_unfolded_is_left_alone_until_stage_s2() {
    let text = "define i32 @f(i1 %c, i32 %n) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %sel, %pre ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %pre ]
  switch i32 %state, label %done [
    i32 0, label %case0
    i32 1, label %case1
  ]

case0:
  br label %pre

case1:
  br label %pre

pre:
  %sel = select i1 %c, i32 1, i32 0
  %i.next = add i32 %i, 1
  br label %loop

done:
  ret i32 %i
}
";
    let before = llrm_mir::print::module(&parsed(text));
    assert_eq!(run(text), before);
}

/// With the pass off, or in a function the pin skips for its size, nothing is
/// done to a machine that is threaded otherwise.
#[test]
fn test_a_disabled_pass_and_an_optsize_function_are_left_alone() {
    let text = state_machine(4);
    let before = llrm_mir::print::module(&parsed(&text));
    let mut module = parsed(&text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(DfaJumpThreading { enabled: false, for_size: false, options: DfaJumpThreadingOptions::default() });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    assert_eq!(llrm_mir::print::module(&module), before);

    let optsize = text.replace("(i32 %n) {", "(i32 %n) optsize {");
    assert_eq!(run(&optsize), llrm_mir::print::module(&parsed(&optsize)));
    assert_ne!(run(&text), before, "the same machine is threaded without optsize");
}

/// The caps are options: a limit of no paths finds no path to thread.
#[test]
fn test_the_caps_are_options_of_the_pass() {
    let text = state_machine(4);
    let none = DfaJumpThreadingOptions { max_num_paths: 0, ..DfaJumpThreadingOptions::default() };
    assert_eq!(run_with(&text, none), llrm_mir::print::module(&parsed(&text)));
    let cheap = DfaJumpThreadingOptions { cost_threshold: 0, ..DfaJumpThreadingOptions::default() };
    assert_eq!(run_with(&text, cheap), llrm_mir::print::module(&parsed(&text)));
}

/// A path through a landing pad is not threaded (DF3): the pin copies the pad
/// and `ehprepare` refuses a second one ("more than one landing pad", as for
/// JumpThreading's J10), so the one pad stays and nothing is copied.
#[test]
fn test_a_path_through_a_landing_pad_is_not_copied() {
    let text = "declare void @f()
declare i32 @__gxx_personality_v0(...)

define i32 @t(i32 %n) personality ptr @__gxx_personality_v0 {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %next, %pad ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %pad ]
  switch i32 %state, label %done [
    i32 0, label %case0
    i32 1, label %case1
  ]

case0:
  invoke void @f() to label %cont unwind label %pad

case1:
  invoke void @f() to label %cont unwind label %pad

cont:
  ret i32 7

pad:
  %next = phi i32 [ 1, %case0 ], [ 0, %case1 ]
  %lp = landingpad { ptr, i32 } cleanup
  %i.next = add i32 %i, 1
  %go = icmp slt i32 %i.next, %n
  br i1 %go, label %loop, label %done

done:
  ret i32 %i
}
";
    let after = run(text);
    assert_eq!(after.matches("landingpad").count(), 1, "{after}");
    assert!(!after.contains(".jt"), "{after}");
}

/// -Os: the pin skips a function that has `optsize` (`hasOptSize`), which
/// clang gives every function of a size build, and llrm's functions do not
/// carry it. x_switch at -Os grew from 108 to 133 lines of assembly with the
/// pass on: a size build threads nothing.
#[test]
fn test_a_build_for_size_threads_nothing() {
    let text = state_machine(4);
    let mut module = parsed(&text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(DfaJumpThreading::from_env().for_size(true));
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    assert_eq!(llrm_mir::print::module(&module), llrm_mir::print::module(&parsed(&text)));
    assert_ne!(run(&text), llrm_mir::print::module(&parsed(&text)), "the same machine is threaded otherwise");
}
