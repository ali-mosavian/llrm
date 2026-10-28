//! Parameters, calls and returns: where each value goes under the platform's
//! convention (`crate::cc`), and the copies between those registers and the
//! function's own.

use llrm_mir::{Attribute, CallInfo, GlobalKind, InstId, Instruction, Opcode, Operand};

use super::val::{AddrExpr, Base, G, Known, Val};
use super::{Selected, Selector, Src, refuse};
use crate::cc::{Assigner, Place};
use crate::machine::Platform;
use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{Callee, INCOMING_ARGUMENTS, Op};

fn has_flag(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name))
}

/// What `attrs` promise of a value narrower than 32 bits: `zeroext` and
/// `signext` say its upper bits are extended, as `AssertZext`/`AssertSext` do.
fn promised_extension(attrs: &[Attribute]) -> Known {
    if has_flag(attrs, "zeroext") {
        Known::Zero
    } else if has_flag(attrs, "signext") {
        Known::Sign
    } else {
        Known::Nothing
    }
}

/// Where the register save areas of a variadic function lie and where its
/// variadic arguments begin, for `va_start`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VaInfo {
    /// Linux: the frame objects the unnamed `x` and `q` argument registers are
    /// saved in, with their sizes.
    gr_slot: Option<(u32, u64)>,
    vr_slot: Option<(u32, u64)>,
    /// The bytes of stack the named arguments take.
    named_stack: u64,
}

fn is_byval(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Type(name, _) if name == "byval"))
}

fn is_sret(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Type(name, _) if name == "sret"))
}

impl Selector<'_> {
    /// Where the convention puts the scalars of a value of type `ty`.
    fn places_of(
        &mut self,
        assigner: &mut Assigner,
        ty: llrm_mir::TypeId,
        fixed: bool,
    ) -> Selected<Vec<Place>> {
        let leaves = self.leaf_types(ty)?;
        let scalars = leaves.iter().map(|leaf| self.scalar(*leaf)).collect::<Selected<Vec<_>>>()?;
        if !self.is_aggregate(ty) {
            return Ok(vec![assigner.assign(scalars[0], fixed)]);
        }
        let homogeneous = scalars.windows(2).all(|pair| pair[0] == pair[1]);
        if homogeneous && !scalars.is_empty() && (fixed || self.platform == Platform::Linux) {
            let size = self.layout.alloc_size(self.types(), leaves[0]);
            let align = self.layout.align(self.types(), ty).min(16);
            return Ok(assigner.assign_block(scalars[0], scalars.len(), size, align));
        }
        Ok(scalars.into_iter().map(|scalar| assigner.assign(scalar, fixed)).collect())
    }

    /// `leaf`, a scalar in the register `place` holds on entry, copied into its
    /// own.
    fn copy_in(
        &mut self,
        place: Place,
        leaf: &Val,
    ) -> Selected<()> {
        match (place, leaf) {
            (Place::Gpr(number), Val::Int(g)) => {
                self.emit(Op::Copy { dst: Reg::Gpr(g.reg), src: Reg::Gpr(Gpr(number)) });
            }
            (Place::Gpr(number), Val::Ptr(AddrExpr { base: Base::Reg(reg), .. })) => {
                self.emit(Op::Copy { dst: Reg::Gpr(*reg), src: Reg::Gpr(Gpr(number)) });
            }
            (Place::Fpr(number), Val::Float { reg, .. }) => {
                self.emit(Op::Copy { dst: Reg::Fpr(*reg), src: Reg::Fpr(Vreg(number)) });
            }
            (Place::Stack { offset, size }, _) => {
                let addr = Addr::Slot { slot: INCOMING_ARGUMENTS, offset: offset as i64 };
                match leaf {
                    Val::Int(g) => self.inst(Inst::Load {
                        mem: Memory::Int {
                            width: if size == 8 { Width::X } else { Width::W },
                            bytes: size as u8,
                            sign: false,
                            reg: g.reg,
                        },
                        addr, mmo: MemOperand::default()
                    }),
                    Val::Ptr(AddrExpr { base: Base::Reg(reg), .. }) => self.inst(Inst::Load {
                        mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: *reg },
                        addr, mmo: MemOperand::default()
                    }),
                    Val::Float { reg, .. } => {
                        self.inst(Inst::Load { mem: Memory::Fp { bytes: size as u8, reg: *reg }, addr, mmo: MemOperand::default() })
                    }
                    _ => return refuse("a parameter that is not a register"),
                }
            }
            _ => return refuse("a parameter in the wrong register class"),
        }
        Ok(())
    }

    /// The incoming arguments, copied into virtual registers at the entry.
    pub(super) fn parameters(&mut self) -> Selected<()> {
        let mut assigner = Assigner::new(self.platform);
        let parameters: Vec<_> = self.function.parameters().to_vec();
        for (position, &parameter) in parameters.iter().enumerate() {
            let ty = self.function.value(parameter).ty;
            let attrs = self.function.parameter_attrs.get(position).cloned().unwrap_or_default();
            let places = if is_sret(&attrs) {
                vec![Place::Gpr(Assigner::RESULT_POINTER)]
            } else {
                self.places_of(&mut assigner, ty, true)?
            };
            let mut value = self.fresh(ty)?;
            for (place, leaf) in places.iter().zip(value.leaves_mut()) {
                self.copy_in(*place, leaf)?;
            }
            // What the attributes promise of its extension.
            let value = match value {
                Val::Int(g) if promised_extension(&attrs) != Known::Nothing => {
                    Val::Int(G { known: promised_extension(&attrs), ..g })
                }
                other => other,
            };
            self.vals.insert(parameter, value);
        }
        if self.module.signature(self.function.ty).2 {
            self.variadic_entry(&assigner)?;
        }
        Ok(())
    }

    /// A variadic function's entry: on Linux the unnamed argument registers are
    /// saved where `va_arg` finds them.
    fn variadic_entry(
        &mut self,
        assigner: &Assigner,
    ) -> Selected<()> {
        let mut info = VaInfo { gr_slot: None, vr_slot: None, named_stack: assigner.stack_bytes().next_multiple_of(8) };
        if self.platform == Platform::Linux {
            let (gprs, fprs) = assigner.registers_used();
            let gr_size = u64::from(8 - gprs) * 8;
            let vr_size = u64::from(8 - fprs) * 16;
            if gr_size > 0 {
                let slot = self.mf.slot(gr_size, 8);
                for (at, number) in (gprs..8).enumerate() {
                    self.inst(Inst::Store {
                        mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(number) },
                        addr: Addr::Slot { slot, offset: at as i64 * 8 },
                        mmo: MemOperand::default(),
                    });
                }
                info.gr_slot = Some((slot, gr_size));
            }
            if vr_size > 0 {
                let slot = self.mf.slot(vr_size, 16);
                for (at, number) in (fprs..8).enumerate() {
                    self.inst(Inst::Store {
                        mem: Memory::Fp { bytes: 16, reg: Vreg(number) },
                        addr: Addr::Slot { slot, offset: at as i64 * 16 },
                        mmo: MemOperand::default(),
                    });
                }
                info.vr_slot = Some((slot, vr_size));
            }
        }
        self.va = Some(info);
        Ok(())
    }

    /// The values of `argument`'s scalars for the registers, extended as the
    /// attributes ask.
    fn argument_value(
        &mut self,
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
            && let Some(name) = self.module.global(global).name.owned()
            && name.starts_with("llvm.")
        {
            return self.intrinsic(&name, instruction, arguments);
        }
        if let Operand::Constant(id) = callee_operand
            && let llrm_mir::ConstantKind::Global(global) = self.module.context.get(id).kind
            && let Some(name) = self.module.global(global).name.owned()
            && name.starts_with("llrm.atomic.")
        {
            return self.atomic_call(&name, instruction);
        }
        let (returns, fixed, _) = {
            let (r, p, v) = self.module.signature(info.function_type);
            (r, p.len(), v)
        };
        let mut assigner = Assigner::new(self.platform);
        let mut passed = Vec::new();
        for (position, &argument) in arguments.iter().enumerate() {
            let ty = self.operand_type(argument)?;
            let attrs = info.argument_attrs.get(position).cloned().unwrap_or_default();
            let src = self.src(argument)?;
            let value = self.argument_value(src, &attrs)?;
            let places = if is_sret(&attrs) {
                vec![Place::Gpr(Assigner::RESULT_POINTER)]
            } else {
                self.places_of(&mut assigner, ty, position < fixed)?
            };
            passed.push((value, places));
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
        let result = self.emit_call(
            callee,
            passed,
            assigner.stack_bytes(),
            returns,
            promised_extension(&info.return_attrs),
            self.sibling_call_eligible(instruction, info),
        )?;
        if let (Some(value), Some(result_id)) = (result, instruction.result) {
            self.vals.insert(result_id, value);
        }
        Ok(())
    }

    /// Whether the call may become a jump (`isEligibleForTailCallOptimization`): marked `tail`, the
    /// caller's calling convention (`ccc`/`fastcc` pass alike) and return attributes, no hidden result
    /// pointer or `byval` argument on either side, and a caller whose frame is just its own.
    fn sibling_call_eligible(
        &self,
        instruction: &Instruction,
        info: &CallInfo,
    ) -> bool {
        matches!(info.tail, llrm_mir::opcode::Tail::Tail | llrm_mir::opcode::Tail::MustTail)
            && matches!(instruction.opcode, Opcode::Call(_))
            && !self.mf.dynamic
            && self.va.is_none()
            && [info.calling_convention, self.function.calling_convention].iter().all(|cc| matches!(cc, 0 | 8))
            && info.return_attrs == self.function.return_attrs
            && !info.argument_attrs.iter().chain(&self.function.parameter_attrs).any(|attrs| is_sret(attrs) || is_byval(attrs))
    }

    /// `leaf` moved to the register or stack slot `place`; the register it
    /// needs held through the call, if one.
    fn copy_out(
        &mut self,
        place: Place,
        leaf: &Val,
        uses: &mut Vec<Reg>,
    ) -> Selected<()> {
        match (place, leaf) {
            (Place::Gpr(number), Val::Int(g)) => {
                self.emit(Op::Copy { dst: Reg::Gpr(Gpr(number)), src: Reg::Gpr(g.reg) });
                uses.push(Reg::Gpr(Gpr(number)));
            }
            (Place::Gpr(number), Val::Ptr(address)) => {
                let reg = self.pointer_register(address)?;
                self.emit(Op::Copy { dst: Reg::Gpr(Gpr(number)), src: Reg::Gpr(reg) });
                uses.push(Reg::Gpr(Gpr(number)));
            }
            (Place::Fpr(number), Val::Float { reg, .. }) => {
                self.emit(Op::Copy { dst: Reg::Fpr(Vreg(number)), src: Reg::Fpr(*reg) });
                uses.push(Reg::Fpr(Vreg(number)));
            }
            (Place::Stack { .. }, _) => {}
            _ => return refuse("an argument in the wrong register class"),
        }
        Ok(())
    }

    fn store_outgoing(
        &mut self,
        place: Place,
        leaf: &Val,
    ) -> Selected<()> {
        let Place::Stack { offset, size } = place else { return Ok(()) };
        let addr = Addr::Offset { base: Gpr::R31, imm: offset as i64 };
        match leaf {
            Val::Int(g) => self.inst(Inst::Store {
                mem: Memory::Int {
                    width: if size == 8 { Width::X } else { Width::W },
                    bytes: size as u8,
                    sign: false,
                    reg: g.reg,
                },
                addr, mmo: MemOperand::default()
            }),
            Val::Ptr(address) => {
                let reg = self.pointer_register(address)?;
                self.inst(Inst::Store { mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg }, addr, mmo: MemOperand::default() });
            }
            Val::Float { reg, .. } => self.inst(Inst::Store { mem: Memory::Fp { bytes: size as u8, reg: *reg }, addr, mmo: MemOperand::default() }),
            Val::Agg(_) => return refuse("an aggregate leaf"),
        }
        Ok(())
    }

    /// A call of `callee` with `passed` (each argument's value and the places
    /// of its scalars) and `stack` bytes of stack arguments, returning type
    /// `returns`; the result in new registers.
    pub(crate) fn emit_call(
        &mut self,
        callee: Callee,
        passed: Vec<(Val, Vec<Place>)>,
        stack: u64,
        returns: llrm_mir::TypeId,
        extension: Known,
        tail: bool,
    ) -> Selected<Option<Val>> {
        let mut uses = Vec::new();
        // With a moving stack pointer the room for the stack arguments is made
        // around the call.
        let room = stack.next_multiple_of(16);
        let adjust = |this: &mut Self, subtract: bool| {
            if this.mf.dynamic && room > 0 {
                this.inst(Inst::AddSubImm {
                    op: AddSub { subtract, set_flags: false },
                    width: Width::X,
                    rd: Gpr::R31,
                    rn: Gpr::R31,
                    imm12: room as u16,
                    shift12: false,
                });
            }
        };
        adjust(self, true);
        // The stack arguments are stored first.
        for (value, places) in &passed {
            for (place, leaf) in places.iter().zip(value.leaves()) {
                self.store_outgoing(*place, leaf)?;
            }
        }
        if !self.mf.dynamic {
            self.mf.outgoing = self.mf.outgoing.max(room);
        }
        for (value, places) in &passed {
            for (place, leaf) in places.iter().zip(value.leaves()) {
                self.copy_out(*place, leaf, &mut uses)?;
            }
        }
        self.mf.calls = true;
        let (result_places, defs) = if self.types().is_void(returns) {
            (Vec::new(), Vec::new())
        } else {
            let places = self.places_of(&mut Assigner::new(self.platform), returns, true)?;
            let defs = places
                .iter()
                .map(|place| match place {
                    Place::Gpr(number) => Ok(Reg::Gpr(Gpr(*number))),
                    Place::Fpr(number) => Ok(Reg::Fpr(Vreg(*number))),
                    Place::Stack { .. } => refuse("a result that does not fit the registers"),
                })
                .collect::<Selected<Vec<_>>>()?;
            (places, defs)
        };
        self.emit(Op::Call { callee, uses, defs: defs.clone(), tail: tail && room == 0 && !self.mf.dynamic });
        adjust(self, false);
        if result_places.is_empty() {
            return Ok(None);
        }
        let mut value = self.fresh(returns)?;
        let leaf_types = self.leaf_types(returns)?;
        for ((register, leaf), leaf_type) in defs.iter().zip(value.leaves_mut()).zip(leaf_types) {
            let _ = leaf_type;
            let dst = match leaf {
                Val::Int(g) => {
                    // A result narrower than 32 bits is as the callee left it:
                    // extended when the call says so.
                    g.known = extension;
                    Reg::Gpr(g.reg)
                }
                Val::Ptr(AddrExpr { base: Base::Reg(reg), .. }) => Reg::Gpr(*reg),
                Val::Float { reg, .. } => Reg::Fpr(*reg),
                _ => return refuse("a call result that is not a register"),
            };
            self.emit(Op::Copy { dst, src: *register });
        }
        Ok(Some(value))
    }

    pub(super) fn ret(
        &mut self,
        instruction: &Instruction,
    ) -> Selected<()> {
        let mut uses = Vec::new();
        if let Some(&operand) = instruction.operands.first() {
            let ty = self.operand_type(operand)?;
            let src = self.src(operand)?;
            let attrs = self.function.return_attrs.clone();
            let value = self.argument_value(src, &attrs)?;
            let places = self.places_of(&mut Assigner::new(self.platform), ty, true)?;
            for (place, leaf) in places.iter().zip(value.leaves()) {
                self.copy_out(*place, leaf, &mut uses)?;
            }
        }
        self.emit(Op::Return { uses, tail: None });
        Ok(())
    }

    /// A call of an `llvm.` intrinsic: made inline, a library call, or dropped.
    fn intrinsic(
        &mut self,
        name: &str,
        instruction: &Instruction,
        arguments: &[Operand],
    ) -> Selected<()> {
        let result = instruction.result;
        let base = name.strip_prefix("llvm.").map(|rest| rest.split('.').next().unwrap_or(rest)).unwrap_or("");
        match base {
            "lifetime" | "assume" | "dbg" | "donothing" | "invariant" => Ok(()),
            // What is left unfolded by now is not a constant, so only a literal operand makes it true.
            "is" if name.starts_with("llvm.is.constant") => {
                let known = matches!(self.src(arguments[0])?, Src::Int { .. } | Src::Float { .. });
                self.define(result, Src::Int { bits: 1, value: u128::from(known) })
            }
            "expect" => {
                let value = self.src(arguments[0])?;
                self.define(result, value)
            }
            "trap" => {
                self.inst(Inst::Brk { imm16: 1 });
                Ok(())
            }
            // The operation as AArch64ISelLowering's LowerPREFETCH spells it: write, instruction cache, level, stream.
            "prefetch" => {
                let constant = |select: &mut Self, at: usize| match select.src(arguments[at])? {
                    Src::Int { value, .. } => Ok(value as u8),
                    _ => refuse("a prefetch whose kind is not a constant"),
                };
                let (write, locality, data) = (constant(self, 1)?, constant(self, 2)?, constant(self, 3)?);
                let level = if locality == 0 { 0 } else { 3 - locality };
                let op = write << 4 | u8::from(data == 0) << 3 | level << 1 | u8::from(locality == 0);
                let address = self.pointer_operand(arguments[0])?;
                let base = self.pointer_register(&address)?;
                self.inst(Inst::Prefetch { op, base });
                Ok(())
            }
            "smax" | "smin" | "umax" | "umin" => {
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
                self.define(result, Src::Val(Val::Int(G { reg: rd, bits, known })))
            }
            "fmuladd" | "fma" => {
                let (a, b, c) =
                    (self.float_arg(arguments[0])?, self.float_arg(arguments[1])?, self.float_arg(arguments[2])?);
                let rd = self.mf.fpr();
                self.inst(Inst::FpFused { op: FpFused::Madd, size: a.1, rd, rn: a.0, rm: b.0, ra: c.0 });
                self.define(result, Src::Val(Val::Float { reg: rd, size: a.1 }))
            }
            "fabs" | "sqrt" | "floor" | "ceil" | "trunc" | "round" | "rint" | "nearbyint" | "roundeven" => {
                let (rn, size) = self.float_arg(arguments[0])?;
                let op = match base {
                    "fabs" => FpUnary::Abs,
                    "sqrt" => FpUnary::Sqrt,
                    "floor" => FpUnary::Rint(Rounding::Down),
                    "ceil" => FpUnary::Rint(Rounding::Up),
                    "trunc" => FpUnary::Rint(Rounding::Zero),
                    "round" => FpUnary::Rint(Rounding::Away),
                    "rint" => FpUnary::Rint(Rounding::Exact),
                    "nearbyint" => FpUnary::Rint(Rounding::Current),
                    _ => FpUnary::Rint(Rounding::Nearest),
                };
                let rd = self.mf.fpr();
                self.inst(Inst::FpUnary { op, size, rd, rn });
                self.define(result, Src::Val(Val::Float { reg: rd, size }))
            }
            // The current rounding mode (to nearest even) and then the
            // conversion: BASIC's CINT.
            "lrint" | "llrint" => {
                let (rn, size) = self.float_arg(arguments[0])?;
                let Some(bits) = self.types().int_bits(instruction.ty) else {
                    return refuse("an lrint to a non-integer");
                };
                let rounded = self.mf.fpr();
                self.inst(Inst::FpUnary { op: FpUnary::Rint(Rounding::Exact), size, rd: rounded, rn });
                let rd = self.mf.gpr();
                self.inst(Inst::FpInt {
                    op: FpConvert::ToIntZero { signed: true },
                    width: super::val::width_of(bits),
                    size,
                    gpr: rd,
                    fpr: rounded,
                });
                let known = if bits == 32 {
                    Known::Zero
                } else if bits < 32 {
                    Known::Hi32Zero
                } else {
                    Known::Nothing
                };
                self.define(result, Src::Val(Val::Int(G { reg: rd, bits, known })))
            }
            "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2" | "sinh" | "cosh" | "tanh" | "exp" | "exp2"
            | "log" | "log2" | "log10" | "pow" => self.libm_intrinsic(base, instruction, arguments),
            "bswap" | "bitreverse" | "ctlz" | "cttz" | "abs" => self.bit_intrinsic(base, instruction, arguments),
            "memcpy" | "memmove" | "memset" => {
                if self.inline_memory(base, arguments)? {
                    return Ok(());
                }
                // The last argument is the volatile flag; the call is
                // `memcpy(dst, src, n)`.
                let mut assigner = Assigner::new(self.platform);
                let mut passed = Vec::new();
                for &argument in &arguments[..3] {
                    let ty = self.operand_type(argument)?;
                    let src = self.src(argument)?;
                    let value = self.argument_value(src, &[])?;
                    let places = self.places_of(&mut assigner, ty, true)?;
                    passed.push((value, places));
                }
                let void = self.types().void();
                let callee = Callee::Symbol(crate::data::symbol_name(self.platform, base));
                self.emit_call(callee, passed, assigner.stack_bytes(), void, Known::Nothing, false)?;
                Ok(())
            }
            "stacksave" => {
                let reg = self.mf.gpr();
                self.inst(Inst::AddSubImm {
                    op: AddSub { subtract: false, set_flags: false },
                    width: Width::X,
                    rd: reg,
                    rn: Gpr::R31,
                    imm12: 0,
                    shift12: false,
                });
                self.define(result, Src::Val(Val::Ptr(AddrExpr::of(Base::Reg(reg)))))
            }
            "stackrestore" => {
                let saved = self.operand_g(arguments[0])?;
                self.inst(Inst::AddSubImm {
                    op: AddSub { subtract: false, set_flags: false },
                    width: Width::X,
                    rd: Gpr::R31,
                    rn: saved.reg,
                    imm12: 0,
                    shift12: false,
                });
                Ok(())
            }
            "va_start" => self.va_start(arguments[0]),
            "va_end" => Ok(()),
            "va_copy" => self.va_copy(arguments[0], arguments[1]),
            _ => refuse(format!("the intrinsic {name}")),
        }
    }

    /// The intrinsic `base` as the C library's function of that name (`sin`, or
    /// `sinf` for a float).
    fn libm_intrinsic(
        &mut self,
        base: &str,
        instruction: &Instruction,
        arguments: &[Operand],
    ) -> Selected<()> {
        let mut assigner = Assigner::new(self.platform);
        let mut passed = Vec::new();
        for &argument in arguments {
            let ty = self.operand_type(argument)?;
            let src = self.src(argument)?;
            let value = self.argument_value(src, &[])?;
            let places = self.places_of(&mut assigner, ty, true)?;
            passed.push((value, places));
        }
        let single = matches!(
            self.ty(instruction.ty),
            llrm_mir::Type::Float(llrm_mir::FloatKind::Float)
        );
        let name = format!("{base}{}", if single { "f" } else { "" });
        let callee = Callee::Symbol(crate::data::symbol_name(self.platform, &name));
        let result = self.emit_call(callee, passed, assigner.stack_bytes(), instruction.ty, Known::Nothing, false)?;
        if let (Some(value), Some(result_id)) = (result, instruction.result) {
            self.vals.insert(result_id, value);
        }
        Ok(())
    }

    /// `va_start(ap)`: Darwin's list is a pointer to the first unnamed stack
    /// argument; AAPCS64's is five fields (`stack`, `gr_top`, `vr_top`,
    /// `gr_offs`, `vr_offs`) over the register save areas.
    fn va_start(
        &mut self,
        list: Operand,
    ) -> Selected<()> {
        let Some(info) = self.va else { return refuse("va_start in a function that is not variadic") };
        let list = match self.src(list)? {
            Src::Val(Val::Ptr(address)) => address,
            _ => return refuse("a va_list that is not an address"),
        };
        let stack = self.mf.gpr();
        self.emit(Op::FrameAddress { dst: stack, slot: INCOMING_ARGUMENTS, offset: info.named_stack as i64 });
        let store = |this: &mut Self, reg: Gpr, bytes: u8, offset: i64| -> Selected<()> {
            let at = AddrExpr { offset: list.offset + offset, ..list.clone() };
            let addr = this.addressing(&at, bytes)?;
            this.inst(Inst::Store {
                mem: Memory::Int { width: if bytes == 8 { Width::X } else { Width::W }, bytes, sign: false, reg },
                addr, mmo: MemOperand::default()
            });
            Ok(())
        };
        store(self, stack, 8, 0)?;
        if self.platform == Platform::Linux {
            let top = |this: &mut Self, area: Option<(u32, u64)>, offset: i64| -> Selected<u64> {
                let reg = this.mf.gpr();
                let size = match area {
                    Some((slot, size)) => {
                        this.emit(Op::FrameAddress { dst: reg, slot, offset: size as i64 });
                        size
                    }
                    None => {
                        this.load_immediate(reg, 64, 0);
                        0
                    }
                };
                store(this, reg, 8, offset)?;
                Ok(size)
            };
            let gr = top(self, info.gr_slot, 8)?;
            let vr = top(self, info.vr_slot, 16)?;
            for (offset, size) in [(24, gr), (28, vr)] {
                let reg = self.mf.gpr();
                self.load_immediate(reg, 32, (-(size as i64)) as u64);
                store(self, reg, 4, offset)?;
            }
        }
        Ok(())
    }

    /// `va_copy(to, from)`: the list's bytes.
    fn va_copy(
        &mut self,
        to: Operand,
        from: Operand,
    ) -> Selected<()> {
        let bytes: u64 = if self.platform == Platform::Linux { 32 } else { 8 };
        let (Src::Val(Val::Ptr(to)), Src::Val(Val::Ptr(from))) = (self.src(to)?, self.src(from)?) else {
            return refuse("a va_list that is not an address");
        };
        for offset in (0..bytes).step_by(8) {
            let source = AddrExpr { offset: from.offset + offset as i64, ..from.clone() };
            let target = AddrExpr { offset: to.offset + offset as i64, ..to.clone() };
            let reg = self.mf.gpr();
            let addr = self.addressing(&source, 8)?;
            self.inst(Inst::Load { mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg }, addr, mmo: MemOperand::default() });
            let addr = self.addressing(&target, 8)?;
            self.inst(Inst::Store { mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg }, addr, mmo: MemOperand::default() });
        }
        Ok(())
    }

    fn float_arg(
        &mut self,
        operand: Operand,
    ) -> Selected<(Vreg, FpSize)> {
        let src = self.src(operand)?;
        match self.materialize(src)? {
            Val::Float { reg, size } => Ok((reg, size)),
            _ => refuse("a float argument that is an integer"),
        }
    }

    /// `bswap`, `bitreverse`, `ctlz`, `cttz` and `abs` of an integer.
    fn bit_intrinsic(
        &mut self,
        base: &str,
        instruction: &Instruction,
        arguments: &[Operand],
    ) -> Selected<()> {
        let ty = self.operand_type(arguments[0])?;
        let Some(bits) = self.types().int_bits(ty) else { return refuse("a bit intrinsic of a non-integer") };
        if !matches!(bits, 16 | 32 | 64) && base != "abs" && base != "ctlz" && base != "cttz" {
            return refuse(format!("{base} of i{bits}"));
        }
        let width = super::val::width_of(bits);
        let g = self.operand_g(arguments[0])?;
        let rd = self.mf.gpr();
        let known = if bits <= 32 { Known::Hi32Zero } else { Known::Nothing };
        match base {
            "abs" => {
                let g = self.extended(g, Known::Sign);
                self.inst(Inst::AddSubImm {
                    op: AddSub { subtract: true, set_flags: true },
                    width,
                    rd: Gpr::R31,
                    rn: g.reg,
                    imm12: 0,
                    shift12: false,
                });
                // cneg rd, g, lt = csneg rd, g, g, ge
                self.inst(Inst::CondSelect { op: Select::Csneg, width, rd, rn: g.reg, rm: g.reg, cond: Cond::Ge });
            }
            "bswap" => {
                self.inst(Inst::OneSource { op: Unary::Rev, width, rd, rn: g.reg });
                if bits == 16 {
                    // The 32-bit reverse puts the bytes in the high half.
                    self.inst(Inst::BitfieldMove {
                        op: Bitfield::Ubfm,
                        width: Width::W,
                        rd,
                        rn: rd,
                        immr: 16,
                        imms: 31,
                    });
                }
            }
            "bitreverse" => self.inst(Inst::OneSource { op: Unary::Rbit, width, rd, rn: g.reg }),
            "ctlz" => {
                let g = self.extended(g, Known::Zero);
                self.inst(Inst::OneSource { op: Unary::Clz, width, rd, rn: g.reg });
                if bits < width.bits() {
                    // The leading zeros of the register count the padding.
                    self.inst(Inst::AddSubImm {
                        op: AddSub { subtract: true, set_flags: false },
                        width,
                        rd,
                        rn: rd,
                        imm12: (width.bits() - bits) as u16,
                        shift12: false,
                    });
                }
            }
            _ => {
                // cttz: reverse the bits, count the leading zeros.
                let reversed = self.mf.gpr();
                self.inst(Inst::OneSource { op: Unary::Rbit, width, rd: reversed, rn: g.reg });
                self.inst(Inst::OneSource { op: Unary::Clz, width, rd, rn: reversed });
                if bits < width.bits() {
                    // A zero argument counts the register's width; the type's
                    // is `bits`.
                    self.inst(Inst::AddSubImm {
                        op: AddSub { subtract: true, set_flags: true },
                        width,
                        rd: Gpr::R31,
                        rn: rd,
                        imm12: bits as u16,
                        shift12: false,
                    });
                    let limit = self.mf.gpr();
                    self.load_immediate(limit, bits, u64::from(bits));
                    self.inst(Inst::CondSelect { op: Select::Csel, width, rd, rn: limit, rm: rd, cond: Cond::Hi });
                }
            }
        }
        self.define(instruction.result, Src::Val(Val::Int(G { reg: rd, bits, known })))
    }
}

#[allow(dead_code)]
fn opcode_is_call(opcode: &Opcode) -> bool {
    matches!(opcode, Opcode::Call(_))
}
