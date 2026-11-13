//! A trial compile resumes where the base compile was before the walk.

use super::{recording_prewalk, resuming_prewalk};
use crate::pipeline::{Applied, Options};
use crate::testing::{parsed, printed};

const LOOPS: &str = "define internal i16 @sum(ptr addrspace(1) %p, i16 %n) noinline {
b0:
  br label %loop
loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %loop ]
  %acc = phi i16 [ 0, %b0 ], [ %add, %loop ]
  %at = getelementptr i16, ptr addrspace(1) %p, i16 %i
  %v = load i16, ptr addrspace(1) %at
  %add = add i16 %acc, %v
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %n
  br i1 %done, label %out, label %loop
out:
  ret i16 %add
}

define i16 @top(ptr addrspace(1) %p, i16 %n) {
b0:
  %r = call i16 @sum(ptr addrspace(1) %p, i16 %n)
  %s = add i16 %r, 1
  ret i16 %s
}
";

/// The pipeline over `LOOPS`: the module's text and the `runs` lines.
fn compiled() -> (String, Vec<String>) {
    let mut module = parsed(&format!("{}{LOOPS}", llrm_analysis::testing::DOS));
    let ((), lines) = crate::pipeline::capturing_runs(|| {
        llrm_mir::program::Program::lend(
            &mut module,
            std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
            |program: &mut llrm_mir::program::Program| {
                program.exports.entries.insert("top".to_owned());
                crate::pipeline::applied(program, &Applied { options: Options::speed(), ..Default::default() })
            },
        )
        .and_then(|done| done)
        .expect("the pipeline runs");
    });
    (printed(&module), lines)
}

fn bodies(lines: &[String]) -> usize {
    lines.iter().filter(|line| line.starts_with("body ")).count()
}

/// Every trial compile of a module ran the whole pipeline again, the passes before the walk
/// (`ipo-early`, the early and cleanup lists) included, though nothing in them reads what a
/// trial reverses: jpeg_fixed -Os ran the mid-end four times (4026 M instructions against 1538 M
/// at -O2, the pin's 377 M). The resumed compile is the same module and runs fewer bodies.
#[test]
fn a_resumed_compile_is_the_same_module_without_the_passes_before_the_walk() {
    let (plain, plain_lines) = compiled();
    let ((recorded, _), kept) = recording_prewalk(compiled);
    let kept = kept.expect("the walk was reached");
    assert_eq!(recorded, plain, "recording changed the result");
    let (resumed, resumed_lines) = resuming_prewalk(&kept, compiled);
    assert_eq!(resumed, plain, "the resumed compile is another module");
    assert!(
        bodies(&resumed_lines) < bodies(&plain_lines),
        "the resumed compile ran {} bodies, the plain one {}",
        bodies(&resumed_lines),
        bodies(&plain_lines)
    );
}
