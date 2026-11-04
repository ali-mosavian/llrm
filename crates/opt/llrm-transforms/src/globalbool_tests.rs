use crate::testing::{parsed, printed, results};

/// `text` through `shrink`: (printed, functions changed). Results must not
/// move.
fn shrunk(text: &str) -> (String, usize) {
    let mut module = parsed(text);
    let before = results(&module, &[&[0], &[7]]);
    let changed = super::shrink(&mut module).len();
    let after = printed(&module);
    assert_eq!(results(&module, &[&[0], &[7]]), before, "{after}");
    (after, changed)
}

/// jpeg_fixed's `bad`: an `i32` the program only ever sets to 1 stayed a 4-byte
/// word and a `cmp` of it (+18 B at wasm32 -O3). It is an `i1`, read by a
/// `zext`.
#[test]
fn a_global_set_once_to_one_from_zero_becomes_a_bool_read_by_zext() {
    let (after, changed) = shrunk(
        "@bad = internal global i32 0

define i32 @f(i32 %x) {
b0:
  %a = load i32, ptr @bad
  store i32 1, ptr @bad
  %b = load i32, ptr @bad
  %s = add i32 %a, %b
  ret i32 %s
}
",
    );
    assert_eq!(changed, 1, "{after}");
    assert!(after.contains("@bad = internal global i1 false"), "{after}");
    assert!(after.contains("store i1 true, ptr @bad"), "{after}");
    assert_eq!(after.matches("zext i1 %").count(), 2, "{after}");
}

/// Any other pair of values selects between them.
#[test]
fn a_global_with_two_values_is_read_by_a_select_of_both() {
    let (after, _) = shrunk(
        "@m = internal global i16 5

define i16 @f(i16 %x) {
b0:
  store i16 9, ptr @m
  %a = load i16, ptr @m
  ret i16 %a
}
",
    );
    assert!(after.contains("@m = internal global i1 false"), "{after}");
    assert!(after.contains("select i1 %a.b, i16 9, i16 5"), "{after}");
}

/// A store of a value loaded from the variable copies the `i1`, not the select.
#[test]
fn a_store_of_a_loaded_copy_copies_the_bool() {
    let (after, _) = shrunk(
        "@g = internal global i32 0

define i32 @f(i32 %x) {
b0:
  store i32 1, ptr @g
  %a = load i32, ptr @g
  store i32 %a, ptr @g
  %b = load i32, ptr @g
  ret i32 %b
}
",
    );
    assert!(after.contains("store i1 %a.b, ptr @g"), "{after}");
}

/// Three values, an address that escapes, an `i1` and a never-loaded variable
/// all stay as they are.
#[test]
fn a_global_that_is_not_a_two_valued_variable_is_left_alone() {
    for text in [
        "@g = internal global i32 0
define i32 @f(i32 %x) {
b0:
  store i32 1, ptr @g
  store i32 2, ptr @g
  %a = load i32, ptr @g
  ret i32 %a
}
",
        "@g = internal global i32 0
define void @take(ptr %p) {
b0:
  ret void
}
define i32 @f(i32 %x) {
b0:
  store i32 1, ptr @g
  call void @take(ptr @g)
  %a = load i32, ptr @g
  ret i32 %a
}
",
        "@g = internal global i1 false
define i1 @f(i32 %x) {
b0:
  store i1 true, ptr @g
  %a = load i1, ptr @g
  ret i1 %a
}
",
        "@g = internal global i32 0
define i32 @f(i32 %x) {
b0:
  store i32 1, ptr @g
  ret i32 %x
}
",
        "@g = global i32 0
define i32 @f(i32 %x) {
b0:
  store i32 1, ptr @g
  %a = load i32, ptr @g
  ret i32 %a
}
",
    ] {
        let (after, changed) = shrunk(text);
        assert_eq!(changed, 0, "{after}");
    }
}
