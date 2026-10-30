//! Branch probabilities of the DAG body (DEVIATE SD403): the IR's, so that
//! block frequencies and spill weights see a loop as a loop.

use super::dag_function::selected;

const LOOP: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %j, %loop ]
  %j = add i32 %i, 1
  %more = icmp ne i32 %j, %n
  br i1 %more, label %loop, label %done
done:
  ret i32 %j
}
";

/// queens' `_place` kept every edge at 50/50 in the DAG body: its inner-loop
/// temporaries weighed less than the values around the loop, and the counter,
/// `n` and the induction variable went to the stack (AM-BT-6, +40% instructions
/// against the old selector). A loop's back edge is the likely one.
#[test]
fn a_loops_back_edge_is_the_likely_edge_of_the_dag_body() {
    let module = llrm_mir::parse::module(LOOP).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let body = &done.body;
    let looped = body.blocks.iter().find(|block| block.succ.contains(&block.at)).expect("a block that loops");
    let back = body.odds.taken.get(&(looped.at, looped.at)).copied().expect("the back edge has odds");
    assert!(
        f64::from(back) > 0.9 * f64::from(crate::model::lir::BlockOdds::CERTAIN),
        "the back edge is {back} of {}",
        crate::model::lir::BlockOdds::CERTAIN
    );
}

const WEIGHTED_SWITCH: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x) {
entry:
  switch i32 %x, label %d [ i32 1, label %a
                            i32 100, label %b
                            i32 5000, label %c ], !prof !0
a:
  ret i32 10
b:
  ret i32 20
c:
  ret i32 30
d:
  ret i32 0
}
!0 = !{!\"branch_weights\", i32 1, i32 90, i32 5, i32 4}
";

/// The probability of reaching `target` from `from` through the body's odds
/// (a missing edge counts as an even split).
fn reach(
    body: &crate::model::lir::LirBody,
    from: i64,
    target: i64,
) -> f64 {
    if from == target {
        return 1.0;
    }
    let Some(block) = body.blocks.iter().find(|one| one.at == from) else { return 0.0 };
    let succs = crate::model::lir::distinct(&block.succ);
    succs
        .iter()
        .map(|to| {
            let p =
                body.odds.taken.get(&(from, *to)).map_or(1.0 / succs.len() as f64, |p| f64::from(*p) / 2_147_483_648.0);
            p * reach(body, *to, target)
        })
        .sum()
}

/// A switch weighted 90 to one case was a chain of 50/50 compares in the DAG
/// body: its likely case weighed like the others. Leaving by each case has the
/// probability the IR's weights gave it.
#[test]
fn a_weighted_switch_leaves_by_each_case_with_the_irs_probability() {
    let module = llrm_mir::parse::module(WEIGHTED_SWITCH).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let body = &done.body;
    let returns = |value: i64| {
        body.blocks
            .iter()
            .find(|block| {
                block.insns.iter().any(|insn| {
                    insn.what.as_ref().is_some_and(|what| {
                        what.sources.iter().any(|s| matches!(s, crate::model::ir::Loc::Imm(imm) if imm.value == value))
                    })
                })
            })
            .map(|block| block.at)
            .unwrap_or_else(|| panic!("a block returning {value}"))
    };
    let a = reach(body, body.entry, returns(10));
    assert!((a - 0.9).abs() < 0.03, "leaves by the 90-weight case with {a}");
}

const SWITCH: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x) {
entry:
  switch i32 %x, label %d [ i32 1, label %a
                            i32 100, label %b
                            i32 5000, label %c ]
a:
  ret i32 10
b:
  ret i32 20
c:
  ret i32 30
d:
  ret i32 0
}
";

/// With every edge even, a switch's compare took its case as likely as the next
/// compare, so placement kept the case on the fall-through and each miss paid a
/// taken jump (bench `switch` under dag: 8 `jne` and no `je` where the pin has
/// 5 `je` and 3 `jne`). A case is one among the rest, and the next compare is
/// the rest.
#[test]
fn a_switch_compare_leaves_for_its_case_with_the_odds_of_one_case_among_the_rest() {
    let module = llrm_mir::parse::module(SWITCH).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let body = &done.body;
    let returns: Vec<i64> = body.blocks.iter().filter(|b| b.succ.is_empty()).map(|b| b.at).collect();
    let mut compares = 0;
    for block in body.blocks.iter().filter(|b| b.succ.len() == 2) {
        let succ: Vec<i64> = block.succ.iter().copied().collect();
        let (Some(case), Some(next)) = (
            succ.iter().copied().find(|to| returns.contains(to)),
            succ.iter().copied().find(|to| !returns.contains(to)),
        ) else {
            continue;
        };
        compares += 1;
        let (case, next) = (body.odds.chance(block.at, &succ, case), body.odds.chance(block.at, &succ, next));
        assert!(case < next, "{:#x}: case {case}, next compare {next}: {:?}", block.at, body.odds);
    }
    assert!(compares > 0, "a compare that leaves for a case");
}

const SELECT_LOOP: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %n) {
entry:
  br label %loop
loop:
  %x = phi i32 [ %n, %entry ], [ %next, %loop ]
  %odd = and i32 %x, 1
  %is = icmp eq i32 %odd, 0
  %half = lshr i32 %x, 1
  %up = mul i32 %x, 3
  %next = select i1 %is, i32 %half, i32 %up
  %more = icmp ne i32 %next, 1
  br i1 %more, label %loop, label %done
done:
  ret i32 %next
}
";

/// x_collatz's loop at -O1 on the 486 (no `cmov`): the select splits the loop
/// block, and the branch with the IR's 97% back edge ends its last piece. The
/// edges were stated on the first piece, found no such successor and were
/// dropped, so the back edge was 50% and placement did not rotate the loop:
/// 28348 taken branches against the pin's 14767.
#[test]
fn a_back_edge_of_a_block_split_by_a_select_keeps_the_ir_odds() {
    let module = llrm_mir::parse::module(SELECT_LOOP).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let body = &done.body;
    let back = body
        .blocks
        .iter()
        .flat_map(|block| block.succ.iter().filter(|to| **to < block.at).map(move |to| (block.at, *to)))
        .filter_map(|edge| body.odds.taken.get(&edge).copied())
        .max()
        .expect("a back edge with odds");
    assert!(f64::from(back) > 0.9 * f64::from(crate::model::lir::BlockOdds::CERTAIN), "the back edge is {back}");
}

/// A loop of 8 trips `induction` proves had no trips in the DAG body (only the
/// old route set them), so the executed-work estimate read it as 31 in 32 and
/// `matmul`'s allocation test counted 28328 memory operands for ~2400 (the old
/// route: 2269).
#[test]
fn a_counted_loop_has_its_proven_trips_in_the_dag_body() {
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %j, %loop ]
  %s = phi i32 [ %a, %entry ], [ %t, %loop ]
  %t = add i32 %s, %i
  %j = add i32 %i, 1
  %more = icmp ne i32 %j, 8
  br i1 %more, label %loop, label %done
done:
  ret i32 %t
}
",
    )
    .expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let body = &done.body;
    let looped = body.blocks.iter().find(|block| block.succ.contains(&block.at)).expect("a block that loops");
    assert_eq!(body.loop_trip_counts, vec![(looped.at, 8)], "{:?}", body.loop_trip_counts);
}
