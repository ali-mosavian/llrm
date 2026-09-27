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
    let before = parsed(&format!("@g = global i16 0\n\ndefine void @h(ptr %s) {{\nb0:\n  store i16 9, ptr %s\n  ret void\n}}\n\n{text}"));
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
    let module = crate::testing::parsed(&format!("{}{}{}", llrm_analysis::testing::DOS, crate::testing::WRITES_ITS_ARGUMENT, "define i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  call void @h(ptr @k)\n  store i16 %x, ptr @g\n  %v = load i16, ptr @g\n  ret i16 %v\n}\n"));
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
