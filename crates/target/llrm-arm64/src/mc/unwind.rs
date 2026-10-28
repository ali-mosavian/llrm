//! AArch64 `.eh_frame` facts: the DWARF register numbers (x0-x30 are 0-30, sp
//! 31, d0-d31 64-95, the return address column is x30) and the frame rows a
//! function's prologue and epilogue make, read from its final instructions as
//! `AArch64FrameLowering` declares them.

use llrm_dwarf::eh_frame::EhTarget;
use llrm_object::Frame;
use llrm_object::debug::FrameRow;

use super::inst::{AddSub, Addr, Gpr, Inst, Memory, Vreg};

/// The DWARF number of a register by name.
pub fn register(name: &str) -> Option<u64> {
    if name == "sp" {
        return Some(31);
    }
    let (class, number) = name.split_at(1);
    let number: u64 = number.parse().ok().filter(|number| *number < 32)?;
    match class {
        "x" if number < 31 => Some(number),
        "d" => Some(64 + number),
        _ => None,
    }
}

/// At a function's entry the frame address is the stack pointer, and the
/// return address is in x30.
pub fn eh_target() -> EhTarget<'static> {
    EhTarget {
        address: 8,
        return_column: 30,
        // The pin's CIE: data alignment -4 and entries padded to 4, and the
        // return address stays in x30.
        data_alignment: -4,
        padding: 4,
        return_saved: false,
        entry: FrameRow { offset: 0, cfa_register: "sp".into(), cfa_offset: 0, saved: Vec::new() },
        register: &register,
    }
}

fn gpr(register: Gpr) -> String {
    if register == Gpr::R31 { "sp".into() } else { format!("x{}", register.0) }
}

fn saved_name(mem: &Memory) -> Option<String> {
    match mem {
        Memory::Int { bytes: 8, reg, .. } => Some(gpr(*reg)),
        Memory::Fp { bytes: 8, reg: Vreg(number) } => Some(format!("d{number}")),
        _ => None,
    }
}

/// Once x29 is reloaded it no longer holds the frame, and the stack pointer
/// (not yet moved) does.
fn give_back_frame_register(
    state: &mut FrameRow,
    loaded: &str,
    from_sp: i64,
) {
    if loaded == "x29" && state.cfa_register == "x29" {
        state.cfa_register = "sp".into();
        state.cfa_offset = from_sp;
    }
}

/// Whether `inst` moves the stack pointer by writeback or by a register.
fn moves_sp_unmodelled(inst: &Inst) -> bool {
    let writeback = |addr: &Addr| {
        matches!(
            addr,
            Addr::Pre { base, .. } | Addr::Post { base, .. } if *base == Gpr::R31
        )
    };
    match inst {
        Inst::Load { addr, .. } | Inst::Store { addr, .. } => writeback(addr),
        Inst::LoadPair { addr, .. } | Inst::StorePair { addr, .. } => writeback(addr),
        Inst::AddSubShifted { rd, .. } | Inst::AddSubExtended { rd, .. } => *rd == Gpr::R31,
        _ => false,
    }
}

/// The frame of the function at `start` of section `section` whose
/// instructions, one word each, are `insts`.
///
/// Rows follow the instructions: `sub sp` moves the frame address, a store of
/// a register to `[sp, #n]` saves it, `add x29, sp, #n` makes x29 the frame
/// register, and the epilogue's loads and `add sp` undo them. After a `ret`
/// the rest of the function is in the state before its epilogue. `None` where
/// the instructions move the stack pointer in a way the rows cannot say.
pub fn frame(
    section: usize,
    start: usize,
    insts: &[&Inst],
) -> Option<Frame> {
    let mut state = FrameRow { offset: 0, ..eh_target().entry };
    let mut rows = vec![state.clone()];
    let mut remembered: Option<FrameRow> = None;
    // The frame address from the stack pointer, while x29 is the frame
    // register.
    let mut from_sp = 0;
    let change = |state: &mut FrameRow, rows: &mut Vec<FrameRow>, at: usize| {
        state.offset = at;
        rows.push(state.clone());
    };
    for (index, inst) in insts.iter().enumerate() {
        let after = (index + 1) * 4;
        // A move of the stack pointer the rows below do not model (writeback
        // addressing, a register amount) would leave wrong rows: no frame.
        if moves_sp_unmodelled(inst) {
            return None;
        }
        match inst {
            Inst::AddSubImm { op: AddSub { set_flags: false, subtract }, rd, rn, imm12, shift12, .. }
                if *rd == Gpr::R31 && *rn == Gpr::R31 && state.cfa_register == "sp" =>
            {
                let amount = i64::from(*imm12) << if *shift12 { 12 } else { 0 };
                if !subtract {
                    remembered.get_or_insert_with(|| state.clone());
                }
                state.cfa_offset += if *subtract { amount } else { -amount };
                change(&mut state, &mut rows, after);
            }
            Inst::AddSubImm { op: AddSub { set_flags: false, subtract: false }, rd, rn, imm12, shift12, .. }
                if *rd == Gpr::FP && *rn == Gpr::R31 && state.cfa_register == "sp" =>
            {
                from_sp = state.cfa_offset;
                state.cfa_offset -= i64::from(*imm12) << if *shift12 { 12 } else { 0 };
                state.cfa_register = "x29".into();
                change(&mut state, &mut rows, after);
            }
            Inst::AddSubImm { op: AddSub { set_flags: false, subtract: false }, rd, rn, imm12, shift12, .. }
                if *rd == Gpr::R31 && *rn == Gpr::FP && state.cfa_register == "x29" =>
            {
                remembered.get_or_insert_with(|| state.clone());
                state.cfa_offset -= i64::from(*imm12) << if *shift12 { 12 } else { 0 };
                state.cfa_register = "sp".into();
                change(&mut state, &mut rows, after);
            }
            Inst::Store { mem, addr: Addr::Offset { base, imm } } if *base == Gpr::R31 => {
                if let Some(name) = saved_name(mem).filter(|_| state.cfa_register == "sp") {
                    state.saved.push((name, imm - state.cfa_offset));
                    change(&mut state, &mut rows, after);
                }
            }
            Inst::StorePair { first, second, addr: Addr::Offset { base, imm } } if *base == Gpr::R31 => {
                if state.cfa_register == "sp" {
                    for (mem, at) in [(first, *imm), (second, imm + 8)] {
                        if let Some(name) = saved_name(mem) {
                            state.saved.push((name, at - state.cfa_offset));
                        }
                    }
                    change(&mut state, &mut rows, after);
                }
            }
            Inst::Load { mem, .. } => {
                remembered.get_or_insert_with(|| state.clone());
                if let Some(name) = saved_name(mem) {
                    state.saved.retain(|(saved, _)| *saved != name);
                    give_back_frame_register(&mut state, &name, from_sp);
                    change(&mut state, &mut rows, after);
                }
            }
            Inst::LoadPair { first, second, .. } => {
                remembered.get_or_insert_with(|| state.clone());
                for name in [first, second].into_iter().filter_map(saved_name) {
                    state.saved.retain(|(saved, _)| *saved != name);
                    give_back_frame_register(&mut state, &name, from_sp);
                }
                change(&mut state, &mut rows, after);
            }
            Inst::AddSubImm { rd, .. } if *rd == Gpr::R31 && state.cfa_register == "sp" => return None,
            Inst::Ret { .. } => {
                if let Some(before) = remembered.take() {
                    state = before;
                    if after < insts.len() * 4 {
                        change(&mut state, &mut rows, after);
                    }
                }
            }
            _ => {}
        }
    }
    Some(Frame { section, offset: start, length: insts.len() * 4, rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::inst::Width;

    fn add_sub(
        subtract: bool,
        rd: Gpr,
        amount: u16,
    ) -> Inst {
        Inst::AddSubImm {
            op: AddSub { subtract, set_flags: false },
            width: Width::X,
            rd,
            rn: Gpr::R31,
            imm12: amount,
            shift12: false,
        }
    }

    fn sub_sp(amount: u16) -> Inst {
        add_sub(true, Gpr::R31, amount)
    }

    fn add_sp(amount: u16) -> Inst {
        add_sub(false, Gpr::R31, amount)
    }

    fn pair(
        store: bool,
        offset: i64,
    ) -> Inst {
        let reg = |reg| Memory::Int { width: Width::X, bytes: 8, sign: false, reg };
        let addr = Addr::Offset { base: Gpr::R31, imm: offset };
        if store {
            Inst::StorePair { first: reg(Gpr::FP), second: reg(Gpr::LR), addr }
        } else {
            Inst::LoadPair { first: reg(Gpr::FP), second: reg(Gpr::LR), addr }
        }
    }

    /// `sub sp,#32; stp x29,x30,[sp]; add x29,sp,#0; ldp; add sp,#32; ret`.
    #[test]
    fn a_frame_record_is_saved_found_and_given_back() {
        let make_fp = add_sub(false, Gpr::FP, 0);
        let insts = [sub_sp(32), pair(true, 0), make_fp, pair(false, 0), add_sp(32), Inst::Ret { rn: Gpr::LR }];
        let frame = frame(0, 0, &insts.iter().collect::<Vec<_>>()).expect("a frame");
        let row = |offset, register: &str, cfa_offset, saved: &[(&str, i64)]| FrameRow {
            offset,
            cfa_register: register.into(),
            cfa_offset,
            saved: saved.iter().map(|(name, at)| ((*name).into(), *at)).collect(),
        };
        assert_eq!(frame.length, 24);
        assert_eq!(
            frame.rows,
            [
                row(0, "sp", 0, &[]),
                row(4, "sp", 32, &[]),
                row(8, "sp", 32, &[("x29", -32), ("x30", -24)]),
                row(12, "x29", 32, &[("x29", -32), ("x30", -24)]),
                row(16, "sp", 32, &[]),
                row(20, "sp", 0, &[]),
            ]
        );
    }

    /// `stp x29,x30,[sp,#-16]!` moves the stack pointer by writeback, which the
    /// rows would have missed: a frame with wrong rows is worse than none.
    #[test]
    fn a_writeback_move_of_the_stack_pointer_gives_no_frame() {
        let reg = |reg| Memory::Int { width: Width::X, bytes: 8, sign: false, reg };
        let push =
            Inst::StorePair { first: reg(Gpr::FP), second: reg(Gpr::LR), addr: Addr::Pre { base: Gpr::R31, imm: -16 } };
        assert!(frame(0, 0, &[&push, &Inst::Ret { rn: Gpr::LR }]).is_none());
    }

    /// The `.eh_frame` the pinned clang writes for `int main(void) { return 42;
    /// }` (`clang -target aarch64-unknown-linux-gnu -O2 -c`): the CIE has
    /// data alignment -4 and no saved return address, and entries are
    /// padded to 4. The writer gave -8, an `offset x30` rule at the frame
    /// address, and padded to 8, and refused any rule on d8 (register 72,
    /// past the short form).
    #[test]
    fn the_eh_frame_of_a_leaf_is_what_the_pins_clang_writes() {
        use llrm_object::{Frame, Object, Role, Section};
        let object = |rows: Vec<FrameRow>| Object {
            name: "a.c".into(),
            bits: 64,
            sections: vec![Section {
                name: ".text".into(),
                role: Role::Text,
                near: true,
                align: 4,
                image: vec![0; 8],
                spans: vec![[0, 8]],
                relocs: Vec::new(),
            }],
            symbols: Vec::new(),
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            debug: None,
            frames: vec![Frame { section: 0, offset: 0, length: 8, rows }],
        };
        let image = |object: &Object| {
            let object = llrm_dwarf::eh_frame::expanded(object, &eh_target()).expect("expands");
            object.sections.iter().find(|one| one.name == ".eh_frame").expect("an .eh_frame").image.clone()
        };
        let leaf = image(&object(vec![eh_target().entry]));
        let pin = [
            // CIE
            0x10u8, 0, 0, 0, 0, 0, 0, 0, 1, b'z', b'R', 0, 1, 0x7C, 0x1E, 1, 0x1B, 0x0C, 0x1F, 0, // FDE
            0x10, 0, 0, 0, 0x18, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(leaf, pin);
        // d8 saved 16 below the frame address: DW_CFA_offset_extended.
        let saved = FrameRow { offset: 4, saved: vec![("d8".into(), -16)], ..eh_target().entry };
        let bytes = image(&object(vec![eh_target().entry, saved]));
        assert!(bytes.windows(3).any(|window| window == [0x05, 72, 4]), "{bytes:?}");
    }

    #[test]
    fn registers_are_numbered_as_the_aarch64_abi_does() {
        assert_eq!(
            [register("x0"), register("x30"), register("sp"), register("d8")],
            [Some(0), Some(30), Some(31), Some(72)]
        );
        assert_eq!([register("x31"), register("d32"), register("w0")], [None, None, None]);
    }
}
