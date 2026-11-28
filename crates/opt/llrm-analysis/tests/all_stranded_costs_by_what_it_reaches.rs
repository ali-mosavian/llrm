//! `stranded` walked the predecessors of every return of the body to answer
//! whether a loop's exit into a block that stops the program was stranded
//! (273 M Ir of a memory_expansion -O2 compile: 219 calls of 1.25 M). The exit
//! block's own successors answer it.
//!
//! The returning blocks are then found once per body state (#7340, which
//! allocates a set sized by the body): the first question of a state pays for
//! it (doubling growth), and every later one allocates nothing for the rest of
//! the body.

use llrm_analysis::noreturn::all_stranded;
use llrm_mir::dense::IdSet;
use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

fn body(returns: usize) -> String {
    let mut text = String::from(
        "declare void @abort()\ndefine void @f(i32 %n) {\nb0:\n  br label %head\n\nhead:\n  %i = phi i32 [ 0, %b0 ], [ %j, %head ]\n  %j = add i32 %i, 1\n  %t = icmp slt i32 %j, %n\n  br i1 %t, label %head, label %stop\n\nstop:\n  call void @abort()\n  unreachable\n",
    );
    for at in 0..returns {
        text += &format!("\nr{at}:\n  ret void\n");
    }
    text += "}\n";
    text
}

/// Allocations of the first question of a body state and of the second.
fn allocations(returns: usize) -> (usize, usize) {
    let module = llrm_mir::parse::module(&body(returns)).unwrap_or_else(|why| panic!("{why:?}"));
    let function = module.named("f").and_then(|id| module.global(id).function()).expect("@f");
    let layout = function.layout();
    let (head, stop) = (llrm_analysis::cfg::id(layout[1]), llrm_analysis::cfg::id(layout[2]));
    let mut starts = IdSet::new();
    starts.insert(stop);
    let (first, made_first) = counted(|| all_stranded(function, head, &starts));
    assert!(first);
    let (stranded, made) = counted(|| all_stranded(function, head, &starts));
    assert!(stranded);
    (made_first, made)
}

/// The first question of a state finds the returning blocks in a set that
/// grows with the body by doubling (at most log2 of the size ratio, 7 for
/// 4 -> 400 returns); a repeat allocates nothing for the rest of the body.
#[test]
fn test_the_first_question_grows_by_doubling_and_a_repeat_allocates_nothing_for_the_body() {
    let ((first_small, again_small), (first_large, again_large)) = (allocations(4), allocations(400));
    assert!(
        first_large <= first_small + 8,
        "first question: {first_small} allocations with 4 returns, {first_large} with 400"
    );
    assert!(again_large <= again_small + 2, "repeat: {again_small} allocations with 4 returns, {again_large} with 400");
}
