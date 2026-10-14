use std::collections::BTreeSet;

use super::*;
use crate::graph::loops;
use crate::memory::Unit;
use crate::testing::{DOS, function, layout, parsed};

/// `for (j = 0; j < n; j++) a[j + 2] = a[j]`, `a` an `i32` array.
const SHIFT: &str = "@a = global [64 x i32] zeroinitializer
define void @f(i32 %n) {
b0:
  br label %b1
b1:
  %j = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  %more = icmp slt i32 %j, %n
  br i1 %more, label %b2, label %b3
b2:
  %off = mul i32 %j, 4
  %from = getelementptr i8, ptr @a, i32 %off
  %v = load i32, ptr %from
  %up = add i32 %j, 2
  %offup = mul i32 %up, 4
  %to = getelementptr i8, ptr @a, i32 %offup
  store i32 %v, ptr %to
  %next = add i32 %j, 1
  br label %b1
b3:
  ret void
}
";

fn listed(body: &str) -> Vec<Reference> {
    let module = parsed(&format!("{DOS}{body}"));
    let layout = layout(&module);
    let function = function(&module, "f");
    let unit = crate::testing::with_registers(Unit::of(&module, &layout, function));
    let found = loops::loops(&crate::cfg::graph(function), None);
    let [one] = &found[..] else { panic!("one loop") };
    references(&unit, &[one], &one.body, &one.body, &mut Roots::default()).expect("affine accesses")
}

/// A counter walks `a` four bytes a trip: the read is at 0 and the write two
/// elements on, both stepping 4 in the inner loop, on one identified object.
#[test]
fn an_array_walked_by_a_counter_is_an_affine_access() {
    let found = listed(SHIFT);
    let shape = |one: &Reference| (one.access.subscripts[0], one.access.width, one.access.write, one.identified);
    assert_eq!(
        found.iter().map(shape).collect::<Vec<_>>(),
        vec![
            (Subscript { constant: 0, outer: 0, inner: 4 }, 4, false, true),
            (Subscript { constant: 8, outer: 0, inner: 4 }, 4, true, true),
        ]
    );
    assert_eq!(found[0].access.object, found[1].access.object);
    assert_eq!(found.iter().map(|one| one.inst).collect::<BTreeSet<_>>().len(), 2);
}

/// A pointer parameter is a root, not an object's own address.
#[test]
fn a_pointer_parameter_is_a_root_that_is_not_identified() {
    let found = listed(
        &SHIFT
            .replace("@a = global [64 x i32] zeroinitializer\n", "")
            .replace("@f(i32 %n)", "@f(i32 %n, ptr %a)")
            .replace("ptr @a", "ptr %a"),
    );
    assert!(found.iter().all(|one| !one.identified));
    assert_eq!(found[0].access.object, found[1].access.object);
}

/// A call in the loop may touch any of it: nothing is listed.
#[test]
fn a_call_in_the_loop_lists_nothing() {
    let module = parsed(&format!(
        "{DOS}declare void @g()\n{}",
        SHIFT.replace("  %next = add i32 %j, 1\n", "  call void @g()\n  %next = add i32 %j, 1\n")
    ));
    let layout = layout(&module);
    let function = function(&module, "f");
    let unit = crate::testing::with_registers(Unit::of(&module, &layout, function));
    let found = loops::loops(&crate::cfg::graph(function), None);
    assert!(references(&unit, &[&found[0]], &found[0].body, &found[0].body, &mut Roots::default()).is_none());
}
