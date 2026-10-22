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
