//! `isDereferenceableAndAlignedInLoop` on loops over globals of known size.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::AssumptionCache;

use super::is_dereferenceable_and_aligned_in_loop;
use crate::scev::{Cx, ScalarEvolution};

/// Whether the one load of `loop` in `@f` is dereferenceable in it.
fn safe(text: &str) -> bool {
    let module =
        llrm_mir::parse::module(&format!("target datalayout = \"e-m:e-p:64:64-i64:64-n8:16:32:64\"\n\n{text}"))
            .expect("a module");
    let (_, _, function) = module.functions().find(|(_, _, function)| function.entry().is_some()).expect("@f");
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
    let header =
        *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("loop")).expect("loop");
    let load = function
        .iter_block(header)
        .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Load { .. }))
        .expect("a load");
    let ptr = function.instruction(load).operands[0];
    let ty = function.value(function.instruction(load).result.expect("a value")).ty;
    let mut se = ScalarEvolution::new();
    is_dereferenceable_and_aligned_in_loop(&mut se, &cx, header, ptr, ty, 1)
}

fn counted(
    size: usize,
    trips: usize,
) -> String {
    format!(
        "@a = global [{size} x i8] zeroinitializer

define i8 @f() {{
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %p = getelementptr inbounds i8, ptr @a, i64 %i
  %v = load i8, ptr %p, align 1
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, {trips}
  br i1 %c, label %exit, label %loop
exit:
  ret i8 %v
}}
"
    )
}

/// A walk of 1200 bytes over a 1200-byte global is inside it; the pin's loop
/// over `differ` is of this shape. No proof existed: the vectorizer could not
/// load 16 bytes ahead of an early exit.
#[test]
fn a_walk_that_ends_inside_the_global_is_dereferenceable() {
    assert!(safe(&counted(1200, 1200)));
}

/// The same walk over a global of 1000 bytes leaves it.
#[test]
fn a_walk_past_the_end_of_the_global_is_not() {
    assert!(!safe(&counted(1000, 1200)));
}
