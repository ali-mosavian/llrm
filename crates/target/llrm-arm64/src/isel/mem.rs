//! Memory: frame objects, loads, stores, address arithmetic and globals. A
//! pointer is kept as an `AddrExpr` until something needs it in a register, so
//! a load or store takes the base, scaled index and offset into its addressing
//! mode (`AArch64DAGToDAGISel::SelectAddrModeIndexed` and its relatives).

use llrm_mir::{Instruction, Operand, Type, TypeId};

use super::val::{AddrExpr, Base, G, Index, Known, Val};
use super::{Selected, Selector, Src, refuse};
use crate::mc::inst::*;
use crate::mfunc::Op;

impl Selector<'_> {
    pub(super) fn alloca(
        &mut self,
        instruction: &Instruction,
        allocated: TypeId,
        align: Option<u64>,
    ) -> Selected<()> {
        // A static alloca: a constant count of one.
        let count = match self.src(instruction.operands[0])? {
            Src::Int { value, .. } => value as u64,
            _ => return refuse("a dynamic alloca"),
        };
        let size = self.layout.alloc_size(self.types(), allocated) * count;
        let align = align.unwrap_or(0).max(self.layout.align(self.types(), allocated)).max(1);
        let slot = self.mf.slot(size.max(1), align);
        self.define(instruction.result, Src::Val(Val::Ptr(AddrExpr::of(Base::Slot(slot)))))
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
                let page = self.mf.gpr();
                self.inst(Inst::Adr { page: true, rd: page, target: Target::Symbol { name: name.clone(), addend: 0 } });
                let dst = self.mf.gpr();
                if *got {
                    self.inst(Inst::Load {
                        mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: dst },
                        addr: Addr::PageOffset {
                            base: page,
                            target: Target::Symbol { name: name.clone(), addend: 0 },
                            got: true,
                        },
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
        match (&address.base, address.index) {
            (Base::Slot(slot), None) => Ok(Addr::Slot { slot: *slot, offset: address.offset }),
            (Base::Reg(base), None) => self.base_offset(*base, address.offset, scale),
            (Base::Reg(base), Some(index))
                if address.offset == 0 && (index.shift == 0 || i64::from(1u8 << index.shift) == scale) =>
            {
                Ok(Addr::Register { base: *base, index: index.reg, extend: index.extend, shifted: index.shift != 0 })
            }
            (Base::Global { name, got: false }, None) => {
                let page = self.mf.gpr();
                self.inst(Inst::Adr {
                    page: true,
                    rd: page,
                    target: Target::Symbol { name: name.clone(), addend: address.offset },
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
        let scaled = offset >= 0 && offset % scale == 0 && offset / scale < 4096;
        let unscaled = (-256..256).contains(&offset);
        if scaled || unscaled {
            return Ok(Addr::Offset { base, imm: offset });
        }
        let register = self.add_constant(base, offset);
        Ok(Addr::Offset { base: register, imm: 0 })
    }

    /// The bytes an access of type `ty` moves and how it is registered.
    fn access(
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

    fn pointer_operand(
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
        let ty = instruction.ty;
        let (bytes, float) = self.access(ty)?;
        let address = self.pointer_operand(instruction.operands[0])?;
        let addr = self.addressing(&address, bytes)?;
        let value = if float {
            let reg = self.mf.fpr();
            self.inst(Inst::Load { mem: Memory::Fp { bytes, reg }, addr });
            Val::Float { reg, size: if bytes == 4 { FpSize::S } else { FpSize::D } }
        } else {
            let reg = self.mf.gpr();
            let width = if bytes == 8 { Width::X } else { Width::W };
            self.inst(Inst::Load { mem: Memory::Int { width, bytes, sign: false, reg }, addr });
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
        };
        self.define(instruction.result, Src::Val(value))
    }

    pub(super) fn store(
        &mut self,
        instruction: &Instruction,
    ) -> Selected<()> {
        let stored = instruction.operands[0];
        let ty = self.operand_type(stored)?;
        let (bytes, float) = self.access(ty)?;
        let value = self.src(stored)?;
        let address = self.pointer_operand(instruction.operands[1])?;
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
        let addr = self.addressing(&address, bytes)?;
        self.inst(Inst::Store { mem, addr });
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
            indices.push(match self.src(*operand)? {
                Src::Int { bits, value } => Some(llrm_mir::context::signed(value, bits)),
                _ => None,
            });
        }
        let (offset, variable) = self.layout.collect_offset(self.types(), source, &indices);
        address.offset += offset as i64;
        for (position, scale) in variable {
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
