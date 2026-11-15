//! `cost_on_counted` parsed the instruction text and walked the timing table on
//! every ask: `extension_cost` 72 times (2.2 M), `switchprice::one` 328 times
//! (1.5 M) of grep's 126 M compile instructions at -O2 68020, the same answer
//! each time.

use llrm_m68k::isa::Cpu;
use llrm_m68k::timing::{cost_on, priced_texts};

#[test]
fn an_instruction_text_is_priced_once_however_often_it_is_asked_for() {
    let before = priced_texts();
    for _ in 0..50 {
        let _ = cost_on(Cpu::M68020, "moveq #0, %d0", 2, false);
        let _ = cost_on(Cpu::M68020, "move.w %d1, %d0", 2, false);
    }
    let priced = priced_texts() - before;
    assert!(priced <= 2, "{priced} prices for 2 distinct texts asked 100 times");
}
