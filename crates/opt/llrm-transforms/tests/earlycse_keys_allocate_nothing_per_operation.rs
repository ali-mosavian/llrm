//! EarlyCSE keyed every operation by a vector of its operands, cloned once to
//! build the key and again to enter it, and put each key's value on a vector
//! of its own: 6 allocations an operation (the pass manager's own make the 3
//! that remain), 0.6 M Ir on d_faces -O2.

use std::rc::Rc;

use llrm_support::alloc_count::{Counting, counted};
use llrm_transforms::earlycse::EarlyCse;

#[global_allocator]
static COUNTING: Counting = Counting;

fn allocations_for(operations: usize) -> usize {
    let mut text = String::from("define i32 @f(i32 %a, i32 %b) {\nentry:\n  %v0 = add i32 %a, %b\n");
    for at in 1..operations {
        text += &format!("  %v{at} = add i32 %v{}, %b\n", at - 1);
    }
    text += &format!("  ret i32 %v{}\n}}\n", operations - 1);
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(EarlyCse { memory_ssa: false });
    let (_, made) = counted(|| manager.run_module(&mut module, Rc::new(llrm_x86_m16::Dos::default())).unwrap());
    made
}

#[test]
fn test_a_hundred_more_operations_cost_at_most_four_allocations_each_not_six() {
    allocations_for(4);
    let (few, many) = (allocations_for(20), allocations_for(120));
    assert!(many <= few + 450, "{few} allocations for 20 operations, {many} for 120");
}
