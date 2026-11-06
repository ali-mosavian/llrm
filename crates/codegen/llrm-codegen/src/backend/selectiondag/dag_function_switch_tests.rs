//! Switch lowering: compare chains, jump tables, peeled cases, phis at the
//! targets.

use super::dag_function::selected;
use crate::model::ir::Loc;

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

/// A `switch` lowers to compare-and-branch blocks the builder makes after the
/// switch's own; they were refused ("a switch lowering", 5 programs of the
/// m32 execution run). Each is emitted after the block that made it and every
/// branch names one.
#[test]
fn a_switch_is_a_chain_of_compare_and_branch_blocks() {
    let module = llrm_mir::parse::module(SWITCH).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let blocks = done.body.blocks.len();
    assert!(blocks > 5, "the switch's blocks and its four targets: {blocks}");
    let at: Vec<i64> = done.body.blocks.iter().map(|b| b.at).collect();
    for block in done.body.blocks.iter() {
        assert!(block.succ.iter().all(|s| at.contains(s)), "{:?} leaves for a block that is not there", block.succ);
    }
}

const TWO_TABLES: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x, i32 %y) {
entry:
  switch i32 %x, label %ad [
    i32 0, label %a0
    i32 1, label %a1
    i32 2, label %a2
    i32 3, label %a3
    i32 4, label %a4
    i32 5, label %a5
  ]
a0:
  %av0 = add i32 %x, 1
  br label %ajoin
a1:
  %av1 = add i32 %x, 8
  br label %ajoin
a2:
  %av2 = add i32 %x, 15
  br label %ajoin
a3:
  %av3 = add i32 %x, 22
  br label %ajoin
a4:
  %av4 = add i32 %x, 29
  br label %ajoin
a5:
  %av5 = add i32 %x, 36
  br label %ajoin
ad:
  br label %ajoin
ajoin:
  %ar = phi i32 [ %x, %ad ], [ %av0, %a0 ], [ %av1, %a1 ], [ %av2, %a2 ], [ %av3, %a3 ], [ %av4, %a4 ], [ %av5, %a5 ]
  br label %second
second:
  switch i32 %y, label %bd [
    i32 0, label %b0
    i32 1, label %b1
    i32 2, label %b2
    i32 3, label %b3
    i32 4, label %b4
    i32 5, label %b5
  ]
b0:
  %bv0 = add i32 %x, 100
  br label %bjoin
b1:
  %bv1 = add i32 %x, 107
  br label %bjoin
b2:
  %bv2 = add i32 %x, 114
  br label %bjoin
b3:
  %bv3 = add i32 %x, 121
  br label %bjoin
b4:
  %bv4 = add i32 %x, 128
  br label %bjoin
b5:
  %bv5 = add i32 %x, 135
  br label %bjoin
bd:
  br label %bjoin
bjoin:
  %br = phi i32 [ %x, %bd ], [ %bv0, %b0 ], [ %bv1, %b1 ], [ %bv2, %b2 ], [ %bv3, %b3 ], [ %bv4, %b4 ], [ %bv5, %b5 ]
  %s = add i32 %ar, %br
  ret i32 %s
}
";

/// A dense `switch` is a jump through a table of the case blocks (the pin's
/// `JMP32m` on the table), and two of them in one function number their
/// blocks apart: both started at the function's block count, so the second's
/// table block was the first's and `switch_jump_table` read a value its
/// header never set (and, before that, refused on `TargetJumpTable`).
#[test]
fn two_dense_switches_are_two_jumps_through_tables() {
    let module = llrm_mir::parse::module(TWO_TABLES).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let at: Vec<i64> = done.body.blocks.iter().map(|b| b.at).collect();
    let mut unique = at.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), at.len(), "a block address is used twice: {at:?}");
    let tables: Vec<_> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.indirect && !w.table.is_empty())
        .map(|w| w.table.clone())
        .collect();
    assert_eq!(tables.len(), 2, "one jump through a table for each switch");
    assert!(tables.iter().all(|table| table.len() == 6 && table.iter().all(|t| at.contains(t))), "{tables:?}");
    assert_ne!(tables[0], tables[1]);
}

const SWITCH_TO_PHI: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x, i32 %a, i32 %b) {
entry:
  switch i32 %x, label %d [ i32 0, label %j
    i32 1, label %k
    i32 2, label %j
    i32 3, label %k
    i32 4, label %j
    i32 5, label %k ]
d:
  ret i32 0
j:
  %p = phi i32 [ %a, %entry ]
  ret i32 %p
k:
  %q = phi i32 [ %b, %entry ]
  ret i32 %q
}
";

/// A switch whose targets have phis: the incoming values are copied once in
/// the switch's block and every block of the lowering that branches there (the
/// header, the compare blocks, the table) is an edge of the phi. The builder
/// refused it ("switch to a block with phis": 29 functions of the corpora with
/// the MIR switch lowering off).
#[test]
fn a_switch_to_blocks_with_phis_is_lowered() {
    use crate::model::ir::Operation;
    let module = llrm_mir::parse::module(SWITCH_TO_PHI).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let branches = done
        .body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|i| i.what.as_ref().is_some_and(|w| matches!(w.op, Operation::Branch | Operation::Jump)))
        .count();
    assert!(branches >= 2, "the switch became branches");
    let edges: usize =
        done.body.blocks.iter().map(|block| block.phis.iter().map(|phi| phi.incoming.len()).sum::<usize>()).sum();
    assert!(edges >= 2, "each phi has an edge from the lowering's blocks");
}

const PEELED_SWITCH: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @a()
declare void @b()
declare void @c()
define void @f(i32 %x) {
entry:
  switch i32 %x, label %d [ i32 1, label %hot
    i32 20, label %b
    i32 40, label %c ], !prof !0
d:
  ret void
hot:
  call void @a()
  ret void
b:
  call void @b()
  ret void
c:
  call void @c()
  ret void
}
!0 = !{!\"branch_weights\", i32 1, i32 1000, i32 1, i32 1}
";

/// A case that takes most of the probability (`peelDominantCaseCluster`) is
/// tested first, in the switch's block, and falls through to a block of the
/// rest. The builder refused it ("switch with a peeled case": bounds, onerr).
#[test]
fn a_dominant_case_is_peeled_and_the_rest_follows() {
    let module = llrm_mir::parse::module(PEELED_SWITCH).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert!(
        done.body.blocks.len() >= 7,
        "the peeled test and the rest are blocks of their own: {}",
        done.body.blocks.len()
    );
}

/// The pin skips `peelDominantCaseCluster` at `hasMinSize` only; ours also
/// skipped it at `optsize` (-Os), where the pin peels.
#[test]
fn a_dominant_case_is_peeled_at_optsize_and_not_at_minsize() {
    use super::switchcg::{BranchProbability, SwitchTarget, plan_with};
    let cases = [
        (1, 1, BranchProbability::new(1, 10)),
        (20, 2, BranchProbability::new(1, 10)),
        (40, 3, BranchProbability::new(8, 10)),
    ];
    let peeled = |opt_size, min_size| {
        let target = SwitchTarget { opt_size, min_size, ..SwitchTarget::x86(32) };
        plan_with(&cases, 0, &target).peeled.is_some()
    };
    assert!(peeled(false, false), "premise: -O2 peels the 80% case");
    assert!(peeled(true, false), "-Os peels as -O2 does");
    assert!(!peeled(true, true), "-Oz peels nothing");
}

const WIDE_SWITCH: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @a()
define void @f(i32 %x) ATTR {
entry:
  switch i32 %x, label %d [ i32 1, label %b
    i32 100, label %b
    i32 200, label %b
    i32 300, label %b
    i32 400, label %b
    i32 500, label %b
    i32 600, label %b ]
d:
  ret void
b:
  call void @a()
  ret void
}
";

/// At `hasMinSize` the pin lowers a wide switch as a chain, not a balanced
/// tree (`NumClusters > 3 && !hasMinSize`): the first compare is the pivot at
/// -Os and -O2, the first case at -Oz.
#[test]
fn a_wide_switch_is_a_tree_unless_minsize() {
    let first_compare = |attribute: &str| {
        let module = llrm_mir::parse::module(&WIDE_SWITCH.replace("ATTR", attribute)).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{}", e.0));
        let first = done
            .body
            .insns()
            .iter()
            .filter_map(|i| i.what.as_ref())
            .find(|w| matches!(w.name.as_deref(), Some("sub" | "cmp")))
            .and_then(|w| w.sources.iter().find_map(|s| if let Loc::Imm(imm) = s { Some(imm.value) } else { None }));
        first
    };
    let plain = first_compare("");
    assert_eq!(first_compare("optsize"), plain, "-Os splits the tree as -O2 does");
    assert_ne!(first_compare("minsize"), plain, "-Oz is a chain: {plain:?} against {:?}", first_compare("minsize"));
}
