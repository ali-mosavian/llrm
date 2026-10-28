//! Win64 unwind data for an x86-64 COFF object: `.xdata` (`UNWIND_INFO`) and
//! `.pdata` (`RUNTIME_FUNCTION`) for each function of `.text`.
//!
//! # How it works
//! There is no Win64 frame lowering to declare the prologue (DEVIATE from
//! LLVM's `.seh_*` directives, `X86WinCOFFStreamer`), so the prologue is read
//! back from the function's final bytes: a straight-line run of `push` of a
//! register, `sub rsp, imm`, `lea`/`mov` of the frame register from `rsp`, and
//! `mov`/`movaps` of a non-volatile register to `[rsp+d]`. The run ends at the
//! first other instruction; a prologue that does not fit what the unwind codes
//! can say is refused, not described wrongly. Each code is stamped with the
//! offset after its instruction.
//!
//! ```text
//!  push rbp            01  PUSH_NONVOL rbp          offset 1
//!  sub  rsp, 32        05  ALLOC_SMALL 32           offset 5
//!  =>  01 05 02 00 | 05 32 | 01 50                  version, prolog, 2 slots, no frame | codes, last first
//! ```
//!
//! A function that needs a stack probe (`__chkstk`) is not described: its
//! `sub rsp` follows the call, which ends the run.
//!
//! # Used by
//! `X86Objects::write_object`, for a 64-bit COFF object only.

use iced_x86::{Decoder, DecoderOptions, Instruction, Mnemonic, OpKind, Register};
use llrm_object::{Binding, Definition, Fix, Object, Reloc, Role, Section, Symbol, Target, Unsupported};

const PUSH_NONVOL: u8 = 0;
const ALLOC_LARGE: u8 = 1;
const ALLOC_SMALL: u8 = 2;
const SET_FPREG: u8 = 3;
const SAVE_NONVOL: u8 = 4;
const SAVE_NONVOL_FAR: u8 = 5;
const SAVE_XMM128: u8 = 8;
const SAVE_XMM128_FAR: u8 = 9;

/// The registers a function must keep (`rsp` excluded); `.xdata` names them
/// by their hardware number.
const NONVOLATILE: [u8; 8] = [3, 5, 6, 7, 12, 13, 14, 15];

fn gpr(register: Register) -> Option<u8> {
    (register.is_gpr64() && register != Register::RIP).then(|| register.number() as u8)
}

fn nonvolatile(register: Register) -> Option<u8> {
    gpr(register).filter(|number| NONVOLATILE.contains(number))
}

/// One function's `UNWIND_INFO`: the prologue's size, the frame register and
/// its offset, and the codes' slots in execution order.
struct Unwind {
    prolog: u8,
    frame: Option<(u8, u8)>,
    slots: Vec<u16>,
}

impl Unwind {
    /// The bytes: the codes last-executed first, padded to an even slot count.
    fn bytes(&self) -> Vec<u8> {
        let mut groups: Vec<&[u16]> = Vec::new();
        let mut at = 0;
        while at < self.slots.len() {
            let width = slot_count(self.slots[at]);
            groups.push(&self.slots[at..at + width]);
            at += width;
        }
        let mut slots: Vec<u16> = groups.iter().rev().flat_map(|group| group.iter().copied()).collect();
        if slots.len() % 2 == 1 {
            slots.push(0);
        }
        let (register, offset) = self.frame.unwrap_or((0, 0));
        let mut out = vec![1, self.prolog, (self.slots_len()) as u8, register | (offset << 4)];
        out.extend(slots.iter().flat_map(|slot| slot.to_le_bytes()));
        out
    }

    /// `CountOfCodes`: the slots before padding.
    fn slots_len(&self) -> usize {
        self.slots.len()
    }
}

/// The slots one code takes, from its first slot's operation.
fn slot_count(first: u16) -> usize {
    let operation = ((first >> 8) & 0xF) as u8;
    let info = (first >> 12) as u8;
    match operation {
        ALLOC_LARGE => 2 + usize::from(info),
        SAVE_NONVOL | SAVE_XMM128 => 2,
        SAVE_NONVOL_FAR | SAVE_XMM128_FAR => 3,
        _ => 1,
    }
}

fn slot(
    offset: usize,
    operation: u8,
    info: u8,
) -> u16 {
    offset as u16 | (u16::from(operation) | u16::from(info) << 4) << 8
}

fn refused(
    function: &str,
    why: &str,
) -> Unsupported {
    Unsupported(format!("{function}: no Win64 unwind data for its prologue: {why}"))
}

/// The memory operand `[rsp + displacement]` of `instruction`'s operand `n`.
fn at_rsp(
    instruction: &Instruction,
    n: u32,
) -> Option<u64> {
    (instruction.op_kind(n) == OpKind::Memory
        && instruction.memory_base() == Register::RSP
        && instruction.memory_index() == Register::None)
        .then(|| instruction.memory_displacement64())
}

/// The unwind data of the function whose bytes are `code`.
fn unwind(
    function: &str,
    code: &[u8],
) -> Result<Unwind, Unsupported> {
    let mut out = Unwind { prolog: 0, frame: None, slots: Vec::new() };
    let mut decoder = Decoder::with_ip(64, code, 0, DecoderOptions::NONE);
    while decoder.can_decode() {
        let instruction = decoder.decode();
        let end = decoder.position();
        let mut slots = Vec::new();
        match instruction.mnemonic() {
            Mnemonic::Push if instruction.op0_kind() == OpKind::Register => {
                match nonvolatile(instruction.op0_register()) {
                    Some(register) => slots.push(slot(end, PUSH_NONVOL, register)),
                    // `push rax` as an 8-byte allocation.
                    None if gpr(instruction.op0_register()).is_some() => slots.push(slot(end, ALLOC_SMALL, 0)),
                    None => break,
                }
            }
            Mnemonic::Sub
                if instruction.op0_register() == Register::RSP && instruction.op1_kind() != OpKind::Register =>
            {
                let size = instruction.immediate(1);
                if size % 8 != 0 || size == 0 {
                    return Err(refused(function, "a stack allocation that is not a multiple of 8"));
                }
                match size {
                    8..=128 => slots.push(slot(end, ALLOC_SMALL, (size / 8 - 1) as u8)),
                    _ if size / 8 <= 0xFFFF => {
                        slots.push(slot(end, ALLOC_LARGE, 0));
                        slots.push((size / 8) as u16);
                    }
                    _ if size <= u64::from(u32::MAX) => {
                        slots.push(slot(end, ALLOC_LARGE, 1));
                        slots.push(size as u16);
                        slots.push((size >> 16) as u16);
                    }
                    _ => return Err(refused(function, "a stack allocation past 4 GiB")),
                }
            }
            Mnemonic::Lea | Mnemonic::Mov if out.frame.is_none() && is_frame_set(&instruction) => {
                let displacement =
                    if instruction.mnemonic() == Mnemonic::Lea { at_rsp(&instruction, 1).unwrap_or(0) } else { 0 };
                if displacement % 16 != 0 || displacement > 240 {
                    return Err(refused(function, "a frame register offset past 240 or not a multiple of 16"));
                }
                let register = gpr(instruction.op0_register()).expect("checked");
                out.frame = Some((register, (displacement / 16) as u8));
                slots.push(slot(end, SET_FPREG, 0));
            }
            Mnemonic::Mov if instruction.op1_kind() == OpKind::Register => {
                match (at_rsp(&instruction, 0), nonvolatile(instruction.op1_register())) {
                    (Some(displacement), Some(register)) => {
                        if displacement % 8 != 0 {
                            return Err(refused(function, "a register saved at an offset that is not a multiple of 8"));
                        }
                        if displacement / 8 <= 0xFFFF {
                            slots.push(slot(end, SAVE_NONVOL, register));
                            slots.push((displacement / 8) as u16);
                        } else {
                            slots.push(slot(end, SAVE_NONVOL_FAR, register));
                            slots.push(displacement as u16);
                            slots.push((displacement >> 16) as u16);
                        }
                    }
                    _ => break,
                }
            }
            Mnemonic::Movaps | Mnemonic::Movups | Mnemonic::Movdqa | Mnemonic::Movdqu
                if instruction.op1_kind() == OpKind::Register =>
            {
                let xmm = instruction.op1_register();
                match (at_rsp(&instruction, 0), xmm.is_xmm().then(|| xmm.number() as u8)) {
                    (Some(displacement), Some(number)) if number >= 6 => {
                        if displacement % 16 != 0 {
                            return Err(refused(
                                function,
                                "an xmm register saved at an offset that is not a multiple of 16",
                            ));
                        }
                        if displacement / 16 <= 0xFFFF {
                            slots.push(slot(end, SAVE_XMM128, number));
                            slots.push((displacement / 16) as u16);
                        } else {
                            slots.push(slot(end, SAVE_XMM128_FAR, number));
                            slots.push(displacement as u16);
                            slots.push((displacement >> 16) as u16);
                        }
                    }
                    _ => break,
                }
            }
            _ => break,
        }
        if end > 255 {
            return Err(refused(function, "a prologue past 255 bytes"));
        }
        out.slots.extend(slots);
        out.prolog = end as u8;
    }
    if out.slots.len() > 255 {
        return Err(refused(function, "more than 255 unwind code slots"));
    }
    Ok(out)
}

/// `mov reg, rsp` or `lea reg, [rsp+d]`.
fn is_frame_set(instruction: &Instruction) -> bool {
    if instruction.op0_kind() != OpKind::Register || gpr(instruction.op0_register()).is_none() {
        return false;
    }
    match instruction.mnemonic() {
        Mnemonic::Mov => instruction.op1_kind() == OpKind::Register && instruction.op1_register() == Register::RSP,
        _ => at_rsp(instruction, 1).is_some(),
    }
}

/// `object` with `.xdata` and `.pdata` for every function of its text, the
/// functions being the defined symbols of a text section that have a size. An
/// object with none is returned as it is.
pub fn expanded(object: &Object) -> Result<Object, Unsupported> {
    let mut functions: Vec<(usize, usize, usize, usize)> = object
        .symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| match symbol.definition {
            Definition::Defined { section, offset }
                if symbol.size > 0 && object.sections[section].role == Role::Text =>
            {
                Some((section, offset, symbol.size, index))
            }
            _ => None,
        })
        .collect();
    if functions.is_empty() {
        return Ok(object.clone());
    }
    functions.sort_unstable();
    functions.dedup_by_key(|one| (one.0, one.1));

    let mut object = object.clone();
    let xdata = object.sections.len();
    let mut xdata_image: Vec<u8> = Vec::new();
    let mut pdata_image: Vec<u8> = Vec::new();
    let mut pdata_relocs: Vec<Reloc> = Vec::new();
    for (section, offset, size, index) in functions {
        let name = object.symbols[index].name.clone();
        let code = &object.sections[section].image;
        let info =
            unwind(&name, code.get(offset..offset + size).ok_or_else(|| refused(&name, "a body past its section"))?)?;

        let at = xdata_image.len();
        xdata_image.extend(info.bytes());
        object
            .symbols
            .push(
                Symbol {
                    name: format!("$unwind${name}"),
                    binding: Binding::Local,
                    definition: Definition::Defined { section: xdata, offset: at },
                    size: 0,
                    group: None,
                },
            );
        let unwind_symbol = object.symbols.len() - 1;

        let entry = pdata_image.len();
        pdata_image.extend([0; 12]);
        let rva = Fix::image_relative(4);
        pdata_relocs.push(Reloc::new(entry, rva, Target::Symbol(index), 0));
        pdata_relocs.push(Reloc::new(entry + 4, rva, Target::Symbol(index), size as i64));
        pdata_relocs.push(Reloc::new(entry + 8, rva, Target::Symbol(unwind_symbol), 0));
    }
    let section = |name: &str, image: Vec<u8>, relocs: Vec<Reloc>| Section {
        name: name.to_owned(),
        role: Role::ROData,
        near: true,
        align: 4,
        spans: vec![[0, image.len()]],
        image,
        relocs,
    };
    object.sections.push(section(".xdata", xdata_image, Vec::new()));
    object.sections.push(section(".pdata", pdata_image, pdata_relocs));
    Ok(object)
}

#[cfg(test)]
#[path = "win64unwind_tests.rs"]
mod tests;
