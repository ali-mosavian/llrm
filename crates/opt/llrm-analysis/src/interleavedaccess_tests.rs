//! `InterleavedAccessInfo::analyze` on loops that stride through memory in
//! steps of two or three elements: the groups LoopVectorize widens into
//! `ld2`/`st2`-shaped accesses. `textfill` stored two bytes per iteration and
//! no group was ever found, so it stayed scalar (32009 instructions against
//! the pin's 4120).

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::AssumptionCache;

use super::InterleavedAccessInfo;
use crate::basicaa::BasicAA;
use crate::loopaccessinfo::LoopAccessInfo;
use crate::memory::Unit;
use crate::scev::{Cx, ScalarEvolution};

const HEAD: &str = "target datalayout = \"e-m:e-p:64:64-i64:64-n8:16:32:64\"\n";

/// Each group of the one loop of `body` as `load|store xFACTOR mMEMBERS`
/// (`+epilogue` when it needs a scalar epilogue), and whether the analysis
/// as a whole requires one.
fn groups(body: &str) -> (Vec<String>, bool) {
    let text = format!(
        "{HEAD}define void @f(ptr %p, ptr %q, i32 %a, i32 %b, i64 %n) {{\nentry:\n  br label %loop\nloop:\n  \
         %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]\n{body}  %i.next = add nuw nsw i64 %i, 1\n  \
         %c = icmp eq i64 %i.next, %n\n  br i1 %c, label %exit, label %loop\nexit:\n  ret void\n}}\n"
    );
    let module = llrm_mir::parse::module(&text).expect("a module");
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
    let mut se = ScalarEvolution::new();
    let header = loops.get_loops_in_preorder().into_iter().next().expect("a loop").get_header();
    let mut aa = BasicAA::new(&unit);
    let lai = LoopAccessInfo::new(&mut se, &cx, header, &mut aa, false, u32::MAX);
    let info = InterleavedAccessInfo::analyze(&mut se, &cx, header, &lai, false);
    let described = info
        .interleave_groups()
        .map(|group| {
            let first = group.members().next().expect("a member");
            let kind = match cx.function.instruction(first).opcode {
                Opcode::Store { .. } => "store",
                _ => "load",
            };
            let epilogue = if group.requires_scalar_epilogue() { "+epilogue" } else { "" };
            format!("{kind} x{} m{}{epilogue}", group.factor(), group.num_members())
        })
        .collect();
    (described, info.requires_scalar_epilogue())
}

/// `p[2i] = a; p[2i+1] = b` (textfill's inner loop) is one full store group.
#[test]
fn two_stores_per_iteration_are_one_store_group() {
    let body = "  %j = shl nuw nsw i64 %i, 1\n  %p0 = getelementptr inbounds i32, ptr %p, i64 %j\n  \
                store i32 %a, ptr %p0, align 4\n  %k = or disjoint i64 %j, 1\n  \
                %p1 = getelementptr inbounds i32, ptr %p, i64 %k\n  store i32 %b, ptr %p1, align 4\n";
    assert_eq!(groups(body), (vec!["store x2 m2".to_string()], false));
}

/// `x = p[2i]; y = p[2i+1]` is one full load group.
#[test]
fn two_loads_per_iteration_are_one_load_group() {
    let body = "  %j = shl nuw nsw i64 %i, 1\n  %p0 = getelementptr inbounds i32, ptr %p, i64 %j\n  \
                %x = load i32, ptr %p0, align 4\n  %k = or disjoint i64 %j, 1\n  \
                %p1 = getelementptr inbounds i32, ptr %p, i64 %k\n  %y = load i32, ptr %p1, align 4\n  \
                %s = add i32 %x, %y\n  %pq = getelementptr inbounds i32, ptr %q, i64 %i\n  \
                store i32 %s, ptr %pq, align 4\n";
    assert_eq!(groups(body), (vec!["load x2 m2".to_string()], false));
}

/// Two of the three elements of a stride-3 load group leave a gap at its
/// end: the loop needs a scalar epilogue.
#[test]
fn a_load_group_with_a_gap_at_its_end_needs_an_epilogue() {
    let body = "  %j = mul nuw nsw i64 %i, 3\n  %p0 = getelementptr inbounds i32, ptr %p, i64 %j\n  \
                %x = load i32, ptr %p0, align 4\n  %k = add nuw nsw i64 %j, 1\n  \
                %p1 = getelementptr inbounds i32, ptr %p, i64 %k\n  %y = load i32, ptr %p1, align 4\n  \
                %s = add i32 %x, %y\n  %pq = getelementptr inbounds i32, ptr %q, i64 %i\n  \
                store i32 %s, ptr %pq, align 4\n";
    assert_eq!(groups(body), (vec!["load x3 m2+epilogue".to_string()], true));
}

/// A store group with a gap is released: it would need a masked wide store.
#[test]
fn a_store_group_with_a_gap_is_released() {
    let body = "  %j = shl nuw nsw i64 %i, 1\n  %p0 = getelementptr inbounds i32, ptr %p, i64 %j\n  \
                store i32 %a, ptr %p0, align 4\n";
    assert_eq!(groups(body), (Vec::<String>::new(), false));
}

/// Unit-stride accesses form no group.
#[test]
fn consecutive_accesses_form_no_group() {
    let body = "  %p0 = getelementptr inbounds i32, ptr %p, i64 %i\n  store i32 %a, ptr %p0, align 4\n";
    assert_eq!(groups(body), (Vec::<String>::new(), false));
}

/// The pin's own example (`canReorderMemAccessesForInterleavedGroups`):
/// `A[j] = a; A[j-1] = b; A[j-3] = c; A[j] = d` at stride 2 makes one store
/// group, as the pin's `opt -passes=loop-vectorize` does (one wide store).
#[test]
fn a_store_over_a_member_does_not_split_the_pins_example_group() {
    let body = "  %j = shl nuw nsw i64 %i, 1\n  %g0 = getelementptr inbounds i32, ptr %p, i64 %j\n  \
                store i32 %a, ptr %g0, align 4\n  %j1 = sub nsw i64 %j, 1\n  \
                %g1 = getelementptr inbounds i32, ptr %p, i64 %j1\n  store i32 %b, ptr %g1, align 4\n  \
                %j3 = sub nsw i64 %j, 3\n  %g3 = getelementptr inbounds i32, ptr %p, i64 %j3\n  \
                store i32 %a, ptr %g3, align 4\n  store i32 %b, ptr %g0, align 4\n";
    assert_eq!(groups(body), (vec!["store x2 m2".to_string()], false));
}

/// A store to the slot between two loads of one stride-2 stream keeps the
/// loads out of one group (hoisting the second over the store is illegal);
/// the store's own group has a gap and is released. The pin: two wide loads.
#[test]
fn a_store_between_two_loads_keeps_them_in_separate_groups() {
    let body = "  %j = shl nuw nsw i64 %i, 1\n  %g0 = getelementptr inbounds i32, ptr %p, i64 %j\n  \
                %x = load i32, ptr %g0, align 4\n  %j1 = or disjoint i64 %j, 1\n  \
                %g1 = getelementptr inbounds i32, ptr %p, i64 %j1\n  store i32 %a, ptr %g1, align 4\n  \
                %y = load i32, ptr %g1, align 4\n  %s = add i32 %x, %y\n  \
                %pq = getelementptr inbounds i32, ptr %q, i64 %i\n  store i32 %s, ptr %pq, align 4\n";
    assert_eq!(groups(body), (vec!["load x2 m1+epilogue".to_string(); 2], true));
}
