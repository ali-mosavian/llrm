//! `FunctionLoweringInfo`: which values live in virtual registers across
//! blocks, and which allocas are frame indices.

use std::collections::{HashMap, HashSet};

use llrm_mir::module::{Function, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;

/// What `FunctionLoweringInfo::set` decides before any block is selected.
pub struct FunctionLoweringInfo {
    /// `ValueMap`: a value used outside its block (or by a phi) and the
    /// virtual register that carries it.
    pub value_regs: HashMap<ValueId, u32>,
    /// `StaticAllocaMap`.
    pub static_allocas: HashMap<ValueId, i32>,
    next_reg: u32,
    next_frame_index: i32,
}

impl FunctionLoweringInfo {
    /// `FunctionLoweringInfo::set`: one walk in layout order, numbering
    /// registers and frame indices as the pin does.
    pub fn new(function: &Function) -> FunctionLoweringInfo {
        let mut outside: HashSet<ValueId> = HashSet::new();
        for &block in function.layout() {
            for inst in function.iter_block(block) {
                let instruction = function.instruction(inst);
                let is_phi = instruction.opcode == Opcode::Phi;
                for operand in &instruction.operands {
                    let Operand::Value(value) = operand else { continue };
                    let ValueDef::Instruction(def) = function.value(*value).def else { continue };
                    if is_phi || function.parent(def) != Some(block) {
                        outside.insert(*value);
                    }
                }
            }
        }
        let mut info = FunctionLoweringInfo { value_regs: HashMap::new(), static_allocas: HashMap::new(), next_reg: 0, next_frame_index: 0 };
        let entry = function.entry();
        for &block in function.layout() {
            for inst in function.iter_block(block) {
                let instruction = function.instruction(inst);
                let Some(result) = instruction.result else { continue };
                if let Opcode::Alloca { .. } = instruction.opcode {
                    if Some(block) == entry && instruction.operands.iter().all(|op| matches!(op, Operand::Constant(_))) {
                        info.static_allocas.insert(result, info.next_frame_index);
                        info.next_frame_index += 1;
                        continue;
                    }
                }
                let phi_used = instruction.opcode == Opcode::Phi && !function.users(result).is_empty();
                if phi_used || outside.contains(&result) {
                    info.create_reg(result);
                }
            }
        }
        info
    }

    /// `CreateRegs`: the next virtual register, recorded for `value`.
    pub fn create_reg(&mut self, value: ValueId) -> u32 {
        let reg = self.new_reg();
        self.value_regs.insert(value, reg);
        reg
    }

    /// A register with no value of its own (a constant a phi receives).
    pub fn new_reg(&mut self) -> u32 {
        let reg = self.next_reg;
        self.next_reg += 1;
        reg
    }
}
