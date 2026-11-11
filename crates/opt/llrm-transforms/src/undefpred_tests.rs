//! Each body is MIR text; the answers are `opt -passes=simplifycfg`'s
//! (`tools/llvmport/simplifycfgcompare.py`).

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::simplified;
use crate::testing::{parsed, printed};
use crate::twoentryphi::Speculation;

fn simplify(text: &str) -> String {
    let mut module = parsed(text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified(context, &spec, &mut declared, function);
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// A way into the block that gives the phi a null the block loads through is
/// undefined behavior: the way is cut and the load reads the other value.
#[test]
fn test_a_predecessor_that_gives_a_null_that_is_loaded_through_stops_reaching_the_block() {
    assert_eq!(
        simplify(
            "define i16 @f(ptr %q, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi ptr [ null, %b1 ], [ %q, %b2 ]
  %v = load i16, ptr %p
  ret i16 %v
}
"
        ),
        "define i16 @f(ptr %q, i1 %c) {
b0:
  %0 = xor i1 %c, true
  call void @llvm.assume(i1 %0)
  %v = load i16, ptr %q
  ret i16 %v
}

declare void @llvm.assume(i1 noundef) nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write)
"
    );
}

/// Where a null pointer is valid (`null_pointer_is_valid`) the load is not
/// undefined: the way in stays.
#[test]
fn test_a_null_that_is_loaded_through_is_valid_where_the_function_says_so() {
    let text = "define i16 @f(ptr %q, i1 %c) null_pointer_is_valid {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi ptr [ null, %b1 ], [ %q, %b2 ]
  %v = load i16, ptr %p
  ret i16 %v
}
";
    let out = simplify(text);
    assert!(out.contains("null"), "{out}");
}

/// The way into a block that divides by a null-fed phi is cut by a conditional
/// branch: the pin keeps `assume(cond)` where we kept nothing, so the facts that
/// the branch gave (`%c` holds past the cut) were lost to later passes.
#[test]
fn test_a_cut_conditional_branch_keeps_an_assume_of_its_condition() {
    let out = simplify(
        "define i32 @f(i1 %c, i32 %x) {
e:
  br i1 %c, label %a, label %m

a:
  br label %m

m:
  %p = phi i32 [ 0, %e ], [ %x, %a ]
  %r = udiv i32 7, %p
  ret i32 %r
}
",
    );
    assert!(out.contains("call void @llvm.assume(i1 %c)"), "{out}");
    assert!(out.contains("declare void @llvm.assume(i1"), "{out}");
}

/// The cut edge was the true one: the assume holds the negation.
#[test]
fn test_a_cut_true_edge_assumes_the_negation() {
    let out = simplify(
        "define i32 @f(i1 %c, i32 %x) {
e:
  br i1 %c, label %m, label %a

a:
  br label %m

m:
  %p = phi i32 [ 0, %e ], [ %x, %a ]
  %r = udiv i32 7, %p
  ret i32 %r
}
",
    );
    assert!(out.contains("xor i1 %c, true"), "{out}");
    assert!(out.contains("call void @llvm.assume(i1 %"), "{out}");
}

/// Negative control: the entry's unconditional way into the loop is cut and
/// leaves no condition to keep (`opt` answers `unreachable`).
#[test]
fn test_a_cut_unconditional_branch_assumes_nothing() {
    let out = simplify(
        "define i32 @f(i1 %c, i32 %n) {
e:
  br label %m

m:
  %p = phi i32 [ 0, %e ], [ %q, %m ]
  %r = udiv i32 7, %p
  %q = add i32 %p, %r
  br i1 %c, label %m, label %x

x:
  ret i32 %r
}
",
    );
    assert!(!out.contains("llvm.assume"), "{out}");
    assert!(out.contains("unreachable"), "{out}");
}
