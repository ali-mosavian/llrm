//! `InterleavedAccessInfo` against the pinned LLVM's `-passes=loop-vectorize
//! -mtriple=aarch64 -enable-interleaved-mem-accesses -force-vector-width=4` on
//! `tests/interleave/*.ll`: a group the pin makes is a `wide.vec` access of the
//! group's factor in the output, "requires scalar epilogue" in its debug log.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::AssumptionCache;

use super::InterleavedAccessInfo;
use crate::basicaa::BasicAA;
use crate::loopaccessinfo::LoopAccessInfo;
use crate::memory::Unit;
use crate::scev::{Cx, ScalarEvolution};

/// The groups of the one loop of the one function `text` defines, one line
/// each: factor, direction and the members by index (`-`: a gap), then the
/// scalar epilogue line.
fn groups(
    text: &str,
    enable_predicated: bool,
) -> String {
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, function) = module.functions().find(|(_, _, function)| function.entry().is_some()).expect("a function");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("a layout");
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
    let unit = Unit::of(&module, &layout, function);
    let header = loops.get_loops_in_preorder().into_iter().next().expect("a loop").get_header();
    let mut se = ScalarEvolution::new();
    let mut aa = BasicAA::new(&unit);
    let laa = LoopAccessInfo::new(&mut se, &cx, header, &mut aa, false, u32::MAX);
    let info = InterleavedAccessInfo::analyze(&mut se, &cx, header, Some(&laa), enable_predicated);
    let name = |inst: InstId| -> String {
        let data = function.instruction(inst);
        let is_store = matches!(data.opcode, Opcode::Store { .. });
        let value = if is_store {
            match data.operands[0] {
                Operand::Value(value) => value,
                _ => return "store".to_owned(),
            }
        } else {
            data.result.expect("a load has a result")
        };
        let label = function.value(value).name.clone().unwrap_or_default();
        if is_store { format!("store %{label}") } else { format!("%{label}") }
    };
    let mut out = String::new();
    for group in info.groups() {
        let members: Vec<String> =
            (0..group.factor()).map(|index| group.member(index).map_or("-".to_owned(), name)).collect();
        out += &format!(
            "factor {} {} [{}]\n",
            group.factor(),
            if group.is_reverse() { "reverse" } else { "forward" },
            members.join(", ")
        );
    }
    out += &format!("scalar epilogue: {}\n", info.requires_scalar_epilogue());
    out
}

macro_rules! fixture {
    ($name:ident, $predicated:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let text = include_str!(concat!("../tests/interleave/", stringify!($name), ".ll"));
            assert_eq!(groups(text, $predicated), $expected);
        }
    };
}

// Both loads of `a[2i]`, `a[2i+1]` are one full group (the pin: one `<8 x i32>`
// load and two shuffles).
fixture!(load2, false, "factor 2 forward [%v0, %v1]\nscalar epilogue: false\n");

// `a[3i]`, `a[3i+1]`: the group has a gap at 2 and needs a scalar epilogue (the
// pin: `<12 x i32>` load, "interleaved group requires scalar epilogue").
fixture!(load3_gap, false, "factor 3 forward [%v0, %v1, -]\nscalar epilogue: true\n");

// A full store group: the pin's `interleaved.vec` shuffle and one wide store.
fixture!(store2, false, "factor 2 forward [store %x, store %y]\nscalar epilogue: false\n");

// A store group with a gap is a masked store, dropped without them (the pin
// scalarizes both stores).
fixture!(store3_gap, false, "scalar epilogue: false\n");

// With masked interleaved accesses the gap is kept.
fixture!(store3_gap_masked, true, "factor 3 forward [store %x, store %y, -]\nscalar epilogue: false\n");

// A store to `a[2i+1]` between the loads of `a[2i]` and `a[2i+1]`: the second
// load cannot be hoisted above it, so each load is a group of its own (the
// pin: two `<8 x i32>` loads, one lane of each used).
fixture!(raw_blocks, false, "factor 2 forward [%v1, -]\nfactor 2 forward [%v0, -]\nscalar epilogue: true\n");

// A reversed group with a gap is dropped (the pin loads each lane).
fixture!(reverse_gap, false, "scalar epilogue: false\n");

// A stride of 9 is more than `MaxInterleaveGroupFactor` (8): no group.
fixture!(stride9, false, "scalar epilogue: false\n");
