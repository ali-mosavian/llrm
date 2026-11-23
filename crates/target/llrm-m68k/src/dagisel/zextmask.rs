// SPDX-License-Identifier: GPL-3.0-or-later
//! `zext (and x, C)` of a byte or a word as one `and.l`: the mask already
//! clears every bit the extension would.
//!
//! # How it works
//! [`select`] turns `zero_extend (and x, C)` (`x` an `i8` or `i16`, `C` inside
//! its width, the `and` read only by the extension) into `and.l #C` of the
//! register `x` lives in: `any_extend` is a copy (see [`super::anyext`]), and
//! the upper bits it leaves undefined are the ones the mask clears. `w[(i * 3)
//! & 127]` was `and.w #127,%d4; and.l #65535,%d4` (10 B, a copy before it);
//! it is `moveq #127,%d1; and.l %d0,%d1` (4 B).
//!
//! Translation: gcc's `andsi3` over a `zero_extend` (combine folds the
//! `(zero_extend:SI (and:HI x C))` into `(and:SI x C)` with the mask widened).
//! LLVM's `visitZERO_EXTEND` has the fold only for `(zext (and (trunc x), C))`:
//! a target with a legal `i16` or `i8` keeps the narrow `and`.
//!
//! DEVIATE M68K-ZEXT-MASK: the pin selects the narrow `and` and then the
//! extension. Reason: gcc -O2 `word_array_abs_x` is 24 B and 9% fewer clocks
//! than the pin's form. Owner m68k. Exit: none. Off with
//! [`super::anyext`] (`Options::PINNED`).

use llrm_codegen::backend::selectiondag::SDValue as Value;
use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::interp::Outcome;

use super::opcodes::code_of;

/// `TargetOpcode::COPY`, as in [`super::anyext`].
const COPY: u16 = 20;

/// `zero_extend (and x, C)` as `and.l #C, (copy x)`.
pub fn select(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
) -> Option<Outcome> {
    let here = Value { node, res: 0 };
    if adapter.dag.opcode(here) != ISD::ZeroExtend || adapter.dag.value_type(here) != EVT::I32 {
        return None;
    }
    let masked = Dag::operand(adapter, node, 0);
    let narrow = adapter.dag.value_type(Value { node: masked.node, res: 0 });
    let bits = match narrow {
        EVT::I8 => 8,
        EVT::I16 => 16,
        _ => return None,
    };
    if adapter.dag.opcode(Value { node: masked.node, res: 0 }) != ISD::And
        || !Dag::has_n_uses_of_value(adapter, SDValue { node: masked.node, res: 0 }, 1)
    {
        return None;
    }
    let (value, mask) = (Dag::operand(adapter, masked.node, 0), Dag::operand(adapter, masked.node, 1));
    let mask = Dag::constant(adapter, mask)? as u64 & ((1u64 << bits) - 1);
    let and = code_of("AND32di")?;
    let vt = adapter.value_type(SDValue { node, res: 0 });
    let wide = adapter.machine_node(COPY, &[vt], &[value]);
    let immediate = adapter.target_constant(mask as i64, vt);
    super::local::morph(adapter, node, and, &[vt], &[SDValue { node: wide, res: 0 }, immediate]);
    Some(Outcome::Selected)
}
