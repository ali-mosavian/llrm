//! DEVIATE UNROLL-SPEED-TRIAL: at -O2/-O3 the module compiled again without
//! the unrolls that add bytes is kept where it is smaller.

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

fn at(
    level: Level,
    ir: &str,
) -> String {
    let text = format!("target datalayout = \"{}\"\n{ir}", Mos.layout().datalayout);
    let mut request = Request::new(level);
    request.cpu = Cpu::Mos6502;
    driver::assembly(&text, &request).unwrap_or_else(|why| panic!("{level:?}: {why}"))
}

/// memmove_overlap's `fill`: 32 constant stores in a static function called
/// three times.
const FILL: &str = "@buf = global [32 x i8] zeroinitializer
declare void @report(i32)

define internal void @fill() {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %p = getelementptr inbounds [32 x i8], ptr @buf, i16 0, i16 %i
  %b = trunc i16 %i to i8
  %v = add i8 %b, 1
  store i8 %v, ptr %p
  %next = add nuw nsw i16 %i, 1
  %done = icmp eq i16 %next, 32
  br i1 %done, label %exit, label %loop
exit:
  ret void
}

define i16 @main() {
  call void @fill()
  call void @report(i32 1)
  call void @fill()
  call void @report(i32 2)
  call void @fill()
  call void @report(i32 3)
  ret i16 0
}
";

fn stores(text: &str) -> usize {
    text.lines().filter(|line| line.trim_start().starts_with("sta buf+")).count()
}

/// memmove_overlap -O3: the early unroll of `fill`, exempt from the byte price
/// because the inliner might take it, stood unrolled with the inliner taking
/// none of its three sites: 32 stores, +147 B (758 B against 611 at -O2).
#[test]
fn a_loop_unrolled_unpriced_that_the_inliner_leaves_stays_rolled_when_that_is_smaller() {
    for level in [Level::O2, Level::O3] {
        let kept = driver::no_grow_variants_kept();
        let text = at(level, FILL);
        assert!(text.matches("jsr fill").count() == 3, "{level:?}: the calls are kept\n{text}");
        assert_eq!(driver::no_grow_variants_kept() - kept, usize::from(level == Level::O3), "{level:?}");
        assert!(stores(&text) < 8, "{level:?}: `fill` is unrolled ({} stores)\n{text}", stores(&text));
    }
}

/// A module with no loop unrolled is compiled once: no variant, no trial.
#[test]
fn a_module_with_no_unroll_is_compiled_once() {
    let before = driver::mid_end_runs();
    let text = at(Level::O3, "define i16 @main() {\n  ret i16 0\n}\n");
    assert!(text.contains("main"), "{text}");
    assert_eq!(driver::mid_end_runs() - before, 1);
}

/// write_through_parameter -O2/-O3: refusing the unroll of the callee's loop
/// before it is inlined left the caller's loops unfolded, 44 B became 250 B.
/// The module with the refusals is larger, so the pin's decisions are kept.
#[test]
fn the_variant_is_not_taken_where_it_is_larger() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/write_through_parameter_o2.ll");
    let ir = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("target datalayout") && !line.starts_with("target triple"))
        .collect::<Vec<_>>()
        .join("\n");
    for level in [Level::O2, Level::O3] {
        let kept = driver::no_grow_variants_kept();
        let text = at(level, &ir);
        assert_eq!(driver::no_grow_variants_kept(), kept, "{level:?}: the larger variant was kept");
        let moves = text
            .lines()
            .filter(|line| line.trim_start().starts_with("lda") || line.trim_start().starts_with("sta"))
            .count();
        assert!(moves < 20, "{level:?}: {moves} loads and stores, the unrolls were refused\n{text}");
    }
}
