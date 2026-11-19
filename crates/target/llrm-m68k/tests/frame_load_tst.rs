//! `move.l (84,%sp), %d0; tst.l %d0; beq`: the load already sets the codes. The
//! flag fix for a made-again argument load (it no longer counts as a setter)
//! kept this `tst`: spill_store_order +2 B at every level.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const PACK_REF: &str = include_str!("fixtures/pack_ref.ll");

#[test]
fn a_frame_load_beside_its_branch_needs_no_tst() {
    let module = llrm_mir::parse::module(PACK_REF).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = text_with(&module, "pack_ref", cpu, Options::default()).expect("compiles");
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for pair in lines.windows(2) {
            let tested = pair[1].strip_prefix("tst.l\t");
            let loaded =
                pair[0].strip_prefix("move.l\t(").and_then(|rest| rest.split_once(",%sp), ")).map(|(_, to)| to);
            assert!(tested.is_none() || tested != loaded, "{cpu:?}: `{}` then `{}`\n{text}", pair[0], pair[1]);
        }
    }
}
