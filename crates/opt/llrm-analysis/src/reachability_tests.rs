//! `isPotentiallyReachable` against CFG.cpp's rules.

use llrm_mir::module::ValueDef;

use super::*;
use crate::testing::{block, function, parsed, value};

fn inst_named(
    function: &Function,
    name: &str,
) -> InstId {
    match function.value(value(function, name)).def {
        ValueDef::Instruction(inst) => inst,
        ValueDef::Argument(_) => panic!("%{name} is an argument"),
    }
}

const DIAMOND: &str = "define void @f(i1 %c) {
entry:
  br i1 %c, label %left, label %right
left:
  %l = add i32 1, 1
  br label %join
right:
  %r = add i32 2, 2
  br label %join
join:
  ret void
}
";

const LOOP: &str = "define void @f(i1 %c) {
entry:
  br label %body
body:
  %a = add i32 1, 1
  %b = add i32 2, 2
  br i1 %c, label %body, label %exit
exit:
  ret void
}
";

#[test]
fn a_diamond_arm_does_not_reach_the_other_arm() {
    let module = parsed(DIAMOND);
    let f = function(&module, "f");
    let tree = DomTree::of(f);
    // Premise: the arms are two blocks, both reachable.
    assert!(tree.is_reachable(block(f, "left")) && tree.is_reachable(block(f, "right")));
    let (l, r) = (inst_named(f, "l"), inst_named(f, "r"));
    assert!(!is_potentially_reachable(f, l, r, Some(&tree)));
    assert!(!is_potentially_reachable(f, r, l, Some(&tree)));
    assert!(is_potentially_reachable_blocks(f, block(f, "left"), block(f, "join"), Some(&tree)));
}

#[test]
fn a_loop_body_reaches_its_earlier_instruction_and_a_straight_line_does_not() {
    let module = parsed(LOOP);
    let f = function(&module, "f");
    let tree = DomTree::of(f);
    let (a, b) = (inst_named(f, "a"), inst_named(f, "b"));
    assert!(is_potentially_reachable(f, a, b, Some(&tree)));
    assert!(is_potentially_reachable(f, b, a, Some(&tree)), "around the back edge");
    let module = parsed(DIAMOND);
    let f = function(&module, "f");
    let tree = DomTree::of(f);
    let join = f.terminator(block(f, "join")).unwrap();
    assert!(!is_potentially_reachable(f, join, inst_named(f, "l"), Some(&tree)));
}

/// A chain longer than the cap answers "maybe" for a block it does not reach:
/// the walk gives up at `MAX_BBS_TO_EXPLORE` blocks, as LLVM's does.
#[test]
fn a_long_chain_past_the_cap_answers_maybe() {
    let chain = |n: usize| {
        let mut text = String::from("define void @f(i1 %c) {\nentry:\n  br i1 %c, label %b0, label %side\n");
        for i in 0..n {
            text.push_str(&format!("b{i}:\n  br label %b{}\n", i + 1));
        }
        text.push_str(&format!("b{n}:\n  ret void\nside:\n  ret void\n}}\n"));
        text
    };
    let asked = |n: usize| {
        let module = parsed(&chain(n));
        let f = function(&module, "f");
        is_potentially_reachable_blocks(f, block(f, "b0"), block(f, "side"), None)
    };
    // `side` is not reachable from the chain; the cap is the only thing that
    // can say it may be.
    assert!(!asked(MAX_BBS_TO_EXPLORE as usize - 4));
    assert!(asked(MAX_BBS_TO_EXPLORE as usize + 8));
}
