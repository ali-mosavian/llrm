//! `InstrEmitter` for wasm: the selected DAG's machine nodes, in the order the
//! caller schedules them, as a [`Function`].
//!
//! Each non-chain result becomes a virtual register, each non-chain operand a
//! use or an immediate. Chains and glue order the nodes and emit nothing.
//!
//! DEVIATE: the order is the caller's. Reason: the scheduler (`RegPressure`
//! for wasm, the fast one at -O0) is not bound to a wasm DAG yet. Exit: the
//! codegen binding (S2 on the m68k branch).
//!
//! # See also
//! `InstrEmitter::EmitMachineNode`, pin d1106deb71cc.

use std::collections::HashMap;

use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::generated::ids::mvt;
use llrm_dagisel::matcher::Vt;

use crate::dagisel::post_ids::END_FUNCTION;
use crate::mir::{Function, Inst, Mem, Operand, VReg, ValType};

/// The register class of a value type, `None` for a chain or glue.
fn class(vt: Vt) -> Option<ValType> {
    match vt {
        v if v == mvt::i32 => Some(ValType::I32),
        v if v == mvt::i64 => Some(ValType::I64),
        v if v == mvt::f32 => Some(ValType::F32),
        v if v == mvt::f64 => Some(ValType::F64),
        _ => None,
    }
}

/// The function whose body is `order`, followed by `END_FUNCTION`.
pub fn emit<D: Dag>(
    dag: &D,
    name: &str,
    params: &[ValType],
    results: &[ValType],
    order: &[u32],
) -> Function {
    let mut f = Function::new(name);
    f.params = params.to_vec();
    f.results = results.to_vec();
    let mut regs: HashMap<(u32, u8), VReg> = HashMap::new();
    for &node in order {
        let mut operands = Vec::new();
        for res in 0..dag.num_values(node) {
            let value = SDValue::new(node, res as u8);
            if let Some(ty) = class(dag.value_type(value)) {
                let reg = f.new_vreg(ty);
                regs.insert((node, res as u8), reg);
                operands.push(Operand::Def(reg));
            }
        }
        for i in 0..dag.num_operands(node) {
            let value = dag.operand(node, i);
            if let Some(imm) = dag.constant(value) {
                operands.push(Operand::Imm(imm));
            } else if class(dag.value_type(value)).is_some() {
                let reg = regs
                    .get(&(value.node, value.res))
                    .unwrap_or_else(|| panic!("operand {i} of node {node} is not scheduled"));
                operands.push(Operand::Use(*reg));
            }
        }
        let opcode = machine_opcode(dag, node);
        let mut inst = Inst { opcode, operands, mem: None, tag: 0 };
        if dag.mem_operands(node).is_some() {
            inst.mem = Some(Mem { size: dag.memory_store_size(node), align: dag.alignment(node) });
        }
        f.blocks[0].push(inst);
    }
    f.blocks[0].push(Inst::new(END_FUNCTION, &[]));
    f
}

/// A machine node's opcode is its complement in the DAG's opcode space.
fn machine_opcode<D: Dag>(
    dag: &D,
    node: u32,
) -> u16 {
    assert!(dag.is_machine(node), "node {node} is not selected");
    !dag.opcode(node)
}
