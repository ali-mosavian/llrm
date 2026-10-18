//! `cfg::graph` made one `Vec<i64>` for the successors of every block: 117,763
//! of the 2.9 M allocations of the d_faces -O2 compile (about 65 for each of
//! its 1,811 graphs). A block names one or two successors, held in the
//! block; only the vector of blocks is allocated.

use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

#[test]
fn test_the_graph_of_a_body_allocates_once_however_many_blocks_it_has() {
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
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let function = module.named("f").and_then(|id| module.global(id).function()).expect("@f");
    let (graph, made) = counted(|| llrm_analysis::cfg::graph(function));
    assert_eq!(graph.len(), blocks + 1);
    assert!(graph.iter().all(|block| block.succ.len() <= 2));
    assert!(made <= 4, "{made} allocations for {} blocks", graph.len());
}
