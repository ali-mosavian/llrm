//! A classic Mac 68k application's resource fork: `CODE 0` (the A5 world and
//! jump table) and `CODE 1` (the code), in the resource-file layout of Inside
//! Macintosh.
//!
//! Calls are pc-relative (`jsr (d16,%pc)`): a segment is loaded anywhere and
//! carries no relocations. `CODE 1` starts with its two header words (the
//! first jump table entry it owns, the count), and the jump table entry's
//! offset counts from the start of the resource, header included.
//!
//! DEVIATE: the layout follows the book and is not run: no Mac emulator or ROM
//! is installed. Only the structure is tested.

use crate::compile::Item;
use crate::object::{Assembled, assemble, call, item};

/// The bytes before a segment's code.
const SEGMENT_HEADER: u16 = 4;
/// Where the resource data starts (the 16-byte header, 112 reserved and 128
/// application-data bytes).
const DATA_AT: u32 = 256;
/// The application parameters above `A5`.
const ABOVE_A5: u32 = 32;

/// `_start`: [`call`] (pc-relative), then `rts` to the caller.
pub fn start(
    callee: &str,
    args: &[i64],
) -> Vec<Item> {
    let mut out = vec![Item::Label("_start".to_owned())];
    out.extend(call(callee, args));
    out.push(item("RTS", vec![]));
    out
}

/// A call through an absolute address becomes pc-relative.
fn position_independent(items: &[Item]) -> Vec<Item> {
    items
        .iter()
        .map(|one| match one {
            Item::Inst(inst) if inst.opcode == "CALLb" => {
                Item::Inst(crate::mc::Inst { opcode: "CALLq", operands: inst.operands.clone() })
            }
            Item::Inst(inst) => Item::Inst(inst.clone()),
            Item::Label(name) => Item::Label(name.clone()),
        })
        .collect()
}

fn resource_fork(resources: &[(&[u8; 4], i16, Vec<u8>)]) -> Vec<u8> {
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    for (_, _, bytes) in resources {
        offsets.push(data.len() as u32);
        data.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        data.extend_from_slice(bytes);
    }
    // the map: header copy, handle, file reference, attributes, offsets of the
    // type list and the name list, then the type list and reference lists
    let mut types: Vec<&[u8; 4]> = Vec::new();
    for (kind, ..) in resources {
        if !types.contains(kind) {
            types.push(kind);
        }
    }
    let type_list = 28u16;
    let type_list_len = 2 + 8 * types.len();
    let mut map = vec![0u8; 24];
    map.extend_from_slice(&type_list.to_be_bytes());
    map.extend_from_slice(&((type_list as usize + type_list_len + 12 * resources.len()) as u16).to_be_bytes());
    map.extend_from_slice(&(types.len() as u16 - 1).to_be_bytes());
    let mut reference = type_list_len as u16;
    for kind in &types {
        let count = resources.iter().filter(|(one, ..)| one == kind).count();
        map.extend_from_slice(*kind);
        map.extend_from_slice(&(count as u16 - 1).to_be_bytes());
        map.extend_from_slice(&reference.to_be_bytes());
        reference += 12 * count as u16;
    }
    for kind in &types {
        for (at, (one, id, _)) in resources.iter().enumerate() {
            if one == kind {
                map.extend_from_slice(&id.to_be_bytes());
                map.extend_from_slice(&0xFFFFu16.to_be_bytes());
                map.push(0);
                map.extend_from_slice(&offsets[at].to_be_bytes()[1..]);
                map.extend_from_slice(&[0; 4]);
            }
        }
    }
    let map_at = DATA_AT + data.len() as u32;
    let mut header = Vec::new();
    for long in [DATA_AT, map_at, data.len() as u32, map.len() as u32] {
        header.extend_from_slice(&long.to_be_bytes());
    }
    map[..16].copy_from_slice(&header);
    let mut out = header;
    out.resize(DATA_AT as usize, 0);
    out.extend_from_slice(&data);
    out.extend_from_slice(&map);
    out
}

/// The resource fork of `items`: one segment, entered at its first byte.
pub fn resource_file(items: &[Item]) -> Result<Vec<u8>, String> {
    let Assembled { code, .. } = assemble(&position_independent(items), 0)?;
    let mut segment = vec![0, 0, 0, 1];
    segment.extend_from_slice(&code);
    // CODE 0: the A5 world above and below, the jump table's size and offset,
    // then its one entry (offset, `move.w #1,-(sp)`, `_LoadSeg`)
    let mut world = Vec::new();
    for long in [ABOVE_A5 + 8, 0, 8, ABOVE_A5] {
        world.extend_from_slice(&long.to_be_bytes());
    }
    world.extend_from_slice(&SEGMENT_HEADER.to_be_bytes());
    world.extend_from_slice(&[0x3F, 0x3C, 0, 1, 0xA9, 0xF0]);
    Ok(resource_fork(&[(b"CODE", 0, world), (b"CODE", 1, segment)]))
}
