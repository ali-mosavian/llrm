//! An address made in another block is made where it is used.

use super::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\n";

/// `text` through the pass on the real-mode target: the text after.
fn sunk(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// The store's block read `a[i]` through a `gep` made in the block before:
/// one register held across the edge, where the access takes the base and the
/// scaled index itself. The pin's `optimizeMemoryInst` makes the address in the
/// store's block.
#[test]
fn test_an_address_made_in_another_block_is_made_where_it_is_used() {
    let after = sunk(
        "define void @f(ptr %a, i16 %i) {
b0:
  %p = getelementptr i32, ptr %a, i16 %i
  br label %b1

b1:
  store i32 0, ptr %p
  ret void
}
",
    );
    let (entry, rest) = after.split_once("b1:").expect("a second block");
    assert!(!entry.contains("getelementptr"), "the address is not made in b0: {after}");
    assert!(rest.contains("mul i16 %i, 4") && rest.contains("getelementptr i8, ptr %a"), "{after}");
}

/// An address made in the access's block stays as it is.
#[test]
fn test_an_address_made_in_the_accesss_block_is_left_alone() {
    let text = "define void @f(ptr %a, i16 %i) {
b0:
  %p = getelementptr i32, ptr %a, i16 %i
  store i32 0, ptr %p
  ret void
}
";
    assert_eq!(sunk(text), printed(&parsed(&format!("{HEAD}{text}"))));
}

/// A constant offset behind a variable index is one `gep` of the constant.
#[test]
fn test_a_field_of_an_element_is_a_scaled_index_and_a_constant() {
    let after = sunk(
        "define void @f(ptr %a, i16 %i) {
b0:
  %e = getelementptr [4 x i16], ptr %a, i16 %i
  %p = getelementptr [4 x i16], ptr %e, i16 0, i16 3
  br label %b1

b1:
  store i16 0, ptr %p
  ret void
}
",
    );
    let (_, rest) = after.split_once("b1:").expect("a second block");
    assert!(rest.contains("mul i16 %i, 8") && rest.contains("i16 6"), "{after}");
}

/// `p = c ? &a[1] : &a[3]` made in the block before the access read two
/// addresses held across the edge; the offsets are selected and the access adds
/// the one to the base.
#[test]
fn test_a_select_of_two_offsets_of_one_base_is_one_select_and_a_gep() {
    let after = sunk(
        "define void @f(ptr %a, i1 %c) {
b0:
  %x = getelementptr i16, ptr %a, i16 1
  %y = getelementptr i16, ptr %a, i16 3
  %p = select i1 %c, ptr %x, ptr %y
  br label %b1

b1:
  store i16 0, ptr %p
  ret void
}
",
    );
    let (entry, rest) = after.split_once("b1:").expect("a second block");
    assert!(!entry.contains("getelementptr"), "the arms' addresses are not made in b0: {after}");
    assert!(rest.contains("select i1 %c, i16 2, i16 6") && rest.contains("getelementptr i8, ptr %a"), "{after}");
}

/// `optimizeExtUses`: a source read in a later block while its extension is too
/// is read there through a `trunc` of the extension, so the narrow value need
/// not be held across the edge beside the wide one.
#[test]
fn test_a_source_read_beside_its_live_out_extension_is_a_trunc_of_it() {
    let after = sunk(
        "define i32 @f(i16 %a) {
b0:
  %s = add i16 %a, 1
  %e = sext i16 %s to i32
  br label %b1

b1:
  %t = add i16 %s, 2
  %v = zext i16 %t to i32
  %w = add i32 %e, %v
  ret i32 %w
}
",
    );
    let (entry, rest) = after.split_once("b1:").expect("a second block");
    assert!(!entry.contains("trunc"), "{after}");
    assert!(rest.contains("trunc i32 %e to i16"), "{after}");
    assert!(!rest.contains("add i16 %s, 2"), "the source is no longer read in b1: {after}");
}

/// A load reading the source in another block keeps it (the pin: "we don't
/// want this xform to end up introducing reloads just before load / store").
#[test]
fn test_a_source_read_by_a_store_elsewhere_is_left_alone() {
    let text = "define i32 @f(i16 %a, ptr %p) {
b0:
  %s = add i16 %a, 1
  %e = sext i16 %s to i32
  br label %b1

b1:
  store i16 %s, ptr %p
  ret i32 %e
}
";
    assert_eq!(sunk(text), printed(&parsed(&format!("{HEAD}{text}"))));
}

/// CodeGenPrepare's `optimizeInst` simplifies every phi (`simplifyInstruction`,
/// L8947), not only the ones read as addresses: a loop's closing phi read by
/// an add stayed to isel, which copied it (`LoopOpened`'s AM-LO-1: +10.9 M Ir
/// and 260 objects moving when that llrm-only pass is off).
#[test]
fn test_a_phi_of_one_value_is_that_value_whatever_reads_it() {
    let after = sunk(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %n
  br i1 %done, label %b2, label %b1

b2:
  %next.lcssa = phi i16 [ %next, %b1 ]
  %read = add i16 %next.lcssa, 7
  ret i16 %read
}
",
    );
    assert!(!after.contains("lcssa"), "{after}");
    assert!(after.contains("add i16 %next, 7"), "{after}");
}

/// The flat 32-bit target of x86: the pin's TTI with scaled indexes free.
pub(crate) struct Flat;

impl llrm_mir::target::Machine for Flat {
    fn foreign_span(
        &self,
        _: (i64, i64),
        _: (i64, i64),
        _: i64,
    ) -> Option<(i64, i64)> {
        None
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::Neutral.costs()
    }

    fn registers(&self) -> i64 {
        llrm_mir::target::Neutral.registers()
    }

    fn call_registers(&self) -> i64 {
        llrm_mir::target::Neutral.call_registers()
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }

    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        Some(std::rc::Rc::new(llrm_x86::tti::X86 {
            sse1: false,
            sse2: false,
            macro_fusion: false,
            quad: false,
            real_mode: None,
            tuning: llrm_mir::target::Tuning::PIN,
        }))
    }
}

fn sunk_flat(text: &str) -> String {
    let mut module = parsed(&format!("target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n\n{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(Flat)).unwrap();
    printed(&module)
}

const BUBBLE: &str = "@a = global [200 x i32] zeroinitializer

define void @f(i32 %n) {
b0:
  br label %b1

b1:
  %j = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  %s = shl i32 %j, 2
  %c = icmp sgt i32 %j, 3
  br i1 %c, label %b2, label %b3

b2:
  %next = add i32 %j, 1
  %p = getelementptr i8, ptr @a, i32 %s
  %q = getelementptr i8, ptr %p, i32 4
  store i32 %next, ptr %q
  br label %b1

b3:
  ret void
}
";

/// x_bubble's swap block stored through `@a + 4 + (j << 2)` with the shift made
/// in the loop header: the shifted index crossed the edge in a register and
/// spilled in the inner loop (clocks x1.59 under dag). The pin folds the shift
/// into the mode as the scale of `j`, which the store's block already reads, so
/// the address is made there from `j` and nothing but `j` crosses.
#[test]
fn test_a_shifted_index_is_the_scale_of_an_index_the_block_reads() {
    let after = sunk_flat(BUBBLE);
    let (_, rest) = after.split_once("b2:").expect("a block b2");
    let b2 = rest.split_once("b3:").expect("a block b3").0;
    assert!(!b2.contains("%s"), "the shifted index crosses the edge: {after}");
    assert!(b2.contains("%j"), "the store's block makes its address from j: {after}");
}

/// x_bubble again, the swap block reading none of the index (`i` is dead there
/// once `i1` is made): the stores took the header's `shl` from a register,
/// `lea ecx,[esi*4]; mov [ecx+_a],..; mov [ecx+_a+4],..` (+3702 instructions
/// against the pin, which folds `_a[esi*4]` in each). Every use of the `shl` is
/// an access that folds it, so the pin folds it into each
/// (`isProfitableToFoldInto AddressingMode`, the last case).
#[test]
fn test_a_shift_only_accesses_read_is_the_scale_where_the_index_is_dead() {
    let after = sunk_flat(
        "@a = global [200 x i32] zeroinitializer

define void @f(i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %i1, %b3 ]
  %s = shl i32 %i, 2
  %p = getelementptr i8, ptr @a, i32 %s
  %x = load i32, ptr %p
  %i1 = add i32 %i, 1
  %q = getelementptr i8, ptr %p, i32 4
  %y = load i32, ptr %q
  %c = icmp sgt i32 %x, %y
  br i1 %c, label %b2, label %b3

b2:
  %p2 = getelementptr i8, ptr @a, i32 %s
  store i32 %y, ptr %p2
  %q2 = getelementptr i8, ptr %p2, i32 4
  store i32 %x, ptr %q2
  br label %b3

b3:
  %done = icmp eq i32 %i1, %n
  br i1 %done, label %b4, label %b1

b4:
  ret void
}
",
    );
    let (_, rest) = after.split_once("b2:").expect("a block b2");
    let b2 = rest.split_once("b3:").expect("a block b3").0;
    assert!(!b2.contains("%s"), "the shifted index crosses the edge: {after}");
}

/// Real mode forms a scaled index with an address-size prefix and a stall of
/// its own (`scaled_index_insns`): folding the shift would pay them at each
/// access where the shift is made once (quicksort at m16 grew 21 bytes).
#[test]
fn test_a_shifted_index_stays_where_a_scaled_index_costs_instructions() {
    let after = sunk(&BUBBLE.replace("i32", "i16").replace("[200 x i16]", "[200 x i16]"));
    let (_, rest) = after.split_once("b2:").expect("a block b2");
    let b2 = rest.split_once("b3:").expect("a block b3").0;
    assert!(b2.contains("%s"), "the shifted index is folded in real mode: {after}");
}

/// `optimizeExtUses` rewrites every reader of the source outside the
/// extension's block, as the pin does. It once skipped a reader the extension's
/// block did not dominate (DEVIATE CGP2); the pin's `llc` deletes such a block
/// before this pass (an unreachable one is the only block the extension cannot
/// reach), so the guard was a difference on no pin input. Here the dead block's
/// reader is rewritten like any other.
#[test]
fn test_every_reader_of_an_extended_source_outside_its_block_reads_the_extension() {
    let after = sunk(
        "define i32 @f(i16 %a, i16 %b) {
b0:
  %s = add i16 %a, %b
  %e = zext i16 %s to i32
  br label %b1

b1:
  %u = add i16 %s, 5
  %ue = zext i16 %u to i32
  %r = add i32 %e, %ue
  ret i32 %r

dead:
  %d = add i16 %s, 7
  %de = zext i16 %d to i32
  ret i32 %de
}
",
    );
    let (live, dead) = after.split_once("dead:").expect("a dead block");
    assert!(live.contains("trunc i32 %e to i16"), "{after}");
    assert!(dead.contains("trunc i32 %e to i16"), "{after}");
}
