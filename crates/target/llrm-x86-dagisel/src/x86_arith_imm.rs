//! `ISD::ADD`/`SUB`/`AND`/`OR`/`XOR` with an immediate that other nodes share
//! ([L5836][case]): at `-Os` the register form, so the constant is loaded once.
//!
//! # How it works
//! The pin selects these nodes to `rr` (or `rm`, folding the first operand's
//! load) directly when `shouldAvoidImmediateInstFormsForSize` says the
//! constant has more than one use, instead of letting the table fold it as
//! `ri`. The table's own `imm_su` predicate never gets the chance: the
//! patterns it guards are unavailable to FastISel, so the pin made this case.
//! `ADD` of 1 or -1 is left to `INC`/`DEC`. The APX `_ND` forms are not
//! translated. In the pin `AND`/`OR`/`XOR` first try `tryShrinkShlLogicImm`,
//! and `AND` `shrinkAndImmediate` and `matchBEXTRFromAndImm`: none is
//! translated (SD301), so this case sees those nodes earlier than the pin's
//! does.
//!
//! [case]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L5836
//!
//! # Used by
//! [`crate::x86_select::select_node`].

use llrm_dagisel::addr::AddrDag;
use llrm_dagisel::dag::{PatternContext, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt, x86};

use crate::x86_hooks::{Cpu, X86Hooks};
use crate::x86_predicates::X86Predicates;

/// The register and the load-folding opcodes of `opcode` on `nvt`.
fn forms(
    opcode: u16,
    nvt: llrm_dagisel::matcher::Vt,
) -> Option<(u16, u16)> {
    Some(match (nvt, opcode) {
        (mvt::i8, isd::ADD) => (x86::ADD8rr, x86::ADD8rm),
        (mvt::i8, isd::SUB) => (x86::SUB8rr, x86::SUB8rm),
        (mvt::i8, isd::AND) => (x86::AND8rr, x86::AND8rm),
        (mvt::i8, isd::OR) => (x86::OR8rr, x86::OR8rm),
        (mvt::i8, isd::XOR) => (x86::XOR8rr, x86::XOR8rm),
        (mvt::i16, isd::ADD) => (x86::ADD16rr, x86::ADD16rm),
        (mvt::i16, isd::SUB) => (x86::SUB16rr, x86::SUB16rm),
        (mvt::i16, isd::AND) => (x86::AND16rr, x86::AND16rm),
        (mvt::i16, isd::OR) => (x86::OR16rr, x86::OR16rm),
        (mvt::i16, isd::XOR) => (x86::XOR16rr, x86::XOR16rm),
        (mvt::i32, isd::ADD) => (x86::ADD32rr, x86::ADD32rm),
        (mvt::i32, isd::SUB) => (x86::SUB32rr, x86::SUB32rm),
        (mvt::i32, isd::AND) => (x86::AND32rr, x86::AND32rm),
        (mvt::i32, isd::OR) => (x86::OR32rr, x86::OR32rm),
        (mvt::i32, isd::XOR) => (x86::XOR32rr, x86::XOR32rm),
        (mvt::i64, isd::ADD) => (x86::ADD64rr, x86::ADD64rm),
        (mvt::i64, isd::SUB) => (x86::SUB64rr, x86::SUB64rm),
        (mvt::i64, isd::AND) => (x86::AND64rr, x86::AND64rm),
        (mvt::i64, isd::OR) => (x86::OR64rr, x86::OR64rm),
        (mvt::i64, isd::XOR) => (x86::XOR64rr, x86::XOR64rm),
        _ => return None,
    })
}

/// Selects `node` to its register form when its constant operand is worth
/// keeping in a register; false leaves it to the table.
pub fn select_arith_register_form<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    // `CurDAG->shouldOptForSize()`. Not at m16 (see `should_opt_for_size`): the
    // pin has no 16-bit target and its 32-bit size model does not describe
    // one.
    if !hooks.should_opt_for_size() {
        return false;
    }
    let nvt = dag.value_type(SDValue::new(node, 0));
    let opcode = dag.opcode(node);
    let Some((reg_op, mem_op)) = forms(opcode, nvt) else { return false };
    if nvt == mvt::i64 && hooks.options.cpu != Cpu::X64 {
        return false;
    }
    let (n0, n1) = (dag.operand(node, 0), dag.operand(node, 1));
    if dag.opcode(n1.node) != isd::Constant {
        return false;
    }
    // `isInt<8>(Val) || isInt<32>(Val)`: the second covers the first.
    let Some(value) = dag.constant(n1).filter(|v| i32::try_from(*v).is_ok()) else { return false };
    if opcode == isd::ADD && (value == 1 || value == -1) {
        return false;
    }
    if !hooks.should_avoid_immediate_inst_forms_for_size(dag, n1.node) {
        return false;
    }
    // A subtract's constant is the second operand: the load cannot fold.
    if opcode != isd::SUB {
        if let Some(a) = hooks.try_fold_load(dag, node, node, n0) {
            let chain = dag.operand(n0.node, 0);
            let ops = [n1, a.base, a.scale, a.index, a.disp, a.segment, chain];
            let folded = dag.machine_node(mem_op, &[nvt, mvt::i32, mvt::Other], &ops);
            dag.replace_uses(SDValue::new(n0.node, 1), SDValue::new(folded, 2));
            if let Some(mems) = dag.mem_operands(n0.node) {
                dag.set_node_mem_refs(folded, &mems);
            }
            dag.replace_uses(SDValue::new(node, 0), SDValue::new(folded, 0));
            dag.remove_dead_node(node);
            return true;
        }
    }
    dag.morph_node_to(node, reg_op, &[nvt, mvt::i32], &[n0, n1]);
    true
}
