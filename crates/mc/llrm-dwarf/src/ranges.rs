//! `.debug_ranges` (2-4) and `.debug_rnglists` (5): the code a DIE covers when
//! it is not one range.

use llrm_debug::Range;
use llrm_object::{Object, Unsupported};

use crate::buffer::{Buf, Done};
use crate::refused;

/// DW_RLE_start_length and DW_RLE_end_of_list.
const RLE_START_LENGTH: u8 = 0x07;
const RLE_END_OF_LIST: u8 = 0x00;

/// Where the offsets table of a `.debug_rnglists` starts: after the 12-byte
/// header, which `DW_AT_rnglists_base` names.
pub const LISTS_BASE: u32 = 12;

/// The section holding `lists`, and the offset of each list in it. Before 5 a
/// list is pairs of addresses and two zeros end it; from 5 it is a header, a
/// table of offsets and `DW_RLE_start_length` entries, as LLVM's
/// `emitRangeList` lays them out ([the pin][pin]), except that LLVM names an
/// address by its `.debug_addr` index (`DW_RLE_startx_length`), a section this
/// writer does not make.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/AsmPrinter/DwarfDebug.cpp#L3480
pub fn section(
    object: &Object,
    lists: &[Vec<Range>],
    version: u16,
    address: usize,
) -> Result<(Done, Vec<u32>), Unsupported> {
    let mut buf = Buf::default();
    let mut starts = Vec::with_capacity(lists.len());
    if version >= 5 {
        // unit_length, version, address size, segment selector size, offset
        // entry count, then the offsets (from the first one).
        buf.u32(0);
        buf.u16(5);
        buf.u8(address as u8);
        buf.u8(0);
        buf.u32(lists.len() as u32);
        debug_assert_eq!(buf.at(), LISTS_BASE as usize);
        buf.bytes.resize(buf.at() + 4 * lists.len(), 0);
    }
    for list in lists {
        starts.push(buf.at() as u32);
        for range in list {
            let (symbol, base) = crate::anchor(object, range.section)?;
            let delta = range.offset as i64 - base as i64;
            if version >= 5 {
                buf.u8(RLE_START_LENGTH);
                buf.address(address, symbol, delta);
                buf.uleb(range.length as u64);
            } else {
                buf.address(address, symbol, delta);
                buf.address(address, symbol, delta + range.length as i64);
            }
        }
        if version >= 5 {
            buf.u8(RLE_END_OF_LIST);
        } else {
            buf.bytes.extend(std::iter::repeat_n(0, 2 * address));
        }
    }
    if version >= 5 {
        for (index, start) in starts.iter().enumerate() {
            let at = LISTS_BASE as usize + 4 * index;
            buf.patch32(at, start - LISTS_BASE);
        }
        let length = buf.at() as u32 - 4;
        buf.patch32(0, length);
    }
    Ok((buf.done(), starts))
}

/// A segmented address cannot be a range: its segment is one per DIE.
pub fn check(address: usize) -> Result<(), Unsupported> {
    if address == 2 {
        return refused("a DIE of several code ranges on a segmented target (DW_AT_segment is one per DIE)");
    }
    Ok(())
}
