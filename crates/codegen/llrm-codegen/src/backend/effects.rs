//! The registers and flags an instruction reads and writes, from its
//! `x86.instr` row.
//!
//! # How it works
//! The target's register file carries one row per instruction form
//! (`Info::effects`): operand roles, implicit reads and writes, and the
//! flags read and written. [`served`] looks `what` up for code of
//! `bits` bits and returns one of three answers.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   sem["<b>Semantics</b>"]:::src
//!   none["<b>None</b><br/><small>no row: caller decodes<br/>the bytes</small>"]:::mid
//!   unk["<b>Some(Unknown)</b><br/><small>transfer of control: the<br/>contract's effects apply</small>"]:::mid
//!   kn["<b>Some(Known(e))</b><br/><small>e.reads, e.writes,<br/>e.maybe_writes,<br/>e.flags_read, e.flags_written</small>"]:::mid
//!   sem -->|"served"| none
//!   sem -->|"served"| unk
//!   sem -->|"served"| kn
//! ```
//!
//! *What a served Semantics row says*
//!
//! `maybe_writes` holds what is written only if the instruction runs, such
//! as a repeated string operation's pointers. The table itself lives in
//! `llrm_x86::effects`; this module picks the rows of the register file
//! asked, since two targets compiled in one process have different rows
//! (real mode has `les`, a flat target has none).
//!
//! # Example
//! No honest example: the module answers one lookup and rewrites nothing, so
//! there is no LIR before and after. Its test
//! `the_rows_are_those_of_the_register_file_asked` shows the answer for a
//! real-mode `les` and a flat target.
//!
//! # Provides
//! The effects of one instruction as register roots and flag masks.
//!
//! # Used by
//! `peephole.rs` (`_effects_by_table`), which adds the instruction's
//! `requires` and `delivers` and turns the registers into byte lanes. The
//! lanes feed `machinecp`, `copysink`, `regthrash`, `exactaddress`, `liveness`,
//! `parcopy`, `upperzero` and other post-allocation passes.

use llrm_lir::registers::Regs;
pub use llrm_x86::effects::{Effects, Served};

use crate::model::ir::Semantics;

/// What the table says of `what`, or None where it has no row (the instruction
/// is then decoded).
pub fn served(
    regs: Regs,
    bits: u32,
    what: &Semantics,
) -> Option<Served> {
    llrm_x86::effects::effects(&regs.effects, bits, what)
}

#[cfg(test)]
mod tests {
    use llrm_lir::registers::Regs;

    use super::*;
    use crate::model::ir::{Held, Loc, Mem, Operation};

    /// Compiles for two targets, taken turns in one thread, each ask the rows
    /// of their own register file: real mode has `les` and a flat target has
    /// none. Rows by code width kept in a registry answered by whichever
    /// target registered first.
    #[test]
    fn the_rows_are_those_of_the_register_file_asked() {
        let load = Semantics {
            name: Some("les".into()),
            dests: vec![Loc::Held(Held { value: 1, width: 2 }), Loc::Held(Held { value: 2, width: 2 })],
            sources: vec![Loc::Mem(Mem::new(None, 4))],
            ..Semantics::new(Operation::Move)
        };
        for (info, bits, known) in [
            (&llrm_x86_m16::REGISTER_INFO, 16, true),
            (&llrm_x86_m32::REGISTER_INFO, 32, false),
            (&llrm_x86_m16::REGISTER_INFO, 16, true),
            (&llrm_x86_m32::REGISTER_INFO, 32, false),
        ] {
            assert_eq!(served(Regs(info), bits, &load).is_some(), known, "{bits} bits");
        }
    }
    /// `mov edi, _a+4[v*4]` before registers are chosen: the effect was
    /// unknown (the index names no register to encode), so MachineSink kept
    /// every flag-setting instruction above such a load in its block
    /// (x_bubble: the inner loop's `inc` stayed in the header).
    #[test]
    fn test_a_load_of_a_symbol_indexed_by_an_unplaced_value_has_known_effects() {
        let cell = Mem {
            index: Some(Held { value: 2, width: 4 }),
            scale: 4,
            ..Mem::new(Some(crate::model::ir::Addr::new(crate::model::ir::Space::Segment, 4)), 4)
        };
        let load = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(Held { value: 1, width: 4 })],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::Move)
        };
        let found = served(Regs(&llrm_x86_m32::REGISTER_INFO), 32, &load);
        assert!(matches!(found, Some(llrm_x86::effects::Served::Known(_))), "{found:?}");
    }

    /// `movsx edi, byte ptr [ebp-1024+v]` with `v` unplaced: the effect was
    /// unknown (the index names no register to encode), so every local-array
    /// access cut the machine scheduler's region (matmul, shellsort: 9
    /// instructions over 71 programs).
    #[test]
    fn test_a_frame_cell_indexed_by_an_unplaced_value_has_known_effects() {
        let cell = Mem {
            index: Some(Held { value: 2, width: 4 }),
            through: llrm_lir::registers::RegId::EBP,
            scale: 1,
            ..Mem::new(Some(crate::model::ir::Addr::new(crate::model::ir::Space::Frame, -1024)), 1)
        };
        let load = Semantics {
            name: Some("movsx".into()),
            dests: vec![Loc::Held(Held { value: 1, width: 4 })],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::Move)
        };
        let found = served(Regs(&llrm_x86_m32::REGISTER_INFO), 32, &load);
        assert!(matches!(found, Some(llrm_x86::effects::Served::Known(_))), "{found:?}");
    }
}
