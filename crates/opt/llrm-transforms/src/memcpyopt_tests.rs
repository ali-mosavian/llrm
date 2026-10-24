//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what `opt -passes=memcpyopt` makes of the same body
//! (checked with `tools/difftest`).

use super::MemCpyOpt;
use crate::testing::{managed, parsed};

const DECLARES: &str = "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
";

const GLOBALS: &str = "@a = global [64 x i8] zeroinitializer
@b = global [64 x i8] zeroinitializer
@c = global [64 x i8] zeroinitializer
@k = constant [64 x i8] zeroinitializer
";

/// @f after the pass, from `define void @f`.
fn after_f(body: &str) -> String {
    let mut module = parsed(&format!("{DECLARES}\n{GLOBALS}\n{body}"));
    let text = managed(&mut module, MemCpyOpt);
    text[text.find("define void @f").expect("@f")..].replace("b0:\n", "")
}

fn memcpy(
    to: &str,
    from: &str,
    bytes: u32,
) -> String {
    format!("  call void @llvm.memcpy.p0.p0.i64(ptr {to}, ptr {from}, i64 {bytes}, i1 false)\n")
}

fn body(lines: &[String]) -> String {
    format!("define void @f() {{\nb0:\n{}  ret void\n}}\n", lines.concat())
}

/// A memcpy of a pointer to itself is a no-op: gone.
#[test]
fn test_a_memcpy_to_itself_is_removed() {
    assert_eq!(after_f(&body(&[memcpy("@a", "@a", 16)])), "define void @f() {\n  ret void\n}\n");
}

/// A memcpy of no bytes is gone.
#[test]
fn test_a_memcpy_of_no_bytes_is_removed() {
    assert_eq!(after_f(&body(&[memcpy("@b", "@a", 0)])), "define void @f() {\n  ret void\n}\n");
}

/// A memcpy of what a memcpy copied copies from the first one's source.
#[test]
fn test_a_memcpy_of_a_memcpy_copies_from_its_source() {
    let before = body(&[memcpy("@b", "@a", 16), memcpy("@c", "@b", 16)]);
    let expected =
        "define void @f() {\n".to_owned() + &memcpy("@b", "@a", 16) + &memcpy("@c", "@a", 16) + "  ret void\n}\n";
    assert_eq!(after_f(&before), expected);
}

/// A write to the first copy's source between the two copies keeps the second
/// reading the first copy's destination.
#[test]
fn test_a_write_to_the_source_between_stops_the_forwarding() {
    let before = body(&[memcpy("@b", "@a", 16), "  store i8 1, ptr @a\n".to_owned(), memcpy("@c", "@b", 16)]);
    assert_eq!(after_f(&before), before.replace("b0:\n", ""));
}

/// A copy out of a constant that repeats one byte is a memset of that byte.
#[test]
fn test_a_memcpy_from_a_zero_constant_is_a_memset() {
    assert_eq!(
        after_f(&body(&[memcpy("@b", "@k", 16)])),
        "define void @f() {\n  call void @llvm.memset.p0.i64(ptr @b, i8 0, i64 16, i1 false)\n  ret void\n}\n"
    );
}

/// A memset followed by a memcpy over its start only needs to set the rest.
#[test]
fn test_a_memset_only_sets_what_the_memcpy_after_it_leaves() {
    let before =
        body(&["  call void @llvm.memset.p0.i64(ptr @a, i8 0, i64 64, i1 false)\n".to_owned(), memcpy("@a", "@b", 16)]);
    assert_eq!(
        after_f(&before),
        "define void @f() {
  %0 = getelementptr i8, ptr @a, i64 16
  call void @llvm.memset.p0.i64(ptr align 1 %0, i8 0, i64 48, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr @a, ptr @b, i64 16, i1 false)
  ret void
}
"
    );
}

/// A memset as long as the memcpy after it is overwritten whole: gone.
#[test]
fn test_a_memset_the_memcpy_overwrites_whole_is_removed() {
    let before =
        body(&["  call void @llvm.memset.p0.i64(ptr @a, i8 0, i64 16, i1 false)\n".to_owned(), memcpy("@a", "@b", 16)]);
    assert_eq!(after_f(&before), "define void @f() {\n".to_owned() + &memcpy("@a", "@b", 16) + "  ret void\n}\n");
}

/// A memcpy of bytes a memset set is a memset of them.
#[test]
fn test_a_memcpy_of_a_memset_is_a_memset() {
    let before =
        body(&["  call void @llvm.memset.p0.i64(ptr @a, i8 1, i64 32, i1 false)\n".to_owned(), memcpy("@b", "@a", 16)]);
    assert_eq!(
        after_f(&before),
        "define void @f() {
  call void @llvm.memset.p0.i64(ptr @a, i8 1, i64 32, i1 false)
  call void @llvm.memset.p0.i64(ptr @b, i8 1, i64 16, i1 false)
  ret void
}
"
    );
}

/// A copy out of an alloca nothing wrote reads undefined bytes: gone.
#[test]
fn test_a_memcpy_of_an_unwritten_alloca_is_removed() {
    let before =
        "define void @f() {\nb0:\n  %t = alloca [16 x i8]\n".to_owned() + &memcpy("@b", "%t", 16) + "  ret void\n}\n";
    assert_eq!(after_f(&before), "define void @f() {\n  %t = alloca [16 x i8]\n  ret void\n}\n");
}

/// A memmove whose source cannot meet its destination is a memcpy.
#[test]
fn test_a_memmove_between_distinct_objects_is_a_memcpy() {
    let before = body(&["  call void @llvm.memmove.p0.p0.i64(ptr @b, ptr @a, i64 16, i1 false)\n".to_owned()]);
    assert_eq!(after_f(&before), "define void @f() {\n".to_owned() + &memcpy("@b", "@a", 16) + "  ret void\n}\n");
}

/// A memmove over its own source stays a memmove.
#[test]
fn test_a_memmove_over_its_own_source_stays() {
    let text = "define void @f() {
b0:
  %p = getelementptr i8, ptr @a, i64 4
  call void @llvm.memmove.p0.p0.i64(ptr %p, ptr @a, i64 16, i1 false)
  ret void
}
";
    assert_eq!(after_f(text), text.replace("b0:\n", ""));
}

/// A volatile memcpy is left alone.
#[test]
fn test_a_volatile_memcpy_is_left_alone() {
    let text =
        "define void @f() {\nb0:\n  call void @llvm.memcpy.p0.p0.i64(ptr @a, ptr @a, i64 16, i1 true)\n  ret void\n}\n";
    assert_eq!(after_f(text), text.replace("b0:\n", ""));
}

/// `@f` of `text` after the pass.
fn after(text: &str) -> String {
    let mut module = parsed(&format!("{DECLARES}\n{GLOBALS}\n{text}"));
    managed(&mut module, MemCpyOpt)
}

fn copy_aligned(
    to: &str,
    from: &str,
    bytes: u32,
) -> String {
    format!("  call void @llvm.memcpy.p0.p0.i64(ptr align 1 {to}, ptr align 1 {from}, i64 {bytes}, i1 false)\n")
}

/// An aggregate loaded and stored right after is one memcpy (`h` of
/// `opt -passes=memcpyopt`): the load and the store were two 16-byte moves.
#[test]
fn test_a_loaded_aggregate_stored_is_a_memcpy() {
    let before =
        "define void @f() {\nb0:\n  %v = load [16 x i8], ptr @a\n  store [16 x i8] %v, ptr @b\n  ret void\n}\n";
    let text = after(before);
    assert!(
        text.contains(&copy_aligned("@b", "@a", 16)) && !text.contains("load") && !text.contains("store"),
        "{text}"
    );
}

/// Pointers that may meet make the copy a memmove (`g`).
#[test]
fn test_a_loaded_aggregate_stored_through_a_pointer_that_may_meet_is_a_memmove() {
    let before = "define void @f(ptr %p, ptr %q) {\nb0:\n  %v = load [16 x i8], ptr %q\n  store [16 x i8] %v, ptr %p\n  ret void\n}\n";
    let text = after(before);
    assert!(
        text.contains("call void @llvm.memmove.p0.p0.i64(ptr align 1 %p, ptr align 1 %q, i64 16, i1 false)")
            && !text.contains("load")
            && !text.contains("store"),
        "{text}"
    );
}

/// A write to the loaded bytes between the load and the store: the store is
/// lifted above it and the copy goes there (`moveUp`, `f`).
#[test]
fn test_the_store_of_a_loaded_aggregate_is_lifted_above_a_write_to_what_was_loaded() {
    let before = "define void @f() {\nb0:\n  %v = load [16 x i8], ptr @a\n  call void @llvm.memset.p0.i64(ptr @a, i8 0, i64 16, i1 false)\n  store [16 x i8] %v, ptr @b\n  ret void\n}\n";
    let text = after(before);
    let copy = text.find(&copy_aligned("@b", "@a", 16)).unwrap_or_else(|| panic!("{text}"));
    let set = text.find("@llvm.memset.p0.i64(ptr @a").expect("the memset");
    assert!(copy < set && !text.contains("load") && !text.contains("store"), "{text}");
}

/// The first write to the loaded bytes reads what the store writes: the store
/// cannot move above it (`moveUp`'s first check), and the load and store stay.
#[test]
fn test_a_store_the_write_in_between_reads_is_not_lifted() {
    let before = "define void @f() {\nb0:\n  %v = load [16 x i8], ptr @a\n  call void @llvm.memcpy.p0.p0.i64(ptr @a, ptr @b, i64 16, i1 false)\n  store [16 x i8] %v, ptr @b\n  ret void\n}\n";
    let text = after(before);
    assert!(text.contains("load [16 x i8]") && text.contains("store [16 x i8]"), "{text}");
}

/// A scalar load and store are not a copy (the pin takes aggregates only).
#[test]
fn test_a_loaded_scalar_stored_stays() {
    let before = "define void @f() {\nb0:\n  %v = load i32, ptr @a\n  store i32 %v, ptr @b\n  ret void\n}\n";
    let text = after(before);
    assert!(text.contains("load i32") && text.contains("store i32"), "{text}");
}

const SLOT_DECLARES: &str = "declare void @f(ptr nocapture) nounwind willreturn memory(argmem: write)
declare void @use(ptr)
";

/// The call, its alloca `%tmp` and the copy of it to `%dst`, after the pass.
fn slot(body: &str) -> String {
    let mut module = parsed(&format!("{DECLARES}{SLOT_DECLARES}\n{body}"));
    managed(&mut module, MemCpyOpt)
}

/// A call that fills a fresh alloca, copied to another: the call fills the
/// destination and the copy is gone (`opt -passes=memcpyopt` `@a`).
#[test]
fn test_a_call_slot_is_the_copys_destination() {
    let text = slot(
        "define void @f2() {\nb0:\n  %dst = alloca [16 x i8]\n  %tmp = alloca [16 x i8]\n  call void @f(ptr nocapture %tmp)\n  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %tmp, i64 16, i1 false)\n  call void @use(ptr %dst)\n  ret void\n}\n",
    );
    assert!(
        text.contains("call void @f(ptr nocapture %dst)") && !text.contains("@llvm.memcpy.p0.p0.i64(ptr %dst"),
        "{text}"
    );
}

/// The same through a load and a store of a scalar (`@scalar`).
#[test]
fn test_a_call_slot_through_a_loaded_scalar_stored() {
    let text = slot(
        "define void @f2() {\nb0:\n  %dst = alloca i32\n  %tmp = alloca i32\n  call void @f(ptr nocapture %tmp)\n  %v = load i32, ptr %tmp\n  store i32 %v, ptr %dst\n  call void @use(ptr %dst)\n  ret void\n}\n",
    );
    assert!(
        text.contains("call void @f(ptr nocapture %dst)") && !text.contains("load") && !text.contains("store"),
        "{text}"
    );
}

/// The destination is used between the call and the copy: the call may not
/// write it early (`@between`).
#[test]
fn test_a_call_slot_is_not_taken_when_the_destination_is_used_between() {
    let text = slot(
        "define void @f2() {\nb0:\n  %dst = alloca [16 x i8]\n  %tmp = alloca [16 x i8]\n  call void @f(ptr nocapture %tmp)\n  call void @use(ptr %dst)\n  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %tmp, i64 16, i1 false)\n  call void @use(ptr %dst)\n  ret void\n}\n",
    );
    assert!(
        text.contains("call void @f(ptr nocapture %tmp)") && text.contains("@llvm.memcpy.p0.p0.i64(ptr %dst"),
        "{text}"
    );
}

/// The source is read elsewhere (`@other`): it holds more than the call wrote.
#[test]
fn test_a_call_slot_is_not_taken_when_the_source_is_read_elsewhere() {
    let text = slot(
        "define void @f2() {\nb0:\n  %dst = alloca [16 x i8]\n  %tmp = alloca [16 x i8]\n  call void @f(ptr nocapture %tmp)\n  %x = load i8, ptr %tmp\n  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %tmp, i64 16, i1 false)\n  call void @use(ptr %dst)\n  ret void\n}\n",
    );
    assert!(
        text.contains("call void @f(ptr nocapture %tmp)") && text.contains("@llvm.memcpy.p0.p0.i64(ptr %dst"),
        "{text}"
    );
}

const MOVE_DECLARES: &str = "declare void @fill(ptr nocapture) nounwind willreturn memory(argmem: write)
declare void @read(ptr nocapture) nounwind willreturn memory(argmem: read)
";

/// A module of `MOVE_DECLARES` and `body`, after the pass.
fn moved(body: &str) -> String {
    let mut module = parsed(&format!("{DECLARES}{MOVE_DECLARES}\n{body}"));
    managed(&mut module, MemCpyOpt)
}

/// Both slots are read after the copy, and neither is written: they are one
/// slot and the copy is gone (`opt -passes=memcpyopt` `@both_read`).
#[test]
fn test_a_copy_of_one_stack_slot_to_another_makes_them_one() {
    let text = moved(
        "define void @f2() {\nb0:\n  %a = alloca [16 x i8]\n  %b = alloca [16 x i8]\n  call void @fill(ptr nocapture %a)\n  call void @llvm.memcpy.p0.p0.i64(ptr %b, ptr %a, i64 16, i1 false)\n  call void @read(ptr nocapture %a)\n  call void @read(ptr nocapture %b)\n  ret void\n}\n",
    );
    let body = &text[text.find("define void @f2").expect("@f2")..];
    assert!(!body.contains("%b") && !body.contains("@llvm.memcpy"), "{body}");
    assert_eq!(body.matches("call void @read(ptr nocapture %a)").count(), 2, "{body}");
}

/// The same through a loaded and stored scalar (`@scalar`).
#[test]
fn test_a_loaded_and_stored_scalar_between_stack_slots_makes_them_one() {
    let text = moved(
        "define void @f2() {\nb0:\n  %a = alloca i32\n  %b = alloca i32\n  store i32 1, ptr %a\n  %v = load i32, ptr %a\n  store i32 %v, ptr %b\n  call void @read(ptr nocapture %b)\n  ret void\n}\n",
    );
    let body = &text[text.find("define void @f2").expect("@f2")..];
    assert!(
        !body.contains("%b") && !body.contains("%v") && body.contains("call void @read(ptr nocapture %a)"),
        "{body}"
    );
}

/// The source is written again after the copy while the destination is read:
/// one slot would show the new bytes (`@src_written_after`).
#[test]
fn test_two_stack_slots_are_kept_when_the_source_is_written_after_the_copy() {
    let text = moved(
        "define void @f2() {\nb0:\n  %a = alloca [16 x i8]\n  %b = alloca [16 x i8]\n  call void @fill(ptr nocapture %a)\n  call void @llvm.memcpy.p0.p0.i64(ptr %b, ptr %a, i64 16, i1 false)\n  call void @fill(ptr nocapture %a)\n  call void @read(ptr nocapture %b)\n  ret void\n}\n",
    );
    let body = &text[text.find("define void @f2").expect("@f2")..];
    assert!(body.contains("%b") && body.contains("@llvm.memcpy"), "{body}");
}

const ARGUMENT_DECLARES: &str = "declare void @take(ptr byval([16 x i8]) align 4)
declare void @peek(ptr noalias nocapture readonly) memory(read)
declare void @poke(ptr)
";

/// A module of `ARGUMENT_DECLARES` and `body`, after the pass.
fn argument(body: &str) -> String {
    let mut module = parsed(&format!("{DECLARES}{ARGUMENT_DECLARES}\n{body}"));
    managed(&mut module, MemCpyOpt)
}

fn copy_to_tmp(
    between: &str,
    call: &str,
) -> String {
    format!(
        "define void @f2(ptr align 4 %src) {{\nb0:\n  %tmp = alloca [16 x i8], align 4\n  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %tmp, ptr align 4 %src, i64 16, i1 false)\n{between}  {call}\n  ret void\n}}\n"
    )
}

/// A `byval` argument fed by a memcpy copies from the memcpy's source instead
/// (`opt -passes=memcpyopt` `@byval`).
#[test]
fn test_a_byval_argument_takes_the_source_of_the_memcpy_that_fed_it() {
    let text = argument(&copy_to_tmp("", "call void @take(ptr byval([16 x i8]) align 4 %tmp)"));
    assert!(text.contains("@take(ptr byval([16 x i8]) align 4 %src)"), "{text}");
}

/// The source is written between the copy and the call (`@byval_written`).
#[test]
fn test_a_byval_argument_keeps_the_copy_when_the_source_is_written_after_it() {
    let text =
        argument(&copy_to_tmp("  call void @poke(ptr %src)\n", "call void @take(ptr byval([16 x i8]) align 4 %tmp)"));
    assert!(text.contains("@take(ptr byval([16 x i8]) align 4 %tmp)"), "{text}");
}

/// A pointer the callee only reads, to a whole alloca a memcpy filled, is the
/// memcpy's source (`@immut`).
#[test]
fn test_an_immutable_argument_takes_the_source_of_the_memcpy_that_filled_it() {
    let text = argument(&copy_to_tmp("", "call void @peek(ptr noalias nocapture readonly %tmp)"));
    assert!(text.contains("@peek(ptr noalias nocapture readonly %src)"), "{text}");
}

/// The source is written between the copy and the call (`@immut_written`).
#[test]
fn test_an_immutable_argument_keeps_the_copy_when_the_source_is_written_after_it() {
    let text =
        argument(&copy_to_tmp("  call void @poke(ptr %src)\n", "call void @peek(ptr noalias nocapture readonly %tmp)"));
    assert!(text.contains("@peek(ptr noalias nocapture readonly %tmp)"), "{text}");
}

/// A callee that writes `segment:0` of a local's selector has not written the
/// local. Call slot forwarding compares the call's argument with the copy's
/// source after `strip_pointer_casts`, which looks through address space
/// casts; it must not make the call write the copy's destination through a
/// selector. Where the layout states the space non-integral (`ni:2`, as the
/// 16-bit targets' does) a cast back from it is not the pointer it came from,
/// and the argument is not the source (`stripPointerCasts` looks through the
/// cast only where BasicAA does); without `ni:` the cast is a no-op, as in the
/// pin, and the copy is forwarded. A direct argument is forwarded, the premise.
/// The call clobbered the slot in the walker's eyes only since the AA stack
/// answers for it: the copy went with the cast read through.
#[test]
fn test_a_call_through_the_selector_of_a_local_is_not_forwarded_into_its_copy() {
    let after = |datalayout: &str, argument: &str| {
        let mut module = parsed(&format!(
            "target datalayout = \"{datalayout}\"

declare void @init(ptr nocapture) nounwind willreturn memory(argmem: write)
declare void @use(ptr)

define void @f() {{
b0:
  %dst = alloca i32
  %slot = alloca i32
  %sg = addrspacecast ptr %slot to ptr addrspace(2)
  %back = addrspacecast ptr addrspace(2) %sg to ptr
  call void @init(ptr nocapture {argument})
  %v = load i32, ptr %slot
  store i32 %v, ptr %dst
  call void @use(ptr %dst)
  ret void
}}
"
        ));
        managed(&mut module, MemCpyOpt)
    };
    let base = "e-p:16:16-p1:32:16:16:16-p2:16:16";
    assert!(
        !after(base, "%slot").contains("load i32, ptr %slot"),
        "premise: a direct argument is forwarded into the copy"
    );
    assert!(!after(base, "%back").contains("load i32, ptr %slot"), "a no-op cast is read through ({base})");
    let non_integral = format!("{base}-ni:2");
    assert!(
        after(&non_integral, "%back").contains("load i32, ptr %slot"),
        "the copy of the local went ({non_integral})"
    );
}

/// MemCpyOpt asked the manager for `Pointers`, the escape phase of the whole
/// body's points-to (2.7 M of d_faces -O2's compile), to give its alias
/// questions a provenance; its questions are `MemoryLocation(Ptr, Size)`, and
/// the stack answers them. `PointerValues` stays: MemorySSA's accesses read it.
#[test]
fn memcpyopt_asks_no_points_to() {
    struct Watching(std::rc::Rc<std::cell::Cell<bool>>);

    impl llrm_mir::passes::FunctionPass for Watching {
        fn name(&self) -> &'static str {
            "memcpyopt"
        }

        fn run(
            &mut self,
            unit: &mut llrm_mir::passes::Unit,
            analyses: &mut llrm_mir::passes::Analyses,
        ) -> llrm_mir::passes::PreservedAnalyses {
            let kept = MemCpyOpt.run(unit, analyses);
            self.0.set(analyses.cached::<llrm_analysis::manager::Pointers>().is_some());
            kept
        }
    }

    let asked = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut module =
        parsed(&format!("{DECLARES}\n{GLOBALS}\n{}", body(&[memcpy("@b", "@a", 16), memcpy("@c", "@b", 16)])));
    managed(&mut module, Watching(std::rc::Rc::clone(&asked)));
    assert!(!asked.get(), "memcpyopt solved the whole body's points-to");
}
