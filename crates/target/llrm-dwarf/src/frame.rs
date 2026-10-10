//! `.debug_frame`: one CIE (the rule at every function's entry) and an FDE for
//! each function whose code could be followed, its rows as `DW_CFA_*`
//! instructions: a debugger finds the caller's frame from them where
//! prologue analysis has nothing to go on (a frameless function, registers
//! saved where first needed).

use llrm_object::debug::{FrameRow, Info};
use llrm_object::{Object, Unsupported};

use crate::buffer::{Buf, Done};
use crate::refused;

const DW_CFA_ADVANCE_LOC: u8 = 0x40;
const DW_CFA_OFFSET: u8 = 0x80;
const DW_CFA_RESTORE: u8 = 0xC0;
const DW_CFA_ADVANCE_LOC1: u8 = 0x02;
const DW_CFA_ADVANCE_LOC2: u8 = 0x03;
const DW_CFA_ADVANCE_LOC4: u8 = 0x04;
const DW_CFA_DEF_CFA: u8 = 0x0C;
const DW_CFA_DEF_CFA_REGISTER: u8 = 0x0D;
const DW_CFA_DEF_CFA_OFFSET: u8 = 0x0E;
/// The version of a CIE in DWARF 4 and 5: it names the address size and the
/// segment selector size.
const CIE_VERSION: u8 = 4;
/// What a CIE's id field holds in `.debug_frame`.
const CIE_ID: u32 = 0xFFFF_FFFF;
/// Every offset a register is saved at is a multiple of this, below the frame
/// address.
const DATA_ALIGNMENT: i64 = -4;

fn number(
    info: &Info,
    name: &str,
) -> Result<u64, Unsupported> {
    match info.registers.iter().find(|one| one.name == name).and_then(|one| one.dwarf) {
        Some(number) => Ok(u64::from(number)),
        None => refused(format!("register {name} has no DWARF number, which a frame rule names")),
    }
}

/// The instructions that take the state `from` to `to`: its frame address, then
/// each register's rule.
fn changes(
    info: &Info,
    buf: &mut Buf,
    from: &FrameRow,
    to: &FrameRow,
) -> Result<(), Unsupported> {
    if (&from.cfa_register, from.cfa_offset) != (&to.cfa_register, to.cfa_offset) {
        if from.cfa_register == to.cfa_register {
            buf.u8(DW_CFA_DEF_CFA_OFFSET);
            buf.uleb(to.cfa_offset as u64);
        } else if from.cfa_offset == to.cfa_offset {
            buf.u8(DW_CFA_DEF_CFA_REGISTER);
            buf.uleb(number(info, &to.cfa_register)?);
        } else {
            buf.u8(DW_CFA_DEF_CFA);
            buf.uleb(number(info, &to.cfa_register)?);
            buf.uleb(to.cfa_offset as u64);
        }
    }
    for (register, at) in &to.saved {
        if !from.saved.iter().any(|(one, was)| one == register && was == at) {
            let register = number(info, register)?;
            if register >= 0x40 {
                return refused(format!("register {register} is past the short form of DW_CFA_offset"));
            }
            buf.u8(DW_CFA_OFFSET | register as u8);
            buf.uleb((at / DATA_ALIGNMENT) as u64);
        }
    }
    for (register, _) in &from.saved {
        if !to.saved.iter().any(|(one, _)| one == register) {
            let register = number(info, register)?;
            if register >= 0x40 {
                return refused(format!("register {register} is past the short form of DW_CFA_restore"));
            }
            buf.u8(DW_CFA_RESTORE | register as u8);
        }
    }
    Ok(())
}

fn advance(
    buf: &mut Buf,
    delta: usize,
) {
    match delta {
        0 => {}
        1..=0x3F => buf.u8(DW_CFA_ADVANCE_LOC | delta as u8),
        0x40..=0xFF => {
            buf.u8(DW_CFA_ADVANCE_LOC1);
            buf.u8(delta as u8);
        }
        0x100..=0xFFFF => {
            buf.u8(DW_CFA_ADVANCE_LOC2);
            buf.u16(delta as u16);
        }
        _ => {
            buf.u8(DW_CFA_ADVANCE_LOC4);
            buf.u32(delta as u32);
        }
    }
}

/// The section, None where no function has rows or the target numbers no return
/// address.
pub fn section(
    object: &Object,
    info: &Info,
    address: usize,
) -> Result<Option<Done>, Unsupported> {
    let Some(entry) = info.functions.iter().find_map(|one| one.frame.first()) else { return Ok(None) };
    if info.return_register.is_empty() {
        return Ok(None);
    }
    let return_column = number(info, &info.return_register)?;
    let mut buf = Buf::default();
    // The CIE: the frame address at entry, and the return address in its slot
    // just below it.
    buf.u32(0);
    buf.u32(CIE_ID);
    buf.u8(CIE_VERSION);
    buf.u8(0);
    buf.u8(address as u8);
    // A 16-bit program's frame descriptions name the segment their code is in.
    buf.u8(if address == 2 { 2 } else { 0 });
    buf.uleb(1);
    buf.sleb(DATA_ALIGNMENT);
    buf.uleb(return_column);
    buf.u8(DW_CFA_DEF_CFA);
    buf.uleb(number(info, &entry.cfa_register)?);
    buf.uleb(entry.cfa_offset as u64);
    buf.u8(DW_CFA_OFFSET | return_column as u8);
    buf.uleb(1);
    while buf.at() % address != 0 {
        buf.u8(0);
    }
    let length = buf.at() as u32 - 4;
    buf.patch32(0, length);
    let initial = FrameRow {
        offset: 0,
        cfa_register: entry.cfa_register.clone(),
        cfa_offset: entry.cfa_offset,
        saved: Vec::new(),
    };
    for function in info.functions.iter().filter(|one| !one.frame.is_empty()) {
        let [range] = function.ranges[..] else { continue };
        let start = buf.at();
        buf.u32(0);
        // The CIE is the section's first record.
        buf.u32(0);
        let (symbol, base) = crate::anchor(object, range.section)?;
        if address == 2 {
            buf.segment(range.section);
        }
        buf.address(address, symbol, range.offset as i64 - base as i64);
        buf.bytes.extend((range.length as u64).to_le_bytes().iter().take(address));
        let mut state = &initial;
        for row in &function.frame {
            if row.offset > 0 || state != row {
                advance(&mut buf, row.offset - state.offset.min(row.offset));
            }
            changes(info, &mut buf, state, row)?;
            state = row;
        }
        while buf.at() % address != 0 {
            buf.u8(0);
        }
        let length = (buf.at() - start - 4) as u32;
        buf.patch32(start, length);
    }
    Ok(Some(buf.done()))
}
