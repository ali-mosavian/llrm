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
    /// The CIE's data alignment factor: a negative slot size.
    pub data_alignment: i64,
    /// What entries are padded to with `DW_CFA_nop`.
    pub padding: usize,
    /// Whether the return address is on the stack at entry (not in a link
    /// register).
    pub return_saved: bool,
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
    let number = |name: &str| {
        (target.register)(name)
            .ok_or_else(|| Unsupported(format!("DWARF: register {name} has no number, which a frame rule names")))
    };
    let alignment = target.data_alignment;
    let mut buf = Buf::default();
    buf.u32(0);
    buf.u32(0);
    buf.u8(1);
    buf.string("zR");
    buf.uleb(1);
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
    pad(&mut buf, target.padding);
    buf.patch32(0, buf.at() as u32 - 4);
    let initial = FrameRow { offset: 0, saved: Vec::new(), ..target.entry.clone() };
    for frame in &object.frames {
        let start = buf.at();
        buf.u32(0);
        buf.u32((buf.at() - 0) as u32);
        let field = buf.at();
        buf.relocs.push(Reloc::new(field, Fix::pc_rel(4, 0), Target::Section(frame.section), frame.offset as i64));
        buf.u32(0);
        buf.u32(frame.length as u32);
        buf.uleb(0);
        let mut state = &initial;
        for row in &frame.rows {
            advance(&mut buf, row.offset - state.offset.min(row.offset));
            changes(&number, &mut buf, state, row, alignment)?;
            state = row;
        }
        pad(&mut buf, target.padding);
        buf.patch32(start, (buf.at() - start - 4) as u32);
    }
    let part = buf.done();
    let mut sections = object.sections.clone();
    sections.push(Section {
        name: ".eh_frame".to_owned(),
        role: Role::Unwind,
        near: true,
        align: target.address,
        spans: vec![[0, part.bytes.len()]],
        image: part.bytes,
        relocs: part.relocs,
    });
    Ok(Object { sections, frames: Vec::new(), ..object.clone() })
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
