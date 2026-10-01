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
