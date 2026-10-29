//! The wasm machine function the post-selection passes work on.
//!
//! DEVIATE: a spike-local model inside llrm-wasm, keyed to the generated
//! opcode ids and the pin's operand order (defs first). Reason: the repo has no
//! target-neutral machine function (`llrm-lir` is x86-register-shaped).
//! Exit: the neutral machine function, one post-main task for every target.
//!
//! # See also
//! `MachineRegisterInfo`, `WebAssemblyFunctionInfo`
//! (`WebAssemblyMachineFunctionInfo.h`).

use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::post_ids as post;

/// A virtual register, by index.
pub type VReg = u32;

/// The register classes the slice uses: `I32RegClass` .. `F64RegClass`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
}

impl ValType {
    /// `getLocalGetOpcode`.
    pub fn local_get(self) -> u16 {
        self.pick([op::LOCAL_GET_I32, op::LOCAL_GET_I64, op::LOCAL_GET_F32, op::LOCAL_GET_F64])
    }

    /// `getLocalSetOpcode`.
    pub fn local_set(self) -> u16 {
        self.pick([op::LOCAL_SET_I32, op::LOCAL_SET_I64, op::LOCAL_SET_F32, op::LOCAL_SET_F64])
    }

    /// `getLocalTeeOpcode`.
    pub fn local_tee(self) -> u16 {
        self.pick([post::LOCAL_TEE_I32, post::LOCAL_TEE_I64, post::LOCAL_TEE_F32, post::LOCAL_TEE_F64])
    }

    /// `getDropOpcode`.
    pub fn drop(self) -> u16 {
        self.pick([post::DROP_I32, post::DROP_I64, post::DROP_F32, post::DROP_F64])
    }

    fn pick(
        self,
        ids: [u16; 4],
    ) -> u16 {
        ids[self as usize]
    }
}

/// `WebAssembly::isArgument`.
pub fn is_argument(opcode: u16) -> bool {
    [op::ARGUMENT_i32, op::ARGUMENT_i64, op::ARGUMENT_f32, op::ARGUMENT_f64].contains(&opcode)
}

/// `WebAssembly::isTee`.
pub fn is_tee(opcode: u16) -> bool {
    [post::TEE_I32, post::TEE_I64, post::TEE_F32, post::TEE_F64].contains(&opcode)
}

/// `WebAssembly::isCopy`.
pub fn is_copy(opcode: u16) -> bool {
    [post::COPY_I32, post::COPY_I64, post::COPY_F32, post::COPY_F64].contains(&opcode)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    Def(VReg),
    Use(VReg),
    Imm(i64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inst {
    pub opcode: u16,
    pub operands: Vec<Operand>,
}

impl Inst {
    pub fn new(
        opcode: u16,
        operands: &[Operand],
    ) -> Inst {
        Inst { opcode, operands: operands.to_vec() }
    }

    /// `MI.defs()`.
    pub fn defs(&self) -> impl Iterator<Item = VReg> + '_ {
        self.operands.iter().filter_map(|o| if let Operand::Def(r) = o { Some(*r) } else { None })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Function {
    pub name: String,
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
    /// `MFI.getLocals()`: the types of the locals after the parameters.
    pub locals: Vec<ValType>,
    pub blocks: Vec<Vec<Inst>>,
    vregs: Vec<ValType>,
    stackified: Vec<bool>,
}

impl Function {
    pub fn new(name: &str) -> Function {
        Function { name: name.to_string(), blocks: vec![Vec::new()], ..Function::default() }
    }

    /// `MRI.createVirtualRegister`.
    pub fn new_vreg(
        &mut self,
        ty: ValType,
    ) -> VReg {
        self.vregs.push(ty);
        self.stackified.push(false);
        (self.vregs.len() - 1) as VReg
    }

    pub fn num_vregs(&self) -> usize {
        self.vregs.len()
    }

    /// `MRI.getRegClass`.
    pub fn vreg_type(
        &self,
        reg: VReg,
    ) -> ValType {
        self.vregs[reg as usize]
    }

    /// `MFI.stackifyVReg`.
    pub fn stackify(
        &mut self,
        reg: VReg,
    ) {
        self.stackified[reg as usize] = true;
    }

    /// `MFI.unstackifyVReg`.
    pub fn unstackify(
        &mut self,
        reg: VReg,
    ) {
        self.stackified[reg as usize] = false;
    }

    /// `MFI.isVRegStackified`.
    pub fn is_stackified(
        &self,
        reg: VReg,
    ) -> bool {
        self.stackified[reg as usize]
    }

    /// `MRI.use_empty`: no instruction reads `reg`.
    pub fn use_empty(
        &self,
        reg: VReg,
    ) -> bool {
        !self.blocks.iter().flatten().any(|i| i.operands.contains(&Operand::Use(reg)))
    }

    /// `MRI.getVRegDef`: the block and index of the defining instruction.
    pub fn vreg_def(
        &self,
        reg: VReg,
    ) -> Option<(usize, usize)> {
        self.blocks
            .iter()
            .enumerate()
            .find_map(|(b, insts)| insts.iter().position(|i| i.defs().any(|d| d == reg)).map(|i| (b, i)))
    }

    /// `MRI.replaceRegWith`: every use and def of `from` names `to`.
    pub fn replace_reg_with(
        &mut self,
        from: VReg,
        to: VReg,
    ) {
        for operand in self.blocks.iter_mut().flatten().flat_map(|i| i.operands.iter_mut()) {
            match operand {
                Operand::Def(r) | Operand::Use(r) if *r == from => *r = to,
                _ => {}
            }
        }
    }
}
