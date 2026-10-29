//! AAPCS64 (Darwin) as the DAG builder's `TargetCalling`: `LowerFormalArguments`
//! and `LowerReturn` for integers and pointers.
//!
//! # How it works
//! `CC_AArch64_DarwinPCS` gives the first eight integer parts `x0`..`x7` (`w0`..
//! `w7` for an `i32`); `RetCC_AArch64_AAPCS` gives a result `x0`/`w0`, with an
//! `i128` in `x0`, `x1`. The builder has split every value into legal
//! `i32`/`i64` parts. A return is `RET_GLUE chain, registers.., glue`.
//!
//! DEVIATE: not translated are stack arguments, calls, floats and vectors,
//! `sret` (`x8`), `i128` alignment to an even register pair, HFAs and varargs;
//! `accepts` refuses them. Reason: the first function needs none. Exit: B3.
//!
//! # See also
//! `AArch64ISelLowering.cpp` at the pin: `LowerFormalArguments`, `LowerReturn`.

use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::target_calling::{FixedObject, In, LiveIn, Out, Site, TargetCalling};
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_lir::registers::RegId;

use crate::reg;

/// `AArch64ISD::RET_GLUE`, the pin's number.
pub const RET_GLUE: u16 = llrm_dagisel::generated::aarch64::ids::isd::RET_GLUE;

const X: [RegId; 8] = [reg::X0, reg::X1, reg::X2, reg::X3, reg::X4, reg::X5, reg::X6, reg::X7];
const W: [RegId; 8] = [reg::W0, reg::W1, reg::W2, reg::W3, reg::W4, reg::W5, reg::W6, reg::W7];

/// The register part `at` of type `vt` is in.
fn register(
    at: usize,
    vt: EVT,
) -> RegId {
    if vt == EVT::I32 { W[at] } else { X[at] }
}

#[derive(Debug, Default)]
pub struct Aarch64Calling;

impl TargetCalling for Aarch64Calling {
    fn accepts(
        &self,
        dag: &SelectionDAG,
        site: Site,
        outs: &[Out],
        ins: &[In],
    ) -> Result<(), &'static str> {
        if site == Site::Call {
            return Err("a call is not translated for arm64");
        }
        let limit = if site == Site::Return { 2 } else { 8 };
        let fits = |vts: Vec<EVT>| vts.len() <= limit && vts.iter().all(|&vt| vt == EVT::I32 || vt == EVT::I64);
        if fits(outs.iter().map(|o| dag.value_type(o.value)).collect()) && fits(ins.iter().map(|i| i.vt).collect()) {
            Ok(())
        } else {
            Err("an arm64 argument or result that is no integer in x0..x7")
        }
    }

    fn lower_call(
        &self,
        _dag: &mut SelectionDAG,
        _chain: SDValue,
        _callee: SDValue,
        _outs: &[Out],
        _ins: &[In],
    ) -> (SDValue, Vec<SDValue>) {
        unreachable!("accepts refuses every call")
    }

    /// Each part is read from the virtual register its live-in register is
    /// copied to (`CopyFromReg`).
    fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        _fixed: FixedObject,
        live_in: LiveIn,
    ) -> Vec<SDValue> {
        ins.iter()
            .enumerate()
            .map(|(at, part)| {
                let vreg = live_in(register(at, part.vt));
                dag.get_copy_from_reg(chain, vreg, part.vt)
            })
            .collect()
    }

    fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue {
        let mut chain = chain;
        let mut glue = None;
        let mut operands = vec![chain];
        for (at, out) in outs.iter().enumerate() {
            let vt = dag.value_type(out.value);
            let reg = register(at, vt);
            let copy = dag.get_copy_to_phys_reg(chain, reg, out.value, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
            operands.push(dag.get_phys_register(vt, reg));
        }
        operands[0] = chain;
        operands.extend(glue);
        dag.get_node(llrm_codegen::backend::selectiondag::isd::ISD::Target(RET_GLUE), EVT::OTHER, &operands)
    }
}
