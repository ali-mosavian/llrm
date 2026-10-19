use llrm_mir::passes::{FunctionPass, PassManager};

use super::Combine;
use crate::testing::{parsed, printed};

const TEXT: &str = "define ptr @f(ptr %b, i64 %i) {
b0:
  %p = getelementptr i32, ptr %b, i64 %i
  ret ptr %p
}
";

/// `TEXT` after `pass` alone.
fn run(pass: impl FunctionPass + 'static) -> String {
    let mut module = parsed(TEXT);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(pass);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    printed(&module)
}

/// The pass combines: a GEP over `i32` becomes the canonical `[4 x i8]` one.
#[test]
fn the_pass_runs_the_translated_families() {
    let out = run(Combine { enabled: true });
    assert!(out.contains("getelementptr [4 x i8], ptr %b, i64 %i"), "{out}");
}

/// Off (`LLRM_NO_COMBINE`), it leaves the body alone.
#[test]
fn off_it_changes_nothing() {
    assert_eq!(run(Combine { enabled: false }), printed(&parsed(TEXT)));
}
