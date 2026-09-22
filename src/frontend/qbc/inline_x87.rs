//! Port of `qbopt/frontend/qb/inline_x87.py`: QB-owned final spelling of rich
//! inline math operations.
//!
//! Allocated `st(0) -> st(0)` intrinsic pseudos become measured inline bytes,
//! deliberately too late to affect optimization or allocation.

use std::sync::Arc;

use indexmap::IndexMap;

use crate::backend::masm;
use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::model::lir;

/// `_CODE`: 80387 encodings. FEXP2 splits x into nearest integer n and
/// fraction f, then computes (2**f) * (2**n).
#[allow(non_snake_case)]
fn _CODE(name: &str) -> Option<Vec<u8>> {
    let hex: &str = match name {
        "fsin" => "d9fe",
        "fcos" => "d9ff",
        "fatan" => "d9e8d9f3",     // fld1; fpatan
        "flog2" => "d9e8d9c9d9f1", // fld1; fxch; fyl2x
        "fexp2" => "d9c0d9fcd9c9d8e1d9f0d9e8dec1d9fdddd9",
        _ => return None,
    };
    Some((0..hex.len()).step_by(2).map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap()).collect())
}

#[derive(Clone, Debug)]
pub struct Finalized {
    pub body: lir::LirBody,
    pub callees: IndexMap<i64, masm::Callee>,
}

/// Replace allocated QB intrinsic pseudos with inline-byte placeholders.
pub fn finalized(body: &lir::LirBody, parameter_bytes: i64) -> Result<Finalized, String> {
    if !(0..=0xFFFF).contains(&parameter_bytes) {
        return Err("QB far-return cleanup exceeds 16 bits".into());
    }
    let mut sites: IndexMap<i64, masm::Callee> = IndexMap::new();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut instructions = Vec::new();
        for instruction in &block.insns {
            let what = instruction.what.as_ref();
            let code = what.and_then(|what| _CODE(what.name.as_deref().unwrap_or("")));
            let Some(code) = code else {
                if let Some(what) = what {
                    if what.op == Operation::Return && parameter_bytes != 0 {
                        let mut replaced = (**instruction).clone();
                        replaced.what = Some(Semantics {
                            sources: vec![Loc::Imm(ir::Imm { value: parameter_bytes, width: 2, address: None })],
                            ..what.clone()
                        });
                        instructions.push(Arc::new(replaced));
                        continue;
                    }
                }
                instructions.push(Arc::clone(instruction));
                continue;
            };
            let what = what.expect("a named operation");
            let st0 = vec![Loc::St(ir::St { index: 0 })];
            let name = what.name.clone().unwrap_or_default();
            if what.op != Operation::FloatUnary || what.dests != st0 || what.sources != st0 {
                return Err(format!("{name} must be allocated as st(0) -> st(0)"));
            }
            sites.insert(
                instruction.at,
                masm::Callee { name: format!("$inline_{name}"), far: false, code: vec![masm::InlinePart::Bytes(code)] },
            );
            let mut replaced = (**instruction).clone();
            replaced.what = Some(Semantics { name: Some(name), ..Semantics::new(Operation::Call) });
            instructions.push(Arc::new(replaced));
        }
        blocks.push(lir::LirBlock { insns: instructions, ..block.clone() });
    }
    Ok(Finalized { body: lir::LirBody { blocks, ..body.clone() }, callees: sites })
}

/// Return one audited expansion for diagnostics and stage dumps.
pub fn expansion(name: &str) -> Option<Vec<u8>> {
    _CODE(name)
}
