//! `RegStackify` with optimization (`-O1` and up): definitions move to their
//! use and a value read twice gets a `local.tee`.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

fn asm(
    ir: &str,
    level: Level,
) -> String {
    assembly(ir, Width::W32, Options { level, ..Options::default() }).unwrap()
}

/// The body of `@name`: from the label to `end_function`.
fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = text.find(&format!("{name}:")).expect("the function");
    let end = text[start..].find("end_function").expect("its end");
    &text[start..start + end]
}

const SKIP: &str = "define i32 @f(i32 %x, ptr %p) {
entry:
  %a = add i32 %x, 1
  store i32 %x, ptr %p
  ret i32 %a
}
";

/// A value computed before an independent store stayed in a local
/// (`local.set` and `local.get` around the store) because only a definition
/// right above its use could stackify: fib's compare, 28 instructions a loop
/// trip against the pin's 24.
#[test]
fn a_definition_moves_past_an_independent_store_when_optimizing() {
    let o0 = asm(SKIP, Level::O0);
    let o1 = asm(SKIP, Level::O1);
    assert!(body(&o0, "f").contains("local.set"), "{o0}");
    assert!(!body(&o1, "f").contains("local.set"), "{o1}");
}

const TWICE: &str = "declare i32 @h(i32)
define i32 @g(i32 %x, i32 %y) {
entry:
  %a = add i32 %x, %y
  %b = call i32 @h(i32 %a)
  %c = add i32 %b, %a
  ret i32 %c
}
";

/// A value read twice was stored and read back (`local.set N; local.get N`);
/// the pin keeps the first read on the stack with `local.tee`.
#[test]
fn a_value_read_twice_is_a_tee_when_optimizing() {
    let o0 = asm(TWICE, Level::O0);
    let o2 = asm(TWICE, Level::O2);
    assert!(!body(&o0, "g").contains("local.tee"), "{o0}");
    assert!(body(&o2, "g").contains("local.tee"), "{o2}");
}

const AFTER_STORE: &str = "define i32 @h(ptr %p, ptr %q) {
entry:
  %a = load i32, ptr %p
  store i32 1, ptr %q
  %b = add i32 %a, 1
  ret i32 %b
}
";

/// A load may not move past a store that can alias it.
#[test]
fn a_load_does_not_move_past_a_store() {
    let o2 = asm(AFTER_STORE, Level::O2);
    let text = body(&o2, "h");
    let load = text.find("i32.load").expect("the load");
    let store = text.find("i32.store").expect("the store");
    assert!(load < store, "{o2}");
}

const TWO_STORES: &str = "define void @s(ptr %p) {
entry:
  store i16 1, ptr %p
  %q = getelementptr i8, ptr %p, i32 2
  store i16 2, ptr %q
  ret void
}
";

/// The selection DAG was combined as at `-O0` on every level (`OptLevel ==
/// None`), so `MergeConsecutiveStores` never ran: matmul kept 94 `i32.store16`
/// where the pin has 26 wider stores, 101 bytes more at `-O2`.
#[test]
fn adjacent_stores_merge_when_optimizing() {
    let o0 = asm(TWO_STORES, Level::O0);
    let o2 = asm(TWO_STORES, Level::O2);
    assert!(body(&o0, "s").matches("i32.store16").count() == 2, "{o0}");
    assert!(!body(&o2, "s").contains("i32.store16"), "{o2}");
}

const TWO_BYTE_STORES: &str = "define void @s(ptr %p) {
entry:
  store i8 66, ptr %p
  %q = getelementptr i8, ptr %p, i32 1
  store i8 77, ptr %q
  ret void
}
";

/// Two adjacent constant byte stores stayed two `i32.store8`: the combiner's
/// truncating-store merge was off for wasm, so jpeg_fixed's BMP header kept
/// 4 more stores and 43 bytes over the pin at `-Os`.
#[test]
fn adjacent_byte_stores_of_constants_are_one_store16() {
    let o2 = asm(TWO_BYTE_STORES, Level::O2);
    assert!(body(&o2, "s").matches("i32.store16").count() == 1 && !body(&o2, "s").contains("i32.store8"), "{o2}");
}

const MASKED_GLOBAL_INDEX: &str =
    "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
@counts = global [256 x i16] zeroinitializer
define void @f(i8 %d, i16 %seed) {
entry:
  %z = zext i8 %d to i32
  %m = mul nuw nsw i32 %z, 7
  %s = zext i16 %seed to i32
  %a = add nuw nsw i32 %m, %s
  %i = and i32 %a, 255
  %p = getelementptr inbounds nuw [2 x i8], ptr @counts, i32 %i
  %v = load i16, ptr %p
  %w = add i16 %v, 1
  store i16 %w, ptr %p
  ret void
}
";

/// A demanded-bits rewrite of an operand left `nuw`/`nsw` on the `mul`, `add`
/// and `shl` above it (the pin clears them: "we can no longer guarantee that
/// we won't wrap"), so the `add nuw` of the address folded `counts` into the
/// load and store offsets, 5 bytes of relocation each; the pin keeps one
/// `i32.const counts` and `i32.add` (histo, 1 byte over the pin at `-Os`).
#[test]
fn a_rewritten_operand_clears_the_wrap_flags_so_the_global_is_not_folded_into_the_offset() {
    let o2 = asm(MASKED_GLOBAL_INDEX, Level::O2);
    let f = body(&o2, "f");
    assert!(f.contains("i32.const\tcounts") && !f.contains("i32.load16_u\tcounts"), "{f}");
}

const SEQUENTIAL: &str = "declare void @h(i32)
define void @k(i32 %x) {
entry:
  %a = add i32 %x, 1
  call void @h(i32 %a)
  call void @h(i32 %a)
  %b = add i32 %x, 2
  call void @h(i32 %b)
  call void @h(i32 %b)
  ret void
}
";

/// The locals of the function: the count after `.local`.
fn locals(
    text: &str,
    name: &str,
) -> usize {
    body(text, name).lines().find(|l| l.contains(".local")).map_or(0, |l| l.matches("i32").count())
}

/// Two values that are never live together each had a local of their own:
/// `RegColoring` only ran in the pin. fib's loop carried 5 locals where the
/// pin's has 3.
#[test]
fn values_that_are_not_live_together_share_a_local_when_optimizing() {
    let o0 = asm(SEQUENTIAL, Level::O0);
    let o2 = asm(SEQUENTIAL, Level::O2);
    assert_eq!(locals(&o0, "k"), 2, "{o0}");
    assert_eq!(locals(&o2, "k"), 1, "{o2}");
}

const CROSS_BLOCK_ZEXT: &str =
    "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
@t = global [256 x i8] zeroinitializer
define void @f(ptr %p, ptr %q) {
entry:
  %v = load i8, ptr %p
  %t = icmp eq i8 %v, 0
  br i1 %t, label %b, label %a
a:
  %z = zext i8 %v to i32
  %g = getelementptr i8, ptr @t, i32 %z
  store i8 1, ptr %g
  store i8 2, ptr %q
  br label %b
b:
  ret void
}
";

/// A byte loaded in one block and zero-extended in another got `and 255`
/// masks in both: the block's load was known zero-extended
/// (`ComputeLiveOutVRegInfo`) but the other block never heard. grep's
/// scan loop ran 137,508 more instructions than the pin's.
#[test]
fn a_zero_extended_load_is_not_masked_in_the_next_block() {
    let text = asm(CROSS_BLOCK_ZEXT, Level::O1);
    assert!(!body(&text, "f").contains("i32.and"), "{text}");
}

const GLOBAL_PLUS_INDEXES: &str =
    "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
@nxt = internal global [2560 x i8] zeroinitializer
@classof = internal global [256 x i8] zeroinitializer
define i32 @f(i32 %s, ptr %q) {
  store i8 1, ptr @nxt
  store i8 1, ptr @classof
  %a = shl nuw nsw i32 %s, 5
  %c8 = load i8, ptr %q
  %c = zext i8 %c8 to i32
  %g = getelementptr inbounds nuw i8, ptr @nxt, i32 %a
  %h = getelementptr inbounds nuw i8, ptr %g, i32 %c
  %v = load i8, ptr %h
  %z = zext i8 %v to i32
  ret i32 %z
}
";

/// `@nxt + (s << 5) + c` was `i32.const nxt; ...; i32.add` before a load at
/// offset 0: the global stayed innermost in the add chain, where the pin treats
/// it as a constant and moves it out to the load's offset. grep's scan loop
/// ran 65,520 more `const`/`add` pairs than clang's.
#[test]
fn a_global_in_an_index_sum_is_the_loads_offset() {
    let text = asm(GLOBAL_PLUS_INDEXES, Level::O1);
    assert!(body(&text, "f").contains("i32.load8_u\tnxt"), "{text}");
}

const OR_BRANCH: &str = "declare void @a()
declare void @b()
define void @f(i32 %x, i32 %y, i32 %z) {
  %c1 = icmp eq i32 %x, %y
  %c2 = icmp eq i32 %x, %z
  %o = select i1 %c1, i1 true, i1 %c2
  br i1 %o, label %t, label %e
t:
  call void @a()
  ret void
e:
  call void @b()
  ret void
}
";

/// A branch on `a || b` was one `setcc` pair, an `i32.and` of the inverses and
/// one `br_if`, because the builder had no cost model to ask whether to keep
/// the compares together: queens' inner loop ran 354,188 instructions against
/// the pin's 313,595. The pin branches on each compare.
#[test]
fn a_branch_on_or_of_compares_is_two_branches() {
    let text = asm(OR_BRANCH, Level::O2);
    let f = body(&text, "f");
    assert_eq!(f.matches("br_if").count(), 2, "{text}");
    assert!(!f.contains("i32.and"), "{text}");
}

const SHARED_SUB: &str = "declare void @a()
declare void @b()
define void @f(ptr %p, i32 %y, i32 %z, i32 %w) {
  %x = load i32, ptr %p
  %d = sub i32 %x, %y
  %c1 = icmp eq i32 %x, %y
  %c2 = icmp eq i32 %z, %d
  %o1 = select i1 %c1, i1 true, i1 %c2
  %c3 = icmp eq i32 %w, %d
  %o2 = select i1 %o1, i1 true, i1 %c3
  br i1 %o2, label %t, label %e
t:
  call void @a()
  ret void
e:
  call void @b()
  ret void
}
";

/// A value only the later compares of an `||` chain read was computed before
/// the first compare's branch (queens: 329,279 instructions against the
/// pin's 313,595): the pin's MachineSink moves it below that branch.
#[test]
fn a_value_the_later_compares_use_is_computed_after_the_first_branch() {
    let text = asm(SHARED_SUB, Level::O2);
    let f = body(&text, "f");
    let branch = f.find("br_if").expect("a branch");
    let sub = f.find("i32.sub").expect("the sub");
    assert!(branch < sub, "{text}");
}

const GLOBAL_PLUS_INDEX_PLUS_OFFSET: &str = "@px = hidden global [101 x i16] zeroinitializer
define void @f(i32 %i, i16 %v) {
  %a = getelementptr i8, ptr @px, i32 %i
  %b = getelementptr i8, ptr %a, i32 202
  store i16 %v, ptr %b
  ret void
}
";

/// `px + i + 202` was `i32.const px; i32.add; i32.const 202; i32.add`, the
/// addressing-mode guard having refused to regroup: particle ran 556,732
/// instructions against the pin's 433,714. The pin folds the constant into the
/// symbol, `i32.const px+202`.
#[test]
fn a_constant_added_to_a_global_and_an_index_joins_the_global() {
    let text = asm(GLOBAL_PLUS_INDEX_PLUS_OFFSET, Level::O2);
    let f = body(&text, "f");
    assert!(f.contains("px+202"), "{text}");
    assert_eq!(f.matches("i32.add").count(), 1, "{text}");
}

const COMMON_RETURN: &str = "define i32 @f(i32 %n) {
  %z = icmp eq i32 %n, 0
  br i1 %z, label %out, label %rec
rec:
  %m = add i32 %n, -1
  %r = call i32 @f(i32 %m)
  %s = shl i32 %r, 1
  %t = or i32 %s, 1
  br label %out
out:
  %v = phi i32 [ 0, %0 ], [ %t, %rec ]
  ret i32 %v
}
";

/// The join of two returns kept its phi in a local: `i32.const 0; local.set;
/// br` into a block that read it back (hanoi, 268 instructions against the
/// pin's 198). The pin copies the return into both predecessors.
#[test]
fn a_join_of_returns_returns_in_each_predecessor() {
    let text = asm(COMMON_RETURN, Level::O2);
    let f = body(&text, "f");
    assert!(!f.contains("local.set"), "{text}");
    assert!(f.contains("return"), "{text}");
}

const OBJECT_OFFSETS: &str = "declare void @use(ptr)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
define void @f() {
  %a = alloca [4 x double], align 16
  %p = getelementptr inbounds nuw i8, ptr %a, i32 16
  call void @llvm.memset.p0.i32(ptr align 16 %p, i8 0, i32 16, i1 false)
  call void @use(ptr %a)
  ret void
}
";

/// The second store of a `memset` of a stack object got `add nuw (add nuw
/// slot, 16), 8` regrouped without `nuw`, so it was `local.get; i32.const 24;
/// i32.add; ...; i64.store 0` instead of `i64.store 24` (nbody O2: 158,944
/// instructions against the pin's 147,730).
#[test]
fn the_stores_of_a_memset_on_a_stack_object_take_their_offsets() {
    let text = asm(OBJECT_OFFSETS, Level::O2);
    let f = body(&text, "f");
    assert!(f.contains("i64.store\t24"), "{text}");
    assert_eq!(f.matches("i32.add").count(), 1, "{text}");
}

const EXIT_CONSTANT: &str = "declare i1 @g(i32)
declare i1 @h(i32)
declare void @k(i32)
define i32 @f() {
entry:
  br label %l
l:
  %i = phi i32 [ 0, %entry ], [ %i1, %m ]
  %a = call i1 @g(i32 %i)
  br i1 %a, label %e, label %m
m:
  %i1 = add i32 %i, 1
  %c = call i1 @h(i32 %i1)
  br i1 %c, label %l, label %e
e:
  %r = phi i32 [ %i, %l ], [ 32, %m ]
  %s = mul i32 %r, 3
  %t = add i32 %s, 7
  call void @k(i32 %t)
  ret i32 %t
}
";

/// The constant a loop-exit edge feeds a phi was set on every iteration,
/// before the latch's branch (mandel O2: 439,218 instructions against the
/// pin's 411,831). The pin's MachineSink splits the edge and sinks the
/// constant into the new block.
#[test]
fn a_constant_for_a_loop_exit_phi_is_set_on_the_exit_edge() {
    let text = asm(EXIT_CONSTANT, Level::O2);
    let f = body(&text, "f");
    let constant = f.find("i32.const\t32").expect("the constant");
    let end_loop = f.find("end_loop").expect("the loop");
    assert!(constant > end_loop, "{text}");
}

// The loops stay rolled: the test is about where the exit blocks sit, and the
// target unrolls a loop of this size.
const TWO_LOOP_EXITS: &str = "declare void @sort(ptr, i32, i32)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture)
define i32 @f(i16 %0) {
  %2 = alloca [1024 x i32], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2)
  br label %3

3:
  %4 = phi i32 [ %12, %3 ], [ -4096, %1 ]
  %5 = phi i16 [ %0, %1 ], [ %7, %3 ]
  %6 = mul i16 %5, 25173
  %7 = add i16 %6, 13849
  %8 = and i16 %7, 32767
  %9 = zext nneg i16 %8 to i32
  %10 = getelementptr i8, ptr %2, i32 %4
  %11 = getelementptr i8, ptr %10, i32 4096
  store i32 %9, ptr %11, align 4
  %12 = add nsw i32 %4, 4
  %exitcond2 = icmp eq i32 %12, 0
  br i1 %exitcond2, label %13, label %3, !llvm.loop !0

13:
  call void @sort(ptr %2, i32 0, i32 1023)
  br label %17

14:
  %15 = add nsw i32 %18, 4
  %exitcond1 = icmp eq i32 %15, 0
  br i1 %exitcond1, label %16, label %17, !llvm.loop !0

16:
  br label %25

17:
  %18 = phi i32 [ %15, %14 ], [ -4092, %13 ]
  %19 = getelementptr i8, ptr %2, i32 %18
  %20 = getelementptr i8, ptr %19, i32 4096
  %21 = getelementptr i8, ptr %19, i32 4092
  %22 = load i32, ptr %21, align 4
  %23 = load i32, ptr %20, align 4
  %24 = icmp sgt i32 %22, %23
  br i1 %24, label %36, label %14

25:
  %26 = phi ptr [ %2, %16 ], [ %35, %25 ]
  %27 = phi i32 [ %33, %25 ], [ 0, %16 ]
  %28 = phi i32 [ %34, %25 ], [ 0, %16 ]
  %29 = load i32, ptr %26, align 4
  %30 = and i32 %28, 15
  %31 = add nuw nsw i32 %30, 1
  %32 = mul nsw i32 %29, %31
  %33 = add nsw i32 %32, %27
  %34 = add nuw nsw i32 %28, 1
  %35 = getelementptr i8, ptr %26, i32 4
  %exitcond = icmp eq i32 1024, %34
  br i1 %exitcond, label %36, label %25, !llvm.loop !0

36:
  %37 = phi i32 [ %33, %25 ], [ -1, %17 ]
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2)
  ret i32 %37
}

!0 = distinct !{!0, !1}
!1 = !{!\"llvm.loop.unroll.disable\"}
";

/// The split block of a loop exit stayed next to the loop, so the code jumped
/// over it and again out of it: two `br` against the pin's one (bench
/// quicksort, 634 bytes against 624). The pin's branch folder moves a block
/// to where a fall-through happens, or to the end.
#[test]
fn a_block_nothing_falls_into_moves_to_where_a_fall_through_happens() {
    let text = asm(TWO_LOOP_EXITS, Level::O2);
    let f = body(&text, "f");
    let jumps = f.lines().filter(|line| line.split_whitespace().next() == Some("br")).count();
    assert_eq!(jumps, 1, "{text}");
}

/// At -O1 the loop exit edge had no block of its own: `SplitPHIEdges` ran only
/// from -O2, so the phi copy sat in the latch and mandel ran 615,228
/// instructions against the pin's 579,736. The pin splits at -O1 too (the
/// coalescer is what it turns off); the exit is `br 2` out of the split block.
#[test]
fn a_loop_exit_phi_edge_has_a_block_of_its_own_at_o1() {
    let text = asm(EXIT_CONSTANT, Level::O1);
    let f = body(&text, "f");
    assert!(f.lines().any(|line| line.split_whitespace().eq(["br", "2"])), "{text}");
}

/// Ten globals a loop adds its index to, each a promotion candidate.
fn ten_counters() -> String {
    let globals: String = (1..=10).map(|k| format!("@g{k} = internal global i32 0\n")).collect();
    let body: String = (1..=10)
        .map(|k| format!("  %x{k} = load i32, ptr @g{k}\n  %y{k} = add i32 %x{k}, %v\n  store i32 %y{k}, ptr @g{k}\n"))
        .collect();
    format!(
        "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
{globals}declare i32 @h(i32) nounwind willreturn memory(none)
define void @f(i32 %n) {{
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %v = call i32 @h(i32 %i)
{body}  %i1 = add nuw i32 %i, 1
  %c = icmp eq i32 %i1, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}}
"
    )
}

/// LICM refused promotions past eight values held across the loop, a limit
/// the pin does not have (wasm locals are virtual): bench nbody_fixed -O2
/// kept `pos_x[0]` and `pos_y[0]` in memory, 2 loads more a step; here five of
/// ten stay stores in the loop.
#[test]
fn every_global_a_loop_updates_is_kept_in_a_local_whatever_the_count() {
    let text = asm(&ten_counters(), Level::O2);
    let f = body(&text, "f");
    let start = f.find("loop").expect("a loop");
    let end = f.rfind("end_loop").expect("its end");
    let accesses = f[start..end].matches("i32.load\tg").count() + f[start..end].matches("i32.store\tg").count();
    assert_eq!(accesses, 0, "{text}");
}
