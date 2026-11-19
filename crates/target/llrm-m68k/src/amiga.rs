//! An AmigaOS load file (hunk format): `HUNK_HEADER` for one hunk,
//! `HUNK_CODE`, `HUNK_RELOC32` for the absolute addresses (the loader adds the
//! hunk's base), `HUNK_END`. All words are big-endian longs.
//!
//! The program returns to its caller with the result in `d0`, which is the
//! process's return code.

use crate::compile::Item;
use crate::object::{Assembled, assemble, call, item};

const HUNK_HEADER: u32 = 0x3F3;
const HUNK_CODE: u32 = 0x3E9;
const HUNK_RELOC32: u32 = 0x3EC;
const HUNK_END: u32 = 0x3F2;

/// `_start`: [`call`], then `rts`.
pub fn start(
    callee: &str,
    args: &[i64],
) -> Vec<Item> {
    let mut out = vec![Item::Label("_start".to_owned())];
    out.extend(call(callee, args));
    out.push(item("RTS", vec![]));
    out
}

/// The load file of `items`, one code hunk entered at its first byte.
pub fn load_file(items: &[Item]) -> Result<Vec<u8>, String> {
    let Assembled { mut code, absolute, .. } = assemble(items, 0)?;
    code.resize(code.len().div_ceil(4) * 4, 0);
    let longs = (code.len() / 4) as u32;
    let mut words = vec![HUNK_HEADER, 0, 1, 0, 0, longs, HUNK_CODE, longs];
    let mut out: Vec<u8> = words.drain(..).flat_map(u32::to_be_bytes).collect();
    out.extend_from_slice(&code);
    let mut tail = Vec::new();
    if !absolute.is_empty() {
        tail.extend([HUNK_RELOC32, absolute.len() as u32, 0]);
        tail.extend(absolute.iter().map(|at| *at as u32));
        tail.push(0);
    }
    tail.push(HUNK_END);
    out.extend(tail.into_iter().flat_map(u32::to_be_bytes));
    Ok(out)
}
