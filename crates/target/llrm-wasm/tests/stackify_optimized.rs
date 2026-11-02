//! `RegStackify` with optimization (`-O1` and up): definitions move to their
//! use and a value read twice gets a `local.tee`.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

fn asm(
    ir: &str,
    level: Level,
) -> String {
    assembly(ir, Width::W32, Options { level, ..Options::default() }).unwrap()
}

/// The body of `@name`: from the label to `end_function`.
fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = text.find(&format!("{name}:")).expect("the function");
    let end = text[start..].find("end_function").expect("its end");
    &text[start..start + end]
}

const SKIP: &str = "define i32 @f(i32 %x, ptr %p) {
entry:
  %a = add i32 %x, 1
  store i32 %x, ptr %p
  ret i32 %a
}
";

/// A value computed before an independent store stayed in a local
/// (`local.set` and `local.get` around the store) because only a definition
/// right above its use could stackify: fib's compare, 28 instructions a loop
/// trip against the pin's 24.
#[test]
fn a_definition_moves_past_an_independent_store_when_optimizing() {
    let o0 = asm(SKIP, Level::O0);
    let o1 = asm(SKIP, Level::O1);
    assert!(body(&o0, "f").contains("local.set"), "{o0}");
    assert!(!body(&o1, "f").contains("local.set"), "{o1}");
}

const TWICE: &str = "declare i32 @h(i32)
define i32 @g(i32 %x, i32 %y) {
entry:
  %a = add i32 %x, %y
  %b = call i32 @h(i32 %a)
  %c = add i32 %b, %a
  ret i32 %c
}
";

/// A value read twice was stored and read back (`local.set N; local.get N`);
/// the pin keeps the first read on the stack with `local.tee`.
#[test]
fn a_value_read_twice_is_a_tee_when_optimizing() {
    let o0 = asm(TWICE, Level::O0);
    let o2 = asm(TWICE, Level::O2);
    assert!(!body(&o0, "g").contains("local.tee"), "{o0}");
    assert!(body(&o2, "g").contains("local.tee"), "{o2}");
}

const AFTER_STORE: &str = "define i32 @h(ptr %p, ptr %q) {
entry:
  %a = load i32, ptr %p
  store i32 1, ptr %q
  %b = add i32 %a, 1
  ret i32 %b
}
";

/// A load may not move past a store that can alias it.
#[test]
fn a_load_does_not_move_past_a_store() {
    let o2 = asm(AFTER_STORE, Level::O2);
    let text = body(&o2, "h");
    let load = text.find("i32.load").expect("the load");
    let store = text.find("i32.store").expect("the store");
    assert!(load < store, "{o2}");
}

const TWO_STORES: &str = "define void @s(ptr %p) {
entry:
  store i16 1, ptr %p
  %q = getelementptr i8, ptr %p, i32 2
  store i16 2, ptr %q
  ret void
}
";

/// The selection DAG was combined as at `-O0` on every level (`OptLevel ==
/// None`), so `MergeConsecutiveStores` never ran: matmul kept 94 `i32.store16`
/// where the pin has 26 wider stores, 101 bytes more at `-O2`.
#[test]
fn adjacent_stores_merge_when_optimizing() {
    let o0 = asm(TWO_STORES, Level::O0);
    let o2 = asm(TWO_STORES, Level::O2);
    assert!(body(&o0, "s").matches("i32.store16").count() == 2, "{o0}");
    assert!(!body(&o2, "s").contains("i32.store16"), "{o2}");
}

const SEQUENTIAL: &str = "declare void @h(i32)
define void @k(i32 %x) {
entry:
  %a = add i32 %x, 1
  call void @h(i32 %a)
  call void @h(i32 %a)
  %b = add i32 %x, 2
  call void @h(i32 %b)
  call void @h(i32 %b)
  ret void
}
";

/// The locals of the function: the count after `.local`.
fn locals(
    text: &str,
    name: &str,
) -> usize {
    body(text, name).lines().find(|l| l.contains(".local")).map_or(0, |l| l.matches("i32").count())
}

/// Two values that are never live together each had a local of their own:
/// `RegColoring` only ran in the pin. fib's loop carried 5 locals where the
/// pin's has 3.
#[test]
fn values_that_are_not_live_together_share_a_local_when_optimizing() {
    let o0 = asm(SEQUENTIAL, Level::O0);
    let o2 = asm(SEQUENTIAL, Level::O2);
    assert_eq!(locals(&o0, "k"), 2, "{o0}");
    assert_eq!(locals(&o2, "k"), 1, "{o2}");
}
