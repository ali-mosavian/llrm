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
  %v = load i16, ptr %q
  ret i16 %v
}
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
