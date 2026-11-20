//! Every price of an instruction was the encoder's size and a manual-table
//! lookup of the printed text, again for each build, candidate and trial:
//! `frame_cost_with` alone was 2380 calls and 3.7% of grep -O2's compile.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::expand::frame_cost;
use llrm_m68k::frame::FrameOp;
use llrm_m68k::timing::keys_made;

#[test]
fn an_instruction_priced_twice_is_looked_up_once() {
    let ops = [FrameOp::SubSp(24), FrameOp::AddSp(24)];
    let first = frame_cost(&ops, Cpu::M68020).expect("priced");
    let before = keys_made();
    let again = frame_cost(&ops, Cpu::M68020).expect("priced");
    let made = keys_made() - before;
    assert_eq!(first, again);
    assert_eq!(made, 0, "{made} timing labels normalized for instructions priced already");
}
