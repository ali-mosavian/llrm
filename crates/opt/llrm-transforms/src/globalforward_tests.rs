use crate::testing::{parsed, printed, results};

/// `text` through `forwarded`: (printed, changed).
fn forwarded(text: &str) -> (String, bool) {
    let mut module = parsed(text);
    let before = results(&module, &[&[]]);
    let changed = !super::forwarded(&mut module).is_empty();
    let after = printed(&module);
    assert_eq!(results(&module, &[&[]]), before, "{after}");
    (after, changed)
}

/// A float stored once (QB's `$data` cell, split by `globalsra`) and read in
/// the loop below the store kept its load, and the loop carried it in a
/// register: currency_convert_pds71 m16 -Os +8 B over the same program with
/// the cell unsplit (the pin's second GlobalOpt reads the constant). The load
/// in `@g`, a longer body, took an instruction id past `@f`'s: qb/resume0.bas
/// panicked in the compiler.
#[test]
fn a_load_the_only_store_dominates_reads_the_stored_constant() {
    let (after, changed) = forwarded(
        "@c = internal global float 0.0

define float @f() {
b0:
  store float 18.0, ptr @c
  br label %b1
b1:
  %a = load float, ptr @c
  ret float %a
}

define float @g() {
b0:
  %x = fadd float 1.0, 2.0
  %y = fadd float %x, 2.0
  %z = fadd float %y, 2.0
  %w = fadd float %z, 2.0
  %a = load float, ptr @c
  ret float %a
}
",
    );
    assert!(changed && after.matches("load float").count() == 1, "{after}");
}

/// The pin forwards the store only to the loads it dominates.
#[test]
fn a_load_the_store_does_not_dominate_keeps_its_load() {
    let (after, changed) = forwarded(
        "@c = internal global float 0.0

define float @f() {
b0:
  %p = icmp eq i16 1, 1
  br i1 %p, label %b1, label %b2
b1:
  store float 18.0, ptr @c
  br label %b2
b2:
  %a = load float, ptr @c
  ret float %a
}
",
    );
    assert!(!changed && after.contains("load float"), "{after}");
}

/// Two stores: a load may read either, or the initializer.
#[test]
fn a_variable_stored_twice_keeps_its_loads() {
    let (after, changed) = forwarded(
        "@c = internal global float 0.0

define float @f() {
b0:
  store float 18.0, ptr @c
  %a = load float, ptr @c
  store float 2.0, ptr @c
  ret float %a
}
",
    );
    assert!(!changed && after.contains("load float"), "{after}");
}

/// The pin forwards constants only; a stored value is not known to the load.
#[test]
fn a_stored_value_that_is_not_a_constant_is_not_forwarded() {
    let (after, changed) = forwarded(
        "@c = internal global float 0.0

define float @f() {
b0:
  %x = fadd float 1.0, 2.0
  store float %x, ptr @c
  %a = load float, ptr @c
  ret float %a
}
",
    );
    assert!(!changed && after.contains("load float"), "{after}");
}
