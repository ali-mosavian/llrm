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

/// Two triangles in a row storing to `%g`: one store after them, of the
/// value the second condition chooses, when either condition holds. Before,
/// both stores ran when both conditions held.
#[test]
fn test_stores_to_one_address_in_two_triangles_are_one_store() {
    assert_eq!(
        simplify(
            "define void @f(i1 %a, i1 %b, ptr %g) {
p:
  br i1 %a, label %pt, label %q

pt:
  store i16 1, ptr %g
  br label %q

q:
  br i1 %b, label %qt, label %post

qt:
  store i16 2, ptr %g
  br label %post

post:
  ret void
}
"
        ),
        "define void @f(i1 %a, i1 %b, ptr %g) {
p:
  %0 = select i1 %b, i16 2, i16 1
  %or = or i1 %a, %b
  br i1 %or, label %1, label %2

1:
  store i16 %0, ptr %g
  br label %2

2:
  ret void
}
"
    );
}

/// A load between the stores may read the first one's value: no merge.
#[test]
fn test_a_load_between_the_stores_keeps_them() {
    let text = "define void @f(i1 %a, i1 %b, ptr %g, ptr %h) {
p:
  br i1 %a, label %pt, label %q

pt:
  store i16 1, ptr %g
  br label %q

q:
  %l = load i16, ptr %h
  br i1 %b, label %qt, label %post

qt:
  store i16 %l, ptr %g
  br label %post

post:
  ret void
}
";
    assert_eq!(simplify(text), text);
}
