//! An Atari ST/TT `.PRG` (TOS executable): a 28-byte header, the text, and a
//! relocation table, since the loader moves the program to any address.
//!
//! The header is `$601A`, text, data, bss and symbol sizes (longs), a reserved
//! long, the program flags (long) and the absolute-flag word (0: relocate).
//! The table is the offset of the first relocated long, then one byte per next
//! one: the distance to it (2..254), `1` meaning "254 further, relocate
//! nothing", and a final `0`; no relocations is a single zero long.
//!
//! The program ends with GEMDOS `Pterm(code)`: the code word, then function
//! `$4C`, `trap #1`.

use crate::compile::Item;
use crate::mc::Operand;
use crate::object::{Assembled, assemble, call, item};
use crate::reg;

const PTERM: i64 = 0x4C;

/// `_start`: [`call`], then `Pterm` with the result.
pub fn start(
    callee: &str,
    args: &[i64],
) -> Vec<Item> {
    let mut out = vec![Item::Label("_start".to_owned())];
    out.extend(call(callee, args));
    out.push(item("MOV16ed", vec![Operand::Reg(reg::SP), Operand::Reg(reg::WD0)]));
    out.push(item("MOV16ei", vec![Operand::Reg(reg::SP), Operand::Imm(PTERM)]));
    out.push(item("TRAP", vec![Operand::Imm(1)]));
    out
}

/// The relocation table for the long words at `offsets` (ascending).
fn relocations(offsets: &[usize]) -> Vec<u8> {
    let Some(first) = offsets.first() else { return vec![0; 4] };
    let mut out = (*first as u32).to_be_bytes().to_vec();
    for pair in offsets.windows(2) {
        let mut distance = pair[1] - pair[0];
        while distance > 254 {
            out.push(1);
            distance -= 254;
        }
        out.push(distance as u8);
    }
    out.push(0);
    out
}

/// The `.PRG` of `items`, entered at their first instruction.
pub fn program(items: &[Item]) -> Result<Vec<u8>, String> {
    let Assembled { mut code, mut absolute, .. } = assemble(items, 0)?;
    absolute.sort_unstable();
    if code.len() % 2 != 0 {
        code.push(0);
    }
    let mut out = 0x601Au16.to_be_bytes().to_vec();
    for long in [code.len() as u32, 0, 0, 0, 0, 0] {
        out.extend_from_slice(&long.to_be_bytes());
    }
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&code);
    out.extend_from_slice(&relocations(&absolute));
    Ok(out)
}
