//! `Dse` under the pass manager, each body MIR text run by llrm-mir's
//! interpreter before and after.

use llrm_analysis::manager::Summaries;
use llrm_mir::passes::PassManager;

use super::Dse;
use crate::promote::Promote;
use crate::testing::{parsed, printed, results};

const INPUTS: &[&[i128]] = &[&[0], &[3], &[-7], &[0x7fff]];

/// `text` through Promote then Dse: @f printed; what @f returns stays.
fn promoted(text: &str) -> String {
    let before = parsed(&format!(
        "@g = global i16 0\n\ndefine void @h(ptr %s) {{\nb0:\n  store i16 9, ptr %s\n  ret void\n}}\n\n{text}"
    ));
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.verify_invalidation = true;
    manager.require::<Summaries>();
    manager.add(Promote);
    manager.add(Dse);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{after}");
    after[after.find("@f(").unwrap()..].to_owned()
}

/// `text` through Dse alone: @f printed; what @f returns stays.
fn dropped(text: &str) -> String {
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(Dse);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{after}");
    after[after.find("@f(").unwrap()..].to_owned()
}

/// Promote leaves the stores to a cell whose loads it replaced; nothing
/// reads the cell after, so they go.
#[test]
fn test_dse_removes_the_stores_promote_left() {
    let after = promoted(
        "define i16 @f(i16 %x) {
b0:
  %s = alloca i16
  store i16 %x, ptr %s
  %c = icmp slt i16 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  store i16 0, ptr %s
  br label %b2

b2:
  %v = load i16, ptr %s
  ret i16 %v
}
",
    );
    assert!(!after.contains("store"), "{after}");
}

/// c/floats: a volatile access touches only its own bytes, so the loop
/// counter beside a volatile cell is promoted and its stores go. It was a
/// barrier: the counter was stored and reloaded every trip.
#[test]
fn a_volatile_access_leaves_another_cell_promoted() {
    let after = promoted(
        "define i16 @f(i16 %n) {
b0:
  %v = alloca i16
  %i = alloca i16
  store volatile i16 1, ptr %v
  store i16 0, ptr %i
  %m = and i16 %n, 15
  br label %b1

b1:
  %k = load i16, ptr %i
  %c = icmp slt i16 %k, %m
  br i1 %c, label %b2, label %b3

b2:
  %x = load volatile i16, ptr %v
  %y = add i16 %x, %k
  store volatile i16 %y, ptr %v
  %j = load i16, ptr %i
  %z = add i16 %j, 1
  store i16 %z, ptr %i
  br label %b1

b3:
  %r = load volatile i16, ptr %v
  ret i16 %r
}
",
    );
    assert!(
        !after.contains("load i16") && !after.contains("store i16") && after.matches("volatile").count() == 4,
        "{after}"
    );
}

/// A call's `initializes` bytes are written before it reads them, so the
/// zero fill before it goes; not where it reads them through another
/// argument. B$ASSN's fixed-length destination was zeroed first.
#[test]
fn a_call_filling_a_buffer_kills_its_earlier_fill() {
    for (source, kept) in [("%s", 0), ("%a", 1)] {
        let after = promoted(&format!(
            "define void @fill(ptr %d, ptr %s) {{
b:
  %v = load i16, ptr %s
  store i16 %v, ptr %d
  %e = getelementptr i8, ptr %d, i16 2
  store i16 %v, ptr %e
  ret void
}}

define i16 @f(i16 %x) {{
b0:
  %a = alloca [4 x i8]
  %s = alloca i16
  store i16 %x, ptr %s
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 4, i1 false)
  call void @fill(ptr nocapture writeonly initializes((0, 4)) %a, ptr {source})
  %v = load i16, ptr %a
  ret i16 %v
}}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)
"
        ));
        assert_eq!(after.matches("call void @llvm.memset").count(), kept, "{source}: {after}");
    }
}

/// A store to a cell a callee reads, or that outlives the function, stays;
/// one overwritten before anything reads it goes.
#[test]
fn test_dse_keeps_an_observed_store() {
    for (body, kept) in [
        ("  %s = alloca i16\n  store i16 %x, ptr %s\n  call void @h(ptr %s)\n  %v = load i16, ptr %s\n", 1),
        ("  store i16 %x, ptr @g\n  %v = load i16, ptr @g\n", 1),
        ("  store i16 1, ptr @g\n  store i16 %x, ptr @g\n  %v = load i16, ptr @g\n", 1),
    ] {
        let after = promoted(&format!("define i16 @f(i16 %x) {{\nb0:\n{body}  ret i16 %v\n}}\n"));
        assert_eq!(after.matches("store").count(), kept, "{after}");
    }
}

/// Without `Summaries` required the pass runs, as an LLVM function pass
/// does without a cached outer result: every call unknown, so less
/// precise. It panicked.
#[test]
fn a_bare_pass_manager_takes_every_call_for_unknown() {
    let module = crate::testing::parsed(&format!(
        "{}{}{}",
        llrm_analysis::testing::DOS,
        crate::testing::WRITES_ITS_ARGUMENT,
        "define i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  call void @h(ptr @k)\n  store i16 %x, ptr @g\n  %v = load i16, ptr @g\n  ret i16 %v\n}\n"
    ));
    let precise = crate::testing::summarized(&module, Dse, true, &[&[0], &[5]]);
    let bare = crate::testing::summarized(&module, Dse, false, &[&[0], &[5]]);
    assert!(precise.matches("store").count() == 1 && bare.matches("store").count() == 2, "{precise}\n{bare}");
}

/// NBODY's frame slots, zeroed by `llvm.memset` and then only written:
/// every write goes, and Dead takes the slot. Only a store named a cell, so
/// the memset, and with it the slot, stayed.
#[test]
fn test_a_zeroed_slot_nothing_reads_goes() {
    let text = "define i16 @f(i16 %x) {
b0:
  %s = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %s, i8 0, i16 8, i1 false)
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %w = sext i16 %i to i32
  store i32 %w, ptr %s
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, %x
  br i1 %go, label %b1, label %b2

b2:
  ret i16 %x
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)
";
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(Dse);
    manager.add(crate::dead::Dead);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, &[&[0], &[3], &[-7]]), results(&before, &[&[0], &[3], &[-7]]), "{after}");
    assert!(!after.contains("alloca") && !after.contains("store") && !after.contains("call void"), "{after}");
}

/// A zeroed slot whose every byte some later store writes, no one store
/// all of them: the memset goes, as LLVM's DSE merges overwritten
/// intervals. HIR's zeroed frames kept one memset per initialized local.
#[test]
fn test_a_memset_the_stores_together_overwrite_goes() {
    for (last, gone) in [(6, true), (4, false)] {
        let text = format!(
            "define i16 @f(i16 %x) {{
b0:
  %s = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %s, i8 0, i16 8, i1 false)
  store i16 %x, ptr %s
  %s2 = getelementptr inbounds i8, ptr %s, i16 2
  store i16 1, ptr %s2
  %s4 = getelementptr inbounds i8, ptr %s, i16 4
  store i16 2, ptr %s4
  %sl = getelementptr inbounds i8, ptr %s, i16 {last}
  store i16 3, ptr %sl
  %s6 = getelementptr inbounds i8, ptr %s, i16 6
  %v = load i16, ptr %s6
  %w = load i16, ptr %s
  %r = add i16 %v, %w
  ret i16 %r
}}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)
"
        );
        let before = parsed(&text);
        let mut module = before.clone();
        let mut manager = PassManager::default();
        manager.verify_each = true;
        manager.require::<Summaries>();
        manager.add(Dse);
        manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
        let after = printed(&module);
        assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{after}");
        assert_eq!(!after.contains("call void"), gone, "{after}");
    }
}

/// A slot read back through its far address kept in another slot, at an
/// unresolved offset: the load may reach an unknown object too, but it
/// names this one, so its stores stay. They went (priced_fill), and the sum
/// read poison.
#[test]
fn test_a_store_read_through_a_pointer_kept_in_memory_stays() {
    let text = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16\"

define i16 @f() {
b0:
  %zeroed = alloca [256 x i8]
  %holder = alloca [8 x i8]
  %a = alloca [8 x i8]
  store i16 1, ptr %a
  %a1 = getelementptr inbounds i16, ptr %a, i16 1
  store i16 2, ptr %a1
  %far = addrspacecast ptr %a to ptr addrspace(1)
  %field = getelementptr inbounds i8, ptr %holder, i16 4
  store ptr addrspace(1) %far, ptr %field
  %holderfar = addrspacecast ptr %holder to ptr addrspace(1)
  br label %zero

zero:
  %j = phi i16 [ 0, %b0 ], [ %jn, %zeroing ]
  %more = icmp slt i16 %j, 64
  br i1 %more, label %zeroing, label %read

zeroing:
  %slot = getelementptr inbounds i32, ptr %zeroed, i16 %j
  store i32 0, ptr %slot
  %jn = add i16 %j, 1
  br label %zero

read:
  %at = getelementptr i8, ptr addrspace(1) %holderfar, i16 4
  %p = load ptr addrspace(1), ptr addrspace(1) %at
  br label %sum

sum:
  %s = phi i16 [ 0, %read ], [ %sn, %add ]
  %i = phi i16 [ 0, %read ], [ %in, %add ]
  %go = icmp slt i16 %i, 2
  br i1 %go, label %add, label %done

add:
  %o = shl i16 %i, 1
  %q = getelementptr i8, ptr addrspace(1) %p, i16 %o
  %v = load i16, ptr addrspace(1) %q
  %sn = add i16 %s, %v
  %in = add i16 %i, 1
  br label %sum

done:
  ret i16 %s
}
";
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(Dse);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, &[&[]]), results(&before, &[&[]]), "{after}");
    assert_eq!(after.matches("store i16").count(), 2, "{after}");
}

/// A zero-byte global and alloca, as an empty segment or frame raises:
/// each overlaps nothing, as in LLVM. Sroa and Dse panicked on them,
/// "an alias slice must contain at least one byte".
#[test]
fn a_zero_byte_object_overlaps_nothing() {
    let mut module = parsed(
        "@z = global [0 x i8] zeroinitializer
@g = global i16 0

declare void @u(ptr)

define i16 @f(i16 %x) {
b0:
  %e = alloca [0 x i8]
  %s = alloca i16
  store i16 %x, ptr %s
  call void @u(ptr %e)
  call void @u(ptr @z)
  %q = getelementptr i8, ptr %e, i16 %x
  %c = icmp eq ptr %q, @z
  store i16 1, ptr @g
  store i16 %x, ptr @g
  %v = load i16, ptr %s
  ret i16 %v
}
",
    );
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(crate::promote::Sroa);
    manager.add(Promote);
    manager.add(Dse);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let after = printed(&module);
    assert!(!after.contains("store i16 1") && after.contains("ret i16 %x"), "{after}");
}

/// A bounds error's path ends in `unreachable`: nothing after it reads,
/// and its noreturn call reads only what escaped before it, as LLVM's DSE
/// ignores such exits. The zeroed slot, overwritten on the other path and
/// only then handed out, loses its memset. Each bounds check kept T028's.
#[test]
fn test_a_path_to_unreachable_reads_only_what_escaped_before() {
    let text = "declare void @use(ptr)

declare void @error() noreturn

define i16 @f(i16 %x) {
b0:
  %s = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %s, i8 0, i16 4, i1 false)
  store i16 %x, ptr %s
  %bad = icmp sgt i16 %x, 100
  br i1 %bad, label %b2, label %b1

b1:
  %s2 = getelementptr inbounds i8, ptr %s, i16 2
  store i16 1, ptr %s2
  call void @use(ptr %s)
  ret i16 %x

b2:
  call void @error()
  unreachable
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)
";
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(Dse);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let after = printed(&module);
    assert!(!after.contains("call void @llvm.memset") && after.matches("store i16").count() == 2, "{after}");
}

/// A load through a pointer read from memory, before the slot's address
/// escapes, cannot read the slot: LLVM's `EarliestEscapeInfo`. The memset
/// the later stores overwrite goes. T028's `update` read `v[i]` through
/// its slice's data pointer between the zeroing and the stores.
#[test]
fn test_a_load_before_the_escape_does_not_read_the_slot() {
    let text = "declare void @use(ptr)

define void @f(ptr %p) {
b0:
  %s = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %s, i8 0, i16 4, i1 false)
  %q = load ptr, ptr %p
  %v = load i16, ptr %q
  store i16 %v, ptr %s
  %s2 = getelementptr inbounds i8, ptr %s, i16 2
  store i16 1, ptr %s2
  call void @use(ptr %s)
  ret void
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)
";
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(Dse);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let after = printed(&module);
    assert!(!after.contains("call void @llvm.memset") && after.matches("store i16").count() == 2, "{after}");
}

/// A local's lifetime ending is the end of what its bytes hold: the store
/// before it, which promotion left once it replaced the load, is dead. The
/// markers read the cell (`memory(argmem: readwrite)`), so it stayed.
#[test]
fn test_a_store_before_the_lifetime_end_is_dead() {
    let after = promoted(
        "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
define i16 @f(i16 %x) {
b0:
  %s = alloca i16
  call void @llvm.lifetime.start.p0(i64 2, ptr %s)
  store i16 %x, ptr %s
  %v = load i16, ptr %s
  call void @llvm.lifetime.end.p0(i64 2, ptr %s)
  ret i16 %v
}
",
    );
    assert!(!after.contains("store "), "{after}");
}

/// A store whose value is also read back whole is still read where a narrower
/// load at an offset reaches it: `store i32` into a cell, the high word read
/// through `getelementptr` and the whole word read as well. Dropped as dead,
/// the narrower load read bytes nothing had written (`((int *)&b)[1]` of an i64
/// parameter, #677).
#[test]
fn test_dse_keeps_a_store_a_narrower_load_at_an_offset_reads() {
    let after = promoted(
        "define i32 @f(i32 %x) {
b0:
  %s = alloca i32
  store i32 %x, ptr %s
  %p = getelementptr inbounds i8, ptr %s, i32 2
  %h = load i16, ptr %p
  %w = load i32, ptr %s
  %e = zext i16 %h to i32
  %r = add i32 %w, %e
  ret i32 %r
}
",
    );
    assert!(after.contains("store i32"), "{after}");
}

/// The same with an i64 and the types C gives them: a `long long` stored and an
/// `int` read at offset 4 of it. Type-based alias analysis may rule two
/// accesses out where the address analysis cannot tell, not where it has them
/// in one object at overlapping bytes (LLVM asks it only of MayAlias): the
/// store went as dead and the read took the cell's bytes unwritten (`((int
/// *)&b)[1]`, #677).
#[test]
fn test_dse_keeps_an_i64_store_its_high_dword_load_reads() {
    let after = dropped(
        "define i32 @f(i32 %x) {
b0:
  %s = alloca i64
  %v = zext i32 %x to i64
  store i64 %v, ptr %s, !tbaa !13
  %p = getelementptr inbounds i8, ptr %s, i32 4
  %h = load i32, ptr %p, !tbaa !11
  %t = trunc i64 %v to i32
  %r = add i32 %t, %h
  ret i32 %r
}

!5 = !{!\"Simple C/C++ TBAA\"}
!6 = !{!\"omnipotent char\", !5, i64 0}
!10 = !{!\"int4\", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!\"int8\", !6, i64 0}
!13 = !{!12, !12, i64 0}
",
    );
    assert!(after.contains("store i64"), "{after}");
}

/// Every cell a function stores to was compared with every other, two ways
/// over: quadratic in the cells, 590 Minstr of GORILLA's `mir dse` at -O1.
/// A cell is compared with the cells of the buckets it reaches: a slot nothing
/// takes the address of is in its own.
#[test]
fn a_cell_is_compared_with_the_cells_that_may_share_its_bytes_not_with_every_cell() {
    let cells = 40;
    let slots: String = (0..cells).map(|i| format!("  %s{i} = alloca i16\n")).collect();
    let stores: String = (0..cells).map(|i| format!("  store i16 {i}, ptr %s{i}\n")).collect();
    let before = llrm_analysis::avail::related_pairs();
    let after = dropped(&format!("define i16 @f() {{\nb0:\n{slots}{stores}{stores}  ret i16 0\n}}\n"));
    let asked = llrm_analysis::avail::related_pairs() - before;
    assert_eq!(after.matches("store").count(), 0, "every slot is dead: {after}");
    assert!(asked <= 4 * cells, "{asked} pairs asked for {cells} cells");
}
