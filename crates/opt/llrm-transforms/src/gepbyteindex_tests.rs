//! The index of an `inbounds` `gep` into a small global is a byte.

use std::rc::Rc;

use llrm_mir::tti::{ArithOp, CastKind, CmpSel, Cost, CostKind, ImmUser, MemoryOp, OperandInfo, Ty};

use crate::gepbyteindex::GepByteIndex;
use crate::testing::{parsed, printed, results};

const HEAD: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-n8-S8\"\n\n";

/// A target with a byte index reach of 255, as the 6502.
struct Byte(llrm_mir::tti::Basic);

impl llrm_mir::tti::Tti for Byte {
    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        self.0.cmp_sel_cost(op, ty, kind)
    }

    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        stored_constant: bool,
    ) -> Cost {
        self.0.memory_op_cost(op, ty, kind, stored_constant)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost {
        self.0.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        imm: i128,
        bits: u32,
    ) -> Cost {
        self.0.int_imm_cost(imm, bits)
    }

    fn int_imm_cost_inst(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        self.0.int_imm_cost_inst(op, index, imm, bits)
    }

    fn is_truncate_free(
        &self,
        from: Ty,
        to: Ty,
    ) -> bool {
        self.0.is_truncate_free(from, to)
    }

    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        self.0.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale)
    }

    fn cast_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Cost {
        self.0.cast_cost(op, dst, src, kind)
    }

    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        first: OperandInfo,
        second: OperandInfo,
    ) -> Cost {
        self.0.arithmetic_cost(op, ty, kind, first, second)
    }

    fn address_index_max(&self) -> Option<u64> {
        Some(255)
    }
}

struct Machine(bool);

impl llrm_mir::target::Machine for Machine {
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

    fn tti(&self) -> Option<Rc<dyn llrm_mir::tti::Tti>> {
        let basic = llrm_mir::tti::Basic;
        Some(if self.0 { Rc::new(Byte(basic)) } else { Rc::new(basic) })
    }
}

/// `text` before and after the pass; `byte_reach`: the target sets
/// `address_index_max`.
fn run(
    text: &str,
    byte_reach: bool,
) -> (String, String) {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let before = printed(&module);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(GepByteIndex);
    manager.run_module(&mut module, Rc::new(Machine(byte_reach))).unwrap();
    (before, printed(&module))
}

/// sat_builtins' four array accesses (`la[i]`, element 4 bytes) were a 16-bit
/// `asl; rol` twice, a 16-bit add and `(zp),y`, 113 sites over the corpus: the
/// byte offset is `abs,x`.
#[test]
fn a_byte_index_scaled_by_its_element_is_a_byte_offset() {
    let ir = "@g = global [2 x [4 x i8]] [[4 x i8] [i8 1, i8 2, i8 3, i8 4], [4 x i8] [i8 5, i8 6, i8 7, i8 8]]
define i8 @f(i8 %x) {
  %z = zext i8 %x to i16
  %p = getelementptr inbounds [4 x i8], ptr @g, i16 %z
  %v = load i8, ptr %p
  ret i8 %v
}
";
    let (before, after) = run(ir, true);
    assert!(after.contains("mul nuw i8 %x, 4") && after.contains("getelementptr inbounds i8, ptr @g"), "{after}");
    assert_eq!(results(&parsed(&format!("{HEAD}{ir}")), &[&[0], &[1]]), results(&parsed(&after), &[&[0], &[1]]));
    assert_ne!(before, after);
}

/// The constant indices stay as a displacement from the byte-indexed address.
#[test]
fn the_constant_part_of_the_indices_is_a_gep_of_its_own() {
    let ir = "@g = global [2 x [4 x i8]] [[4 x i8] [i8 1, i8 2, i8 3, i8 4], [4 x i8] [i8 5, i8 6, i8 7, i8 8]]
define i8 @f(i8 %x) {
  %z = zext i8 %x to i16
  %p = getelementptr inbounds [2 x [4 x i8]], ptr @g, i16 0, i16 %z, i16 2
  %v = load i8, ptr %p
  ret i8 %v
}
";
    let (_, after) = run(ir, true);
    assert!(after.contains("mul nuw i8 %x, 4") && after.contains("i16 2"), "{after}");
    assert_eq!(results(&parsed(&format!("{HEAD}{ir}")), &[&[0], &[1]]), results(&parsed(&after), &[&[0], &[1]]));
}

/// What the rewrite must not touch: a target with no byte reach, a global
/// larger than it, a `gep` that is not `inbounds`, and a scale of 1.
#[test]
fn other_geps_are_left_alone() {
    let ir = |global: &str, gep: &str| {
        format!(
            "@g = global {global} zeroinitializer
define i8 @f(i8 %x) {{
  %z = zext i8 %x to i16
  %p = {gep}
  %v = load i8, ptr %p
  ret i8 %v
}}
"
        )
    };
    let scaled = "getelementptr inbounds [4 x i8], ptr @g, i16 %z";
    let cases = [
        ("no reach", ir("[2 x [4 x i8]]", scaled), false),
        ("too large", ir("[100 x [4 x i8]]", scaled), true),
        ("not inbounds", ir("[2 x [4 x i8]]", "getelementptr [4 x i8], ptr @g, i16 %z"), true),
        ("scale 1", ir("[8 x i8]", "getelementptr inbounds i8, ptr @g, i16 %z"), true),
    ];
    for (name, text, reach) in cases {
        let (before, after) = run(&text, reach);
        assert_eq!(before, after, "{name}");
    }
}
