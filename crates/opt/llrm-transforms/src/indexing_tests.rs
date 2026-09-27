//! The one decision a counter makes over its addresses, through `Strength`
//! on a target priced as a 486: native `[bp+si]` unscaled and free, the 67h
//! form scaled at a prefix a use.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_analysis::testing::DOS;
use llrm_mir::passes::PassManager;
use llrm_mir::target::AddressForm;

use crate::profit::OperationCosts;
use crate::strength::Strength;
use crate::testing::{Tuned, parsed, printed, results};

fn target(registers: i64) -> Tuned {
    let costs = OperationCosts { memory_update: 3, prefix: 1, extend: 3, ..Default::default() };
    let native = AddressForm { partners: Some(2), ..AddressForm::new(2, BTreeSet::from([1]), 0, 0, 0, false, None).unwrap() };
    let wide = AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 1, 1, 3, true, None).unwrap();
    Tuned { costs, registers, call_registers: registers, address_forms: vec![native, wide] }
}

/// `text` in the DOS layout through `Strength` on `target`, computing what
/// it did for each of `inputs`; printed.
fn reduced(text: &str, target: Tuned, inputs: &[&[i128]]) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut after = before.clone();
    let mut manager = PassManager::default();
    manager.add(Strength::default());
    manager.run_module(&mut after, Rc::new(target)).unwrap();
    let printed = printed(&after);
    assert_eq!(results(&after, inputs), results(&before, inputs), "{printed}");
    printed
}

/// Frame arrays of 4-, 2- and 1-byte elements, one counter, each updated
/// from the others: `strides.c`'s loop.
const STRIDES: &str = "define i32 @f(i16 %n) {
b0:
  %l = alloca [16 x i32]
  %w = alloca [16 x i16]
  %b = alloca [16 x i8]
  call void @llvm.memset.p0.i16(ptr %l, i8 1, i16 64, i1 false)
  call void @llvm.memset.p0.i16(ptr %w, i8 2, i16 32, i1 false)
  call void @llvm.memset.p0.i16(ptr %b, i8 3, i16 16, i1 false)
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %more = icmp ult i16 %i, 16
  br i1 %more, label %b2, label %b3

b2:
  %ml = mul nsw i16 %i, 4
  %pl = getelementptr inbounds i8, ptr %l, i16 %ml
  %vl = load i32, ptr %pl
  %mw = mul nsw i16 %i, 2
  %pw = getelementptr inbounds i8, ptr %w, i16 %mw
  %vw = load i16, ptr %pw
  %pb = getelementptr inbounds i8, ptr %b, i16 %i
  %vb = load i8, ptr %pb
  %xb = zext i8 %vb to i16
  %sum = add i16 %vw, %xb
  %xs = sext i16 %sum to i32
  %nl = add i32 %vl, %xs
  store i32 %nl, ptr %pl
  %tl = trunc i32 %nl to i16
  %nw = xor i16 %vw, %tl
  store i16 %nw, ptr %pw
  %tw = trunc i16 %nw to i8
  %nb = add i8 %vb, %tw
  store i8 %nb, ptr %pb
  %i.next = add i16 %i, 1
  br label %b1

b3:
  %rl = load i32, ptr %l
  %rw = load i16, ptr %w
  %rb = load i8, ptr %b
  %ew = sext i16 %rw to i32
  %eb = sext i8 %rb to i32
  %r1 = add i32 %rl, %ew
  %r = add i32 %r1, %eb
  ret i32 %r
}

declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
";

/// With no register to spare, one widened counter indexes every array at
/// its own scale: carried, each stride took a recurrence of its own and
/// two lived in the frame, loaded at every access and stepped there.
#[test]
fn test_a_crowded_loop_indexes_every_stride_by_one_wide_counter() {
    let printed = reduced(STRIDES, target(4), &[&[0], &[5]]);
    assert!(!printed.contains("lsr.iv"), "{printed}");
    for base in ["%l", "%w", "%b"] {
        assert!(printed.contains(&format!("getelementptr inbounds i8, ptr {base}, i32")), "{base}: {printed}");
    }
}

/// With registers to spare, each address is its own recurrence, an add a
/// trip where the 67h form costs a prefix at each access, and the counter
/// gives way to one of them.
#[test]
fn test_room_carries_every_address_and_widens_nothing() {
    let printed = reduced(STRIDES, target(40), &[&[0], &[5]]);
    assert!(printed.contains("lsr.iv"), "{printed}");
    assert!(!printed.contains("phi i32"), "{printed}");
}

/// A counter nothing but a byte address reads dies once that address is
/// carried and takes over the control: indexed, the counter was kept, and
/// deedlines' CYCLEBLOBS paid a compare of its spilled counter each trip.
#[test]
fn test_a_counter_only_an_address_reads_gives_way_to_its_pointer() {
    let text = "define i8 @f(i16 %n) {
b0:
  %b = alloca [16 x i8]
  call void @llvm.memset.p0.i16(ptr %b, i8 3, i16 16, i1 false)
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %more = icmp ult i16 %i, 16
  br i1 %more, label %b2, label %b3

b2:
  %pb = getelementptr inbounds i8, ptr %b, i16 %i
  %vb = load i8, ptr %pb
  %nb = add i8 %vb, 7
  store i8 %nb, ptr %pb
  %i.next = add i16 %i, 1
  br label %b1

b3:
  %r = load i8, ptr %b
  ret i8 %r
}

declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
";
    let printed = reduced(text, target(40), &[&[0]]);
    assert!(printed.contains("phi ptr"), "{printed}");
}

/// An outer counter's address read in the loop inside pays the form's
/// price every inner trip, and a recurrence only an add each outer one:
/// priced once a trip of the outer loop, nbody's `x[i]` took the 67h form
/// throughout its `j` loop.
#[test]
fn test_an_address_read_in_an_inner_loop_weighs_its_trips() {
    let text = "define i32 @f(i16 %n) {
b0:
  %l = alloca [16 x i32]
  call void @llvm.memset.p0.i16(ptr %l, i8 1, i16 64, i1 false)
  br label %b1

b1:
  %k = phi i16 [ 0, %b0 ], [ %k.next, %b4 ]
  %more = icmp ult i16 %k, 16
  br i1 %more, label %b2, label %b5

b2:
  %m = mul nsw i16 %k, 4
  %p = getelementptr inbounds i8, ptr %l, i16 %m
  br label %b3

b3:
  %j = phi i16 [ 0, %b2 ], [ %j.next, %b3 ]
  %v = load i32, ptr %p
  %x = zext i16 %j to i32
  %w = add i32 %v, %x
  store i32 %w, ptr %p
  %j.next = add i16 %j, 1
  %again = icmp ult i16 %j.next, %n
  br i1 %again, label %b3, label %b4

b4:
  %k.next = add i16 %k, 1
  br label %b1

b5:
  %r = load i32, ptr %l
  ret i32 %r
}

declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
";
    let printed = reduced(text, target(3), &[&[1], &[3]]);
    assert!(!printed.contains("phi i32"), "{printed}");
}
