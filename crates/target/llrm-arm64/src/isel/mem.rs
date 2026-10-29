//! Memory: frame objects, loads, stores, address arithmetic and globals. A
//! pointer is kept as an `AddrExpr` until something needs it in a register, so
//! a load or store takes the base, scaled index and offset into its addressing
//! mode (`AArch64DAGToDAGISel::SelectAddrModeIndexed` and its relatives). A
//! global every reader of which is a plain load or store of it keeps only its
//! `adrp` page, each access taking `@PAGEOFF` (`isWorthFoldingADDlow`).

use llrm_mir::{CastOp, InstId, Instruction, Opcode, Operand, Type, TypeId};

use super::val::{AddrExpr, Base, G, Index, Known, Val};
use super::{Selected, Selector, Src, refuse};
use crate::mc::inst::*;
use crate::mfunc::Op;

/// What an access at `address` reaches, so that two accesses can be told apart
/// later (`pairs`): the global or frame object it starts from and, where no
/// index moves it, the byte offset in it.
pub(super) fn memory_operand(address: &AddrExpr) -> MemOperand {
    let offset = address.index.is_none().then_some(address.offset);
    match &address.base {
        Base::Global { name, .. } => MemOperand { object: MemObject::Global(name.clone()), offset },
        Base::Slot(slot) => MemOperand { object: MemObject::Slot(*slot), offset },
        Base::Reg(_) => MemOperand::default(),
    }
}

impl Selector<'_> {
    /// Whether `inst`, an `alloca`, is a frame object: a constant count, in the
    /// entry block.
    pub(super) fn is_static_alloca(
        &self,
        inst: llrm_mir::InstId,
    ) -> bool {
        let instruction = self.function.instruction(inst);
        self.function.parent(inst) == self.function.entry()
            && instruction.operands.first().is_none_or(|operand| {
                matches!(
                    operand,
                    Operand::Constant(id) if matches!(self.module.context.get(*id).kind, llrm_mir::ConstantKind::Int(_))
                )
            })
    }

    pub(super) fn alloca(
        &mut self,
        inst: llrm_mir::InstId,
        instruction: &Instruction,
        allocated: TypeId,
        align: Option<u64>,
    ) -> Selected<()> {
        let element = self.layout.alloc_size(self.types(), allocated);
        let align = align.unwrap_or(0).max(self.layout.align(self.types(), allocated)).max(1);
        if self.is_static_alloca(inst) {
            let count = match instruction.operands.first() {
                None => 1,
                Some(operand) => match self.src(*operand)? {
                    Src::Int { value, .. } => value as u64,
                    _ => unreachable!("a static alloca has a constant count"),
                },
            };
            let slot = self.mf.slot((element * count).max(1), align);
            return self.define(instruction.result, Src::Val(Val::Ptr(AddrExpr::of(Base::Slot(slot)))));
        }
        if align > 16 {
            return refuse("a dynamic alloca aligned past 16");
        }
        // `sp -= (count * size + 15) & -16`, the object at the new `sp`.
        let count = match instruction.operands.first() {
            Some(&count) => {
                let count = self.operand_g(count)?;
                self.widened(count, 64, false).reg
            }
            // `alloca T` outside the entry block has no count operand.
            None => {
                let one = self.mf.gpr();
                self.load_immediate(one, 64, 1);
                one
            }
        };
        let bytes = self.mf.gpr();
        let size = self.mf.gpr();
        self.load_immediate(size, 64, element);
        self.inst(Inst::ThreeSource { op: Mul3::Madd, width: Width::X, rd: bytes, rn: count, rm: size, ra: Gpr::R31 });
        let rounded = self.mf.gpr();
        self.inst(Inst::AddSubImm {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: rounded,
            rn: bytes,
            imm12: 15,
            shift12: false,
        });
        let aligned = self.mf.gpr();
        self.inst(Inst::LogicImm { op: Logic::And, width: Width::X, rd: aligned, rn: rounded, imm: !15u64 });
        self.inst(Inst::AddSubExtended {
            op: AddSub { subtract: true, set_flags: false },
            width: Width::X,
            rd: Gpr::R31,
            rn: Gpr::R31,
            rm: aligned,
            extend: Extend::Uxtx,
            amount: 0,
        });
        let object = self.mf.gpr();
        self.inst(Inst::AddSubImm {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: object,
            rn: Gpr::R31,
            imm12: 0,
            shift12: false,
        });
        self.define(instruction.result, Src::Val(Val::Ptr(AddrExpr::of(Base::Reg(object)))))
    }

    /// `base + offset` as a register: the instructions that make an address a
    /// value.
    pub(crate) fn pointer_register(
        &mut self,
        address: &AddrExpr,
    ) -> Selected<Gpr> {
        let mut register = match &address.base {
            Base::Reg(reg) if address.offset == 0 && address.index.is_none() => return Ok(*reg),
            Base::Reg(reg) => *reg,
            Base::Slot(slot) => {
                let dst = self.mf.gpr();
                self.emit(Op::FrameAddress { dst, slot: *slot, offset: address.offset });
                return self.with_index(dst, address);
            }
            Base::Global { name, got } => {
                if let Some(shared) = self.shared_global(name, *got) {
                    return self.add_offset_and_index(shared, address);
                }
                let page = self.mf.gpr();
                // the page and the page offset name the same address: a GOT
                // slot has no offset
                let addend = if *got { 0 } else { address.offset };
                self.inst(Inst::Adr {
                    page: true,
                    rd: page,
                    target: Target::Symbol { name: name.clone(), addend },
                    got: *got,
                });
                let dst = self.mf.gpr();
                if *got {
                    self.inst(Inst::Load {
                        mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: dst },
                        addr: Addr::PageOffset {
                            base: page,
                            target: Target::Symbol { name: name.clone(), addend: 0 },
                            got: true,
                        },
                        mmo: MemOperand::default(),
                    });
                    // The offset is added to the loaded address.
                    return self.add_offset_and_index(dst, address);
                }
                self.inst(Inst::AddPageOffset {
                    rd: dst,
                    rn: page,
                    target: Target::Symbol { name: name.clone(), addend: address.offset },
                });
                return self.with_index(dst, address);
            }
        };
        if address.offset != 0 {
            register = self.add_constant(register, address.offset);
        }
        self.with_index(register, address)
    }

    /// The address of global `name` in a register this block already holds it
    /// in, or makes now, where the function names it more than once
    /// (clang's `adrp` + `add` once, each access then `[x, #offset]`); `None`
    /// where one access folds the symbol's offset into its own `adrp` for
    /// less.
    fn shared_global(
        &mut self,
        name: &str,
        got: bool,
    ) -> Option<Gpr> {
        if self.global_uses.get(name).copied().unwrap_or(0) < 2 {
            return None;
        }
        let key = (name.to_owned(), self.current);
        if let Some(register) = self.global_regs.get(&key) {
            return Some(*register);
        }
        let target = Target::Symbol { name: name.to_owned(), addend: 0 };
        let register = if got {
            let page = self.mf.gpr();
            self.inst(Inst::Adr { page: true, rd: page, target: target.clone(), got });
            let register = self.mf.gpr();
            self.inst(Inst::Load {
                mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: register },
                addr: Addr::PageOffset { base: page, target, got: true },
                mmo: MemOperand::default(),
            });
            register
        } else {
            let page = self.shared_page(name);
            let register = self.mf.gpr();
            self.inst(Inst::AddPageOffset { rd: register, rn: page, target });
            register
        };
        self.global_regs.insert(key, register);
        Some(register)
    }

    /// The page of global `name` in a register this block already holds it in,
    /// or makes now.
    fn shared_page(
        &mut self,
        name: &str,
    ) -> Gpr {
        let key = (name.to_owned(), self.current);
        if let Some(page) = self.global_pages.get(&key) {
            return *page;
        }
        let page = self.mf.gpr();
        self.inst(Inst::Adr {
            page: true,
            rd: page,
            target: Target::Symbol { name: name.to_owned(), addend: 0 },
            got: false,
        });
        self.global_pages.insert(key, page);
        page
    }

    fn add_offset_and_index(
        &mut self,
        register: Gpr,
        address: &AddrExpr,
    ) -> Selected<Gpr> {
        let register = if address.offset != 0 { self.add_constant(register, address.offset) } else { register };
        self.with_index(register, address)
    }

    fn with_index(
        &mut self,
        register: Gpr,
        address: &AddrExpr,
    ) -> Selected<Gpr> {
        let Some(index) = address.index else { return Ok(register) };
        let rd = self.mf.gpr();
        let extended = matches!(index.extend, Extend::Uxtw | Extend::Sxtw);
        if extended {
            self.inst(Inst::AddSubExtended {
                op: AddSub { subtract: false, set_flags: false },
                width: Width::X,
                rd,
                rn: register,
                rm: index.reg,
                extend: index.extend,
                amount: index.shift,
            });
        } else {
            self.inst(Inst::AddSubShifted {
                op: AddSub { subtract: false, set_flags: false },
                width: Width::X,
                rd,
                rn: register,
                rm: index.reg,
                shift: Shift::Lsl,
                amount: index.shift,
            });
        }
        Ok(rd)
    }

    /// `base + constant` in a new register.
    pub(crate) fn add_constant(
        &mut self,
        base: Gpr,
        offset: i64,
    ) -> Gpr {
        let rd = self.mf.gpr();
        let magnitude = offset.unsigned_abs();
        let subtract = offset < 0;
        let (imm12, shift12) = if magnitude < 4096 {
            (Some(magnitude as u16), false)
        } else if magnitude & 0xFFF == 0 && magnitude >> 12 < 4096 {
            (Some((magnitude >> 12) as u16), true)
        } else {
            (None, false)
        };
        if let Some(imm12) = imm12 {
            self.inst(Inst::AddSubImm {
                op: AddSub { subtract, set_flags: false },
                width: Width::X,
                rd,
                rn: base,
                imm12,
                shift12,
            });
        } else {
            let constant = self.mf.gpr();
            self.load_immediate(constant, 64, offset as u64);
            self.inst(Inst::AddSubShifted {
                op: AddSub { subtract: false, set_flags: false },
                width: Width::X,
                rd,
                rn: base,
                rm: constant,
                shift: Shift::Lsl,
                amount: 0,
            });
        }
        rd
    }

    /// The addressing mode for an access of `bytes` at `address`.
    pub(crate) fn addressing(
        &mut self,
        address: &AddrExpr,
        bytes: u8,
    ) -> Selected<Addr> {
        let scale = i64::from(bytes);
        // The access takes the symbol's page offset itself, so a global
        // accessed again holds only its page (clang's `adrp x8,
        // _g@PAGE` once, then `ldr w9, [x8, _g@PAGEOFF]`), not the address
        // `add` makes. Only where no other reader of the symbol needs
        // its address (`isWorthFoldingADDlow`), and at offset 0,
        // where the page is the symbol's (DEVIATE: the pin folds a GEP offset
        // as well).
        if let (Base::Global { name, got: false }, None, 0) = (&address.base, address.index, address.offset)
            && self.global_uses.get(name.as_str()).copied().unwrap_or(0) >= 2
            && !self.global_needs_address.contains(name.as_str())
        {
            let page = self.shared_page(name);
            return Ok(Addr::PageOffset {
                base: page,
                target: Target::Symbol { name: name.clone(), addend: 0 },
                got: false,
            });
        }
        if let (Base::Global { name, got }, None) = (&address.base, address.index) {
            if let Some(shared) = self.shared_global(name, *got) {
                return self.base_offset(shared, address.offset, scale);
            }
        }
        match (&address.base, address.index) {
            (Base::Slot(slot), None) => Ok(Addr::Slot { slot: *slot, offset: address.offset }),
            (Base::Reg(base), None) => self.base_offset(*base, address.offset, scale),
            (Base::Reg(base), Some(index))
                if address.offset == 0 && (index.shift == 0 || i64::from(1u8 << index.shift) == scale) =>
            {
                Ok(Addr::Register { base: *base, index: index.reg, extend: index.extend, shifted: index.shift != 0 })
            }
            // An offset the access takes as it is: the sum of base and index in
            // a register, the offset in the access. Accesses of one object at
            // several offsets share the sum (clang's
            // `add x8, x0, x1` once, then `[x8, #8]`, `[x8, #16]`).
            (_, Some(_)) if address.offset != 0 && offset_in_access(address.offset, scale) => {
                let base = self.pointer_register(&AddrExpr { index: None, offset: 0, ..address.clone() })?;
                let sum = self.with_index(base, address)?;
                Ok(Addr::Offset { base: sum, imm: address.offset })
            }
            // A global, frame object or offset base with an index: the base in
            // a register, the index in the access.
            (_, Some(index)) if index.shift == 0 || i64::from(1u8 << index.shift) == scale => {
                let base = self.pointer_register(&AddrExpr { index: None, ..address.clone() })?;
                Ok(Addr::Register { base, index: index.reg, extend: index.extend, shifted: index.shift != 0 })
            }
            // A page-offset relocation scales its offset by the access size: an
            // offset that is not a multiple of it (`load i16, ptr getelementptr
            // (@g, 1), align 1`) would lose its low bits.
            (Base::Global { name, got: false }, None) if address.offset % scale == 0 => {
                let page = self.mf.gpr();
                self.inst(Inst::Adr {
                    page: true,
                    rd: page,
                    target: Target::Symbol { name: name.clone(), addend: address.offset },
                    got: false,
                });
                Ok(Addr::PageOffset {
                    base: page,
                    target: Target::Symbol { name: name.clone(), addend: address.offset },
                    got: false,
                })
            }
            _ => {
                let register = self.pointer_register(address)?;
                Ok(Addr::Offset { base: register, imm: 0 })
            }
        }
    }

    fn base_offset(
        &mut self,
        base: Gpr,
        offset: i64,
        scale: i64,
    ) -> Selected<Addr> {
        if offset_in_access(offset, scale) {
            return Ok(Addr::Offset { base, imm: offset });
        }
        let register = self.add_constant(base, offset);
        Ok(Addr::Offset { base: register, imm: 0 })
    }

    /// The bytes an access of type `ty` moves and how it is registered.
    pub(super) fn access(
        &self,
        ty: TypeId,
    ) -> Selected<(u8, bool)> {
        Ok(match self.ty(ty) {
            Type::Int(bits) if *bits <= 8 => (1, false),
            Type::Int(bits) if *bits <= 16 => (2, false),
            Type::Int(bits) if *bits <= 32 => (4, false),
            Type::Int(bits) if *bits <= 64 => (8, false),
            Type::Pointer(_) => (8, false),
            Type::Float(llrm_mir::FloatKind::Float) => (4, true),
            Type::Float(llrm_mir::FloatKind::Double) => (8, true),
            other => return refuse(format!("a memory access of type {other:?}")),
        })
    }

    pub(super) fn pointer_operand(
        &mut self,
        operand: Operand,
    ) -> Selected<AddrExpr> {
        match self.src(operand)? {
            Src::Val(Val::Ptr(address)) => Ok(address),
            Src::Val(Val::Int(g)) => Ok(AddrExpr::of(Base::Reg(g.reg))),
            Src::Int { value, .. } => {
                let reg = self.mf.gpr();
                self.load_immediate(reg, 64, value as u64);
                Ok(AddrExpr::of(Base::Reg(reg)))
            }
            _ => refuse("a pointer operand that is not a pointer"),
        }
    }

    pub(super) fn load(
        &mut self,
        instruction: &Instruction,
    ) -> Selected<()> {
        let address = self.pointer_operand(instruction.operands[0])?;
        if let Some((cast, to)) = self.extended_by_reader(instruction) {
            let g = self.load_signed(instruction.ty, &address, to)?;
            let result = self.function.instruction(cast).result;
            self.fused.insert(cast);
            return self.define(result, Src::Val(Val::Int(g)));
        }
        let value = self.load_typed(instruction.ty, &address)?;
        self.define(instruction.result, Src::Val(value))
    }

    /// The `sext` that is the only reader of `load`, in its block, and the bits
    /// it extends to: it is part of the load (`ldrsb`, `ldrsh`, `ldrsw`),
    /// as the pin's DAGCombiner makes an extending load of it.
    fn extended_by_reader(
        &self,
        load: &Instruction,
    ) -> Option<(InstId, u32)> {
        let result = load.result?;
        if self.use_count(result) != 1 || !matches!(self.ty(load.ty), Type::Int(8 | 16 | 32)) {
            return None;
        }
        let user = self.function.users(result).into_iter().next()?.user;
        let reader = self.function.instruction(user);
        let own = super::value_def(self.function, result).and_then(|inst| self.function.parent(inst));
        let to = self.types().int_bits(reader.ty)?;
        // An extension the addressing mode takes is not part of the load.
        (reader.opcode == Opcode::Cast(CastOp::SExt)
            && matches!(to, 32 | 64)
            && own == self.function.parent(user)
            && !self.folds_into_index(reader))
        .then_some((user, to))
    }

    /// A scalar integer of type `ty` read from `address` and sign-extended to
    /// `to` bits (32 or 64).
    fn load_signed(
        &mut self,
        ty: TypeId,
        address: &AddrExpr,
        to: u32,
    ) -> Selected<G> {
        let (bytes, _) = self.access(ty)?;
        let addr = self.addressing(address, bytes)?;
        let reg = self.mf.gpr();
        let width = if to == 64 { Width::X } else { Width::W };
        self.inst(Inst::Load {
            mem: Memory::Int { width, bytes, sign: true, reg },
            addr,
            mmo: memory_operand(address),
        });
        // A 32-bit result of a `w` register leaves bits 32 and above zero.
        Ok(G { reg, bits: to, known: if to == 32 { Known::Zero } else { Known::Nothing } })
    }

    /// A scalar of type `ty` read from `address`.
    pub(crate) fn load_scalar(
        &mut self,
        ty: TypeId,
        address: &AddrExpr,
    ) -> Selected<Val> {
        let (bytes, float) = self.access(ty)?;
        let addr = self.addressing(address, bytes)?;
        Ok(if float {
            let reg = self.mf.fpr();
            self.inst(Inst::Load { mem: Memory::Fp { bytes, reg }, addr, mmo: memory_operand(address) });
            Val::Float { reg, size: if bytes == 4 { FpSize::S } else { FpSize::D } }
        } else {
            let reg = self.mf.gpr();
            let width = if bytes == 8 { Width::X } else { Width::W };
            self.inst(Inst::Load {
                mem: Memory::Int { width, bytes, sign: false, reg },
                addr,
                mmo: memory_operand(address),
            });
            let bits = match self.ty(ty) {
                Type::Int(bits) => *bits,
                _ => 64,
            };
            if matches!(self.ty(ty), Type::Pointer(_)) {
                Val::Ptr(AddrExpr::of(Base::Reg(reg)))
            } else {
                // A narrow load zero-extends to its register.
                let known = if bits as u8 == bytes * 8 && bits >= 32 {
                    if bits == 32 { Known::Zero } else { Known::Nothing }
                } else {
                    Known::Zero
                };
                Val::Int(G { reg, bits, known })
            }
        })
    }

    pub(super) fn store(
        &mut self,
        instruction: &Instruction,
    ) -> Selected<()> {
        let stored = instruction.operands[0];
        let ty = self.operand_type(stored)?;
        let value = self.src(stored)?;
        let address = self.pointer_operand(instruction.operands[1])?;
        if self.is_aggregate(ty) {
            let Src::Val(value) = value else { return refuse("a constant aggregate stored") };
            return self.store_typed(ty, &value, &address);
        }
        // A float constant `fmov` cannot make is stored as the integer of its
        // bits, from the zero register where it
        // is zero (`DAGCombiner::visitSTORE`'s `store fpconst` -> `store
        // iconst`): no trip through a float register.
        if let Src::Float { size, bits } = &value {
            let double = match size {
                FpSize::S => f64::from(f32::from_bits(*bits as u32)),
                _ => f64::from_bits(*bits),
            };
            if crate::mc::immediate::fp_immediate(double).is_none() {
                let (bytes, int_bits) = if *size == FpSize::S { (4, 32) } else { (8, 64) };
                let reg = if *bits == 0 {
                    Gpr::R31
                } else {
                    self.gpr_of(Src::Int { bits: int_bits, value: u128::from(*bits) })?.reg
                };
                let mem = Memory::Int { width: if bytes == 8 { Width::X } else { Width::W }, bytes, sign: false, reg };
                let addr = self.addressing(&address, bytes)?;
                self.inst(Inst::Store { mem, addr, mmo: memory_operand(&address) });
                return Ok(());
            }
        }
        self.store_scalar_src(ty, value, &address)
    }

    /// A scalar value, held in registers, written to `address`.
    pub(crate) fn store_scalar(
        &mut self,
        ty: TypeId,
        value: Val,
        address: &AddrExpr,
    ) -> Selected<()> {
        self.store_scalar_src(ty, Src::Val(value), address)
    }

    fn store_scalar_src(
        &mut self,
        ty: TypeId,
        value: Src,
        address: &AddrExpr,
    ) -> Selected<()> {
        let (bytes, float) = self.access(ty)?;
        let mem = if float {
            let Val::Float { reg, .. } = self.materialize(value)? else { return refuse("a float store of an integer") };
            Memory::Fp { bytes, reg }
        } else {
            // A zero constant is stored from the zero register.
            let reg = match value {
                Src::Int { value: 0, .. } => Gpr::R31,
                other => self.gpr_of(other)?.reg,
            };
            Memory::Int { width: if bytes == 8 { Width::X } else { Width::W }, bytes, sign: false, reg }
        };
        let addr = self.addressing(address, bytes)?;
        self.inst(Inst::Store { mem, addr, mmo: memory_operand(address) });
        Ok(())
    }

    pub(super) fn gep(
        &mut self,
        instruction: &Instruction,
        source: TypeId,
    ) -> Selected<()> {
        let base = self.pointer_operand(instruction.operands[0])?;
        let mut address = base;
        // The constant part of the indices, and the variable ones with their
        // element sizes.
        let mut indices = Vec::new();
        for operand in &instruction.operands[1..] {
            if self.is_folded_index(*operand) {
                indices.push(None);
                continue;
            }
            indices.push(match self.src(*operand)? {
                Src::Int { bits, value } => Some(llrm_mir::context::signed(value, bits)),
                _ => None,
            });
        }
        let (offset, variable) = self.layout.collect_offset(self.types(), source, &indices);
        address.offset += offset as i64;
        for (position, scale) in variable {
            if let Some(index) = self.folded_index(instruction.operands[1 + position], scale)? {
                if address.index.is_some() {
                    return refuse("a folded index on an address that has one");
                }
                address.index = Some(index);
                continue;
            }
            let g = self.operand_g(instruction.operands[1 + position])?;
            address = self.add_scaled_index(address, g, scale)?;
        }
        self.define(instruction.result, Src::Val(Val::Ptr(address)))
    }

    /// `address + index * scale`, as an index register where the scale is a
    /// power of two small enough for an addressing mode, else a multiply.
    fn add_scaled_index(
        &mut self,
        address: AddrExpr,
        g: G,
        scale: u64,
    ) -> Selected<AddrExpr> {
        // The index is a 64-bit signed value.
        let wide = if g.bits < 64 { self.widened(G { known: g.known, ..g }, 64, true) } else { g };
        if address.index.is_none() && scale.is_power_of_two() && scale <= 16 {
            return Ok(AddrExpr {
                index: Some(Index { reg: wide.reg, shift: scale.trailing_zeros() as u8, extend: Extend::Uxtx }),
                ..address
            });
        }
        // Otherwise the product is added into a base register.
        let product = self.mf.gpr();
        if scale.is_power_of_two() {
            self.shift_left_immediate(Width::X, product, wide.reg, scale.trailing_zeros());
        } else {
            let constant = self.mf.gpr();
            self.load_immediate(constant, 64, scale);
            self.inst(Inst::ThreeSource {
                op: Mul3::Madd,
                width: Width::X,
                rd: product,
                rn: wide.reg,
                rm: constant,
                ra: Gpr::R31,
            });
        }
        let base = self.pointer_register(&address)?;
        let sum = self.mf.gpr();
        self.inst(Inst::AddSubShifted {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: sum,
            rn: base,
            rm: product,
            shift: Shift::Lsl,
            amount: 0,
        });
        Ok(AddrExpr::of(Base::Reg(sum)))
    }
}

/// Whether an access of `scale` bytes takes `offset` in its own encoding:
/// scaled and unsigned (`ldr x, [x, #8]`) or unscaled and signed (`ldur`).
fn offset_in_access(
    offset: i64,
    scale: i64,
) -> bool {
    let scaled = offset >= 0 && offset % scale == 0 && offset / scale < 4096;
    scaled || (-256..256).contains(&offset)
}
