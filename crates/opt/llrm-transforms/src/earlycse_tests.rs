//! Tests of the EarlyCSE translation (`early-cse<memssa>`): what its scoped
//! tables and generation count do.

use llrm_analysis::testing::DOS;

use super::EarlyCse;
use crate::testing::{managed, parsed};

fn run(body: &str) -> String {
    managed(&mut parsed(&format!("{DOS}{body}")), EarlyCse)
}

/// A commuted add was a second add, which `gvn` found much later.
#[test]
fn test_a_commuted_operation_is_the_one_before() {
    let after = run("define i32 @f(i32 %x, i32 %y) {
b0:
  %a = add i32 %x, %y
  %b = add i32 %y, %x
  %c = mul i32 %a, %b
  ret i32 %c
}
");
    assert_eq!(after.matches("add i32").count(), 1, "{after}");
}

/// A compare with its operands exchanged and the predicate swapped is the same
/// value.
#[test]
fn test_a_swapped_compare_is_the_one_before() {
    let after = run("define i1 @f(i32 %x, i32 %y) {
b0:
  %a = icmp slt i32 %x, %y
  %b = icmp sgt i32 %y, %x
  %c = and i1 %a, %b
  ret i1 %c
}
");
    assert_eq!(after.matches("icmp").count(), 1, "{after}");
}

/// A select on the inverted condition with its arms exchanged is the same
/// value.
#[test]
fn test_an_inverted_select_is_the_one_before() {
    let after = run("define i32 @f(i32 %x, i32 %y, i32 %p, i32 %q) {
b0:
  %c1 = icmp slt i32 %x, %y
  %c2 = icmp sge i32 %x, %y
  %a = select i1 %c1, i32 %p, i32 %q
  %b = select i1 %c2, i32 %q, i32 %p
  %r = add i32 %a, %b
  ret i32 %r
}
");
    assert_eq!(after.matches("select").count(), 1, "{after}");
}

/// A load of a cell just stored is the stored value.
#[test]
fn test_a_load_after_a_store_is_the_stored_value() {
    let after = run("define i32 @f(ptr %p, i32 %v) {
b0:
  store i32 %v, ptr %p
  %l = load i32, ptr %p
  ret i32 %l
}
");
    assert!(!after.contains("load") && after.contains("ret i32 %v"), "{after}");
}

/// Two loads with no write between are one.
#[test]
fn test_a_second_load_is_the_first() {
    let after = run("define i32 @f(ptr %p) {
b0:
  %a = load i32, ptr %p
  %b = load i32, ptr %p
  %r = add i32 %a, %b
  ret i32 %r
}
");
    assert_eq!(after.matches("load").count(), 1, "{after}");
}

/// Of two stores to a cell with nothing reading it between, the first is dead.
#[test]
fn test_a_store_overridden_by_the_next_is_removed() {
    let after = run("define void @f(ptr %p) {
b0:
  store i32 1, ptr %p
  store i32 2, ptr %p
  ret void
}
");
    assert_eq!(after.matches("store").count(), 1, "{after}");
    assert!(after.contains("store i32 2"), "{after}");
}

/// A call that may write the cell ends what was loaded from it.
#[test]
fn test_a_load_is_not_forwarded_across_a_call_that_may_write() {
    let after = run("declare void @opaque(ptr)

define i32 @f(ptr %p) {
b0:
  %a = load i32, ptr %p
  call void @opaque(ptr %p)
  %b = load i32, ptr %p
  %r = add i32 %a, %b
  ret i32 %r
}
");
    assert_eq!(after.matches("load").count(), 2, "{after}");
}

/// The branch taken tells the condition: in the arm a repeat of it is a
/// constant.
#[test]
fn test_a_condition_is_known_in_the_arm_it_chose() {
    let after = run("define i1 @f(i32 %x) {
b0:
  %c = icmp eq i32 %x, 0
  br i1 %c, label %t, label %e

t:
  %d = icmp eq i32 %x, 0
  ret i1 %d

e:
  ret i1 false
}
");
    assert!(after.contains("ret i1 true"), "{after}");
}

/// `CallValue::canHandle` takes a call that only reads or only writes memory.
/// A callee that also touches `inaccessiblemem` is neither, and LLVM's
/// `early-cse<memssa>` keeps both calls of `@a` and `@b` below (checked with
/// the pinned `opt`, 24.0.0git d1106deb71cc) while it takes the second of
/// `@c`'s. `call_key` reads `memory::of`, which counts every location; the
/// narrower `accessible` summary would take `@b` for a reader and `@a` for a
/// call that touches nothing, and drop the second call of each.
#[test]
fn test_a_call_that_touches_inaccessible_memory_is_not_a_read_only_or_write_only_call() {
    let after = run("declare i32 @a(ptr) memory(inaccessiblemem: readwrite)
declare i32 @b(ptr) memory(argmem: read, inaccessiblemem: write)
declare i32 @c(ptr) memory(argmem: read)

define i32 @f(ptr %p) {
b0:
  %a1 = call i32 @a(ptr %p)
  %a2 = call i32 @a(ptr %p)
  %b1 = call i32 @b(ptr %p)
  %b2 = call i32 @b(ptr %p)
  %c1 = call i32 @c(ptr %p)
  %c2 = call i32 @c(ptr %p)
  %s1 = add i32 %a1, %a2
  %s2 = add i32 %b1, %b2
  %s3 = add i32 %c1, %c2
  %s4 = add i32 %s1, %s2
  %s5 = add i32 %s4, %s3
  ret i32 %s5
}
");
    assert_eq!(after.matches("call i32 @a").count(), 2, "{after}");
    assert_eq!(after.matches("call i32 @b").count(), 2, "{after}");
    assert_eq!(after.matches("call i32 @c").count(), 1, "{after}");
}

/// `simplifyInstruction` was a stub of a few identities, so a compare that only
/// known bits decide (`(x & 7) u< 8`) stayed in the function.
#[test]
fn test_a_compare_the_known_bits_decide_is_folded() {
    let after = run("define i1 @f(i32 %x) {
b0:
  %m = and i32 %x, 7
  %c = icmp ult i32 %m, 8
  ret i1 %c
}
");
    assert!(!after.contains("icmp"), "{after}");
}

/// A compare implied by the branch that dominates it is the branch's answer
/// (`isImpliedByDomCondition`, which needs the dominator tree in the query).
#[test]
fn test_a_compare_the_dominating_branch_implies_is_folded() {
    let after = run("define i32 @f(i32 %x) {
b0:
  %a = icmp ult i32 %x, 10
  br i1 %a, label %b1, label %b2
b1:
  %c = icmp ult i32 %x, 20
  %r = zext i1 %c to i32
  ret i32 %r
b2:
  ret i32 0
}
");
    assert_eq!(after.matches("icmp").count(), 1, "{after}");
}
