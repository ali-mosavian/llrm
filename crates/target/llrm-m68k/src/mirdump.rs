//! `LLRM_DEBUG=mir` writes each function's machine code to stderr before
//! allocation and after the passes that follow it, as the pin's
//! `-print-before=greedy` / `-print-after=...` would.

use crate::mfunc::MFunction;
use crate::regalloc::Assignment;

/// One function at `stage`; `assignment` names the registers once there is one.
pub fn dump(
    stage: &str,
    function: &MFunction,
    assignment: Option<&Assignment>,
) {
    if llrm_support::debug::enabled("mir") {
        eprint!("{}", render(stage, function, assignment));
    }
}

/// The text [`dump`] writes.
pub fn render(
    stage: &str,
    function: &MFunction,
    assignment: Option<&Assignment>,
) -> String {
    let mut out = format!("# {stage}: @{}\n", function.name);
    for block in &function.blocks {
        out.push_str(&format!("bb.{}:\n", block.id));
        for phi in &block.phis {
            out.push_str(&format!("  {phi:?}\n"));
        }
        for inst in &block.insts {
            let operands: Vec<String> = inst
                .operands
                .iter()
                .map(|operand| match (operand, assignment) {
                    (llrm_spike_mf::Operand::Use(llrm_spike_mf::Reg::Virt(v)), Some(a)) => {
                        format!("Use({})", crate::REGISTER_INFO.name(a.0[*v as usize]).unwrap_or("?"))
                    }
                    (llrm_spike_mf::Operand::Def(llrm_spike_mf::Reg::Virt(v)), Some(a)) => {
                        format!("Def({})", crate::REGISTER_INFO.name(a.0[*v as usize]).unwrap_or("?"))
                    }
                    (llrm_spike_mf::Operand::Use(llrm_spike_mf::Reg::Phys(r)), _) => {
                        format!("Use({})", crate::REGISTER_INFO.name(*r).unwrap_or("?"))
                    }
                    (llrm_spike_mf::Operand::Def(llrm_spike_mf::Reg::Phys(r)), _) => {
                        format!("Def({})", crate::REGISTER_INFO.name(*r).unwrap_or("?"))
                    }
                    (other, _) => format!("{other:?}"),
                })
                .collect();
            out.push_str(&format!("  {:?} {}\n", inst.kind, operands.join(", ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use llrm_spike_mf::{Inst, Kind, Operand, Reg};

    use super::*;

    /// Before `LLRM_DEBUG=mir` there was no way to read m68k's machine code at
    /// all (`LLRM_MIR_PRINT` is refused for m68k), so the 222 copies of
    /// known_loads_walks' `f` could not be looked at.
    #[test]
    fn a_function_renders_its_blocks_and_instructions() {
        let mut function = MFunction::new("f");
        function
            .blocks
            .push(
                llrm_spike_mf::Block {
                    id: 7,
                    insts: vec![Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(1)), Operand::Use(Reg::Virt(0))])],
                    phis: Vec::new(),
                },
            );
        let text = render("before allocation", &function, None);
        assert_eq!(text, "# before allocation: @f\nbb.7:\n  Copy Def(Virt(1)), Use(Virt(0))\n");
    }
}
