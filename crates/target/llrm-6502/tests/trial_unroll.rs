//! DEVIATE UNROLL-TRIAL: a loop the unroll threshold refused is unrolled on
//! trial and kept when the simplified function pays.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn module(body: &str) -> String {
    format!("target datalayout = \"{}\"\n{body}\n", datalayout())
}

/// A table filled by one loop and summed by the next: 16 trips of a body the
/// threshold of 24 refuses. Rolled it is two loops and 32 table accesses a run;
/// unrolled, every load folds to its store and the sum is the constant 360.
/// shrink_wrapped_frame at -O2 was 952 B and 28,433 clocks, 51 B and 489 once
/// the pin-scale threshold let it unroll.
const FOLDING: &str = "@t = internal global [16 x i16] zeroinitializer
define i16 @sum() {
entry:
  br label %fill
fill:
  %i = phi i16 [ 0, %entry ], [ %in, %fill ]
  %p = getelementptr [16 x i16], ptr @t, i16 0, i16 %i
  %v = mul i16 %i, 3
  store i16 %v, ptr %p
  %in = add i16 %i, 1
  %c = icmp ult i16 %in, 16
  br i1 %c, label %fill, label %add
add:
  %j = phi i16 [ 0, %fill ], [ %jn, %add ]
  %acc = phi i16 [ 0, %fill ], [ %an, %add ]
  %q = getelementptr [16 x i16], ptr @t, i16 0, i16 %j
  %w = load i16, ptr %q
  %an = add i16 %acc, %w
  %jn = add i16 %j, 1
  %d = icmp ult i16 %jn, 16
  br i1 %d, label %add, label %done
done:
  ret i16 %an
}";

/// A hash over 16 words of memory the function is handed: nothing folds, so
/// unrolling only copies the body (a multiply by a constant is shifts and adds)
/// and saves the counter and the index: 1.0 clock for each byte it adds, short
/// of the 3 the rate asks. It stays a loop.
const NOT_FOLDING: &str = "define i16 @hash(ptr %s) {
entry:
  br label %l
l:
  %i = phi i16 [ 0, %entry ], [ %in, %l ]
  %h = phi i16 [ 0, %entry ], [ %hn, %l ]
  %p = getelementptr i16, ptr %s, i16 %i
  %x = load i16, ptr %p
  %y = xor i16 %h, %x
  %m1 = mul i16 %y, 31
  %z = xor i16 %m1, %x
  %m2 = mul i16 %z, 29
  %w = xor i16 %m2, %y
  %hn = mul i16 %w, 27
  %in = add i16 %i, 1
  %c = icmp ult i16 %in, 16
  br i1 %c, label %l, label %done
done:
  ret i16 %hn
}";

#[test]
fn a_fill_and_a_sum_that_fold_are_unrolled_to_a_constant() {
    let text = assembly(&module(FOLDING), &Request::new(Level::O2)).unwrap();
    let loops = text.lines().filter(|line| line.trim_start().starts_with('b') && !line.contains("bit")).count();
    assert!(loops == 0 && text.contains("#104"), "the loops stay (sum is 360 = $168):\n{text}");
}

#[test]
fn an_unroll_that_folds_nothing_is_put_back() {
    let text = assembly(&module(NOT_FOLDING), &Request::new(Level::O2)).unwrap();
    let xors = text.lines().filter(|line| line.trim_start().starts_with("eor")).count();
    assert!(xors < 8, "{xors} xors: the 16 trips were copied out:\n{text}");
}

/// `fcmp_all` was unrolled to 85,508 bytes more at 23 clk/B: it pays by the
/// rate and would not fit the 64 KiB address space. A trial is put back past
/// the growth limit whatever it saves, at either level.
#[test]
fn a_trial_that_grows_the_code_past_the_limit_is_put_back() {
    use llrm_6502::driver::trial_pays;
    assert!(trial_pays(Level::O2, 3000, 100_000, 8_000));
    assert!(!trial_pays(Level::O2, 3000, 2_008_446, 85_508));
    assert!(trial_pays(Level::Os, 0, 0, -1));
    assert!(!trial_pays(Level::Os, 0, 1_000_000, 17_000));
}

/// `fcmp_all`'s trial took 14 s of selection to price 85,508 bytes more: a 4
/// min corpus compile became 6 (+76%, 2026-11-08). A trial past the instruction
/// growth limit is put back before it is priced.
#[test]
fn a_trial_that_grows_the_ir_past_the_limit_is_not_priced() {
    use llrm_6502::driver::trial_fits;
    assert!(trial_fits(461)); // loop_fusion, kept
    assert!(!trial_fits(2808)); // fcmp_all
}

/// The trial priced the first build, then the trial, then selected the winner
/// a third time: +16.9% of corpus compile at -O2 (2026-11-08). Each build is
/// selected once and the winner's text is the one priced.
#[test]
fn a_trial_selects_each_build_once() {
    let before = llrm_6502::driver::selected();
    assembly(&module(FOLDING), &Request::new(Level::O2)).unwrap();
    let functions = llrm_6502::driver::selected() - before;
    let before = llrm_6502::driver::selected();
    assembly(&module(FOLDING), &Request::new(Level::O1)).unwrap();
    let once = llrm_6502::driver::selected() - before;
    assert_eq!(functions, 2 * once, "two builds, one selection each");
}

/// A 4-trip loop that calls out each trip: unrolled it saves 252 clocks for 96
/// more bytes (2.6 clk/B, measured 235 for +96, 2.4), under the 3 clk/B a byte
/// is worth at -O2. The estimate weighed the loop at ten trips and kept it at
/// four times the clocks (`extension_chain_fused`, 2026-11-08).
const FOUR_TRIPS: &str = "@owner = global [4 x i8] c\"\\01\\FE\\03\\FC\"
declare void @report(i32)
define i16 @main() {
entry:
  br label %loop
loop:
  %k = phi i16 [ 0, %entry ], [ %kn, %loop ]
  %p = getelementptr [4 x i8], ptr @owner, i16 0, i16 %k
  %b = load i8, ptr %p
  %s = sext i8 %b to i16
  %a = add i16 %s, -1
  %w = sext i16 %a to i32
  call void @report(i32 %w)
  %kn = add i16 %k, 1
  %c = icmp ult i16 %kn, 4
  br i1 %c, label %loop, label %done
done:
  ret i16 0
}";

#[test]
fn a_four_trip_loop_is_priced_at_four_trips() {
    let asm = assembly(&module(FOUR_TRIPS), &Request::new(Level::O2)).unwrap();
    let calls = asm.lines().filter(|line| line.trim() == "jsr report").count();
    assert_eq!(calls, 1, "the loop was unrolled to {calls} calls:\n{asm}");
}

/// A full unroll that repaid its bytes at 3 clocks each was made at -O2/-O3:
/// eight stores of `v + i` were 8 `clc; txa; adc` and a 22 B body, against a 12
/// B loop; bytes come first (2026-11-14).
#[test]
fn a_full_unroll_that_adds_bytes_is_not_made_at_speed() {
    let body = "@g = global [8 x i8] zeroinitializer
define void @f(i8 %v) {
entry:
  br label %loop
loop:
  %i = phi i8 [ 0, %entry ], [ %n, %loop ]
  %p = getelementptr [8 x i8], ptr @g, i16 0, i8 %i
  %w = add i8 %v, %i
  store i8 %w, ptr %p
  %n = add i8 %i, 1
  %c = icmp ult i8 %n, 8
  br i1 %c, label %loop, label %done
done:
  ret void
}";
    for level in [Level::O2, Level::O3] {
        let text = assembly(&module(body), &Request::new(level)).unwrap();
        let adds = text.lines().filter(|line| line.trim_start().starts_with("adc")).count();
        assert!(adds < 4 && text.contains("bne"), "{level:?}: {adds} adds, the loop was copied out:\n{text}");
    }
}
