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

/// A pass that makes `add x, 1` into `add x, 0` and says it changed the
/// function, or does not.
struct Spoil {
    reports_a_change: bool,
}

impl FunctionPass for Spoil {
    fn name(&self) -> &'static str {
        "spoil"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        _: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        let (_, add) = unit.function.walk().next().expect("an add");
        let zero = llrm_mir::module::Operand::Constant(unit.context.int(unit.function.instruction(add).ty, 0));
        unit.function.set_operand(add, 1, zero);
        if self.reports_a_change {
            llrm_mir::passes::PreservedAnalyses::none()
        } else {
            llrm_mir::passes::PreservedAnalyses::all()
        }
    }
}

const ADD: &str = "define i32 @f(i32 %x) {
b0:
  %a = add i32 %x, 1
  ret i32 %a
}
";

/// `ADD` through `Combine`, `Spoil`, `Combine`.
fn combine_spoil_combine(reports_a_change: bool) -> String {
    let mut module = parsed(ADD);
    let mut manager = PassManager::default();
    manager.add(Combine { enabled: true });
    manager.add(Spoil { reports_a_change });
    manager.add(Combine { enabled: true });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    printed(&module)
}

/// A pass that changed the function in between drops what InstCombine last saw
/// (LLVM's `LastRunTrackingAnalysis`): the second run folds `add x, 0`.
#[test]
fn a_changing_pass_between_two_runs_makes_the_second_run_visit() {
    let out = combine_spoil_combine(true);
    assert!(!out.contains("add"), "{out}");
}

/// With nothing reported changed since the last run the second one is skipped,
/// as `InstCombinePass::run` returns at `LRT.shouldSkip(&ID)`: the pipeline's
/// idle `combine@N` slots each walked the whole function again (66 M
/// instructions over five slots at d_faces -O2, the pin's ~5 K).
#[test]
fn a_run_with_nothing_changed_since_the_last_is_skipped() {
    let out = combine_spoil_combine(false);
    assert!(out.contains("add i32 %x, 0"), "{out}");
}

/// Runs `Combine` and keeps what it preserved.
struct Recording(std::rc::Rc<std::cell::Cell<Option<bool>>>);

impl FunctionPass for Recording {
    fn name(&self) -> &'static str {
        "recording"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        let preserved = Combine { enabled: true }.run(unit, analyses);
        self.0.set(Some(preserved.cfg_preserved()));
        preserved
    }
}

/// A run that changed the function still preserves the CFG analyses, as
/// `InstCombinePass::run` returns `preserveSet<CFGAnalyses>`: it changes no
/// block or edge. It returned `none()`, and the dominator tree and loops were
/// rebuilt after every combine@N that changed anything.
#[test]
fn a_run_that_changed_the_function_preserves_the_cfg_analyses() {
    let seen = std::rc::Rc::new(std::cell::Cell::new(None));
    let mut module = parsed(TEXT);
    let mut manager = PassManager::default();
    manager.add(Recording(std::rc::Rc::clone(&seen)));
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    assert!(printed(&module).contains("[4 x i8]"), "the run changed the body");
    assert_eq!(seen.get(), Some(true));
}
