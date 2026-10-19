//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::Outer;

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let options = Options { speculate_blocks: false, sink_common_insts: true, ..Default::default() };
    let mut module = parsed(text);
    let before = (!inputs.is_empty()).then(|| results(&module, inputs));
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(simplified_with(&options, context, &spec, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    if let Some(before) = before {
        assert_eq!(results(&module, inputs), before);
    }
}

/// Two arms end in `add x, 1; store` with different `x`: both run once in the
/// join, on a phi of the two (LLVM's `sink` of a column and its user); the
/// emptied arms then go and the phi is a select, as in `opt`.
#[test]
fn test_the_instructions_both_arms_end_with_run_once_in_the_join() {
    check(
        "define void @f(i16 %p, i16 %q, ptr %g, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %x = add i16 %p, 1
  store i16 %x, ptr %g
  br label %e

b2:
  %y = add i16 %q, 1
  store i16 %y, ptr %g
  br label %e

e:
  ret void
}
",
        "define void @f(i16 %p, i16 %q, ptr %g, i1 %c) {
b0:
  %0 = select i1 %c, i16 %p, i16 %q
  %y = add i16 %0, 1
  store i16 %y, ptr %g
  ret void
}
",
        &[],
    );
}

/// Different operations are not one.
#[test]
fn test_an_add_and_a_sub_are_not_sunk() {
    let text = "define i16 @f(i16 %p, i16 %q, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %x = add i16 %p, 1
  br label %e

b2:
  %y = sub i16 %q, 1
  br label %e

e:
  %z = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  ret i16 %z
}
";
    check(text, text, &[&[3, 4, 1], &[3, 4, 0]]);
}

/// A way into the join that is not a jump stays out of it: the jumps get a
/// block of their own to sink into, when what is sunk cannot run
/// speculatively (a store).
#[test]
fn test_a_conditional_way_in_makes_the_jumps_a_block_to_sink_into() {
    check(
        "define void @f(ptr %p, ptr %q, i1 %c, i1 %d) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 1, ptr %p
  br label %end

b2:
  br i1 %d, label %b3, label %end

b3:
  store i16 1, ptr %q
  br label %end

end:
  ret void
}
",
        "define void @f(ptr %p, ptr %q, i1 %c, i1 %d) {
b0:
  br i1 %c, label %0, label %b2

b2:
  br i1 %d, label %0, label %end

end:
  ret void

0:
  %sink = phi ptr [ %p, %b0 ], [ %q, %b2 ]
  store i16 1, ptr %sink
  br label %end
}
",
        &[],
    );
}

/// `store i16` and `store i8` are not the same operation: the operand types
/// differ (`isSameOperationAs`). Both stores were sunk on a phi of an `i16`
/// and an `i8`, which sccp's solver met at -O2 with `decide@832` wired.
#[test]
fn test_stores_of_different_widths_are_not_sunk_together() {
    const TEXT: &str = "define void @f(i16 %p, i8 %q, ptr %g, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %p, ptr %g
  br label %e

b2:
  store i8 %q, ptr %g
  br label %e

e:
  ret void
}
";
    check(TEXT, TEXT, &[]);
}
