//! `any_extend` of a byte or a word is a register copy: the upper bits are
//! undefined, so no instruction is owed for them.
//!
//! # How it works
//! [`select`] morphs `any_extend` from `i8` or `i16` to a wider type into
//! `TargetOpcode::COPY` (the machine function sees a copy of the narrow
//! register into a wide one; where the two are one data register it is no
//! code at all). `and (any_extend x), 63` is then `moveq #63,%d1; and.l
//! %d0,%d1` (4 B), not `moveq #0,%d1; move.b %d0,%d1; moveq #63,%d2; and.l
//! %d2,%d1`.
//!
//! DEVIATE M68K-ANYEXT: the pin selects `(i32 (anyext i8))` as `MOVZXd32d8`
//! and `(i16 (anyext i8))` as its low word
//! (`M68kInstrData.td`, "i32 <- anyext i8"); x86 and ARM select it as
//! `INSERT_SUBREG (IMPLICIT_DEF), x, sub_8bit` (`X86InstrCompiler.td`, the
//! `anyext` patterns), which is a copy. Reason: gcc's `bench/tile` -Os is
//! 4 B smaller. Owner m68k. Exit: none. Off for `Options::PINNED`.

use llrm_codegen::backend::selectiondag::SDValue as Value;
use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::interp::Outcome;

/// `TargetOpcode::COPY`, which has no name in the pin's instruction table.
const COPY: u16 = 20;

/// `any_extend` of an `i8` or an `i16` as a copy.
pub fn select(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
) -> Option<Outcome> {
    let here = Value { node, res: 0 };
    if adapter.dag.opcode(here) != ISD::AnyExtend {
        return None;
    }
    let source = adapter.dag.operand(here, 0);
    let from = adapter.dag.value_type(source);
    let to = adapter.dag.value_type(here);
    if !matches!(
        (from, to),
        (EVT::I8, EVT::I16 | EVT::I32) | (EVT::I16, EVT::I32)
    ) {
        return None;
    }
    let value = Dag::operand(adapter, node, 0);
    let vt = adapter.value_type(SDValue { node, res: 0 });
    super::local::morph(adapter, node, COPY, &[vt], &[value]);
    Some(Outcome::Selected)
}
