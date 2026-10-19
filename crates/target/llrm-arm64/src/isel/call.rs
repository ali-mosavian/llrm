//! Parameters, calls and returns: where each value goes under the platform's
//! convention (`crate::cc`), and the copies between those registers and the
//! function's own.

use llrm_mir::{Attribute, CallInfo, GlobalKind, InstId, Instruction, Opcode, Operand};

use super::val::{AddrExpr, Base, G, Known, Val};
use super::{Selected, Selector, Src, refuse};
use crate::cc::{Assigner, Place, Scalar};
use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{Callee, INCOMING_ARGUMENTS, Op};

fn has_flag(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name))
}

impl Selector<'_> {
    /// The incoming arguments, copied into virtual registers at the entry.
    pub(super) fn parameters(&mut self) -> Selected<()> {
        let mut assigner = Assigner::new(self.platform);
        let parameters: Vec<_> = self.function.parameters().to_vec();
        for (position, &parameter) in parameters.iter().enumerate() {
            let ty = self.function.value(parameter).ty;
            let scalar = self.scalar(ty)?;
            let attrs = self.function.parameter_attrs.get(position).cloned().unwrap_or_default();
            let place = assigner.assign(scalar, true);
            let value = self.fresh(ty)?;
            match (place, &value) {
                (Place::Gpr(number), Val::Int(g)) => {
                    // `mov w, w` for a value narrower than 64 bits: it also
                    // clears the bits above the 32nd, which
                    // the caller leaves undefined.
                    let source = Gpr(number);
                    if scalar == Scalar::I32 {
                        self.inst(Inst::LogicShifted {
                            op: Logic::Orr,
                            invert: false,
                            width: Width::W,
                            rd: g.reg,
                            rn: Gpr::R31,
                            rm: source,
                            shift: Shift::Lsl,
                            amount: 0,
                        });
                    } else {
                        self.emit(Op::Copy { dst: Reg::Gpr(g.reg), src: Reg::Gpr(source) });
                    }
                }
                (Place::Gpr(number), Val::Ptr(AddrExpr { base: Base::Reg(reg), .. })) => {
                    self.emit(Op::Copy { dst: Reg::Gpr(*reg), src: Reg::Gpr(Gpr(number)) });
                }
                (Place::Fpr(number), Val::Float { reg, .. }) => {
                    self.emit(Op::Copy { dst: Reg::Fpr(*reg), src: Reg::Fpr(Vreg(number)) });
                }
                (Place::Stack { offset, size }, _) => {
                    let addr = Addr::Slot { slot: INCOMING_ARGUMENTS, offset: offset as i64 };
                    match &value {
                        Val::Int(g) => self.inst(Inst::Load {
                            mem: Memory::Int {
                                width: if size == 8 { Width::X } else { Width::W },
                                bytes: size as u8,
                                sign: false,
                                reg: g.reg,
                            },
                            addr,
                        }),
                        Val::Ptr(AddrExpr { base: Base::Reg(reg), .. }) => self.inst(Inst::Load {
                            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: *reg },
                            addr,
                        }),
                        Val::Float { reg, .. } => {
                            self.inst(Inst::Load { mem: Memory::Fp { bytes: size as u8, reg: *reg }, addr })
                        }
                        Val::Ptr(_) => return refuse("a pointer parameter that is not a register"),
                    }
                }
                _ => return refuse("a parameter in the wrong register class"),
            }
            // What the attributes promise of its extension.
            let value = match value {
                Val::Int(g) if has_flag(&attrs, "zeroext") => Val::Int(G { known: Known::Zero, ..g }),
                Val::Int(g) if has_flag(&attrs, "signext") => Val::Int(G { known: Known::Sign, ..g }),
                other => other,
            };
            self.vals.insert(parameter, value);
        }
        Ok(())
    }

    /// `value` as the 32- or 64-bit register of the convention's `scalar`,
    /// extended as the attributes ask.
    fn argument_value(
        &mut self,
        scalar: Scalar,
        src: Src,
        attrs: &[Attribute],
    ) -> Selected<Val> {
        let value = self.materialize(src)?;
        Ok(match value {
            Val::Int(g) => {
                let g = if g.bits < 32 && has_flag(attrs, "zeroext") {
                    self.extended(g, Known::Zero)
                } else if g.bits < 32 && has_flag(attrs, "signext") {
                    self.extended(g, Known::Sign)
                } else {
                    g
                };
                let _ = scalar;
                Val::Int(g)
            }
            other => other,
        })
    }

    pub(super) fn call(
        &mut self,
        inst: InstId,
        instruction: &Instruction,
        info: &CallInfo,
    ) -> Selected<()> {
        let _ = inst;
        let (&callee_operand, arguments) = instruction.operands.split_last().expect("a call has a callee");
        if let Operand::Constant(id) = callee_operand
            && let llrm_mir::ConstantKind::Global(global) = self.module.context.get(id).kind
            && let Some(name) = self.module.global(global).name.clone()
            && let Some(done) = self.intrinsic(&name, instruction, arguments)?
        {
            return Ok(done);
        }
        let (returns, parameters, variadic) = {
            let (r, p, v) = self.module.signature(info.function_type);
            (r, p.len(), v)
        };
        let _ = variadic;
        let mut assigner = Assigner::new(self.platform);
        let mut uses = Vec::new();
        let mut stores: Vec<(Place, Val, Scalar)> = Vec::new();
        for (position, &argument) in arguments.iter().enumerate() {
            let ty = self.operand_type(argument)?;
            let scalar = self.scalar(ty)?;
            let attrs = info.argument_attrs.get(position).cloned().unwrap_or_default();
            let src = self.src(argument)?;
            let value = self.argument_value(scalar, src, &attrs)?;
            let place = assigner.assign(scalar, position < parameters);
            stores.push((place, value, scalar));
        }
        // The stack arguments are stored first: they use scratch registers the
        // register arguments' copies must not be between.
        for (place, value, scalar) in &stores {
            if let Place::Stack { offset, size } = *place {
                let addr = Addr::Offset { base: Gpr::R31, imm: offset as i64 };
                match value {
                    Val::Int(g) => self.inst(Inst::Store {
                        mem: Memory::Int {
                            width: if size == 8 { Width::X } else { Width::W },
                            bytes: size as u8,
                            sign: false,
                            reg: g.reg,
                        },
                        addr,
                    }),
                    Val::Ptr(address) => {
                        let reg = self.pointer_register(address)?;
                        self.inst(Inst::Store {
                            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg },
                            addr,
                        });
                    }
                    Val::Float { reg, .. } => {
                        self.inst(Inst::Store { mem: Memory::Fp { bytes: size as u8, reg: *reg }, addr })
                    }
                }
                let _ = scalar;
            }
        }
        self.mf.outgoing = self.mf.outgoing.max(assigner.stack_bytes().next_multiple_of(16));
        for (place, value, _) in &stores {
            match (place, value) {
                (Place::Gpr(number), Val::Int(g)) => {
                    self.emit(Op::Copy { dst: Reg::Gpr(Gpr(*number)), src: Reg::Gpr(g.reg) });
                    uses.push(Reg::Gpr(Gpr(*number)));
                }
                (Place::Gpr(number), Val::Ptr(address)) => {
                    let reg = self.pointer_register(address)?;
                    self.emit(Op::Copy { dst: Reg::Gpr(Gpr(*number)), src: Reg::Gpr(reg) });
                    uses.push(Reg::Gpr(Gpr(*number)));
                }
                (Place::Fpr(number), Val::Float { reg, .. }) => {
                    self.emit(Op::Copy { dst: Reg::Fpr(Vreg(*number)), src: Reg::Fpr(*reg) });
                    uses.push(Reg::Fpr(Vreg(*number)));
                }
                (Place::Stack { .. }, _) => {}
                _ => return refuse("an argument in the wrong register class"),
            }
        }
        // The callee: a symbol, or a register.
        let callee = match callee_operand {
            Operand::Constant(id) => match self.module.context.get(id).kind {
                llrm_mir::ConstantKind::Global(global) => {
                    if let GlobalKind::Variable(_) = self.module.global(global).kind {
                        return refuse("a call to a variable");
                    }
                    Callee::Symbol(self.symbol(global)?)
                }
                _ => return refuse("a call through a constant expression"),
            },
            other => {
                let g = self.operand_g(other)?;
                Callee::Register(g.reg)
            }
        };
        self.mf.calls = true;
        let mut defs = Vec::new();
        let result_register = if self.types().is_void(returns) {
            None
        } else {
            let scalar = self.scalar(returns)?;
            Some(match crate::cc::result(scalar) {
                Place::Gpr(number) => Reg::Gpr(Gpr(number)),
                Place::Fpr(number) => Reg::Fpr(Vreg(number)),
                Place::Stack { .. } => unreachable!("a result is in a register"),
            })
        };
        defs.extend(result_register);
        self.emit(Op::Call { callee, uses, defs });
        if let (Some(register), Some(result)) = (result_register, instruction.result) {
            let value = self.fresh(returns)?;
            let dst = match (&value, register) {
                (Val::Int(g), _) => Reg::Gpr(g.reg),
                (Val::Ptr(AddrExpr { base: Base::Reg(reg), .. }), _) => Reg::Gpr(*reg),
                (Val::Float { reg, .. }, _) => Reg::Fpr(*reg),
                _ => return refuse("a call result that is not a register"),
            };
            self.emit(Op::Copy { dst, src: register });
            // A result narrower than 32 bits is as the callee left it.
            let value = match value {
                Val::Int(g) => Val::Int(G { known: Known::Nothing, ..g }),
                other => other,
            };
            self.vals.insert(result, value);
        }
        Ok(())
    }

    pub(super) fn ret(
        &mut self,
        instruction: &Instruction,
    ) -> Selected<()> {
        let mut uses = Vec::new();
        if let Some(&operand) = instruction.operands.first() {
            let ty = self.operand_type(operand)?;
            let scalar = self.scalar(ty)?;
            let src = self.src(operand)?;
            let attrs = self.function.return_attrs.clone();
            let value = self.argument_value(scalar, src, &attrs)?;
            match (crate::cc::result(scalar), value) {
                (Place::Gpr(number), Val::Int(g)) => {
                    self.emit(Op::Copy { dst: Reg::Gpr(Gpr(number)), src: Reg::Gpr(g.reg) });
                    uses.push(Reg::Gpr(Gpr(number)));
                }
                (Place::Gpr(number), Val::Ptr(address)) => {
                    let reg = self.pointer_register(&address)?;
                    self.emit(Op::Copy { dst: Reg::Gpr(Gpr(number)), src: Reg::Gpr(reg) });
                    uses.push(Reg::Gpr(Gpr(number)));
                }
                (Place::Fpr(number), Val::Float { reg, .. }) => {
                    self.emit(Op::Copy { dst: Reg::Fpr(Vreg(number)), src: Reg::Fpr(reg) });
                    uses.push(Reg::Fpr(Vreg(number)));
                }
                _ => return refuse("a result in the wrong register class"),
            }
        }
        self.emit(Op::Return { uses });
        Ok(())
    }

    /// A call the selector makes inline or drops, by the callee's name; `None`
    /// to call it.
    fn intrinsic(
        &mut self,
        name: &str,
        instruction: &Instruction,
        arguments: &[Operand],
    ) -> Selected<Option<()>> {
        let result = instruction.result;
        let base = name.strip_prefix("llvm.").map(|rest| rest.split('.').next().unwrap_or(rest));
        match base {
            Some("lifetime" | "assume" | "dbg" | "donothing" | "invariant") => Ok(Some(())),
            Some("trap") => {
                self.inst(Inst::Brk { imm16: 1 });
                Ok(Some(()))
            }
            Some("smax" | "smin" | "umax" | "umin") => {
                let signed = name.contains(".s");
                let max = name.contains("max");
                let ty = self.operand_type(arguments[0])?;
                let Some(bits) = self.types().int_bits(ty) else { return refuse("a min or max of a non-integer") };
                let predicate = match (signed, max) {
                    (true, true) => llrm_mir::IntPredicate::Sgt,
                    (true, false) => llrm_mir::IntPredicate::Slt,
                    (false, true) => llrm_mir::IntPredicate::Ugt,
                    (false, false) => llrm_mir::IntPredicate::Ult,
                };
                let cond = self.compare(predicate, arguments[0], arguments[1])?;
                let (a, b) = (self.operand_g(arguments[0])?, self.operand_g(arguments[1])?);
                let rd = self.mf.gpr();
                self.inst(Inst::CondSelect {
                    op: Select::Csel,
                    width: super::val::width_of(bits),
                    rd,
                    rn: a.reg,
                    rm: b.reg,
                    cond,
                });
                let known = if bits <= 32 { Known::Hi32Zero } else { Known::Nothing };
                self.define(result, Src::Val(Val::Int(G { reg: rd, bits, known })))?;
                Ok(Some(()))
            }
            _ => Ok(None),
        }
    }
}

#[allow(dead_code)]
fn opcode_is_call(opcode: &Opcode) -> bool {
    matches!(opcode, Opcode::Call(_))
}
