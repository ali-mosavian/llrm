//! An `llvm.fmuladd` of a loop whose factors the loop does not write is an
//! `fmul` before the loop and an `fadd` in it, where the target asks.

use crate::licm::Licm;
use crate::testing::{parsed, printed};

const TEXT: &str = "declare float @llvm.fmuladd.f32(float, float, float)

define float @f(float %r, float %k, ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %in, %loop ]
  %acc = phi float [ 0.0, %entry ], [ %sum, %loop ]
  %a = getelementptr float, ptr %p, i32 %i
  %x = load float, ptr %a
  %y = call float @llvm.fmuladd.f32(float %r, float %k, float %x)
  %z = call float @llvm.fmuladd.f32(float %x, float %k, float %acc)
  %sum = fadd float %y, %z
  %in = add i32 %i, 1
  %done = icmp eq i32 %in, %n
  br i1 %done, label %exit, label %loop
exit:
  ret float %sum
}
";

/// i486 with the hook as asked, over a machine that states nothing else.
struct Fused(bool);

impl llrm_mir::target::Machine for Fused {
    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        let mut tti = llrm_x86::tti::X86::of_cpu("i486").expect("i486");
        tti.tuning = llrm_mir::target::Tuning { splits_invariant_fmuladd: self.0, ..llrm_mir::target::Tuning::PIN };
        Some(std::rc::Rc::new(tti))
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }

    fn foreign_span(
        &self,
        a: (i64, i64),
        b: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(a, b, width)
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::Neutral.costs()
    }

    fn registers(&self) -> i64 {
        0
    }

    fn call_registers(&self) -> i64 {
        0
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }
}

/// `TEXT` through LICM on a target that `splits`.
fn licmed(splits: bool) -> String {
    let mut module = parsed(TEXT);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(Fused(splits))).unwrap_or_else(|error| panic!("{error}"));
    printed(&module)
}

/// x_ld_to_ulong m32 -O1: clang's `x[i] + r * K` was a `fmuladd` LICM could not
/// split, and `r * K` was made on every trip of the inner loop (+1,719 clk;
/// gcc holds it). Where the target asks the product of `%r` and `%k` is made
/// once, before the loop; a call with a factor the loop changes (`%x`) stays.
#[test]
fn an_invariant_product_of_a_fmuladd_leaves_the_loop_where_the_target_asks() {
    let split = licmed(true);
    let (head, body) = split.split_once("loop:").expect("a loop");
    assert!(head.contains("fmul float %r, %k"), "the product is before the loop:\n{split}");
    assert!(!body.contains("fmul float %r, %k"), "and not in it:\n{split}");
    assert!(body.contains("@llvm.fmuladd.f32(float %x, float %k"), "a varying product stays fused:\n{split}");
}

/// The pin's order is the target's default: the call stays whole in the loop.
#[test]
fn a_fmuladd_stays_whole_where_the_target_does_not_ask() {
    let kept = licmed(false);
    let (head, body) = kept.split_once("loop:").expect("a loop");
    assert!(!head.contains("fmul float"), "{kept}");
    assert!(body.contains("@llvm.fmuladd.f32(float %r, float %k"), "{kept}");
}
