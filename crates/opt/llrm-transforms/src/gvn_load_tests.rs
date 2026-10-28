//! `processLoad` and `processNonLocalLoad` of `GVNPass` (`gvn::pass::loads`):
//! a load served by a store or a load before it, locally and across blocks,
//! with the value coerced where the sizes differ. Each case runs the
//! interpreter before and after.

use llrm_analysis::testing::m16_ir;
use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use crate::gvn::pass::run_impl_cfg;
use crate::testing::{parsed, printed, results};

const XY: &[&[i128]] = &[&[0, 0], &[3, 5], &[-7, 2], &[0x7fff, 1]];
const XYC: &[&[i128]] = &[&[0, 0, 0], &[3, 5, 1], &[-7, 2, 0], &[0x7fff, 1, 1]];

/// `body`'s @f after the pass, with `@g` a global of 16 bytes; whether it
/// changed.
fn loaded(
    body: &str,
    inputs: &[&[i128]],
) -> (String, bool) {
    loaded_with(body, inputs, true)
}

/// As `loaded`; `pre` says whether load PRE is on.
fn loaded_with(
    body: &str,
    inputs: &[&[i128]],
    pre: bool,
) -> (String, bool) {
    let text = format!("@g = global [8 x i16] zeroinitializer\n@h = global [8 x i16] zeroinitializer\n\n{body}");
    let before = parsed(&text);
    let mut module: Module = before.clone();
    let layout = llrm_analysis::testing::layout(&module);
    let outer = Outer::of(&module, None);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = run_impl_cfg(context, &layout, function, &outer, pre, None).0;
    let printed = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{printed}");
    (printed, changed)
}

#[test]
fn test_a_load_after_a_store_of_the_same_bytes_is_the_stored_value() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @g
  %r = load i16, ptr @g
  ret i16 %r
}
",
        XY,
    );
    assert!(changed && text.contains("ret i16 %x") && !text.contains("load"), "{text}");
}

#[test]
fn test_two_loads_of_the_same_bytes_are_one() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = load i16, ptr @g
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}
",
        XY,
    );
    assert!(changed && text.matches("load").count() == 1, "{text}");
}

#[test]
fn test_a_store_that_may_alias_keeps_the_second_load() {
    let (text, _) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %p = getelementptr i8, ptr @g, i16 %y
  %a = load i16, ptr @g
  store i16 %x, ptr %p
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}
",
        &[&[1, 2]],
    );
    assert_eq!(text.matches("load").count(), 2, "{text}");
}

#[test]
fn test_a_volatile_load_is_kept() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @g
  %r = load volatile i16, ptr @g
  ret i16 %r
}
",
        XY,
    );
    assert!(!changed && text.contains("load volatile"), "{text}");
}

#[test]
fn test_a_load_nothing_uses_is_removed() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %unused = load i16, ptr @g
  ret i16 %x
}
",
        XY,
    );
    assert!(changed && !text.contains("load"), "{text}");
}

#[test]
fn test_a_load_below_a_join_takes_the_value_each_arm_stored() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %x, ptr @g
  br label %b3

b2:
  store i16 %y, ptr @g
  br label %b3

b3:
  %r = load i16, ptr @g
  ret i16 %r
}
",
        XYC,
    );
    assert!(changed && !text.contains("load") && text.contains("phi i16 [ %y, %b2 ], [ %x, %b1 ]"), "{text}");
}

const PARTLY: &str = "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %x, ptr @g
  br label %b3

b2:
  br label %b3

b3:
  %r = load i16, ptr @g
  ret i16 %r
}
";

#[test]
fn test_a_load_partly_available_is_loaded_in_the_predecessor_that_lacks_it() {
    let (text, changed) = loaded(PARTLY, XYC);
    assert!(changed && text.contains("%r.pre = load i16, ptr @g") && text.contains("phi i16"), "{text}");
    assert!(!text.contains("b3:\n  %r = load"), "{text}");
}

/// x_heapsort's sift loop: the child's element is loaded on the arm that chose
/// it by a select of two indexes, not on the other arm. The GEPs off `@g` are
/// in blocks that dominate both arms, but the translation of the address into
/// an arm found none (a global has no use list), so the load stayed in the
/// join: one load more per trip.
const SELECTED: &str = "define i16 @f(i16 %i, i1 %c) {
b0:
  %p0 = getelementptr i16, ptr @g, i16 %i
  br i1 %c, label %b1, label %b2

b1:
  %v0 = load i16, ptr %p0
  %j = add i16 %i, 1
  %p1 = getelementptr i16, ptr @g, i16 %j
  %v1 = load i16, ptr %p1
  %lt = icmp slt i16 %v0, %v1
  %s = select i1 %lt, i16 %j, i16 %i
  br label %b3

b2:
  br label %b3

b3:
  %k = phi i16 [ %s, %b1 ], [ %i, %b2 ]
  %pk = getelementptr i16, ptr @g, i16 %k
  %r = load i16, ptr %pk
  ret i16 %r
}
";

#[test]
fn test_a_load_of_a_global_array_element_chosen_by_a_select_is_loaded_in_the_arm_that_lacks_it() {
    let (text, changed) = loaded(SELECTED, &[&[0, 1], &[0, 0], &[3, 1], &[3, 0]]);
    assert!(changed && text.contains(".pre = load i16, ptr %p0"), "{text}");
    assert!(!text.contains("b3:\n  %k = phi i16 [ %s, %b1 ], [ %i, %b2 ]\n  %pk"), "{text}");
}

/// The unrelated arm must not acquire a read that could fault or observe
/// memory: the load goes on the edge, taken or not.
#[test]
fn test_a_missing_load_on_a_conditional_edge_gets_its_own_block() {
    for branch in ["br i1 %d, label %b3, label %b4", "br i1 %d, label %b4, label %b3"] {
        let (text, changed) = loaded(
            &format!(
                "define i16 @f(i1 %c, i1 %d, i16 %v, i16 %s) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  %y = load i16, ptr @g
  %z = add i16 %y, 1
  br label %b3

b2:
  {branch}

b3:
  %x = load i16, ptr @g
  ret i16 %x

b4:
  ret i16 0
}}
"
            ),
            &[&[1, 1, 7, 0], &[1, 0, 7, 0], &[0, 1, 7, 0], &[0, 0, 7, 0]],
        );
        assert!(changed, "{text}");
        assert!(!text.contains("b2:\n  br label %b3"), "the edge is split, not b2's own:\n{text}");
        assert!(text.contains("load i16, ptr @g\n  br label %b3"), "{text}");
    }
}

#[test]
fn test_a_load_partly_available_is_left_where_pre_is_off() {
    let (text, _) = loaded_with(PARTLY, XYC, false);
    assert!(text.contains("%r = load i16, ptr @g") && !text.contains(".pre"), "{text}");
}

#[test]
fn test_a_loop_header_load_of_an_invariant_address_is_loaded_in_the_preheader_and_after_the_clobber() {
    let (text, changed) = loaded(
        "define void @w() {
b0:
  store i16 5, ptr @h
  ret void
}

define i16 @f(i16 %x, i16 %n, i1 %c) {
b0:
  %a = load i16, ptr @h
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b3 ]
  %s = phi i16 [ %a, %b0 ], [ %t, %b3 ]
  %r = load i16, ptr @h
  %t = add i16 %s, %r
  %skip = icmp eq i16 %i, 1
  br i1 %skip, label %b2, label %b3

b2:
  call void @w()
  br label %b3

b3:
  %j = add i16 %i, 1
  %more = icmp slt i16 %j, %n
  br i1 %more, label %b1, label %b4

b4:
  ret i16 %t
}
",
        &[&[0, 3, 1], &[0, 1, 0], &[2, 5, 1]],
    );
    assert!(changed, "{text}");
    assert_eq!(text.matches("load i16, ptr @h").count(), 2, "the preheader's and the one after the call:\n{text}");
}

#[test]
fn test_a_load_of_a_part_of_a_wider_store_is_cut_from_it() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %w = zext i16 %x to i32
  store i32 %w, ptr @g
  %r = load i16, ptr @g
  ret i16 %r
}
",
        XY,
    );
    assert!(changed && !text.contains("load"), "{text}");
}

#[test]
fn test_a_load_of_stack_memory_nothing_wrote_is_poison() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %slot = alloca i16
  %r = load i16, ptr %slot
  ret i16 %x
}
",
        XY,
    );
    assert!(changed && !text.contains("load"), "{text}");
}

#[test]
fn test_a_load_after_a_memset_is_the_byte_repeated() {
    let (text, changed) = loaded(
        "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)

define i16 @f(i16 %x, i16 %y) {
b0:
  call void @llvm.memset.p0.i16(ptr @g, i8 7, i16 16, i1 false)
  %r = load i16, ptr @g
  ret i16 %r
}
",
        XY,
    );
    assert!(changed && !text.contains("load i16"), "{text}");
}

/// A store through a `getelementptr` constant expression of the same global
/// does not clobber a load of another part: BasicAA read only the instruction
/// GEP as a GEPOperator, so x_fir's 16 unrolled loads of `coef[k]` after 16
/// constant stores all stayed (the pin's GVN leaves 3) and Fold's numbers had
/// to carry them.
#[test]
fn test_a_store_through_a_constant_gep_of_the_global_leaves_a_load_of_another_part() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @g
  store i16 %y, ptr getelementptr inbounds nuw (i8, ptr @g, i32 2)
  store i16 %x, ptr getelementptr inbounds nuw (i8, ptr @g, i32 4)
  %a = load i16, ptr @g
  %b = load i16, ptr getelementptr inbounds nuw (i8, ptr @g, i32 2)
  %r = add i16 %a, %b
  ret i16 %r
}
",
        XY,
    );
    assert!(changed && !text.contains("load"), "{text}");
}

/// lru.c: a pointer stored at `@c+8` and `@c+12` was loaded again through
/// `addrspacecast (@c)` +8 (the far alias): the cast constant was no
/// base to look through, so the store at +12 might alias the load. +101 B at
/// -Os m16, and the pin forwards.
#[test]
fn test_a_load_through_a_far_cast_of_a_global_sees_the_store_to_its_own_offset() {
    let mut module = parsed(
        m16_ir("target datalayout = \"@m16-datalayout@-ni:2:3\"
@c = internal global [48 x i8] zeroinitializer
@b = internal global [6 x i8] zeroinitializer
define i32 @f(i16 %0) {
b1:
  store ptr addrspace(1) addrspacecast (ptr @b to ptr addrspace(1)), ptr getelementptr inbounds nuw (i8, ptr @c, i16 8)
  store ptr addrspace(1) addrspacecast (ptr @b to ptr addrspace(1)), ptr getelementptr inbounds nuw (i8, ptr @c, i16 12)
  %1 = load ptr addrspace(1), ptr addrspace(1) getelementptr inbounds nuw (i8, ptr addrspace(1) addrspacecast (ptr @c to ptr addrspace(1)), i16 8)
  %2 = ptrtoint ptr addrspace(1) %1 to i32
  ret i32 %2
}
"),
    );
    let layout = llrm_analysis::testing::layout(&module);
    let outer = Outer::of(&module, None);
    let (context, function) = module.function_mut("f").expect("@f");
    run_impl_cfg(context, &layout, function, &outer, true, None);
    let after = printed(&module);
    assert!(!after.contains("load"), "{after}");
}
