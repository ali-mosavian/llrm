//! `combine_loadstore` against `visitStoreInst` at the pin: what makes a store
//! vanish, and what keeps it.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitStoreInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Store { .. } => c.visit_store(inst),
        Opcode::Alloca { .. } => c.visit_alloca(inst),
        _ => Visit::None,
    }
}

/// `@f`'s body (between the braces' first block label and the end) after
/// `visitStoreInst` and `visitAllocaInst` alone, through the driver.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `combined` with `visitLoadInst` too.
fn combined_with_loads(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, |c, inst| {
        match c.function.instruction(inst).opcode {
            Opcode::Load { .. } => c.visit_load(inst),
            _ => Visit::None,
        }
    });
    printed(&module)
}

fn stores(text: &str) -> usize {
    combined(text).matches("store ").count()
}

/// A store to an alloca nothing else reads was kept: the alloca stayed live.
#[test]
fn a_store_to_a_one_use_alloca_is_erased() {
    let out = combined(
        "define void @f(i16 %x) {
b0:
  %a = alloca i16
  store i16 %x, ptr %a
  ret void
}
",
    );
    assert!(!out.contains("store"), "{out}");
}

/// A store through a GEP of a one-use alloca was kept.
#[test]
fn a_store_through_a_gep_of_a_one_use_alloca_is_erased() {
    let out = combined(
        "define void @f(i16 %x) {
b0:
  %a = alloca [2 x i16]
  %g = getelementptr [2 x i16], ptr %a, i16 0, i16 1
  store i16 %x, ptr %g
  ret void
}
",
    );
    assert!(!out.contains("store"), "{out}");
}

/// An alloca that is read stays written.
#[test]
fn a_store_to_a_read_alloca_stays() {
    let n = stores(
        "define i16 @f(i16 %x) {
b0:
  %a = alloca i16
  store i16 %x, ptr %a
  %v = load i16, ptr %a
  ret i16 %v
}
",
    );
    assert_eq!(n, 1);
}

/// The store overwritten a few instructions later was kept: two stores stood
/// where one does.
#[test]
fn an_overwritten_store_is_erased() {
    let out = combined(
        "define void @f(ptr %p, i16 %a) {
b0:
  store i16 1, ptr %p
  %x = add i16 %a, 1
  store i16 %x, ptr %p
  ret void
}
",
    );
    assert_eq!(out.matches("store ").count(), 1, "{out}");
    assert!(!out.contains("store i16 1,"), "{out}");
}

/// A load between the two stores may read the first.
#[test]
fn a_load_between_two_stores_keeps_both() {
    let n = stores(
        "define i16 @f(ptr %p, ptr %q) {
b0:
  store i16 1, ptr %p
  %v = load i16, ptr %q
  store i16 2, ptr %p
  ret i16 %v
}
",
    );
    assert_eq!(n, 2);
}

/// The scan looks at 6 instructions: the store 7 back is kept.
#[test]
fn the_scan_stops_after_six_instructions() {
    // A chain, so the driver does not erase the adds as dead.
    let text = |n: usize| {
        let chain: String = (0..n)
            .map(|i| {
                format!("  %t{i} = add i16 {}, 1\n", if i == 0 { "%a".to_owned() } else { format!("%t{}", i - 1) })
            })
            .collect();
        format!(
            "define i16 @f(ptr %p, i16 %a) {{\nb0:\n  store i16 1, ptr %p\n{chain}  store i16 2, ptr %p\n  ret i16 %t{}\n}}\n",
            n - 1
        )
    };
    assert_eq!(stores(&text(5)), 1);
    assert_eq!(stores(&text(6)), 2);
}

/// `%x = load %p; store %x, %p` writes what is there.
#[test]
fn a_store_of_the_value_just_loaded_is_erased() {
    let out = combined(
        "define i16 @f(ptr %p) {
b0:
  %x = load i16, ptr %p
  store i16 %x, ptr %p
  ret i16 %x
}
",
    );
    assert!(!out.contains("store"), "{out}");
}

/// A store of `undef` stored it: a write the next read cannot tell from none.
#[test]
fn a_store_of_undef_is_erased() {
    for value in ["undef", "poison"] {
        let out = combined(&format!("define void @f(ptr %p) {{\nb0:\n  store i16 {value}, ptr %p\n  ret void\n}}\n"));
        assert!(!out.contains("store"), "{value}: {out}");
    }
}

/// A store to null writes poison: SimplifyCFG's `unreachable` marker.
#[test]
fn a_store_to_null_stores_poison() {
    for (setup, address) in [("", "null"), ("  %g = getelementptr i16, ptr null, i16 %y\n", "%g")] {
        let out = combined(&format!(
            "define void @f(i16 %x, i16 %y) {{\nb0:\n{setup}  store i16 %x, ptr {address}\n  ret void\n}}\n"
        ));
        assert!(out.contains("store i16 poison"), "{address}: {out}");
    }
}

/// A volatile store is left alone.
#[test]
fn a_volatile_store_stays() {
    let n = stores(
        "define void @f(i16 %x) {
b0:
  %a = alloca i16
  store volatile i16 %x, ptr %a
  ret void
}
",
    );
    assert_eq!(n, 1);
}

/// An alloca with a count of 4 stayed a count: it is `alloca [4 x i16]`.
#[test]
fn a_constant_count_is_an_array_type() {
    let out = combined("define ptr @f() {\nb0:\n  %a = alloca i16, i32 4\n  ret ptr %a\n}\n");
    assert!(out.contains("alloca [4 x i16]") && !out.contains("i32 4"), "{out}");
}

/// A count of 1 is no count.
#[test]
fn a_count_of_one_is_dropped() {
    let out = combined("define ptr @f() {\nb0:\n  %a = alloca i16, i32 1\n  ret ptr %a\n}\n");
    assert!(out.contains("alloca i16\n"), "{out}");
}

/// A count of `undef` makes the pointer poison.
#[test]
fn an_undef_count_is_a_poison_pointer() {
    let out = combined("define ptr @f() {\nb0:\n  %a = alloca i16, i32 undef\n  ret ptr %a\n}\n");
    assert!(out.contains("ret ptr poison"), "{out}");
}

/// A count not as wide as an index is widened where it is made, so the cast
/// is seen early.
#[test]
fn a_narrow_count_is_widened_to_an_index() {
    let out = combined("define ptr @f(i8 %n) {\nb0:\n  %a = alloca i16, i8 %n\n  ret ptr %a\n}\n");
    assert!(out.contains("zext i8 %n to i64") && out.contains("alloca i16, i64"), "{out}");
}

/// A zero-size alloca in a later block goes to the top of the entry block.
#[test]
fn a_zero_size_alloca_moves_to_the_entry_block() {
    let out = combined(
        "define ptr @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %z = alloca [0 x i16]
  ret ptr %z

b2:
  ret ptr null
}
",
    );
    let (entry, rest) = out.split_once("b1:").expect("b1");
    assert!(entry.contains("alloca [0 x i16]") && !rest.contains("alloca"), "{out}");
}

/// Two zero-size allocas are one, aligned for both.
#[test]
fn two_zero_size_allocas_merge_with_the_larger_alignment() {
    let out = combined(
        "define i1 @f() {
b0:
  %a = alloca [0 x i16], align 2
  %b = alloca [0 x i32], align 8
  %e = icmp eq ptr %a, %b
  ret i1 %e
}
",
    );
    assert_eq!(out.matches("alloca").count(), 1, "{out}");
    assert!(out.contains("align 8"), "{out}");
}

/// An alloca that two stores write and nothing reads was kept, with both
/// stores: GORILLA's DRAWGORILLA wrote a statement's argument box twice (the
/// box's second field never read) and the dead stores were 232 of its movs
/// against main's 140. `visitStoreInst` erases a store to an alloca with one
/// use only; `visitAllocSite` erases the alloca and every store.
#[test]
fn an_alloca_only_written_goes_with_all_its_stores() {
    let out = combined(
        "define void @f(i16 %x) {
b0:
  %a = alloca i16
  %b = alloca [2 x i16]
  store i16 0, ptr %a
  %g = getelementptr [2 x i16], ptr %b, i16 0, i16 1
  store i16 %x, ptr %g
  store i16 %x, ptr %a
  store i16 1, ptr %b
  ret void
}
",
    );
    assert!(!out.contains("store") && !out.contains("alloca") && !out.contains("getelementptr"), "{out}");
}

/// A load, a call that is handed the pointer or a store of the pointer keeps
/// the alloca and its stores.
#[test]
fn an_alloca_that_is_read_or_escapes_keeps_its_stores() {
    for user in ["%v = load i16, ptr %a\n  store i16 %v, ptr @k", "call void @g(ptr %a)", "store ptr %a, ptr @h"] {
        let out = combined(&format!(
            "@h = global ptr null
@k = global i16 0
declare void @g(ptr)
define void @f(i16 %x) {{
b0:
  %a = alloca i16
  store i16 0, ptr %a
  store i16 %x, ptr %a
  {user}
  ret void
}}
"
        ));
        assert!(out.contains("alloca i16") && out.contains("store i16 %x, ptr %a"), "{user}:\n{out}");
    }
}

/// `visitAllocaInst` was translated but never reached: `visitors` had no entry
/// for an alloca, so the combine driver left a box written twice and never
/// read. GORILLA m32 -Os 49,860 bytes against main's 47,769; with the entry
/// 47,025.
#[test]
fn the_combine_driver_visits_an_alloca() {
    let mut module = parsed(
        "define void @f(i16 %x) {
b0:
  %a = alloca [2 x i16]
  %g0 = getelementptr [2 x i16], ptr %a, i16 0, i16 0
  %g1 = getelementptr [2 x i16], ptr %a, i16 0, i16 1
  store i16 0, ptr %g0
  store i16 %x, ptr %g1
  ret void
}
",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, crate::combine::visit);
    let out = printed(&module);
    assert!(!out.contains("alloca") && !out.contains("store"), "{out}");
}

/// QCport `_host_changelevel@3` (with the C front end's `memcpy`, #3621): a
/// local written by a `memcpy` and by lifetime markers, never read, stayed with
/// its 64 to 258 bytes of `rep movsd` until the end. `isAllocSiteRemovable`
/// takes the alloca and every user of it away.
#[test]
fn an_alloca_written_by_a_memcpy_and_marked_by_lifetimes_is_erased() {
    let after = combined(
        "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)

define i16 @f(ptr %src) {
b0:
  %s = alloca [64 x i8]
  call void @llvm.lifetime.start.p0(i64 64, ptr %s)
  call void @llvm.memcpy.p0.p0.i16(ptr %s, ptr %src, i16 64, i1 false)
  call void @llvm.lifetime.end.p0(i64 64, ptr %s)
  ret i16 0
}
",
    );
    assert!(!after.contains("alloca") && !after.contains("call void"), "{after}");
}

/// A `memcpy` into the alloca followed by a load of it is a use: it stays.
#[test]
fn an_alloca_a_memcpy_wrote_and_a_load_reads_is_kept() {
    let after = combined(
        "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)

define i16 @f(ptr %src) {
b0:
  %s = alloca [64 x i8]
  call void @llvm.memcpy.p0.p0.i16(ptr %s, ptr %src, i16 64, i1 false)
  %v = load i16, ptr %s
  ret i16 %v
}
",
    );
    assert!(
        after.contains("alloca") && after.contains("call void @llvm.memcpy") && after.contains("load i16"),
        "{after}"
    );
}

/// A volatile `memcpy` is not removable.
#[test]
fn a_volatile_memcpy_keeps_its_alloca() {
    let after = combined(
        "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)

define i16 @f(ptr %src) {
b0:
  %s = alloca [64 x i8]
  call void @llvm.memcpy.p0.p0.i16(ptr %s, ptr %src, i16 64, i1 true)
  ret i16 0
}
",
    );
    assert!(after.contains("alloca") && after.contains("call void @llvm.memcpy"), "{after}");
}

/// LICM leaves `%x = load @I` right behind `store 1, @I` and the stores of
/// other globals; the loop's trip count stayed unknown (split.bas -Os m16).
#[test]
fn a_load_behind_a_store_of_its_global_and_stores_of_others_reads_the_stored_value() {
    let out = combined_with_loads(
        "@a = global i16 0
@b = global i16 0
define i16 @f() {
b0:
  store i16 1, ptr @a
  store i16 2, ptr @b
  %x = load i16, ptr @a
  ret i16 %x
}
",
    );
    assert!(!out.contains("load"), "{out}");
    assert!(out.contains("ret i16 1"), "{out}");
}

/// A store through a pointer that may alias the global ends the forwarding.
#[test]
fn a_store_through_an_unknown_pointer_between_keeps_the_load() {
    let out = combined_with_loads(
        "@a = global i16 0
define i16 @f(ptr %p) {
b0:
  store i16 1, ptr @a
  store i16 2, ptr %p
  %x = load i16, ptr @a
  ret i16 %x
}
",
    );
    assert!(out.contains("load"), "{out}");
}

#[test]
fn a_volatile_load_is_kept() {
    let out = combined_with_loads(
        "@a = global i16 0
define i16 @f() {
b0:
  store i16 1, ptr @a
  %x = load volatile i16, ptr @a
  ret i16 %x
}
",
    );
    assert!(out.contains("load volatile"), "{out}");
}

/// `int a[] = {1, 2, 3}; return a[i]` copied the constant into the frame and
/// read the copy, so no loop over it folded (`numeric`'s CRC: 504 bytes against
/// 420): an alloca only read after one `memcpy` from a constant global is that
/// global.
#[test]
fn an_alloca_only_copied_from_a_constant_global_is_the_global() {
    let text = combined(
        "@g = private unnamed_addr constant [3 x i32] [i32 1, i32 2, i32 3], align 4
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
define i32 @f(i64 %i) {
b0:
  %a = alloca [3 x i32], align 4
  call void @llvm.memcpy.p0.p0.i64(ptr %a, ptr @g, i64 12, i1 0)
  %p = getelementptr inbounds i32, ptr %a, i64 %i
  %v = load i32, ptr %p, align 4
  ret i32 %v
}
",
    );
    assert!(!text.contains("alloca") && !text.contains("memcpy.p0.p0.i64(ptr %a"), "{text}");
    assert!(text.contains("getelementptr inbounds i32, ptr @g, i64 %i"), "{text}");
}

/// A second write to the alloca, or the alloca passed on as a stored value,
/// keeps the copy.
#[test]
fn an_alloca_written_besides_the_copy_stays() {
    let text = combined(
        "@g = private unnamed_addr constant [3 x i32] [i32 1, i32 2, i32 3], align 4
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
define i32 @f(i64 %i) {
b0:
  %a = alloca [3 x i32], align 4
  call void @llvm.memcpy.p0.p0.i64(ptr %a, ptr @g, i64 12, i1 0)
  store i32 9, ptr %a, align 4
  %p = getelementptr inbounds i32, ptr %a, i64 %i
  %v = load i32, ptr %p, align 4
  ret i32 %v
}
",
    );
    assert!(text.contains("alloca"), "{text}");
}
