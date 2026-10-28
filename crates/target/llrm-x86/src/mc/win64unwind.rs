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
//! `Info::bytes` takes a list of codes, so a declared prologue can feed it as
//! `from_prologue` does. Saves are recorded relative to `rsp` even when the
//! code addresses them from the frame register.
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

/// What one unwind code does (`UNWIND_CODE`), with the sizes and offsets in
/// bytes: the encoding (small or large, near or far) is the builder's choice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Op {
    /// `push` of a non-volatile register, by its hardware number.
    PushNonvol(u8),
    /// `sub rsp, size`.
    Alloc(u64),
    /// The frame register is set (`SET_FPREG`; its number and offset are the
    /// info's).
    SetFpreg,
    /// A non-volatile register saved at `rsp + offset`.
    SaveNonvol(u8, u64),
    /// An `xmm` register saved at `rsp + offset`.
    SaveXmm128(u8, u64),
}

/// An unwind code, from the offset after its instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Code {
    pub offset: u8,
    pub op: Op,
}

/// One function's `UNWIND_INFO`: the prologue's size, the frame register and
/// its offset in 16-byte units, and the codes in execution order. Any
/// producer of this list uses the builder: reading the prologue from the
/// code's bytes (`from_prologue`) is one; a declared prologue is another.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Info {
    pub prolog: u8,
    pub frame: Option<(u8, u8)>,
    pub codes: Vec<Code>,
}

impl Info {
    /// The bytes: the codes last-executed first, padded to an even slot count
    /// (https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64#struct-unwind_info).
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let mut groups: Vec<Vec<u16>> = Vec::new();
        for code in &self.codes {
            let at = |operation, info| slot(code.offset, operation, info);
            groups.push(match code.op {
                Op::PushNonvol(register) => vec![at(PUSH_NONVOL, register)],
                Op::Alloc(size) if size % 8 != 0 || size == 0 => {
                    return Err("a stack allocation that is not a multiple of 8".into());
                }
                Op::Alloc(size @ 8..=128) => vec![at(ALLOC_SMALL, (size / 8 - 1) as u8)],
                Op::Alloc(size) if size / 8 <= 0xFFFF => vec![at(ALLOC_LARGE, 0), (size / 8) as u16],
                Op::Alloc(size) if size <= u64::from(u32::MAX) => {
                    vec![at(ALLOC_LARGE, 1), size as u16, (size >> 16) as u16]
                }
                Op::Alloc(_) => return Err("a stack allocation past 4 GiB".into()),
                Op::SetFpreg => vec![at(SET_FPREG, 0)],
                Op::SaveNonvol(register, offset) | Op::SaveXmm128(register, offset) => {
                    let xmm = matches!(code.op, Op::SaveXmm128(..));
                    let unit = if xmm { 16 } else { 8 };
                    if offset % unit != 0 {
                        return Err(format!("a register saved at an offset that is not a multiple of {unit}"));
                    }
                    if offset / unit <= 0xFFFF {
                        vec![at(if xmm { SAVE_XMM128 } else { SAVE_NONVOL }, register), (offset / unit) as u16]
                    } else if offset <= u64::from(u32::MAX) {
                        vec![
                            at(if xmm { SAVE_XMM128_FAR } else { SAVE_NONVOL_FAR }, register),
                            offset as u16,
                            (offset >> 16) as u16,
                        ]
                    } else {
                        return Err("a save offset past 4 GiB".into());
                    }
                }
            });
        }
        let count: usize = groups.iter().map(Vec::len).sum();
        if count > 255 {
            return Err("more than 255 unwind code slots".into());
        }
        let mut slots: Vec<u16> = groups.iter().rev().flatten().copied().collect();
        if slots.len() % 2 == 1 {
            slots.push(0);
        }
        let (register, offset) = self.frame.unwrap_or((0, 0));
        let mut out = vec![1, self.prolog, count as u8, register | (offset << 4)];
        out.extend(slots.iter().flat_map(|slot| slot.to_le_bytes()));
        Ok(out)
    }
}

fn slot(
    offset: u8,
    operation: u8,
    info: u8,
) -> u16 {
    u16::from(offset) | (u16::from(operation) | u16::from(info) << 4) << 8
}

fn refused(
    function: &str,
    why: &str,
) -> Unsupported {
    Unsupported(format!("{function}: no Win64 unwind data for its prologue: {why}"))
}

/// Where operand `n` of `instruction` is, as an offset from `rsp` once the
/// prologue is done: its displacement from `rsp`, or from the frame register
/// plus the frame's offset (LLVM saves relative to the frame register when
/// there is one, and the codes still say `rsp`-relative).
fn from_rsp(
    instruction: &Instruction,
    n: u32,
    frame: Option<(u8, u8)>,
) -> Option<i64> {
    if instruction.op_kind(n) != OpKind::Memory || instruction.memory_index() != Register::None {
        return None;
    }
    let displacement = instruction.memory_displacement64() as i64;
    match (instruction.memory_base(), frame) {
        (Register::RSP, _) => Some(displacement),
        (base, Some((register, offset))) if gpr(base) == Some(register) => Some(displacement + i64::from(offset) * 16),
        _ => None,
    }
}

/// The unwind data of the function whose bytes are `code`.
pub fn from_prologue(
    function: &str,
    code: &[u8],
) -> Result<Info, Unsupported> {
    let mut out = Info { prolog: 0, frame: None, codes: Vec::new() };
    let mut decoder = Decoder::with_ip(64, code, 0, DecoderOptions::NONE);
    while decoder.can_decode() {
        let instruction = decoder.decode();
        let end = decoder.position();
        if end > 255 {
            return Err(refused(function, "a prologue past 255 bytes"));
        }
        let offset = end as u8;
        let op = match instruction.mnemonic() {
            Mnemonic::Push if instruction.op0_kind() == OpKind::Register => {
                match nonvolatile(instruction.op0_register()) {
                    Some(register) => Op::PushNonvol(register),
                    // `push rax` as an 8-byte allocation.
                    None if gpr(instruction.op0_register()).is_some() => Op::Alloc(8),
                    None => break,
                }
            }
            Mnemonic::Sub
                if instruction.op0_register() == Register::RSP && instruction.op1_kind() != OpKind::Register =>
            {
                Op::Alloc(instruction.immediate(1))
            }
            Mnemonic::Lea | Mnemonic::Mov if out.frame.is_none() && is_frame_set(&instruction) => {
                let displacement = if instruction.mnemonic() == Mnemonic::Lea {
                    from_rsp(&instruction, 1, None).unwrap_or(0)
                } else {
                    0
                };
                if displacement % 16 != 0 || !(0..=240).contains(&displacement) {
                    return Err(refused(function, "a frame register offset past 240 or not a multiple of 16"));
                }
                out.frame = Some((gpr(instruction.op0_register()).expect("checked"), (displacement / 16) as u8));
                Op::SetFpreg
            }
            Mnemonic::Mov if instruction.op1_kind() == OpKind::Register => {
                match (from_rsp(&instruction, 0, out.frame), nonvolatile(instruction.op1_register())) {
                    (Some(at), Some(register)) if at >= 0 => Op::SaveNonvol(register, at as u64),
                    _ => break,
                }
            }
            Mnemonic::Movaps
            | Mnemonic::Movups
            | Mnemonic::Movapd
            | Mnemonic::Movupd
            | Mnemonic::Movdqa
            | Mnemonic::Movdqu
                if instruction.op1_kind() == OpKind::Register =>
            {
                let xmm = instruction.op1_register();
                match (from_rsp(&instruction, 0, out.frame), xmm.is_xmm().then(|| xmm.number() as u8)) {
                    (Some(at), Some(number)) if number >= 6 && at >= 0 => Op::SaveXmm128(number, at as u64),
                    _ => break,
                }
            }
            _ => break,
        };
        out.codes.push(Code { offset, op });
        out.prolog = offset;
    }
    out.bytes().map_err(|why| refused(function, &why))?;
    Ok(out)
}

/// `mov reg, rsp` or `lea reg, [rsp+d]`.
fn is_frame_set(instruction: &Instruction) -> bool {
    if instruction.op0_kind() != OpKind::Register || gpr(instruction.op0_register()).is_none() {
        return false;
    }
    match instruction.mnemonic() {
        Mnemonic::Mov => instruction.op1_kind() == OpKind::Register && instruction.op1_register() == Register::RSP,
        _ => from_rsp(instruction, 1, None).is_some(),
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
        let info = from_prologue(
            &name,
            code.get(offset..offset + size).ok_or_else(|| refused(&name, "a body past its section"))?,
        )?;

        let at = xdata_image.len();
        xdata_image.extend(info.bytes().map_err(|why| refused(&name, &why))?);
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
