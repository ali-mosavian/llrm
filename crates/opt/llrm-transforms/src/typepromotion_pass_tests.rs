//! A narrow counter of a loop is a register's width: no mask after each step.

use crate::testing::{parsed, printed};
use crate::typepromotion_pass::TypePromotion;

const HEAD: &str = "target datalayout = \"e-p:64:64-i64:64-n32:64\"\n\n";

/// arm64's `Lowering` where it matters here: 32 and 64 are the legal widths.
struct Wide;

impl llrm_mir::lowering::Lowering for Wide {
    fn is_type_legal(
        &self,
        bits: u32,
    ) -> bool {
        matches!(bits, 32 | 64)
    }

    fn runs_type_promotion(&self) -> bool {
        true
    }

    fn is_sext_cheaper_than_zext(
        &self,
        from_bits: u32,
        to_bits: u32,
    ) -> bool {
        from_bits == 32 && to_bits == 64
    }
}

/// `Basic`'s prices with a 64-bit scalar register.
struct WideTti;

impl llrm_mir::tti::Tti for WideTti {
    fn scalar_register_bits(&self) -> u32 {
        64
    }

    fn is_legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool {
        imm.unsigned_abs() < 4096
    }

    fn cmp_sel_cost(
        &self,
        op: llrm_mir::tti::CmpSel,
        ty: llrm_mir::tti::Ty,
        kind: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cmp_sel_cost(op, ty, kind)
    }

    fn memory_op_cost(
        &self,
        op: llrm_mir::tti::MemoryOp,
        ty: llrm_mir::tti::Ty,
        kind: llrm_mir::tti::CostKind,
        stored_constant: bool,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.memory_op_cost(op, ty, kind, stored_constant)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        imm: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.int_imm_cost(imm, bits)
    }

    fn int_imm_cost_inst(
        &self,
        op: llrm_mir::tti::ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.int_imm_cost_inst(op, index, imm, bits)
    }

    fn is_truncate_free(
        &self,
        from: llrm_mir::tti::Ty,
        to: llrm_mir::tti::Ty,
    ) -> bool {
        llrm_mir::tti::Basic.is_truncate_free(from, to)
    }

    fn is_legal_addressing_mode(
        &self,
        gv: bool,
        offset: i64,
        base: bool,
        scale: i64,
    ) -> bool {
        llrm_mir::tti::Basic.is_legal_addressing_mode(gv, offset, base, scale)
    }

    fn cast_cost(
        &self,
        op: llrm_mir::tti::CastKind,
        dst: llrm_mir::tti::Ty,
        src: llrm_mir::tti::Ty,
        kind: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cast_cost(op, dst, src, kind)
    }

    fn arithmetic_cost(
        &self,
        op: llrm_mir::tti::ArithOp,
        ty: llrm_mir::tti::Ty,
        kind: llrm_mir::tti::CostKind,
        first: llrm_mir::tti::OperandInfo,
        second: llrm_mir::tti::OperandInfo,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.arithmetic_cost(op, ty, kind, first, second)
    }
}

/// A target that is `Neutral` but for the `Lowering` and `Tti` above.
struct Arm64Like(bool);

impl llrm_mir::target::Machine for Arm64Like {
    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(selectors, offsets, width)
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
        self.0.then(|| std::rc::Rc::new(WideTti) as _)
    }

    fn lowering(&self) -> Option<std::rc::Rc<dyn llrm_mir::lowering::Lowering>> {
        self.0.then(|| std::rc::Rc::new(Wide) as _)
    }
}

/// `text` through the pass, on a target that promotes or one that does not.
fn promoted(
    text: &str,
    target_promotes: bool,
) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(TypePromotion);
    manager.run_module(&mut module, std::rc::Rc::new(Arm64Like(target_promotes))).unwrap();
    printed(&module)
}

/// crc16's inner counter: `add nuw` on an `i16` phi, compared `samesign ult`.
/// Selected as it stood it was `add w14, w13, #1; and w13, w13, #0xffff;
/// cmp w13, #7` (crc.1 ran 705 instructions for the pin's 506).
const COUNTER: &str = "define i64 @f(i64 %x) {
b0:
  br label %b1

b1:
  %c = phi i16 [ 0, %b0 ], [ %n, %b1 ]
  %v = phi i64 [ %x, %b0 ], [ %w, %b1 ]
  %w = lshr i64 %v, 1
  %n = add nuw nsw i16 %c, 1
  %k = icmp samesign ult i16 %c, 7
  br i1 %k, label %b1, label %b2

b2:
  ret i64 %w
}
";

#[test]
fn test_a_narrow_counter_with_no_wrap_is_a_register_wide() {
    let after = promoted(COUNTER, true);
    assert!(after.contains("%c = phi i32 [ 0, %b0 ], [ %n, %b1 ]"), "{after}");
    assert!(after.contains("%n = add nuw nsw i32 %c, 1"), "{after}");
    assert!(after.contains("%k = icmp samesign ult i32 %c, 7"), "{after}");
    assert!(!after.contains("i16"), "{after}");
}

/// A counter that may wrap is not promoted: `i32` would not wrap where `i16`
/// does.
#[test]
fn test_a_counter_that_may_wrap_is_left_alone() {
    let text = COUNTER.replace("add nuw nsw i16", "add i16");
    assert_eq!(promoted(&text, true), printed(&parsed(&format!("{HEAD}{text}"))));
}

/// A target whose `Lowering` does not run the pass keeps the narrow counter.
#[test]
fn test_a_target_that_does_not_run_the_pass_is_unchanged() {
    assert_eq!(promoted(COUNTER, false), printed(&parsed(&format!("{HEAD}{COUNTER}"))));
}

/// A store of the counter needs the `i16` back: a `trunc` before the store.
#[test]
fn test_a_store_of_a_promoted_counter_gets_a_trunc() {
    let text =
        COUNTER.replace("  %k = icmp", "  store i16 %c, ptr %p\n  %k = icmp").replace("(i64 %x)", "(i64 %x, ptr %p)");
    let after = promoted(&text, true);
    assert!(after.contains("%c = phi i32"), "{after}");
    let (_, rest) = after.split_once("trunc i32 %c to i16").unwrap_or_else(|| panic!("{after}"));
    assert!(rest.contains("store i16"), "{after}");
}
