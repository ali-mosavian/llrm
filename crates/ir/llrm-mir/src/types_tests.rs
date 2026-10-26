use super::{Type, Types};

/// A width with a direct slot must name the id the table gave it first,
/// whatever order widths are asked in, through `int` or `intern`, and in a
/// clone.
#[test]
fn test_an_integer_width_has_one_id_however_it_is_asked() {
    let types = Types::default();
    let (i32_, i1) = (types.int(32), types.int(1));
    assert_ne!(i32_, i1);
    assert_eq!(types.intern(Type::Int(32)), i32_);
    assert_eq!(types.int(1), i1);
    let wide = types.int(65);
    assert_eq!(types.int(65), wide);
    assert_eq!(types.get(wide), &Type::Int(65));
    let copy = types.clone();
    assert_eq!((copy.int(32), copy.int(1), copy.int(65)), (i32_, i1, wide));
    let ptr = types.ptr(0);
    assert_ne!(ptr, i32_);
    assert_eq!(types.get(types.int(0)), &Type::Int(0));
}
