//! A callee that holds a loop, inlined at several sites of a loop of its caller, is a variant the driver prices
//! against the calls (DEVIATE LOOP-COPY-TRIAL): idioms -O2 made 13 copies of `sum` and 10 of `init`, 4555 B and
//! 25.58M T, where the calls are 1993 B and 24.49M T (-O1: 1869 B, 24.47M T).

use llrm_z80::compile::{effective, refuses_loop_copies, text};
use llrm_z80::driver::{Level, Request, loop_copies_made, selections_with_alternatives};

/// `sum` called at three sites of a loop of `main`: clang -O2 IR, the loop kept as a callee.
const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
@a = internal global [160 x i16] zeroinitializer\n\
declare void @report(i32)\n\
define internal i32 @sum(ptr %p, i16 %n) {\n\
entry:\n  %c = icmp sgt i16 %n, 0\n  br i1 %c, label %loop, label %done\n\
loop:\n  %i = phi i16 [0, %entry], [%i1, %loop]\n  %s = phi i32 [0, %entry], [%s1, %loop]\n\
  %g = getelementptr i16, ptr %p, i16 %i\n  %v = load i16, ptr %g\n  %w = sext i16 %v to i32\n\
  %i1 = add i16 %i, 1\n  %k = sext i16 %i1 to i32\n  %m = mul i32 %k, %w\n  %s1 = add i32 %s, %m\n\
  %e = icmp slt i16 %i1, %n\n  br i1 %e, label %loop, label %done\n\
done:\n  %r = phi i32 [0, %entry], [%s1, %loop]\n  ret i32 %r\n}\n\
define i16 @main() {\n\
entry:\n  br label %outer\n\
outer:\n  %t = phi i16 [0, %entry], [%t1, %outer]\n\
  %x = call i32 @sum(ptr @a, i16 %t)\n  call void @report(i32 %x)\n\
  %q = getelementptr i16, ptr @a, i16 8\n  %y = call i32 @sum(ptr %q, i16 %t)\n  call void @report(i32 %y)\n\
  %r = getelementptr i16, ptr @a, i16 16\n  %z = call i32 @sum(ptr %r, i16 %t)\n  call void @report(i32 %z)\n\
  %t1 = add i16 %t, 1\n  %d = icmp slt i16 %t1, 6\n  br i1 %d, label %outer, label %out\n\
out:\n  ret i16 0\n}\n";

fn calls_of_sum(request: &Request) -> usize {
    text(IR, "main", 0, request).expect("compiles").matches("call _sum").count()
}

/// The mid-end of the trial compile leaves the calls where the default one inlines them, and the default one
/// says it made such copies: a driver that cannot tell has nothing to price.
#[test]
fn the_trial_compile_refuses_the_copies_the_default_one_makes_and_notes() {
    let inlined = Request::new(Level::O2);
    selections_with_alternatives(IR, &inlined).expect("selects");
    assert!(loop_copies_made() >= 3, "the default compile made {} such copies", loop_copies_made());
    let refused = Request { refuse_loop_copies: true, ..Request::new(Level::O2) };
    selections_with_alternatives(IR, &refused).expect("selects");
    assert_eq!(loop_copies_made(), 0);
    assert!(calls_of_sum(&refused) >= 3, "the refused compile keeps the calls");
    assert_eq!(calls_of_sum(&inlined), 0, "the default compile inlines them");
}

/// The copies stay where they pay: this loop runs its callee 15 times and the calls cost more clocks than the
/// bytes of the copies are worth at 3 T a byte, so `effective` keeps the default compile.
#[test]
fn copies_that_pay_stay() {
    let chosen = effective(IR, &Request::new(Level::O2)).expect("chooses");
    assert!(!chosen.refuse_loop_copies, "refused at {:?}", chosen.level);
}

/// idioms -O2 (measured on ucsim): the copies are bigger and slower, so the module without them is kept at the
/// rate, at -O3, and at the strict levels by bytes; byval_inline -O2 (244 B / 4981 T against 329 B / 9148 T
/// without) keeps its copy.
#[test]
fn the_module_without_the_copies_is_kept_where_it_is_smaller_and_not_slower() {
    let (with, without) = ((4555, 25_582_412), (1993, 24_491_357));
    assert!(refuses_loop_copies(Level::O2, with, without) && refuses_loop_copies(Level::O3, with, without));
    assert!(refuses_loop_copies(Level::O1, with, without) && refuses_loop_copies(Level::Os, with, without));
    assert!(!refuses_loop_copies(Level::O2, (244, 4981), (329, 9148)));
    assert!(!refuses_loop_copies(Level::O2, with, with), "a tie keeps the copies");
}
