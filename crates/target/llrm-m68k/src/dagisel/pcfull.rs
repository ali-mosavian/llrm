//! `lea (sym,%pc,%zd0)`: the address of a symbol in the full format, with a
//! 32-bit displacement ([`DataAccess::PcFull`]).
//!
//! DEVIATE: the pin selects `WrapperPC` of a symbol to `LEA32q`, `(d16,%pc)`
//! (`M68kInstrCompiler.td` L43-L47), which overflows past 32 KiB. This selects
//! `LEA32l`, the pin's `(bd32,%pc,Xn)` row, with `NoReg` as the index. Owner:
//! m68k-68020. Exit: [`CodeModel`](crate::compile::CodeModel)'s.

use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_dagisel::dag::Dag;
use llrm_dagisel::generated::ids::mvt;
use llrm_dagisel::interp::Outcome;

use super::generated::ids::isd as m68k_isd;
use super::indexed::code_of;

/// `WrapperPC` of a symbol as `LEA32l [symbol, NoReg, 1]`.
pub fn select(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
) -> Option<Outcome> {
    if Dag::opcode(adapter, node) != m68k_isd::WrapperPC {
        return None;
    }
    let symbol = Dag::operand(adapter, node, 0);
    let noreg = Dag::register(adapter, 0, mvt::i32);
    let scale = Dag::target_constant(adapter, 1, mvt::i8);
    Dag::morph_node_to(adapter, node, code_of("LEA32l")?, &[mvt::i32], &[symbol, noreg, scale]);
    Some(Outcome::Selected)
}
