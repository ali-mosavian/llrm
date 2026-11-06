//! `stranded` walked the predecessors of every return of the body to answer
//! whether a loop's exit into a block that stops the program was stranded
//! (273 M Ir of a memory_expansion -O2 compile: 219 calls of 1.25 M). The exit
//! block's own successors answer it.

use llrm_analysis::noreturn::all_stranded;
use llrm_support::alloc_count::{Counting, counted};
use llrm_mir::dense::IdSet;

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

fn allocations(returns: usize) -> usize {
    let module = llrm_mir::parse::module(&body(returns)).unwrap_or_else(|why| panic!("{why:?}"));
    let function = module.named("f").and_then(|id| module.global(id).function()).expect("@f");
    let layout = function.layout();
    let (head, stop) = (llrm_analysis::cfg::id(layout[1]), llrm_analysis::cfg::id(layout[2]));
    let mut starts = IdSet::new();
    starts.insert(stop);
    let (stranded, made) = counted(|| all_stranded(function, head, &starts));
    assert!(stranded);
    made
}

#[test]
fn test_asking_whether_an_exit_is_stranded_allocates_nothing_for_the_rest_of_the_body() {
    let (small, large) = (allocations(4), allocations(400));
    assert!(large <= small + 2, "{small} allocations with 4 returns in the body, {large} with 400");
}
