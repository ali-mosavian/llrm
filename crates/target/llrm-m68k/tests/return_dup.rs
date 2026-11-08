//! absdiff -O2 on the 68000 ran `bra` to a shared `rts` (10 clk) where the
//! pin's `llc` has an `rts` in each arm: +4 rate against gcc. The pin's
//! post-allocation tail duplication copies a block that only returns
//! (`shouldTailDuplicate`: no successors, at most `TailDupSize` instructions)
//! into each predecessor.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(i32 %a, i32 %b) {
entry:
  %c = icmp sgt i32 %a, %b
  br i1 %c, label %gt, label %le
gt:
  %x = sub i32 %a, %b
  br label %done
le:
  %y = sub i32 %b, %a
  br label %done
done:
  %r = phi i32 [%x, %gt], [%y, %le]
  ret i32 %r
}
";

fn text(options: Options) -> String {
    let module = llrm_mir::parse::module(IR).expect("parses");
    text_with(&module, "k", Cpu::M68000, options).expect("compiles")
}

#[test]
fn a_return_block_is_copied_into_each_arm_instead_of_branched_to() {
    let text = text(Options::default());
    assert_eq!(text.matches("rts").count(), 2, "{text}");
    assert!(!text.contains("bra\t"), "{text}");
}

#[test]
fn a_size_build_keeps_the_one_shared_return() {
    let text = text(Options { size: true, ..Options::default() });
    assert_eq!(text.matches("rts").count(), 1, "{text}");
}

const SAVING: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @ext(i32)
define i32 @g(i32 %a, i32 %b) {
entry:
  %x = call i32 @ext(i32 %a)
  %y = call i32 @ext(i32 %b)
  %c = icmp sgt i32 %x, %y
  br i1 %c, label %gt, label %le
gt:
  %p = sub i32 %x, %y
  br label %done
le:
  %q = sub i32 %y, %x
  br label %done
done:
  %r = phi i32 [%p, %gt], [%q, %le]
  ret i32 %r
}
";

/// A copy of the return is the function's epilogue (the saved registers, the
/// frame): it grows the code where the shared one does not. Where neither bytes
/// nor clocks may grow (-O1, `strict`) the build without the copy is kept;
/// where clocks buy bytes (-O2) the copy stays.
#[test]
fn a_return_whose_epilogue_costs_bytes_is_copied_only_where_clocks_pay_for_them() {
    let module = llrm_mir::parse::module(SAVING).expect("parses");
    let build = |options| text_with(&module, "g", Cpu::M68000, options).expect("compiles");
    let strict = build(Options { strict: true, ..Options::default() });
    assert_eq!(strict.matches("rts").count(), 1, "{strict}");
    let speed = build(Options::default());
    assert_eq!(speed.matches("rts").count(), 2, "{speed}");
}
