//! The calls of emitted code and what their argument registers hold
//! (`DW_TAG_call_site`).
//!
//! # How it works
//! The emitted bytes are decoded. Each `call` is a site; its callee is the
//! symbol a fixup names in the instruction. For each argument register the
//! callee takes, the straight run of instructions before the call is read
//! backwards to the first that writes it:
//!
//! - `mov reg, imm` or `xor reg, reg`: a constant;
//! - `mov reg, saved` or `lea reg, [saved+d]`, `saved` a register calls
//!   preserve and no later instruction writes: that register plus `d`;
//! - none, back to the entry: the value the register had at entry.
//!
//! A branch, a call or a branch target ends the run, and the register is then
//! left out: a site that says less is true, one that says more may not be.
//!
//! # Example
//! ```text
//!  mov esi, 7        ; rsi = 7
//!  mov rdi, rbx      ; rdi = rbx + 0
//!  call g            ; site: g, return pc, rsi=7, rdi=rbx
//! ```
//!
//! # Runs
//! On the emitted code of a procedure, from `debuginfo`, for targets whose
//! description names argument registers.

use iced_x86::{Decoder, DecoderOptions, FlowControl, InstructionInfoFactory, Mnemonic, OpAccess, OpKind, Register};

/// What an argument register holds at a call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Constant(i64),
    /// What it held when the function was entered.
    Entry,
    /// A register calls preserve, plus a displacement.
    Register(String, i64),
}

/// A call found in the code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Call {
    /// The offset of the instruction after the call.
    pub offset: usize,
    /// The symbol it calls; None for a call through a register or a cell.
    pub symbol: Option<String>,
    /// Each argument register, with what it holds where that is known.
    pub parameters: Vec<(String, Value)>,
}

/// The calls of `code`, and whether it also leaves by a jump to another symbol
/// (a tail call, which a site would describe and this does not).
pub struct Scan {
    pub calls: Vec<Call>,
    pub tail_call: bool,
}

fn general(name: &str) -> Option<Register> {
    (0..16).map(|at| Register::RAX + at).find(|one| format!("{one:?}").eq_ignore_ascii_case(name))
}

/// `code` of a function of `bits`, whose fixups put a symbol at the offsets
/// `fixups` give. `arguments` and `preserved` are the registers a call passes
/// its arguments in and keeps; `wanted` says how many a callee takes, given
/// its symbol, and `entered` how many arrive in the function itself.
pub fn scan(
    code: &[u8],
    bits: u32,
    fixups: &[(usize, &str)],
    arguments: &[String],
    preserved: &[String],
    entered: usize,
    wanted: &dyn Fn(Option<&str>) -> usize,
) -> Scan {
    let mut decoder = Decoder::with_ip(bits, code, 0, DecoderOptions::NONE);
    let mut body = Vec::new();
    while decoder.can_decode() {
        let one = decoder.decode();
        if one.is_invalid() {
            return Scan { calls: Vec::new(), tail_call: false };
        }
        body.push(one);
    }
    let arguments: Vec<Register> = arguments.iter().filter_map(|name| general(name)).collect();
    let preserved: Vec<Register> = preserved.iter().filter_map(|name| general(name)).collect();
    let targets: Vec<u64> = body
        .iter()
        .filter(|one| matches!(
            one.flow_control(),
            FlowControl::ConditionalBranch | FlowControl::UnconditionalBranch
        ))
        .map(|one| one.near_branch_target())
        .collect();
    let named = |from: usize, to: usize| fixups.iter().find(|(at, _)| (from..to).contains(at)).map(|&(_, name)| name);
    let mut info = InstructionInfoFactory::new();
    let mut writes = |one: &iced_x86::Instruction, register: Register| {
        info.info(one)
            .used_registers()
            .iter()
            .any(
                |used| used.register().full_register() == register
                    && matches!(
                        used.access(),
                        OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite
                    ),
            )
    };
    let mut out = Scan { calls: Vec::new(), tail_call: false };
    for (at, one) in body.iter().enumerate() {
        let (from, to) = (one.ip() as usize, one.next_ip() as usize);
        match one.flow_control() {
            FlowControl::UnconditionalBranch if named(from, to).is_some() => out.tail_call = true,
            FlowControl::Call => {
                let symbol = named(from, to);
                let mut parameters = Vec::new();
                for &register in arguments.iter().take(wanted(symbol)) {
                    let mut value = None;
                    let mut entry = true;
                    for back in (0..at).rev() {
                        let before = &body[back];
                        if writes(before, register) {
                            entry = false;
                            value = match (before.mnemonic(), before.op0_kind(), before.op1_kind()) {
                                (Mnemonic::Mov, OpKind::Register, kind)
                                    if matches!(
                                        kind,
                                        OpKind::Immediate32
                                            | OpKind::Immediate64
                                            | OpKind::Immediate32to64
                                            | OpKind::Immediate8to32
                                            | OpKind::Immediate8to64
                                    ) && before.op0_register().full_register() == register
                                        && before.op0_register().size() >= 4 =>
                                {
                                    Some(Value::Constant(before.immediate(1) as i64))
                                }
                                (Mnemonic::Xor, OpKind::Register, OpKind::Register)
                                    if before.op0_register() == before.op1_register()
                                        && before.op0_register().size() == 4 =>
                                {
                                    Some(Value::Constant(0))
                                }
                                (Mnemonic::Mov, OpKind::Register, OpKind::Register)
                                    if before.op0_register() == register
                                        && preserved.contains(&before.op1_register()) =>
                                {
                                    let saved = before.op1_register();
                                    body[back + 1..at]
                                        .iter()
                                        .all(|between| !writes(between, saved))
                                        .then(|| Value::Register(format!("{saved:?}").to_lowercase(), 0))
                                }
                                (Mnemonic::Lea, OpKind::Register, OpKind::Memory)
                                    if before.op0_register() == register
                                        && preserved.contains(&before.memory_base())
                                        && before.memory_index() == Register::None
                                        && before.segment_prefix() == Register::None =>
                                {
                                    let saved = before.memory_base();
                                    body[back + 1..at]
                                        .iter()
                                        .all(|between| !writes(between, saved))
                                        .then(
                                            || Value::Register(
                                                format!("{saved:?}").to_lowercase(),
                                                before.memory_displacement64() as i64,
                                            ),
                                        )
                                }
                                _ => None,
                            };
                            break;
                        }
                        if before.flow_control() != FlowControl::Next || targets.contains(&before.ip()) {
                            entry = false;
                            break;
                        }
                    }
                    if entry && !targets.contains(&body[0].ip()) {
                        let first = arguments.iter().position(|one| *one == register).unwrap_or(usize::MAX);
                        if first < entered {
                            value = Some(Value::Entry);
                        }
                    }
                    if let Some(value) = value {
                        parameters.push((format!("{register:?}").to_lowercase(), value));
                    }
                }
                out.calls.push(Call { offset: to, symbol: symbol.map(str::to_owned), parameters });
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
#[path = "callsites_tests.rs"]
mod tests;
