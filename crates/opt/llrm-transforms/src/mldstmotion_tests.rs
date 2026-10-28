//! Stores to one address on both arms of a diamond.

use super::MergedLoadStoreMotion;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:32:32-i32:32-i64:32-n8:16:32\"\n\n";

fn run(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(MergedLoadStoreMotion);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    printed(&module)
}

/// The text of block `name`: its label through the line before the next label.
fn block(
    text: &str,
    name: &str,
) -> String {
    let mut lines = text.lines().skip_while(|one| !one.starts_with(&format!("{name}:")));
    let mut out = vec![lines.next().unwrap_or_else(|| panic!("no block {name} in\n{text}"))];
    out.extend(lines.take_while(|one| !one.is_empty() && !one.ends_with(':')));
    out.join("\n")
}

fn stores(text: &str) -> usize {
    text.lines().filter(|one| one.trim_start().starts_with("store ")).count()
}

const DIAMOND: &str = "define void @f(i1 %c, ptr %p) {
h:
  br i1 %c, label %t, label %e

t:
  store i32 1, ptr %p
  br label %j

e:
  store i32 2, ptr %p
  br label %j

j:
  ret void
}
";

/// Two stores of different values to one pointer stayed one in each arm: the
/// merged store below the diamond writes a phi of the two.
#[test]
fn test_stores_to_one_address_in_both_arms_become_one_below_with_a_phi() {
    let after = run(DIAMOND);
    assert_eq!(stores(&after), 1, "{after}");
    let tail = block(&after, "j");
    assert!(tail.contains("phi i32"), "{after}");
    assert!(tail.contains("store i32 %sink") || tail.contains("store i32 %"), "{after}");
}

/// A load of the stored bytes after the store in its arm may read what the
/// store wrote: moving the store below it would change what the load sees.
#[test]
fn test_a_read_of_the_bytes_after_the_store_blocks_the_merge() {
    let text = DIAMOND
        .replace(
            "  store i32 1, ptr %p\n  br label %j",
            "  store i32 1, ptr %p\n  %x = load i32, ptr %p\n  store i32 %x, ptr %q\n  br label %j",
        )
        .replace("ptr %p) {", "ptr %p, ptr %q) {");
    let after = run(&text);
    assert_eq!(stores(&after), 3, "{after}");
}

/// A volatile store is not simple: nothing merges it.
#[test]
fn test_volatile_stores_are_not_merged() {
    let text =
        DIAMOND.replace("store i32 1,", "store volatile i32 1,").replace("store i32 2,", "store volatile i32 2,");
    assert_eq!(stores(&run(&text)), 2);
}

/// A triangle (one arm empty) is not a diamond.
#[test]
fn test_a_triangle_is_left_alone() {
    let text = "define void @f(i1 %c, ptr %p) {
h:
  br i1 %c, label %t, label %j

t:
  store i32 1, ptr %p
  br label %j

j:
  store i32 2, ptr %p
  ret void
}
";
    assert_eq!(stores(&run(text)), 2);
}

/// The same value on both arms needs no phi.
#[test]
fn test_equal_values_make_no_phi() {
    let text = DIAMOND.replace("store i32 2,", "store i32 1,");
    let after = run(&text);
    assert_eq!(stores(&after), 1, "{after}");
    assert!(!after.contains("phi"), "{after}");
}

/// Two identical single-use address computations, each in its arm, go with the
/// store: one `getelementptr` and one store below the diamond.
#[test]
fn test_identical_geps_move_with_the_store() {
    let text = "define void @f(i1 %c, ptr %p) {
h:
  br i1 %c, label %t, label %e

t:
  %a = getelementptr i32, ptr %p, i32 4
  store i32 1, ptr %a
  br label %j

e:
  %b = getelementptr i32, ptr %p, i32 4
  store i32 2, ptr %b
  br label %j

j:
  ret void
}
";
    let after = run(text);
    assert_eq!(stores(&after), 1, "{after}");
    assert_eq!(after.matches("getelementptr").count(), 1, "{after}");
    assert!(block(&after, "j").contains("getelementptr"), "{after}");
}

/// A tail with a third predecessor is left alone (`no-split-footer-bb`).
#[test]
fn test_a_tail_with_three_predecessors_is_left_alone() {
    let text = "define void @f(i32 %n, ptr %p) {
h:
  %c = icmp eq i32 %n, 0
  br i1 %c, label %t, label %e

t:
  store i32 1, ptr %p
  br label %j

e:
  store i32 2, ptr %p
  br label %j

j:
  %d = icmp eq i32 %n, 1
  br i1 %d, label %x, label %j2

j2:
  ret void

x:
  ret void
}
";
    // The tail has two predecessors here, so it merges; three arrive in the
    // next case.
    assert_eq!(stores(&run(text)), 1);
    let three = "define void @f(i32 %n, ptr %p) {
g:
  switch i32 %n, label %j [ i32 7, label %h ]

h:
  %c = icmp eq i32 %n, 0
  br i1 %c, label %t, label %e

t:
  store i32 1, ptr %p
  br label %j

e:
  store i32 2, ptr %p
  br label %j

j:
  ret void
}
";
    assert_eq!(stores(&run(three)), 2);
}
