use llrm_mir::dense::IdSet;
use llrm_mir::module::BlockId;

use super::{find_function_backedges, inverse_post_order_ext};
use crate::cfg::Shape;
use crate::testing::{block, function, parsed};

const DIAMOND: &str = "define void @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  ret void
}
";

/// The edges of `text`'s @f that close a cycle, as block names.
fn backedges(text: &str) -> Vec<(String, String)> {
    let module = parsed(text);
    let f = function(&module, "f");
    let name = |b: BlockId| f.block(b).name.clone().expect("a named block");
    find_function_backedges(f).into_iter().map(|(from, to)| (name(from), name(to))).collect()
}

fn pair(
    from: &str,
    to: &str,
) -> (String, String) {
    (from.to_string(), to.to_string())
}

#[test]
fn a_loop_has_one_back_edge_and_a_diamond_none() {
    assert!(backedges(DIAMOND).is_empty());
    let looped = "define void @f(i1 %c) {
b0:
  br label %b1

b1:
  br label %b2

b2:
  br i1 %c, label %b1, label %b3

b3:
  ret void
}
";
    assert_eq!(backedges(looped), [pair("b2", "b1")]);
}

/// A cycle entered at two blocks has no dominating header, so loop info finds
/// no loop; the walk calls the edge that closes it on the stack a back edge.
#[test]
fn an_irreducible_cycle_has_the_back_edge_the_walk_meets() {
    let text = "define void @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  br i1 %c, label %b1, label %b3

b3:
  ret void
}
";
    assert_eq!(backedges(text), [pair("b2", "b1")]);
}

/// From the exit up, a block comes after every block that reaches it through
/// the walk, the later-written predecessor first.
#[test]
fn the_reverse_walk_from_the_exit_numbers_blocks_in_post_order() {
    let module = parsed(DIAMOND);
    let f = function(&module, "f");
    let at = |name: &str| block(f, name);
    let mut visited = IdSet::new();
    let order = inverse_post_order_ext(f, at("b3"), &mut visited);
    assert_eq!(order, [at("b0"), at("b2"), at("b1"), at("b3")]);
    // Blocks already seen are left out, and a second walk from a seen block is
    // empty.
    assert!(inverse_post_order_ext(f, at("b3"), &mut visited).is_empty());
}

/// LoopSimplify asked `irreducible(..).is_empty()` of a whole function:
/// 137 k Ir a call on d_faces -O2 (graph, sets and maps built each time).
/// `is_irreducible` is the same answer from one walk.
#[test]
fn is_irreducible_agrees_with_the_blocks_left_in_a_cycle() {
    let reducible = "define void @f(i1 %c) {
b0:
  br label %b1

b1:
  br i1 %c, label %b1, label %b2

b2:
  ret void
}
";
    // A cycle that nothing reaches is not counted, as `irreducible` leaves it
    // out.
    let unreachable = "define void @f(i1 %c) {
b0:
  ret void

b1:
  br label %b2

b2:
  br i1 %c, label %b1, label %b3

b3:
  ret void
}
";
    let two_entries = "define void @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  br i1 %c, label %b1, label %b3

b3:
  ret void
}
";
    for (text, want) in [(DIAMOND, false), (reducible, false), (unreachable, false), (two_entries, true)] {
        let module = parsed(text);
        let f = function(&module, "f");
        let shape = Shape::of(f);
        assert_eq!(shape.dominance.irreducible(f).is_empty(), !want, "{text}");
        assert_eq!(shape.dominance.is_irreducible(f), want, "{text}");
    }
}
