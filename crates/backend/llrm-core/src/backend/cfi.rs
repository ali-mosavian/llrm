//! Call frame information, read back from the code as it was emitted: from
//! every place in a function, how to find the caller's frame (the canonical
//! frame address) and the registers the function saved.
//!
//! The code is decoded and followed from its entry along every path, so the
//! rule is the code's own, whatever produced it: a frame register, none (a
//! stack pointer that moves with each argument pushed), registers saved
//! where first needed and restored before the return that needs it. A rule the
//! code does not make single-valued (two paths reach a place with different
//! stack depths) is refused: no information is better than a wrong one.

use iced_x86::{
    Code, Decoder, DecoderOptions, FlowControl, Instruction, InstructionInfoFactory, Mnemonic, OpAccess, OpKind,
    Register,
};
use llrm_object::debug::FrameRow;

/// Where the canonical frame address is measured from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Base {
    Stack,
    Frame,
}

/// What is known at a place, before its instruction runs.
#[derive(Clone, Debug, Eq, PartialEq)]
struct State {
    base: Base,
    /// The frame address minus the stack pointer.
    stack: i64,
    /// The frame address minus the frame register, once it is set.
    frame: i64,
    /// A register and where its value at entry was saved: its distance below
    /// the frame address.
    saved: Vec<(Register, i64)>,
    /// Registers written since the entry: a push of one of those is no save.
    changed: Vec<Register>,
}

fn full(register: Register) -> Register {
    if register.is_gpr() { register.full_register32() } else { register }
}

fn name(register: Register) -> String {
    format!("{register:?}").to_lowercase()
}

/// The rows of the function whose bytes are `code`, entered with the frame
/// address `entry` bytes past the stack pointer (the return address). `frame`
/// is the register the code keeps its frame in, `stack` the stack pointer;
/// `pops` the bytes a call popped, by the offset the call ends at. Code no path
/// reaches (the targets of an indirect jump) is given no row, so it has the
/// rule of the code before it.
pub fn rows(
    code: &[u8],
    bits: u32,
    frame: Register,
    stack: Register,
    entry: i64,
    pops: &[(usize, i64)],
) -> Result<Vec<FrameRow>, String> {
    // Decoded where a path leads, never past a jump table or padding that is
    // not code.
    let mut decoder = Decoder::with_ip(bits, code, 0, DecoderOptions::NONE);
    let mut decode = |at: usize| -> Result<Instruction, String> {
        decoder.set_position(at).map_err(|_| format!("a branch to {at}, outside the function"))?;
        decoder.set_ip(at as u64);
        let one = decoder.decode();
        if one.is_invalid() { Err(format!("invalid code at {at}")) } else { Ok(one) }
    };
    let mut states: std::collections::BTreeMap<usize, (Instruction, State)> = std::collections::BTreeMap::new();
    let mut info = InstructionInfoFactory::new();
    let first = State { base: Base::Stack, stack: entry, frame: 0, saved: Vec::new(), changed: Vec::new() };
    let mut work = vec![(0usize, first)];
    while let Some((at, state)) = work.pop() {
        match states.get(&at) {
            Some((_, seen)) if seen.base == state.base && seen.stack == state.stack && seen.frame == state.frame => {
                continue;
            }
            Some((_, seen)) => {
                return Err(format!(
                    "the code reaches {at} with two stack depths ({:?} and {:?})",
                    (seen.base, seen.stack),
                    (state.base, state.stack)
                ));
            }
            None => {}
        }
        let one = decode(at)?;
        states.insert(at, (one, state.clone()));
        let after = step(&one, state, frame, stack, pops, &mut info)?;
        let next = at + one.len();
        let jump = (matches!(one.op0_kind(), OpKind::NearBranch32 | OpKind::NearBranch16))
            .then(|| one.near_branch_target() as usize);
        let (falls, jumps) = match one.flow_control() {
            FlowControl::Next
            | FlowControl::Call
            | FlowControl::IndirectCall
            | FlowControl::XbeginXabortXend
            | FlowControl::Interrupt
            | FlowControl::Exception => (true, false),
            FlowControl::ConditionalBranch => (true, true),
            FlowControl::UnconditionalBranch => (false, true),
            FlowControl::Return | FlowControl::IndirectBranch => (false, false),
        };
        if falls && next < code.len() {
            work.push((next, after.clone()));
        }
        // A jump out of the function (a tail call) ends the path.
        if let Some(target) = jump.filter(|&target| jumps && target < code.len()) {
            work.push((target, after));
        }
    }
    let mut out: Vec<FrameRow> = Vec::new();
    for (&at, (_, state)) in &states {
        let row = FrameRow {
            offset: at,
            cfa_register: name(if state.base == Base::Stack { stack } else { frame }),
            cfa_offset: if state.base == Base::Stack { state.stack } else { state.frame },
            saved: state.saved.iter().map(|&(register, below)| (name(register), -below)).collect(),
        };
        if out.last().is_none_or(|last| {
            (&last.cfa_register, last.cfa_offset, &last.saved) != (&row.cfa_register, row.cfa_offset, &row.saved)
        }) {
            out.push(row);
        }
    }
    Ok(out)
}

/// The state after `one`, from the state before it.
fn step(
    one: &Instruction,
    mut state: State,
    frame: Register,
    stack: Register,
    pops: &[(usize, i64)],
    info: &mut InstructionInfoFactory,
) -> Result<State, String> {
    let written: Vec<Register> = info
        .info(one)
        .used_registers()
        .iter()
        .filter(|used| {
            matches!(
                used.access(),
                OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite | OpAccess::ReadCondWrite
            )
        })
        .map(|used| full(used.register()))
        .collect();
    let mnemonic = one.mnemonic();
    let end = (one.ip() + one.len() as u64) as usize;
    let immediate = || {
        (one.op1_kind() != OpKind::Register && one.op1_kind() != OpKind::Memory).then(|| one.immediate(1) as i32 as i64)
    };
    match mnemonic {
        Mnemonic::Call => {
            // The return address is pushed and popped by the callee; what it
            // pops of the arguments is the stack's.
            state.stack -= pops.iter().filter(|(at, _)| *at == end).map(|(_, bytes)| bytes).sum::<i64>();
            return Ok(state);
        }
        Mnemonic::Ret | Mnemonic::Retf | Mnemonic::Iret | Mnemonic::Iretd => return Ok(state),
        Mnemonic::Push
        | Mnemonic::Pushad
        | Mnemonic::Pushfd
        | Mnemonic::Pushf
        | Mnemonic::Pop
        | Mnemonic::Popad
        | Mnemonic::Popfd
        | Mnemonic::Popf => {
            let moved = -i64::from(one.stack_pointer_increment());
            state.stack += moved;
            if mnemonic == Mnemonic::Push && one.op0_kind() == OpKind::Register {
                let register = full(one.op0_register());
                if register.is_gpr32()
                    && register != stack
                    && !state.changed.contains(&register)
                    && !state.saved.iter().any(|(saved, _)| *saved == register)
                {
                    state.saved.push((register, state.stack));
                }
            }
            if mnemonic == Mnemonic::Pop && one.op0_kind() == OpKind::Register {
                let register = full(one.op0_register());
                // Its own slot, popped: it has the entry value again.
                if let Some(at) =
                    state.saved.iter().position(|&(saved, below)| saved == register && below == state.stack + 4)
                {
                    state.saved.remove(at);
                    state.changed.retain(|one| *one != register);
                } else if !state.changed.contains(&register) {
                    state.changed.push(register);
                }
            }
            return Ok(state);
        }
        Mnemonic::Leave => {
            // MOV esp, ebp; POP ebp.
            state.stack = state.frame;
            state.base = Base::Stack;
            state.stack -= 4;
            state.saved.retain(|&(register, below)| !(register == full(frame) && below == state.stack + 4));
            state.changed.retain(|one| *one != full(frame));
            return Ok(state);
        }
        Mnemonic::Sub | Mnemonic::Add if full(one.op0_register()) == stack && one.op0_kind() == OpKind::Register => {
            let Some(amount) = immediate() else {
                return Err(format!("{:?} of the stack pointer by a register at {}", mnemonic, one.ip()));
            };
            state.stack += if mnemonic == Mnemonic::Sub { amount } else { -amount };
            return Ok(state);
        }
        Mnemonic::Mov if one.op0_kind() == OpKind::Register && one.op1_kind() == OpKind::Register => {
            let (to, from) = (full(one.op0_register()), full(one.op1_register()));
            if to == full(frame) && from == stack && state.base == Base::Stack {
                // The frame register now stands where the stack pointer does.
                state.base = Base::Frame;
                state.frame = state.stack;
                state.changed.push(to);
                return Ok(state);
            }
            if to == stack && from == full(frame) {
                state.stack = state.frame;
                state.base = Base::Stack;
                return Ok(state);
            }
        }
        _ => {}
    }
    if written.contains(&stack) && !matches!(one.code(), Code::Pushad | Code::Popad) {
        return Err(format!("{mnemonic:?} writes the stack pointer at {}, which is not followed", one.ip()));
    }
    // A write to the frame register while it holds the frame address would move
    // it.
    if state.base == Base::Frame && written.contains(&full(frame)) {
        return Err(format!("{mnemonic:?} writes the frame register at {}", one.ip()));
    }
    for register in written {
        if !state.changed.contains(&register) {
            state.changed.push(register);
        }
    }
    Ok(state)
}

#[cfg(test)]
#[path = "cfi_tests.rs"]
mod cfi_tests;
