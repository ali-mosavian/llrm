//! `LoopGraph::children` of a function found the terminator once per edge
//! (`get_exiting_blocks` cost 4.6x the pin's on d_faces). It finds it once per
//! block; the edges, repeats and order must stay those of `child`.

use llrm_mir::loopinfo::LoopGraph;
use llrm_mir::parse;

#[test]
fn test_children_are_the_edges_of_child_with_repeats_in_operand_order() {
    let mut module = parse::module(
        "define i32 @f(i32 %x) {\nentry:\n  switch i32 %x, label %b [ i32 0, label %a i32 1, label %b i32 2, label %a ]\na:\n  br label %b\nb:\n  ret i32 0\n}\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let f = module.function_mut("f").expect("@f").1;
    for &block in f.layout() {
        let mut cursor = 0;
        let mut by_child = Vec::new();
        while let Some(next) = f.child(block, &mut cursor) {
            by_child.push(next);
        }
        assert_eq!(f.children(block).collect::<Vec<_>>(), by_child);
    }
    let entry = f.layout()[0];
    assert_eq!(f.children(entry).count(), 4, "default and three cases, repeats kept");
}
