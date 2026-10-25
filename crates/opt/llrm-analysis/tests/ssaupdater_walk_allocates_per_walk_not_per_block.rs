//! `SsaUpdater::get_value_at_end_of_block` allocated a vector of predecessors,
//! another of successors and a third for the infos of each block it reached,
//! and grew its list of infos from empty: 4 allocations a block, 3.4 M Ir
//! across the d_faces -O2 compile. A walk allocates for the whole walk.

use llrm_analysis::ssaupdater::SsaUpdater;
use llrm_mir::module::Operand;
use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

fn allocations_for(blocks: usize) -> usize {
    let mut text = String::from("define i32 @f(i32 %n) {\nb0:\n  br label %b1\n\n");
    for at in 1..blocks {
        text += &format!("b{at}:\n  br label %b{}\n\n", at + 1);
    }
    text += &format!("b{blocks}:\n  ret i32 %n\n}}\n");
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let (context, function) = module.function_mut("f").expect("@f");
    let entry = function.entry().expect("an entry");
    let last = *function.layout().last().expect("a last block");
    let value = Operand::Value(function.parameters()[0]);
    let ty = function.value(function.parameters()[0]).ty;
    let mut updater = SsaUpdater::new(ty, None);
    updater.add_available_value(entry, value);
    let (got, made) = counted(|| updater.get_value_at_end_of_block(context, function, last));
    assert_eq!(got, value);
    made
}

#[test]
fn test_a_walk_back_over_a_chain_of_blocks_allocates_the_same_for_ten_as_for_a_hundred_and_sixty() {
    let (few, many) = (allocations_for(10), allocations_for(160));
    assert!(many <= few + 32, "{few} allocations for 10 blocks, {many} for 160");
}
