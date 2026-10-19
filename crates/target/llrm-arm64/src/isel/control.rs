//! Branches and the copies that carry phi values along an edge. An edge into a
//! block with phis from a block with several successors (a critical edge) gets
//! a block of its own for its copies, laid out after the rest.

use llrm_mir::{BlockId, Instruction, Opcode, Operand};

use super::val::{AddrExpr, Base, Val};
use super::{Selected, Selector, Src, refuse};
use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{MBlock, Op, branch_to};

/// What a conditional branch tests: the flags, a register against zero, or one
/// bit of a register (`cbz`, `tbz`).
pub(super) enum Test {
    Flags(Cond),
    Zero { reg: Gpr, width: Width, nonzero: bool },
    Bit { reg: Gpr, bit: u8, nonzero: bool },
}

impl Test {
    fn inverted(&self) -> Test {
        match *self {
            Test::Flags(cond) => Test::Flags(cond.inverted()),
            Test::Zero { reg, width, nonzero } => Test::Zero { reg, width, nonzero: !nonzero },
            Test::Bit { reg, bit, nonzero } => Test::Bit { reg, bit, nonzero: !nonzero },
        }
    }
}

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

    /// How to branch on the i1 `operand`: a comparison with zero is `cbz`, a
    /// sign test or a single-bit mask is `tbz`; anything else the flags.
    fn branch_test(
        &mut self,
        operand: Operand,
    ) -> Selected<Test> {
        use llrm_mir::IntPredicate;
        if let Operand::Value(value) = operand
            && self.fusable(value)
            && let Some(inst) = super::value_def(self.function, value)
            && let Opcode::ICmp(predicate) = self.function.instruction(inst).opcode
        {
            let compare = self.function.instruction(inst).clone();
            let (left, right) = (compare.operands[0], compare.operands[1]);
            let bits = self.types().int_bits(self.operand_type(left)?).unwrap_or(64);
            if self.is_zero_constant(right) && bits <= 64 {
                // An `and` left for a `tst`: one bit is `tbz`, any other mask
                // the flags.
                if let Operand::Value(masked) = left
                    && let Some(and) = self.fused_and(masked)
                {
                    if matches!(predicate, IntPredicate::Eq | IntPredicate::Ne)
                        && let Src::Int { value, .. } = self.src(and.operands[1])?
                        && value.is_power_of_two()
                    {
                        let g = self.operand_g(and.operands[0])?;
                        return Ok(Test::Bit {
                            reg: g.reg,
                            bit: value.trailing_zeros() as u8,
                            nonzero: predicate == IntPredicate::Ne,
                        });
                    }
                    self.test_bits(&and)?;
                    return Ok(Test::Flags(super::int::condition_of(predicate)));
                }
                match predicate {
                    IntPredicate::Slt | IntPredicate::Sge => {
                        let g = self.operand_g(left)?;
                        return Ok(Test::Bit {
                            reg: g.reg,
                            bit: (bits - 1) as u8,
                            nonzero: predicate == IntPredicate::Slt,
                        });
                    }
                    IntPredicate::Eq | IntPredicate::Ne => {
                        let g = self.operand_g(left)?;
                        let g = self.extended(g, super::val::Known::Zero);
                        return Ok(Test::Zero {
                            reg: g.reg,
                            width: super::val::width_of(bits),
                            nonzero: predicate == IntPredicate::Ne,
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(Test::Flags(self.condition(operand)?))
    }

    fn is_zero_constant(
        &self,
        operand: Operand,
    ) -> bool {
        matches!(
            operand,
            Operand::Constant(id) if matches!(
                self.module.context.get(id).kind,
                llrm_mir::ConstantKind::Int(0) | llrm_mir::ConstantKind::Zero | llrm_mir::ConstantKind::Null
            )
        )
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
        let test = self.branch_test(instruction.operands[0])?;
        let true_label = self.edge(from, on_true, 2)?;
        let false_label = self.edge(from, on_false, 2)?;
        let jump = |test: &Test, label: &str| {
            let target = Target::Label(label.to_owned());
            Op::I(match *test {
                Test::Flags(cond) => Inst::BranchCond { cond, target },
                Test::Zero { reg, width, nonzero } => Inst::CompareBranch { nonzero, width, rt: reg, target },
                Test::Bit { reg, bit, nonzero } => Inst::TestBranch { nonzero, rt: reg, bit, target },
            })
        };
        if next == Some(on_false) && !self.is_edge_label(&false_label) {
            self.emit(jump(&test, &true_label));
        } else if next == Some(on_true) && !self.is_edge_label(&true_label) {
            self.emit(jump(&test.inverted(), &false_label));
        } else {
            self.emit(jump(&test, &true_label));
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
