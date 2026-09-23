//! Port of `qbopt/model/ir.py`: `instruction_effects`, `instruction_semantics` and their helpers.
//!
//! The module docstring and every table's rationale are Python's.

use std::collections::BTreeSet;

use iced_x86::{Code, Mnemonic, OpKind, Register};

use super::{
    ANY_MEMORY, Address, Effects, Flag, Imm, Loc, Mem, Operation, Reg, Semantics, St, UNMODELLED, barrier, root,
};
use crate::analysis::flags::{ALL, CLOBBERS, written_by};
use crate::frontends::bc::declen::{Insn, READS, WRITES, instruction_info_factory, to_signed};
use crate::legacy::lift::{Resolver, operand as long_operand};
use crate::objectfile::module::Space;

/// Which roots this instruction may write, and which it may read.
pub fn _register_effects(insn: &Insn) -> (BTreeSet<Register>, BTreeSet<Register>) {
    let mut factory = instruction_info_factory();
    let used = factory.info(&insn.insn).used_registers().to_vec();
    let mut defs = BTreeSet::new();
    let mut uses = BTreeSet::new();
    for one in used {
        let target = root(one.register());
        if WRITES.contains(&one.access()) {
            defs.insert(target);
            if target != one.register() {
                uses.insert(target);
            }
        }
        if READS.contains(&one.access()) {
            uses.insert(target);
        }
    }
    (defs, uses)
}

/// A memory access through sp is a stack cell, whose address this layer does
/// not name.
pub const STACK_BASES: [Register; 2] = [Register::SP, Register::ESP];

/// Every cell this instruction reads, and every one it writes.
pub fn _memory_effects(insn: &Insn, resolve: &Resolver) -> (Vec<Mem>, Vec<Mem>) {
    let mut factory = instruction_info_factory();
    let used = factory.info(&insn.insn).used_memory().to_vec();
    let named = used.iter().filter(|one| !STACK_BASES.contains(&one.base())).count();
    let where_ = if named == 1 { long_operand(insn, resolve) } else { None };

    let mut loads = Vec::new();
    let mut stores = Vec::new();
    for one in &used {
        let cell = Mem::new(
            if STACK_BASES.contains(&one.base()) { None } else { where_ },
            one.memory_size().size() as u32,
        );
        if READS.contains(&one.access()) {
            loads.push(cell.clone());
        }
        if WRITES.contains(&one.access()) {
            stores.push(cell);
        }
    }
    (loads, stores)
}

/// Whether this instruction pushes, pops, or reads/writes any x87 stack
/// register -- the fact `Effects.fp_stack` carries.
pub fn _touches_fp_stack(insn: &Insn) -> bool {
    if insn.insn.fpu_stack_increment_info().increment() != 0 {
        return true;
    }
    let mut factory = instruction_info_factory();
    factory.info(&insn.insn).used_registers().iter().any(|one| one.register().is_st())
}

pub const OPAQUE_MEMORY_COMPLETE: [Code; 8] = [
    Code::Fcompp,
    Code::Fnstsw_m2byte,
    Code::Fnstsw_AX,
    Code::Sahf,
    Code::Fstp_sti,
    Code::Les_r16_m1616,
    Code::Cld,
    Code::Std,
];

/// The conservative effect of one real instruction.
pub fn instruction_effects(insn: &Insn, resolve: &Resolver) -> Effects {
    if CLOBBERS.contains(&insn.flow()) {
        return Effects {
            defs: None,
            uses: None,
            flags_written: written_by(insn),
            flags_read: ALL,
            loads: ANY_MEMORY.clone(),
            stores: ANY_MEMORY.clone(),
            fp_stack: true,
            memory_complete: false,
        };
    }
    let (defs, uses) = _register_effects(insn);
    let read = Flag(insn.reads() & ALL.0);
    let fp_stack = _touches_fp_stack(insn);
    let complete = OPAQUE_MEMORY_COMPLETE.contains(&insn.insn.code());
    if barrier(&instruction_semantics(insn, resolve)) && !complete {
        return Effects {
            defs: Some(defs),
            uses: Some(uses),
            flags_written: written_by(insn),
            flags_read: read,
            loads: ANY_MEMORY.clone(),
            stores: ANY_MEMORY.clone(),
            fp_stack,
            memory_complete: false,
        };
    }
    let (loads, stores) = _memory_effects(insn, resolve);
    Effects {
        defs: Some(defs),
        uses: Some(uses),
        flags_written: written_by(insn),
        flags_read: read,
        loads,
        stores,
        fp_stack,
        memory_complete: complete,
    }
}

/// What each immediate encoding means once sign extension has been applied.
pub const IMMEDIATE_WIDTH: [(OpKind, u32); 5] = [
    (OpKind::Immediate8, 1),
    (OpKind::Immediate8to16, 2),
    (OpKind::Immediate8to32, 4),
    (OpKind::Immediate16, 2),
    (OpKind::Immediate32, 4),
];

/// One operand as a typed location, or `None` if this layer cannot say.
pub fn _location(insn: &Insn, index: u32, resolve: &Resolver) -> Option<Loc> {
    let kind = insn.insn.op_kind(index);
    match kind {
        OpKind::Register => {
            let register = insn.insn.op_register(index);
            if register.is_gpr() || register.is_segment_register() {
                return Some(Loc::Reg(Reg { register, width: register.size() as u32 }));
            }
            None
        }
        OpKind::Memory => Some(Loc::Mem(Mem {
            through: insn.insn.memory_base(),
            offset: insn.displacement(),
            disp_width: insn.disp_len as u32,
            ..Mem::new(long_operand(insn, resolve), insn.insn.memory_size().size() as u32)
        })),
        _ => {
            let &(_, width) = IMMEDIATE_WIDTH.iter().find(|(one, _)| *one == kind)?;
            let value = to_signed(insn.insn.immediate(index) & ((1_u64 << (width * 8)) - 1), width as usize);
            let mut address = insn.imm_at.map(|imm_at| resolve(imm_at as i64, value));
            if address.is_some_and(|address| address.space == Space::Literal) {
                address = None;
            }
            Some(Loc::Imm(Imm { value, width, address }))
        }
    }
}

pub fn _destination(insn: &Insn, index: u32, resolve: &Resolver) -> Option<Loc> {
    let found = _location(insn, index, resolve);
    match found {
        Some(Loc::Reg(_) | Loc::Mem(_)) => found,
        _ => None,
    }
}

/// One x87 register operand, `st(i)`.
pub fn _stack_register(insn: &Insn, index: u32) -> Option<St> {
    if insn.insn.op_kind(index) != OpKind::Register {
        return None;
    }
    let register = insn.insn.op_register(index);
    register.is_st().then(|| St { index: register as u32 - Register::ST0 as u32 })
}

pub type Builder = fn(&Insn, &Resolver, Operation, &str) -> Option<Semantics>;

fn shaped(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

pub fn _move(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 2 {
        return None;
    }
    let dest = _destination(insn, 0, resolve)?;
    let source = _location(insn, 1, resolve)?;
    Some(shaped(op, name, vec![dest], vec![source]))
}

/// `xchg`: both operands read AND written, each receiving the other's old value.
pub fn _exchange(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 2 {
        return None;
    }
    let (first, second) = (_destination(insn, 0, resolve), _destination(insn, 1, resolve));
    let (first, second) = (first?, second?);
    Some(shaped(op, name, vec![first.clone(), second.clone()], vec![second, first]))
}

pub fn _address(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 2 || insn.insn.op_kind(1) != OpKind::Memory {
        return None;
    }
    let dest = _destination(insn, 0, resolve)?;
    let where_ = Address {
        addr: long_operand(insn, resolve),
        through: insn.insn.memory_base(),
        index: insn.insn.memory_index(),
        scale: i64::from(insn.insn.memory_index_scale()),
        offset: insn.displacement(),
        disp_width: insn.disp_len as u32,
    };
    Some(shaped(op, name, vec![dest], vec![Loc::Address(where_)]))
}

pub fn _binary(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 2 {
        return None;
    }
    let dest = _destination(insn, 0, resolve);
    let source = _location(insn, 1, resolve);
    let (dest, source) = (dest?, source?);
    Some(shaped(op, name, vec![dest.clone()], vec![dest, source]))
}

const fn reg(register: Register, width: u32) -> Loc {
    Loc::Reg(Reg { register, width })
}

/// imul's own implicit pair in its one-operand, widening form: the (low, high)
/// destinations and the accumulator half it reads, by the width of its one
/// explicit operand. The 8-bit form is a different shape and left out.
pub const WIDE_MULTIPLY: [(u32, ([Loc; 2], Loc)); 2] = [
    (4, ([reg(Register::EAX, 4), reg(Register::EDX, 4)], reg(Register::EAX, 4))),
    (2, ([reg(Register::AX, 2), reg(Register::DX, 2)], reg(Register::AX, 2))),
];

/// `imul` in all three of its forms.
pub fn _multiply(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    match insn.insn.op_count() {
        1 => {
            let factor = _location(insn, 0, resolve)?;
            let width = match &factor {
                Loc::Reg(one) => one.width,
                Loc::Mem(one) => one.width,
                _ => return None,
            };
            let (_, (dests, low)) = WIDE_MULTIPLY.iter().find(|(one, _)| *one == width)?.clone();
            Some(shaped(op, name, dests.to_vec(), vec![low, factor]))
        }
        2 => {
            let (dest, source) = (_destination(insn, 0, resolve), _location(insn, 1, resolve));
            let (dest, source) = (dest?, source?);
            Some(shaped(op, name, vec![dest.clone()], vec![dest, source]))
        }
        3 => {
            let dest = _destination(insn, 0, resolve);
            let (left, right) = (_location(insn, 1, resolve), _location(insn, 2, resolve));
            let (dest, left, right) = (dest?, left?, right?);
            Some(shaped(op, name, vec![dest], vec![left, right]))
        }
        _ => None,
    }
}

/// idiv's own implicit pair, by the width of its one explicit operand:
/// (quotient, remainder), and the dividend halves it reads. The 8-bit form is
/// a different shape and left out.
pub const DIVIDE_PAIR: [(u32, ([Loc; 2], [Loc; 2])); 2] = [
    (4, ([reg(Register::EAX, 4), reg(Register::EDX, 4)], [reg(Register::EDX, 4), reg(Register::EAX, 4)])),
    (2, ([reg(Register::AX, 2), reg(Register::DX, 2)], [reg(Register::DX, 2), reg(Register::AX, 2)])),
];

/// `idiv rm`: quotient in the accumulator, remainder in its partner.
pub fn _divide(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 1 {
        return None;
    }
    let divisor = _location(insn, 0, resolve)?;
    let width = match &divisor {
        Loc::Reg(one) => one.width,
        Loc::Mem(one) => one.width,
        _ => return None,
    };
    let (_, (dests, [high, low])) = DIVIDE_PAIR.iter().find(|(one, _)| *one == width)?.clone();
    Some(shaped(op, name, dests.to_vec(), vec![high, low, divisor]))
}

pub fn _compare(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 2 {
        return None;
    }
    let (left, right) = (_location(insn, 0, resolve), _location(insn, 1, resolve));
    let (left, right) = (left?, right?);
    Some(shaped(op, name, Vec::new(), vec![left, right]))
}

pub fn _unary(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 1 {
        return None;
    }
    let dest = _destination(insn, 0, resolve)?;
    Some(shaped(op, name, vec![dest.clone()], vec![dest]))
}

/// cwd/cdq: which register receives the sign of which, at what width.
pub const EXTEND_PAIR: [(Mnemonic, (Loc, Loc)); 2] = [
    (Mnemonic::Cwd, (reg(Register::DX, 2), reg(Register::AX, 2))),
    (Mnemonic::Cdq, (reg(Register::EDX, 4), reg(Register::EAX, 4))),
];

pub fn _extend(insn: &Insn, _resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    let mnemonic = insn.insn.mnemonic();
    let (_, (dest, source)) = EXTEND_PAIR.iter().find(|(one, _)| *one == mnemonic)?.clone();
    Some(shaped(op, name, vec![dest], vec![source]))
}

pub fn _push(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 1 {
        return None;
    }
    let source = _location(insn, 0, resolve)?;
    Some(shaped(op, name, Vec::new(), vec![source]))
}

pub fn _pop(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 1 {
        return None;
    }
    let dest = _destination(insn, 0, resolve)?;
    Some(shaped(op, name, vec![dest], Vec::new()))
}

/// `leave` is exactly `mov sp,bp` then `pop bp`.
pub fn _leave(insn: &Insn, _resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.code() != Code::Leavew {
        return None;
    }
    let (stack, frame) = (reg(Register::SP, 2), reg(Register::BP, 2));
    Some(shaped(op, name, vec![stack, frame.clone()], vec![frame]))
}

/// `rep stosw`: cx words of ax written through es:di, di stepping by DF.
pub fn _fill(insn: &Insn, _resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.code() != Code::Stosw_m16_AX || !insn.insn.has_rep_prefix() {
        return None;
    }
    let (value, count) = (reg(Register::AX, 2), reg(Register::CX, 2));
    let (through, segment) = (reg(Register::DI, 2), reg(Register::ES, 2));
    Some(shaped(op, name, vec![Loc::Mem(Mem::new(None, 0))], vec![value, count, through, segment]))
}

/// A jump or a conditional branch whose target is a computable offset.
pub fn _transfer(insn: &Insn, _resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    let target = insn.target()?;
    Some(Semantics { name: Some(name.to_owned()), target: Some(target as i64), ..Semantics::new(op) })
}

/// A direct far branch's target: a segment:offset immediate a fixup writes.
pub const FAR_BRANCH: [OpKind; 2] = [OpKind::FarBranch16, OpKind::FarBranch32];

/// A near `jmp` goes where the instruction says. A direct far `jmp` does not.
pub fn _jump(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    let near = _transfer(insn, resolve, op, name);
    if near.is_some() {
        return near;
    }
    FAR_BRANCH
        .contains(&insn.insn.op0_kind())
        .then(|| Semantics { name: Some(name.to_owned()), ..Semantics::new(Operation::Escape) })
}

/// A call site's own shape; its effect stays conservative.
pub fn _call(insn: &Insn, _resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    Some(Semantics {
        name: Some(name.to_owned()),
        target: insn.target().map(|target| target as i64),
        ..Semantics::new(op)
    })
}

/// An instruction that does nothing at all -- padding between bodies.
pub fn _nothing(insn: &Insn, _resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    (insn.insn.op_count() == 0).then(|| shaped(op, name, Vec::new(), Vec::new()))
}

pub fn _return(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    match insn.insn.op_count() {
        0 => Some(shaped(op, name, Vec::new(), Vec::new())),
        1 => match _location(insn, 0, resolve) {
            Some(popped @ Loc::Imm(_)) => Some(shaped(op, name, Vec::new(), vec![popped])),
            _ => None,
        },
        _ => None,
    }
}

/// `fld`/`fild`: pushes the one real memory operand onto the stack.
pub fn _float_load(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    let mnemonic = insn.insn.mnemonic();
    if insn.insn.op_count() == 0 && matches!(mnemonic, Mnemonic::Fldz | Mnemonic::Fld1) {
        let value = Imm { value: i64::from(mnemonic == Mnemonic::Fld1), width: 2, address: None };
        return Some(shaped(op, name, vec![Loc::St(St { index: 0 })], vec![Loc::Imm(value)]));
    }
    if insn.insn.op_count() != 1 || insn.insn.op_kind(0) != OpKind::Memory {
        return None;
    }
    let source = _location(insn, 0, resolve)?;
    Some(shaped(op, name, vec![Loc::St(St { index: 0 })], vec![source]))
}

/// `fstp`/`fistp`: the current top, written to the one real memory operand,
/// then popped.
pub fn _float_store(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 1 || insn.insn.op_kind(0) != OpKind::Memory {
        return None;
    }
    let dest = _location(insn, 0, resolve)?;
    Some(shaped(op, name, vec![dest], vec![Loc::St(St { index: 0 })]))
}

pub fn _float_arith(insn: &Insn, resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() == 2 && ["fadd", "fsub", "fmul", "fdiv"].contains(&name) {
        let (dest, source) = (_stack_register(insn, 0), _stack_register(insn, 1));
        let (Some(dest), Some(source)) = (dest, source) else {
            return None;
        };
        if dest.index != 0 && source.index != 0 {
            return None;
        }
        return Some(shaped(op, name, vec![Loc::St(dest)], vec![Loc::St(dest), Loc::St(source)]));
    }
    if insn.insn.op_count() != 1 || insn.insn.op_kind(0) != OpKind::Memory {
        return None;
    }
    let source = _location(insn, 0, resolve)?;
    Some(shaped(op, name, vec![Loc::St(St { index: 0 })], vec![Loc::St(St { index: 0 }), source]))
}

/// `faddp`/`fsubp`/`fmulp`/`fdivp st(i),st(0)`: combines the two, writes the
/// result to st(i), then pops.
pub fn _float_arith_pop(insn: &Insn, _resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    if insn.insn.op_count() != 2 {
        return None;
    }
    let (dest, second) = (_stack_register(insn, 0), _stack_register(insn, 1));
    let (Some(dest), Some(second)) = (dest, second) else {
        return None;
    };
    if second.index != 0 {
        return None;
    }
    Some(shaped(op, name, vec![Loc::St(dest)], vec![Loc::St(dest), Loc::St(second)]))
}

/// `fchs`/`fabs`/`fsqrt`: the top, transformed in place.
pub fn _float_unary(insn: &Insn, _resolve: &Resolver, op: Operation, name: &str) -> Option<Semantics> {
    (insn.insn.op_count() == 0)
        .then(|| shaped(op, name, vec![Loc::St(St { index: 0 })], vec![Loc::St(St { index: 0 })]))
}

pub const BUILD: [(Operation, Builder); 23] = [
    (Operation::Move, _move),
    (Operation::Exchange, _exchange),
    (Operation::Address, _address),
    (Operation::Binary, _binary),
    (Operation::Multiply, _multiply),
    (Operation::Divide, _divide),
    (Operation::Compare, _compare),
    (Operation::Unary, _unary),
    (Operation::Extend, _extend),
    (Operation::Push, _push),
    (Operation::Pop, _pop),
    (Operation::Leave, _leave),
    (Operation::Fill, _fill),
    (Operation::Jump, _jump),
    (Operation::Branch, _transfer),
    (Operation::Call, _call),
    (Operation::Return, _return),
    (Operation::Nothing, _nothing),
    (Operation::FloatLoad, _float_load),
    (Operation::FloatStore, _float_store),
    (Operation::FloatArith, _float_arith),
    (Operation::FloatArithPop, _float_arith_pop),
    (Operation::FloatUnary, _float_unary),
];

/// The vocabulary, keyed on the mnemonic. Python's `SHAPE` comment says what
/// is deliberately absent and why.
pub const SHAPE: [(Mnemonic, Operation, &str); 68] = [
    (Mnemonic::Mov, Operation::Move, "mov"),
    (Mnemonic::Xchg, Operation::Exchange, "xchg"),
    (Mnemonic::Lea, Operation::Address, "lea"),
    (Mnemonic::Add, Operation::Binary, "add"),
    (Mnemonic::Adc, Operation::Binary, "adc"),
    (Mnemonic::Sub, Operation::Binary, "sub"),
    (Mnemonic::Sbb, Operation::Binary, "sbb"),
    (Mnemonic::And, Operation::Binary, "and"),
    (Mnemonic::Or, Operation::Binary, "or"),
    (Mnemonic::Xor, Operation::Binary, "xor"),
    (Mnemonic::Shl, Operation::Binary, "shl"),
    (Mnemonic::Shr, Operation::Binary, "shr"),
    (Mnemonic::Sar, Operation::Binary, "sar"),
    (Mnemonic::Imul, Operation::Multiply, "imul"),
    (Mnemonic::Idiv, Operation::Divide, "idiv"),
    (Mnemonic::Cmp, Operation::Compare, "cmp"),
    (Mnemonic::Test, Operation::Compare, "test"),
    (Mnemonic::Neg, Operation::Unary, "neg"),
    (Mnemonic::Not, Operation::Unary, "not"),
    (Mnemonic::Inc, Operation::Unary, "inc"),
    (Mnemonic::Dec, Operation::Unary, "dec"),
    (Mnemonic::Cwd, Operation::Extend, "cwd"),
    (Mnemonic::Cdq, Operation::Extend, "cdq"),
    (Mnemonic::Nop, Operation::Nothing, "nop"),
    (Mnemonic::Push, Operation::Push, "push"),
    (Mnemonic::Pop, Operation::Pop, "pop"),
    (Mnemonic::Leave, Operation::Leave, "leave"),
    (Mnemonic::Stosw, Operation::Fill, "stosw"),
    (Mnemonic::Jmp, Operation::Jump, "jmp"),
    (Mnemonic::Call, Operation::Call, "call"),
    (Mnemonic::Ret, Operation::Return, "ret"),
    (Mnemonic::Retf, Operation::Return, "retf"),
    (Mnemonic::Ja, Operation::Branch, "ja"),
    (Mnemonic::Jae, Operation::Branch, "jae"),
    (Mnemonic::Jb, Operation::Branch, "jb"),
    (Mnemonic::Jbe, Operation::Branch, "jbe"),
    (Mnemonic::Je, Operation::Branch, "je"),
    (Mnemonic::Jg, Operation::Branch, "jg"),
    (Mnemonic::Jge, Operation::Branch, "jge"),
    (Mnemonic::Jl, Operation::Branch, "jl"),
    (Mnemonic::Jle, Operation::Branch, "jle"),
    (Mnemonic::Jne, Operation::Branch, "jne"),
    (Mnemonic::Jno, Operation::Branch, "jno"),
    (Mnemonic::Jnp, Operation::Branch, "jnp"),
    (Mnemonic::Jns, Operation::Branch, "jns"),
    (Mnemonic::Jo, Operation::Branch, "jo"),
    (Mnemonic::Jp, Operation::Branch, "jp"),
    (Mnemonic::Js, Operation::Branch, "js"),
    (Mnemonic::Fld, Operation::FloatLoad, "fld"),
    // These exact constants have integer-to-real semantics, independent of RC.
    (Mnemonic::Fldz, Operation::FloatLoad, "fild"),
    (Mnemonic::Fld1, Operation::FloatLoad, "fild"),
    (Mnemonic::Fild, Operation::FloatLoad, "fild"),
    (Mnemonic::Fstp, Operation::FloatStore, "fstp"),
    (Mnemonic::Fistp, Operation::FloatStore, "fistp"),
    (Mnemonic::Fadd, Operation::FloatArith, "fadd"),
    (Mnemonic::Fsub, Operation::FloatArith, "fsub"),
    (Mnemonic::Fmul, Operation::FloatArith, "fmul"),
    (Mnemonic::Fdiv, Operation::FloatArith, "fdiv"),
    (Mnemonic::Fidiv, Operation::FloatArith, "fidiv"),
    (Mnemonic::Fisub, Operation::FloatArith, "fisub"),
    (Mnemonic::Faddp, Operation::FloatArithPop, "faddp"),
    (Mnemonic::Fsubp, Operation::FloatArithPop, "fsubp"),
    (Mnemonic::Fmulp, Operation::FloatArithPop, "fmulp"),
    (Mnemonic::Fdivp, Operation::FloatArithPop, "fdivp"),
    (Mnemonic::Fchs, Operation::FloatUnary, "fchs"),
    (Mnemonic::Fabs, Operation::FloatUnary, "fabs"),
    (Mnemonic::Fsqrt, Operation::FloatUnary, "fsqrt"),
    // A synchronisation point, not arithmetic: Operation.NOTHING.
    (Mnemonic::Wait, Operation::Nothing, "wait"),
];

/// What one real instruction computes, or `Operation::Barrier`.
pub fn instruction_semantics(insn: &Insn, resolve: &Resolver) -> Semantics {
    let mnemonic = insn.insn.mnemonic();
    let Some(&(_, op, name)) = SHAPE.iter().find(|(one, _, _)| *one == mnemonic) else {
        return UNMODELLED.clone();
    };
    let build = BUILD.iter().find(|(one, _)| *one == op).expect("SHAPE names only built operations").1;
    build(insn, resolve, op, name).unwrap_or_else(|| UNMODELLED.clone())
}

#[cfg(test)]
#[path = "semantics_tests.rs"]
mod tests;
