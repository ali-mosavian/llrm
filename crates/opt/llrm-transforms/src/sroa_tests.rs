//! Each test is a symptom: the text a rewrite must leave, which an alloca
//! left as it is does not.

use llrm_analysis::testing::DOS;
use llrm_mir::module::Module;

use super::Sroa;
use crate::testing::{managed, parsed};

fn module(body: &str) -> Module {
    parsed(&format!("{DOS}{body}"))
}

fn sroa(body: &str) -> String {
    managed(&mut module(body), Sroa)
}

/// A struct of two ints, each field stored and loaded by its own address: two
/// partitions, each promoted. The alloca, its stores and loads are gone.
#[test]
fn test_a_struct_of_two_ints_is_split_and_promoted() {
    let after = sroa(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %s = alloca { i32, i32 }
  %p0 = getelementptr { i32, i32 }, ptr %s, i32 0, i32 0
  %p1 = getelementptr { i32, i32 }, ptr %s, i32 0, i32 1
  store i32 %a, ptr %p0
  store i32 %b, ptr %p1
  %x = load i32, ptr %p0
  %y = load i32, ptr %p1
  %r = add i32 %x, %y
  ret i32 %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("add i32 %a, %b"), "{after}");
}

/// An array indexed by constant byte offsets: one partition per element.
#[test]
fn test_an_array_indexed_by_constants_is_split_and_promoted() {
    let after = sroa(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %s = alloca [2 x i32]
  %p1 = getelementptr i8, ptr %s, i16 4
  store i32 %a, ptr %s
  store i32 %b, ptr %p1
  %x = load i32, ptr %s
  %y = load i32, ptr %p1
  %r = sub i32 %x, %y
  ret i32 %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("sub i32 %a, %b"), "{after}");
}

/// An i64 written as two i32 halves and read whole: the load is split between
/// the partitions and the halves are put together with shifts and an `or`; no
/// memory is left.
#[test]
fn test_an_i64_written_as_two_i32_halves_is_widened_and_promoted() {
    let after = sroa(
        "define i64 @f(i32 %a, i32 %b) {
b0:
  %s = alloca i64
  %hi = getelementptr i8, ptr %s, i16 4
  store i32 %a, ptr %s
  store i32 %b, ptr %hi
  %v = load i64, ptr %s
  ret i64 %v
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("zext i32 %a") && after.contains("zext i32 %b") && after.contains(" or "), "{after}");
}

/// A first-class aggregate stored and loaded: split into its leaves first,
/// then promoted.
#[test]
fn test_an_aggregate_store_and_load_are_split_into_leaves_and_promoted() {
    let after = sroa(
        "define i32 @f({ i32, i32 } %v) {
b0:
  %s = alloca { i32, i32 }
  store { i32, i32 } %v, ptr %s
  %w = load { i32, i32 }, ptr %s
  %x = extractvalue { i32, i32 } %w, 1
  ret i32 %x
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load "), "{after}");
}

/// A pointer to the alloca goes to a call: it escapes, and nothing is split.
#[test]
fn test_an_escaping_alloca_stays() {
    let after = sroa(
        "declare void @use(ptr)

define i32 @f(i32 %a, i32 %b) {
b0:
  %s = alloca { i32, i32 }
  %p1 = getelementptr { i32, i32 }, ptr %s, i32 0, i32 1
  store i32 %a, ptr %s
  store i32 %b, ptr %p1
  call void @use(ptr %s)
  %x = load i32, ptr %p1
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca { i32, i32 }") && after.contains("call void @use(ptr %s)"), "{after}");
    assert_eq!(after.matches("store").count(), 2, "{after}");
}

/// An array indexed by a variable: the offset is unknown, nothing is split.
#[test]
fn test_an_array_with_a_variable_index_stays() {
    let after = sroa(
        "define i32 @f(i32 %a, i16 %i) {
b0:
  %s = alloca [2 x i32]
  %p = getelementptr i32, ptr %s, i16 %i
  store i32 %a, ptr %s
  store i32 7, ptr %p
  %x = load i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca [2 x i32]") && after.contains("store i32 7, ptr %p"), "{after}");
}

/// A volatile access cannot be promoted: the alloca and the access stay.
#[test]
fn test_a_volatile_store_keeps_its_alloca() {
    let after = sroa(
        "define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  store volatile i32 %a, ptr %s
  %x = load i32, ptr %s
  ret i32 %x
}
",
    );
    assert!(after.contains("alloca i32") && after.contains("store volatile i32 %a"), "{after}");
}

/// A struct filled by a memcpy and read by field: the copy becomes a load and
/// a store of each part (the stores promoted away), so the loads read the
/// source. With the memcpy escaping the alloca, it stays and so do the loads.
#[test]
fn test_a_struct_copied_by_memcpy_and_read_by_field_is_split() {
    let after = sroa(
        "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)

define i32 @f(ptr %src) {
b0:
  %s = alloca { i32, i32 }
  call void @llvm.memcpy.p0.p0.i16(ptr %s, ptr %src, i16 8, i1 false)
  %p1 = getelementptr i8, ptr %s, i16 4
  %x = load i32, ptr %s
  %y = load i32, ptr %p1
  %r = add i32 %x, %y
  ret i32 %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("call void @llvm.memcpy"), "{after}");
    assert_eq!(after.matches("load i32").count(), 2, "{after}");
}

/// A local set by a memset and read whole: the byte is spread over the integer
/// and the load is that constant.
#[test]
fn test_a_memset_of_a_local_then_a_load_is_the_constant() {
    let after = sroa(
        "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)

define i32 @f() {
b0:
  %s = alloca i32
  call void @llvm.memset.p0.i16(ptr %s, i8 1, i16 4, i1 false)
  %v = load i32, ptr %s
  ret i32 %v
}
",
    );
    assert!(
        !after.contains("alloca") && !after.contains("call void @llvm.memset") && !after.contains("load"),
        "{after}"
    );
    assert!(after.contains("ret i32 16843009"), "{after}");
}

/// Lifetime markers about a local do not keep it in memory.
#[test]
fn test_lifetime_markers_about_a_local_go_with_it() {
    let after = sroa(
        "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)

define i32 @f(i32 %a) {
b0:
  %s = alloca i32
  call void @llvm.lifetime.start.p0(i64 4, ptr %s)
  store i32 %a, ptr %s
  %v = load i32, ptr %s
  call void @llvm.lifetime.end.p0(i64 4, ptr %s)
  ret i32 %v
}
",
    );
    assert!(
        !after.contains("alloca") && !after.contains("call void @llvm.lifetime") && !after.contains("load"),
        "{after}"
    );
    assert!(after.contains("ret i32 %a"), "{after}");
}

/// A select of two locals' addresses, loaded: the load becomes a select of the
/// values (the loads speculated and promoted), so no memory is left. A select
/// that aborts the slice walk leaves both allocas.
#[test]
fn test_a_load_through_a_select_of_two_locals_is_speculated() {
    let after = sroa(
        "define i32 @f(i1 %c, i32 %a, i32 %b) {
b0:
  %x = alloca i32
  %y = alloca i32
  store i32 %a, ptr %x
  store i32 %b, ptr %y
  %p = select i1 %c, ptr %x, ptr %y
  %v = load i32, ptr %p
  ret i32 %v
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("load") && !after.contains("store"), "{after}");
    assert!(after.contains("select i1 %c, i32 %a, i32 %b"), "{after}");
}

/// A phi of two locals' addresses, loaded in its block: the loads go into the
/// predecessors and are promoted.
#[test]
fn test_a_load_through_a_phi_of_two_locals_is_speculated() {
    let after = sroa(
        "define i32 @f(i1 %c, i32 %a, i32 %b) {
b0:
  %x = alloca i32
  %y = alloca i32
  store i32 %a, ptr %x
  store i32 %b, ptr %y
  br i1 %c, label %t, label %e

t:
  br label %j

e:
  br label %j

j:
  %p = phi ptr [ %x, %t ], [ %y, %e ]
  %v = load i32, ptr %p
  ret i32 %v
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("load") && !after.contains("store"), "{after}");
    assert!(after.contains("phi i32 [ %a, %t ], [ %b, %e ]"), "{after}");
}

/// `presplitLoadsAndStores`: an i32 loaded from two pointers and stored over
/// the next two. The pointers are not splittable, so the i32 access
/// (splittable) straddles their partitions; split into two i16 loads and
/// stores, all three parts are promoted. Not split, the alloca stays. LLVM's
/// `sroa<preserve-cfg>` gives the same, a ptrtoint and an inttoptr for each
/// part (its doc example uses floats, which are not reinterpreted here: isel
/// selects no such bitcast).
#[test]
fn test_an_i32_load_and_store_pair_across_pointers_is_presplit() {
    let after = sroa(
        "define ptr @f(ptr %x, ptr %y) {
b0:
  %a = alloca [6 x i8]
  %g2 = getelementptr i8, ptr %a, i16 2
  %g4 = getelementptr i8, ptr %a, i16 4
  store ptr %x, ptr %a
  store ptr %y, ptr %g2
  %v = load i32, ptr %a
  store i32 %v, ptr %g2
  %f1 = load ptr, ptr %g2
  %f2 = load ptr, ptr %g4
  %c = icmp eq ptr %f1, %f2
  %r = select i1 %c, ptr %f1, ptr %f2
  ret ptr %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("load") && !after.contains("store"), "{after}");
}

/// A float and an int of one size in one slot are not reinterpreted (isel
/// selects no such bitcast): the slot stays in memory.
#[test]
fn test_a_float_read_as_an_int_is_not_bitcast() {
    let after = sroa(
        "define i32 @f(float %x) {
b0:
  %a = alloca i32
  store float %x, ptr %a
  %v = load i32, ptr %a
  ret i32 %v
}
",
    );
    assert!(after.contains("alloca") && !after.contains("bitcast"), "{after}");
}

/// An i32 stored whole and its first byte loaded: the store is cut into a
/// 1-byte and a 3-byte piece, and the 3-byte piece was an `i24` that isel
/// refused (tests/run `struct_with_i64`). A store is not cut into a piece
/// that is not a machine integer.
#[test]
fn test_a_store_is_not_cut_into_an_i24() {
    let after = sroa(
        "define i8 @f(i32 %x) {
b0:
  %a = alloca [4 x i8]
  store i32 %x, ptr %a
  %v = load i8, ptr %a
  ret i8 %v
}
",
    );
    assert!(!after.contains("i24"), "{after}");
}

/// Old: a partition read before any store was `poison`, then the zero of the
/// type; the pin's is `undef`.
#[test]
fn test_a_partition_nothing_wrote_reads_as_undef_not_poison() {
    let after = sroa(
        "define i32 @f(i32 %a) {
b0:
  %s = alloca { i32, i32 }
  %p0 = getelementptr { i32, i32 }, ptr %s, i32 0, i32 0
  %p1 = getelementptr { i32, i32 }, ptr %s, i32 0, i32 1
  store i32 %a, ptr %p0
  %y = load i32, ptr %p1
  %r = add i32 %y, %a
  ret i32 %r
}
",
    );
    assert!(!after.contains("poison") && !after.contains("alloca"), "{after}");
    assert!(after.contains("undef"), "{after}");
}

/// Old: an offset was a signed number that did not wrap at the index width, so
/// the byte 32798 of a 32,800-byte array, written `i16 -32738` (what 16 bits
/// make of it), was "below the alloca": the store and its load were dead and
/// the load read poison (c/wide_frame at -O2 printed 17, wanted 75). LLVM's
/// offset is an `APInt` of the index width, compared as unsigned.
#[test]
fn test_an_offset_past_32k_written_as_a_negative_i16_is_inside_the_alloca() {
    let after = sroa(
        "define i16 @f() {
b0:
  %s = alloca [32800 x i8]
  %p = getelementptr inbounds i8, ptr %s, i16 -32738
  store i16 9, ptr %p
  %x = load i16, ptr %p
  ret i16 %x
}
",
    );
    assert!(!after.contains("poison"), "{after}");
    assert!(after.contains("ret i16 9"), "{after}");
}

/// SROA over `n` split-and-promoted structs: the least work of three runs.
fn split_all(n: usize) -> f64 {
    let mut body = String::from("define i32 @f(i32 %a) {\nb0:\n");
    for i in 0..n {
        body += &format!(
            "  %s{i} = alloca {{ i32, i32 }}\n  %p{i} = getelementptr {{ i32, i32 }}, ptr %s{i}, i32 0, i32 1\n  store i32 %a, ptr %s{i}\n  store i32 %a, ptr %p{i}\n  %l{i} = load i32, ptr %p{i}\n"
        );
    }
    body += "  ret i32 %a\n}\n";
    (0..3)
        .map(|_| {
            let mut m = module(&body);
            llrm_support::debug::work_of(|| managed(&mut m, Sroa)).1 as f64
        })
        .fold(f64::MAX, f64::min)
}

/// The worklists were `Vec`s searched and filtered for every alloca, and the
/// allocas it deleted a bitset as wide as the largest id: 4x the allocas took
/// 7.4x as long (d_faces: sroa@646 2.6x on a 1.37x body). LLVM's are
/// `SetVector`s, `set_subtract` of the few deleted.
#[test]
fn test_four_times_the_allocas_take_about_four_times_as_long() {
    let (small, large) = (split_all(1000), split_all(4000));
    assert!(large / small < 6.0, "{small:.0} work for 1000, {large:.0} for 4000: {:.2}x", large / small);
}

/// A load at a variable offset escapes the pointer read-only, so the alloca
/// stays; the stores at known offsets still reach the loads at known offsets.
/// The pin's `propagateStoredValuesToLoads` makes a phi of what each path
/// stores. Without it the loads stayed (bench_fpbench, 11% slower once the old
/// promotion went: its float arrays are read in a loop by index).
#[test]
fn test_stores_reach_the_known_offset_loads_of_an_alloca_read_at_an_unknown_one() {
    let after = sroa(
        "define float @f(float %a, i1 %c, i32 %i) {
b0:
  %s = alloca [8 x i8]
  store float %a, ptr %s
  br i1 %c, label %b1, label %b2

b1:
  store float 2.000000e+00, ptr %s
  br label %b3

b2:
  br label %b3

b3:
  %v = load float, ptr %s
  %q = getelementptr inbounds i32, ptr %s, i32 %i
  %w = load float, ptr %q
  %r = fadd float %v, %w
  ret float %r
}
",
    );
    assert!(after.contains("phi float"), "{after}");
    assert!(after.contains("[ 2.000000e+00, %b1 ]") && after.contains("[ %a, %b2 ]"), "{after}");
    assert!(!after.contains("load float, ptr %s"), "{after}");
    // The alloca, the stores and the load at the unknown offset stay.
    assert!(after.contains("alloca") && after.contains("store float %a, ptr %s"), "{after}");
    assert!(after.contains("load float, ptr %q"), "{after}");
}

/// A slice of another type or range in the partition ends the promotion: an
/// `i32` and a `float` at one offset are not made into a phi.
#[test]
fn test_loads_and_stores_of_two_types_at_one_offset_are_not_forwarded() {
    let after = sroa(
        "define float @f(float %a, i32 %b, i1 %c, i32 %i) {
b0:
  %s = alloca [8 x i8]
  store float %a, ptr %s
  br i1 %c, label %b1, label %b2

b1:
  store i32 %b, ptr %s
  br label %b3

b2:
  br label %b3

b3:
  %v = load float, ptr %s
  %q = getelementptr inbounds i32, ptr %s, i32 %i
  %w = load float, ptr %q
  %r = fadd float %v, %w
  ret float %r
}
",
    );
    assert!(!after.contains("phi"), "{after}");
    assert!(after.contains("load float, ptr %s"), "{after}");
}

/// `isInstructionTriviallyDead` named opcodes only: the pure call whose value
/// only a dead user of the alloca (an unused address) held stayed after SROA
/// deleted that user.
#[test]
fn test_the_pure_call_only_a_dead_alloca_user_held_goes_with_it() {
    let after = sroa(
        "define internal i32 @pure(i32 %a) memory(none) nounwind willreturn {
b0:
  %r = add i32 %a, 1
  ret i32 %r
}

define i32 @f(i32 %a) {
b0:
  %s = alloca [4 x i32]
  %v = call i32 @pure(i32 %a)
  %p = getelementptr [4 x i32], ptr %s, i32 0, i32 %v
  ret i32 %a
}
",
    );
    assert!(!after.contains("call i32 @pure"), "{after}");
}

/// `-g` left the variables it declares to `mem2reg`, which promotes them in
/// another order: the loop header's phis came out as `i` then `crc` where they
/// are `crc` then `i` without `-g`, and the code of bench/crc (and ten more
/// programs) differed with `-g` by the order of two `mov`s.
#[test]
fn test_a_declared_scalar_is_promoted_in_the_order_it_is_without_debug_info() {
    let body = |declares: bool| {
        let (d1, d2) =
            if declares { ("  #dbg_declare(ptr %crc, !0)\n", "  #dbg_declare(ptr %i, !1)\n") } else { ("", "") };
        format!(
            "define i32 @f(i32 %s) {{
b0:
  %crc = alloca i32
{d1}  %i = alloca i16
{d2}  store i32 %s, ptr %crc
  store i16 0, ptr %i
  br label %b1
b1:
  %iv = load i16, ptr %i
  %c = icmp slt i16 %iv, 9
  br i1 %c, label %b2, label %b3
b2:
  %v = load i32, ptr %crc
  %w = mul i32 %v, 3
  store i32 %w, ptr %crc
  %n = add i16 %iv, 1
  store i16 %n, ptr %i
  br label %b1
b3:
  %r = load i32, ptr %crc
  ret i32 %r
}}

!0 = !{{!\"f\", !\"crc\", !2, i64 0}}
!1 = !{{!\"f\", !\"i\", !3, i64 0}}
!2 = !{{!\"long\"}}
!3 = !{{!\"short\"}}
"
        )
    };
    let plain = sroa(&body(false));
    let declared = sroa(&body(true));
    let code = |text: &str| {
        text.lines()
            .filter(|line| !line.trim_start().starts_with("#dbg_") && !line.starts_with('!'))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(code(&declared).trim_end(), code(&plain).trim_end(), "{declared}\n----\n{plain}");
}

/// A struct `-g` declares was left in memory (bench/nbody -O2: three phis
/// where the build without `-g` had them reordered, and slots of different
/// offsets): SROA splits and promotes it as it does without `-g`. The
/// variable's location goes with the alloca, which is reported dropped.
#[test]
fn test_a_declared_struct_is_split_and_promoted_as_without_debug_info() {
    let body = |declares: bool| {
        let d = if declares { "  #dbg_declare(ptr %s, !0)\n" } else { "" };
        format!(
            "define i32 @f(i32 %a, i32 %b) {{
b0:
  %s = alloca {{ i32, i32 }}
{d}  %p0 = getelementptr {{ i32, i32 }}, ptr %s, i32 0, i32 0
  %p1 = getelementptr {{ i32, i32 }}, ptr %s, i32 0, i32 1
  store i32 %a, ptr %p0
  store i32 %b, ptr %p1
  %x = load i32, ptr %p0
  %y = load i32, ptr %p1
  %r = add i32 %x, %y
  ret i32 %r
}}

!0 = !{{!\"f\", !\"s\", !1, i64 0}}
!1 = !{{!\"pair\"}}
"
        )
    };
    let code = |text: &str| {
        text.lines()
            .filter(|line| !line.trim_start().starts_with("#dbg_") && !line.starts_with('!'))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let (plain, declared) = (sroa(&body(false)), sroa(&body(true)));
    assert!(!declared.contains("alloca"), "{declared}");
    assert_eq!(code(&declared).trim_end(), code(&plain).trim_end(), "{declared}\n----\n{plain}");
}

/// QCport `_ent_lights_selftest@3`: the local is read and written through an
/// `addrspacecast` (the far-data view of the stack). The pin's slice builder
/// follows the cast (`visitAddrSpaceCastInst`, SROA.cpp#L1091); ours treated
/// it as a use it could not follow, so 26 stores and 92 loads stayed.
#[test]
fn test_an_alloca_read_through_an_addrspacecast_is_split_and_promoted() {
    let after = sroa(
        "define i16 @f(i16 %a, i16 %b) {
b0:
  %s = alloca [8 x i8]
  %p1 = getelementptr inbounds nuw i8, ptr %s, i16 4
  store i16 %a, ptr %s
  store i16 %b, ptr %p1
  %c = addrspacecast ptr %s to ptr addrspace(1)
  %q1 = getelementptr inbounds nuw i8, ptr addrspace(1) %c, i16 4
  %x = load i16, ptr addrspace(1) %c
  %y = load i16, ptr addrspace(1) %q1
  %r = add i16 %x, %y
  ret i16 %r
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("add i16 %a, %b"), "{after}");
}

/// The same with a store through the cast and a load through the alloca.
#[test]
fn test_a_store_through_an_addrspacecast_reaches_a_load_of_the_alloca() {
    let after = sroa(
        "define i16 @f(i16 %a) {
b0:
  %s = alloca i16
  %c = addrspacecast ptr %s to ptr addrspace(1)
  store i16 %a, ptr addrspace(1) %c
  %x = load i16, ptr %s
  ret i16 %x
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("load"), "{after}");
    assert!(after.contains("ret i16 %a"), "{after}");
}

/// Seeing through an `addrspacecast` makes an alloca splittable by a memcpy
/// that takes the cast pointer: the rewritten call still takes the pointer
/// type it declares (`getNewAllocaSlicePtr`'s
/// `CreatePointerBitCastOrAddrSpaceCast`, SROA.cpp#L2042). The first version
/// passed the new alloca's `ptr` where the callee wants `ptr addrspace(1)`:
/// LLRM_VERIFY refused nib-examples-entries.
#[test]
fn test_a_memcpy_through_an_addrspacecast_keeps_its_pointer_type() {
    let after = sroa(
        "declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1), ptr addrspace(1), i16, i1)

define i16 @f(ptr addrspace(1) %src) {
b0:
  %s = alloca [8 x i8]
  %c = addrspacecast ptr %s to ptr addrspace(1)
  call void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) %c, ptr addrspace(1) %src, i16 8, i1 false)
  %v = load i16, ptr %s
  ret i16 %v
}
",
    );
    assert!(after.contains("= addrspacecast ptr %s.sroa.1 to ptr addrspace(1)"), "{after}");
    assert!(!after.contains("memcpy.p1.p1.i16(ptr %"), "{after}");
}
