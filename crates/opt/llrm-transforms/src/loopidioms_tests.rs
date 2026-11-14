//! `loopidioms` had no tests of its own in llrm-codegen; these are behaviour
//! tests, each body MIR text run by llrm-mir's interpreter before and after.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::{Declared, Outer};

use super::{LoopIdioms, Merge, filled};
use crate::testing::{Tuned, managed, managed_on, parsed, printed, results};

const MEMSET: &str = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\n";

/// `text`'s @f filled: its printed form, and whether it changed. @f
/// computes what it did on `inputs`.
fn fill(
    text: &str,
    inputs: &[&[i128]],
) -> (String, bool) {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let callees = llrm_mir::memory::callees(&module);
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = filled(context, &layout, &callees, function, &outer, &mut declared, false);
    let after = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (after, changed)
}

/// `text` under the pass manager, which declares what fill calls: its
/// printed form, run as before.
fn managed_fill(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    managed_fill_on(DOS, text, inputs)
}

/// `managed_fill` on the datalayout `target` names.
fn managed_fill_on(
    target: &str,
    text: &str,
    inputs: &[&[i128]],
) -> String {
    managed_fill_by(target, text, inputs, Tuned::default())
}

/// `managed_fill_on` for `machine`.
fn managed_fill_by(
    target: &str,
    text: &str,
    inputs: &[&[i128]],
    machine: Tuned,
) -> String {
    let before = parsed(&format!("{target}{text}"));
    let mut module = before.clone();
    let after = managed_on(&mut module, LoopIdioms { size: false }, machine);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

fn kept(
    text: &str,
    inputs: &[&[i128]],
) {
    let (after, changed) = fill(text, inputs);
    assert!(!changed, "{after}");
    assert_eq!(after, printed(&parsed(&format!("{DOS}{text}"))));
}

/// `i` from 0 while `i < bound`, the latch running `body` with `%p`
/// addressing `@buf`'s cells of `cell`; @f returns cell `%q` and `result`.
fn looped(
    cell: &str,
    bound: &str,
    body: &str,
    result: &str,
) -> String {
    let widened = if cell == "i16" { "or i16 %v, 0".to_owned() } else { format!("zext {cell} %v to i16") };
    format!(
        "@buf = global [64 x {cell}] zeroinitializer

{MEMSET}define i16 @f(i16 %n, i16 %q) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, {bound}
  br i1 %c, label %b2, label %b3

b2:
{body}  %next = add i16 %i, 1
  br label %b1

b3:
  %e = phi i16 [ %i, %b1 ]
  %r = getelementptr [64 x {cell}], ptr @buf, i16 0, i16 %q
  %v = load {cell}, ptr %r
  %w = {widened}
  %x = add i16 %w, {result}
  ret i16 %x
}}
"
    )
}

const TRIPS: &[&[i128]] = &[&[0, 0], &[1, 0], &[1, 1], &[40, 39], &[40, 40], &[-5, 0]];

/// Bytes a symbolic count of trips stores are one memset, guarded by the
/// header's test; the counter leaves with its exit value.
#[test]
fn a_byte_loop_is_one_memset() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  store i8 65, ptr %p\n";
    let (text, changed) = fill(&looped("i8", "%n", body, "%e"), TRIPS);
    assert!(changed);
    assert!(text.contains("b1:\n  %c = icmp slt i16 0, %n\n  br i1 %c, label %b2, label %b3\n"), "{text}");
    assert!(
        text.contains("  call void @llvm.memset.p0.i16(ptr align 1 %p, i8 65, i16 %1, i1 false)\n")
            && text.contains("  br label %b3\n\nb3:\n  %e = phi i16 [ 0, %b1 ], [ %2, %b2 ]\n"),
        "{text}"
    );
}

/// A `"no-builtins"` function has every library function off in the pin's TLI,
/// so no fill or copy loop becomes a call: x_radix's zero and copy loops were
/// 3 memsets and 3 memcpys where the pin keeps the loops.
#[test]
fn a_no_builtins_function_keeps_its_fill_loop() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  store i8 65, ptr %p\n";
    let text = looped("i8", "%n", body, "%e").replacen(") {", ") \"no-builtins\" {", 1);
    assert!(text.contains("no-builtins"), "{text}");
    kept(&text, TRIPS);
}

/// A known positive count enters without a test; a word of one repeated
/// byte fills as bytes, twice the trips.
#[test]
fn a_counted_word_loop_is_one_memset_of_its_bytes() {
    for (value, byte) in [(0, 0), (0x4141, 65)] {
        let body =
            format!("  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 {value}, ptr %p\n");
        let (text, changed) = fill(&looped("i16", "10", &body, "0"), &[&[0, 0], &[0, 9], &[0, 10]]);
        assert!(changed);
        assert!(text.contains(&format!("b1:\n  %c = icmp slt i16 0, 10\n  br label %b2\n")), "{text}");
        assert!(
            text.contains(&format!("call void @llvm.memset.p0.i16(ptr align 2 %p, i8 {byte}, i16 20, i1 false)")),
            "{text}"
        );
    }
}

/// A trip that fills four bytes, stepping four: one memset of them all.
#[test]
fn a_loop_of_memsets_is_one_memset() {
    let body = "  %j = mul i16 %i, 4\n  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %j\n  call void @llvm.memset.p0.i16(ptr %p, i8 7, i16 4, i1 false)\n";
    let (text, changed) = fill(&looped("i8", "10", body, "%e"), &[&[0, 0], &[0, 39], &[0, 40]]);
    assert!(changed && text.contains("call void @llvm.memset.p0.i16(ptr %p, i8 7, i16 40, i1 false)"), "{text}");
}

#[test]
fn a_fill_whose_stride_is_not_the_element_size_is_kept() {
    let body = "  %j = mul i16 %i, 2\n  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %j\n  store i8 65, ptr %p\n";
    kept(&looped("i8", "20", body, "%e"), &[&[0, 0], &[0, 1], &[0, 2]]);
}

#[test]
fn a_loop_with_a_volatile_store_is_kept() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  store volatile i8 65, ptr %p\n";
    kept(&looped("i8", "%n", body, "%e"), TRIPS);
}

/// `managed_fill` on a target whose library has `memset_pattern16`.
fn patterned(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    managed_fill_by(DOS, text, inputs, Tuned { memset_pattern: true, ..Tuned::default() })
}

/// A target whose library lacks `memset_pattern16` (every one but Darwin, the
/// pin's `TLI->has`) kept a word fill a store loop for the vectorizer: the
/// pass turned it into `llvm.experimental.memset.pattern`, which arm64-linux
/// expands back to a scalar `strh` loop.
#[test]
fn a_word_fill_stays_a_store_loop_without_memset_pattern16() {
    let body = "  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 4660, ptr %p\n";
    let text = looped("i16", "10", body, "0");
    let after = managed_fill(&text, &[&[0, 0], &[0, 9], &[0, 10]]);
    assert!(!after.contains("memset.pattern") && after.contains("store i16 4660"), "{after}");
}

/// m16 states no library with `memset_pattern16`, so #9570 kept every word fill
/// a store loop there: copy_idioms `palette` -O2 lost its `rep stosw` (a
/// `add si,2; jne` loop of 5 clk a word instead of 4). A target whose isel
/// fills a pattern itself forms it with no library.
#[test]
fn a_word_fill_forms_where_the_target_fills_a_pattern_itself() {
    let body = "  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 4660, ptr %p\n";
    let text = looped("i16", "10", body, "0");
    let after =
        managed_fill_by(DOS, &text, &[&[0, 0], &[0, 9], &[0, 10]], Tuned { fills_pattern: true, ..Tuned::default() });
    assert!(after.contains("memset.pattern") && !after.contains("store i16 4660"), "{after}");
}

/// A word whose bytes differ stayed a loop, `rep stosw` being isel's alone:
/// each of its trips cost a store. It is one pattern fill of the cells.
#[test]
fn a_word_of_two_bytes_is_one_pattern_fill() {
    let body = "  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 4660, ptr %p\n";
    let text = looped("i16", "10", body, "0");
    assert!(text.contains("store i16 4660"), "premise: a loop of word stores");
    let after = patterned(&text, &[&[0, 0], &[0, 9], &[0, 10]]);
    assert!(
        after.contains(
            "call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr align 2 %p, i16 4660, i16 10, i1 false)"
        ),
        "{after}"
    );
}

/// A loop-invariant word that is no constant is a pattern too, counted in
/// cells: a count of `%n` is not doubled.
#[test]
fn an_invariant_word_is_one_pattern_fill() {
    let body = "  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 %q, ptr %p\n";
    let after = patterned(&looped("i16", "%n", body, "%e"), &[&[0, 7], &[1, 7], &[40, 4660], &[-5, 3]]);
    assert!(
        after.contains(
            "call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr align 2 %p, i16 %q, i16 %1, i1 false)"
        ),
        "{after}"
    );
}

#[test]
fn a_value_the_loop_changes_is_kept() {
    let body =
        "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  %b = trunc i16 %i to i8\n  store i8 %b, ptr %p\n";
    kept(&looped("i8", "%n", body, "%e"), TRIPS);
}

/// Under the pass manager, fill declares the memset the module lacks, as
/// LLVM's `Intrinsic::getDeclaration` does.
#[test]
fn an_undeclared_memset_is_declared() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  store i8 65, ptr %p\n";
    let before = parsed(&format!("{DOS}{}", looped("i8", "%n", body, "%e").replace(MEMSET, "")));
    let mut module = before.clone();
    let text = managed(&mut module, LoopIdioms { size: false });
    assert!(
        text.contains("declare void @llvm.memset.p0.i16(ptr")
            && text.contains("call void @llvm.memset.p0.i16(ptr align 1 %p, i8 65"),
        "{text}"
    );
    assert_eq!(results(&module, TRIPS), results(&before, TRIPS));
}

/// Trips times two bytes may wrap the index where no GEP is `inbounds`
/// and an unsigned test bounds the trips only by the width; a signed
/// test from 0 bounds them by half of it, so the words fit.
#[test]
fn a_word_count_that_may_wrap_is_kept() {
    let body = |inbounds: &str| {
        format!("  %p = getelementptr {inbounds}[64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 0, ptr %p\n")
    };
    let unsigned = |text: String| text.replace("icmp slt", "icmp ult");
    let trips: &[&[i128]] = &[&[0, 0], &[1, 0], &[1, 1], &[40, 39], &[40, 40]];
    kept(&unsigned(looped("i16", "%n", &body(""), "%e")), trips);
    let (text, changed) = fill(&unsigned(looped("i16", "%n", &body("inbounds "), "%e")), trips);
    assert!(
        changed && text.contains("call void @llvm.memset.p0.i16(ptr align 2 %p, i8 0, i16 %2, i1 false)"),
        "{text}"
    );
    assert!(fill(&looped("i16", "%n", &body(""), "%e"), TRIPS).1);
}

/// Nib's zeroed `i16[20, 20]` stored its rows again after the memset once
/// affine spelled the cell `gep (gep @buf, i * 40), j * 2`: the row base
/// was no address of the outer counter, so only rows filled, 200 stores.
#[test]
fn a_nest_addressed_off_its_row_base_is_one_memset() {
    let text = "@buf = global [400 x i16] zeroinitializer

MEMSETdefine i16 @f(i16 %n, i16 %q) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b5 ]
  %c = icmp slt i16 %i, 20
  br i1 %c, label %b2, label %b6

b2:
  %row = mul i16 %i, 40
  %base = getelementptr i8, ptr @buf, i16 %row
  br label %b3

b3:
  %j = phi i16 [ 0, %b2 ], [ %j.next, %b4 ]
  %d = icmp slt i16 %j, 20
  br i1 %d, label %b4, label %b5

b4:
  %o = mul i16 %j, 2
  %p = getelementptr i8, ptr %base, i16 %o
  store i16 0, ptr %p
  %j.next = add i16 %j, 1
  br label %b3

b5:
  %i.next = add i16 %i, 1
  br label %b1

b6:
  %r = getelementptr [400 x i16], ptr @buf, i16 0, i16 %q
  %v = load i16, ptr %r
  ret i16 %v
}
"
    .replace("MEMSET", MEMSET);
    let (after, _) = fill(&text, &[&[0, 0], &[0, 399]]);
    assert!(after.contains("call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 800, i1 false)"), "{after}");
}

/// @f storing `stores` into an 8-byte local, then returning the byte at `%q`.
fn local(stores: &str) -> String {
    format!(
        "{MEMSET}define i16 @f(i16 %n, i16 %q) {{
b0:
  %a = alloca [8 x i8]
  %p1 = getelementptr i8, ptr %a, i16 1
  %p2 = getelementptr i8, ptr %a, i16 2
  %p4 = getelementptr i8, ptr %a, i16 4
  %p6 = getelementptr i8, ptr %a, i16 6
  %p7 = getelementptr i8, ptr %a, i16 7
{stores}  %r = getelementptr i8, ptr %a, i16 %q
  %v = load i8, ptr %r
  %w = zext i8 %v to i16
  ret i16 %w
}}"
    )
}

const BYTES: &[&[i128]] = &[&[0, 0], &[0, 3], &[0, 6], &[0, 7]];

/// Every dialect's zeroed locals
/// were zeroed a word store at a time: qbdemo's PLASMA grew 38 of them where
/// B$ENRA had cleared the frame, 10% more code over the program.
#[test]
fn adjacent_stores_of_one_byte_are_one_memset() {
    let stores = "  store i16 0, ptr %a\n  store i16 0, ptr %p2\n  store i16 0, ptr %p4\n  store i8 0, ptr %p6\n  store i8 0, ptr %p7\n";
    let before = parsed(&format!("{DOS}{}", local(stores)));
    let mut module = before.clone();
    let text = managed(&mut module, Merge { size: false, widen: true });
    assert!(
        text.contains("call void @llvm.memset.p0.i16(ptr align 2 %a, i8 0, i16 8, i1 false)")
            && !text.contains("store"),
        "{text}"
    );
    assert_eq!(results(&module, BYTES), results(&before, BYTES));
}

/// Three words are a dword and a word as a memset, no fewer stores: kept,
/// as LLVM keeps them. Merged, deedlines' `1, 0, 0, 0` words lost the two
/// dword stores the backend had paired them into, for three.
#[test]
fn stores_a_memset_would_not_reduce_are_kept() {
    let stores = "  store i16 0, ptr %p2\n  store i16 0, ptr %p4\n  store i16 0, ptr %p6\n";
    let before = parsed(&format!("{DOS}{}", local(stores)));
    let mut module = before.clone();
    let text = managed(&mut module, Merge { size: false, widen: true });
    assert!(!text.contains("call void @llvm.memset"), "{text}");
}

/// A gap, another byte, a read between, or a store that overlaps keeps them.
#[test]
fn stores_that_are_not_one_fill_are_kept() {
    let unchanged = |stores: &str| {
        let before = parsed(&format!("{DOS}{}", local(stores)));
        let mut module = before.clone();
        let text = managed(&mut module, Merge { size: false, widen: true });
        assert!(!text.contains("call void @llvm.memset"), "{text}");
        assert_eq!(results(&module, BYTES), results(&before, BYTES));
    };
    unchanged("  store i16 0, ptr %a\n  store i16 0, ptr %p4\n");
    unchanged("  store i16 0, ptr %a\n  store i16 257, ptr %p2\n");
    unchanged("  store i16 0, ptr %a\n  %x = load i8, ptr %p2\n  store i16 0, ptr %p2\n");
    unchanged("  store i16 0, ptr %a\n  store i16 0, ptr %p2\n  store i8 9, ptr %p2\n");
}

/// A pattern fill set against a loop under the DOS target's own costs: a
/// loop that costs less a trip than `rep stos` costs once stays a loop, on
/// the 486's clocks and under -Os' bytes; one that costs more is filled.
#[test]
fn a_fill_is_priced_against_its_loop() {
    use llrm_mir::target::Machine;
    let dos = llrm_x86_m16::Dos::default();
    let (speed, size) = (dos.costs(), dos.size_costs());
    assert!(
        !super::_cheaper(10, None, Some(1), &speed, false, None),
        "premise: a short unknown loop does not pay its setup"
    );
    assert!(super::_cheaper(10, Some(1000), None, &speed, false, None));
    assert!(!super::_cheaper(1, None, None, &size, true, None));
    assert!(super::_cheaper(size.fill + 1, None, None, &size, true, None));
}

/// A fill through a huge pointer is one fill in the pointer's space, with a
/// count of 32 bits: the intrinsic carries the address space, and isel splits
/// it at each 64K segment (`isel/huge.rs`). It was left a loop, ten
/// instructions to a byte. A byte is a memset. A word of two different bytes
/// would be a pattern, but its index is wider than space 0's, which the pattern
/// fill cannot count: it stays a loop (huge fillw, 4 rows, #6195).
#[test]
fn a_byte_fill_through_a_huge_pointer_is_one_fill_in_its_space_and_a_word_fill_stays_a_loop() {
    let text = "@big = addrspace(3) global [40000 x i16] zeroinitializer

define i16 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i32 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %p = getelementptr inbounds [40000 x i16], ptr addrspace(3) @big, i32 0, i32 %i
  store i16 4660, ptr addrspace(3) %p
  %next = add i32 %i, 1
  br label %b1

b3:
  ret i16 0
}
";
    assert!(text.contains("store i16 4660"), "premise: a loop of word stores");
    let after = patterned(text, &[&[0], &[1], &[300]]);
    assert!(!after.contains("memset.pattern"), "{after}");
    assert!(after.contains("store i16 4660"), "{after}");
    let bytes =
        text.replace("[40000 x i16]", "[40000 x i8]").replace("i16 4660", "i8 65").replace("store i16", "store i8");
    let after = managed_fill(&bytes, &[&[0], &[1], &[300]]);
    assert!(after.contains("call void @llvm.memset.p3.i32(ptr addrspace(3) "), "{after}");
}

/// Under -Os a one-store loop through a huge pointer stays a loop: the fill is
/// a routine laid down at the site, 94 bytes against the loop's dozen (fillw
/// -Os 86 -> 176 bytes against main).
#[test]
fn a_one_store_loop_through_a_huge_pointer_stays_a_loop_under_size() {
    let text = "@big = addrspace(3) global [40000 x i8] zeroinitializer

define i16 @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i32 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %p = getelementptr inbounds [40000 x i8], ptr addrspace(3) @big, i32 0, i32 %i
  store i8 65, ptr addrspace(3) %p
  %next = add i32 %i, 1
  br label %b1

b3:
  ret i16 0
}
";
    let before = parsed(&format!("{DOS}{text}"));
    let on_dos = |size: bool| {
        let mut module = before.clone();
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.add(LoopIdioms { size });
        manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
        printed(&module)
    };
    let after = on_dos(true);
    assert!(!after.contains("llvm.memset"), "{after}");
    let speed = on_dos(false);
    assert!(speed.contains("call void @llvm.memset.p3.i32("), "premise: speed fills it\n{speed}");
}

/// A copy loop through huge pointers stays a loop under -Os as well (copyw
/// -Os 321 -> 470 bytes against main, the routine laid down at the site).
#[test]
fn a_copy_loop_through_huge_pointers_stays_a_loop_under_size() {
    let text = "@a = addrspace(3) global [40000 x i16] zeroinitializer
@b = addrspace(3) global [40000 x i16] zeroinitializer

define i16 @f() {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp ult i16 %i, -25536
  br i1 %c, label %b2, label %b3

b2:
  %z = zext i16 %i to i32
  %o = shl nuw nsw i32 %z, 1
  %p = getelementptr inbounds nuw i8, ptr addrspace(3) @a, i32 %o
  %v = load i16, ptr addrspace(3) %p
  %q = getelementptr inbounds nuw i8, ptr addrspace(3) @b, i32 %o
  store i16 %v, ptr addrspace(3) %q
  %next = add nuw i16 %i, 1
  br label %b1

b3:
  ret i16 0
}
";
    let before = parsed(&format!("{DOS}{text}"));
    let on_dos = |size: bool| {
        let mut module = before.clone();
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.add(LoopIdioms { size });
        manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
        printed(&module)
    };
    let speed = on_dos(false);
    assert!(speed.contains("call void @llvm.memcpy.p3.p3.i32("), "premise: speed copies it\n{speed}");
    let after = on_dos(true);
    assert!(!after.contains("llvm.memcpy"), "{after}");
}

/// Two arrays of 64 words, each cell its own number, and @f(n, q): `body`
/// run for `i` from 0 below `n`, or, where `down`, from `n - 1` to 0; then
/// cell `q` of @a. Each cell is read back by some input, so a copy that
/// moves the wrong cells shows.
fn copying(
    body: &str,
    down: bool,
) -> String {
    let cells = |base: i32| (0..64).map(|at| format!("i16 {}", base + at)).collect::<Vec<_>>().join(", ");
    let (start, test, step) =
        if down { ("%last", "icmp sge i16 %i, 0", "-1") } else { ("0", "icmp slt i16 %i, %n", "1") };
    format!(
        "@a = global [64 x i16] [{}]
@b = global [64 x i16] [{}]

define i16 @f(i16 %n, i16 %q) {{
b0:
  %last = sub i16 %n, 1
  br label %b1

b1:
  %i = phi i16 [ {start}, %b0 ], [ %next, %b2 ]
  %c = {test}
  br i1 %c, label %b2, label %b3

b2:
{body}  %next = add i16 %i, {step}
  br label %b1

b3:
  %r = getelementptr [64 x i16], ptr @a, i16 0, i16 %q
  %out = load i16, ptr %r
  ret i16 %out
}}
",
        cells(100),
        cells(1000)
    )
}

const COPY_INPUTS: &[&[i128]] =
    &[&[0, 0], &[1, 0], &[1, 1], &[5, 3], &[30, 0], &[30, 10], &[30, 29], &[30, 30], &[30, 31]];

fn cells(
    from: &str,
    to: &str,
    shift: &str,
) -> String {
    format!(
        "  %j = add i16 %i, {shift}\n  %s = getelementptr inbounds [64 x i16], ptr {from}, i16 0, i16 %i\n  %v = load i16, ptr %s\n  %d = getelementptr inbounds [64 x i16], ptr {to}, i16 0, i16 %j\n  store i16 %v, ptr %d\n"
    )
}

/// Cells of one array read into another were loop trips: two objects are
/// apart, so one `llvm.memcpy`, its count the trips.
#[test]
fn a_copy_between_two_objects_is_one_memcpy() {
    let body = cells("@b", "@a", "0");
    let text = copying(&body, false);
    assert!(
        text.contains("load i16, ptr %s") && text.contains("store i16 %v, ptr %d"),
        "premise: a load and a store each trip"
    );
    let after = managed_fill(&text, COPY_INPUTS);
    assert!(after.contains("call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 %"), "{after}");
}

/// A copy of an array onto itself, reading ahead of its writes (`a[i] =
/// a[i+1]`), makes each trip read what the loop has not yet touched: that is
/// `llvm.memmove` running up.
#[test]
fn an_overlapping_copy_that_reads_ahead_is_a_forward_memmove() {
    let body = "  %j = add i16 %i, 1\n  %s = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %j\n  %v = load i16, ptr %s\n  %d = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %i\n  store i16 %v, ptr %d\n";
    let text = copying(body, false);
    assert!(
        text.contains("load i16, ptr %s") && text.contains("store i16 %v, ptr %d"),
        "premise: a load and a store each trip"
    );
    let after = managed_fill(&text, COPY_INPUTS);
    assert!(
        after.contains("call void @llvm.memmove.p0.p0.i16(ptr %d, ptr %s, i16 %") && after.contains("!llrm.forward"),
        "{after}"
    );
}

/// The same shifted up, from the end (`a[i+1] = a[i]`, `i` falling), is a
/// `llvm.memmove` running down.
#[test]
fn an_overlapping_copy_that_runs_down_is_a_backward_memmove() {
    let body = "  %j = add i16 %i, 1\n  %s = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %i\n  %v = load i16, ptr %s\n  %d = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %j\n  store i16 %v, ptr %d\n";
    let text = copying(body, true);
    assert!(
        text.contains("load i16, ptr %s") && text.contains("icmp sge"),
        "premise: a falling loop of loads and stores"
    );
    let after = managed_fill(&text, COPY_INPUTS);
    assert!(after.contains("call void @llvm.memmove.p0.p0.i16(") && after.contains("!llrm.backward"), "{after}");
}

/// Shifted up and rising (`a[i+1] = a[i]`), each trip reads what the one
/// before wrote: the first cell smeared across the rest. No copy does that.
#[test]
fn a_smearing_copy_is_kept() {
    let body = "  %j = add i16 %i, 1\n  %s = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %i\n  %v = load i16, ptr %s\n  %d = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %j\n  store i16 %v, ptr %d\n";
    let text = copying(body, false);
    assert!(text.contains("store i16 %v, ptr %d"), "premise: a store of the loaded cell");
    kept(&text, COPY_INPUTS);
}

/// A value the loop changes between the load and the store is no copy.
#[test]
fn a_transformed_copy_is_kept() {
    let body = "  %s = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %i\n  %v = load i16, ptr %s\n  %w = add i16 %v, 1\n  %d = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %i\n  store i16 %w, ptr %d\n";
    let text = copying(body, false);
    assert!(
        text.contains("load i16, ptr %s") && text.contains("store i16 %w"),
        "premise: a load and a store each trip"
    );
    kept(&text, COPY_INPUTS);
}

/// A second store each trip is another effect: not one copy.
#[test]
fn a_copy_that_also_stores_elsewhere_is_kept() {
    let body = format!(
        "{}  %e = getelementptr inbounds [64 x i16], ptr @b, i16 0, i16 %i\n  store i16 7, ptr %e\n",
        cells("@b", "@a", "0")
    );
    kept(&copying(&body, false), COPY_INPUTS);
}

/// A copy of words is priced as `rep movs` in dwords against the loop on the
/// DOS target's clocks: a short one stays a loop (or is loads and stores), a
/// long one is a string move, and running down costs its `std` and `cld`.
#[test]
fn a_copy_is_priced_against_its_loop() {
    use llrm_mir::target::Machine;
    let speed = llrm_x86_m16::Dos::default().costs();
    assert!(
        !super::_cheaper(1, None, Some(1), &speed, false, Some((2, false))),
        "premise: a short unknown loop does not pay the setup"
    );
    assert!(super::_cheaper(10, Some(1000), None, &speed, false, Some((2, false))));
    let forward = super::_cheaper(1, Some(40), None, &speed, false, Some((2, false)));
    let backward = super::_cheaper(1, Some(40), None, &speed, false, Some((2, true)));
    assert!(!(backward && !forward), "running down is dearer, never cheaper");
    // Seven known words under -Os: isel makes four loads and four stores of
    // them, as large as they are, so a loop of 30 bytes stays. Priced as
    // `rep movs` it grew lru.nib by 39 bytes.
    let size = llrm_x86_m16::Dos::default().size_costs();
    assert!(!super::_cheaper(30, Some(7), None, &size, true, Some((2, false))));
}

const FLAT: &str = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-n8:16:32\"\n\n";

/// bench/scroll on -m32: a `short` counter, its cell addressed `2 * sext(i +
/// k)` off an i8 GEP. The loops were not made `memmove`: the counter's width
/// (16) is not the index's (32), and scroll ran 1196306 instructions against
/// gcc's 176970.
fn scroll(up: bool) -> String {
    let cells = (0..64).map(|at| format!("i16 {}", 100 + at)).collect::<Vec<_>>().join(", ");
    let (start, test, step, from, to) =
        if up { ("0", "icmp slt i16 %i, 20", "1", "%j", "%w") } else { ("19", "icmp sge i16 %i, 0", "-1", "%w", "%j") };
    format!(
        "@a = global [128 x i16] [{cells}, {cells}]

define i32 @f(i16 %n, i32 %q) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ {start}, %b0 ], [ %next, %b2 ]
  %c = {test}
  br i1 %c, label %b2, label %b3

b2:
  %w = sext i16 %i to i32
  %j = add nsw i32 %w, 8
  %x = mul i32 {from}, 2
  %s = getelementptr inbounds i8, ptr @a, i32 %x
  %v = load i16, ptr %s
  %y = mul i32 {to}, 2
  %d = getelementptr inbounds i8, ptr @a, i32 %y
  store i16 %v, ptr %d
  %next = add nsw i16 %i, {step}
  br label %b1

b3:
  %r = getelementptr i16, ptr @a, i32 %q
  %out = load i16, ptr %r
  %z = sext i16 %out to i32
  ret i32 %z
}}
"
    )
}

const SCROLL_INPUTS: &[&[i128]] = &[&[0, 0], &[1, 3], &[20, 0], &[20, 8], &[20, 27], &[30, 40], &[40, 70]];

/// `a[i] = a[i + 8]` on a 16-bit counter over 32-bit pointers is one forward
/// `llvm.memmove`.
#[test]
fn a_short_counter_scroll_up_is_one_memmove() {
    let after = managed_fill_on(FLAT, &scroll(true), SCROLL_INPUTS);
    assert!(after.contains("llvm.memmove") && after.contains("!llrm.forward"), "{after}");
}

/// `a[i + 8] = a[i]` with `i` falling is one backward `llvm.memmove`.
#[test]
fn a_short_counter_scroll_down_is_one_memmove() {
    let after = managed_fill_on(FLAT, &scroll(false), SCROLL_INPUTS);
    assert!(after.contains("llvm.memmove") && after.contains("!llrm.backward"), "{after}");
}

/// `looped`, rotated by `-ftree-ch`: the test copied before the loop and kept
/// at its latch, the loop entered through a block of its own. The exit takes
/// the counter after its last step.
fn guarded(
    cell: &str,
    bound: &str,
    body: &str,
) -> String {
    let widened = if cell == "i16" { "or i16 %v, 0".to_owned() } else { format!("zext {cell} %v to i16") };
    format!(
        "@buf = global [64 x {cell}] zeroinitializer

{MEMSET}define i16 @f(i16 %n, i16 %q) {{
b0:
  %go = icmp slt i16 0, {bound}
  br i1 %go, label %pre, label %b3

pre:
  br label %b1

b1:
  %i = phi i16 [ 0, %pre ], [ %next, %b1 ]
{body}  %next = add i16 %i, 1
  %c = icmp slt i16 %next, {bound}
  br i1 %c, label %b1, label %b3

b3:
  %e = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %r = getelementptr [64 x {cell}], ptr @buf, i16 0, i16 %q
  %v = load {cell}, ptr %r
  %w = {widened}
  %x = add i16 %w, %e
  ret i16 %x
}}
"
    )
}

/// A loop tested after its trips and entered behind its own test is one memset
/// too, as the loop the test was copied from was: the copied loop's fills were
/// lost (bench/sieve, -ftree-ch: 12,106 → 16,411 instructions).
#[test]
fn a_guarded_byte_loop_is_one_memset() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  store i8 65, ptr %p\n";
    let (text, changed) = fill(&guarded("i8", "%n", body), TRIPS);
    assert!(changed, "{text}");
    assert!(text.contains("call void @llvm.memset.p0.i16(ptr align 1 %p, i8 65, i16 %"), "{text}");
}

/// Constants that are not a memset, being less than the stores they would make,
/// are stores of the widest integer: three words of -1 are a dword and a word,
/// as gcc's `store-merging` and LLVM's `MergeConsecutiveStores` make them
/// (lru's `bprev` and `bnext`: six word stores each at -O2, 20 bytes of its
/// code more than clang's). Two bytes of different values are one word of both,
/// little-endian.
#[test]
fn adjacent_constants_are_stores_of_the_widest_integer() {
    let stores = |text: &str| {
        let before = parsed(&format!("{DOS}{}", local(text)));
        let mut module = before.clone();
        let after = managed(&mut module, Merge { size: false, widen: true });
        assert_eq!(results(&module, BYTES), results(&before, BYTES), "{after}");
        after
    };
    let words = stores("  store i16 -1, ptr %a\n  store i16 -1, ptr %p2\n  store i16 -1, ptr %p4\n");
    assert!(words.contains("store i32 -1, ptr %a") && words.matches("store").count() == 2, "{words}");
    assert!(!words.contains("call void @llvm.memset"), "{words}");
    let bytes = stores("  store i8 18, ptr %a\n  store i8 52, ptr %p1\n");
    assert!(bytes.contains("store i16 13330, ptr %a") && bytes.matches("store").count() == 1, "{bytes}");
}

/// A read between, a store that overlaps, a volatile store, and a dword that
/// would start off its alignment keep their stores.
#[test]
fn constants_that_cannot_be_one_wider_store_are_kept() {
    let kept = |text: &str, count: usize| {
        let before = parsed(&format!("{DOS}{}", local(text)));
        let mut module = before.clone();
        let after = managed(&mut module, Merge { size: false, widen: true });
        assert_eq!(after.matches("store").count(), count, "{after}");
        assert_eq!(results(&module, BYTES), results(&before, BYTES));
    };
    kept("  store i8 18, ptr %a\n  %x = load i8, ptr %a\n  store i8 52, ptr %p1\n", 2);
    kept("  store i16 7, ptr %a\n  store i16 9, ptr %p2\n  store i8 1, ptr %p2\n", 3);
    kept("  store volatile i8 18, ptr %a\n  store i8 52, ptr %p1\n", 2);
    kept("  store i8 18, ptr %p1\n  store i8 52, ptr %p2\n", 2);
}

/// A loop of `trips` that stores `first` to `@s`'s first array and `second` to
/// its second, and @f returns cell `%q` of both and the first of the third.
fn two_fills(
    trips: i32,
    first: &str,
    second: &str,
    second_index: &str,
) -> String {
    format!(
        "%S = type {{ [32 x i16], [32 x i16], [32 x i16] }}
@s = global %S zeroinitializer

{MEMSET}define i16 @f(i16 %n, i16 %q) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, {trips}
  br i1 %c, label %b2, label %b3

b2:
  %p = getelementptr inbounds %S, ptr @s, i16 0, i32 0, i16 %i
  store i16 {first}, ptr %p
  %r = getelementptr inbounds %S, ptr @s, i16 0, i32 1, i16 {second_index}
  store i16 {second}, ptr %r
  %next = add i16 %i, 1
  br label %b1

b3:
  %a = getelementptr %S, ptr @s, i16 0, i32 0, i16 %q
  %b = getelementptr %S, ptr @s, i16 0, i32 1, i16 %q
  %v = load i16, ptr %a
  %w = load i16, ptr %b
  %x = add i16 %v, %w
  ret i16 %x
}}
"
    )
}

const CELLS: &[&[i128]] = &[&[0, 0], &[0, 3], &[0, 6], &[0, 7]];

/// Two stores a trip, to arrays that cannot meet, are a memset each by the same
/// trips: lru's `lhead` and `ltail` (a 7-trip loop of two word stores, 42
/// clocks, in the init of a kernel whose gcc and clang spend 8).
#[test]
fn a_loop_of_two_fills_to_apart_bytes_is_a_memset_each() {
    let (text, changed) = fill(&two_fills(30, "-1", "-1", "%i"), CELLS);
    assert!(changed);
    assert!(
        text.contains("call void @llvm.memset.p0.i16(ptr align 2 %p, i8 -1, i16 60, i1 false)")
            && text.contains("call void @llvm.memset.p0.i16(ptr align 2 %r, i8 -1, i16 60, i1 false)")
            && !text.contains("store"),
        "{text}"
    );
}

/// Fills that may meet keep the loop: 33 trips over 32 cells run the first
/// array into the second's bytes, and a word that is not a repeated byte is no
/// memset.
#[test]
fn fills_that_may_meet_are_kept() {
    for text in [two_fills(33, "-1", "-1", "%i"), two_fills(30, "-1", "%n", "%i")] {
        let (after, changed) = fill(&text, CELLS);
        assert!(!changed && !after.contains("call void @llvm.memset"), "{after}");
    }
}

/// Seven trips of two word stores are stores enough: priced against the loop in
/// bytes as well as clocks, the memsets are no gain (bench: lru -O2 grew 78
/// bytes in 16-bit code where its loop was 20).
#[test]
fn a_short_loop_of_two_fills_is_kept_where_the_stores_are_no_smaller() {
    let (after, changed) = fill(&two_fills(7, "-1", "-1", "%i"), CELLS);
    assert!(!changed && !after.contains("call void @llvm.memset"), "{after}");
}

/// Tuned for size, a pair of constants is one store only where that is fewer
/// code bytes on the target: in 16-bit code two word stores of a value held in
/// a register are six bytes a pair, the dword immediate with its operand-size
/// prefix nine (sum_three -Os +3 bytes); two values that are not held are
/// twelve against nine.
#[test]
fn a_pair_of_constants_is_one_store_tuned_for_size_where_the_bytes_are_fewer() {
    let stores = |text: &str, size: bool| {
        let before = parsed(&format!("{DOS}{}", local(text)));
        let mut module = before.clone();
        let (layout, outer) =
            (layout(&module), Outer::of(&module, Some(std::rc::Rc::new(llrm_x86_m16::Dos::default()))));
        let callees = llrm_mir::memory::callees(&module);
        let (context, function) = module.function_mut("f").expect("@f");
        crate::mergestores::merged_constants(context, &layout, &callees, function, &outer, size);
        let after = printed(&module);
        assert_eq!(results(&module, BYTES), results(&before, BYTES), "{after}");
        after.matches("store").count()
    };
    let held = "  store i16 4, ptr %a\n  store i16 4, ptr %p2\n  store i16 4, ptr %p4\n  store i16 4, ptr %p6\n";
    let unique = "  store i16 7, ptr %a\n  store i16 9, ptr %p2\n";
    assert_eq!(stores(held, false), 2, "speed: two dwords");
    assert_eq!(stores(held, true), 4, "size: a register already holds the 4");
    assert_eq!(stores(unique, true), 1, "size: nine bytes against twelve");
}

/// lru's init: `lhead` and `ltail` are adjacent arrays of one struct, a loop of
/// `trips` stores `-1` to each.
fn adjacent_fills(trips: i32) -> String {
    format!(
        "%S = type {{ [{trips} x i16], [{trips} x i16] }}
@s = global %S zeroinitializer

{MEMSET}define i16 @f(i16 %n, i16 %q) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, {trips}
  br i1 %c, label %b2, label %b3

b2:
  %p = getelementptr inbounds %S, ptr @s, i16 0, i32 0, i16 %i
  store i16 -1, ptr %p
  %r = getelementptr inbounds %S, ptr @s, i16 0, i32 1, i16 %i
  store i16 -1, ptr %r
  %next = add i16 %i, 1
  br label %b1

b3:
  %a = getelementptr %S, ptr @s, i16 0, i32 0, i16 %q
  %b = getelementptr %S, ptr @s, i16 0, i32 1, i16 %q
  %v = load i16, ptr %a
  %w = load i16, ptr %b
  %x = add i16 %v, %w
  ret i16 %x
}}
"
    )
}

/// The streams of a loop that tile their bytes with one byte are one memset:
/// gcc distributes lru's 7-trip loop of two word stores into two memsets and
/// merges them, adjacent as they are, into one 28-byte `rep stosl`. Kept as two
/// stores a trip it ran 150 clocks against gcc's 127.
#[test]
fn a_loop_of_two_adjacent_fills_is_one_memset_of_both() {
    let before = parsed(&format!("{DOS}{}", adjacent_fills(7)));
    let mut module = before.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, Some(std::rc::Rc::new(llrm_x86_m16::Dos::default()))));
    let callees = llrm_mir::memory::callees(&module);
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = filled(context, &layout, &callees, function, &outer, &mut declared, false);
    let after = printed(&module);
    assert!(changed, "{after}");
    assert_eq!(after.matches("call void @llvm.memset").count(), 1, "{after}");
    assert!(after.contains("i8 -1, i16 28, i1 false)") && !after.contains("store"), "{after}");
    assert_eq!(results(&module, CELLS), results(&before, CELLS), "{after}");
}

/// Streams with a gap between them are two memsets, not one over the gap.
#[test]
fn a_loop_of_two_fills_with_a_gap_is_not_one_memset() {
    let (text, changed) = fill(&two_fills(30, "-1", "-1", "%i"), CELLS);
    assert!(changed && text.matches("call void @llvm.memset").count() == 2, "{text}");
}

/// A loop of `undef` byte stores is a memset of `undef`, not poison
/// (`isBytewiseValue` answers `UndefValue`). A guard: it also passes with
/// `byte_operand` giving poison, so the arm is reached another way.
#[test]
fn a_loop_of_undef_bytes_is_a_memset_of_undef() {
    let body = "  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i\n  store i8 undef, ptr %p\n";
    let (text, changed) = fill(&looped("i8", "%n", body, "%e"), TRIPS);
    assert!(changed);
    assert!(text.contains("call void @llvm.memset.p0.i16(ptr align 1 %p, i8 undef, i16 %1, i1 false)"), "{text}");
}

/// `buf[i + 7] = buf[i]` for `i` falling from `n - 1` (idiom_edges.c's
/// `bytes_down`), the cells `body` addresses. Seven bytes apart and read
/// before they are written: a backward `llvm.memmove`. The alias query's
/// answer for two accesses off one index (apart: a constant 7 > the cell) is
/// about one trip; across trips the ranges overlap, and a `memcpy` copies them
/// forward through each other.
fn bytes_down(
    body: &str,
    start: &str,
) -> String {
    format!(
        "@buf = global [140 x i8] zeroinitializer

{MEMSET}define i16 @f(i16 %n, i16 %q) {{
b0:
  %s = sub i16 %n, 1
  br label %b1

b1:
  %i = phi i16 [ {start}, %b0 ], [ %next, %b2 ]
  %c = icmp sge i16 %i, 0
  br i1 %c, label %b2, label %b3

b2:
{body}  %next = sub i16 %i, 1
  br label %b1

b3:
  %r = getelementptr i8, ptr @buf, i16 %q
  %v = load i8, ptr %r
  %w = zext i8 %v to i16
  ret i16 %w
}}
"
    )
}

#[test]
fn a_falling_byte_copy_seven_apart_is_never_a_memcpy_whatever_its_addresses_look_like() {
    const INPUTS: &[&[i128]] = &[&[0, 0], &[1, 7], &[17, 20], &[17, 3], &[30, 29]];
    for body in [
        "  %p = getelementptr i8, ptr @buf, i16 %i\n  %x = load i8, ptr %p\n  %j = add i16 %i, 7\n  %d = getelementptr i8, ptr @buf, i16 %j\n  store i8 %x, ptr %d\n",
        "  %p = getelementptr i8, ptr @buf, i16 %i\n  %x = load i8, ptr %p\n  %k = getelementptr i8, ptr @buf, i16 %i\n  %d = getelementptr i8, ptr %k, i16 7\n  store i8 %x, ptr %d\n",
    ] {
        // A count only the call knows, and a constant one.
        for start in ["%s", "16"] {
            let after = managed_fill(&bytes_down(body, start), INPUTS);
            assert!(!after.contains("llvm.memcpy"), "{start}\n{after}");
            assert!(!after.contains("llvm.memmove") || after.contains("!llrm.backward"), "{start}\n{after}");
        }
    }
}

/// `screen[i + 80] = screen[i]` for `i` falling from `n - 1` (bench/scroll's
/// scroll back down) on words.
fn words_down(
    body: &str,
    start: &str,
) -> String {
    format!(
        "@buf = global [2000 x i16] zeroinitializer

{MEMSET}define i16 @f(i16 %n, i16 %q) {{
b0:
  %s = sub i16 %n, 1
  br label %b1

b1:
  %i = phi i16 [ {start}, %b0 ], [ %next, %b2 ]
  %c = icmp sge i16 %i, 0
  br i1 %c, label %b2, label %b3

b2:
{body}  %next = sub i16 %i, 1
  br label %b1

b3:
  %r = getelementptr i16, ptr @buf, i16 %q
  %v = load i16, ptr %r
  ret i16 %v
}}
"
    )
}

#[test]
fn a_falling_word_copy_eighty_cells_up_is_never_a_memcpy_whatever_its_addresses_look_like() {
    const INPUTS: &[&[i128]] = &[&[0, 0], &[1, 80], &[100, 150], &[100, 3], &[1920, 1999]];
    for body in [
        "  %p = getelementptr i16, ptr @buf, i16 %i\n  %x = load i16, ptr %p\n  %j = add i16 %i, 80\n  %d = getelementptr i16, ptr @buf, i16 %j\n  store i16 %x, ptr %d\n",
        "  %p = getelementptr i16, ptr @buf, i16 %i\n  %x = load i16, ptr %p\n  %k = getelementptr i16, ptr @buf, i16 %i\n  %d = getelementptr i8, ptr %k, i16 160\n  store i16 %x, ptr %d\n",
    ] {
        for start in ["%s", "1919"] {
            let after = managed_fill(&words_down(body, start), INPUTS);
            assert!(!after.contains("llvm.memcpy"), "{start}\n{after}");
            assert!(!after.contains("llvm.memmove") || after.contains("!llrm.backward"), "{start}\n{after}");
        }
    }
}

/// QCport m16 -Os, loop and fill bytes as the model sums them: mod_tex's two
/// fills (loop 28) beat its loop; snd_mix's (24) and lru's four (42) lose to
/// theirs. A fill is priced at its encoded 12 bytes, not a fitted fraction of
/// the doubled table (3 * 18 / 4 = 13.5 priced a 24-byte loop above two fills).
#[test]
fn a_fill_is_priced_at_its_encoded_size() {
    assert!(super::fills_cheaper(28, 2, 12));
    assert!(!super::fills_cheaper(24, 2, 12));
    assert!(!super::fills_cheaper(42, 4, 12));
}

/// lru's init at m16 -Os: the loop of two word stores is 5 bytes over the
/// `rep stosb` the pin makes of it and 87 clocks under it (42 against 129),
/// 17 clocks a byte against the target's 10. Formed on bytes alone it took
/// bench lru -Os from 42 to 129 clocks for those 5 bytes.
#[test]
fn a_fill_that_saves_five_bytes_for_87_clocks_is_not_worth_it() {
    assert!(!super::clocks_pay(5, 10_000, 129, 42));
    assert!(super::clocks_pay(5, 10_000, 90, 42), "48 clocks for 5 bytes pay");
}

/// A word fill made by this module carries the stores' alignment (the ABI one
/// where a store states none) on the memset's destination: without it the
/// backend filled by bytes (`rep stosb` of 202, particle nib -O2, +2424 clocks
/// over `rep stosw`).
#[test]
fn a_word_fill_keeps_the_alignment_of_its_stores() {
    let text = "@z = global [101 x i16] zeroinitializer

define void @f() {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %p = getelementptr [101 x i16], ptr @z, i16 0, i16 %i
  store i16 0, ptr %p
  %next = add nuw nsw i16 %i, 1
  %done = icmp eq i16 %next, 101
  br i1 %done, label %b2, label %b1

b2:
  ret void
}
";
    let after = managed_fill(text, &[&[]]);
    assert!(after.contains("llvm.memset"), "{after}");
    assert!(after.contains("@llvm.memset.p0.i16(ptr align 2 "), "{after}");
}

/// A target whose backend merges stores (`widen: false`) keeps the stores the
/// pin's IR has: wasm `scroll` -O3 inlined `bench_scroll` into `main` because
/// 80 word stores had become 20 qword stores by the time the inliner priced
/// them (cost under 250 where the pin's is 525), 2150 B for the pin's 1152.
#[test]
fn a_target_that_merges_stores_in_the_backend_keeps_adjacent_constants_apart() {
    let text = "  store i8 18, ptr %a\n  store i8 52, ptr %p1\n";
    let mut module = parsed(&format!("{DOS}{}", local(text)));
    let kept = managed(&mut module, Merge { size: false, widen: false });
    assert_eq!(kept.matches("store i8").count(), 2, "{kept}");
    let mut module = parsed(&format!("{DOS}{}", local(text)));
    let merged = managed(&mut module, Merge { size: false, widen: true });
    assert_eq!(merged.matches("store i8").count(), 0, "{merged}");
}

const TWO_BYTE_ARRAYS: &str = "@a = global [8 x i8] zeroinitializer
@b = global [8 x i8] zeroinitializer

define void @f() {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b1 ]
  %p = getelementptr [8 x i8], ptr @a, i32 0, i32 %i
  store i8 0, ptr %p
  %q = getelementptr [8 x i8], ptr @b, i32 0, i32 %i
  store i8 7, ptr %q
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 8
  br i1 %done, label %b2, label %b1

b2:
  ret void
}
";

/// lru -Os at m32 (no segment setup, no clock price) took the pin's memsets
/// the loop was smaller than: five inline fills, +38 B over keeping the loop
/// (815 against 777). A fill is taken when its stores are fewer bytes than the
/// loop.
#[test]
fn a_fill_stored_inline_is_taken_only_when_its_stores_are_fewer_bytes_than_the_loop() {
    let sized = |store_immediate: i64| {
        let costs = crate::profit::OperationCosts {
            fill: 12,
            store: 6,
            store_immediate,
            add: 4,
            branch: 2,
            ..Default::default()
        };
        let machine = crate::testing::Tuned { sizes: costs, ..Default::default() };
        let mut module = parsed(&format!("target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n{TWO_BYTE_ARRAYS}"));
        let mut pass = crate::loopidiom::pass::LoopIdiom::new();
        pass.for_size = true;
        crate::testing::managed_on(&mut module, pass, machine)
    };
    let (cheap, dear) = (sized(1), sized(10));
    assert!(cheap.contains("call void @llvm.memset"), "stores under the loop's bytes: the memsets\n{cheap}");
    assert!(!dear.contains("call void @llvm.memset"), "stores over the loop's bytes: the loop\n{dear}");
}

const SMALL_INIT_LOOP: &str = "@a = global [3 x i16] zeroinitializer
@b = global [3 x i16] zeroinitializer
@c = global [3 x i16] zeroinitializer
@d = global [3 x i32] zeroinitializer

define void @f() {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %pa = getelementptr [3 x i16], ptr @a, i16 0, i16 %i
  store i16 0, ptr %pa
  %pb = getelementptr [3 x i16], ptr @b, i16 0, i16 %i
  store i16 -1, ptr %pb
  %pc = getelementptr [3 x i16], ptr @c, i16 0, i16 %i
  store i16 -1, ptr %pc
  %pd = getelementptr [3 x i32], ptr @d, i16 0, i16 %i
  store i32 0, ptr %pd
  %next = add nuw nsw i16 %i, 1
  %done = icmp eq i16 %next, 3
  br i1 %done, label %b2, label %b1

b2:
  ret void
}
";

/// lru's init loops (three trips of four stores to four small objects) stayed
/// loops at m16 -O2: the model priced each fill as `rep stos` (17 clocks and 4
/// a dword), 128 against the loop's 54, while the backend stores a fill of up
/// to `MaxStoresPerMemset` stores inline, as main did (+24 clocks, +19
/// instructions, +5 memory operands over main on lru c).
#[test]
fn a_small_fill_the_backend_stores_inline_is_priced_as_those_stores() {
    let mut module = parsed(&format!("{DOS}{SMALL_INIT_LOOP}"));
    let machine = std::rc::Rc::new(llrm_x86_m16::Dos::default());
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::loopidiom::pass::LoopIdiom::new());
    manager.run_module(&mut module, machine).unwrap();
    let after = printed(&module);
    assert!(after.matches("call void @llvm.memset").count() == 4, "{after}");
}

const ZEROED_NEST: &str = "@a = global [8 x [8 x i16]] zeroinitializer
define void @f() {
b0:
  br label %o
o:
  %i = phi i16 [ 0, %b0 ], [ %in, %ol ]
  br label %j
j:
  %jj = phi i16 [ 0, %o ], [ %jn, %j ]
  %p = getelementptr [8 x [8 x i16]], ptr @a, i16 0, i16 %i, i16 %jj
  store i16 0, ptr %p
  %jn = add nuw nsw i16 %jj, 1
  %jd = icmp eq i16 %jn, 8
  br i1 %jd, label %ol, label %j
ol:
  %in = add nuw nsw i16 %i, 1
  %id = icmp eq i16 %in, 8
  br i1 %id, label %b2, label %o
b2:
  ret void
}
";

/// The same nest whose inner loop also sums, and the sum is returned.
const LIVE_NEST: &str = "@a = global [8 x [8 x i16]] zeroinitializer
define i16 @f() {
b0:
  br label %o
o:
  %i = phi i16 [ 0, %b0 ], [ %in, %ol ]
  %s0 = phi i16 [ 0, %b0 ], [ %s1, %ol ]
  br label %j
j:
  %jj = phi i16 [ 0, %o ], [ %jn, %j ]
  %s = phi i16 [ %s0, %o ], [ %sn, %j ]
  %p = getelementptr [8 x [8 x i16]], ptr @a, i16 0, i16 %i, i16 %jj
  store i16 0, ptr %p
  %sn = add i16 %s, 1
  %jn = add nuw nsw i16 %jj, 1
  %jd = icmp eq i16 %jn, 8
  br i1 %jd, label %ol, label %j
ol:
  %s1 = phi i16 [ %sn, %j ]
  %in = add nuw nsw i16 %i, 1
  %id = icmp eq i16 %in, 8
  br i1 %id, label %b2, label %o
b2:
  %r = phi i16 [ %s1, %ol ]
  ret i16 %r
}
";

fn through_the_loop_passes(text: &str) -> String {
    let mut module = parsed(&format!("{DOS}{text}"));
    let machine = std::rc::Rc::new(llrm_x86_m16::Dos::default());
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::loopidiom::pass::LoopIdiom::new());
    manager.add(crate::loopidiom::pass::AfterDeletion {
        deletion: crate::loopdeletion::LoopDeletion,
        idiom: crate::loopidiom::pass::LoopIdiom::new(),
    });
    manager.run_module(&mut module, machine).unwrap();
    printed(&module)
}

/// matmul nib -O2: the zero loops of an 8 by 8 array were eight memsets of a
/// row in a loop (+135 clocks, +52 instructions, +73 memory operands over one
/// memset of the array). The pin's loop pass manager deletes the inner loop the
/// idiom made dead before it visits the parent, which then holds a memset in a
/// loop and promotes it (`processLoopMemSet`). At speed on the 486 the nest's
/// row is inline dword stores against `rep stosd`'s 4 a dword: 144 clocks for
/// the nest and 145 for the array, 36 bytes fewer, which pays at 1 clock = 2 B.
/// Not measured on matmul nib -O2 since the loop passes see the intrinsic they
/// declared (#6534): the nest is one memset.
#[test]
fn a_nest_of_zero_stores_is_one_memset_once_the_inner_loop_is_deleted() {
    let after = through_the_loop_passes(ZEROED_NEST);
    assert!(after.contains("i8 0, i16 128"), "{after}");
}

/// An inner loop that computes something the function returns is not dead: it
/// stays, and so does the outer loop.
#[test]
fn a_nest_whose_inner_loop_is_live_stays_a_nest() {
    let after = through_the_loop_passes(LIVE_NEST);
    assert!(after.contains("phi"), "{after}");
    assert!(!after.contains("ret i16 poison"), "{after}");
}

/// lru at m32 -O1: a fill of fewer bytes and more clocks than its loop (-32 B
/// for +21 clocks) was taken. A target that prices its fills and has no rate
/// keeps the loop unless neither bytes nor clocks grow.
#[test]
fn a_priced_fill_with_no_rate_is_taken_only_if_neither_metric_grows() {
    let after = |fill_clocks: i64, sized: bool| {
        let clocks = crate::profit::OperationCosts {
            fill: fill_clocks,
            fill_cell: 0,
            store: 1,
            store_immediate: 1,
            add: 1,
            branch: 3,
            ..Default::default()
        };
        let bytes = crate::profit::OperationCosts {
            fill: 8,
            fill_cell: 0,
            store: 6,
            store_immediate: 6,
            add: 4,
            branch: 2,
            ..Default::default()
        };
        let machine = crate::testing::Tuned {
            costs: clocks,
            sizes: bytes,
            price_fills: true,
            rep_fills: true,
            ..Default::default()
        };
        let mut module = parsed(&format!("target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n{TWO_BYTE_ARRAYS}"));
        let mut pass = crate::loopidiom::pass::LoopIdiom::new();
        pass.for_size = sized;
        crate::testing::managed_on(&mut module, pass, machine).matches("call void @llvm.memset").count()
    };
    assert_eq!(after(30, true), 0, "the fills take more clocks than the loop: the loop stays");
    assert_eq!(after(2, true), 2, "fewer bytes and fewer clocks: both are fills");
}

/// A loop that holds only the memset its inner loop became was priced as a
/// fill of nothing (no stores, no fills counted), so it always paid: matmul nib
/// -O2 took 145 clocks of `rep stosd` over 136 of the loop it replaced. The
/// memset is the one fill the loop makes.
#[test]
fn the_memset_a_loop_promotes_is_priced_as_one_fill() {
    let after = |fill_clocks: i64| {
        let clocks = crate::profit::OperationCosts {
            fill: fill_clocks,
            fill_cell: 0,
            store: 1,
            store_immediate: 1,
            add: 1,
            branch: 3,
            ..Default::default()
        };
        let bytes = crate::profit::OperationCosts {
            fill: 8,
            fill_cell: 0,
            store: 2,
            store_immediate: 2,
            add: 4,
            branch: 2,
            ..Default::default()
        };
        let machine = crate::testing::Tuned {
            costs: clocks,
            sizes: bytes,
            price_fills: true,
            rep_fills: true,
            ..Default::default()
        };
        let mut module = parsed(&format!("{DOS}{ZEROED_NEST}"));
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.add(crate::loopidiom::pass::LoopIdiom::new());
        manager.add(crate::loopidiom::pass::AfterDeletion {
            deletion: crate::loopdeletion::LoopDeletion,
            idiom: crate::loopidiom::pass::LoopIdiom::new(),
        });
        manager.run_module(&mut module, std::rc::Rc::new(machine)).unwrap();
        printed(&module)
    };
    assert!(after(1).contains("i8 0, i16 128"), "a cheap fill merges the nest");
    let dear = after(100);
    assert!(!dear.contains("i8 0, i16 128"), "a fill dearer than its loop stays out\n{dear}");
}

/// matmul nib -O2: the row-zeroing nest priced its trip at two branches (a
/// conditional and the jump back) where the code has one `jne`, so its loop
/// looked 40 clocks dearer than it ran and a `rep stosd` of 145 clocks replaced
/// a 96-clock loop (+289 clocks for -47 bytes). A jump to the next block is
/// free in a trip.
#[test]
fn a_loop_trip_is_priced_at_its_conditional_branch_only() {
    let merges = |fill_clocks: i64| {
        let clocks = crate::profit::OperationCosts {
            fill: fill_clocks,
            fill_cell: 0,
            store: 1,
            store_immediate: 1,
            add: 1,
            branch: 3,
            ..Default::default()
        };
        let bytes = crate::profit::OperationCosts {
            fill: 8,
            fill_cell: 0,
            store: 2,
            store_immediate: 2,
            add: 4,
            branch: 2,
            ..Default::default()
        };
        let machine = crate::testing::Tuned { costs: clocks, sizes: bytes, price_fills: true, ..Default::default() };
        let mut module = parsed(&format!("{DOS}{ZEROED_NEST}"));
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.add(crate::loopidiom::pass::LoopIdiom::new());
        manager.add(crate::loopidiom::pass::AfterDeletion {
            deletion: crate::loopdeletion::LoopDeletion,
            idiom: crate::loopidiom::pass::LoopIdiom::new(),
        });
        manager.run_module(&mut module, std::rc::Rc::new(machine)).unwrap();
        printed(&module).contains("i8 0, i16 128")
    };
    // The loop is 8 trips of 13 clocks (the branch 3, the step 1, the row's
    // memset and its address).
    assert!(merges(60), "a fill under the loop's clocks is taken");
    assert!(!merges(120), "a fill over the loop's clocks is not: the loop ran 104");
}

/// A constant fill was priced at the register store's bytes; its immediate
/// stores encode longer, so lru -Os m32 took nine stores for a seven-trip loop.
#[test]
fn inline_fill_stores_are_priced_as_immediate_stores() {
    let prices = crate::profit::OperationCosts { store: 6, store_immediate: 10, prefix: 1, ..Default::default() };
    let fill = super::InlineFills { max_stores: 8, bytes: 8, prefixed: true };
    assert_eq!(fill.price(&prices), 2 * (10 + 1));
}

/// Flat code stores a dword with no operand-size prefix: the inline fill of
/// 64 bytes was priced 16 x (store + prefix), +60% of its clocks, which the
/// clock gate read as a loop that costs more than `rep stos` (fill_nest -O2
/// at x86_32-dos priced 42 clocks a trip for 26).
#[test]
fn a_flat_dword_store_is_priced_without_the_prefix() {
    let prices = crate::profit::OperationCosts { store_immediate: 1, prefix: 1, ..Default::default() };
    let flat = super::InlineFills { max_stores: 16, bytes: 64, prefixed: false };
    let sixteen_bit = super::InlineFills { prefixed: true, ..flat };
    assert_eq!((flat.price(&prices), sixteen_bit.price(&prices)), (16, 32));
}

/// A loop of a known count that zeroes one array and numbers another.
fn mixed_loop() -> String {
    format!(
        "@zero = global [64 x i16] zeroinitializer
@seq = global [64 x i16] zeroinitializer

{MEMSET}define i16 @f(i16 %q) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, 6
  br i1 %c, label %b2, label %b3

b2:
  %p = getelementptr inbounds [64 x i16], ptr @zero, i16 0, i16 %i
  store i16 0, ptr %p
  %s = getelementptr inbounds [64 x i16], ptr @seq, i16 0, i16 %i
  store i16 %i, ptr %s
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = getelementptr [64 x i16], ptr @seq, i16 0, i16 %q
  %v = load i16, ptr %r
  %z = getelementptr [64 x i16], ptr @zero, i16 0, i16 %q
  %w = load i16, ptr %z
  %x = add i16 %v, %w
  ret i16 %x
}}
"
    )
}

/// The same loop reading the cells it zeroes.
fn reading_loop() -> String {
    format!(
        "@zero = global [64 x i16] zeroinitializer

{MEMSET}define i16 @f(i16 %q) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %a = phi i16 [ 0, %b0 ], [ %b, %b2 ]
  %c = icmp slt i16 %i, 6
  br i1 %c, label %b2, label %b3

b2:
  %p = getelementptr inbounds [64 x i16], ptr @zero, i16 0, i16 %i
  store i16 0, ptr %p
  %t = getelementptr inbounds [64 x i16], ptr @zero, i16 0, i16 %q
  %l = load i16, ptr %t
  %b = add i16 %a, %l
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %a
}}
"
    )
}

/// `fill`, on a machine that prices memsets in `memset_store` bytes of store.
fn fill_priced(
    memset_store: i64,
    text: &str,
    inputs: &[&[i128]],
) -> (String, bool) {
    let clocks = crate::profit::OperationCosts { store: 1, fill: 10, fill_cell: 1, memset_store, ..Default::default() };
    let bytes =
        crate::profit::OperationCosts { store: 6, store_immediate: 6, fill: 20, fill_cell: 1, ..Default::default() };
    let machine = std::rc::Rc::new(crate::testing::Tuned { costs: clocks, sizes: bytes, ..Default::default() });
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, Some(machine)));
    let callees = llrm_mir::memory::callees(&module);
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = filled(context, &layout, &callees, function, &outer, &mut declared, false);
    let after = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (after, changed)
}

/// Where the loop stays rolled, a memset replaces one store: x86-16 nbody_fixed
/// -O2 took two `rep stosd` setups for two stores and grew 7 bytes.
#[test]
fn a_fill_that_would_not_shrink_the_loop_that_stays_is_kept() {
    let (after, changed) = fill_priced(0, &mixed_loop(), &[&[0], &[3], &[5], &[9]]);
    assert!(!changed && !after.contains("call void @llvm.memset"), "{after}");
}

/// Where a few trips are unrolled and a memset is wide stores, it replaces
/// them all.
#[test]
fn a_fill_of_a_target_that_unrolls_its_trips_is_a_memset_ahead_of_the_loop() {
    let (after, changed) = fill_priced(16, &mixed_loop(), &[&[0], &[3], &[5], &[9]]);
    assert!(
        changed && after.contains("call void @llvm.memset.p0.i16(ptr align 2 @zero, i8 0, i16 12, i1 false)"),
        "{after}"
    );
}

/// The fill and the other access share an object: hoisting the memset would
/// change what the loop's load reads.
#[test]
fn a_fill_in_a_loop_that_reads_its_object_is_kept() {
    let (after, changed) = fill_priced(16, &reading_loop(), &[&[0], &[3], &[5]]);
    assert!(!changed, "{after}");
}

/// A loop that keeps a computed store beside a constant one was never filled
/// under a rate: the fill's own clocks were not weighed against the removed
/// stores', so a 1200-trip `dst[i] = 0xEE` stayed in the loop beside
/// `src[i] = ...` (memory_expansion `reset`, 13M T over the program).
#[test]
fn a_constant_store_beside_a_computed_one_is_filled_when_the_trips_pay() {
    let sheet = llrm_mir::target::OperationCosts { store_immediate: 1, fill: 1, fill_cell: 1, ..Default::default() };
    let priced = |trips| Some((&sheet, 3000, Some(trips), Some((trips * 2 + 3) / 4)));
    assert!(super::fill_beside_computed_stores_pays(true, priced(1200), 2, 1, 1));
    assert!(!super::fill_beside_computed_stores_pays(true, priced(1), 2, 1, 1));
    assert!(!super::fill_beside_computed_stores_pays(false, priced(1200), 2, 1, 1));
    assert!(!super::fill_beside_computed_stores_pays(true, None, 2, 1, 1));
}

/// The cells were those of every store in the loop: with the T-state sheet (a
/// store 38 T, a fill 47 T and 84 T a dword cell) a computed and a constant
/// store over 1200 trips priced 600 cells for the 300 the fill stores, 50k T
/// against the 45.6k the removed store runs: the fill was refused
/// (`fill_beside_computed`, red on release/next).
#[test]
fn the_fill_beside_a_computed_store_is_priced_for_the_cells_it_stores() {
    let sheet = llrm_mir::target::OperationCosts { store_immediate: 38, fill: 47, fill_cell: 84, ..Default::default() };
    let cells = (1200 * 2 + 3) / 4;
    assert!(super::fill_beside_computed_stores_pays(true, Some((&sheet, 3000, Some(1200), Some(cells))), 2, 1, 1));
    // a fill of the same cells for a store that costs less than the fill's own
    // does not pay
    let cheap = llrm_mir::target::OperationCosts { store_immediate: 10, ..sheet };
    assert!(!super::fill_beside_computed_stores_pays(true, Some((&cheap, 3000, Some(1200), Some(cells))), 2, 1, 1));
}

/// A function of `loops` sequential guarded loops that each store the same byte
/// to a masked cell, so `loopidioms` proves the trips of each and rewrites none
/// (the mask is no stride): the body keeps one state through all of them.
fn counter_loops(loops: usize) -> String {
    let mut text = format!(
        "@buf = global [64 x i8] zeroinitializer\n\n{MEMSET}define i16 @f(i16 %n) {{\nb0:\n  %b = trunc i16 %n to i8\n  br label %g0\n\n"
    );
    for k in 0..loops {
        let next = if k + 1 == loops { "done".to_owned() } else { format!("g{}", k + 1) };
        text += &format!(
            "g{k}:\n  %go{k} = icmp slt i16 0, %n\n  br i1 %go{k}, label %pre{k}, label %x{k}\n\n\
             pre{k}:\n  br label %l{k}\n\n\
             l{k}:\n  %i{k} = phi i16 [ 0, %pre{k} ], [ %n{k}, %l{k} ]\n  %m{k} = and i16 %i{k}, 63\n  %p{k} = getelementptr [64 x i8], ptr @buf, i16 0, i16 %m{k}\n  store i8 %b, ptr %p{k}\n  %n{k} = add i16 %i{k}, 1\n  %c{k} = icmp slt i16 %n{k}, %n\n  br i1 %c{k}, label %l{k}, label %x{k}\n\n\
             x{k}:\n  br label %{next}\n\n",
        );
    }
    text + "done:\n  ret i16 %n\n}\n"
}

/// `loopidioms` asked for the body's `llvm.assume`s once for each loop and each
/// idiom it tried: 80 sequential loops cost 160 whole-body searches, 60% of the
/// pass at 80 loops (callgrind, 2026-11-13). It is one search for a body whose
/// state does not change.
#[test]
fn test_a_bodys_assumptions_are_searched_once_for_all_its_loops() {
    let before = llrm_analysis::assumptions::built();
    let (after, changed) = fill(&counter_loops(6), &[&[0], &[3], &[40]]);
    let searched = llrm_analysis::assumptions::built() - before;
    assert!(!changed, "{after}");
    assert!(searched <= 1, "{searched} searches for the assumptions of one body of 6 loops");
}
