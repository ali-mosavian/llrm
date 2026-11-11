//! Loop strength reduction under wasm's `Tti`.

use std::rc::Rc;

use llrm_mir::passes::PassManager;
use llrm_wasm::machine::WasmMachine;

const RECPOW: &str = "\
target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"

declare fastcc i32 @power(i32, i32) nounwind memory(none)

define i32 @f(i32 %n) nounwind {
entry:
  %c = icmp slt i32 %n, 1
  br i1 %c, label %out, label %pre
pre:
  br label %loop
loop:
  %i = phi i32 [ %next, %loop ], [ 1, %pre ]
  %s = phi i32 [ %sum, %loop ], [ 0, %pre ]
  %r = urem i32 %i, 97
  %b = add nuw nsw i32 %r, 2
  %e = add nuw nsw i32 %i, 40
  %p = tail call fastcc i32 @power(i32 noundef %b, i32 noundef %e)
  %t = trunc nsw i32 %p to i16
  %m = srem i16 %t, 1000
  %x = sext i16 %m to i32
  %sum = add nsw i32 %s, %x
  %next = add nuw i32 %i, 1
  %d = icmp eq i32 %i, %n
  br i1 %d, label %exit, label %loop
exit:
  br label %out
out:
  %z = phi i32 [ 0, %entry ], [ %sum, %exit ]
  ret i32 %z
}
";

/// wasm's `Tti` left `isLegalAddImmediate`/`isLegalICmpImmediate` at the base
/// `false`, where the pin's `WebAssemblyTTIImpl` (a `BasicTTIImplBase`) takes
/// the lowering's `true`: LSR never rewrote `i + 40` as `lsr.iv + 41`, and
/// recpow's code section was 22 bytes (282 vs 260) over the pin's.
#[test]
fn an_immediate_offset_of_the_counter_is_legal() {
    let mut module = llrm_mir::parse::module(RECPOW).expect("parses");
    let mut manager = PassManager::default();
    let elements = llrm_transforms::pipeline_text::parse("loop-reduce").expect("pipeline");
    llrm_transforms::pipeline_text::build(&elements, &mut manager).ok().expect("builds");
    manager.run_module(&mut module, Rc::new(WasmMachine)).expect("runs");
    let text = llrm_mir::print::module(&module);
    assert!(text.contains(", 41\n") && !text.contains(", 40\n"), "{text}");
}

/// `for (i < n) a[i] = b[i]` unrolled by four: the epilog's address base and
/// its count-down are one counter in LG3's solution (`-2 * xtraiter` up to
/// zero), which needs an add of the base to the counter in each of the two
/// accesses; the pin's two counters need none (wasm32 -O2: 110 instructions
/// against the pin's 102, idioms +310 B). LG3 adds a base to the counter in
/// both accesses.
#[test]
fn the_epilog_of_a_copy_keeps_the_pins_two_counters() {
    let mut module = llrm_mir::parse::module(include_str!("lsr/epilog_copy.ll")).expect("parses");
    let mut manager = PassManager::default();
    let elements = llrm_transforms::pipeline_text::parse("loop-reduce").expect("pipeline");
    llrm_transforms::pipeline_text::build(&elements, &mut manager).ok().expect("builds");
    manager.run_module(&mut module, Rc::new(WasmMachine)).expect("runs");
    let text = llrm_mir::print::module(&module);
    assert!(!text.contains("mul nsw i32 %xtraiter, -2"), "{text}");
}

/// jpeg_fixed `emit` (-O1): the loop counted `-len` up to zero and hoisted
/// `ptr + len` for its address, +16 instructions on the pin's pointer and
/// `len` counted down.
#[test]
fn a_sum_over_a_buffer_keeps_the_pointer_and_the_down_counter() {
    let mut module = llrm_mir::parse::module(include_str!("lsr/adler_loop.ll")).expect("parses");
    let mut manager = PassManager::default();
    let elements = llrm_transforms::pipeline_text::parse("loop-reduce").expect("pipeline");
    llrm_transforms::pipeline_text::build(&elements, &mut manager).ok().expect("builds");
    manager.run_module(&mut module, Rc::new(WasmMachine)).expect("runs");
    let text = llrm_mir::print::module(&module);
    assert!(!text.contains("getelementptr i8, ptr %1, i32 %2"), "{text}");
}

/// Four streams stepped by a four-trip inner loop: the end-biased counter
/// (LG3) compares against zero but needs four `base + 32` in the preheader,
/// where the pin keeps the count up to 32 and the bases as they are (nbody and
/// fpbench -O1: +25 B each).
#[test]
fn a_counter_keeps_the_bases_when_the_end_biased_one_costs_setup() {
    let mut module = llrm_mir::parse::module(include_str!("lsr/four_streams.ll")).expect("parses");
    let mut manager = PassManager::default();
    let elements = llrm_transforms::pipeline_text::parse("loop-reduce").expect("pipeline");
    llrm_transforms::pipeline_text::build(&elements, &mut manager).ok().expect("builds");
    manager.run_module(&mut module, Rc::new(WasmMachine)).expect("runs");
    let text = llrm_mir::print::module(&module);
    assert!(text.contains(", 32\n") && !text.contains("i32 32\n  %scevgep"), "{text}");
}

/// bench/quicksort's order check (`a[i - 1] > a[i]`, i = 1..1023): the end-biased
/// formulae tied the pin's solution on instructions plus setup (11 against 11)
/// and won on the compare immediate, counting `-4092` up to zero with two
/// constants set up outside the loop and a negative-offset load: +10 B (wasm32
/// quicksort O1/O2/O3, spill_store_order +5).
#[test]
fn a_tie_of_instructions_and_setup_keeps_the_pin_counting_up() {
    let mut module = llrm_mir::parse::module(include_str!("lsr/order_check.ll")).expect("parses");
    let mut manager = PassManager::default();
    let elements = llrm_transforms::pipeline_text::parse("loop-reduce").expect("pipeline");
    llrm_transforms::pipeline_text::build(&elements, &mut manager).ok().expect("builds");
    manager.run_module(&mut module, Rc::new(WasmMachine)).expect("runs");
    let text = llrm_mir::print::module(&module);
    let exit = text.lines().find(|line| line.contains("icmp eq")).expect("an exit test");
    assert!(!exit.trim_end().ends_with(", 0"), "{exit}\n{text}");
}
