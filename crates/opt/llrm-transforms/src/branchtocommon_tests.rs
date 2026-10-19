//! Each body is MIR text; the answers are `opt -passes=simplifycfg`'s
//! (`tools/llvmport/simplifycfgcompare.py`).

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::Outer;

use crate::simplifycfg::simplified;
use crate::testing::{parsed, printed};
use crate::twoentryphi::Speculation;

fn simplify(text: &str) -> String {
    let mut module = parsed(text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let (context, function) = module.function_mut("f").expect("@f");
    simplified(context, &spec, function);
    printed(&module)
}

/// `p` branches to `t` or to `bb`, which is only a comparison branching to
/// `t` or `e`: the comparison runs in `p` and one branch goes to `t` or `e`
/// (`foldBranchToCommonDest`). Before, two branches tested `%a` and `%b`.
#[test]
fn test_a_comparison_block_with_a_common_successor_folds_into_its_predecessor() {
    assert_eq!(
        simplify(
            "declare void @g1()

declare void @g2()

define void @f(i1 %a, i16 %x) {
p:
  br i1 %a, label %t, label %bb

bb:
  %b = icmp eq i16 %x, 0
  br i1 %b, label %t, label %e

t:
  call void @g1()
  ret void

e:
  call void @g2()
  ret void
}
"
        ),
        "declare void @g1()

declare void @g2()

define void @f(i1 %a, i16 %x) {
p:
  %0 = icmp eq i16 %x, 0
  %or.cond = select i1 %a, i1 true, i1 %0
  br i1 %or.cond, label %t, label %e

common.ret:
  ret void

t:
  call void @g1()
  br label %common.ret

e:
  call void @g2()
  br label %common.ret
}
"
    );
}

/// Two instructions besides the condition do not fit the budget of one.
#[test]
fn test_a_block_with_two_instructions_besides_the_condition_stays() {
    let text = "declare void @g1()

declare void @g2()

define void @f(i1 %a, i16 %x) {
p:
  br i1 %a, label %t, label %bb

bb:
  %m = mul i16 %x, 3
  %n = add i16 %m, 5
  %b = icmp eq i16 %n, 0
  br i1 %b, label %t, label %e

common.ret:
  ret void

t:
  call void @g1()
  br label %common.ret

e:
  call void @g2()
  br label %common.ret
}
";
    let text_in = text.replace("common.ret:\n  ret void\n\n", "").replace("br label %common.ret", "ret void");
    assert_eq!(simplify(&text_in), text);
}
