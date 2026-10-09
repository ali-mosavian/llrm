//! `fill` had no tests of its own in llrm-core; these are behaviour tests,
//! each body MIR text run by llrm-mir's interpreter before and after.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::{Declared, Outer};

use super::{Fill, Merge, filled};
use crate::testing::{managed, parsed, printed, results};

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
    let before = parsed(&format!("{target}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, Fill { size: false });
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
        text.contains("  call void @llvm.memset.p0.i16(ptr %p, i8 65, i16 %1, i1 false)\n")
            && text.contains("  br label %b3\n\nb3:\n  %e = phi i16 [ 0, %b1 ], [ %2, %b2 ]\n"),
        "{text}"
    );
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
            text.contains(&format!("call void @llvm.memset.p0.i16(ptr %p, i8 {byte}, i16 20, i1 false)")),
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

/// A word whose bytes differ stayed a loop, `rep stosw` being isel's alone:
/// each of its trips cost a store. It is one pattern fill of the cells.
#[test]
fn a_word_of_two_bytes_is_one_pattern_fill() {
    let body = "  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 4660, ptr %p\n";
    let text = looped("i16", "10", body, "0");
    assert!(text.contains("store i16 4660"), "premise: a loop of word stores");
    let after = managed_fill(&text, &[&[0, 0], &[0, 9], &[0, 10]]);
    assert!(
        after.contains("call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr %p, i16 4660, i16 10, i1 false)"),
        "{after}"
    );
}

/// A loop-invariant word that is no constant is a pattern too, counted in
/// cells: a count of `%n` is not doubled.
#[test]
fn an_invariant_word_is_one_pattern_fill() {
    let body = "  %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i\n  store i16 %q, ptr %p\n";
    let after = managed_fill(&looped("i16", "%n", body, "%e"), &[&[0, 7], &[1, 7], &[40, 4660], &[-5, 3]]);
    assert!(
        after.contains("call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr %p, i16 %q, i16 %1, i1 false)"),
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
    let text = managed(&mut module, Fill { size: false });
    assert!(
        text.contains("declare void @llvm.memset.p0.i16(ptr")
            && text.contains("call void @llvm.memset.p0.i16(ptr %p, i8 65"),
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
    assert!(changed && text.contains("call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 %2, i1 false)"), "{text}");
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
    let text = managed(&mut module, Merge);
    assert!(
        text.contains("call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 8, i1 false)") && !text.contains("store"),
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
    let text = managed(&mut module, Merge);
    assert!(!text.contains("call void @llvm.memset"), "{text}");
}

/// A gap, another byte, a read between, or a store that overlaps keeps them.
#[test]
fn stores_that_are_not_one_fill_are_kept() {
    let unchanged = |stores: &str| {
        let before = parsed(&format!("{DOS}{}", local(stores)));
        let mut module = before.clone();
        let text = managed(&mut module, Merge);
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

/// A fill through a huge pointer reached isel as a count of 32 bits it
/// refused, failing a Nib build; `rep stos` would wrap at 64K besides. It
/// stays a loop, whose pointer carries.
#[test]
fn a_fill_through_a_huge_pointer_is_kept() {
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
    kept(text, &[&[0], &[1], &[300]]);
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
