//! A value an address register would hold across a loop only to be copied into
//! one after it: memcpy -O2 on the 68000 kept `d` (the base of `d[12]` after
//! the copy loop) in `%a0` beside the loop's two pointers, so the pointer took
//! the callee-saved `%a2` and the function saved and restored it
//! (+40 rate against gcc, which holds `d` in a data register). The pin joins
//! the copy and saves `%a2` too.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

/// Two pointers step in the loop, one data register counts; `d` is read only
/// after it.
const COPY: &str = "define i32 @k(ptr %d, ptr %s, i32 %n) {
entry:
  %e = icmp sgt i32 %n, 0
  br i1 %e, label %loop, label %done
loop:
  %m = phi i32 [%n, %entry], [%m1, %loop]
  %p = phi ptr [%d, %entry], [%p1, %loop]
  %q = phi ptr [%s, %entry], [%q1, %loop]
  %v = load i8, ptr %q
  store i8 %v, ptr %p
  %p1 = getelementptr i8, ptr %p, i32 1
  %q1 = getelementptr i8, ptr %q, i32 1
  %m1 = sub i32 %m, 1
  %c = icmp sgt i32 %m1, 0
  br i1 %c, label %loop, label %done
done:
  %a = getelementptr i8, ptr %d, i32 12
  %r = load i8, ptr %a
  %x = sext i8 %r to i32
  ret i32 %x
}
";

/// One pointer steps; `d` is read only after the loop. Holding `d` in a data
/// register saves nothing here (a scratch address register is free for it) and
/// costs a `move.l`.
const SUM: &str = "define i32 @k(ptr %d, ptr %s, i32 %n) {
entry:
  %e = icmp sgt i32 %n, 0
  br i1 %e, label %loop, label %done
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %t = phi i32 [0, %entry], [%t1, %loop]
  %q = getelementptr i32, ptr %s, i32 %i
  %v = load i32, ptr %q
  %t1 = add i32 %t, %v
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %done
done:
  %u = phi i32 [0, %entry], [%t1, %loop]
  %a = getelementptr i8, ptr %d, i32 12
  %r = load i8, ptr %a
  %x = sext i8 %r to i32
  %y = add i32 %x, %u
  ret i32 %y
}
";

fn text(
    body: &str,
    options: Options,
) -> String {
    let module = llrm_mir::parse::module(&format!("{LAYOUT}{body}")).expect("parses");
    text_with(&module, "k", Cpu::M68000, options).expect("compiles")
}

#[test]
fn a_value_used_only_after_the_loop_is_not_held_in_an_address_register_across_it() {
    let text = text(COPY, Options::default());
    assert!(!text.contains("%a2"), "{text}");
}

#[test]
fn the_hold_is_dropped_where_it_saves_nothing() {
    let strict = Options { strict: true, ..Options::default() };
    let with = text(SUM, strict);
    let without = text(SUM, Options { class_split: false, ..strict });
    assert_eq!(with, without, "{with}");
}
