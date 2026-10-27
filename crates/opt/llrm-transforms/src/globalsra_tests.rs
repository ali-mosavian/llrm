use crate::testing::{parsed, printed, results};

/// The x_fir shape, run through `split`: coefficients stored as `i32` pairs by
/// the `Merge` slot, read as `i16`.
#[test]
fn a_global_stored_as_wide_constants_and_loaded_as_halves_is_split_and_localized() {
    let text = "@c = internal global [8 x i16] zeroinitializer

define i16 @f() norecurse {
b0:
  store i32 65537, ptr @c
  store i32 131075, ptr getelementptr inbounds (i8, ptr @c, i32 4)
  %a = load i16, ptr @c
  %b = load i16, ptr getelementptr inbounds (i8, ptr @c, i32 2)
  %d = load i16, ptr getelementptr inbounds (i8, ptr @c, i32 6)
  %s = add i16 %a, %b
  %t = add i16 %s, %d
  ret i16 %t
}
";
    let mut module = parsed(text);
    let before = results(&module, &[&[]]);
    let layout = llrm_analysis::testing::layout(&module);
    let changed = super::split(&mut module, &layout);
    let after = printed(&module);
    assert_eq!(results(&module, &[&[]]), before, "{after}");
    assert!(!changed.is_empty(), "{after}");
    assert!(!after.contains("ptr @c"), "{after}");
    assert_eq!(after.matches("alloca i16").count(), 3, "{after}");
}

/// `text` through `split`: (printed, changed).
fn splitted(text: &str) -> (String, bool) {
    let mut module = parsed(text);
    let before = results(&module, &[&[]]);
    let layout = llrm_analysis::testing::layout(&module);
    let changed = !super::split(&mut module, &layout).is_empty();
    let after = printed(&module);
    assert_eq!(results(&module, &[&[]]), before, "{after}");
    (after, changed)
}

/// A load no store dominates reads the initializer: the piece stays a global.
#[test]
fn a_piece_read_before_it_is_written_stays_a_global() {
    let (after, changed) = splitted(
        "@c = internal global [2 x i16] [i16 7, i16 9]

define i16 @f() norecurse {
b0:
  %a = load i16, ptr @c
  store i16 1, ptr @c
  %b = load i16, ptr getelementptr inbounds (i8, ptr @c, i32 2)
  %s = add i16 %a, %b
  ret i16 %s
}
",
    );
    assert!(changed && !after.contains("alloca"), "{after}");
    assert!(
        after.contains("@c.0 = internal global i16 7") && after.contains("@c.1 = internal global i16 9"),
        "{after}"
    );
}

/// A body that may recurse sees its own earlier call's value: no slot.
#[test]
fn a_piece_of_a_body_that_may_recurse_is_not_a_stack_slot() {
    let (after, _) = splitted(
        "@c = internal global [2 x i16] zeroinitializer

define i16 @f() {
b0:
  store i16 1, ptr @c
  %a = load i16, ptr @c
  %b = load i16, ptr getelementptr inbounds (i8, ptr @c, i32 2)
  %s = add i16 %a, %b
  ret i16 %s
}
",
    );
    assert!(!after.contains("alloca"), "{after}");
}

/// Two bodies share the pieces: no slot.
#[test]
fn a_piece_two_bodies_use_is_not_a_stack_slot() {
    let (after, changed) = splitted(
        "@c = internal global [2 x i16] zeroinitializer

define void @g() norecurse {
b0:
  store i16 3, ptr @c
  ret void
}

define i16 @f() norecurse {
b0:
  store i16 1, ptr @c
  %a = load i16, ptr @c
  %b = load i16, ptr getelementptr inbounds (i8, ptr @c, i32 2)
  %s = add i16 %a, %b
  ret i16 %s
}
",
    );
    assert!(changed && !after.contains("alloca"), "{after}");
}

/// Loads of one offset at two types are not split, as `collectSRATypes`; the
/// load of the stored type is still the stored constant
/// (`forwardStoredOnceStore`).
#[test]
fn loads_of_one_offset_at_two_types_leave_the_global_alone() {
    let (after, changed) = splitted(
        "@c = internal global [2 x i16] zeroinitializer

define i16 @f() norecurse {
b0:
  store i16 1, ptr @c
  %a = load i16, ptr @c
  %w = load i32, ptr @c
  %t = trunc i32 %w to i16
  %s = add i16 %a, %t
  ret i16 %s
}
",
    );
    assert!(changed && after.contains("@c = internal global [2 x i16]") && !after.contains("alloca"), "{after}");
    assert!(!after.contains("%a = load") && after.contains("%w = load i32"), "{after}");
}

/// A scalar the frontend spelled as bytes is one piece that is all of it: no
/// split. d_faces' `fp_cnt`, `fp_tex` and `fp_ofs` were renamed, moved in the
/// data and cost 623 bytes.
#[test]
fn a_scalar_spelled_as_bytes_is_not_split_into_itself() {
    let (after, changed) = splitted(
        "@c = internal global [2 x i8] zeroinitializer

define void @g(i16 %x) {
b0:
  store i16 %x, ptr @c
  ret void
}

define i16 @f() {
b0:
  %a = load i16, ptr @c
  ret i16 %a
}
",
    );
    assert!(!changed && after.contains("@c = internal global [2 x i8]") && !after.contains("@c.0"), "{after}");
}

/// A 16-bit scalar spelled as `[2 x i8]` (QB's `S%`) that one body writes
/// before it reads kept its global: the whole-variable piece of another type
/// was no split, so it never became a slot (hotlop.bas m16 -Os kept `S%`, +2 B
/// of data and a store and a reload past the runtime's calls; hotlpx, lngmix,
/// press and rotate likewise).
#[test]
fn a_scalar_spelled_as_bytes_is_localized() {
    let text = "@s = internal global [2 x i8] zeroinitializer

define i16 @f() norecurse {
b0:
  store i16 630, ptr @s
  %r = load i16, ptr @s
  ret i16 %r
}
";
    let (after, changed) = splitted(text);
    assert!(changed, "{after}");
    assert!(!after.contains("ptr @s"), "{after}");
    assert_eq!(after.matches("alloca i16").count(), 1, "{after}");
}

/// `forwardStoredOnceStore`: a global stored once with a constant gives that
/// constant to the loads in the storing function the store dominates. `scores`
/// kept `mov word ptr G$25, 6 ... mov cx, word ptr G$25` after its `$data` was
/// split into pieces (+75 B at m16 -Os): the load is the constant.
#[test]
fn a_load_the_one_store_dominates_reads_the_stored_constant() {
    let (after, changed) = splitted(
        "@g = internal global i16 0

define i16 @f() {
b0:
  %early = load i16, ptr @g
  store i16 6, ptr @g
  %a = load i16, ptr @g
  %s = add i16 %a, %early
  ret i16 %s
}

define i16 @h() {
b0:
  %b = load i16, ptr @g
  ret i16 %b
}
",
    );
    assert!(changed, "{after}");
    assert!(after.contains("%early = load i16, ptr @g"), "the load before the store stays: {after}");
    assert!(!after.contains("%a = load"), "{after}");
    assert!(after.contains("%b = load i16, ptr @g"), "another function's load stays: {after}");
}

/// Two stores, or a store of the initializer, are not a store of one value.
#[test]
fn a_global_stored_twice_or_with_its_initializer_forwards_nothing() {
    let (after, _) = splitted(
        "@g = internal global i16 0
@k = internal global i16 3

define i16 @f() {
b0:
  store i16 6, ptr @g
  store i16 7, ptr @g
  store i16 3, ptr @k
  %a = load i16, ptr @g
  %c = load i16, ptr @k
  %s = add i16 %a, %c
  ret i16 %s
}
",
    );
    assert!(after.contains("%a = load") && after.contains("%c = load"), "{after}");
}
