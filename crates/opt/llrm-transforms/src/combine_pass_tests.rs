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

/// `TEXT` through the whole pipeline with `Options::combine` as given.
fn pipeline_with(combine: bool) -> String {
    use crate::pipeline::{self, Applied};
    let mut module = parsed(
        "target datalayout = \"e-p:64:64-i64:64\"
define internal ptr @g(ptr %b, i64 %i) {
b0:
  %p = getelementptr i32, ptr %b, i64 %i
  ret ptr %p
}

define ptr @main(ptr %b, i64 %i) {
b0:
  %r = call ptr @g(ptr %b, i64 %i)
  ret ptr %r
}
",
    );
    llrm_mir::program::Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("main".to_owned());
        let options = pipeline::Options { combine, ..pipeline::Options::default() };
        pipeline::applied(program, &Applied { options, ..Applied::default() })
    })
    .and_then(|done| done)
    .unwrap();
    printed(&module)
}

/// `LLRM_NO_COMBINE` (`Options::combine` false) leaves the whole pipeline
/// without any of the translated families' output; set, they are in it.
#[test]
fn the_pipeline_honours_the_combine_switch() {
    let off = pipeline_with(false);
    let on = pipeline_with(true);
    assert!(!off.contains("[4 x i8]"), "{off}");
    assert!(on.contains("[4 x i8]"), "{on}");
}
