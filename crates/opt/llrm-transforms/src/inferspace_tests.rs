//! A far pointer cast from DGROUP's near one is read as the near one; each case's text is the pass's input.

use crate::inferspace::InferAddressSpaces;
use crate::testing::{managed, parsed};

const HEAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32\"\n\n@g = global [64 x i16] zeroinitializer\ndeclare void @sink(ptr addrspace(1))\n\n";

fn run(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    managed(&mut module, InferAddressSpaces)
}

/// Nib's `total(table, i)` read `a[i]` through `les bx, [bp+4]; es:[bx+si]`, a segment loaded and
/// held for a pointer the caller made from a global.
#[test]
fn a_load_through_a_step_of_a_cast_global_is_a_near_load() {
    let after = run("define i16 @f(i16 %i) {
b0:
  %w = addrspacecast ptr @g to ptr addrspace(1)
  %a = getelementptr i8, ptr addrspace(1) %w, i16 %i
  %v = load i16, ptr addrspace(1) %a
  ret i16 %v
}
");
    assert!(after.contains("load i16, ptr %") && !after.contains("load i16, ptr addrspace(1)"), "{after}");
}

/// A cast of a stack object makes SS:offset, which a near pointer, read through DS, is not.
#[test]
fn a_cast_stack_object_stays_far() {
    let after = run("define i16 @f(i16 %i) {
b0:
  %s = alloca [4 x i16]
  %w = addrspacecast ptr %s to ptr addrspace(1)
  %a = getelementptr i8, ptr addrspace(1) %w, i16 %i
  %v = load i16, ptr addrspace(1) %a
  ret i16 %v
}
");
    assert!(after.contains("load i16, ptr addrspace(1) %a"), "{after}");
}

/// A pointer a call reads needs its selector: it stays far, and the load beside it reads the near one.
#[test]
fn a_step_a_call_reads_keeps_its_selector() {
    let after = run("define i16 @f(i16 %i) {
b0:
  %w = addrspacecast ptr @g to ptr addrspace(1)
  %a = getelementptr i8, ptr addrspace(1) %w, i16 %i
  call void @sink(ptr addrspace(1) %a)
  %v = load i16, ptr addrspace(1) %a
  ret i16 %v
}
");
    assert!(after.contains("call void @sink(ptr addrspace(1) %a)") && !after.contains("load i16, ptr addrspace(1)"), "{after}");
}

/// A walking pointer: its phi and its step are near where nothing reads them whole but a load.
#[test]
fn a_pointer_that_walks_a_cast_global_walks_near() {
    let after = run("define i16 @f(i16 %n) {
b0:
  %w = addrspacecast ptr @g to ptr addrspace(1)
  br label %loop
loop:
  %p = phi ptr addrspace(1) [ %w, %b0 ], [ %q, %loop ]
  %i = phi i16 [ 0, %b0 ], [ %j, %loop ]
  %v = load i16, ptr addrspace(1) %p
  %q = getelementptr i8, ptr addrspace(1) %p, i16 2
  %j = add i16 %i, 1
  %c = icmp slt i16 %j, %n
  br i1 %c, label %loop, label %out
out:
  ret i16 %v
}
");
    assert!(after.contains("phi ptr [") && after.contains("load i16, ptr %"), "{after}");
}

/// Comparing two far pointers wants both selectors: a compare is no read through memory, the phi stays far.
#[test]
fn a_phi_a_compare_reads_stays_far() {
    let after = run("define i16 @f(i16 %n) {
b0:
  %w = addrspacecast ptr @g to ptr addrspace(1)
  br label %loop
loop:
  %p = phi ptr addrspace(1) [ %w, %b0 ], [ %q, %loop ]
  %v = load i16, ptr addrspace(1) %p
  %q = getelementptr i8, ptr addrspace(1) %p, i16 2
  %c = icmp ne ptr addrspace(1) %p, %w
  br i1 %c, label %loop, label %out
out:
  ret i16 %v
}
");
    assert!(!after.contains("phi ptr ["), "{after}");
}
