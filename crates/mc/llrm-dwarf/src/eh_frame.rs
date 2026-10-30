//! `.eh_frame`: the call frame information the unwinder reads at run time, in
//! the form LLVM's `MCDwarfFrameEmitter` writes for ELF: one CIE (augmentation
//! `zR`, FDE pointers pc-relative `sdata4`) and an FDE for each function of
//! `Object::frames`.

use llrm_object::debug::FrameRow;
use llrm_object::{Fix, Object, Reloc, Role, Section, Target, Unsupported};

use crate::buffer::Buf;
use crate::frame::{advance, changes};

const DW_CFA_DEF_CFA: u8 = 0x0C;
const DW_CFA_OFFSET: u8 = 0x80;
/// `DW_EH_PE_pcrel | DW_EH_PE_sdata4`.
const FDE_ENCODING: u8 = 0x1B;

/// What the target says of its frames.
pub struct EhTarget<'a> {
    /// The bytes of an address, which a stack slot also is.
    pub address: usize,
    /// The column of the return address.
    pub return_column: u64,
    /// Whether the return address is in memory at entry (a call pushes it);
    /// otherwise it is in the register of `return_column`.
    pub return_saved: bool,
    /// The bytes an advance of the location is counted in.
    pub code_alignment: u64,
    /// The rule at a function's entry: the frame address and where the return
    /// address is.
    pub entry: FrameRow,
    /// A register's DWARF number.
    pub register: &'a dyn Fn(&str) -> Option<u64>,
}

/// `object` with its `.eh_frame` added.
pub fn expanded(
    object: &Object,
    target: &EhTarget,
) -> Result<Object, Unsupported> {
    Ok(expanded_as(object, target, ".eh_frame")?.0)
}

/// `object` with the section `name` added (`__eh_frame` in Mach-O), and the
/// offset in it of each frame's FDE, which a compact unwind entry can name.
pub fn expanded_as(
    object: &Object,
    target: &EhTarget,
    name: &str,
) -> Result<(Object, Vec<usize>), Unsupported> {
    let number = |name: &str| {
        (target.register)(name)
            .ok_or_else(|| Unsupported(format!("DWARF: register {name} has no number, which a frame rule names")))
    };
    let alignment = -(target.address as i64);
    let mut buf = Buf::default();
    buf.u32(0);
    buf.u32(0);
    buf.u8(1);
    buf.string("zR");
    buf.uleb(target.code_alignment);
    buf.sleb(alignment);
    buf.u8(target.return_column as u8);
    buf.uleb(1);
    buf.u8(FDE_ENCODING);
    buf.u8(DW_CFA_DEF_CFA);
    buf.uleb(number(&target.entry.cfa_register)?);
    buf.uleb(target.entry.cfa_offset as u64);
    if target.return_saved {
        buf.u8(DW_CFA_OFFSET | target.return_column as u8);
        buf.uleb(target.entry.cfa_offset as u64 / target.address as u64);
    }
    pad(&mut buf, target.address);
    buf.patch32(0, buf.at() as u32 - 4);
    let initial = FrameRow { offset: 0, saved: Vec::new(), ..target.entry.clone() };
    let mut entries = Vec::new();
    for frame in &object.frames {
        let start = buf.at();
        entries.push(start);
        buf.u32(0);
        buf.u32((buf.at() - 0) as u32);
        let field = buf.at();
        buf.relocs.push(Reloc::new(field, Fix::pc_rel(4, 0), Target::Section(frame.section), frame.offset as i64));
        buf.u32(0);
        buf.u32(frame.length as u32);
        buf.uleb(0);
        let mut state = &initial;
        for row in &frame.rows {
            advance(&mut buf, (row.offset - state.offset.min(row.offset)) / target.code_alignment as usize);
            changes(&number, &mut buf, state, row, alignment)?;
            state = row;
        }
        pad(&mut buf, target.address);
        buf.patch32(start, (buf.at() - start - 4) as u32);
    }
    let part = buf.done();
    let mut sections = object.sections.clone();
    sections.push(Section {
        name: name.to_owned(),
        role: Role::Unwind,
        near: true,
        align: target.address,
        spans: vec![[0, part.bytes.len()]],
        image: part.bytes,
        relocs: part.relocs,
    });
    Ok((Object { sections, frames: Vec::new(), ..object.clone() }, entries))
}

/// `DW_CFA_nop` to the next multiple of `to`.
fn pad(
    buf: &mut Buf,
    to: usize,
) {
    while buf.at() % to != 0 {
        buf.u8(0);
    }
}
