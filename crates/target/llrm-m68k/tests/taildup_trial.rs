//! gcd -O1 on the 68000 copied the loop's `cmp; bne` into both arms: 32 B
//! against 26 B without the copies, 2 clk slower, so 4 B behind gcc's 28. A
//! copy is kept only where the build with it beats the build without.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, cpu_of, lowered_for};

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

const GCD: &str = "define i32 @k(i32 %a, i32 %b) {
entry:
  %e = icmp eq i32 %a, %b
  br i1 %e, label %done, label %loop
loop:
  %x = phi i32 [%a, %entry], [%x1, %latch]
  %y = phi i32 [%b, %entry], [%y1, %latch]
  %g = icmp sgt i32 %x, %y
  br i1 %g, label %subx, label %suby
subx:
  %d1 = sub i32 %x, %y
  br label %latch
suby:
  %d2 = sub i32 %y, %x
  br label %latch
latch:
  %x1 = phi i32 [%d1, %subx], [%x, %suby]
  %y1 = phi i32 [%y, %subx], [%d2, %suby]
  %ne = icmp ne i32 %x1, %y1
  br i1 %ne, label %loop, label %done
done:
  %r = phi i32 [%a, %entry], [%x1, %latch]
  ret i32 %r
}
";

/// clang -O1 IR of the library bsearch (the fixture of `jump_thread_o1`): its
/// latch compare is copied into both arms, 4 B more and 20 clk fewer at the
/// 68000.
const BSEARCH: &str = include_str!("fixtures/bsearch_o1_clang.ll");

/// bsearch through the mid-end of `level`, then the backend with or without
/// the copies, priced as the level prices.
fn bsearch(
    level: Level,
    tail_duplicate: bool,
) -> String {
    let cpu = cpu_of("68000").unwrap();
    let module = lowered_for(BSEARCH, cpu, level).expect("lowers");
    let options = Options { mid_end: false, strict: level == Level::O1, tail_duplicate, ..Options::default() };
    text_with(&module, "k", cpu, options).expect("compiles")
}

fn text(options: Options) -> String {
    let module = llrm_mir::parse::module(&format!("{LAYOUT}{GCD}")).expect("parses");
    text_with(&module, "k", Cpu::M68000, options).expect("compiles")
}

#[test]
fn a_copy_that_makes_the_loop_larger_without_a_gain_is_not_kept_at_o1() {
    let strict = Options { strict: true, ..Options::default() };
    let kept = text(strict);
    let plain = text(Options { tail_duplicate: false, ..strict });
    assert_eq!(kept, plain, "{kept}");
}

#[test]
fn a_copy_of_4_bytes_for_20_clocks_is_kept_where_clocks_are_priced() {
    let kept = bsearch(Level::O2, true);
    assert_ne!(kept, bsearch(Level::O2, false), "{kept}");
}

#[test]
fn the_same_copy_is_kept_at_o1_where_clocks_are_priced_too() {
    let kept = bsearch(Level::O1, true);
    assert_ne!(kept, bsearch(Level::O1, false), "{kept}");
}
