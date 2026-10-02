//! What llrm-mir's own pipeline passes were tested for, asked of the pipeline
//! the compilers run: each test below stood in `llrm_mir::transforms_tests`
//! for a pass deleted with #237 (mem2reg, simplifycfg, earlycse, licm, adce,
//! loop-reduce, ipsccp, indvars, loop-deletion), on the same input.

use llrm_mir::module::Module;
use llrm_mir::program::Program;

use crate::pipeline::{self, Applied};
use crate::testing::{parsed, printed, results};

/// `module` through the pipeline the compilers run, for the DOS target.
fn pipe(module: &mut Module) {
    Program::lend(module, std::rc::Rc::new(llrm_x86_code16::Dos::default()), |program: &mut Program| pipeline::applied(program, &Applied::default()))
        .and_then(|done| done)
        .expect("the pipeline runs");
}

/// `text` through it, printed.
fn piped(text: &str) -> String {
    let mut module = parsed(text);
    pipe(&mut module);
    printed(&module)
}

/// Nib's locals were stack cells loaded and stored around each use (the isel path ran 1.98 times the old path's instructions): through the pipeline a loop's counter is a phi and what it sums another.
#[test]
fn a_loop_counter_and_its_sum_are_phis_not_stack_cells() {
    const TEXT: &str = r#"define i16 @sum(i16 %n) {
b1:
  %i = alloca i16
  %total = alloca i16
  store i16 0, ptr %i
  store i16 0, ptr %total
  br label %b2

b2:
  %0 = load i16, ptr %i
  %1 = icmp slt i16 %0, %n
  br i1 %1, label %b3, label %b4

b3:
  %2 = load i16, ptr %total
  %3 = load i16, ptr %i
  %4 = add i16 %2, %3
  store i16 %4, ptr %total
  %5 = add i16 %3, 1
  store i16 %5, ptr %i
  br label %b2

b4:
  %6 = load i16, ptr %total
  ret i16 %6
}
"#;
    let out = piped(TEXT);
    assert!(!out.contains("alloca") && !out.contains("load") && !out.contains("store"), "{out}");
    let header = &out[out.find("b2:").unwrap()..out.find("b3:").unwrap()];
    assert_eq!(header.matches("phi i16").count(), 2, "{out}");
}

/// A volatile store keeps its cell in memory; the array a GEP reaches is promoted and its value folded.
#[test]
fn a_volatile_cell_stays_memory_and_an_array_the_gep_reaches_is_folded() {
    const TEXT: &str = r#"define i16 @f() {
b1:
  %a = alloca [2 x i16]
  %v = alloca i16
  store volatile i16 1, ptr %v
  %p = getelementptr i16, ptr %a, i16 1
  store i16 2, ptr %p
  %x = load i16, ptr %p
  %y = load i16, ptr %v
  %z = add i16 %x, %y
  ret i16 %z
}
"#;
    let out = piped(TEXT);
    assert!(out.contains("alloca i16") && out.contains("store volatile i16 1"), "{out}");
    assert!(out.contains("ret i16 3"), "{out}");
}

/// A condition folded to a constant leaves a branch that goes one way, an unreached block and a chain of blocks that only jump: one block.
#[test]
fn a_constant_branch_and_the_chain_after_it_become_one_block() {
    const TEXT: &str = r#"define i16 @f(i16 %x) {
b1:
  br i1 true, label %b3, label %b2

b2:
  br label %b3

b3:
  %0 = phi i16 [ %x, %b1 ], [ 0, %b2 ]
  br label %b4

b4:
  %1 = add i16 %0, 1
  br label %b5

b5:
  ret i16 %1
}
"#;
    let out = piped(TEXT);
    assert!(out.contains("b1:\n  %0 = add i16 %x, 1\n  ret i16 %0\n}"), "{out}");
    assert!(!out.contains("b2:") && !out.contains("b3:"), "{out}");
}

/// T048 ran two more instructions per iteration once a loop's exit block was bypassed: whatever the pipeline does to the two loops, each n gets the answer it had.
#[test]
fn a_loop_exit_before_phis_keeps_its_answers() {
    const TEXT: &str = r#"define i16 @f(i16 %n) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %1, %b3 ]
  %c = icmp slt i16 %0, %n
  br i1 %c, label %b3, label %b4

b3:
  %1 = add i16 %0, 1
  br label %b2

b4:
  br label %b5

b5:
  %2 = phi i16 [ 0, %b4 ], [ %3, %b5 ]
  %3 = add i16 %2, 3
  %d = icmp slt i16 %3, 9
  br i1 %d, label %b5, label %b6

b6:
  ret i16 %3
}
"#;
    let before = parsed(TEXT);
    let mut after = before.clone();
    pipe(&mut after);
    let inputs: Vec<[i128; 1]> = (0..6).map(|n| [n]).collect();
    let inputs: Vec<&[i128]> = inputs.iter().map(|one| &one[..]).collect();
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{}", printed(&after));
}

/// nbody ran 201 more instructions once an inner loop's preheader was bypassed (its counter's start moved to the outer header): the answer stays, and the counter-only loop is gone.
#[test]
fn a_loop_preheader_is_not_moved_into_the_outer_loop() {
    const TEXT: &str = r#"define i16 @f(i16 %n, i1 %p) {
b1:
  br i1 %p, label %b5, label %b4

b5:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b5 ], [ %1, %b3 ]
  %1 = add i16 %0, 1
  %c = icmp slt i16 %1, %n
  br i1 %c, label %b3, label %b4

b3:
  br label %b2

b4:
  ret i16 %n
}
"#;
    let before = parsed(TEXT);
    let mut after = before.clone();
    pipe(&mut after);
    let inputs: Vec<[i128; 2]> = (0..4).flat_map(|n| [[n, 0], [n, 1]]).collect();
    let inputs: Vec<&[i128]> = inputs.iter().map(|one| &one[..]).collect();
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{}", printed(&after));
    assert!(!printed(&after).contains("phi"), "{}", printed(&after));
}

/// matmul8's inner loop loaded a view's dimension three times an iteration and checked `k < dim` twice: the check and the reloads go, a load after a store reads the stored value, one after a call that may write stays.
#[test]
fn a_load_and_a_known_condition_are_reused_but_not_across_a_call_that_may_write() {
    const TEXT: &str = r#"declare void @panic()
declare void @write()

define i16 @f(ptr %v, i16 %k, ptr %out) {
b1:
  %0 = load i16, ptr %v
  %1 = icmp ult i16 %k, %0
  br i1 %1, label %b2, label %b4

b2:
  %2 = load i16, ptr %v
  %3 = icmp ult i16 %k, %2
  br i1 %3, label %b3, label %b5

b3:
  store i16 %k, ptr %out
  %4 = load i16, ptr %out
  call void @write()
  %5 = load i16, ptr %out
  %6 = add i16 %4, %5
  ret i16 %6

b4:
  ret i16 0

b5:
  call void @panic()
  unreachable
}
"#;
    let out = piped(TEXT);
    assert_eq!(out.matches("load i16, ptr %v").count(), 1, "{out}");
    assert_eq!(out.matches("icmp ult").count(), 1, "{out}");
    assert!(out.contains("store i16 %k, ptr %out\n  call void @write()\n  %2 = load i16, ptr %out\n  %3 = add i16 %k, %2"), "{out}");
    assert!(!out.contains("@panic()\n  unreachable"), "{out}");
}

/// matmul8 reloaded each view descriptor's shape and data pointer in its innermost loop: a load that may run anywhere, of memory the loop cannot write, leaves the loop; one through a plain pointer stays.
#[test]
fn an_invariant_load_leaves_a_loop_from_a_conditional_block() {
    const TEXT: &str = r#"define void @f(ptr noalias readonly dereferenceable(4) %v, ptr %w, i16 %n, ptr %out) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %7, %b4 ]
  %1 = icmp slt i16 %0, %n
  br i1 %1, label %b3, label %b5

b3:
  %2 = getelementptr i8, ptr %v, i16 2
  %3 = load i16, ptr %2
  %4 = load i16, ptr %w
  %5 = add i16 %3, %4
  %6 = getelementptr i16, ptr %out, i16 %0
  store i16 %5, ptr %6
  br label %b4

b4:
  %7 = add i16 %0, 1
  br label %b2

b5:
  ret void
}
"#;
    let out = piped(TEXT);
    let entry = &out[out.find("b1:").unwrap()..out.find("b2:").unwrap()];
    assert!(entry.contains("getelementptr i8, ptr %v, i16 2") && entry.contains("load i16"), "{out}");
    let body = &out[out.find("b3:").unwrap()..out.find("b5:").unwrap()];
    assert!(body.contains("load i16, ptr %w"), "{out}");
}

/// A QB view's shape load, which the loop's tagged stores cannot reach, leaves the loop.
#[test]
fn a_load_a_loops_stores_are_tagged_apart_from_leaves_it() {
    const TEXT: &str = r#"declare void @dim(ptr)

define void @f(i16 %n) {
b1:
  %d = alloca [4 x i8]
  call void @dim(ptr %d)
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %7, %b3 ]
  %1 = icmp slt i16 %0, %n
  br i1 %1, label %b3, label %b4

b3:
  %2 = getelementptr i8, ptr %d, i16 2
  %3 = load i16, ptr %2, !tbaa !3
  %4 = inttoptr i16 %3 to ptr addrspace(2)
  %5 = addrspacecast ptr addrspace(2) %4 to ptr addrspace(1)
  %6 = getelementptr i16, ptr addrspace(1) %5, i16 %0
  store i16 %0, ptr addrspace(1) %6, !tbaa !4
  %7 = add i16 %0, 1
  br label %b2

b4:
  ret void
}

!0 = !{!"qb"}
!1 = !{!"place", !0, i64 0}
!2 = !{!"allocation", !0, i64 0}
!3 = !{!1, !1, i64 0}
!4 = !{!2, !2, i64 0}
"#;
    let out = piped(TEXT);
    let entry = &out[out.find("b1:").unwrap()..out.find("b2:").unwrap()];
    assert!(entry.contains("load i16"), "{out}");
}

/// A static the loop's stores are tagged apart from is loaded once, before the loop.
#[test]
fn a_load_from_a_global_the_loop_does_not_write_leaves_it() {
    const TEXT: &str = r#"@d = global [4 x i8] zeroinitializer

define void @f(i16 %n) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %6, %b3 ]
  %1 = icmp slt i16 %0, %n
  br i1 %1, label %b3, label %b4

b3:
  %2 = load i16, ptr getelementptr (i8, ptr @d, i16 2), !tbaa !3
  %3 = inttoptr i16 %2 to ptr addrspace(2)
  %4 = addrspacecast ptr addrspace(2) %3 to ptr addrspace(1)
  %5 = getelementptr i16, ptr addrspace(1) %4, i16 %0
  store i16 0, ptr addrspace(1) %5, !tbaa !4
  %6 = add i16 %0, 1
  br label %b2

b4:
  ret void
}

!0 = !{!"qb"}
!1 = !{!"place", !0, i64 0}
!2 = !{!"allocation", !0, i64 0}
!3 = !{!1, !1, i64 0}
!4 = !{!2, !2, i64 0}
"#;
    let out = piped(TEXT);
    let entry = &out[out.find("b1:").unwrap()..out.find("b2:").unwrap()];
    assert!(entry.contains("load i16"), "{out}");
}

/// A second counter that nothing but its own update reads is dead.
#[test]
fn a_counter_only_its_own_cycle_reads_is_removed() {
    const TEXT: &str = r#"define void @f(i16 %n, ptr %out) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %4, %b3 ]
  %1 = phi i16 [ 0, %b1 ], [ %3, %b3 ]
  %2 = icmp slt i16 %0, %n
  br i1 %2, label %b3, label %b4

b3:
  %3 = add i16 %1, 1
  %4 = add i16 %0, 1
  store i16 %0, ptr %out
  br label %b2

b4:
  ret void
}
"#;
    let out = piped(TEXT);
    assert_eq!(out.matches("phi i16").count(), 1, "{out}");
}

/// A fill of a 8x8 array of longs: the loops are gone and each store has its constant offset.
#[test]
fn an_address_stepped_by_its_stride_leaves_no_multiply_or_inner_counter_product() {
    const TEXT: &str = r#"define void @fill(ptr %0) {
b1:
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %9, %b6 ]
  %2 = icmp slt i16 %1, 8
  br i1 %2, label %b3, label %b7

b3:
  %3 = shl i16 %1, 3
  br label %b4

b4:
  %4 = phi i16 [ 0, %b3 ], [ %8, %b5 ]
  %5 = icmp slt i16 %4, 8
  br i1 %5, label %b5, label %b6

b5:
  %6 = add i16 %3, %4
  %7 = getelementptr inbounds i32, ptr %0, i16 %6
  store i32 0, ptr %7
  %8 = add i16 %4, 1
  br label %b4

b6:
  %9 = add i16 %1, 1
  br label %b2

b7:
  ret void
}
"#;
    let out = piped(TEXT);
    assert!(!out.contains("phi") && !out.contains("mul") && !out.contains("shl"), "{out}");
    assert_eq!(out.matches("store i32 0").count(), 64, "{out}");
}

/// What every call of a private function passes is the parameter.
#[test]
fn a_parameter_every_call_passes_the_same_constant_for_is_that_constant() {
    const TEXT: &str = r#"define internal i16 @f(i16 %k, i16 %n) {
b1:
  %0 = add i16 %k, %n
  ret i16 %0
}

define i16 @g() {
b1:
  %0 = call i16 @f(i16 3, i16 1)
  %1 = call i16 @f(i16 3, i16 2)
  %2 = add i16 %0, %1
  ret i16 %2
}
"#;
    let out = piped(TEXT);
    assert!(out.contains("ret i16 9"), "{out}");
}

/// A bounds check the loop's own test implies goes: nothing calls the failure.
#[test]
fn a_check_the_loop_test_implies_is_settled() {
    const TEXT: &str = r#"declare void @fail()

define void @fill(ptr %p) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %3, %b3 ]
  %1 = icmp slt i16 %0, 8
  br i1 %1, label %b4, label %b5

b4:
  %2 = icmp ult i16 %0, 8
  br i1 %2, label %b3, label %b6

b3:
  store i16 %0, ptr %p
  %3 = add i16 %0, 1
  br label %b2

b5:
  ret void

b6:
  call void @fail()
  unreachable
}
"#;
    let out = piped(TEXT);
    assert!(!out.contains("call void @fail()"), "{out}");
    assert!(out.contains("store i16 7, ptr %p"), "{out}");
}

/// A counted loop that computes nothing leaves its constant answer.
#[test]
fn a_loop_that_does_nothing_is_gone() {
    const TEXT: &str = r#"define i16 @f() {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %2, %b3 ]
  %1 = icmp slt i16 %0, 20
  br i1 %1, label %b3, label %b4

b3:
  %2 = add i16 %0, 1
  br label %b2

b4:
  ret i16 7
}
"#;
    let out = piped(TEXT);
    assert!(out.contains("define i16 @f() memory(none) willreturn nounwind {\nb1:\n  ret i16 7\n}"), "{out}");
}
