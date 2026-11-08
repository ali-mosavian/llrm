//! The coalescer joins the copies of values that live in several blocks with
//! the others (`JoinGlobalCopies`: the allocator's tuning, not the pin's M68k).
//! Neither order is smaller for every function: a function that saves a
//! register is built both ways and the cheaper kept (46 cells of the 68000
//! corpus got smaller, 23 at the 68020). The pin's order wins only in whole
//! programs (shellsort, recmany); the corpus A/B covers that direction.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;

const FIB: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(i32 %n) {
entry:
  br label %loop
loop:
  %a = phi i32 [0, %entry], [%b, %loop]
  %b = phi i32 [1, %entry], [%s, %loop]
  %i = phi i32 [0, %entry], [%i1, %loop]
  %s = add i32 %a, %b
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s
}
";

fn bytes(
    ir: &str,
    name: &str,
    options: Options,
) -> usize {
    let module = llrm_mir::parse::module(ir).expect("parses");
    let items = items_with(&module, name, Cpu::M68000, options).expect("compiles");
    llrm_m68k::object::assemble(&items, 0).expect("assembles").code.len()
}

/// The tuned order is smaller here; the pin's order is tried and dropped.
#[test]
fn the_tuned_join_order_is_kept_where_it_builds_smaller() {
    let options = Options { size: true, ..Options::default() };
    let pin = bytes(FIB, "k", Options { global_copies: false, ..options });
    assert!(bytes(FIB, "k", options) < pin);
}
