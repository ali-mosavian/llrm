//! `GetRangeFromMetadata`: a `range` attribute on a parameter or a call's
//! return bounds the SCEV of that value, as `!range` does.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Operand;
use llrm_mir::valuetracking::AssumptionCache;

use super::{Cx, ScalarEvolution};

/// The unsigned range of `%name` in `text`'s `@f`.
fn unsigned_range_of(
    text: &str,
    name: &str,
) -> (u128, u128) {
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, function) = module.functions().last().expect("@f");
    let layout = DataLayout::default();
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let value = (0..function.value_count() as u32)
        .map(llrm_mir::module::ValueId)
        .find(|&v| function.value(v).name.as_deref() == Some(name))
        .expect("the named value");
    let mut se = ScalarEvolution::new();
    let s = se.get_scev(&cx, Operand::Value(value));
    let range = se.get_unsigned_range(&cx, s);
    (range.get_unsigned_min(), range.get_unsigned_max())
}

/// `range(i16 1, 11)` on a parameter bounds it to 1..10. The attribute was
/// ignored (0..65535 before), so `umax(%n, 1)` stayed a trip count that
/// loop_fusion -O2 expanded as a call the pin does not make.
#[test]
fn test_a_parameter_range_attribute_bounds_its_scev() {
    let text = "define i16 @f(i16 range(i16 1, 11) %n) {\nentry:\n  ret i16 %n\n}\n";
    assert_eq!(unsigned_range_of(text, "n"), (1, 10));
}

#[test]
fn test_a_call_return_range_attribute_bounds_its_scev() {
    let text = "declare i16 @g()\ndefine i16 @f() {\nentry:\n  %r = call range(i16 2, 5) i16 @g()\n  ret i16 %r\n}\n";
    assert_eq!(unsigned_range_of(text, "r"), (2, 4));
}
