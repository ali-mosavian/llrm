//! The price of an instruction: the encoder's bytes and the manuals' clocks
//! for its printed text, once per instruction.
//!
//! Each build, call-target candidate and trial priced the same instructions
//! again, through the encoder and a table lookup of the printed text.

use std::cell::RefCell;

use llrm_support::hash::HashMap;

use crate::dagisel::hooks::Cpu;
use crate::mc::{self, Inst};

#[derive(Default)]
struct Memo {
    sizes: HashMap<Inst, usize>,
    /// Per CPU, an instruction's clocks at each size it was priced at.
    clocks: [HashMap<Inst, Vec<(u32, u32)>>; 6],
}

thread_local! {
    static MEMO: RefCell<Memo> = RefCell::new(Memo::default());
}

/// The bytes `inst` encodes to.
pub fn size_of(inst: &Inst) -> Result<usize, String> {
    if let Some(&size) = MEMO.with(|memo| memo.borrow().sizes.get(inst).copied()).as_ref() {
        return Ok(size);
    }
    let size = crate::encode::encode(inst)?.bytes.len();
    MEMO.with(|memo| memo.borrow_mut().sizes.insert(inst.clone(), size));
    Ok(size)
}

/// The clocks of `inst` at `size` bytes on `cpu`, not taken.
pub fn clocks_of(
    cpu: Cpu,
    inst: &Inst,
    size: u32,
) -> Result<u32, String> {
    let known = MEMO.with(|memo| {
        let memo = memo.borrow();
        memo.clocks[cpu as usize].get(inst)?.iter().find(|(at, _)| *at == size).map(|(_, clocks)| *clocks)
    });
    if let Some(clocks) = known {
        return Ok(clocks);
    }
    let clocks = crate::timing::cost(cpu, &mc::print(inst)?, size, false)?.clocks;
    MEMO.with(|memo| memo.borrow_mut().clocks[cpu as usize].entry(inst.clone()).or_default().push((size, clocks)));
    Ok(clocks)
}

/// Bytes and clocks of `inst` on `cpu`.
pub fn price_of(
    cpu: Cpu,
    inst: &Inst,
) -> Result<(usize, u32), String> {
    let size = size_of(inst)?;
    Ok((size, clocks_of(cpu, inst, size as u32)?))
}
