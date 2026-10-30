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
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::dagisel::post_ids as post;

/// `TargetOpcode::PHI`: `PHI dst, src, block, src, block, ..`, the blocks being
/// the predecessors.
pub const PHI: u16 = u16::MAX;

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

    /// `COPY`.
    pub fn copy(self) -> u16 {
        self.pick([post::COPY_I32, post::COPY_I64, post::COPY_F32, post::COPY_F64])
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
    /// A symbol and addend, by index into [`Function::symbols`].
    Sym(u32),
    /// A basic block, by its number in the IR function.
    Block(u32),
    /// A stack object, by frame index, until the frame lowering gives it an
    /// address.
    Frame(i32),
    /// The bits of an `f32` immediate.
    F32(u32),
    /// The bits of an `f64` immediate.
    F64(u64),
}

impl Operand {
    /// An operand that is no register: printed and encoded after the opcode.
    pub fn is_immediate(&self) -> bool {
        !matches!(self, Operand::Def(_) | Operand::Use(_))
    }
}

/// The one `MachineMemOperand` of a load or store: its size and alignment in
/// bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mem {
    pub size: u64,
    pub align: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inst {
    pub opcode: u16,
    pub operands: Vec<Operand>,
    pub mem: Option<Mem>,
    /// Names a scope marker (`BLOCK`, `LOOP` and their ends) while CFGStackify
    /// works; 0 for any other.
    pub tag: u32,
}

impl Inst {
    pub fn new(
        opcode: u16,
        operands: &[Operand],
    ) -> Inst {
        Inst { opcode, operands: operands.to_vec(), mem: None, tag: 0 }
    }

    pub fn with_mem(
        mut self,
        mem: Mem,
    ) -> Inst {
        self.mem = Some(mem);
        self
    }

    /// `MI.isTerminator()`.
    pub fn is_terminator(&self) -> bool {
        desc(self.opcode).is_some_and(|desc| desc.has(flag::TERMINATOR))
    }

    /// `MI.isBranch()`.
    pub fn is_branch(&self) -> bool {
        desc(self.opcode).is_some_and(|desc| desc.has(flag::BRANCH))
    }

    /// `MI.isBarrier()`.
    pub fn is_barrier(&self) -> bool {
        desc(self.opcode).is_some_and(|desc| desc.has(flag::BARRIER))
    }

    /// `MI.defs()`.
    pub fn defs(&self) -> impl Iterator<Item = VReg> + '_ {
        self.operands.iter().filter_map(|o| if let Operand::Def(r) = o { Some(*r) } else { None })
    }
}

/// `MRI`'s def and use lists, as counts: valid until instructions are added,
/// removed or moved.
pub struct DefUse {
    def: Vec<Option<(usize, usize)>>,
    defs: Vec<u32>,
    uses: Vec<u32>,
}

impl DefUse {
    /// `MRI.getVRegDef`: the block and index of the first definition.
    pub fn def(
        &self,
        reg: VReg,
    ) -> Option<(usize, usize)> {
        self.def[reg as usize]
    }

    pub fn def_count(
        &self,
        reg: VReg,
    ) -> usize {
        self.defs[reg as usize] as usize
    }

    pub fn use_count(
        &self,
        reg: VReg,
    ) -> usize {
        self.uses[reg as usize] as usize
    }

    /// `MRI.use_empty`: no instruction reads `reg`.
    pub fn use_empty(
        &self,
        reg: VReg,
    ) -> bool {
        self.uses[reg as usize] == 0
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
    /// The successors of each block, by position, in the terminator's order.
    pub succs: Vec<Vec<usize>>,
    /// The symbol and addend of each [`Operand::Sym`].
    pub symbols: Vec<(String, i64)>,
    /// The (name, parameters, results) of each function the code names: the
    /// signature the object file gives an undefined function.
    /// Internal or private: no `.globl`, binding local in the object.
    pub local: bool,
    pub callees: Vec<(String, Vec<ValType>, Vec<ValType>)>,
    /// The (name, type) of each wasm global the code names: an import of the object file.
    pub globals: Vec<(String, ValType)>,
    /// The (size, alignment) of each stack object, by frame index:
    /// `MachineFrameInfo`.
    pub frame: Vec<(u64, u64)>,
    vregs: Vec<ValType>,
    stackified: Vec<bool>,
}

impl Function {
    pub fn new(name: &str) -> Function {
        Function { name: name.to_string(), blocks: vec![Vec::new()], ..Function::default() }
    }

    /// The blocks in the order `order` gives (`order[new] = old`):
    /// `MachineFunction::RenumberBlocks` after the layout changed. Block
    /// operands and successors follow.
    pub fn apply_order(
        &mut self,
        order: &[usize],
    ) {
        let mut new_of = vec![usize::MAX; self.blocks.len()];
        for (new, &old) in order.iter().enumerate() {
            new_of[old] = new;
        }
        let mut blocks = std::mem::take(&mut self.blocks).into_iter().map(Some).collect::<Vec<_>>();
        let mut succs = std::mem::take(&mut self.succs).into_iter().map(Some).collect::<Vec<_>>();
        self.blocks = order.iter().map(|&old| blocks[old].take().expect("a block once")).collect();
        self.succs = order
            .iter()
            .map(|&old| succs[old].take().expect("a block once").into_iter().map(|to| new_of[to]).collect())
            .collect();
        for operand in self.blocks.iter_mut().flatten().flat_map(|inst| inst.operands.iter_mut()) {
            if let Operand::Block(block) = operand {
                *block = new_of[*block as usize] as u32;
            }
        }
    }

    /// The index of `name + offset` in [`Function::symbols`].
    pub fn symbol(
        &mut self,
        name: &str,
        offset: i64,
    ) -> u32 {
        let at = self
            .symbols
            .iter()
            .position(|(n, o)| n == name && *o == offset)
            .unwrap_or_else(
                || {
                    self.symbols.push((name.to_string(), offset));
                    self.symbols.len() - 1
                },
            );
        at as u32
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

    /// The definitions and uses of every register, in one pass over the
    /// function.
    pub fn def_use(&self) -> DefUse {
        let n = self.vregs.len();
        let mut index = DefUse { def: vec![None; n], defs: vec![0; n], uses: vec![0; n] };
        for (b, insts) in self.blocks.iter().enumerate() {
            for (i, inst) in insts.iter().enumerate() {
                for operand in &inst.operands {
                    match *operand {
                        Operand::Def(r) => {
                            index.defs[r as usize] += 1;
                            index.def[r as usize].get_or_insert((b, i));
                        }
                        Operand::Use(r) => index.uses[r as usize] += 1,
                        _ => {}
                    }
                }
            }
        }
        index
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
