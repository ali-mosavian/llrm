//! The x87 stack as SSA float values, and the runtime's float helpers as
//! what they compute. Adapted from llrm-core's `raising_floats` (formats),
//! `fpstack` and `raising_float_values` (the stack as values),
//! `raising_float_calls` (`B$FILD`, `B$FIL2`, `B$FCMP`) and
//! `raising_float_results` (`B$FIST`, `B$FIS2`).
//!
//! A slot is `Var::St`, counted from the bottom, so a value keeps its
//! variable while pushes renumber st(i); the depth is part of a block's
//! entry state, as the 8086 stack's is. MIR has no 80-bit type: a value is
//! a `double`, what the x87 holds under precision control 53, and one it
//! could hold exactly and a double cannot refuses. Rounding to an integer is
//! `llvm.lrint`, the x87's default round-to-nearest.

use std::collections::BTreeSet;

use iced_x86::Register;
use llrm_mir::{BinaryOp, CastOp, Constant, ConstantKind, FloatKind, FloatPredicate, Module, Operand, Type, TypeId};
use llrm_qbruntime::{self as runtime, Control, Memory};
use llrm_x86_bcmachine::frontends::bc::declen::Insn;
use llrm_x86_bcmachine::model::ir::nodes::{Call, Node};
use llrm_x86_bcmachine::model::ir::{Loc, Operation, Semantics};
use llrm_x86_bcmachine::objectfile::module;

use crate::emit::{Bit, Emit, Emitter, Var};
use crate::machine::Facts;
use crate::sites::Recognizer;

/// The x87's eight registers.
const DEPTH: u8 = 8;

/// What a runtime float helper computes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Helper {
    /// Pushes the LONG in DX:AX.
    Fild,
    /// Pushes the INTEGER in AX; its `cwd` leaves AX's sign in DX.
    Fil2,
    /// Pops st0 into DX:AX, rounded.
    Fist,
    /// Pops st0 into AX, rounded.
    Fis2,
    /// Compares st0 with st1 and pops both: `fcompp`, `fnstsw`, `sahf`.
    Fcmp,
}

fn helper(name: &str) -> Option<Helper> {
    Some(match name {
        "B$FILD" => Helper::Fild,
        "B$FIL2" => Helper::Fil2,
        "B$FIST" => Helper::Fist,
        "B$FIS2" => Helper::Fis2,
        "B$FCMP" => Helper::Fcmp,
        _ => return None,
    })
}

/// Whether the raise computes `name` instead of calling it.
pub fn absorbed(name: &str) -> bool {
    helper(name).is_some()
}

fn x87(op: Operation) -> bool {
    matches!(
        op,
        Operation::FloatLoad
            | Operation::FloatStore
            | Operation::FloatArith
            | Operation::FloatArithPop
            | Operation::FloatUnary
    )
}

pub struct Floats;

impl Recognizer for Floats {
    fn node(
        &self,
        emitter: &mut Emitter,
        node: &Node,
    ) -> Option<Emit<()>> {
        match node {
            Node::Opaque(one) if x87(one.semantics.op) => Some(instruction(emitter, &one.insn, &one.semantics)),
            Node::Call(call) => match helper(&call.name) {
                Some(helper) => Some(helped(emitter, helper, call)),
                None => (emitter.floats > 0).then(|| Err(format!("calls {} with values on the x87 stack", call.name))),
            },
            _ => None,
        }
    }
}

fn double(emitter: &mut Emitter) -> TypeId {
    emitter.b.context.types.intern(Type::Float(FloatKind::Double))
}

/// The value in st(`index`).
fn st(
    emitter: &mut Emitter,
    index: u32,
) -> Emit<Operand> {
    let depth = u32::from(emitter.floats);
    if index >= depth {
        return Err(format!("reads st({index}) of an x87 stack {depth} deep"));
    }
    Ok(emitter.get(Var::St((depth - 1 - index) as u8)))
}

fn set_st(
    emitter: &mut Emitter,
    index: u32,
    value: Operand,
) -> Emit<()> {
    let depth = u32::from(emitter.floats);
    if index >= depth {
        return Err(format!("writes st({index}) of an x87 stack {depth} deep"));
    }
    emitter.set(Var::St((depth - 1 - index) as u8), value);
    Ok(())
}

fn push(
    emitter: &mut Emitter,
    value: Operand,
) -> Emit<()> {
    if emitter.floats >= DEPTH {
        return Err("a ninth value on the x87 stack".to_owned());
    }
    emitter.set(Var::St(emitter.floats), value);
    emitter.floats += 1;
    Ok(())
}

fn pop(emitter: &mut Emitter) -> Emit<Operand> {
    let top = st(emitter, 0)?;
    emitter.floats -= 1;
    Ok(top)
}

fn slot(loc: &Loc) -> Option<u32> {
    match loc {
        Loc::St(one) => Some(one.index),
        _ => None,
    }
}

/// A memory operand as the x87 reads it: a real of its width, or an
/// integer for `fi*`.
fn load(
    emitter: &mut Emitter,
    insn: &Insn,
    width: u32,
    integer: bool,
) -> Emit<Operand> {
    let pointer = emitter.pointer(insn)?;
    let to = double(emitter);
    if integer {
        if width == 8 {
            return Err("fild of a 64-bit integer, which a double cannot hold exactly".to_owned());
        }
        if width != 2 && width != 4 {
            return Err(format!("an x87 integer of {width} bytes"));
        }
        let ty = emitter.b.context.types.int(width * 8);
        let value = emitter.b.load(ty, pointer, false, "");
        return Ok(emitter.cast(CastOp::SIToFP, value, to));
    }
    let kind = match width {
        4 => FloatKind::Float,
        8 => FloatKind::Double,
        _ => return Err(format!("an x87 real of {width} bytes")),
    };
    let ty = emitter.b.context.types.intern(Type::Float(kind));
    let value = emitter.b.load(ty, pointer, false, "");
    Ok(emitter.cast(CastOp::FPExt, value, to))
}

fn float_constant(
    emitter: &mut Emitter,
    value: f64,
) -> Operand {
    let ty = double(emitter);
    Operand::Constant(emitter.b.context.constant(Constant { ty, kind: ConstantKind::Float(value.to_bits()) }))
}

/// `@name`, declared by `declare`, called on `arguments`.
fn intrinsic(
    emitter: &mut Emitter,
    name: &str,
    arguments: &[Operand],
) -> Emit<Operand> {
    let &(callee, ty) = emitter.unit.intrinsics.get(name).ok_or_else(|| format!("@{name} undeclared"))?;
    let made = emitter.b.call(ty, Operand::Constant(callee), arguments, "").ok_or("an intrinsic answering nothing")?;
    emitter.note_pure();
    Ok(made)
}

/// `value` rounded to a `bits`-wide integer.
fn rounded(
    emitter: &mut Emitter,
    value: Operand,
    bits: u32,
) -> Emit<Operand> {
    intrinsic(emitter, &format!("llvm.lrint.i{bits}.f64"), &[value])
}

fn arithmetic(name: &str) -> Option<BinaryOp> {
    Some(match name.trim_start_matches("fi").trim_start_matches('f').trim_end_matches('p') {
        "add" => BinaryOp::FAdd,
        "sub" => BinaryOp::FSub,
        "mul" => BinaryOp::FMul,
        "div" => BinaryOp::FDiv,
        _ => return None,
    })
}

fn instruction(
    emitter: &mut Emitter,
    insn: &Insn,
    what: &Semantics,
) -> Emit<()> {
    let name = what.name.as_deref().unwrap_or("");
    match what.op {
        Operation::FloatLoad => {
            let value = match &what.sources[..] {
                [Loc::Imm(imm)] => float_constant(emitter, imm.value as f64),
                [Loc::Mem(mem)] => load(emitter, insn, mem.width, name == "fild")?,
                _ => return Err(format!("x87 {name} of that operand")),
            };
            push(emitter, value)
        }
        Operation::FloatStore => {
            let [Loc::Mem(mem)] = &what.dests[..] else { return Err(format!("x87 {name} to that operand")) };
            let value = pop(emitter)?;
            let stored = match (name, mem.width) {
                ("fstp", 4) => {
                    let ty = emitter.b.context.types.intern(Type::Float(FloatKind::Float));
                    emitter.cast(CastOp::FPTrunc, value, ty)
                }
                ("fstp", 8) => value,
                ("fistp", 2 | 4 | 8) => rounded(emitter, value, mem.width * 8)?,
                _ => return Err(format!("x87 {name} of {} bytes", mem.width)),
            };
            let pointer = emitter.pointer(insn)?;
            emitter.b.store(stored, pointer, false);
            Ok(())
        }
        Operation::FloatArith | Operation::FloatArithPop => {
            let op = arithmetic(name).ok_or_else(|| format!("x87 {name}"))?;
            let dest = what.dests.first().and_then(slot).ok_or_else(|| format!("x87 {name} into memory"))?;
            let a = st(emitter, what.sources.first().and_then(slot).ok_or_else(|| format!("x87 {name}"))?)?;
            let b = match what.sources.get(1) {
                Some(Loc::St(one)) => st(emitter, one.index)?,
                Some(Loc::Mem(mem)) => load(emitter, insn, mem.width, name.starts_with("fi"))?,
                _ => return Err(format!("x87 {name} of that operand")),
            };
            let result = emitter.binary(op, a, b);
            set_st(emitter, dest, result)?;
            if what.op == Operation::FloatArithPop {
                pop(emitter)?;
            }
            Ok(())
        }
        Operation::FloatUnary => {
            let value = st(emitter, 0)?;
            let result = match name {
                "fchs" => {
                    let made = emitter.b.fneg(value, "");
                    emitter.note_pure();
                    made
                }
                "fabs" => intrinsic(emitter, "llvm.fabs.f64", &[value])?,
                "fsqrt" => intrinsic(emitter, "llvm.sqrt.f64", &[value])?,
                other => return Err(format!("x87 {other}")),
            };
            set_st(emitter, 0, result)
        }
        _ => unreachable!("an x87 operation"),
    }
}

/// Whether the call at `at` is the emulator's `name`, whose contract is the
/// one its meaning is: no memory, no error, returns, and the table's own
/// inputs and clobbers.
fn trusted(
    emitter: &Emitter,
    name: &str,
    at: usize,
) -> Emit<()> {
    let found = emitter.unit.facts.found;
    if !module::emulated(&found.records) {
        return Err(format!("{name} outside the FP emulator's protocol"));
    }
    if module::defines(&found.records, found.seg).contains(name) {
        return Err(format!("{name} is this module's own"));
    }
    let contract = emitter.unit.facts.contract(at).ok_or_else(|| format!("{name} has no contract"))?;
    let expected = runtime::contract(Some(name));
    let kept = contract.established
        && contract.writes == Memory::None
        && contract.reads == Memory::None
        && contract.cleanup == Some(0)
        && contract.control == Control::Returns
        && !(contract.enters_user_code || contract.raises_error || contract.error_handling)
        && contract.inputs == expected.inputs
        && contract.clobbers == expected.clobbers;
    if kept { Ok(()) } else { Err(format!("{name}'s contract is not its meaning's")) }
}

fn helped(
    emitter: &mut Emitter,
    helper: Helper,
    call: &Call,
) -> Emit<()> {
    trusted(emitter, &call.name, call.insn.at)?;
    let why = format!("{} clobbers it", call.name);
    let to = double(emitter);
    let word = emitter.b.context.types.int(16);
    match helper {
        Helper::Fild => {
            let (low, high) = (emitter.register(Register::AX)?, emitter.register(Register::DX)?);
            let long = emitter.join(low, high);
            emitter.clobber(&[], &why);
            let value = emitter.cast(CastOp::SIToFP, long, to);
            push(emitter, value)
        }
        Helper::Fil2 => {
            let integer = emitter.register(Register::AX)?;
            emitter.clobber(&[], &why);
            let fifteen = emitter.b.int(16, 15);
            let sign = emitter.binary(BinaryOp::AShr, integer, fifteen);
            emitter.set_register(Register::DX, sign)?;
            let value = emitter.cast(CastOp::SIToFP, integer, to);
            push(emitter, value)
        }
        Helper::Fist => {
            let value = pop(emitter)?;
            let long = rounded(emitter, value, 32)?;
            emitter.clobber(&[Register::EAX, Register::EDX], &why);
            let low = emitter.cast(CastOp::Trunc, long, word);
            let sixteen = emitter.b.int(32, 16);
            let high = emitter.binary(BinaryOp::LShr, long, sixteen);
            let high = emitter.cast(CastOp::Trunc, high, word);
            emitter.set_register(Register::AX, low)?;
            emitter.set_register(Register::DX, high)
        }
        Helper::Fis2 => {
            let value = pop(emitter)?;
            let integer = rounded(emitter, value, 16)?;
            emitter.clobber(&[Register::EAX], &why);
            emitter.set_register(Register::AX, integer)
        }
        Helper::Fcmp => {
            let (first, second) = (st(emitter, 0)?, st(emitter, 1)?);
            pop(emitter)?;
            pop(emitter)?;
            emitter.clobber(&[Register::EAX], &why);
            // sahf: CF is C0, ZF is C3; each is set when unordered too. SF
            // is the busy bit, and OF is not written.
            let below = emitter.b.fcmp(FloatPredicate::Ult, first, second, "");
            emitter.note_pure();
            let equal = emitter.b.fcmp(FloatPredicate::Ueq, first, second, "");
            emitter.note_pure();
            emitter.set_bit(Bit::C, below);
            emitter.set_bit(Bit::Z, equal);
            Ok(())
        }
    }
}

/// The intrinsics the module's x87 code calls: each `@llvm.*` a body's
/// nodes need, declared once.
pub fn declare(
    facts: &Facts,
    module: &mut Module,
    into: &mut std::collections::BTreeMap<String, (llrm_mir::ConstantId, TypeId)>,
) -> Result<(), String> {
    let mut needed: BTreeSet<(String, u32)> = BTreeSet::new();
    for node in facts.bodies.iter().flat_map(|body| body.nodes.values()) {
        match &**node {
            Node::Opaque(one) if x87(one.semantics.op) => {
                match (one.semantics.name.as_deref(), &one.semantics.dests[..]) {
                    (Some("fsqrt"), _) => {
                        needed.insert(("llvm.sqrt.f64".to_owned(), 0));
                    }
                    (Some("fabs"), _) => {
                        needed.insert(("llvm.fabs.f64".to_owned(), 0));
                    }
                    (Some("fistp"), [Loc::Mem(mem)]) => {
                        needed.insert((format!("llvm.lrint.i{}.f64", mem.width * 8), mem.width * 8));
                    }
                    _ => {}
                }
            }
            Node::Call(call) => match helper(&call.name) {
                Some(Helper::Fist) => {
                    needed.insert(("llvm.lrint.i32.f64".to_owned(), 32));
                }
                Some(Helper::Fis2) => {
                    needed.insert(("llvm.lrint.i16.f64".to_owned(), 16));
                }
                _ => {}
            },
            _ => {}
        }
    }
    for (name, bits) in needed {
        let types = &mut module.context.types;
        let from = types.intern(Type::Float(FloatKind::Double));
        let returns = if bits == 0 { from } else { types.int(bits) };
        let ty = types.intern(Type::Function { returns, parameters: vec![from], variadic: false });
        let global = module.add_function(&name, ty, llrm_mir::Linkage::External)?;
        into.insert(name, (module.reference(global), ty));
    }
    Ok(())
}
