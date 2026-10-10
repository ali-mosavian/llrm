//! Port of `tests/test_float_loop_exit.py`, each body now MIR text run by
//! llrm-mir's interpreter before and after.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::specialized;
use crate::testing::{parsed, printed, results};

/// FPCSE: 48.75 added ten times, the counter stored in the header and
/// both read after the loop.
const FPCSE: &str = "@m = global [8 x i8] zeroinitializer

define float @f() {
b0:
  store float 0.000000e+00, ptr @m
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b2 ]
  store i16 %i, ptr getelementptr (i8, ptr @m, i16 4)
  %c = icmp slt i16 %i, TRIPS
  br i1 %c, label %b2, label %b3

b2:
  %v = load float, ptr @m
  %w = fadd float %v, STEP
  store float %w, ptr @m
  %n = add i16 %i, 1
  br label %b1

b3:
  %r = load float, ptr @m
  %k = load i16, ptr getelementptr (i8, ptr @m, i16 4)
  %j = sitofp i16 %i to float
  %kf = sitofp i16 %k to float
  %s = fadd float %r, %j
  %t = fadd float %s, %kf
  ret float %t
}
";

fn fpcse(trips: i64) -> String {
    FPCSE.replace("TRIPS", &trips.to_string()).replace("STEP", "4.875000e+01")
}

/// `body` specialized, which must compute what it did: whether it changed,
/// and its text.
fn specialize(body: &str) -> (bool, String) {
    let mut module: Module = parsed(&format!("{DOS}{body}"));
    let before = results(&module, &[&[]]);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let calls = llrm_analysis::consts::Calls::default();
    let solved = {
        let unit = llrm_analysis::testing::with_registers(llrm_analysis::memory::Unit::within(
            context, &layout, function, &outer,
        ));
        llrm_analysis::floatfacts::solved_with(&unit, &calls)
    };
    let changed = specialized(context, &layout, &callees, function, &outer, &calls, &solved);
    assert_eq!(results(&module, &[&[]]), before, "{body}");
    (changed, printed(&module))
}

/// FPCSE computes 487.5, but repeated its exact body ten times: it runs
/// the last trip from 438.75, and the counter leaves at 10.
#[test]
fn test_exact_loop_runs_its_last_trip_alone() {
    let (changed, text) = specialize(&fpcse(10));
    assert!(changed);
    assert_eq!(
        text,
        format!(
            "{DOS}@m = global [8 x i8] zeroinitializer

define float @f() {{
b0:
  store float 0.000000e+00, ptr @m
  br label %b1

b1:
  store i16 0, ptr getelementptr (i8, ptr @m, i16 4)
  %c = icmp slt i16 0, 10
  br label %b2

b2:
  store float 4.387500e+02, ptr @m
  %v = load float, ptr @m
  %w = fadd float %v, 4.875000e+01
  store float %w, ptr @m
  %n = add i16 0, 1
  br label %b3

b3:
  store i16 10, ptr getelementptr (i8, ptr @m, i16 4)
  %r = load float, ptr @m
  %k = load i16, ptr getelementptr (i8, ptr @m, i16 4)
  %j = sitofp i16 10 to float
  %kf = sitofp i16 %k to float
  %s = fadd float %r, %j
  %t = fadd float %s, %kf
  ret float %t
}}
"
        )
    );
}

/// No trip, or one, has nothing to save.
#[test]
fn test_zero_or_one_trip_stays() {
    for trips in [0, 1] {
        let body = fpcse(trips);
        let (changed, text) = specialize(&body);
        assert!(!changed, "{trips}");
        assert_eq!(text, format!("{DOS}{body}"));
    }
}

/// The counter a phi after the loop reads is its final number too.
#[test]
fn test_a_join_after_the_loop_reads_the_final_counter() {
    let body = "@m = global [4 x i8] zeroinitializer

define i16 @f() {
b0:
  store float 0.000000e+00, ptr @m
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b2 ]
  %c = icmp slt i16 %i, 6
  br i1 %c, label %b2, label %b3

b2:
  %v = load float, ptr @m
  %w = fadd float %v, 2.500000e-01
  store float %w, ptr @m
  %n = add i16 %i, 2
  br label %b1

b3:
  br label %b4

b4:
  %j = phi i16 [ %i, %b3 ]
  ret i16 %j
}
";
    let (changed, text) = specialize(body);
    assert!(changed);
    assert!(text.contains("%j = phi i16 [ 6, %b3 ]"), "{text}");
}

/// An inexact or unbounded recurrence, a call, float work reading the
/// counter or a value read after the loop is no proved last trip.
#[test]
fn test_unproved_or_observable_iterations_remain() {
    for (from, to) in [
        ("STEP", "0x3FB99999A0000000"),
        ("%w = fadd float %v, STEP", "%w = fdiv float 1.000000e+00, 3.000000e+00"),
        ("  %n = add", "  call void @h()\n  %n = add"),
        ("%w = fadd float %v, STEP", "%ic = sitofp i16 %i to float\n  %w = fadd float %v, %ic"),
        ("%kf = sitofp i16 %k to float", "%kf = uitofp i1 %c to float"),
    ] {
        let body = format!(
            "{}\ndefine void @h() {{\nb0:\n  ret void\n}}\n",
            FPCSE.replace("TRIPS", "10").replace(from, to).replace("STEP", "4.875000e+01")
        );
        let (changed, text) = specialize(&body);
        assert!(!changed, "{to}: {text}");
    }
}

#[test]
fn test_the_pass_runs_under_the_manager() {
    let mut module = parsed(&format!("{DOS}{}", fpcse(10)));
    let before = results(&module, &[&[]]);
    let text = crate::floatfold::tests::summarized(&mut module, super::FloatLoop);
    assert!(text.contains("store float 4.387500e+02, ptr @m") && !text.contains("br i1"), "{text}");
    assert_eq!(results(&module, &[&[]]), before);
}

/// A body with no float in it was solved for floats whole (a dataflow over
/// memory) for every round of every body, and never changed: 18.9 G of QCport's
/// 458 G at -O2 was billed to it. It is not solved, and nothing is changed.
#[test]
fn test_a_body_with_no_float_is_not_solved_for_floats() {
    let integer = "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b1 ]
  %j = add i16 %i, 1
  %c = icmp slt i16 %j, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %j
}
";
    let before = llrm_analysis::manager::float_solves();
    let mut module = parsed(&format!("{DOS}{integer}"));
    let text = printed(&module);
    assert_eq!(crate::testing::managed(&mut module, super::FloatLoop), text);
    assert_eq!(llrm_analysis::manager::float_solves(), before, "solved the floats of a body with none");
    let mut module = parsed(&format!("{DOS}{}", fpcse(10)));
    crate::testing::managed(&mut module, super::FloatLoop);
    assert!(llrm_analysis::manager::float_solves() > before, "a body with floats is solved");
}

#[test]
fn test_what_touches_floats_is_every_float_value_operand_and_aggregate() {
    let touches = |text: &str| {
        let module = parsed(&format!("{DOS}{text}"));
        llrm_analysis::floatfacts::touches(&module.context, llrm_analysis::testing::function(&module, "f"))
    };
    assert!(!touches("define i16 @f(i16 %n) {\nb0:\n  %a = add i16 %n, 1\n  ret i16 %a\n}\n"));
    assert!(touches(
        "define i16 @f(i16 %n) {\nb0:\n  %a = sitofp i16 %n to float\n  %b = fptosi float %a to i16\n  ret i16 %b\n}\n"
    ));
    assert!(touches(
        "@m = global [2 x float] zeroinitializer\ndefine i16 @f(i16 %n) {\nb0:\n  store float 1.0, ptr @m\n  ret i16 %n\n}\n"
    ));
}
