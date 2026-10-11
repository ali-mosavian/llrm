//! `lsr`'s pointer for a counter some trips step, on loops the interpreter
//! runs.

use std::collections::BTreeSet;

use llrm_analysis::testing::DOS;
use llrm_mir::target::AddressForm;

use crate::lsr::Lsr;
use crate::profit::OperationCosts;
use crate::testing::{Tuned, managed_on, parsed, results};

/// The 16-bit target's word address: no scale.
fn bare() -> Tuned {
    let word = AddressForm {
        partners: Some(2),
        bases: Some(1),
        indices: Some(2),
        ..AddressForm::new(2, BTreeSet::from([1]), 0, 0, 0, false, None).expect("a form")
    };
    let costs = OperationCosts {
        add: 1,
        multiply: 13,
        shift: 2,
        address: 1,
        load: 1,
        store: 1,
        memory_update: 3,
        extend: 3,
        prefix: 1,
        ..OperationCosts::default()
    };
    Tuned { costs, registers: 6, call_registers: 2, address_forms: vec![word], ..Tuned::default() }
}

/// The same with the scale a 32-bit address takes.
fn scaled() -> Tuned {
    let mut machine = bare();
    machine.address_forms[0] = AddressForm::new(2, BTreeSet::from([1, 2, 4, 8]), 0, 0, 0, false, None).expect("a form");
    machine
}

/// quicksort's partition: `i` is stepped on the trips that swap, and read as
/// `a[i]`. The array's checksum is the result, so that a wrong address shows.
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

const INPUTS: &[&[i128]] = &[&[0, 1], &[0, 2], &[0, 9], &[2, 7], &[5, 6], &[3, 30], &[0, 0], &[4, 2]];

fn through(machine: Tuned) -> String {
    let mut module = parsed(&format!("{DOS}{PARTITION}"));
    let expected = results(&module, INPUTS);
    let after = managed_on(&mut module, Lsr::default(), machine);
    assert_eq!(results(&module, INPUTS), expected, "{after}");
    after
}

/// quicksort -m16 shuffled bx between `a[j]` and `a[i]`: both were a base
/// register and an offset, for want of a scale. A pointer that moves with `i`
/// reads `[p]`.
#[test]
fn test_a_counter_some_trips_step_has_a_pointer_where_the_address_takes_no_scale() {
    let after = through(bare());
    assert!(after.contains("phi ptr"), "{after}");
    // The accesses read the pointer, not `@a` plus `i * 2`.
    assert!(!after.contains("load i16, ptr %pi") && !after.contains("store i16 %j, ptr %pi"), "{after}");
}

#[test]
fn test_an_address_that_takes_the_scale_is_left() {
    let after = through(scaled());
    assert!(!after.contains("phi ptr"), "{after}");
}

#[test]
fn test_a_build_for_size_keeps_the_scaled_address() {
    let mut module = parsed(&format!("{DOS}{PARTITION}"));
    let after = managed_on(&mut module, Lsr { size: true, ..Lsr::default() }, bare());
    assert!(!after.contains("phi ptr"), "{after}");
}
