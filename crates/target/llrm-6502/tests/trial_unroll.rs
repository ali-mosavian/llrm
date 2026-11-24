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

/// 16 trips that fold nothing: 306 B more for 1,858 clocks (6 clk/B). Bytes
/// first (ruling 2026-11-14) put it back at every level; at -O2 and -O3 the
/// judge's 3 clk/B keeps it (DEVIATE 6502-UNROLL-RATE), -Os still puts it back.
#[test]
fn an_unroll_that_folds_nothing_is_kept_where_the_clocks_repay_the_bytes() {
    let xors = |level| {
        let text = assembly(&module(NOT_FOLDING), &Request::new(level)).unwrap();
        text.lines().filter(|line| line.trim_start().starts_with("eor")).count()
    };
    assert!(xors(Level::O2) >= 8, "-O2 kept the loop: the unroll repays 6 clk a byte");
    assert!(xors(Level::Os) < 8, "-Os copied the 16 trips out");
}

/// `fcmp_all` was unrolled to 85,508 bytes more at 23 clk/B and would not fit
/// the 64 KiB address space. A trial that grows the code is put back at -Os and
/// -O1 whatever it saves (bytes first, 2026-11-14). At -O2 and -O3 one that
/// grows it is kept where the clocks repay the bytes at the judge's 3 clk/B and
/// it is under the pin's size bound (300 instructions, 2 B each, a function):
/// long_double_constant_store -O3 was 2,490 clk against 979 for 47 B.
#[test]
fn a_trial_that_grows_the_code_is_priced_at_the_judge_under_a_size_bound() {
    use llrm_6502::driver::trial_pays;
    assert!(!trial_pays(Level::O2, 100_000, 8_000, 1), "past the size bound of a function");
    assert!(!trial_pays(Level::O3, 2_008_446, 85_508, 1));
    assert!(!trial_pays(Level::Os, 1_000_000, 17_000, 1), "-Os is bytes only");
    assert!(!trial_pays(Level::O1, 1_000_000, 100, 1), "-O1 is strict");
    assert!(trial_pays(Level::O3, 2_100, 278, 1), "dirty: 278 B for 2,100 clk");
    assert!(!trial_pays(Level::O3, 600, 278, 1), "under 3 clk a byte");
    assert!(trial_pays(Level::O2, 0, -1, 1));
    assert!(trial_pays(Level::Os, 0, -1, 1));
    assert!(trial_pays(Level::O3, 5, 0, 1));
    assert!(trial_pays(Level::O3, 90_000, 1_000, 2), "the bound grows with the functions");
}

/// A 96-trip loop of volatile byte stores (gcc.c-torture's `dirty`, a stack
/// the program overwrites) is 47 B rolled and 325 B unrolled: unrolled it runs
/// 2.6x faster, which at -O3's 3 clk/B repays the 278 B once it runs often
/// enough (the trial counts a clock per call: 4 calls saved 3,080, 2 saved 770
/// and were put back). -Os keeps the loop.
const STORE_LOOP: &str = "define internal void @dirty() noinline {
entry:
  %junk = alloca [96 x i8]
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %in, %loop ]
  %p = getelementptr [96 x i8], ptr %junk, i16 0, i16 %i
  store volatile i8 -1, ptr %p
  %in = add i16 %i, 1
  %c = icmp ult i16 %in, 96
  br i1 %c, label %loop, label %done
done:
  ret void
}
define void @main() {
  call void @dirty()
  call void @dirty()
  call void @dirty()
  call void @dirty()
  ret void
}";

#[test]
fn a_store_loop_the_clocks_repay_is_unrolled_at_o3_and_kept_at_os() {
    let stores = |level| {
        let text = assembly(&module(STORE_LOOP), &Request::new(level)).unwrap();
        text.lines().filter(|line| line.trim_start().starts_with("sta")).count()
    };
    assert!(stores(Level::O3) >= 90, "-O3 copied the 96 stores out");
    assert!(stores(Level::O2) >= 90, "-O2 copied the 96 stores out");
    assert!(stores(Level::Os) < 8, "-Os kept the loop");
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

/// The trial priced the functions' bytes only: static_pointers' unroll folds a
/// 6-byte switch table away for 3 bytes of code, was put back as +3 B, and the
/// program was 196 B against 193 (2026-11-14).
#[test]
fn a_trial_prices_the_initialised_data_too() {
    use llrm_6502::driver::data_bytes;
    let text = ".segment \"RODATA\"\nt:\n  .byte 11,12,30,6,40,13\n  .word 1,2\n.segment \"BSS\"\nb: .res 9\n.segment \"DATA\"\nd:\n  .res 3\n";
    assert_eq!(data_bytes(text), 6 + 4 + 3);
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

/// int64_division's nest at -O2 and -O3 kept both loops and linked `__divdi3`,
/// `__udivdi3` and `__umoddi3`: 4,035 B for four constants. The inner loop (6
/// trips, 1,422 units) is under the pin's threshold of 300 in the pin's own
/// units but over this target's trial size of 1,100; held to the cost the
/// analysis finds (1,200, boosted by 1.18 to 1,017) it is tried, and with it
/// gone the outer loop folds. llvm-mos -O3 and the pin's x86 -O3 do the same.
/// -Os kept the nest too (3,336 B against 62 B at -O2): the pin's size bar is
/// the loop's size plus one and its boost is 100, so the trial never saw it.
#[test]
fn a_nest_of_constant_divisions_folds_to_its_results() {
    for level in [Level::O2, Level::O3, Level::Os] {
        let text = assembly(include_str!("data/division_nest.ll"), &Request::new(level)).unwrap();
        let divisions = text.lines().filter(|line| line.contains("jsr __") && line.contains("div")).count();
        let reports = text.lines().filter(|line| line.contains("jsr report")).count();
        assert!(divisions == 0 && reports == 4, "{divisions} calls to a division, {reports} reports:\n{text}");
    }
}

/// loop_fusion at -Os kept every callee and its loops, 1,041 B against 120 B at
/// -O2: the size rule refuses the callees the unrolled loops fold away, and the
/// pair that inlines them was priced only at -O2. -Os prices the -O2 pair too.
#[test]
fn the_pair_of_inlines_and_unrolls_is_priced_at_os() {
    let text = assembly(include_str!("data/loop_fusion_os.ll"), &Request::new(Level::Os)).unwrap();
    let kept: Vec<&str> = ["clear", "total", "same", "behind", "ahead", "over", "longer"]
        .into_iter()
        .filter(|name| text.contains(&format!("{name}:")))
        .collect();
    assert!(kept.is_empty(), "{kept:?} stay as functions:\n{text}");
}

/// loop_fusion at -O2 (2026-11-22): once a byte-index loop is cheaper rolled,
/// the inline trial kept all three refused sites and the pair of inlines and
/// unrolls was never priced: 756 B and 74,768 clocks against 131 B and 1,067.
#[test]
fn the_pair_of_inlines_and_unrolls_is_priced_after_every_inline_is_kept() {
    let text = assembly(include_str!("data/loop_fusion_os.ll"), &Request::new(Level::O2)).unwrap();
    let branch = ["bne", "beq", "bcc", "bcs", "bpl", "bmi", "bvc", "bvs", "bra", "jmp"];
    let branches =
        text.lines().filter(|line| branch.iter().any(|op| line.trim().starts_with(&format!("{op} ")))).count();
    assert_eq!(branches, 0, "a loop is left:\n{text}");
}
