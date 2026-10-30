//! A function as the arm64 backend holds it between instruction selection and
//! encoding: blocks of machine instructions over virtual registers
//! (`MachineFunction`), the frame objects it needs, and the pseudo
//! instructions only the backend's own phases understand.

use std::collections::HashMap;

use crate::mc::inst::{Gpr, Inst, Target, Vreg};
use crate::mc::visit::{Reg, RegRef, Role};

/// A machine instruction: a real one, or a pseudo that a later phase turns into
/// real ones (LLVM's pseudo instructions and `COPY`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Op {
    I(Inst),
    /// `dst = src`, registers of one class (LLVM's `COPY`): `mov` or `fmov`
    /// once registers are assigned, nothing if they are the same.
    Copy {
        dst: Reg,
        src: Reg,
    },
    /// `dst = value` in a `bits`-wide (32 or 64) register: `MOVi32imm` and
    /// `MOVi64imm`, expanded to the shortest `movz`/`movk`/`orr` sequence.
    Imm {
        dst: Gpr,
        bits: u32,
        value: u64,
    },
    /// A call: `uses` are the argument registers it reads (physical), `defs`
    /// the result registers it writes; every caller-saved register is
    /// clobbered.
    Call {
        callee: Callee,
        uses: Vec<Reg>,
        defs: Vec<Reg>,
        /// The IR call is marked `tail` and takes no stack arguments: a
        /// candidate for [`crate::sibcall`].
        tail: bool,
    },
    /// A return reading the result registers; or, with `tail`, the jump to
    /// that callee, reading the argument registers (`uses`).
    Return {
        uses: Vec<Reg>,
        tail: Option<Callee>,
    },
    /// `dst` = the address of a frame object plus `offset`, once the frame is
    /// laid out (`ADDXri` on a frame index).
    FrameAddress {
        dst: Gpr,
        slot: u32,
        offset: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Callee {
    Symbol(String),
    Register(Gpr),
}

impl Op {
    /// `f` on each register operand with its role.
    pub fn visit(
        &mut self,
        f: &mut dyn FnMut(RegRef<'_>, Role),
    ) {
        match self {
            Op::I(inst) => inst.visit(f),
            Op::Copy { dst, src } => {
                visit_reg(dst, Role::Def, f);
                visit_reg(src, Role::Use, f);
            }
            Op::Imm { dst, .. } | Op::FrameAddress { dst, .. } => f(RegRef::Gpr(dst), Role::Def),
            Op::Call { callee, uses, defs, .. } => {
                if let Callee::Register(register) = callee {
                    f(RegRef::Gpr(register), Role::Use);
                }
                for register in uses {
                    visit_reg(register, Role::Use, f);
                }
                for register in defs {
                    visit_reg(register, Role::Def, f);
                }
            }
            Op::Return { uses, tail } => {
                if let Some(Callee::Register(register)) = tail {
                    f(RegRef::Gpr(register), Role::Use);
                }
                for register in uses {
                    visit_reg(register, Role::Use, f);
                }
            }
        }
    }

    pub fn defs_and_uses(&self) -> (Vec<Reg>, Vec<Reg>) {
        let (mut defs, mut uses) = (Vec::new(), Vec::new());
        self.clone().visit(&mut |register, role| {
            let key = register.key();
            if role.writes() {
                defs.push(key);
            }
            if role.reads() {
                uses.push(key);
            }
        });
        (defs, uses)
    }
}

fn visit_reg(
    register: &mut Reg,
    role: Role,
    f: &mut dyn FnMut(RegRef<'_>, Role),
) {
    match register {
        Reg::Gpr(inner) => {
            let mut copy = *inner;
            f(RegRef::Gpr(&mut copy), role);
            *inner = copy;
        }
        Reg::Fpr(inner) => {
            let mut copy = *inner;
            f(RegRef::Fpr(&mut copy), role);
            *inner = copy;
        }
    }
}

/// A basic block: a label and its instructions, the last of which is a branch
/// or a return.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MBlock {
    pub label: String,
    pub ops: Vec<Op>,
}

/// A frame object: a local the function addresses, or a spill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Slot {
    pub size: u64,
    pub align: u64,
}

/// Where each value of a function lives before registers are assigned.
#[derive(Clone, Debug, Default)]
pub struct MFunc {
    pub name: String,
    pub blocks: Vec<MBlock>,
    pub slots: Vec<Slot>,
    /// The next virtual register number, of either class.
    pub next_register: u32,
    /// The most bytes of outgoing arguments any call passes on the stack.
    pub outgoing: u64,
    /// Whether the function calls another (it saves the link register).
    pub calls: bool,
    /// The bytes of incoming arguments the caller put on the stack are above
    /// the frame at this offset from the frame pointer.
    pub exported: bool,
    /// The stack pointer moves in the body (`alloca` of a run-time size): frame
    /// objects are addressed from the frame pointer and outgoing arguments are
    /// made room for around each call.
    pub dynamic: bool,
    /// The frame object each source variable lives in (`llvm.dbg.declare` of
    /// an `alloca`), by the variable's metadata node.
    pub declared: Vec<(llrm_mir::MetadataId, u32)>,
}

impl MFunc {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_owned(), next_register: Gpr::FIRST_VIRTUAL, ..Self::default() }
    }

    pub fn gpr(&mut self) -> Gpr {
        let number = self.next_register;
        self.next_register += 1;
        Gpr(number)
    }

    pub fn fpr(&mut self) -> Vreg {
        let number = self.next_register;
        self.next_register += 1;
        Vreg(number)
    }

    pub fn slot(
        &mut self,
        size: u64,
        align: u64,
    ) -> u32 {
        self.slots.push(Slot { size, align });
        (self.slots.len() - 1) as u32
    }
}

/// The frame object that stands for the caller's outgoing argument area: an
/// `Addr::Slot` of it is `offset` bytes above the frame record.
pub const INCOMING_ARGUMENTS: u32 = u32::MAX;

/// The label of the `n`th block of `function`: local to the object.
pub fn block_label(
    function: &str,
    n: usize,
) -> String {
    format!(".L{function}.{n}")
}

pub fn branch_to(label: &str) -> Op {
    Op::I(Inst::Branch { link: false, target: Target::Label(label.to_owned()) })
}

/// The blocks `block` (the `at`th of `count`) can go to, from the labels its
/// branches name and its fall-through.
pub fn successors_of(
    block: &MBlock,
    at: usize,
    count: usize,
    labels: &HashMap<&str, usize>,
) -> Vec<usize> {
    let mut out = Vec::new();
    let mut falls_through = true;
    for op in &block.ops {
        match op {
            Op::I(Inst::Branch { link: false, target: Target::Label(label) }) => {
                out.extend(labels.get(label.as_str()).copied());
                falls_through = false;
            }
            Op::I(Inst::BranchCond { target: Target::Label(label), .. })
            | Op::I(Inst::CompareBranch { target: Target::Label(label), .. })
            | Op::I(Inst::TestBranch { target: Target::Label(label), .. }) => {
                out.extend(labels.get(label.as_str()).copied());
            }
            Op::Return { .. } => falls_through = false,
            Op::I(Inst::Brk { .. }) => falls_through = false,
            _ => {}
        }
    }
    if falls_through && at + 1 < count {
        out.push(at + 1);
    }
    out
}
