//! Branches and the copies that carry phi values along an edge. An edge into a
//! block with phis from a block with several successors (a critical edge) gets
//! a block of its own for its copies, laid out after the rest.

use llrm_mir::{BlockId, Instruction, Opcode, Operand};

use super::val::{AddrExpr, Base, Val};
use super::{Selected, Selector, Src, refuse};
use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{MBlock, Op, branch_to};

/// A phi's incoming value, to put in its register.
struct PhiCopy {
    dst: Reg,
    src: PhiSource,
}

enum PhiSource {
    Reg(Reg),
    Value(Src),
}

impl Selector<'_> {
    /// The phi copies for the edge `from` to `to`.
    fn phi_copies(
        &mut self,
        from: BlockId,
        to: BlockId,
    ) -> Selected<Vec<PhiCopy>> {
        let mut copies = Vec::new();
        for inst in self.function.iter_block(to).collect::<Vec<_>>() {
            let phi = self.function.instruction(inst);
            if phi.opcode != Opcode::Phi {
                break;
            }
            let Some(pair) = phi.operands.chunks(2).find(|pair| pair[1] == Operand::Block(from)) else {
                return refuse("a phi with no value for an edge");
            };
            let incoming = pair[0];
            let result = phi.result.expect("a phi has a result");
            let dst = self.vals.get(&result).and_then(|value| self.reg_of(value)).ok_or("a phi with no register")?;
            let src = self.src(incoming)?;
            let src = match &src {
                Src::Val(value) => match self.reg_of(value) {
                    Some(reg) => PhiSource::Reg(reg),
                    None => PhiSource::Value(src),
                },
                _ => PhiSource::Value(src),
            };
            copies.push(PhiCopy { dst, src });
        }
        Ok(copies)
    }

    /// Emits `copies` as a parallel copy: no copy reads a register an earlier
    /// one wrote.
    fn emit_copies(
        &mut self,
        copies: Vec<PhiCopy>,
    ) -> Selected<()> {
        let mut moves: Vec<(Reg, Reg)> = Vec::new();
        let mut loads = Vec::new();
        for copy in copies {
            match copy.src {
                PhiSource::Reg(src) if src != copy.dst => moves.push((copy.dst, src)),
                PhiSource::Reg(_) => {}
                PhiSource::Value(src) => loads.push((copy.dst, src)),
            }
        }
        while !moves.is_empty() {
            // A move whose destination no other move reads.
            let ready = moves.iter().position(|(dst, _)| !moves.iter().any(|(_, src)| src == dst));
            match ready {
                Some(at) => {
                    let (dst, src) = moves.remove(at);
                    self.emit(Op::Copy { dst, src });
                }
                None => {
                    // A cycle: park one source in a new register.
                    let (dst, src) = moves[0];
                    let temporary = match src {
                        Reg::Gpr(_) => Reg::Gpr(self.mf.gpr()),
                        Reg::Fpr(_) => Reg::Fpr(self.mf.fpr()),
                    };
                    self.emit(Op::Copy { dst: temporary, src });
                    for (_, reader) in moves.iter_mut() {
                        if *reader == src {
                            *reader = temporary;
                        }
                    }
                    let _ = dst;
                }
            }
        }
        for (dst, src) in loads {
            self.load_into(dst, src)?;
        }
        Ok(())
    }

    /// `dst = src` for a constant or a symbolic address.
    fn load_into(
        &mut self,
        dst: Reg,
        src: Src,
    ) -> Selected<()> {
        match (dst, src) {
            (Reg::Gpr(reg), Src::Int { bits, value }) => {
                self.load_immediate(reg, bits, value as u64);
                Ok(())
            }
            (Reg::Gpr(reg), Src::Val(Val::Ptr(address))) => {
                let source = self.pointer_register(&address)?;
                self.emit(Op::Copy { dst: Reg::Gpr(reg), src: Reg::Gpr(source) });
                Ok(())
            }
            (Reg::Fpr(reg), Src::Float { size, bits }) => {
                let Val::Float { reg: loaded, .. } = self.materialize(Src::Float { size, bits })? else {
                    unreachable!()
                };
                self.emit(Op::Copy { dst: Reg::Fpr(reg), src: Reg::Fpr(loaded) });
                Ok(())
            }
            _ => refuse("a phi value in the wrong register class"),
        }
    }

    /// The label to branch to for the edge from the current block to `to`;
    /// copies on a critical edge are in a block made for them.
    fn edge(
        &mut self,
        from: BlockId,
        to: BlockId,
        branches: usize,
    ) -> Selected<String> {
        let Some(index) = self.block_index(to) else { return refuse("a branch to an unreachable block") };
        let label = self.mf.blocks[index].label.clone();
        let has_phis = self
            .function
            .iter_block(to)
            .next()
            .is_some_and(|first| self.function.instruction(first).opcode == Opcode::Phi);
        if !has_phis {
            return Ok(label);
        }
        let copies = self.phi_copies(from, to)?;
        if branches == 1 {
            self.emit_copies(copies)?;
            return Ok(label);
        }
        // A block of its own, after the function's: the copies, then the
        // branch.
        let saved = self.current;
        let edge_label = format!(".L{}.e{}", self.mf.name, self.mf.blocks.len());
        self.mf.blocks.push(MBlock { label: edge_label.clone(), ops: Vec::new() });
        self.current = self.mf.blocks.len() - 1;
        let result = self.emit_copies(copies);
        self.emit(branch_to(&label));
        self.current = saved;
        result?;
        Ok(edge_label)
    }

    pub(super) fn br(
        &mut self,
        at: usize,
        instruction: &Instruction,
    ) -> Selected<()> {
        let from = self.block_order()[at];
        let next = self.block_order().get(at + 1).copied();
        let target = |operand: Operand| match operand {
            Operand::Block(block) => Ok(block),
            _ => refuse("a branch to a non-block"),
        };
        if instruction.operands.len() == 1 {
            let to = target(instruction.operands[0])?;
            let label = self.edge(from, to, 1)?;
            if next != Some(to) {
                self.emit(branch_to(&label));
            }
            return Ok(());
        }
        let (on_true, on_false) = (target(instruction.operands[1])?, target(instruction.operands[2])?);
        let cond = self.condition(instruction.operands[0])?;
        let true_label = self.edge(from, on_true, 2)?;
        let false_label = self.edge(from, on_false, 2)?;
        let conditional =
            |cond: Cond, label: &str| Op::I(Inst::BranchCond { cond, target: Target::Label(label.to_owned()) });
        if next == Some(on_false) && !self.is_edge_label(&false_label) {
            self.emit(conditional(cond, &true_label));
        } else if next == Some(on_true) && !self.is_edge_label(&true_label) {
            self.emit(conditional(cond.inverted(), &false_label));
        } else {
            self.emit(conditional(cond, &true_label));
            self.emit(branch_to(&false_label));
        }
        Ok(())
    }
}

#[allow(dead_code)]
fn unused(
    _: AddrExpr,
    _: Base,
) {
}
