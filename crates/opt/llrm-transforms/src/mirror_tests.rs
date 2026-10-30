//! The pointer for a counter some trips step, on loops the interpreter runs.

use std::collections::BTreeSet;

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;
use llrm_mir::target::AddressForm;

use crate::testing::{parsed, printed, results};

/// The 16-bit target's word address: no scale.
fn bare() -> Vec<AddressForm> {
    vec![AddressForm::new(2, BTreeSet::from([1]), 0, 0, 0, false, None).expect("a form")]
}

/// The same with the scale a 32-bit address takes.
fn scaled() -> Vec<AddressForm> {
    vec![AddressForm::new(2, BTreeSet::from([1, 2, 4, 8]), 0, 0, 0, false, None).expect("a form")]
}

const PARTITION: &str = "@a = global [64 x i16] zeroinitializer

define i16 @f(i16 %lo, i16 %hi) {
b0:
  %some = icmp slt i16 %lo, %hi
  br i1 %some, label %pre, label %out

pre:
  br label %b1

b1:
  %j = phi i16 [ %lo, %pre ], [ %j1, %b3 ]
  %i = phi i16 [ %lo, %pre ], [ %i2, %b3 ]
  %acc = phi i16 [ 0, %pre ], [ %acc2, %b3 ]
  %o = mul i16 %j, 2
  %pj = getelementptr inbounds i8, ptr @a, i16 %o
  %v = and i16 %j, 3
  %c = icmp slt i16 %v, 2
  br i1 %c, label %b2, label %b3

b2:
  %oi = mul i16 %i, 2
  %pi = getelementptr inbounds i8, ptr @a, i16 %oi
  %w = load i16, ptr %pi
  store i16 %j, ptr %pi
  store i16 %w, ptr %pj
  %i1 = add nsw i16 %i, 1
  %sum = add i16 %acc, %w
  br label %b3

b3:
  %i2 = phi i16 [ %i, %b1 ], [ %i1, %b2 ]
  %acc2 = phi i16 [ %acc, %b1 ], [ %sum, %b2 ]
  %j1 = add nsw i16 %j, 1
  %more = icmp slt i16 %j1, %hi
  br i1 %more, label %b1, label %done

done:
  %i3 = phi i16 [ %i2, %b3 ]
  %acc3 = phi i16 [ %acc2, %b3 ]
  %r = add i16 %i3, %acc3
  br label %out

out:
  %res = phi i16 [ 0, %b0 ], [ %r, %done ]
  ret i16 %res
}
";

/// The same loop with the pin's canonical addresses: `gep [2 x i8], @a, i`,
/// which is what clang hands a C `a[i]`.
fn typed() -> String {
    PARTITION
        .replace(
            "  %o = mul i16 %j, 2\n  %pj = getelementptr inbounds i8, ptr @a, i16 %o",
            "  %pj = getelementptr inbounds [2 x i8], ptr @a, i16 %j",
        )
        .replace(
            "  %oi = mul i16 %i, 2\n  %pi = getelementptr inbounds i8, ptr @a, i16 %oi",
            "  %pi = getelementptr inbounds [2 x i8], ptr @a, i16 %i",
        )
}

const INPUTS: &[&[i128]] = &[&[0, 1], &[0, 2], &[0, 9], &[2, 7], &[5, 6], &[3, 30], &[0, 0], &[4, 2]];

/// (whether the pass changed `body`, the function after), the interpreter's
/// results unchanged.
fn through(
    body: &str,
    forms: &[AddressForm],
) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{body}"));
    let expected = results(&module, INPUTS);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = crate::mirror::mirrored(context, &layout, function, &outer, forms);
    let after = printed(&module);
    assert_eq!(results(&module, INPUTS), expected, "{after}");
    (changed, after)
}

/// quicksort -m16 shuffled bx between `a[j]` and `a[i]` (nib's swap block ran
/// 10 memory operands a swap where main's ran 4): `i` is stepped by some trips,
/// so no loop pass gave `a[i]` a pointer.
#[test]
fn test_a_counter_some_trips_step_has_a_pointer_where_the_address_takes_no_scale() {
    let (changed, after) = through(PARTITION, &bare());
    assert!(changed && after.contains("phi ptr"), "{after}");
    assert!(!after.contains("load i16, ptr %pi") && !after.contains("store i16 %j, ptr %pi"), "{after}");
}

/// C's `a[i]` is `gep [2 x i8], a, i`, not a multiply into a byte gep: the
/// matcher found no loop in quicksort.c and the pass changed none of its rows.
#[test]
fn test_a_typed_gep_of_the_counter_has_a_pointer_too() {
    let (changed, after) = through(&typed(), &bare());
    assert!(changed && after.contains("phi ptr"), "{after}");
}

#[test]
fn test_an_address_that_takes_the_scale_is_left() {
    for body in [PARTITION.to_string(), typed()] {
        let (changed, after) = through(&body, &scaled());
        assert!(!changed && !after.contains("phi ptr"), "{after}");
    }
}
