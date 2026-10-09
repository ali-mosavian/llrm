//! Guards proven by the branches over a block.

use super::{guards, holds};
use crate::induction::{AffineOperand, Scev};
use crate::induction::tests::Parsed;
use llrm_mir::opcode::IntPredicate;

const GUARDED: &str = "define i16 @f(i16 %n, i16 %len) {
entry:
  %fits = icmp ult i16 %n, %len
  br i1 %fits, label %loop, label %done
loop:
  br label %done
done:
  ret i16 0
}
";

/// The edge taken into `loop` proves `n < len` there, and what that
/// implies, either way round; not at the join, which the other edge reaches.
#[test]
fn test_a_branch_proves_its_compare_where_its_edge_alone_leads() {
    let parsed = Parsed::new(GUARDED);
    let unit = parsed.unit();
    let (n, len) = (Scev::of(&AffineOperand::Value(parsed.value("n"), 16), 16), Scev::of(&AffineOperand::Value(parsed.value("len"), 16), 16));
    let at = |name: &str| block_named(&parsed, name);
    assert!(holds(&unit, at("loop"), IntPredicate::Ult, &n, &len));
    assert!(holds(&unit, at("loop"), IntPredicate::Ule, &n, &len));
    assert!(holds(&unit, at("loop"), IntPredicate::Ugt, &len, &n));
    assert!(!holds(&unit, at("loop"), IntPredicate::Ult, &len, &n));
    assert!(guards(&unit, at("done")).is_empty());
}

fn block_named(parsed: &Parsed, name: &str) -> i64 {
    let function = parsed.function();
    crate::cfg::id(function.layout().iter().copied().find(|&block| function.block(block).name.as_deref() == Some(name)).expect(name))
}

/// What a block assumes holds in every block it dominates, as an edge's
/// branch does: a check lowered away, or a language promise, was no guard.
#[test]
fn test_an_assume_proves_its_compare_below_its_block() {
    let parsed = Parsed::new(
        "declare void @llvm.assume(i1)

define i16 @f(i16 %n, i16 %len, i1 %c) {
entry:
  %fits = icmp ult i16 %n, %len
  call void @llvm.assume(i1 %fits)
  br i1 %c, label %loop, label %done
loop:
  br label %done
done:
  ret i16 0
}
",
    );
    let unit = parsed.unit();
    let (n, len) = (Scev::of(&AffineOperand::Value(parsed.value("n"), 16), 16), Scev::of(&AffineOperand::Value(parsed.value("len"), 16), 16));
    for name in ["loop", "done"] {
        assert!(holds(&unit, block_named(&parsed, name), IntPredicate::Ult, &n, &len), "{name}");
    }
    assert!(guards(&unit, block_named(&parsed, "entry")).is_empty(), "not in its own block: earlier code is not covered");
}

/// `n != 0` proves `0 < n`, `n > 0` and `n >= 1`: the guard a copied loop test leaves for a counter that starts at zero
/// (bench/floats with `-ftree-ch`: the symbolic trip count was lost behind `icmp ne i16 %n, 0`).
#[test]
fn test_a_guard_that_n_is_not_zero_proves_zero_below_n() {
    let parsed = Parsed::new(
        "define i16 @f(i16 %n) {
entry:
  %any = icmp ne i16 %n, 0
  br i1 %any, label %loop, label %done
loop:
  br label %done
done:
  ret i16 0
}
",
    );
    let unit = parsed.unit();
    let n = Scev::of(&AffineOperand::Value(parsed.value("n"), 16), 16);
    let zero = Scev::constant(0, 16);
    let at = block_named(&parsed, "loop");
    assert!(holds(&unit, at, IntPredicate::Ult, &zero, &n));
    assert!(holds(&unit, at, IntPredicate::Ugt, &n, &zero));
    assert!(!holds(&unit, at, IntPredicate::Slt, &zero, &n), "n may be negative");
    assert!(!holds(&unit, block_named(&parsed, "done"), IntPredicate::Ult, &zero, &n));
}

/// A guard in the narrow type proves the same unsigned compare of its zero extension: `gap < 64` in i16 is `zext gap < 64` in i32, which is
/// what a widened counter starting at `gap` tests (bench/shellsort with `-ftree-ch`: the inner loop's symbolic count was lost).
#[test]
fn test_a_guard_on_a_narrow_value_proves_the_compare_of_its_zero_extension() {
    let parsed = Parsed::new(
        "define i16 @f(i16 %gap) {
entry:
  %wide = zext i16 %gap to i32
  %fits = icmp ult i16 %gap, 64
  br i1 %fits, label %loop, label %done
loop:
  br label %done
done:
  ret i16 0
}
",
    );
    let unit = parsed.unit();
    let wide = Scev::of(&AffineOperand::Value(parsed.value("wide"), 32), 32);
    let at = block_named(&parsed, "loop");
    assert!(holds(&unit, at, IntPredicate::Ult, &wide, &Scev::constant(64, 32)));
    assert!(!holds(&unit, at, IntPredicate::Ult, &wide, &Scev::constant(32, 32)));
    assert!(!holds(&unit, block_named(&parsed, "done"), IntPredicate::Ult, &wide, &Scev::constant(64, 32)));
}

/// A phi of the loop entry is tested on each edge into it: `lo < hi` where the loop is first entered and `next < hi` on the edge back.
/// Both prove `phi < hi` in the body, though no branch dominates it (the tail-recursion loop of quicksort's `sort` with `-ftree-ch`).
#[test]
fn test_a_phi_tested_on_every_edge_into_its_block_is_proven_below_it() {
    let text = |back_edge_tested: bool| {
        format!(
            "define i32 @f(i32 %lo, i32 %hi) {{
entry:
  %first = icmp slt i32 %lo, %hi
  br i1 %first, label %body, label %done
body:
  %p = phi i32 [ %lo, %entry ], [ %next, %latch ]
  %next = add i32 %p, 1
  br label %latch
latch:
  %again = icmp slt i32 %next, {}
  br i1 %again, label %body, label %done
done:
  ret i32 0
}}
",
            if back_edge_tested { "%hi" } else { "100" }
        )
    };
    for (tested, expected) in [(true, true), (false, false)] {
        let parsed = Parsed::new(&text(tested));
        let unit = parsed.unit();
        let (p, hi) = (Scev::of(&AffineOperand::Value(parsed.value("p"), 32), 32), Scev::of(&AffineOperand::Value(parsed.value("hi"), 32), 32));
        assert_eq!(holds(&unit, block_named(&parsed, "body"), IntPredicate::Slt, &p, &hi), expected, "back edge tested against hi: {tested}");
    }
}
