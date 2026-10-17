//! The predecessor and successor lists of `forest` (`LoopInfo::new`) and the
//! feeders of every value in `exposed_allocas` were a `Vec` each: 70 K, 32 K
//! and 28 K of the d_faces -O2 compile's 2.8 M allocations. Up to three are
//! held in the list's own 24 bytes; only a body's vectors are allocated.

use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

fn parsed(text: &str) -> llrm_mir::Module {
    llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{why:?}"))
}

#[test]
fn test_the_loops_of_a_body_allocate_per_body_not_per_block() {
    let blocks = 400;
    let mut text = String::from("define i32 @f(i32 %n) {\nb0:\n  br label %b1\n\n");
    for at in 1..blocks {
        text += &format!(
            "b{at}:\n  %c{at} = icmp slt i32 %n, {at}\n  br i1 %c{at}, label %b{}, label %b{}\n\n",
            at + 1,
            if at + 2 <= blocks { at + 2 } else { blocks }
        );
    }
    text += &format!("b{blocks}:\n  ret i32 %n\n}}\n");
    let module = parsed(&text);
    let function = module.named("f").and_then(|id| module.global(id).function()).expect("@f");
    let tree = llrm_mir::dominators::DominatorTree::new(function);
    let (loops, made) = counted(|| llrm_mir::loops::LoopInfo::new(function, &tree));
    assert!(loops.loops.is_empty());
    assert!(made <= 40, "{made} allocations for the loops of {blocks} blocks");
}

#[test]
fn test_the_exposed_allocas_of_a_body_allocate_per_body_not_per_value() {
    let blocks = 400;
    let mut text = String::from("define void @f(ptr %p0) {\nb0:\n  br label %b1\n\n");
    for at in 1..=blocks {
        text += &format!("b{at}:\n  %p{at} = phi ptr [ %p{}, %b{} ]\n  br label %b{}\n\n", at - 1, at - 1, at + 1);
    }
    text += &format!("b{}:\n  ret void\n}}\n", blocks + 1);
    let module = parsed(&text);
    let function = module.named("f").and_then(|id| module.global(id).function()).expect("@f");
    let (_, made) = counted(|| llrm_analysis::frameescape::exposed_allocas(function, |_| false));
    assert!(made <= 12, "{made} allocations for {blocks} values that each feed one");
}

/// Cloning a function made a `Vec` for the operands of every instruction: 31 K
/// and 14 K of the d_faces -O2 compile's 2.8 M allocations. Up to two operands
/// are held in the instruction.
#[test]
fn test_cloning_a_function_allocates_per_body_not_per_instruction() {
    let n = 400;
    let mut text = String::from("define i32 @f(i32 %a) {\nentry:\n");
    for at in 0..n {
        text += &format!("  %{at} = add i32 %a, {at}\n");
    }
    text += "  ret i32 %a\n}\n";
    let module = parsed(&text);
    let function = module.named("f").and_then(|id| module.global(id).function()).expect("@f");
    let (copy, made) = counted(|| function.clone());
    assert_eq!(copy.layout().len(), function.layout().len());
    assert!(made <= 40, "{made} allocations cloning {n} instructions of two operands");
}
