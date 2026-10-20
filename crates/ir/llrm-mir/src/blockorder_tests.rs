use super::reverse_post_order_traversal;
use crate::module::{BlockId, Function, Module};
use crate::parse;

fn order(text: &str) -> Vec<String> {
    let module: Module = parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    let function: &Function = module.functions().find(|(_, g, _)| g.name.as_deref() == Some("f")).expect("@f").2;
    reverse_post_order_traversal(function)
        .into_iter()
        .map(|BlockId(at)| function.block(BlockId(at)).name.clone().expect("a named block"))
        .collect()
}

/// A diamond: the first successor is walked first, so it is listed last of the
/// two (`a c b d`, as LLVM's `ReversePostOrderTraversal`).
#[test]
fn test_a_diamond_lists_the_second_successor_before_the_first() {
    let text = "define void @f(i1 %c) {
a:
  br i1 %c, label %b, label %c

b:
  br label %d

c:
  br label %d

d:
  ret void
}
";
    assert_eq!(order(text), ["a", "c", "b", "d"]);
}

/// A block no path from the entry reaches is not listed, even one that loops
/// to itself: the unreachable tail `llrm_support::graph::reverse_postorder`
/// appends was ranked and walked by Reassociate, whose rank walk never ended
/// on an instruction there that reads itself (158 GB).
#[test]
fn test_a_block_the_entry_does_not_reach_is_not_listed() {
    let text = "define void @f() {
a:
  ret void

dead:
  br label %dead
}
";
    assert_eq!(order(text), ["a"]);
}

/// A loop: the header before the body, the exit last.
#[test]
fn test_a_loop_lists_its_header_before_its_body() {
    let text = "define void @f(i1 %c) {
a:
  br label %h

h:
  br i1 %c, label %body, label %exit

body:
  br label %h

exit:
  ret void
}
";
    assert_eq!(order(text), ["a", "h", "exit", "body"]);
}

/// A declaration has no blocks.
#[test]
fn test_a_declaration_has_no_blocks() {
    let module = parse::module("declare void @f()\n").expect("parses");
    let function = module.functions().next().expect("@f").2;
    assert!(reverse_post_order_traversal(function).is_empty());
}
